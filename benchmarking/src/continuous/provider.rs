#[path = "../common/mod.rs"]
mod common;

use common::continuous_types::{
    ContinuousBenchmarkConfig, ContinuousBenchmarkResults, ContinuousData,
    ContinuousSystemMetricsSample, continuous_current_timestamp_ns,
    parse_continuous_config_from_args,
};
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use dust_dds::std_runtime::StdRuntime;
use mycellium_computing::core::application::Application;
use mycellium_computing::provides;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};

/// Global state for tracking provider metrics
static MESSAGES_PUBLISHED: AtomicU64 = AtomicU64::new(0);
static TOTAL_BYTES_PUBLISHED: AtomicU64 = AtomicU64::new(0);

/// Provider implementation for continuous benchmarking
/// The continuous handle will be used to publish data
#[provides(StdRuntime, [
    Continuous("benchmark_stream", ContinuousData)
])]
struct ContinuousBenchmarkProvider;

// No trait implementation needed for continuous-only providers
// The data is published via the ContinuousHandle

/// Collect system metrics sample
fn collect_system_metrics(sys: &mut System, pid: Pid) -> ContinuousSystemMetricsSample {
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    sys.refresh_memory();

    let process = sys.process(pid);
    let cpu_usage = process.map(|p| p.cpu_usage() as f64).unwrap_or(0.0);
    let memory_usage = process.map(|p| p.memory()).unwrap_or(0);
    let total_memory = sys.total_memory();
    let memory_percent = if total_memory > 0 {
        (memory_usage as f64 / total_memory as f64) * 100.0
    } else {
        0.0
    };

    ContinuousSystemMetricsSample {
        timestamp_ns: continuous_current_timestamp_ns(),
        cpu_usage_percent: cpu_usage,
        memory_usage_bytes: memory_usage,
        total_memory_bytes: total_memory,
        memory_usage_percent: memory_percent,
    }
}

/// Metrics collector task
fn start_metrics_collector(
    config: &ContinuousBenchmarkConfig,
    samples: Arc<Mutex<Vec<ContinuousSystemMetricsSample>>>,
    running: Arc<AtomicBool>,
) {
    let interval_ms = config.metrics_sample_interval_ms;

    std::thread::spawn(move || {
        let mut sys = System::new_all();
        let pid = Pid::from_u32(std::process::id());

        // Initial refresh to get accurate CPU readings
        sys.refresh_all();
        std::thread::sleep(Duration::from_millis(100));

        while running.load(Ordering::SeqCst) {
            let sample = collect_system_metrics(&mut sys, pid);
            if let Ok(mut guard) = samples.lock() {
                guard.push(sample);
            }
            std::thread::sleep(Duration::from_millis(interval_ms));
        }
    });
}

/// Generate payload of specified size
fn generate_payload(size: u32) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Rate limiter to achieve target PPS (publications per second)
struct RateLimiter {
    target_pps: u64,
    interval_ns: u64,
    last_publish_time: Instant,
    tokens: f64,
}

impl RateLimiter {
    fn new(target_pps: u64) -> Self {
        let interval_ns = if target_pps > 0 {
            1_000_000_000 / target_pps
        } else {
            0
        };
        Self {
            target_pps,
            interval_ns,
            last_publish_time: Instant::now(),
            tokens: 1.0,
        }
    }

    async fn wait_for_token(&mut self) {
        if self.target_pps == 0 {
            // Unlimited rate
            return;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_publish_time);

        // Add tokens based on elapsed time
        let tokens_to_add = elapsed.as_secs_f64() * self.target_pps as f64;
        self.tokens = (self.tokens + tokens_to_add).min(self.target_pps as f64);
        self.last_publish_time = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
        } else {
            // Wait for next token
            let wait_time_ns = self.interval_ns as f64 * (1.0 - self.tokens);
            smol::Timer::after(Duration::from_nanos(wait_time_ns as u64)).await;
            self.tokens = 0.0;
            self.last_publish_time = Instant::now();
        }
    }
}

