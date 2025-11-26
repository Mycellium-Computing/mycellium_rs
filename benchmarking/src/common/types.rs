use dust_dds::infrastructure::type_support::DdsType;
use serde::{Deserialize, Serialize};

/// Request message for benchmarking
#[derive(DdsType, Debug, Clone)]
pub struct BenchmarkRequest {
    pub request_id: u64,
    pub timestamp_ns: i64,
    pub payload_size: u32,
    pub payload: Vec<u8>,
}

/// Response message for benchmarking
#[derive(DdsType, Debug, Clone)]
pub struct BenchmarkResponse {
    pub request_id: u64,
    pub request_timestamp_ns: i64,
    pub response_timestamp_ns: i64,
    pub payload_size: u32,
    pub payload: Vec<u8>,
}

/// Configuration for the benchmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Domain ID for DDS communication
    pub domain_id: u32,
    /// Duration of the benchmark in seconds
    pub duration_secs: u64,
    /// Target requests per second (0 = unlimited)
    pub target_rps: u64,
    /// Size of the payload in bytes
    pub payload_size: u32,
    /// Timeout for each request in seconds
    pub request_timeout_secs: u64,
    /// Interval for sampling system metrics in milliseconds
    pub metrics_sample_interval_ms: u64,
    /// Output file path for JSON results
    pub output_file: String,
    /// Name/identifier for this benchmark run
    pub benchmark_name: String,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            domain_id: 0,
            duration_secs: 60,
            target_rps: 100,
            payload_size: 1024,
            request_timeout_secs: 5,
            metrics_sample_interval_ms: 100,
            output_file: "benchmark_results.json".to_string(),
            benchmark_name: "request_response_benchmark".to_string(),
        }
    }
}

/// Single sample of system metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetricsSample {
    /// Timestamp when sample was taken (Unix timestamp in nanoseconds)
    pub timestamp_ns: i64,
    /// CPU usage as percentage (0.0 - 100.0)
    pub cpu_usage_percent: f64,
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// Total system memory in bytes
    pub total_memory_bytes: u64,
    /// Memory usage as percentage (0.0 - 100.0)
    pub memory_usage_percent: f64,
}

/// Request tracking information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetrics {
    pub request_id: u64,
    pub sent_timestamp_ns: i64,
    pub received_timestamp_ns: Option<i64>,
    pub round_trip_time_ns: Option<i64>,
    pub was_successful: bool,
    pub error_message: Option<String>,
}

/// Aggregated benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    /// Configuration used for this benchmark
    pub config: BenchmarkConfig,
    /// Role of this node (consumer or provider)
    pub role: String,
    /// Start time of the benchmark (Unix timestamp in nanoseconds)
    pub start_time_ns: i64,
    /// End time of the benchmark (Unix timestamp in nanoseconds)
    pub end_time_ns: i64,
    /// Total duration of the benchmark in seconds
    pub total_duration_secs: f64,

    // Request/Response metrics
    /// Total number of requests sent (consumer) or received (provider)
    pub total_requests: u64,
    /// Number of successful requests
    pub successful_requests: u64,
    /// Number of lost/failed requests
    pub lost_requests: u64,
    /// Loss rate as percentage (0.0 - 100.0)
    pub loss_rate_percent: f64,
    /// Requests per second achieved
    pub requests_per_second: f64,
    /// Successful requests per second
    pub successful_requests_per_second: f64,

    // Latency metrics (consumer only, in nanoseconds)
    /// Minimum round-trip time
    pub min_latency_ns: Option<i64>,
    /// Maximum round-trip time
    pub max_latency_ns: Option<i64>,
    /// Average round-trip time
    pub avg_latency_ns: Option<f64>,
    /// Median round-trip time (p50)
    pub p50_latency_ns: Option<i64>,
    /// 95th percentile latency
    pub p95_latency_ns: Option<i64>,
    /// 99th percentile latency
    pub p99_latency_ns: Option<i64>,
    /// Standard deviation of latency
    pub latency_std_dev_ns: Option<f64>,

    // System resource metrics
    /// Average CPU usage during benchmark
    pub avg_cpu_usage_percent: f64,
    /// Maximum CPU usage during benchmark
    pub max_cpu_usage_percent: f64,
    /// Minimum CPU usage during benchmark
    pub min_cpu_usage_percent: f64,
    /// Average memory usage during benchmark (bytes)
    pub avg_memory_usage_bytes: u64,
    /// Maximum memory usage during benchmark (bytes)
    pub max_memory_usage_bytes: u64,
    /// Minimum memory usage during benchmark (bytes)
    pub min_memory_usage_bytes: u64,
    /// Average memory usage as percentage
    pub avg_memory_usage_percent: f64,
    /// Memory size transferred per second (bytes/sec)
    pub memory_throughput_bytes_per_sec: f64,

    // Detailed samples for time-series analysis
    /// System metrics samples over time
    pub system_metrics_samples: Vec<SystemMetricsSample>,
    /// Per-request metrics (optional, can be large)
    pub request_metrics: Option<Vec<RequestMetrics>>,
}

