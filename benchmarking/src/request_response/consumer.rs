#[path = "../common/mod.rs"]
mod common;

use common::types::{
    BenchmarkConfig, BenchmarkRequest, BenchmarkResponse, BenchmarkResults, RequestMetrics,
    SystemMetricsSample, current_timestamp_ns, parse_config_from_args,
};
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use dust_dds::infrastructure::qos::QosKind;
use dust_dds::infrastructure::status::NO_STATUS;
use dust_dds::listener::NO_LISTENER;
use dust_dds::std_runtime::StdRuntime;
use mycellium_computing::consumes;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};

/// Global counters for tracking requests
static REQUESTS_SENT: AtomicU64 = AtomicU64::new(0);
static REQUESTS_SUCCESS: AtomicU64 = AtomicU64::new(0);
static REQUESTS_FAILED: AtomicU64 = AtomicU64::new(0);

/// Consumer implementation for benchmarking
#[consumes(StdRuntime, [
    RequestResponse("benchmark_request", BenchmarkRequest, BenchmarkResponse)
])]
struct BenchmarkConsumer;

/// Collect system metrics sample
fn collect_system_metrics(sys: &mut System, pid: Pid) -> SystemMetricsSample {
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

    SystemMetricsSample {
        timestamp_ns: current_timestamp_ns(),
        cpu_usage_percent: cpu_usage,
        memory_usage_bytes: memory_usage,
        total_memory_bytes: total_memory,
        memory_usage_percent: memory_percent,
    }
}

/// Metrics collector task
fn start_metrics_collector(
    config: &BenchmarkConfig,
    samples: Arc<Mutex<Vec<SystemMetricsSample>>>,
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

/// Rate limiter to achieve target RPS
struct RateLimiter {
    target_rps: u64,
    interval_ns: u64,
    last_request_time: Instant,
    tokens: f64,
}

impl RateLimiter {
    fn new(target_rps: u64) -> Self {
        let interval_ns = if target_rps > 0 {
            1_000_000_000 / target_rps
        } else {
            0
        };
        Self {
            target_rps,
            interval_ns,
            last_request_time: Instant::now(),
            tokens: 1.0,
        }
    }

    async fn wait_for_token(&mut self) {
        if self.target_rps == 0 {
            // Unlimited rate
            return;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_request_time);

        // Add tokens based on elapsed time
        let tokens_to_add = elapsed.as_secs_f64() * self.target_rps as f64;
        self.tokens = (self.tokens + tokens_to_add).min(self.target_rps as f64);
        self.last_request_time = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
        } else {
            // Wait for next token
            let wait_time_ns = self.interval_ns as f64 * (1.0 - self.tokens);
            smol::Timer::after(Duration::from_nanos(wait_time_ns as u64)).await;
            self.tokens = 0.0;
            self.last_request_time = Instant::now();
        }
    }
}

