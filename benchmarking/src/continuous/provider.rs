#[path = "../common/mod.rs"]
mod common;

use common::continuous_types::{
    ContinuousBenchmarkConfig, ContinuousData, parse_continuous_config_from_args,
};
use common::metrics_collector::{AggregatedMetrics, MetricsCollector, MetricsCollectorConfig};
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use dust_dds::std_runtime::StdRuntime;
use mycellium_computing::core::application::Application;
use mycellium_computing::provides;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Append a log entry to the log file if configured
fn append_to_log(log_file: &Option<String>, message: &str) {
    if let Some(path) = log_file {
        match OpenOptions::new().append(true).create(true).open(path) {
            Ok(mut file) => {
                if let Err(e) = writeln!(file, "{}", message) {
                    eprintln!("[Provider] Failed to write to log file: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[Provider] Failed to open log file: {}", e);
            }
        }
    }
}

/// Provider implementation for continuous benchmarking
/// The continuous handle will be used to publish data
#[provides(StdRuntime, [
    Continuous("benchmark_stream", ContinuousData)
])]
struct ContinuousBenchmarkProvider;

// No trait implementation needed for continuous-only providers
// The data is published via the ContinuousHandle

/// Generate payload of specified size
fn generate_payload(size: u32) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Rate limiter to achieve target PPS (publications per second)
/// Uses token bucket with max 1 token to prevent bursts
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

        // Add tokens based on elapsed time (max 1.0 to prevent bursts)
        let tokens_to_add = elapsed.as_secs_f64() * self.target_pps as f64;
        self.tokens = (self.tokens + tokens_to_add).min(1.0);
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
    println!("       (Low-Overhead Metrics Collection)");
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

    // Initialize the low-overhead metrics collector
    let metrics_config = MetricsCollectorConfig {
        duration_secs: config.duration_secs,
        sample_interval_ms: config.metrics_sample_interval_ms,
        latency_reservoir_size: 1_000, // Provider doesn't measure latency, smaller buffer
        track_latencies: false,        // Provider doesn't track latencies
    };

    let mut metrics_collector = MetricsCollector::new(metrics_config);

    // Capture baseline memory BEFORE initializing DDS
    println!("[Provider] Capturing baseline memory...");
    metrics_collector.capture_baseline();

    // Get shared counters (lock-free)
    let counters = metrics_collector.get_counters();

    // Initialize DDS
    let factory = DomainParticipantFactoryAsync::get_instance();
    let mut app = Application::new(config.domain_id, "ContinuousBenchmarkProvider", factory).await;

    // Register provider and get the continuous handle for publishing
    let continuous_handle = app.register_provider::<ContinuousBenchmarkProvider>().await;
    println!("[Provider] Registered and ready to publish data");

    // Wait for consumers to connect
    println!("[Provider] Waiting for consumers (3 seconds)...");
    smol::Timer::after(Duration::from_secs(3)).await;

    // Start metrics collection in background thread
    let metrics_handle = metrics_collector.start();
    println!("[Provider] Metrics collection started");

    let benchmark_start = Instant::now();
    println!(
        "[Provider] Benchmark started at timestamp: {}",
        MetricsCollector::current_timestamp_ns()
    );

    // Progress reporting (separate from measurement to avoid interference)
    let running = Arc::new(AtomicBool::new(true));
    let progress_running = running.clone();
    let progress_counters = counters.clone();
    let duration_secs = config.duration_secs;

    let progress_handle = std::thread::spawn(move || {
        let start = Instant::now();
        let mut last_published = 0u64;
        let mut last_bytes = 0u64;
        let mut last_time = start;

        while progress_running.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));

            let elapsed = start.elapsed().as_secs();
            let snapshot = progress_counters.snapshot();
            let now = Instant::now();
            let interval_duration = now.duration_since(last_time).as_secs_f64();

            let instant_pps = if interval_duration > 0.0 {
                (snapshot.messages_sent - last_published) as f64 / interval_duration
            } else {
                0.0
            };

            let instant_throughput_mb = if interval_duration > 0.0 {
                ((snapshot.bytes_sent - last_bytes) as f64 / interval_duration) / 1_048_576.0
            } else {
                0.0
            };

            let overall_pps = if elapsed > 0 {
                snapshot.messages_sent as f64 / elapsed as f64
            } else {
                0.0
            };

            let overall_throughput_mb = if elapsed > 0 {
                (snapshot.bytes_sent as f64 / elapsed as f64) / 1_048_576.0
            } else {
                0.0
            };

            println!(
                "[Provider] Progress: {}/{} sec | Published: {} | PPS: {:.2} (avg {:.2}) | Throughput: {:.2} MB/s (avg {:.2} MB/s)",
                elapsed.min(duration_secs),
                duration_secs,
                snapshot.messages_sent,
                instant_pps,
                overall_pps,
                instant_throughput_mb,
                overall_throughput_mb
            );

            last_published = snapshot.messages_sent;
            last_bytes = snapshot.bytes_sent;
            last_time = now;

            if elapsed >= duration_secs {
                break;
            }
        }
    });

    // Create rate limiter (capped at 1 token to prevent bursts)
    let mut rate_limiter = RateLimiter::new(config.target_pps);

    // Pre-generate payload ONCE to avoid allocation in hot path
    let payload = generate_payload(config.payload_size);
    let payload_size = config.payload_size as u64;

    // Log benchmark start
    append_to_log(
        &config.log_file,
        &format!("\nSTART {}/s", config.target_pps),
    );

    // Main benchmark loop - minimal overhead
    let mut sequence_id: u64 = 0;
    while benchmark_start.elapsed().as_secs() < config.duration_secs {
        // Rate limiting
        rate_limiter.wait_for_token().await;

        // Check if benchmark is still running
        if benchmark_start.elapsed().as_secs() >= config.duration_secs {
            break;
        }

        sequence_id += 1;
        let timestamp_ns = MetricsCollector::current_timestamp_ns();

        let data = ContinuousData {
            sequence_id,
            timestamp_ns,
            payload_size: config.payload_size,
            payload: payload.clone(),
        };

        // Publish via the continuous handle
        continuous_handle.benchmark_stream(&data).await;

        // Update counters (lock-free atomic operations)
        counters.inc_sent();
        counters.add_bytes_sent(payload_size);
        counters.update_highest_sequence(sequence_id);
    }

    // Log benchmark end
    append_to_log(&config.log_file, &format!("\nEND {}/s", config.target_pps));

    // Stop progress reporting
    running.store(false, Ordering::SeqCst);

    // Stop metrics collection
    metrics_collector.stop();

    // Wait for background threads to finish
    let _ = progress_handle.join();
    let _ = metrics_handle.join();

    println!("\n[Provider] Benchmark completed!");

    // Compute and display aggregated metrics
    let metrics = AggregatedMetrics::from_collector(&metrics_collector);
    metrics.print_summary("PROVIDER");

    // Save results to JSON
    if let Err(e) = metrics.save_to_file(&config.output_file) {
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
