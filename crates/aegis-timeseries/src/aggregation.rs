//! Aegis Time Series Aggregation
//!
//! Aggregation functions and downsampling for time series data.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::types::DataPoint;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

// =============================================================================
// Aggregate Function
// =============================================================================

/// Aggregation function type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregateFunction {
    Sum,
    Count,
    Min,
    Max,
    Avg,
    First,
    Last,
    Median,
    StdDev,
    Variance,
    Rate,
    Increase,
}

impl AggregateFunction {
    /// Apply the aggregation to a set of values.
    pub fn apply(&self, values: &[f64]) -> Option<f64> {
        if values.is_empty() {
            return None;
        }

        Some(match self {
            Self::Sum => values.iter().sum(),
            Self::Count => values.len() as f64,
            Self::Min => values.iter().copied().fold(f64::INFINITY, f64::min),
            Self::Max => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            Self::Avg => values.iter().sum::<f64>() / values.len() as f64,
            Self::First => *values.first()?,
            Self::Last => *values.last()?,
            Self::Median => {
                let mut sorted = values.to_vec();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let mid = sorted.len() / 2;
                if sorted.len() % 2 == 0 {
                    (sorted[mid - 1] + sorted[mid]) / 2.0
                } else {
                    sorted[mid]
                }
            }
            Self::StdDev => {
                let variance = Self::Variance.apply(values)?;
                variance.sqrt()
            }
            Self::Variance => {
                let mean = Self::Avg.apply(values)?;
                let sum_sq: f64 = values.iter().map(|v| (v - mean).powi(2)).sum();
                sum_sq / values.len() as f64
            }
            Self::Rate => {
                if values.len() < 2 {
                    return None;
                }
                (values.last()? - values.first()?) / (values.len() - 1) as f64
            }
            Self::Increase => {
                if values.len() < 2 {
                    return None;
                }
                values.last()? - values.first()?
            }
        })
    }
}

// =============================================================================
// Aggregator
// =============================================================================

/// Streaming aggregator for data points.
pub struct Aggregator {
    function: AggregateFunction,
    values: Vec<f64>,
    count: usize,
    sum: f64,
    min: f64,
    max: f64,
    first: Option<f64>,
    last: Option<f64>,
}

impl Aggregator {
    pub fn new(function: AggregateFunction) -> Self {
        Self {
            function,
            values: Vec::new(),
            count: 0,
            sum: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            first: None,
            last: None,
        }
    }

    /// Add a value to the aggregator.
    pub fn add(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        if self.first.is_none() {
            self.first = Some(value);
        }
        self.last = Some(value);

        match self.function {
            AggregateFunction::Median
            | AggregateFunction::StdDev
            | AggregateFunction::Variance
            | AggregateFunction::Rate
            | AggregateFunction::Increase => {
                self.values.push(value);
            }
            _ => {}
        }
    }

    /// Get the current aggregate value.
    pub fn value(&self) -> Option<f64> {
        if self.count == 0 {
            return None;
        }

        Some(match self.function {
            AggregateFunction::Sum => self.sum,
            AggregateFunction::Count => self.count as f64,
            AggregateFunction::Min => self.min,
            AggregateFunction::Max => self.max,
            AggregateFunction::Avg => self.sum / self.count as f64,
            AggregateFunction::First => self.first?,
            AggregateFunction::Last => self.last?,
            AggregateFunction::Median
            | AggregateFunction::StdDev
            | AggregateFunction::Variance
            | AggregateFunction::Rate
            | AggregateFunction::Increase => self.function.apply(&self.values)?,
        })
    }

    /// Reset the aggregator.
    pub fn reset(&mut self) {
        self.values.clear();
        self.count = 0;
        self.sum = 0.0;
        self.min = f64::INFINITY;
        self.max = f64::NEG_INFINITY;
        self.first = None;
        self.last = None;
    }
}

// =============================================================================
// Downsampler
// =============================================================================

/// Downsamples time series data to a lower resolution.
pub struct Downsampler {
    interval: Duration,
    function: AggregateFunction,
}

impl Downsampler {
    pub fn new(interval: Duration, function: AggregateFunction) -> Self {
        Self { interval, function }
    }

