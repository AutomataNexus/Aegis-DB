//! Aegis Metrics Collection
//!
//! Prometheus-compatible metrics for monitoring database performance.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

// =============================================================================
// Metric Types
// =============================================================================

/// Type of metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

/// A metric value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(HistogramValue),
    Summary(SummaryValue),
}

// =============================================================================
// Counter
// =============================================================================

/// A monotonically increasing counter.
#[derive(Debug, Default)]
pub struct Counter {
    value: AtomicU64,
    labels: HashMap<String, String>,
}

impl Counter {
    /// Create a new counter.
    pub fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
            labels: HashMap::new(),
        }
    }

    /// Create with labels.
    pub fn with_labels(labels: HashMap<String, String>) -> Self {
        Self {
            value: AtomicU64::new(0),
            labels,
        }
    }

    /// Increment by 1.
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment by a specific amount.
    pub fn inc_by(&self, amount: u64) {
        self.value.fetch_add(amount, Ordering::Relaxed);
    }

    /// Get the current value.
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Reset the counter.
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }

    /// Get labels.
    pub fn labels(&self) -> &HashMap<String, String> {
        &self.labels
    }
}

// =============================================================================
// Gauge
// =============================================================================

/// A gauge that can increase and decrease.
#[derive(Debug, Default)]
pub struct Gauge {
    value: AtomicI64,
    labels: HashMap<String, String>,
}

impl Gauge {
    /// Create a new gauge.
    pub fn new() -> Self {
        Self {
            value: AtomicI64::new(0),
            labels: HashMap::new(),
        }
    }

    /// Create with labels.
    pub fn with_labels(labels: HashMap<String, String>) -> Self {
        Self {
            value: AtomicI64::new(0),
            labels,
        }
    }

    /// Set the value.
    pub fn set(&self, value: i64) {
        self.value.store(value, Ordering::Relaxed);
    }

    /// Set from f64.
    pub fn set_f64(&self, value: f64) {
        self.value.store((value * 1000.0) as i64, Ordering::Relaxed);
    }

    /// Increment by 1.
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement by 1.
    pub fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    /// Add a value.
    pub fn add(&self, value: i64) {
        self.value.fetch_add(value, Ordering::Relaxed);
    }

    /// Subtract a value.
    pub fn sub(&self, value: i64) {
        self.value.fetch_sub(value, Ordering::Relaxed);
    }

    /// Get the current value.
    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Get as f64.
    pub fn get_f64(&self) -> f64 {
        self.value.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get labels.
    pub fn labels(&self) -> &HashMap<String, String> {
        &self.labels
    }
}

// =============================================================================
// Histogram
// =============================================================================

/// Histogram value with buckets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramValue {
    pub count: u64,
    pub sum: f64,
    pub buckets: Vec<(f64, u64)>,
}

/// A histogram for measuring distributions.
#[derive(Debug)]
pub struct Histogram {
    count: AtomicU64,
    sum: RwLock<f64>,
    buckets: Vec<f64>,
    bucket_counts: Vec<AtomicU64>,
    labels: HashMap<String, String>,
}

