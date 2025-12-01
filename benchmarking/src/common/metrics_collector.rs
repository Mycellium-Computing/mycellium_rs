//! Metrics Collector Module
//!
//! This module provides low-overhead metrics collection that isolates
//! benchmark instrumentation from the actual system being measured.
//!
//! Key design principles:
//! 1. Pre-allocate all memory upfront to avoid allocation during measurement
//! 2. Use lock-free data structures where possible
//! 3. Separate the benchmark process's own memory from measurements
//! 4. Sample-based latency tracking instead of storing every request

// Allow unused code since this module is shared across multiple binaries
// and each binary only uses a subset of the functionality
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, System};

/// Configuration for the metrics collector
#[derive(Debug, Clone)]
pub struct MetricsCollectorConfig {
    /// Duration of the benchmark in seconds
    pub duration_secs: u64,
    /// Interval for sampling system metrics in milliseconds
    pub sample_interval_ms: u64,
    /// Number of latency samples to keep (reservoir sampling)
    pub latency_reservoir_size: usize,
    /// Whether to track per-message latencies (uses reservoir sampling)
    pub track_latencies: bool,
}

impl Default for MetricsCollectorConfig {
    fn default() -> Self {
        Self {
            duration_secs: 60,
            sample_interval_ms: 100,
            latency_reservoir_size: 10_000, // Keep 10k samples max
            track_latencies: true,
        }
    }
}

/// Pre-allocated system metrics sample
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SystemSample {
    pub timestamp_ns: i64,
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub total_memory_bytes: u64,
}

/// Lock-free counters for high-frequency updates
#[derive(Debug)]
pub struct AtomicCounters {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub messages_failed: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub out_of_order: AtomicU64,
    pub duplicates: AtomicU64,
    pub highest_sequence: AtomicU64,
}

