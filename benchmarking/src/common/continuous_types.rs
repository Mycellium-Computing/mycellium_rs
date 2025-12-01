// Allow unused code since this module is shared across multiple binaries
// and each binary only uses a subset of the functionality
#![allow(dead_code)]

use dust_dds::infrastructure::type_support::DdsType;
use serde::{Deserialize, Serialize};

/// Data message for continuous benchmarking (published by provider)
#[derive(DdsType, Debug, Clone)]
pub struct ContinuousData {
    pub sequence_id: u64,
    pub timestamp_ns: i64,
    pub payload_size: u32,
    pub payload: Vec<u8>,
}

/// Configuration for the continuous benchmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousBenchmarkConfig {
    /// Domain ID for DDS communication
    pub domain_id: u32,
    /// Duration of the benchmark in seconds
    pub duration_secs: u64,
    /// Target publications per second (0 = unlimited)
    pub target_pps: u64,
    /// Size of the payload in bytes
    pub payload_size: u32,
    /// Interval for sampling system metrics in milliseconds
    pub metrics_sample_interval_ms: u64,
    /// Output file path for JSON results
    pub output_file: String,
    /// Name/identifier for this benchmark run
    pub benchmark_name: String,
}

impl Default for ContinuousBenchmarkConfig {
    fn default() -> Self {
        Self {
            domain_id: 0,
            duration_secs: 60,
            target_pps: 100,
            payload_size: 1024,
            metrics_sample_interval_ms: 100,
            output_file: "continuous_benchmark_results.json".to_string(),
            benchmark_name: "continuous_benchmark".to_string(),
        }
    }
}

/// Single sample of system metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousSystemMetricsSample {
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

/// Per-message tracking information (consumer side)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetrics {
    pub sequence_id: u64,
    pub sent_timestamp_ns: i64,
    pub received_timestamp_ns: i64,
    pub latency_ns: i64,
    /// Whether this message was received out of order
    pub out_of_order: bool,
}

/// Aggregated benchmark results for continuous streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousBenchmarkResults {
    /// Configuration used for this benchmark
    pub config: ContinuousBenchmarkConfig,
    /// Role of this node (consumer or provider)
    pub role: String,
    /// Start time of the benchmark (Unix timestamp in nanoseconds)
    pub start_time_ns: i64,
    /// End time of the benchmark (Unix timestamp in nanoseconds)
    pub end_time_ns: i64,
    /// Total duration of the benchmark in seconds
    pub total_duration_secs: f64,

    // Message metrics
    /// Total number of messages sent (provider) or expected (consumer)
    pub total_messages_sent: u64,
    /// Number of messages received (consumer only)
    pub messages_received: u64,
    /// Number of lost messages (consumer only)
    pub messages_lost: u64,
    /// Loss rate as percentage (0.0 - 100.0)
    pub loss_rate_percent: f64,
    /// Messages per second achieved (publication or reception rate)
    pub messages_per_second: f64,
    /// Number of out-of-order messages (consumer only)
    pub out_of_order_messages: u64,
    /// Out-of-order rate as percentage
    pub out_of_order_rate_percent: f64,
    /// Number of duplicate messages (consumer only)
    pub duplicate_messages: u64,

    // Latency metrics (consumer only, in nanoseconds)
    /// Minimum latency
    pub min_latency_ns: Option<i64>,
    /// Maximum latency
    pub max_latency_ns: Option<i64>,
    /// Average latency
    pub avg_latency_ns: Option<f64>,
    /// Median latency (p50)
    pub p50_latency_ns: Option<i64>,
    /// 95th percentile latency
    pub p95_latency_ns: Option<i64>,
    /// 99th percentile latency
    pub p99_latency_ns: Option<i64>,
    /// Standard deviation of latency
    pub latency_std_dev_ns: Option<f64>,

    // Jitter metrics (consumer only, in nanoseconds)
    /// Average inter-arrival jitter
    pub avg_jitter_ns: Option<f64>,
    /// Maximum jitter
    pub max_jitter_ns: Option<i64>,

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
    /// Data throughput (bytes/sec)
    pub throughput_bytes_per_sec: f64,

    // Detailed samples for time-series analysis
    /// System metrics samples over time
    pub system_metrics_samples: Vec<ContinuousSystemMetricsSample>,
    /// Per-message metrics (optional, can be large)
    pub message_metrics: Option<Vec<MessageMetrics>>,
}

impl ContinuousBenchmarkResults {
    pub fn new(config: ContinuousBenchmarkConfig, role: &str) -> Self {
        Self {
            config,
            role: role.to_string(),
            start_time_ns: 0,
            end_time_ns: 0,
            total_duration_secs: 0.0,
            total_messages_sent: 0,
            messages_received: 0,
            messages_lost: 0,
            loss_rate_percent: 0.0,
            messages_per_second: 0.0,
            out_of_order_messages: 0,
            out_of_order_rate_percent: 0.0,
            duplicate_messages: 0,
            min_latency_ns: None,
            max_latency_ns: None,
            avg_latency_ns: None,
            p50_latency_ns: None,
            p95_latency_ns: None,
            p99_latency_ns: None,
            latency_std_dev_ns: None,
            avg_jitter_ns: None,
            max_jitter_ns: None,
            avg_cpu_usage_percent: 0.0,
            max_cpu_usage_percent: 0.0,
            min_cpu_usage_percent: 100.0,
            avg_memory_usage_bytes: 0,
            max_memory_usage_bytes: 0,
            min_memory_usage_bytes: u64::MAX,
            avg_memory_usage_percent: 0.0,
            throughput_bytes_per_sec: 0.0,
            system_metrics_samples: Vec::new(),
            message_metrics: Some(Vec::new()),
        }
    }

