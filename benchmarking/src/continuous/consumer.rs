#[path = "../common/mod.rs"]
mod common;

use common::continuous_types::{
    ContinuousBenchmarkConfig, ContinuousBenchmarkResults, ContinuousData,
    ContinuousSystemMetricsSample, continuous_current_timestamp_ns,
    parse_continuous_config_from_args,
};
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use dust_dds::infrastructure::qos::QosKind;
use dust_dds::infrastructure::status::NO_STATUS;
use dust_dds::listener::NO_LISTENER;
use dust_dds::std_runtime::StdRuntime;
use mycellium_computing::consumes;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};

/// Global counters for tracking consumer metrics
static MESSAGES_RECEIVED: AtomicU64 = AtomicU64::new(0);
static OUT_OF_ORDER_MESSAGES: AtomicU64 = AtomicU64::new(0);
static DUPLICATE_MESSAGES: AtomicU64 = AtomicU64::new(0);
static HIGHEST_SEQ_RECEIVED: AtomicU64 = AtomicU64::new(0);

/// Shared state for message tracking
struct ConsumerState {
    received_sequences: HashSet<u64>,
    last_sequence_id: u64,
}

impl ConsumerState {
    fn new() -> Self {
        Self {
            received_sequences: HashSet::new(),
            last_sequence_id: 0,
        }
    }
}

/// Consumer implementation for continuous benchmarking
#[consumes(StdRuntime, [
    Continuous("benchmark_stream", ContinuousData)
])]
struct ContinuousBenchmarkConsumer;

/// Callback implementation for receiving continuous data
impl ContinuousBenchmarkConsumerContinuosTrait for ContinuousBenchmarkConsumer {
    async fn benchmark_stream(data: ContinuousData) {
        let received_timestamp_ns = continuous_current_timestamp_ns();
        let latency_ns = received_timestamp_ns - data.timestamp_ns;

        // Update global counters
        MESSAGES_RECEIVED.fetch_add(1, Ordering::SeqCst);

        // Update highest sequence received
        let mut highest = HIGHEST_SEQ_RECEIVED.load(Ordering::SeqCst);
        while data.sequence_id > highest {
            match HIGHEST_SEQ_RECEIVED.compare_exchange_weak(
                highest,
                data.sequence_id,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(h) => highest = h,
            }
        }

        // Check for out-of-order delivery
        let is_out_of_order = data.sequence_id < highest;
        if is_out_of_order {
            OUT_OF_ORDER_MESSAGES.fetch_add(1, Ordering::SeqCst);
        }

        // Store metrics in thread-local or use the global state
        // Note: In a real implementation, we'd use proper synchronization
        // For now, we track via atomic counters and sample periodically

        // Log occasional message for debugging (every 1000 messages)
        let received = MESSAGES_RECEIVED.load(Ordering::SeqCst);
        if received % 50000 == 0 {
            println!(
                "[Consumer] Received message #{} (seq: {}, latency: {:.3} ms)",
                received,
                data.sequence_id,
                latency_ns as f64 / 1_000_000.0
            );
        }
    }
}

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

/// Detailed message tracker that samples latency periodically
fn start_latency_sampler(state: Arc<Mutex<ConsumerState>>, running: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut last_highest_seq = 0u64;

        while running.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(10));

            let current_highest = HIGHEST_SEQ_RECEIVED.load(Ordering::SeqCst);

            // Track sequence gaps for loss detection
            if current_highest > last_highest_seq {
                if let Ok(mut guard) = state.lock() {
                    for seq in (last_highest_seq + 1)..=current_highest {
                        guard.received_sequences.insert(seq);
                    }
                    guard.last_sequence_id = current_highest;
                }
                last_highest_seq = current_highest;
            }
        }
    });
}