impl Histogram {
    /// Default bucket boundaries (in seconds).
    pub const DEFAULT_BUCKETS: &'static [f64] = &[
        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];

    /// Create a new histogram with default buckets.
    pub fn new() -> Self {
        Self::with_buckets(Self::DEFAULT_BUCKETS.to_vec())
    }

    /// Create with custom bucket boundaries.
    pub fn with_buckets(buckets: Vec<f64>) -> Self {
        let bucket_counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Self {
            count: AtomicU64::new(0),
            sum: RwLock::new(0.0),
            buckets,
            bucket_counts,
            labels: HashMap::new(),
        }
    }

    /// Create with labels.
    pub fn with_labels(labels: HashMap<String, String>) -> Self {
        let mut h = Self::new();
        h.labels = labels;
        h
    }

    /// Observe a value.
    pub fn observe(&self, value: f64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        *self.sum.write().expect("Histogram sum RwLock poisoned") += value;

        for (i, &boundary) in self.buckets.iter().enumerate() {
            if value <= boundary {
                self.bucket_counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Observe a duration.
    pub fn observe_duration(&self, duration: Duration) {
        self.observe(duration.as_secs_f64());
    }

    /// Start a timer that observes when dropped.
    pub fn start_timer(&self) -> HistogramTimer<'_> {
        HistogramTimer {
            histogram: self,
            start: Instant::now(),
        }
    }

    /// Get the count of observations.
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Get the sum of observations.
    pub fn sum(&self) -> f64 {
        *self.sum.read().expect("Histogram sum RwLock poisoned")
    }

    /// Get the histogram value.
    pub fn value(&self) -> HistogramValue {
        let buckets: Vec<_> = self
            .buckets
            .iter()
            .zip(&self.bucket_counts)
            .map(|(&b, c)| (b, c.load(Ordering::Relaxed)))
            .collect();

        HistogramValue {
            count: self.count(),
            sum: self.sum(),
            buckets,
        }
    }

    /// Get labels.
    pub fn labels(&self) -> &HashMap<String, String> {
        &self.labels
    }

    /// Reset the histogram.
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        *self.sum.write().expect("Histogram sum RwLock poisoned") = 0.0;
        for bc in &self.bucket_counts {
            bc.store(0, Ordering::Relaxed);
        }
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer for histogram observations.
pub struct HistogramTimer<'a> {
    histogram: &'a Histogram,
    start: Instant,
}

impl<'a> Drop for HistogramTimer<'a> {
    fn drop(&mut self) {
        self.histogram.observe_duration(self.start.elapsed());
    }
}

// =============================================================================
// Summary
// =============================================================================

/// Summary value with quantiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryValue {
    pub count: u64,
    pub sum: f64,
    pub quantiles: Vec<(f64, f64)>,
}

/// A summary for calculating quantiles.
#[derive(Debug)]
pub struct Summary {
    count: AtomicU64,
    sum: RwLock<f64>,
    values: RwLock<Vec<f64>>,
    quantiles: Vec<f64>,
    max_samples: usize,
    labels: HashMap<String, String>,
}

impl Summary {
    /// Default quantiles.
    pub const DEFAULT_QUANTILES: &'static [f64] = &[0.5, 0.9, 0.95, 0.99];

    /// Create a new summary with default quantiles.
    pub fn new() -> Self {
        Self::with_quantiles(Self::DEFAULT_QUANTILES.to_vec())
    }

    /// Create with custom quantiles.
    pub fn with_quantiles(quantiles: Vec<f64>) -> Self {
        Self {
            count: AtomicU64::new(0),
            sum: RwLock::new(0.0),
            values: RwLock::new(Vec::new()),
            quantiles,
            max_samples: 1000,
            labels: HashMap::new(),
        }
    }

    /// Create with labels.
    pub fn with_labels(labels: HashMap<String, String>) -> Self {
        let mut s = Self::new();
        s.labels = labels;
        s
    }

    /// Observe a value.
    pub fn observe(&self, value: f64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        *self.sum.write().expect("Summary sum RwLock poisoned") += value;

        let mut values = self.values.write().expect("Summary values RwLock poisoned");
        values.push(value);

        // Keep only max_samples
        if values.len() > self.max_samples {
            values.remove(0);
        }
    }

    /// Observe a duration.
    pub fn observe_duration(&self, duration: Duration) {
        self.observe(duration.as_secs_f64());
    }

    /// Get the count of observations.
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Get the sum of observations.
    pub fn sum(&self) -> f64 {
        *self.sum.read().expect("Summary sum RwLock poisoned")
    }

    /// Get the summary value with quantiles.
    pub fn value(&self) -> SummaryValue {
        let values = self.values.read().expect("Summary values RwLock poisoned");
        let mut sorted: Vec<f64> = values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let quantiles: Vec<_> = self
            .quantiles
            .iter()
            .map(|&q| {
                let idx = ((sorted.len() as f64) * q) as usize;
                let idx = idx.min(sorted.len().saturating_sub(1));
                (q, sorted.get(idx).copied().unwrap_or(0.0))
            })
            .collect();

        SummaryValue {
            count: self.count(),
            sum: self.sum(),
            quantiles,
        }
    }