    /// Calculate derived metrics from raw data
    pub fn finalize(&mut self) {
        // Calculate duration
        if self.end_time_ns > self.start_time_ns {
            self.total_duration_secs =
                (self.end_time_ns - self.start_time_ns) as f64 / 1_000_000_000.0;
        }

        // Calculate loss metrics (consumer only)
        if self.role == "consumer" && self.total_messages_sent > 0 {
            self.messages_lost = self
                .total_messages_sent
                .saturating_sub(self.messages_received);
            self.loss_rate_percent =
                (self.messages_lost as f64 / self.total_messages_sent as f64) * 100.0;
        }

        // Calculate message rate
        if self.total_duration_secs > 0.0 {
            if self.role == "provider" {
                self.messages_per_second =
                    self.total_messages_sent as f64 / self.total_duration_secs;
            } else {
                self.messages_per_second = self.messages_received as f64 / self.total_duration_secs;
            }
        }

        // Calculate out-of-order rate
        if self.messages_received > 0 {
            self.out_of_order_rate_percent =
                (self.out_of_order_messages as f64 / self.messages_received as f64) * 100.0;
        }

        // Calculate latency statistics from message metrics
        if let Some(ref metrics) = self.message_metrics {
            let latencies: Vec<i64> = metrics.iter().map(|m| m.latency_ns).collect();

            if !latencies.is_empty() {
                let mut sorted_latencies = latencies.clone();
                sorted_latencies.sort();

                self.min_latency_ns = sorted_latencies.first().copied();
                self.max_latency_ns = sorted_latencies.last().copied();

                let sum: i64 = latencies.iter().sum();
                self.avg_latency_ns = Some(sum as f64 / latencies.len() as f64);

                // Percentiles
                self.p50_latency_ns = Some(percentile(&sorted_latencies, 50.0));
                self.p95_latency_ns = Some(percentile(&sorted_latencies, 95.0));
                self.p99_latency_ns = Some(percentile(&sorted_latencies, 99.0));

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

                // Calculate jitter (variation in inter-arrival times)
                if metrics.len() > 1 {
                    let mut jitters: Vec<i64> = Vec::new();
                    let mut sorted_by_seq: Vec<_> = metrics.iter().collect();
                    sorted_by_seq.sort_by_key(|m| m.sequence_id);

                    for i in 1..sorted_by_seq.len() {
                        let prev_arrival = sorted_by_seq[i - 1].received_timestamp_ns;
                        let curr_arrival = sorted_by_seq[i].received_timestamp_ns;
                        let inter_arrival = curr_arrival - prev_arrival;

                        // Expected inter-arrival time based on target PPS
                        let expected_interval = if self.config.target_pps > 0 {
                            1_000_000_000i64 / self.config.target_pps as i64
                        } else {
                            0
                        };

                        let jitter = (inter_arrival - expected_interval).abs();
                        jitters.push(jitter);
                    }

                    if !jitters.is_empty() {
                        self.avg_jitter_ns =
                            Some(jitters.iter().sum::<i64>() as f64 / jitters.len() as f64);
                        self.max_jitter_ns = jitters.iter().max().copied();
                    }
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

        // Calculate throughput
        if self.total_duration_secs > 0.0 {
            let message_count = if self.role == "provider" {
                self.total_messages_sent
            } else {
                self.messages_received
            };
            let total_bytes = message_count as f64 * self.config.payload_size as f64;
            self.throughput_bytes_per_sec = total_bytes / self.total_duration_secs;
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
pub fn continuous_current_timestamp_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64
}

/// Parse command line arguments for continuous benchmark configuration
pub fn parse_continuous_config_from_args() -> ContinuousBenchmarkConfig {
    let args: Vec<String> = std::env::args().collect();
    let mut config = ContinuousBenchmarkConfig::default();

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
            "--pps" | "-r" => {
                if i + 1 < args.len() {
                    config.target_pps = args[i + 1].parse().unwrap_or(config.target_pps);
                    i += 1;
                }
            }
            "--payload-size" | "-p" => {
                if i + 1 < args.len() {
                    config.payload_size = args[i + 1].parse().unwrap_or(config.payload_size);
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
                print_continuous_help();
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    config
}

fn print_continuous_help() {
    println!(
        r#"
Continuous (Pub/Sub) Benchmark

USAGE:
    benchmark [OPTIONS]

OPTIONS:
    -d, --domain-id <ID>        DDS domain ID (default: 0)
    -t, --duration <SECS>       Benchmark duration in seconds (default: 60)
    -r, --pps <COUNT>           Target publications per second, 0 for unlimited (default: 100)
    -p, --payload-size <BYTES>  Size of message payload (default: 1024)
        --sample-interval <MS>  System metrics sampling interval in ms (default: 100)
    -o, --output <FILE>         Output JSON file path (default: continuous_benchmark_results.json)
    -n, --name <NAME>           Benchmark name/identifier (default: continuous_benchmark)
    -h, --help                  Print this help message

EXAMPLES:
    # Run provider (publisher) on machine A
    ./continuous_provider -d 0 -r 100 -p 1024 -o provider_results.json

    # Run consumer (subscriber) on machine B
    ./continuous_consumer -d 0 -t 60 -o consumer_results.json
"#
    );
}