impl AtomicCounters {
    pub fn new() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            messages_failed: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            out_of_order: AtomicU64::new(0),
            duplicates: AtomicU64::new(0),
            highest_sequence: AtomicU64::new(0),
        }
    }

    pub fn reset(&self) {
        self.messages_sent.store(0, Ordering::SeqCst);
        self.messages_received.store(0, Ordering::SeqCst);
        self.messages_failed.store(0, Ordering::SeqCst);
        self.bytes_sent.store(0, Ordering::SeqCst);
        self.bytes_received.store(0, Ordering::SeqCst);
        self.out_of_order.store(0, Ordering::SeqCst);
        self.duplicates.store(0, Ordering::SeqCst);
        self.highest_sequence.store(0, Ordering::SeqCst);
    }

    #[inline(always)]
    pub fn inc_sent(&self) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn inc_received(&self) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn inc_failed(&self) {
        self.messages_failed.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn add_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn add_bytes_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn inc_out_of_order(&self) {
        self.out_of_order.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn inc_duplicates(&self) {
        self.duplicates.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn update_highest_sequence(&self, seq: u64) {
        self.highest_sequence.fetch_max(seq, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> CountersSnapshot {
        CountersSnapshot {
            messages_sent: self.messages_sent.load(Ordering::SeqCst),
            messages_received: self.messages_received.load(Ordering::SeqCst),
            messages_failed: self.messages_failed.load(Ordering::SeqCst),
            bytes_sent: self.bytes_sent.load(Ordering::SeqCst),
            bytes_received: self.bytes_received.load(Ordering::SeqCst),
            out_of_order: self.out_of_order.load(Ordering::SeqCst),
            duplicates: self.duplicates.load(Ordering::SeqCst),
            highest_sequence: self.highest_sequence.load(Ordering::SeqCst),
        }
    }
}

impl Default for AtomicCounters {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of counters at a point in time
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct CountersSnapshot {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_failed: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub out_of_order: u64,
    pub duplicates: u64,
    pub highest_sequence: u64,
}

/// Reservoir sampling for latency measurements
/// Uses Algorithm R for uniform sampling without knowing total count upfront
#[derive(Debug)]
pub struct LatencyReservoir {
    /// Pre-allocated buffer for latency samples (in nanoseconds)
    samples: Vec<AtomicI64>,
    /// Number of samples seen so far
    count: AtomicU64,
    /// Random seed for sampling decisions
    seed: AtomicU64,
    /// Capacity of the reservoir
    capacity: usize,
}

impl LatencyReservoir {
    /// Create a new reservoir with pre-allocated capacity
    pub fn new(capacity: usize) -> Self {
        let mut samples = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            samples.push(AtomicI64::new(0));
        }
        Self {
            samples,
            count: AtomicU64::new(0),
            seed: AtomicU64::new(0x12345678), // Initial seed
            capacity,
        }
    }

    /// Reset the reservoir for a new benchmark run
    pub fn reset(&self) {
        self.count.store(0, Ordering::SeqCst);
        for sample in &self.samples {
            sample.store(0, Ordering::Relaxed);
        }
    }

    /// Add a latency sample using reservoir sampling
    /// This is lock-free and has O(1) complexity
    #[inline(always)]
    pub fn add_sample(&self, latency_ns: i64) {
        let n = self.count.fetch_add(1, Ordering::Relaxed);

        if (n as usize) < self.capacity {
            // Fill the reservoir first
            self.samples[n as usize].store(latency_ns, Ordering::Relaxed);
        } else {
            // Reservoir sampling: replace with probability capacity/n
            let rand = self.next_random();
            let j = rand % (n + 1);
            if (j as usize) < self.capacity {
                self.samples[j as usize].store(latency_ns, Ordering::Relaxed);
            }
        }
    }

    /// Simple xorshift64 PRNG for fast random numbers
    #[inline(always)]
    fn next_random(&self) -> u64 {
        loop {
            let mut x = self.seed.load(Ordering::Relaxed);
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            if self
                .seed
                .compare_exchange_weak(
                    self.seed.load(Ordering::Relaxed),
                    x,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return x;
            }
        }
    }

    /// Get all collected samples for analysis
    pub fn get_samples(&self) -> Vec<i64> {
        let count = self.count.load(Ordering::SeqCst);
        let actual_count = (count as usize).min(self.capacity);

        let mut result = Vec::with_capacity(actual_count);
        for i in 0..actual_count {
            let val = self.samples[i].load(Ordering::Relaxed);
            if val > 0 {
                result.push(val);
            }
        }
        result
    }

    /// Get the total number of samples that were offered (not just stored)
    pub fn total_count(&self) -> u64 {
        self.count.load(Ordering::SeqCst)
    }
}

/// Running statistics calculator using Welford's online algorithm
/// Computes mean and variance in a single pass without storing all values
#[derive(Debug)]
pub struct RunningStats {
    count: AtomicU64,
    // We use i64 to store f64 bits for atomic operations
    mean_bits: AtomicU64,
    m2_bits: AtomicU64,
    min: AtomicI64,
    max: AtomicI64,
}

impl RunningStats {
    pub fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            mean_bits: AtomicU64::new(0),
            m2_bits: AtomicU64::new(0),
            min: AtomicI64::new(i64::MAX),
            max: AtomicI64::new(i64::MIN),
        }
    }

    pub fn reset(&self) {
        self.count.store(0, Ordering::SeqCst);
        self.mean_bits.store(0, Ordering::SeqCst);
        self.m2_bits.store(0, Ordering::SeqCst);
        self.min.store(i64::MAX, Ordering::SeqCst);
        self.max.store(i64::MIN, Ordering::SeqCst);
    }

    /// Note: This is NOT thread-safe for concurrent updates to mean/variance
    /// For concurrent scenarios, use the LatencyReservoir and compute stats after
    pub fn add_value(&self, value: i64) {
        // Update min/max atomically
        self.min.fetch_min(value, Ordering::Relaxed);
        self.max.fetch_max(value, Ordering::Relaxed);

        // Update count, mean, and M2 (not atomic, use only from single thread)
        let count = self.count.fetch_add(1, Ordering::Relaxed) + 1;
        let mean = f64::from_bits(self.mean_bits.load(Ordering::Relaxed));
        let m2 = f64::from_bits(self.m2_bits.load(Ordering::Relaxed));

        let delta = value as f64 - mean;
        let new_mean = mean + delta / count as f64;
        let delta2 = value as f64 - new_mean;
        let new_m2 = m2 + delta * delta2;

        self.mean_bits.store(new_mean.to_bits(), Ordering::Relaxed);
        self.m2_bits.store(new_m2.to_bits(), Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> StatsSnapshot {
        let count = self.count.load(Ordering::SeqCst);
        let mean = f64::from_bits(self.mean_bits.load(Ordering::SeqCst));
        let m2 = f64::from_bits(self.m2_bits.load(Ordering::SeqCst));
        let min = self.min.load(Ordering::SeqCst);
        let max = self.max.load(Ordering::SeqCst);

        let variance = if count > 1 {
            m2 / (count - 1) as f64
        } else {
            0.0
        };

        StatsSnapshot {
            count,
            mean,
            variance,
            std_dev: variance.sqrt(),
            min: if min == i64::MAX { 0 } else { min },
            max: if max == i64::MIN { 0 } else { max },
        }
    }
}

impl Default for RunningStats {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct StatsSnapshot {
    pub count: u64,
    pub mean: f64,
    pub variance: f64,
    pub std_dev: f64,
    pub min: i64,
    pub max: i64,
}

/// Estimate the memory overhead of the metrics collector itself
/// This is subtracted from measured memory to get true system memory usage
#[derive(Debug, Clone, Copy)]
pub struct MetricsOverhead {
    /// Estimated bytes used by the metrics collector
    pub estimated_bytes: u64,
    /// Memory usage at baseline (before benchmark starts)
    pub baseline_memory_bytes: u64,
}

impl MetricsOverhead {
    pub fn calculate(config: &MetricsCollectorConfig) -> Self {
        // Calculate expected allocations
        let num_samples = config.duration_secs * 1000 / config.sample_interval_ms;
        let system_samples_size = num_samples as u64 * std::mem::size_of::<SystemSample>() as u64;
        let latency_reservoir_size =
            config.latency_reservoir_size as u64 * std::mem::size_of::<AtomicI64>() as u64;
        let counters_size = std::mem::size_of::<AtomicCounters>() as u64;
        let running_stats_size = std::mem::size_of::<RunningStats>() as u64;

        // Add some overhead for Vec metadata, Arc, etc.
        let overhead_estimate = 4096; // 4KB buffer for misc allocations

        Self {
            estimated_bytes: system_samples_size
                + latency_reservoir_size
                + counters_size
                + running_stats_size
                + overhead_estimate,
            baseline_memory_bytes: 0,
        }
    }

    pub fn set_baseline(&mut self, baseline: u64) {
        self.baseline_memory_bytes = baseline;
    }

    /// Get the corrected memory usage (subtract our overhead)
    pub fn correct_memory(&self, measured: u64) -> u64 {
        measured.saturating_sub(self.estimated_bytes)
    }
}

/// The main metrics collector that coordinates all measurement
pub struct MetricsCollector {
    config: MetricsCollectorConfig,
    /// Pre-allocated buffer for system samples
    system_samples: Vec<SystemSample>,
    /// Current write index for system samples
    sample_index: AtomicU64,
    /// Lock-free counters
    pub counters: Arc<AtomicCounters>,
    /// Latency reservoir for sampling
    pub latency_reservoir: Arc<LatencyReservoir>,
    /// Running stats for jitter calculation
    pub jitter_stats: RunningStats,
    /// Last arrival time for jitter calculation (nanoseconds)
    last_arrival_ns: AtomicI64,
    /// Whether collection is active
    running: Arc<AtomicBool>,
    /// Overhead estimation
    overhead: MetricsOverhead,
    /// Start timestamp
    start_time_ns: AtomicI64,
    /// End timestamp
    end_time_ns: AtomicI64,
}

impl MetricsCollector {
    /// Create a new metrics collector with pre-allocated buffers
    pub fn new(config: MetricsCollectorConfig) -> Self {
        let overhead = MetricsOverhead::calculate(&config);

        // Pre-allocate system samples buffer
        let num_samples = (config.duration_secs * 1000 / config.sample_interval_ms + 10) as usize; // +10 for safety margin
        let mut system_samples = Vec::with_capacity(num_samples);
        system_samples.resize(num_samples, SystemSample::default());

        let latency_reservoir = Arc::new(LatencyReservoir::new(config.latency_reservoir_size));

        Self {
            config,
            system_samples,
            sample_index: AtomicU64::new(0),
            counters: Arc::new(AtomicCounters::new()),
            latency_reservoir,
            jitter_stats: RunningStats::new(),
            last_arrival_ns: AtomicI64::new(0),
            running: Arc::new(AtomicBool::new(false)),
            overhead,
            start_time_ns: AtomicI64::new(0),
            end_time_ns: AtomicI64::new(0),
        }
    }

    /// Get current timestamp in nanoseconds
    #[inline(always)]
    pub fn current_timestamp_ns() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64
    }

    /// Reset all metrics for a new benchmark run
    pub fn reset(&mut self) {
        self.sample_index.store(0, Ordering::SeqCst);
        self.counters.reset();
        self.latency_reservoir.reset();
        self.jitter_stats.reset();
        self.last_arrival_ns.store(0, Ordering::SeqCst);
        self.start_time_ns.store(0, Ordering::SeqCst);
        self.end_time_ns.store(0, Ordering::SeqCst);

        // Clear samples
        for sample in &mut self.system_samples {
            *sample = SystemSample::default();
        }
    }

    /// Capture baseline memory before starting the benchmark
    pub fn capture_baseline(&mut self) {
        let mut sys = System::new_all();
        sys.refresh_all();
        std::thread::sleep(Duration::from_millis(100));

        let pid = Pid::from_u32(std::process::id());
        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

        if let Some(process) = sys.process(pid) {
            self.overhead.set_baseline(process.memory());
        }
    }

    /// Start metrics collection in a background thread
    pub fn start(&self) -> std::thread::JoinHandle<()> {
        self.running.store(true, Ordering::SeqCst);
        let start_ns = Self::current_timestamp_ns();
        self.start_time_ns.store(start_ns, Ordering::SeqCst);

        let running = self.running.clone();
        let interval_ms = self.config.sample_interval_ms;
        let overhead = self.overhead;

        // We need to pass mutable access to system_samples
        // Since we pre-allocated and use atomic index, we can use raw pointer
        let samples_ptr = self.system_samples.as_ptr() as usize;
        let samples_len = self.system_samples.len();
        let sample_index = &self.sample_index as *const AtomicU64 as usize;

        std::thread::spawn(move || {
            let mut sys = System::new_all();
            let pid = Pid::from_u32(std::process::id());

            // Initial refresh for accurate CPU readings
            sys.refresh_all();
            std::thread::sleep(Duration::from_millis(100));

            while running.load(Ordering::SeqCst) {
                sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                sys.refresh_memory();

                let timestamp_ns = Self::current_timestamp_ns();

                let (cpu_usage, raw_memory) = if let Some(process) = sys.process(pid) {
                    (process.cpu_usage() as f64, process.memory())
                } else {
                    (0.0, 0)
                };

                // Correct memory by subtracting our overhead
                let corrected_memory = overhead.correct_memory(raw_memory);

                let sample = SystemSample {
                    timestamp_ns,
                    cpu_usage_percent: cpu_usage,
                    memory_usage_bytes: corrected_memory,
                    total_memory_bytes: sys.total_memory(),
                };

                // Write to pre-allocated buffer using atomic index
                unsafe {
                    let index_ptr = sample_index as *const AtomicU64;
                    let idx = (*index_ptr).fetch_add(1, Ordering::Relaxed) as usize;
                    if idx < samples_len {
                        let samples = samples_ptr as *mut SystemSample;
                        *samples.add(idx) = sample;
                    }
                }

                std::thread::sleep(Duration::from_millis(interval_ms));
            }
        })
    }

    /// Stop metrics collection
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.end_time_ns
            .store(Self::current_timestamp_ns(), Ordering::SeqCst);
    }

    /// Record a latency measurement (call from hot path)
    #[inline(always)]
    pub fn record_latency(&self, latency_ns: i64) {
        if self.config.track_latencies {
            self.latency_reservoir.add_sample(latency_ns);
        }
    }

    /// Record message arrival for jitter calculation
    #[inline(always)]
    pub fn record_arrival(&self) {
        let now = Self::current_timestamp_ns();
        let last = self.last_arrival_ns.swap(now, Ordering::Relaxed);

        if last > 0 {
            let inter_arrival = now - last;
            // Jitter is the variation in inter-arrival times
            // We track the inter-arrival time itself; jitter computed at end
            self.jitter_stats.add_value(inter_arrival);
        }
    }

    /// Check if collection is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get collected system samples
    pub fn get_system_samples(&self) -> Vec<SystemSample> {
        let count = self.sample_index.load(Ordering::SeqCst) as usize;
        let actual_count = count.min(self.system_samples.len());
        self.system_samples[..actual_count].to_vec()
    }

    /// Get the start timestamp
    pub fn get_start_time_ns(&self) -> i64 {
        self.start_time_ns.load(Ordering::SeqCst)
    }

    /// Get the end timestamp
    pub fn get_end_time_ns(&self) -> i64 {
        self.end_time_ns.load(Ordering::SeqCst)
    }

    /// Get the overhead estimation
    pub fn get_overhead(&self) -> &MetricsOverhead {
        &self.overhead
    }

    /// Get a clone of the counters Arc for sharing with other threads
    pub fn get_counters(&self) -> Arc<AtomicCounters> {
        self.counters.clone()
    }

    /// Get a clone of the latency reservoir Arc for sharing
    pub fn get_latency_reservoir(&self) -> Arc<LatencyReservoir> {
        self.latency_reservoir.clone()
    }
}

/// Final aggregated results from the metrics collector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    // Timing
    pub start_time_ns: i64,
    pub end_time_ns: i64,
    pub duration_secs: f64,

    // Message counters
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_failed: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub out_of_order: u64,
    pub duplicates: u64,
    pub messages_lost: u64,
    pub loss_rate_percent: f64,

    // Throughput
    pub messages_per_second: f64,
    pub bytes_per_second: f64,

    // Latency statistics (nanoseconds)
    pub latency_min_ns: i64,
    pub latency_max_ns: i64,
    pub latency_mean_ns: f64,
    pub latency_std_dev_ns: f64,
    pub latency_p50_ns: i64,
    pub latency_p95_ns: i64,
    pub latency_p99_ns: i64,
    pub latency_sample_count: u64,

    // Jitter statistics (nanoseconds)
    pub jitter_mean_ns: f64,
    pub jitter_std_dev_ns: f64,
    pub jitter_max_ns: i64,

    // System metrics
    pub cpu_mean_percent: f64,
    pub cpu_max_percent: f64,
    pub cpu_min_percent: f64,
    pub memory_mean_bytes: u64,
    pub memory_max_bytes: u64,
    pub memory_min_bytes: u64,
    pub memory_mean_percent: f64,

    // Overhead info
    pub metrics_overhead_bytes: u64,
    pub baseline_memory_bytes: u64,

    // Raw samples for time-series analysis
    pub system_samples: Vec<SystemSample>,
}

impl AggregatedMetrics {
    /// Compute aggregated metrics from collector
    pub fn from_collector(collector: &MetricsCollector) -> Self {
        let counters = collector.counters.snapshot();
        let start_time_ns = collector.get_start_time_ns();
        let end_time_ns = collector.get_end_time_ns();
        let duration_secs = (end_time_ns - start_time_ns) as f64 / 1_000_000_000.0;

        // Calculate loss
        let expected = counters.highest_sequence.max(counters.messages_sent);
        let messages_lost = expected.saturating_sub(counters.messages_received);
        let loss_rate_percent = if expected > 0 {
            (messages_lost as f64 / expected as f64) * 100.0
        } else {
            0.0
        };

        // Calculate throughput
        let messages_per_second = if duration_secs > 0.0 {
            counters.messages_received.max(counters.messages_sent) as f64 / duration_secs
        } else {
            0.0
        };
        let bytes_per_second = if duration_secs > 0.0 {
            counters.bytes_received.max(counters.bytes_sent) as f64 / duration_secs
        } else {
            0.0
        };

        // Calculate latency statistics
        let mut latency_samples = collector.latency_reservoir.get_samples();
        latency_samples.sort();

        let (latency_min_ns, latency_max_ns, latency_mean_ns, latency_std_dev_ns) =
            if !latency_samples.is_empty() {
                let min = *latency_samples.first().unwrap();
                let max = *latency_samples.last().unwrap();
                let sum: i64 = latency_samples.iter().sum();
                let mean = sum as f64 / latency_samples.len() as f64;
                let variance: f64 = latency_samples
                    .iter()
                    .map(|&x| {
                        let diff = x as f64 - mean;
                        diff * diff
                    })
                    .sum::<f64>()
                    / latency_samples.len() as f64;
                (min, max, mean, variance.sqrt())
            } else {
                (0, 0, 0.0, 0.0)
            };

        let latency_p50_ns = percentile(&latency_samples, 50.0);
        let latency_p95_ns = percentile(&latency_samples, 95.0);
        let latency_p99_ns = percentile(&latency_samples, 99.0);

        // Calculate jitter statistics
        let jitter_stats = collector.jitter_stats.get_stats();

        // Calculate system metrics statistics
        let system_samples = collector.get_system_samples();
        let (cpu_mean, cpu_max, cpu_min, mem_mean, mem_max, mem_min, mem_percent_mean) =
            calculate_system_stats(&system_samples);

        let overhead = collector.get_overhead();

        Self {
            start_time_ns,
            end_time_ns,
            duration_secs,
            messages_sent: counters.messages_sent,
            messages_received: counters.messages_received,
            messages_failed: counters.messages_failed,
            bytes_sent: counters.bytes_sent,
            bytes_received: counters.bytes_received,
            out_of_order: counters.out_of_order,
            duplicates: counters.duplicates,
            messages_lost,
            loss_rate_percent,
            messages_per_second,
            bytes_per_second,
            latency_min_ns,
            latency_max_ns,
            latency_mean_ns,
            latency_std_dev_ns,
            latency_p50_ns,
            latency_p95_ns,
            latency_p99_ns,
            latency_sample_count: collector.latency_reservoir.total_count(),
            jitter_mean_ns: jitter_stats.mean,
            jitter_std_dev_ns: jitter_stats.std_dev,
            jitter_max_ns: jitter_stats.max,
            cpu_mean_percent: cpu_mean,
            cpu_max_percent: cpu_max,
            cpu_min_percent: cpu_min,
            memory_mean_bytes: mem_mean,
            memory_max_bytes: mem_max,
            memory_min_bytes: mem_min,
            memory_mean_percent: mem_percent_mean,
            metrics_overhead_bytes: overhead.estimated_bytes,
            baseline_memory_bytes: overhead.baseline_memory_bytes,
            system_samples,
        }
    }

    /// Save metrics to JSON file
    pub fn save_to_file(&self, path: &str) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)?;
        println!("Results saved to: {}", path);
        Ok(())
    }

    /// Print a summary of the metrics
    pub fn print_summary(&self, role: &str) {
        println!("\n===========================================");
        println!("         {} RESULTS SUMMARY", role.to_uppercase());
        println!("===========================================");
        println!("Duration: {:.2} seconds", self.duration_secs);
        println!("-------------------------------------------");
        println!("MESSAGE METRICS:");
        println!("  Sent: {}", self.messages_sent);
        println!("  Received: {}", self.messages_received);
        println!("  Failed: {}", self.messages_failed);
        println!("  Lost: {}", self.messages_lost);
        println!("  Loss Rate: {:.2}%", self.loss_rate_percent);
        println!("  Out of Order: {}", self.out_of_order);
        println!("  Duplicates: {}", self.duplicates);
        println!("-------------------------------------------");
        println!("THROUGHPUT:");
        println!("  Messages/sec: {:.2}", self.messages_per_second);
        println!(
            "  Throughput: {:.2} MB/s",
            self.bytes_per_second / 1_048_576.0
        );
        println!("-------------------------------------------");
        println!("LATENCY (from {} samples):", self.latency_sample_count);
        println!("  Min: {:.3} ms", self.latency_min_ns as f64 / 1_000_000.0);
        println!("  Max: {:.3} ms", self.latency_max_ns as f64 / 1_000_000.0);
        println!("  Mean: {:.3} ms", self.latency_mean_ns / 1_000_000.0);
        println!("  Std Dev: {:.3} ms", self.latency_std_dev_ns / 1_000_000.0);
        println!("  P50: {:.3} ms", self.latency_p50_ns as f64 / 1_000_000.0);
        println!("  P95: {:.3} ms", self.latency_p95_ns as f64 / 1_000_000.0);
        println!("  P99: {:.3} ms", self.latency_p99_ns as f64 / 1_000_000.0);
        println!("-------------------------------------------");
        println!("JITTER:");
        println!("  Mean: {:.3} ms", self.jitter_mean_ns / 1_000_000.0);
        println!("  Std Dev: {:.3} ms", self.jitter_std_dev_ns / 1_000_000.0);
        println!("  Max: {:.3} ms", self.jitter_max_ns as f64 / 1_000_000.0);
        println!("-------------------------------------------");
        println!("CPU USAGE (corrected for benchmark overhead):");
        println!("  Mean: {:.2}%", self.cpu_mean_percent);
        println!("  Max: {:.2}%", self.cpu_max_percent);
        println!("  Min: {:.2}%", self.cpu_min_percent);
        println!("-------------------------------------------");
        println!("MEMORY USAGE (corrected for benchmark overhead):");
        println!(
            "  Mean: {:.2} MB",
            self.memory_mean_bytes as f64 / 1_048_576.0
        );
        println!(
            "  Max: {:.2} MB",
            self.memory_max_bytes as f64 / 1_048_576.0
        );
        println!(
            "  Min: {:.2} MB",
            self.memory_min_bytes as f64 / 1_048_576.0
        );
        println!("  Mean (%): {:.2}%", self.memory_mean_percent);
        println!("-------------------------------------------");
        println!("BENCHMARK OVERHEAD:");
        println!(
            "  Estimated metrics memory: {:.2} KB",
            self.metrics_overhead_bytes as f64 / 1024.0
        );
        println!(
            "  Baseline memory: {:.2} MB",
            self.baseline_memory_bytes as f64 / 1_048_576.0
        );
        println!("===========================================\n");
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

/// Calculate system metrics statistics
fn calculate_system_stats(samples: &[SystemSample]) -> (f64, f64, f64, u64, u64, u64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0, 0, 0, 0, 0.0);
    }

    let cpu_values: Vec<f64> = samples.iter().map(|s| s.cpu_usage_percent).collect();
    let mem_values: Vec<u64> = samples.iter().map(|s| s.memory_usage_bytes).collect();

    let cpu_mean = cpu_values.iter().sum::<f64>() / cpu_values.len() as f64;
    let cpu_max = cpu_values.iter().cloned().fold(f64::MIN, f64::max);
    let cpu_min = cpu_values.iter().cloned().fold(f64::MAX, f64::min);

    let mem_mean = mem_values.iter().sum::<u64>() / mem_values.len() as u64;
    let mem_max = *mem_values.iter().max().unwrap_or(&0);
    let mem_min = *mem_values.iter().min().unwrap_or(&0);

    let mem_percent_mean: f64 = samples
        .iter()
        .map(|s| {
            if s.total_memory_bytes > 0 {
                (s.memory_usage_bytes as f64 / s.total_memory_bytes as f64) * 100.0
            } else {
                0.0
            }
        })
        .sum::<f64>()
        / samples.len() as f64;

    (
        cpu_mean,
        cpu_max,
        cpu_min,
        mem_mean,
        mem_max,
        mem_min,
        mem_percent_mean,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_counters() {
        let counters = AtomicCounters::new();
        counters.inc_sent();
        counters.inc_sent();
        counters.inc_received();
        counters.inc_failed();

        let snapshot = counters.snapshot();
        assert_eq!(snapshot.messages_sent, 2);
        assert_eq!(snapshot.messages_received, 1);
        assert_eq!(snapshot.messages_failed, 1);
    }

    #[test]
    fn test_latency_reservoir() {
        let reservoir = LatencyReservoir::new(100);

        // Add fewer samples than capacity
        for i in 0..50 {
            reservoir.add_sample(i * 1000);
        }

        let samples = reservoir.get_samples();
        assert_eq!(samples.len(), 50);
        assert_eq!(reservoir.total_count(), 50);
    }

    #[test]
    fn test_latency_reservoir_overflow() {
        let reservoir = LatencyReservoir::new(10);

        // Add more samples than capacity
        for i in 0..1000 {
            reservoir.add_sample(i * 1000);
        }

        let samples = reservoir.get_samples();
        assert_eq!(samples.len(), 10); // Should only keep 10
        assert_eq!(reservoir.total_count(), 1000); // But track total count
    }

    #[test]
    fn test_running_stats() {
        let stats = RunningStats::new();
        stats.add_value(10);
        stats.add_value(20);
        stats.add_value(30);

        let snapshot = stats.get_stats();
        assert_eq!(snapshot.count, 3);
        assert!((snapshot.mean - 20.0).abs() < 0.001);
        assert_eq!(snapshot.min, 10);
        assert_eq!(snapshot.max, 30);
    }

    #[test]
    fn test_percentile() {
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(percentile(&values, 50.0), 5);
        assert_eq!(percentile(&values, 90.0), 9);
        assert_eq!(percentile(&values, 100.0), 10);
    }
}