impl BenchmarkResults {
    pub fn new(config: BenchmarkConfig, role: &str) -> Self {
        Self {
            config,
            role: role.to_string(),
            start_time_ns: 0,
            end_time_ns: 0,
            total_duration_secs: 0.0,
            total_requests: 0,
            successful_requests: 0,
            lost_requests: 0,
            loss_rate_percent: 0.0,
            requests_per_second: 0.0,
            successful_requests_per_second: 0.0,
            min_latency_ns: None,
            max_latency_ns: None,
            avg_latency_ns: None,
            p50_latency_ns: None,
            p95_latency_ns: None,
            p99_latency_ns: None,
            latency_std_dev_ns: None,
            avg_cpu_usage_percent: 0.0,
            max_cpu_usage_percent: 0.0,
            min_cpu_usage_percent: 100.0,
            avg_memory_usage_bytes: 0,
            max_memory_usage_bytes: 0,
            min_memory_usage_bytes: u64::MAX,
            avg_memory_usage_percent: 0.0,
            memory_throughput_bytes_per_sec: 0.0,
            system_metrics_samples: Vec::new(),
            request_metrics: Some(Vec::new()),
        }
    }

    /// Calculate derived metrics from raw data
    pub fn finalize(&mut self) {
        // Calculate duration
        if self.end_time_ns > self.start_time_ns {
            self.total_duration_secs =
                (self.end_time_ns - self.start_time_ns) as f64 / 1_000_000_000.0;
        }

        // Calculate loss metrics
        self.lost_requests = self.total_requests.saturating_sub(self.successful_requests);
        if self.total_requests > 0 {
            self.loss_rate_percent =
                (self.lost_requests as f64 / self.total_requests as f64) * 100.0;
        }

        // Calculate throughput
        if self.total_duration_secs > 0.0 {
            self.requests_per_second = self.total_requests as f64 / self.total_duration_secs;
            self.successful_requests_per_second =
                self.successful_requests as f64 / self.total_duration_secs;
        }

        // Calculate latency statistics from request metrics
        if let Some(ref metrics) = self.request_metrics {
            let mut latencies: Vec<i64> = metrics
                .iter()
                .filter_map(|m| m.round_trip_time_ns)
                .collect();

            if !latencies.is_empty() {
                latencies.sort();

                self.min_latency_ns = latencies.first().copied();
                self.max_latency_ns = latencies.last().copied();

                let sum: i64 = latencies.iter().sum();
                self.avg_latency_ns = Some(sum as f64 / latencies.len() as f64);

                // Percentiles
                self.p50_latency_ns = Some(percentile(&latencies, 50.0));
                self.p95_latency_ns = Some(percentile(&latencies, 95.0));
                self.p99_latency_ns = Some(percentile(&latencies, 99.0));

                // Standard deviation
                if let Some(avg) = self.avg_latency_ns {
                    let variance: f64 = latencies
                        .iter()
                        .map(|&x| {
                            let diff = x as f64 - avg;
                            diff * diff
                        })
                        .sum::<f64>()
                        / latencies.len() as f64;
                    self.latency_std_dev_ns = Some(variance.sqrt());
                }
            }
        }

        // Calculate system metrics statistics
        if !self.system_metrics_samples.is_empty() {
            let cpu_values: Vec<f64> = self
                .system_metrics_samples
                .iter()
                .map(|s| s.cpu_usage_percent)
                .collect();
            let mem_values: Vec<u64> = self
                .system_metrics_samples
                .iter()
                .map(|s| s.memory_usage_bytes)
                .collect();
            let mem_percent_values: Vec<f64> = self
                .system_metrics_samples
                .iter()
                .map(|s| s.memory_usage_percent)
                .collect();

            self.avg_cpu_usage_percent = cpu_values.iter().sum::<f64>() / cpu_values.len() as f64;
            self.max_cpu_usage_percent = cpu_values.iter().cloned().fold(f64::MIN, f64::max);
            self.min_cpu_usage_percent = cpu_values.iter().cloned().fold(f64::MAX, f64::min);

            self.avg_memory_usage_bytes = mem_values.iter().sum::<u64>() / mem_values.len() as u64;
            self.max_memory_usage_bytes = *mem_values.iter().max().unwrap_or(&0);
            self.min_memory_usage_bytes = *mem_values.iter().min().unwrap_or(&0);

            self.avg_memory_usage_percent =
                mem_percent_values.iter().sum::<f64>() / mem_percent_values.len() as f64;
        }

        // Calculate memory throughput (based on payload size and successful requests)
        if self.total_duration_secs > 0.0 {
            let total_bytes = self.successful_requests as f64 * self.config.payload_size as f64;
            self.memory_throughput_bytes_per_sec = total_bytes / self.total_duration_secs;
        }
    }