async fn run_consumer(config: BenchmarkConfig) {
    println!("===========================================");
    println!("  Request-Response Benchmark - CONSUMER");
    println!("===========================================");
    println!("Configuration:");
    println!("  Domain ID: {}", config.domain_id);
    println!("  Duration: {} seconds", config.duration_secs);
    println!("  Target RPS: {} (0 = unlimited)", config.target_rps);
    println!("  Payload size: {} bytes", config.payload_size);
    println!("  Request timeout: {} seconds", config.request_timeout_secs);
    println!(
        "  Metrics sample interval: {} ms",
        config.metrics_sample_interval_ms
    );
    println!("  Output file: {}", config.output_file);
    println!("===========================================\n");

    // Initialize DDS
    let factory = DomainParticipantFactoryAsync::get_instance();

    let participant = factory
        .create_participant(
            config.domain_id as i32,
            QosKind::Default,
            NO_LISTENER,
            NO_STATUS,
        )
        .await
        .unwrap();

    let subscriber = participant
        .create_subscriber(QosKind::Default, NO_LISTENER, NO_STATUS)
        .await
        .unwrap();

    let publisher = participant
        .create_publisher(QosKind::Default, NO_LISTENER, NO_STATUS)
        .await
        .unwrap();

    // Initialize consumer
    let consumer = BenchmarkConsumer::init(&participant, &subscriber, &publisher).await;
    println!("[Consumer] Initialized and ready to send requests");

    // Wait for provider to be ready
    println!("[Consumer] Waiting for provider (3 seconds)...");
    smol::Timer::after(Duration::from_secs(3)).await;

    // Initialize metrics collection
    let samples: Arc<Mutex<Vec<SystemMetricsSample>>> = Arc::new(Mutex::new(Vec::new()));
    let request_metrics: Arc<Mutex<Vec<RequestMetrics>>> = Arc::new(Mutex::new(Vec::new()));
    let running = Arc::new(AtomicBool::new(true));

    // Start metrics collector
    start_metrics_collector(&config, samples.clone(), running.clone());

    // Reset counters
    REQUESTS_SENT.store(0, Ordering::SeqCst);
    REQUESTS_SUCCESS.store(0, Ordering::SeqCst);
    REQUESTS_FAILED.store(0, Ordering::SeqCst);

    // Record start time
    let start_time_ns = current_timestamp_ns();
    let benchmark_start = Instant::now();
    println!(
        "[Consumer] Benchmark started at timestamp: {}",
        start_time_ns
    );

    // Progress reporting task
    let duration_secs = config.duration_secs;
    let progress_running = running.clone();
    let progress_handle = std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut last_sent = 0u64;
        let mut last_time = start;

        while progress_running.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));

            let elapsed = start.elapsed().as_secs();
            let current_sent = REQUESTS_SENT.load(Ordering::SeqCst);
            let current_success = REQUESTS_SUCCESS.load(Ordering::SeqCst);
            let current_failed = REQUESTS_FAILED.load(Ordering::SeqCst);
            let now = std::time::Instant::now();
            let interval_duration = now.duration_since(last_time).as_secs_f64();

            let instant_rps = if interval_duration > 0.0 {
                (current_sent - last_sent) as f64 / interval_duration
            } else {
                0.0
            };

            let success_rate = if current_sent > 0 {
                (current_success as f64 / current_sent as f64) * 100.0
            } else {
                0.0
            };

            println!(
                "[Consumer] Progress: {}/{} sec | Sent: {} | Success: {} | Failed: {} | RPS: {:.2} | Success Rate: {:.1}%",
                elapsed.min(duration_secs),
                duration_secs,
                current_sent,
                current_success,
                current_failed,
                instant_rps,
                success_rate
            );

            last_sent = current_sent;
            last_time = now;

            if elapsed >= duration_secs {
                break;
            }
        }
    });

    // Create rate limiter
    let mut rate_limiter = RateLimiter::new(config.target_rps);

    // Pre-generate payload
    let payload = generate_payload(config.payload_size);
    let timeout =
        dust_dds::dcps::infrastructure::time::Duration::new(config.request_timeout_secs as i32, 0);

    // Send requests
    let mut request_id: u64 = 0;
    while benchmark_start.elapsed().as_secs() < config.duration_secs {
        // Rate limiting
        rate_limiter.wait_for_token().await;

        // Check if benchmark is still running
        if benchmark_start.elapsed().as_secs() >= config.duration_secs {
            break;
        }

        request_id += 1;
        let sent_timestamp_ns = current_timestamp_ns();

        let request = BenchmarkRequest {
            request_id,
            timestamp_ns: sent_timestamp_ns,
            payload_size: config.payload_size,
            payload: payload.clone(),
        };

        REQUESTS_SENT.fetch_add(1, Ordering::SeqCst);

        // Send request and track response
        let result = consumer.benchmark_request(request, timeout).await;

        let received_timestamp_ns = current_timestamp_ns();
        let (was_successful, round_trip_time_ns, error_message) = match result {
            Some(response) => {
                REQUESTS_SUCCESS.fetch_add(1, Ordering::SeqCst);
                let rtt = received_timestamp_ns - sent_timestamp_ns;
                // Verify response integrity
                if response.request_id != request_id {
                    (
                        false,
                        Some(rtt),
                        Some(format!(
                            "Request ID mismatch: expected {}, got {}",
                            request_id, response.request_id
                        )),
                    )
                } else {
                    (true, Some(rtt), None)
                }
            }
            None => {
                REQUESTS_FAILED.fetch_add(1, Ordering::SeqCst);
                (false, None, Some("Request timed out or failed".to_string()))
            }
        };

        // Record request metrics
        let req_metrics = RequestMetrics {
            request_id,
            sent_timestamp_ns,
            received_timestamp_ns: if was_successful {
                Some(received_timestamp_ns)
            } else {
                None
            },
            round_trip_time_ns,
            was_successful,
            error_message,
        };

        if let Ok(mut guard) = request_metrics.lock() {
            guard.push(req_metrics);
        }
    }

    // Stop metrics collection
    running.store(false, Ordering::SeqCst);
    let end_time_ns = current_timestamp_ns();

    // Wait for progress thread to finish
    let _ = progress_handle.join();

    // Collect final metrics
    let total_sent = REQUESTS_SENT.load(Ordering::SeqCst);
    let total_success = REQUESTS_SUCCESS.load(Ordering::SeqCst);
    let total_failed = REQUESTS_FAILED.load(Ordering::SeqCst);

    println!("\n[Consumer] Benchmark completed!");
    println!("  Total requests sent: {}", total_sent);
    println!("  Successful requests: {}", total_success);
    println!("  Failed requests: {}", total_failed);

    // Build results
    let mut results = BenchmarkResults::new(config.clone(), "consumer");
    results.start_time_ns = start_time_ns;
    results.end_time_ns = end_time_ns;
    results.total_requests = total_sent;
    results.successful_requests = total_success;

    // Copy system metrics samples
    if let Ok(guard) = samples.lock() {
        results.system_metrics_samples = guard.clone();
    }

    // Copy request metrics
    if let Ok(guard) = request_metrics.lock() {
        results.request_metrics = Some(guard.clone());
    }

    // Calculate derived metrics
    results.finalize();

    // Print summary
    println!("\n===========================================");
    println!("           CONSUMER RESULTS SUMMARY");
    println!("===========================================");
    println!("Duration: {:.2} seconds", results.total_duration_secs);
    println!("-------------------------------------------");
    println!("REQUEST METRICS:");
    println!("  Total Requests: {}", results.total_requests);
    println!("  Successful Requests: {}", results.successful_requests);
    println!("  Lost/Failed Requests: {}", results.lost_requests);
    println!("  Loss Rate: {:.2}%", results.loss_rate_percent);
    println!("  Requests/Second: {:.2}", results.requests_per_second);
    println!(
        "  Successful Requests/Second: {:.2}",
        results.successful_requests_per_second
    );
    println!("-------------------------------------------");
    println!("LATENCY METRICS:");
    if let Some(min) = results.min_latency_ns {
        println!("  Min Latency: {:.3} ms", min as f64 / 1_000_000.0);
    }
    if let Some(max) = results.max_latency_ns {
        println!("  Max Latency: {:.3} ms", max as f64 / 1_000_000.0);
    }
    if let Some(avg) = results.avg_latency_ns {
        println!("  Avg Latency: {:.3} ms", avg / 1_000_000.0);
    }
    if let Some(p50) = results.p50_latency_ns {
        println!("  P50 Latency: {:.3} ms", p50 as f64 / 1_000_000.0);
    }
    if let Some(p95) = results.p95_latency_ns {
        println!("  P95 Latency: {:.3} ms", p95 as f64 / 1_000_000.0);
    }
    if let Some(p99) = results.p99_latency_ns {
        println!("  P99 Latency: {:.3} ms", p99 as f64 / 1_000_000.0);
    }
    if let Some(std_dev) = results.latency_std_dev_ns {
        println!("  Std Dev: {:.3} ms", std_dev / 1_000_000.0);
    }
    println!("-------------------------------------------");
    println!("CPU USAGE:");
    println!("  Average: {:.2}%", results.avg_cpu_usage_percent);
    println!("  Maximum: {:.2}%", results.max_cpu_usage_percent);
    println!("  Minimum: {:.2}%", results.min_cpu_usage_percent);
    println!("-------------------------------------------");
    println!("MEMORY USAGE:");
    println!(
        "  Average: {:.2} MB",
        results.avg_memory_usage_bytes as f64 / 1_048_576.0
    );
    println!(
        "  Maximum: {:.2} MB",
        results.max_memory_usage_bytes as f64 / 1_048_576.0
    );
    println!(
        "  Minimum: {:.2} MB",
        results.min_memory_usage_bytes as f64 / 1_048_576.0
    );
    println!("  Average (%): {:.2}%", results.avg_memory_usage_percent);
    println!("-------------------------------------------");
    println!("THROUGHPUT:");
    println!(
        "  Memory Throughput: {:.2} MB/s",
        results.memory_throughput_bytes_per_sec / 1_048_576.0
    );
    println!("===========================================\n");

    // Save results to JSON
    if let Err(e) = results.save_to_file() {
        eprintln!("[Consumer] Failed to save results: {}", e);
    }
}

fn main() {
    let config = parse_config_from_args();

    // Set default output file for consumer if not specified
    let config = if config.output_file == "benchmark_results.json" {
        BenchmarkConfig {
            output_file: "consumer_benchmark_results.json".to_string(),
            ..config
        }
    } else {
        config
    };

    smol::block_on(run_consumer(config));
}
