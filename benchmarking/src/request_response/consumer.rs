#[path = "../common/mod.rs"]
mod common;

use common::metrics_collector::{AggregatedMetrics, MetricsCollector, MetricsCollectorConfig};
use common::types::{BenchmarkConfig, BenchmarkRequest, BenchmarkResponse, parse_config_from_args};
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use dust_dds::infrastructure::qos::QosKind;
use dust_dds::infrastructure::status::NO_STATUS;
use dust_dds::listener::NO_LISTENER;
use dust_dds::std_runtime::StdRuntime;
use mycellium_computing::consumes;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Consumer implementation for benchmarking
#[consumes(StdRuntime, [
    RequestResponse("benchmark_request", BenchmarkRequest, BenchmarkResponse)
])]
struct BenchmarkConsumer;

/// Generate payload of specified size
fn generate_payload(size: u32) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Rate limiter to achieve target RPS using token bucket algorithm
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

        // Add tokens based on elapsed time (max 1.0 to prevent bursts)
        let tokens_to_add = elapsed.as_secs_f64() * self.target_rps as f64;
        self.tokens = (self.tokens + tokens_to_add).min(1.0);
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
    println!("       (Low-Overhead Metrics Collection)");
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

    // Initialize the low-overhead metrics collector
    let metrics_config = MetricsCollectorConfig {
        duration_secs: config.duration_secs,
        sample_interval_ms: config.metrics_sample_interval_ms,
        latency_reservoir_size: 10_000, // Keep up to 10k latency samples
        track_latencies: true,
    };

    let mut metrics_collector = MetricsCollector::new(metrics_config);

    // Capture baseline memory BEFORE initializing DDS
    println!("[Consumer] Capturing baseline memory...");
    metrics_collector.capture_baseline();

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

    // Get shared references for metrics (lock-free)
    let counters = metrics_collector.get_counters();
    let latency_reservoir = metrics_collector.get_latency_reservoir();

    // Start metrics collection in background thread
    let metrics_handle = metrics_collector.start();
    println!("[Consumer] Metrics collection started");

    let benchmark_start = Instant::now();
    println!(
        "[Consumer] Benchmark started at timestamp: {}",
        MetricsCollector::current_timestamp_ns()
    );

    // Progress reporting (separate from measurement to avoid interference)
    let running = Arc::new(AtomicBool::new(true));
    let progress_running = running.clone();
    let progress_counters = counters.clone();
    let duration_secs = config.duration_secs;

    let progress_handle = std::thread::spawn(move || {
        let start = Instant::now();
        let mut last_sent = 0u64;
        let mut last_success = 0u64;
        let mut last_time = start;

        while progress_running.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));

            let elapsed = start.elapsed().as_secs();
            let snapshot = progress_counters.snapshot();
            let now = Instant::now();
            let interval_duration = now.duration_since(last_time).as_secs_f64();

            let instant_rps = if interval_duration > 0.0 {
                (snapshot.messages_sent - last_sent) as f64 / interval_duration
            } else {
                0.0
            };

            let success_rate = if snapshot.messages_sent > 0 {
                (snapshot.messages_received as f64 / snapshot.messages_sent as f64) * 100.0
            } else {
                0.0
            };

            let instant_success_rps = if interval_duration > 0.0 {
                (snapshot.messages_received - last_success) as f64 / interval_duration
            } else {
                0.0
            };

            println!(
                "[Consumer] Progress: {}/{} sec | Sent: {} | Success: {} | Failed: {} | RPS: {:.2} | Success RPS: {:.2} | Rate: {:.1}%",
                elapsed.min(duration_secs),
                duration_secs,
                snapshot.messages_sent,
                snapshot.messages_received,
                snapshot.messages_failed,
                instant_rps,
                instant_success_rps,
                success_rate
            );

            last_sent = snapshot.messages_sent;
            last_success = snapshot.messages_received;
            last_time = now;

            if elapsed >= duration_secs {
                break;
            }
        }
    });

    // Create rate limiter (capped at 1 token to prevent bursts)
    let mut rate_limiter = RateLimiter::new(config.target_rps);

    // Pre-generate payload ONCE to avoid allocation in hot path
    let payload = generate_payload(config.payload_size);
    let timeout =
        dust_dds::dcps::infrastructure::time::Duration::new(config.request_timeout_secs as i32, 0);
    let payload_size = config.payload_size as u64;

    // Main benchmark loop - minimal overhead
    let mut request_id: u64 = 0;
    while benchmark_start.elapsed().as_secs() < config.duration_secs {
        // Rate limiting
        rate_limiter.wait_for_token().await;

        // Check if benchmark is still running
        if benchmark_start.elapsed().as_secs() >= config.duration_secs {
            break;
        }

        request_id += 1;
        let sent_timestamp_ns = MetricsCollector::current_timestamp_ns();

        let request = BenchmarkRequest {
            request_id,
            timestamp_ns: sent_timestamp_ns,
            payload_size: config.payload_size,
            payload: payload.clone(),
        };

        // Increment sent counter (lock-free atomic)
        counters.inc_sent();
        counters.add_bytes_sent(payload_size);

        // Send request and measure round-trip time
        let result = consumer.benchmark_request(request, timeout).await;
        let received_timestamp_ns = MetricsCollector::current_timestamp_ns();

        match result {
            Some(response) => {
                // Calculate round-trip latency
                let rtt_ns = received_timestamp_ns - sent_timestamp_ns;

                // Verify response integrity
                if response.request_id == request_id {
                    counters.inc_received();
                    counters.add_bytes_received(response.payload_size as u64);

                    // Record latency using reservoir sampling (lock-free)
                    latency_reservoir.add_sample(rtt_ns);
                } else {
                    // ID mismatch counts as failure
                    counters.inc_failed();
                }
            }
            None => {
                // Timeout or failure
                counters.inc_failed();
            }
        }
    }

    // Stop progress reporting
    running.store(false, Ordering::SeqCst);

    // Stop metrics collection
    metrics_collector.stop();

    // Wait for background threads to finish
    let _ = progress_handle.join();
    let _ = metrics_handle.join();

    println!("\n[Consumer] Benchmark completed!");

    // Compute and display aggregated metrics
    let metrics = AggregatedMetrics::from_collector(&metrics_collector);
    metrics.print_summary("CONSUMER");

    // Save results to JSON
    if let Err(e) = metrics.save_to_file(&config.output_file) {
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
