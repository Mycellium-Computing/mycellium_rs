#[path = "../common/mod.rs"]
mod common;

use common::continuous_types::{
    ContinuousBenchmarkConfig, ContinuousData, parse_continuous_config_from_args,
};
use common::metrics_collector::{AggregatedMetrics, MetricsCollector, MetricsCollectorConfig};
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use dust_dds::infrastructure::qos::QosKind;
use dust_dds::infrastructure::status::NO_STATUS;
use dust_dds::listener::NO_LISTENER;
use dust_dds::std_runtime::StdRuntime;
use mycellium_computing::consumes;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::common::metrics_collector::{AtomicCounters, LatencyReservoir};

/// Append a log entry to the log file if configured
fn append_to_log(log_file: &Option<String>, message: &str) {
    if let Some(path) = log_file {
        match OpenOptions::new().append(true).create(true).open(path) {
            Ok(mut file) => {
                if let Err(e) = writeln!(file, "{}", message) {
                    eprintln!("[Consumer] Failed to write to log file: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[Consumer] Failed to open log file: {}", e);
            }
        }
    }
}

/// Shared state for tracking sequence numbers (for out-of-order detection)
/// Using atomics to avoid mutex contention in the hot path
static LAST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Shared counters and latency reservoir for the consumer callback
static CONSUMER_COUNTERS: std::sync::OnceLock<Arc<AtomicCounters>> = std::sync::OnceLock::new();
static CONSUMER_LATENCY: std::sync::OnceLock<Arc<LatencyReservoir>> = std::sync::OnceLock::new();

/// Consumer implementation for continuous benchmarking
#[consumes(StdRuntime, [
    Continuous("benchmark_stream", ContinuousData)
])]
struct ContinuousBenchmarkConsumer;

impl ContinuousBenchmarkConsumerContinuosTrait for ContinuousBenchmarkConsumer {
    async fn benchmark_stream(data: ContinuousData) {
        let received_ns = MetricsCollector::current_timestamp_ns();

        // Get shared counters (lock-free)
        if let Some(counters) = CONSUMER_COUNTERS.get() {
            counters.inc_received();
            counters.add_bytes_received(data.payload_size as u64);
            counters.update_highest_sequence(data.sequence_id);

            // Check for out-of-order delivery
            let last_seq = LAST_SEQUENCE.load(Ordering::Relaxed);
            if data.sequence_id <= last_seq && last_seq > 0 {
                if data.sequence_id == last_seq {
                    counters.inc_duplicates();
                } else {
                    counters.inc_out_of_order();
                }
            }
            LAST_SEQUENCE.store(data.sequence_id, Ordering::Relaxed);
        }

        // Record latency using reservoir sampling (lock-free)
        if let Some(latency_reservoir) = CONSUMER_LATENCY.get() {
            let latency_ns = received_ns - data.timestamp_ns;
            if latency_ns > 0 {
                latency_reservoir.add_sample(latency_ns);
            }
        }
    }
}

async fn run_consumer(config: ContinuousBenchmarkConfig) {
    println!("===========================================");
    println!("    Continuous Benchmark - CONSUMER");
    println!("       (Low-Overhead Metrics Collection)");
    println!("===========================================");
    println!("Configuration:");
    println!("  Domain ID: {}", config.domain_id);
    println!("  Duration: {} seconds", config.duration_secs);
    println!("  Expected PPS: {} (from provider)", config.target_pps);
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
        latency_reservoir_size: 10_000, // Keep up to 10k latency samples
        track_latencies: true,
    };

    let mut metrics_collector = MetricsCollector::new(metrics_config);

    // Capture baseline memory BEFORE initializing DDS
    println!("[Consumer] Capturing baseline memory...");
    metrics_collector.capture_baseline();

    // Set up shared counters and latency reservoir for the callback
    let counters = metrics_collector.get_counters();
    let latency_reservoir = metrics_collector.get_latency_reservoir();
    let _ = CONSUMER_COUNTERS.set(counters.clone());
    let _ = CONSUMER_LATENCY.set(latency_reservoir.clone());

    // Reset sequence tracking
    LAST_SEQUENCE.store(0, Ordering::SeqCst);

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

    // Initialize consumer (this sets up the subscription with callback)
    let _consumer = ContinuousBenchmarkConsumer::init(&participant, &subscriber, &publisher).await;
    println!("[Consumer] Initialized and subscribed to benchmark_stream");

    // Wait for provider to start publishing
    println!("[Consumer] Waiting for provider (3 seconds)...");
    smol::Timer::after(Duration::from_secs(3)).await;

    // Start metrics collection in background thread
    let metrics_handle = metrics_collector.start();
    println!("[Consumer] Metrics collection started");

    let benchmark_start = Instant::now();
    println!(
        "[Consumer] Benchmark started at timestamp: {}",
        MetricsCollector::current_timestamp_ns()
    );

    // Log benchmark start
    append_to_log(
        &config.log_file,
        &format!("\nSTART {}/s", config.target_pps),
    );

    // Progress reporting (separate from measurement to avoid interference)
    let running = Arc::new(AtomicBool::new(true));
    let progress_running = running.clone();
    let progress_counters = counters.clone();
    let duration_secs = config.duration_secs;

    let progress_handle = std::thread::spawn(move || {
        let start = Instant::now();
        let mut last_received = 0u64;
        let mut last_time = start;

        while progress_running.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));

            let elapsed = start.elapsed().as_secs();
            let snapshot = progress_counters.snapshot();
            let now = Instant::now();
            let interval_duration = now.duration_since(last_time).as_secs_f64();

            let instant_mps = if interval_duration > 0.0 {
                (snapshot.messages_received - last_received) as f64 / interval_duration
            } else {
                0.0
            };

            let overall_mps = if elapsed > 0 {
                snapshot.messages_received as f64 / elapsed as f64
            } else {
                0.0
            };

            // Estimate loss rate based on highest sequence seen
            let expected_messages = snapshot.highest_sequence;
            let loss_rate = if expected_messages > 0 {
                ((expected_messages - snapshot.messages_received) as f64 / expected_messages as f64)
                    * 100.0
            } else {
                0.0
            };

            println!(
                "[Consumer] Progress: {}/{} sec | Received: {} | MPS: {:.2} (avg {:.2}) | Loss: {:.2}% | OOO: {} | Dup: {}",
                elapsed.min(duration_secs),
                duration_secs,
                snapshot.messages_received,
                instant_mps,
                overall_mps,
                loss_rate.max(0.0),
                snapshot.out_of_order,
                snapshot.duplicates
            );

            last_received = snapshot.messages_received;
            last_time = now;

            if elapsed >= duration_secs {
                break;
            }
        }
    });

    // Run for the specified duration (consumer is passive, just waiting)
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

    println!("\n[Consumer] Benchmark completed!");

    // Log benchmark end
    append_to_log(&config.log_file, &format!("\nEND {}/s", config.target_pps));

    // Compute and display aggregated metrics
    let metrics = AggregatedMetrics::from_collector(&metrics_collector);
    metrics.print_summary("CONSUMER");

    // Save results to JSON
    if let Err(e) = metrics.save_to_file(&config.output_file) {
        eprintln!("[Consumer] Failed to save results: {}", e);
    }
}

fn main() {
    let config = parse_continuous_config_from_args();

    // Set default output file for consumer if not specified
    let config = if config.output_file == "continuous_benchmark_results.json" {
        ContinuousBenchmarkConfig {
            output_file: "continuous_consumer_results.json".to_string(),
            ..config
        }
    } else {
        config
    };

    smol::block_on(run_consumer(config));
}