    /// Downsample data points.
    pub fn downsample(&self, points: &[DataPoint]) -> Vec<DataPoint> {
        if points.is_empty() {
            return Vec::new();
        }

        let mut result = Vec::new();
        let mut current_bucket: Option<DateTime<Utc>> = None;
        let mut aggregator = Aggregator::new(self.function);

        for point in points {
            let bucket = self.bucket_start(point.timestamp);

            if Some(bucket) != current_bucket {
                if let Some(bucket_time) = current_bucket {
                    if let Some(value) = aggregator.value() {
                        result.push(DataPoint {
                            timestamp: bucket_time,
                            value,
                        });
                    }
                }
                current_bucket = Some(bucket);
                aggregator.reset();
            }

            aggregator.add(point.value);
        }

        if let Some(bucket_time) = current_bucket {
            if let Some(value) = aggregator.value() {
                result.push(DataPoint {
                    timestamp: bucket_time,
                    value,
                });
            }
        }

        result
    }

    fn bucket_start(&self, timestamp: DateTime<Utc>) -> DateTime<Utc> {
        let millis = timestamp.timestamp_millis();
        let interval_millis = self.interval.num_milliseconds();
        let bucket_millis = (millis / interval_millis) * interval_millis;
        DateTime::from_timestamp_millis(bucket_millis).expect("failed to create DateTime from bucket timestamp")
    }
}

// =============================================================================
// Rolling Window
// =============================================================================

/// Rolling window aggregation.
pub struct RollingWindow {
    window_size: Duration,
    function: AggregateFunction,
}

impl RollingWindow {
    pub fn new(window_size: Duration, function: AggregateFunction) -> Self {
        Self {
            window_size,
            function,
        }
    }

    /// Apply rolling window aggregation.
    pub fn apply(&self, points: &[DataPoint]) -> Vec<DataPoint> {
        let mut result = Vec::with_capacity(points.len());

        for (i, point) in points.iter().enumerate() {
            let window_start = point.timestamp - self.window_size;

            let window_values: Vec<f64> = points[..=i]
                .iter()
                .filter(|p| p.timestamp >= window_start)
                .map(|p| p.value)
                .collect();

            if let Some(value) = self.function.apply(&window_values) {
                result.push(DataPoint {
                    timestamp: point.timestamp,
                    value,
                });
            }
        }

        result
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_functions() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        assert_eq!(AggregateFunction::Sum.apply(&values), Some(15.0));
        assert_eq!(AggregateFunction::Count.apply(&values), Some(5.0));
        assert_eq!(AggregateFunction::Min.apply(&values), Some(1.0));
        assert_eq!(AggregateFunction::Max.apply(&values), Some(5.0));
        assert_eq!(AggregateFunction::Avg.apply(&values), Some(3.0));
        assert_eq!(AggregateFunction::First.apply(&values), Some(1.0));
        assert_eq!(AggregateFunction::Last.apply(&values), Some(5.0));
        assert_eq!(AggregateFunction::Median.apply(&values), Some(3.0));
    }

    #[test]
    fn test_aggregator() {
        let mut agg = Aggregator::new(AggregateFunction::Avg);
        agg.add(10.0);
        agg.add(20.0);
        agg.add(30.0);

        assert_eq!(agg.value(), Some(20.0));
    }

    #[test]
    fn test_downsampler() {
        let base_time = DateTime::from_timestamp(1700000000, 0).expect("failed to create test base_time");
        let points: Vec<DataPoint> = (0..100)
            .map(|i| DataPoint {
                timestamp: base_time + Duration::seconds(i),
                value: i as f64,
            })
            .collect();

        let downsampler = Downsampler::new(Duration::seconds(10), AggregateFunction::Avg);
        let result = downsampler.downsample(&points);

        assert_eq!(result.len(), 10);
    }

    #[test]
    fn test_rolling_window() {
        let base_time = Utc::now();
        let points: Vec<DataPoint> = (0..10)
            .map(|i| DataPoint {
                timestamp: base_time + Duration::seconds(i),
                value: i as f64,
            })
            .collect();

        let window = RollingWindow::new(Duration::seconds(3), AggregateFunction::Avg);
        let result = window.apply(&points);

        assert_eq!(result.len(), 10);
    }
}