async fn run_provider(config: ContinuousBenchmarkConfig) {
    println!("===========================================");
    println!("    Continuous Benchmark - PROVIDER");
    println!("===========================================");
    println!("Configuration:");
    println!("  Domain ID: {}", config.domain_id);
    println!("  Duration: {} seconds", config.duration_secs);
    println!("  Target PPS: {} (0 = unlimited)", config.target_pps);
    println!("  Payload size: {} bytes", config.payload_size);
    println!(
        "  Metrics sample interval: {} ms",
        config.metrics_sample_interval_ms
    );
    println!("  Output file: {}", config.output_file);
    println!("===========================================\n");

    // Initialize DDS
    let factory = DomainParticipantFactoryAsync::get_instance();
    let mut app = Application::new(config.domain_id, "ContinuousBenchmarkProvider", factory).await;

    // Register provider and get the continuous handle for publishing
    let continuous_handle = app.register_provider::<ContinuousBenchmarkProvider>().await;
    println!("[Provider] Registered and ready to publish data");

    // Wait for consumers to connect
    println!("[Provider] Waiting for consumers (3 seconds)...");
    smol::Timer::after(Duration::from_secs(3)).await;

    // Initialize metrics collection
    let samples: Arc<Mutex<Vec<ContinuousSystemMetricsSample>>> = Arc::new(Mutex::new(Vec::new()));
    let running = Arc::new(AtomicBool::new(true));

    // Start metrics collector
    start_metrics_collector(&config, samples.clone(), running.clone());

    // Record start time
    let start_time_ns = continuous_current_timestamp_ns();
    let benchmark_start = Instant::now();
    println!(
        "[Provider] Benchmark started at timestamp: {}",
        start_time_ns
    );

    // Reset counters
    MESSAGES_PUBLISHED.store(0, Ordering::SeqCst);
    TOTAL_BYTES_PUBLISHED.store(0, Ordering::SeqCst);

    // Progress reporting task
    let duration_secs = config.duration_secs;
    let progress_running = running.clone();
    let progress_handle = std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut last_published = 0u64;
        let mut last_bytes = 0u64;
        let mut last_time = start;

        while progress_running.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));

            let elapsed = start.elapsed().as_secs();
            let current_published = MESSAGES_PUBLISHED.load(Ordering::SeqCst);
            let current_bytes = TOTAL_BYTES_PUBLISHED.load(Ordering::SeqCst);
            let now = std::time::Instant::now();
            let interval_duration = now.duration_since(last_time).as_secs_f64();

            let instant_pps = if interval_duration > 0.0 {
                (current_published - last_published) as f64 / interval_duration
            } else {
                0.0
            };

            let instant_throughput_mb = if interval_duration > 0.0 {
                ((current_bytes - last_bytes) as f64 / interval_duration) / 1_048_576.0
            } else {
                0.0
            };

            let overall_pps = if elapsed > 0 {
                current_published as f64 / elapsed as f64
            } else {
                0.0
            };

            let overall_throughput_mb = if elapsed > 0 {
                (current_bytes as f64 / elapsed as f64) / 1_048_576.0
            } else {
                0.0
            };

            println!(
                "[Provider] Progress: {}/{} sec | Published: {} | PPS: {:.2} (avg {:.2}) | Throughput: {:.2} MB/s (avg {:.2} MB/s)",
                elapsed.min(duration_secs),
                duration_secs,
                current_published,
                instant_pps,
                overall_pps,
                instant_throughput_mb,
                overall_throughput_mb
            );

            last_published = current_published;
            last_bytes = current_bytes;
            last_time = now;

            if elapsed >= duration_secs {
                break;
            }
        }
    });

    // Create rate limiter
    let mut rate_limiter = RateLimiter::new(config.target_pps);

    // Pre-generate payload
    let payload = generate_payload(config.payload_size);

    // Publish messages
    let mut sequence_id: u64 = 0;
    while benchmark_start.elapsed().as_secs() < config.duration_secs {
        // Rate limiting
        rate_limiter.wait_for_token().await;

        // Check if benchmark is still running
        if benchmark_start.elapsed().as_secs() >= config.duration_secs {
            break;
        }

        sequence_id += 1;
        let timestamp_ns = continuous_current_timestamp_ns();

        let data = ContinuousData {
            sequence_id,
            timestamp_ns,
            payload_size: config.payload_size,
            payload: payload.clone(),
        };

        // Publish via the continuous handle
        continuous_handle.benchmark_stream(&data).await;

        MESSAGES_PUBLISHED.fetch_add(1, Ordering::SeqCst);
        TOTAL_BYTES_PUBLISHED.fetch_add(config.payload_size as u64, Ordering::SeqCst);
    }

    // Stop metrics collection
    running.store(false, Ordering::SeqCst);
    let end_time_ns = continuous_current_timestamp_ns();

    // Wait for progress thread to finish
    let _ = progress_handle.join();

    // Collect final metrics
    let total_published = MESSAGES_PUBLISHED.load(Ordering::SeqCst);
    let total_bytes = TOTAL_BYTES_PUBLISHED.load(Ordering::SeqCst);

    println!("\n[Provider] Benchmark completed!");
    println!("  Total messages published: {}", total_published);
    println!(
        "  Total bytes published: {} ({:.2} MB)",
        total_bytes,
        total_bytes as f64 / 1_048_576.0
    );

    // Build results
    let mut results = ContinuousBenchmarkResults::new(config.clone(), "provider");
    results.start_time_ns = start_time_ns;
    results.end_time_ns = end_time_ns;
    results.total_messages_sent = total_published;

    // Copy system metrics samples
    if let Ok(guard) = samples.lock() {
        results.system_metrics_samples = guard.clone();
    }

    // Don't include per-message metrics for provider (too much data, not meaningful)
    results.message_metrics = None;

    // Calculate derived metrics
    results.finalize();

    // Override throughput with actual bytes published
    if results.total_duration_secs > 0.0 {
        results.throughput_bytes_per_sec = total_bytes as f64 / results.total_duration_secs;
    }

    // Print summary
    println!("\n===========================================");
    println!("       PROVIDER RESULTS SUMMARY");
    println!("===========================================");
    println!("Duration: {:.2} seconds", results.total_duration_secs);
    println!("Total Messages Published: {}", results.total_messages_sent);
    println!("Messages/Second: {:.2}", results.messages_per_second);
    println!("-------------------------------------------");
    println!("CPU Usage (avg): {:.2}%", results.avg_cpu_usage_percent);
    println!("CPU Usage (max): {:.2}%", results.max_cpu_usage_percent);
    println!("CPU Usage (min): {:.2}%", results.min_cpu_usage_percent);
    println!("-------------------------------------------");
    println!(
        "Memory Usage (avg): {:.2} MB",
        results.avg_memory_usage_bytes as f64 / 1_048_576.0
    );
    println!(
        "Memory Usage (max): {:.2} MB",
        results.max_memory_usage_bytes as f64 / 1_048_576.0
    );
    println!(
        "Memory Usage (min): {:.2} MB",
        results.min_memory_usage_bytes as f64 / 1_048_576.0
    );
    println!(
        "Memory Usage (avg %): {:.2}%",
        results.avg_memory_usage_percent
    );
    println!("-------------------------------------------");
    println!(
        "Throughput: {:.2} MB/s",
        results.throughput_bytes_per_sec / 1_048_576.0
    );
    println!("===========================================\n");

    // Save results to JSON
    if let Err(e) = results.save_to_file() {
        eprintln!("[Provider] Failed to save results: {}", e);
    }
}

fn main() {
    let config = parse_continuous_config_from_args();

    // Set default output file for provider if not specified
    let config = if config.output_file == "continuous_benchmark_results.json" {
        ContinuousBenchmarkConfig {
            output_file: "continuous_provider_results.json".to_string(),
            ..config
        }
    } else {
        config
    };

    smol::block_on(run_provider(config));
}
