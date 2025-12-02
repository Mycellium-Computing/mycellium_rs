#[path = "../common/mod.rs"]
mod common;

use common::metrics_collector::{AggregatedMetrics, MetricsCollector, MetricsCollectorConfig};
use common::types::{BenchmarkConfig, BenchmarkRequest, BenchmarkResponse, parse_config_from_args};
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use dust_dds::std_runtime::StdRuntime;
use mycelium_computing::core::application::Application;
use mycelium_computing::provides;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::common::metrics_collector::AtomicCounters;

/// Shared counters for the provider handler
/// Using a static Arc to allow access from the handler
static PROVIDER_COUNTERS: std::sync::OnceLock<Arc<AtomicCounters>> = std::sync::OnceLock::new();

/// Provider implementation for benchmarking
#[provides(StdRuntime, [
    RequestResponse("benchmark_request", BenchmarkRequest, BenchmarkResponse)
])]
struct BenchmarkProvider;

impl BenchmarkProviderProviderTrait for BenchmarkProvider {
    async fn benchmark_request(request: BenchmarkRequest) -> BenchmarkResponse {
        // Get counters (lock-free atomic operations)
        if let Some(counters) = PROVIDER_COUNTERS.get() {
            counters.inc_received();
            counters.add_bytes_received(request.payload.len() as u64);
        }

        // Create response with same payload size
        let response = BenchmarkResponse {
            request_id: request.request_id,
            request_timestamp_ns: request.timestamp_ns,
            response_timestamp_ns: MetricsCollector::current_timestamp_ns(),
            payload_size: request.payload_size,
            payload: request.payload, // Echo back the payload
        };

        // Track response
        if let Some(counters) = PROVIDER_COUNTERS.get() {
            counters.inc_sent();
            counters.add_bytes_sent(response.payload_size as u64);
        }

        response
    }
}

async fn run_provider(config: BenchmarkConfig) {
    println!("===========================================");
    println!("  Request-Response Benchmark - PROVIDER");
    println!("       (Low-Overhead Metrics Collection)");
    println!("===========================================");
    println!("Configuration:");
    println!("  Domain ID: {}", config.domain_id);
    println!("  Duration: {} seconds", config.duration_secs);
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

    // Set up shared counters for the handler
    let counters = metrics_collector.get_counters();
    let _ = PROVIDER_COUNTERS.set(counters.clone());

    // Initialize DDS
    let factory = DomainParticipantFactoryAsync::get_instance();
    let mut app = Application::new(config.domain_id, "BenchmarkProvider", factory).await;

    // Register provider
    app.register_provider::<BenchmarkProvider>().await;
    println!("[Provider] Registered and ready to serve requests");

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
        let mut last_received = 0u64;
        let mut last_bytes = 0u64;
        let mut last_time = start;

        while progress_running.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));

            let elapsed = start.elapsed().as_secs();
            let snapshot = progress_counters.snapshot();
            let now = Instant::now();
            let interval_duration = now.duration_since(last_time).as_secs_f64();

            let instant_rps = if interval_duration > 0.0 {
                (snapshot.messages_received - last_received) as f64 / interval_duration
            } else {
                0.0
            };

            let instant_throughput_mb = if interval_duration > 0.0 {
                ((snapshot.bytes_received - last_bytes) as f64 / interval_duration) / 1_048_576.0
            } else {
                0.0
            };

            let overall_rps = if elapsed > 0 {
                snapshot.messages_received as f64 / elapsed as f64
            } else {
                0.0
            };

            let overall_throughput_mb = if elapsed > 0 {
                (snapshot.bytes_received as f64 / elapsed as f64) / 1_048_576.0
            } else {
                0.0
            };

            println!(
                "[Provider] Progress: {}/{} sec | Requests: {} | RPS: {:.2} (avg {:.2}) | Throughput: {:.2} MB/s (avg {:.2} MB/s)",
                elapsed.min(duration_secs),
                duration_secs,
                snapshot.messages_received,
                instant_rps,
                overall_rps,
                instant_throughput_mb,
                overall_throughput_mb
            );

            last_received = snapshot.messages_received;
            last_bytes = snapshot.bytes_received;
            last_time = now;

            if elapsed >= duration_secs {
                break;
            }
        }
    });

    // Run for the specified duration
    while benchmark_start.elapsed().as_secs() < config.duration_secs {
        smol::Timer::after(Duration::from_millis(100)).await;
    }

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
    let config = parse_config_from_args();

    // Set default output file for provider if not specified
    let config = if config.output_file == "benchmark_results.json" {
        BenchmarkConfig {
            output_file: "provider_benchmark_results.json".to_string(),
            ..config
        }
    } else {
        config
    };

    smol::block_on(run_provider(config));
}