    /// Get labels.
    pub fn labels(&self) -> &HashMap<String, String> {
        &self.labels
    }

    /// Reset the summary.
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        *self.sum.write().expect("Summary sum RwLock poisoned") = 0.0;
        self.values.write().expect("Summary values RwLock poisoned").clear();
    }
}

impl Default for Summary {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Metric Registry
// =============================================================================

/// A registry for managing metrics.
#[derive(Debug, Default)]
pub struct MetricRegistry {
    counters: RwLock<HashMap<String, Arc<Counter>>>,
    gauges: RwLock<HashMap<String, Arc<Gauge>>>,
    histograms: RwLock<HashMap<String, Arc<Histogram>>>,
    summaries: RwLock<HashMap<String, Arc<Summary>>>,
}

impl MetricRegistry {
    /// Create a new registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register or get a counter.
    pub fn counter(&self, name: &str) -> Arc<Counter> {
        let mut counters = self.counters.write().expect("MetricRegistry counters RwLock poisoned");
        counters
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Counter::new()))
            .clone()
    }

    /// Register or get a counter with labels.
    pub fn counter_with_labels(
        &self,
        name: &str,
        labels: HashMap<String, String>,
    ) -> Arc<Counter> {
        let key = Self::labeled_key(name, &labels);
        let mut counters = self.counters.write().expect("MetricRegistry counters RwLock poisoned");
        counters
            .entry(key)
            .or_insert_with(|| Arc::new(Counter::with_labels(labels)))
            .clone()
    }

    /// Register or get a gauge.
    pub fn gauge(&self, name: &str) -> Arc<Gauge> {
        let mut gauges = self.gauges.write().expect("MetricRegistry gauges RwLock poisoned");
        gauges
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Gauge::new()))
            .clone()
    }

    /// Register or get a gauge with labels.
    pub fn gauge_with_labels(&self, name: &str, labels: HashMap<String, String>) -> Arc<Gauge> {
        let key = Self::labeled_key(name, &labels);
        let mut gauges = self.gauges.write().expect("MetricRegistry gauges RwLock poisoned");
        gauges
            .entry(key)
            .or_insert_with(|| Arc::new(Gauge::with_labels(labels)))
            .clone()
    }

    /// Register or get a histogram.
    pub fn histogram(&self, name: &str) -> Arc<Histogram> {
        let mut histograms = self.histograms.write().expect("MetricRegistry histograms RwLock poisoned");
        histograms
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Histogram::new()))
            .clone()
    }

    /// Register or get a histogram with custom buckets.
    pub fn histogram_with_buckets(&self, name: &str, buckets: Vec<f64>) -> Arc<Histogram> {
        let mut histograms = self.histograms.write().expect("MetricRegistry histograms RwLock poisoned");
        histograms
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Histogram::with_buckets(buckets)))
            .clone()
    }

    /// Register or get a summary.
    pub fn summary(&self, name: &str) -> Arc<Summary> {
        let mut summaries = self.summaries.write().expect("MetricRegistry summaries RwLock poisoned");
        summaries
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Summary::new()))
            .clone()
    }

    /// Get all counter values.
    pub fn get_counters(&self) -> HashMap<String, u64> {
        self.counters
            .read()
            .expect("MetricRegistry counters RwLock poisoned")
            .iter()
            .map(|(k, v)| (k.clone(), v.get()))
            .collect()
    }

    /// Get all gauge values.
    pub fn get_gauges(&self) -> HashMap<String, i64> {
        self.gauges
            .read()
            .expect("MetricRegistry gauges RwLock poisoned")
            .iter()
            .map(|(k, v)| (k.clone(), v.get()))
            .collect()
    }

    /// Get all histogram values.
    pub fn get_histograms(&self) -> HashMap<String, HistogramValue> {
        self.histograms
            .read()
            .expect("MetricRegistry histograms RwLock poisoned")
            .iter()
            .map(|(k, v)| (k.clone(), v.value()))
            .collect()
    }

    /// Get all summary values.
    pub fn get_summaries(&self) -> HashMap<String, SummaryValue> {
        self.summaries
            .read()
            .expect("MetricRegistry summaries RwLock poisoned")
            .iter()
            .map(|(k, v)| (k.clone(), v.value()))
            .collect()
    }

    /// Export metrics in Prometheus format.
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // Export counters
        for (name, counter) in self.counters.read().expect("MetricRegistry counters RwLock poisoned").iter() {
            output.push_str(&format!(
                "# TYPE {} counter\n{} {}\n",
                name,
                name,
                counter.get()
            ));
        }

        // Export gauges
        for (name, gauge) in self.gauges.read().expect("MetricRegistry gauges RwLock poisoned").iter() {
            output.push_str(&format!(
                "# TYPE {} gauge\n{} {}\n",
                name,
                name,
                gauge.get()
            ));
        }

        // Export histograms
        for (name, histogram) in self.histograms.read().expect("MetricRegistry histograms RwLock poisoned").iter() {
            output.push_str(&format!("# TYPE {} histogram\n", name));
            let value = histogram.value();
            for (boundary, count) in &value.buckets {
                output.push_str(&format!("{}_bucket{{le=\"{}\"}} {}\n", name, boundary, count));
            }
            output.push_str(&format!("{}_bucket{{le=\"+Inf\"}} {}\n", name, value.count));
            output.push_str(&format!("{}_sum {}\n", name, value.sum));
            output.push_str(&format!("{}_count {}\n", name, value.count));
        }

        // Export summaries
        for (name, summary) in self.summaries.read().expect("MetricRegistry summaries RwLock poisoned").iter() {
            output.push_str(&format!("# TYPE {} summary\n", name));
            let value = summary.value();
            for (quantile, v) in &value.quantiles {
                output.push_str(&format!("{}{{quantile=\"{}\"}} {}\n", name, quantile, v));
            }
            output.push_str(&format!("{}_sum {}\n", name, value.sum));
            output.push_str(&format!("{}_count {}\n", name, value.count));
        }

        output
    }

    /// Create a labeled metric key.
    fn labeled_key(name: &str, labels: &HashMap<String, String>) -> String {
        if labels.is_empty() {
            name.to_string()
        } else {
            let mut parts: Vec<_> = labels.iter().collect();
            parts.sort_by_key(|(k, _)| *k);
            let label_str: Vec<_> = parts.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
            format!("{}_{{{}}}", name, label_str.join(","))
        }
    }
}

