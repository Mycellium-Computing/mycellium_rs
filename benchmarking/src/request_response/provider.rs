#[path = "../common/mod.rs"]
mod common;

use common::types::{
    BenchmarkConfig, BenchmarkRequest, BenchmarkResponse, BenchmarkResults, SystemMetricsSample,
    current_timestamp_ns, parse_config_from_args,
};
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use dust_dds::std_runtime::StdRuntime;
use mycellium_computing::core::application::Application;
use mycellium_computing::provides;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, System};

/// Global state for tracking provider metrics
static REQUESTS_RECEIVED: AtomicU64 = AtomicU64::new(0);
static REQUESTS_PROCESSED: AtomicU64 = AtomicU64::new(0);

/// Provider implementation for benchmarking
#[provides(StdRuntime, [
    RequestResponse("benchmark_request", BenchmarkRequest, BenchmarkResponse)
])]
struct BenchmarkProvider;

impl BenchmarkProviderProviderTrait for BenchmarkProvider {
    async fn benchmark_request(request: BenchmarkRequest) -> BenchmarkResponse {
        REQUESTS_RECEIVED.fetch_add(1, Ordering::SeqCst);

        // Create response with same payload size
        let response = BenchmarkResponse {
            request_id: request.request_id,
            request_timestamp_ns: request.timestamp_ns,
            response_timestamp_ns: current_timestamp_ns(),
            payload_size: request.payload_size,
            payload: request.payload, // Echo back the payload
        };

        REQUESTS_PROCESSED.fetch_add(1, Ordering::SeqCst);
        response
    }
}

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

async fn run_provider(config: BenchmarkConfig) {
    println!("===========================================");
    println!("  Request-Response Benchmark - PROVIDER");
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

    // Initialize DDS
    let factory = DomainParticipantFactoryAsync::get_instance();
    let mut app = Application::new(config.domain_id, "BenchmarkProvider", factory).await;

    // Register provider
    app.register_provider::<BenchmarkProvider>().await;
    println!("[Provider] Registered and ready to serve requests");

    // Initialize metrics collection
    let samples: Arc<Mutex<Vec<SystemMetricsSample>>> = Arc::new(Mutex::new(Vec::new()));
    let running = Arc::new(AtomicBool::new(true));

    // Start metrics collector
    start_metrics_collector(&config, samples.clone(), running.clone());

    // Record start time
    let start_time_ns = current_timestamp_ns();
    println!(
        "[Provider] Benchmark started at timestamp: {}",
        start_time_ns
    );

    // Reset counters
    REQUESTS_RECEIVED.store(0, Ordering::SeqCst);
    REQUESTS_PROCESSED.store(0, Ordering::SeqCst);

    // Progress reporting task
    let duration_secs = config.duration_secs;
    let progress_running = running.clone();
    let progress_handle = std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut last_requests = 0u64;
        let mut last_time = start;

        while progress_running.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));

            let elapsed = start.elapsed().as_secs();
            let current_requests = REQUESTS_PROCESSED.load(Ordering::SeqCst);
            let now = std::time::Instant::now();
            let interval_duration = now.duration_since(last_time).as_secs_f64();

            let instant_rps = if interval_duration > 0.0 {
                (current_requests - last_requests) as f64 / interval_duration
            } else {
                0.0
            };

            let overall_rps = if elapsed > 0 {
                current_requests as f64 / elapsed as f64
            } else {
                0.0
            };

            println!(
                "[Provider] Progress: {}/{} sec | Requests: {} | Current RPS: {:.2} | Avg RPS: {:.2}",
                elapsed.min(duration_secs),
                duration_secs,
                current_requests,
                instant_rps,
                overall_rps
            );

            last_requests = current_requests;
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
    let end_time_ns = current_timestamp_ns();

    // Wait for progress thread to finish
    let _ = progress_handle.join();

    // Collect final metrics
    let total_received = REQUESTS_RECEIVED.load(Ordering::SeqCst);
    let total_processed = REQUESTS_PROCESSED.load(Ordering::SeqCst);

    println!("\n[Provider] Benchmark completed!");
    println!("  Total requests received: {}", total_received);
    println!("  Total requests processed: {}", total_processed);

    // Build results
    let mut results = BenchmarkResults::new(config.clone(), "provider");
    results.start_time_ns = start_time_ns;
    results.end_time_ns = end_time_ns;
    results.total_requests = total_received;
    results.successful_requests = total_processed;

    // Copy system metrics samples
    if let Ok(guard) = samples.lock() {
        results.system_metrics_samples = guard.clone();
    }

    // Don't include per-request metrics for provider (too much data)
    results.request_metrics = None;

    // Calculate derived metrics
    results.finalize();

    // Print summary
    println!("\n===========================================");
    println!("           PROVIDER RESULTS SUMMARY");
    println!("===========================================");
    println!("Duration: {:.2} seconds", results.total_duration_secs);
    println!("Total Requests: {}", results.total_requests);
    println!("Processed Requests: {}", results.successful_requests);
    println!("Requests/Second: {:.2}", results.requests_per_second);
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
        results.memory_throughput_bytes_per_sec / 1_048_576.0
    );
    println!("===========================================\n");

    // Save results to JSON
    if let Err(e) = results.save_to_file() {
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