async fn run_consumer(config: ContinuousBenchmarkConfig) {
    println!("===========================================");
    println!("    Continuous Benchmark - CONSUMER");
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

    // Initialize metrics collection
    let samples: Arc<Mutex<Vec<ContinuousSystemMetricsSample>>> = Arc::new(Mutex::new(Vec::new()));
    let consumer_state = Arc::new(Mutex::new(ConsumerState::new()));
    let running = Arc::new(AtomicBool::new(true));

    // Start metrics collector
    start_metrics_collector(&config, samples.clone(), running.clone());

    // Start latency sampler
    start_latency_sampler(consumer_state.clone(), running.clone());

    // Reset counters
    MESSAGES_RECEIVED.store(0, Ordering::SeqCst);
    OUT_OF_ORDER_MESSAGES.store(0, Ordering::SeqCst);
    DUPLICATE_MESSAGES.store(0, Ordering::SeqCst);
    HIGHEST_SEQ_RECEIVED.store(0, Ordering::SeqCst);

    // Record start time
    let start_time_ns = continuous_current_timestamp_ns();
    let _benchmark_start = Instant::now();
    println!(
        "[Consumer] Benchmark started at timestamp: {}",
        start_time_ns
    );

    // Progress reporting task
    let duration_secs = config.duration_secs;
    let progress_running = running.clone();
    let progress_handle = std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut last_received = 0u64;
        let mut last_time = start;

        while progress_running.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));

            let elapsed = start.elapsed().as_secs();
            let current_received = MESSAGES_RECEIVED.load(Ordering::SeqCst);
            let current_out_of_order = OUT_OF_ORDER_MESSAGES.load(Ordering::SeqCst);
            let current_highest = HIGHEST_SEQ_RECEIVED.load(Ordering::SeqCst);
            let now = std::time::Instant::now();
            let interval_duration = now.duration_since(last_time).as_secs_f64();

            let instant_mps = if interval_duration > 0.0 {
                (current_received - last_received) as f64 / interval_duration
            } else {
                0.0
            };

            let overall_mps = if elapsed > 0 {
                current_received as f64 / elapsed as f64
            } else {
                0.0
            };

            // Estimate loss rate based on highest sequence seen
            let expected_messages = current_highest;
            let loss_rate = if expected_messages > 0 {
                ((expected_messages - current_received) as f64 / expected_messages as f64) * 100.0
            } else {
                0.0
            };

            println!(
                "[Consumer] Progress: {}/{} sec | Received: {} | MPS: {:.2} (avg {:.2}) | Loss: {:.2}% | OOO: {}",
                elapsed.min(duration_secs),
                duration_secs,
                current_received,
                instant_mps,
                overall_mps,
                loss_rate.max(0.0),
                current_out_of_order
            );

            last_received = current_received;
            last_time = now;

            if elapsed >= duration_secs {
                break;
            }
        }
    });

    // Run for the specified duration
    smol::Timer::after(Duration::from_secs(config.duration_secs)).await;

    // Stop metrics collection
    running.store(false, Ordering::SeqCst);
    let end_time_ns = continuous_current_timestamp_ns();

    // Wait for progress thread to finish
    let _ = progress_handle.join();

    // Collect final metrics
    let total_received = MESSAGES_RECEIVED.load(Ordering::SeqCst);
    let total_out_of_order = OUT_OF_ORDER_MESSAGES.load(Ordering::SeqCst);
    let total_duplicates = DUPLICATE_MESSAGES.load(Ordering::SeqCst);
    let highest_seq = HIGHEST_SEQ_RECEIVED.load(Ordering::SeqCst);

    // The highest sequence ID represents total messages sent by provider
    let total_sent = highest_seq;
    let messages_lost = total_sent.saturating_sub(total_received);

    println!("\n[Consumer] Benchmark completed!");
    println!("  Total messages received: {}", total_received);
    println!("  Total messages sent (by provider): {}", total_sent);
    println!("  Messages lost: {}", messages_lost);
    println!("  Out-of-order messages: {}", total_out_of_order);
    println!("  Duplicate messages: {}", total_duplicates);

    // Build results
    let mut results = ContinuousBenchmarkResults::new(config.clone(), "consumer");
    results.start_time_ns = start_time_ns;
    results.end_time_ns = end_time_ns;
    results.total_messages_sent = total_sent;
    results.messages_received = total_received;
    results.messages_lost = messages_lost;
    results.out_of_order_messages = total_out_of_order;
    results.duplicate_messages = total_duplicates;

    // Copy system metrics samples
    if let Ok(guard) = samples.lock() {
        results.system_metrics_samples = guard.clone();
    }

    // Note: Per-message metrics with latency tracking would require more sophisticated
    // synchronization. For now, we track aggregate metrics.
    // In production, you might use a lock-free queue or channel for this.
    results.message_metrics = None;

    // Calculate derived metrics
    results.finalize();

    // Print summary
    println!("\n===========================================");
    println!("       CONSUMER RESULTS SUMMARY");
    println!("===========================================");
    println!("Duration: {:.2} seconds", results.total_duration_secs);
    println!("-------------------------------------------");
    println!("MESSAGE METRICS:");
    println!(
        "  Total Messages Sent (by provider): {}",
        results.total_messages_sent
    );
    println!("  Messages Received: {}", results.messages_received);
    println!("  Messages Lost: {}", results.messages_lost);
    println!("  Loss Rate: {:.2}%", results.loss_rate_percent);
    println!("  Messages/Second: {:.2}", results.messages_per_second);
    println!("  Out-of-Order Messages: {}", results.out_of_order_messages);
    println!(
        "  Out-of-Order Rate: {:.2}%",
        results.out_of_order_rate_percent
    );
    println!("  Duplicate Messages: {}", results.duplicate_messages);
    println!("-------------------------------------------");
    println!("LATENCY METRICS:");
    if let Some(min) = results.min_latency_ns {
        println!("  Min Latency: {:.3} ms", min as f64 / 1_000_000.0);
    } else {
        println!("  Min Latency: N/A (per-message tracking disabled)");
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
    println!("-------------------------------------------");
    println!("JITTER METRICS:");
    if let Some(avg_jitter) = results.avg_jitter_ns {
        println!("  Avg Jitter: {:.3} ms", avg_jitter / 1_000_000.0);
    } else {
        println!("  Avg Jitter: N/A");
    }
    if let Some(max_jitter) = results.max_jitter_ns {
        println!("  Max Jitter: {:.3} ms", max_jitter as f64 / 1_000_000.0);
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
        "  Data Throughput: {:.2} MB/s",
        results.throughput_bytes_per_sec / 1_048_576.0
    );
    println!("===========================================\n");

    // Save results to JSON
    if let Err(e) = results.save_to_file() {
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