// =============================================================================
// Database Metrics
// =============================================================================

/// Pre-defined database metrics.
pub struct DatabaseMetrics {
    pub registry: MetricRegistry,
    pub queries_total: Arc<Counter>,
    pub query_duration: Arc<Histogram>,
    pub active_connections: Arc<Gauge>,
    pub bytes_read: Arc<Counter>,
    pub bytes_written: Arc<Counter>,
    pub cache_hits: Arc<Counter>,
    pub cache_misses: Arc<Counter>,
    pub transactions_total: Arc<Counter>,
    pub transaction_duration: Arc<Histogram>,
    pub errors_total: Arc<Counter>,
}

impl DatabaseMetrics {
    /// Create a new set of database metrics.
    pub fn new() -> Self {
        let registry = MetricRegistry::new();

        Self {
            queries_total: registry.counter("aegis_queries_total"),
            query_duration: registry.histogram("aegis_query_duration_seconds"),
            active_connections: registry.gauge("aegis_active_connections"),
            bytes_read: registry.counter("aegis_bytes_read_total"),
            bytes_written: registry.counter("aegis_bytes_written_total"),
            cache_hits: registry.counter("aegis_cache_hits_total"),
            cache_misses: registry.counter("aegis_cache_misses_total"),
            transactions_total: registry.counter("aegis_transactions_total"),
            transaction_duration: registry.histogram("aegis_transaction_duration_seconds"),
            errors_total: registry.counter("aegis_errors_total"),
            registry,
        }
    }

    /// Record a query execution.
    pub fn record_query(&self, duration: Duration) {
        self.queries_total.inc();
        self.query_duration.observe_duration(duration);
    }

    /// Record a transaction.
    pub fn record_transaction(&self, duration: Duration) {
        self.transactions_total.inc();
        self.transaction_duration.observe_duration(duration);
    }