    /// Save results to JSON file
    pub fn save_to_file(&self) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.config.output_file, json)?;
        println!("Results saved to: {}", self.config.output_file);
        Ok(())
    }
}

/// Calculate percentile from sorted values
fn percentile(sorted_values: &[i64], p: f64) -> i64 {
    if sorted_values.is_empty() {
        return 0;
    }
    let idx = ((p / 100.0) * (sorted_values.len() - 1) as f64).round() as usize;
    sorted_values[idx.min(sorted_values.len() - 1)]
}

/// Get current timestamp in nanoseconds
pub fn current_timestamp_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64
}

/// Parse command line arguments for benchmark configuration
pub fn parse_config_from_args() -> BenchmarkConfig {
    let args: Vec<String> = std::env::args().collect();
    let mut config = BenchmarkConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--domain-id" | "-d" => {
                if i + 1 < args.len() {
                    config.domain_id = args[i + 1].parse().unwrap_or(config.domain_id);
                    i += 1;
                }
            }
            "--duration" | "-t" => {
                if i + 1 < args.len() {
                    config.duration_secs = args[i + 1].parse().unwrap_or(config.duration_secs);
                    i += 1;
                }
            }
            "--rps" | "-r" => {
                if i + 1 < args.len() {
                    config.target_rps = args[i + 1].parse().unwrap_or(config.target_rps);
                    i += 1;
                }
            }
            "--payload-size" | "-p" => {
                if i + 1 < args.len() {
                    config.payload_size = args[i + 1].parse().unwrap_or(config.payload_size);
                    i += 1;
                }
            }
            "--timeout" => {
                if i + 1 < args.len() {
                    config.request_timeout_secs =
                        args[i + 1].parse().unwrap_or(config.request_timeout_secs);
                    i += 1;
                }
            }
            "--sample-interval" => {
                if i + 1 < args.len() {
                    config.metrics_sample_interval_ms = args[i + 1]
                        .parse()
                        .unwrap_or(config.metrics_sample_interval_ms);
                    i += 1;
                }
            }
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    config.output_file = args[i + 1].clone();
                    i += 1;
                }
            }
            "--name" | "-n" => {
                if i + 1 < args.len() {
                    config.benchmark_name = args[i + 1].clone();
                    i += 1;
                }
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    config
}

fn print_help() {
    println!(
        r#"
Request-Response Benchmark

USAGE:
    benchmark [OPTIONS]

OPTIONS:
    -d, --domain-id <ID>        DDS domain ID (default: 0)
    -t, --duration <SECS>       Benchmark duration in seconds (default: 60)
    -r, --rps <COUNT>           Target requests per second, 0 for unlimited (default: 100)
    -p, --payload-size <BYTES>  Size of request/response payload (default: 1024)
        --timeout <SECS>        Request timeout in seconds (default: 5)
        --sample-interval <MS>  System metrics sampling interval in ms (default: 100)
    -o, --output <FILE>         Output JSON file path (default: benchmark_results.json)
    -n, --name <NAME>           Benchmark name/identifier (default: request_response_benchmark)
    -h, --help                  Print this help message

EXAMPLES:
    # Run provider on machine A
    ./request_response_provider -d 0 -o provider_results.json

    # Run consumer on machine B
    ./request_response_consumer -d 0 -t 60 -r 100 -p 1024 -o consumer_results.json
"#
    );
}