    /// Record bytes read.
    pub fn record_read(&self, bytes: u64) {
        self.bytes_read.inc_by(bytes);
    }

    /// Record bytes written.
    pub fn record_write(&self, bytes: u64) {
        self.bytes_written.inc_by(bytes);
    }

    /// Record a cache hit.
    pub fn record_cache_hit(&self) {
        self.cache_hits.inc();
    }

    /// Record a cache miss.
    pub fn record_cache_miss(&self) {
        self.cache_misses.inc();
    }

    /// Record an error.
    pub fn record_error(&self) {
        self.errors_total.inc();
    }

    /// Set active connection count.
    pub fn set_active_connections(&self, count: i64) {
        self.active_connections.set(count);
    }

    /// Get cache hit ratio.
    pub fn cache_hit_ratio(&self) -> f64 {
        let hits = self.cache_hits.get() as f64;
        let misses = self.cache_misses.get() as f64;
        let total = hits + misses;
        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }

    /// Export in Prometheus format.
    pub fn export_prometheus(&self) -> String {
        self.registry.export_prometheus()
    }
}

impl Default for DatabaseMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter() {
        let counter = Counter::new();
        assert_eq!(counter.get(), 0);

        counter.inc();
        assert_eq!(counter.get(), 1);

        counter.inc_by(5);
        assert_eq!(counter.get(), 6);

        counter.reset();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_gauge() {
        let gauge = Gauge::new();
        assert_eq!(gauge.get(), 0);

        gauge.set(100);
        assert_eq!(gauge.get(), 100);

        gauge.inc();
        assert_eq!(gauge.get(), 101);

        gauge.dec();
        assert_eq!(gauge.get(), 100);

        gauge.add(50);
        assert_eq!(gauge.get(), 150);

        gauge.sub(30);
        assert_eq!(gauge.get(), 120);
    }

    #[test]
    fn test_histogram() {
        let histogram = Histogram::new();

        histogram.observe(0.005);
        histogram.observe(0.05);
        histogram.observe(0.5);
        histogram.observe(5.0);

        assert_eq!(histogram.count(), 4);

        let value = histogram.value();
        assert_eq!(value.count, 4);
    }

    #[test]
    fn test_histogram_timer() {
        let histogram = Histogram::new();

        {
            let _timer = histogram.start_timer();
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(histogram.count(), 1);
        assert!(histogram.sum() > 0.0);
    }

    #[test]
    fn test_summary() {
        let summary = Summary::new();

        for i in 1..=100 {
            summary.observe(i as f64);
        }

        assert_eq!(summary.count(), 100);

        let value = summary.value();
        assert!(!value.quantiles.is_empty());
    }

    #[test]
    fn test_registry() {
        let registry = MetricRegistry::new();

        let counter = registry.counter("requests");
        counter.inc();
        counter.inc();

        let gauge = registry.gauge("connections");
        gauge.set(10);

        let counters = registry.get_counters();
        assert_eq!(counters.get("requests"), Some(&2));

        let gauges = registry.get_gauges();
        assert_eq!(gauges.get("connections"), Some(&10));
    }

    #[test]
    fn test_registry_with_labels() {
        let registry = MetricRegistry::new();

        let mut labels = HashMap::new();
        labels.insert("method".to_string(), "GET".to_string());

        let counter = registry.counter_with_labels("http_requests", labels);
        counter.inc();

        assert_eq!(counter.get(), 1);
    }

    #[test]
    fn test_database_metrics() {
        let metrics = DatabaseMetrics::new();

        metrics.record_query(Duration::from_millis(10));
        metrics.record_query(Duration::from_millis(20));

        assert_eq!(metrics.queries_total.get(), 2);

        metrics.record_cache_hit();
        metrics.record_cache_hit();
        metrics.record_cache_miss();

        let ratio = metrics.cache_hit_ratio();
        assert!((ratio - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_prometheus_export() {
        let registry = MetricRegistry::new();

        let counter = registry.counter("test_counter");
        counter.inc_by(42);

        let gauge = registry.gauge("test_gauge");
        gauge.set(100);

        let output = registry.export_prometheus();
        assert!(output.contains("test_counter 42"));
        assert!(output.contains("test_gauge 100"));
    }
}
