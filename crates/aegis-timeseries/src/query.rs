//! Aegis Time Series Query
//!
//! Query execution for time series data.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::aggregation::{AggregateFunction, Downsampler, RollingWindow};
use crate::types::{DataPoint, Series, Tags};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

// =============================================================================
// Time Series Query
// =============================================================================

/// A time series query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesQuery {
    pub metric: String,
    pub tags: Option<Tags>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub aggregation: Option<QueryAggregation>,
    pub limit: Option<usize>,
}

impl TimeSeriesQuery {
    /// Create a new query for a metric.
    pub fn new(metric: impl Into<String>, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            metric: metric.into(),
            tags: None,
            start,
            end,
            aggregation: None,
            limit: None,
        }
    }

    /// Query the last N duration.
    pub fn last(metric: impl Into<String>, duration: Duration) -> Self {
        let end = Utc::now();
        let start = end - duration;
        Self::new(metric, start, end)
    }

    /// Add tag filters.
    pub fn with_tags(mut self, tags: Tags) -> Self {
        self.tags = Some(tags);
        self
    }

    /// Add aggregation.
    pub fn with_aggregation(mut self, aggregation: QueryAggregation) -> Self {
        self.aggregation = Some(aggregation);
        self
    }

    /// Add downsampling.
    pub fn downsample(self, interval: Duration, function: AggregateFunction) -> Self {
        self.with_aggregation(QueryAggregation::Downsample { interval, function })
    }

    /// Add a result limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Get the time range duration.
    pub fn duration(&self) -> Duration {
        self.end - self.start
    }
}

/// Aggregation configuration for queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryAggregation {
    Downsample {
        interval: Duration,
        function: AggregateFunction,
    },
    RollingWindow {
        window: Duration,
        function: AggregateFunction,
    },
    Instant {
        function: AggregateFunction,
    },
}

// =============================================================================
// Query Result
// =============================================================================

/// Result of a time series query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub series: Vec<Series>,
    pub query_time_ms: u64,
    pub points_scanned: usize,
    pub points_returned: usize,
}

impl QueryResult {
    pub fn empty() -> Self {
        Self {
            series: Vec::new(),
            query_time_ms: 0,
            points_scanned: 0,
            points_returned: 0,
        }
    }

    pub fn with_series(series: Vec<Series>) -> Self {
        let points_returned = series.iter().map(|s| s.points.len()).sum();
        Self {
            series,
            query_time_ms: 0,
            points_scanned: 0,
            points_returned,
        }
    }

    /// Get total number of data points.
    pub fn total_points(&self) -> usize {
        self.series.iter().map(|s| s.points.len()).sum()
    }

    /// Check if the result is empty.
    pub fn is_empty(&self) -> bool {
        self.series.is_empty() || self.total_points() == 0
    }
}

// =============================================================================
// Query Executor
// =============================================================================

/// Executes time series queries.
pub struct QueryExecutor;

impl QueryExecutor {
    /// Execute a query on a set of series.
    pub fn execute(query: &TimeSeriesQuery, series: Vec<Series>) -> QueryResult {
        let start_time = std::time::Instant::now();
        let mut points_scanned = 0;

        let mut result_series: Vec<Series> = series
            .into_iter()
            .filter(|s| {
                if let Some(ref filter) = query.tags {
                    s.tags.matches(filter)
                } else {
                    true
                }
            })
            .map(|mut s| {
                points_scanned += s.points.len();

                s.points = s
                    .points
                    .into_iter()
                    .filter(|p| p.timestamp >= query.start && p.timestamp < query.end)
                    .collect();

                if let Some(ref agg) = query.aggregation {
                    s.points = Self::apply_aggregation(&s.points, agg);
                }

                if let Some(limit) = query.limit {
                    s.points.truncate(limit);
                }

                s
            })
            .filter(|s| !s.points.is_empty())
            .collect();

        let points_returned = result_series.iter().map(|s| s.points.len()).sum();
        let query_time_ms = start_time.elapsed().as_millis() as u64;

        QueryResult {
            series: result_series,
            query_time_ms,
            points_scanned,
            points_returned,
        }
    }

    fn apply_aggregation(points: &[DataPoint], aggregation: &QueryAggregation) -> Vec<DataPoint> {
        match aggregation {
            QueryAggregation::Downsample { interval, function } => {
                let downsampler = Downsampler::new(*interval, *function);
                downsampler.downsample(points)
            }
            QueryAggregation::RollingWindow { window, function } => {
                let rolling = RollingWindow::new(*window, *function);
                rolling.apply(points)
            }
            QueryAggregation::Instant { function } => {
                let values: Vec<f64> = points.iter().map(|p| p.value).collect();
                if let Some(value) = function.apply(&values) {
                    if let Some(last) = points.last() {
                        vec![DataPoint {
                            timestamp: last.timestamp,
                            value,
                        }]
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            }
        }
    }
}

// =============================================================================
// Instant Query
// =============================================================================

/// Query for a single instant in time.
#[derive(Debug, Clone)]
pub struct InstantQuery {
    pub metric: String,
    pub tags: Option<Tags>,
    pub time: DateTime<Utc>,
    pub lookback: Duration,
}

impl InstantQuery {
    pub fn new(metric: impl Into<String>, time: DateTime<Utc>) -> Self {
        Self {
            metric: metric.into(),
            tags: None,
            time,
            lookback: Duration::minutes(5),
        }
    }

    pub fn now(metric: impl Into<String>) -> Self {
        Self::new(metric, Utc::now())
    }

    pub fn with_lookback(mut self, lookback: Duration) -> Self {
        self.lookback = lookback;
        self
    }

    /// Convert to a range query.
    pub fn to_range_query(&self) -> TimeSeriesQuery {
        let start = self.time - self.lookback;
        TimeSeriesQuery {
            metric: self.metric.clone(),
            tags: self.tags.clone(),
            start,
            end: self.time,
            aggregation: Some(QueryAggregation::Instant {
                function: AggregateFunction::Last,
            }),
            limit: Some(1),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Metric;

    fn create_test_series() -> Series {
        let metric = Metric::gauge("test_metric");
        let tags = Tags::new();
        let base_time = Utc::now() - Duration::hours(1);

        let points: Vec<DataPoint> = (0..100)
            .map(|i| DataPoint {
                timestamp: base_time + Duration::minutes(i),
                value: i as f64,
            })
            .collect();

        Series::with_points(metric, tags, points)
    }

    #[test]
    fn test_query_builder() {
        let query = TimeSeriesQuery::last("cpu_usage", Duration::hours(1))
            .downsample(Duration::minutes(5), AggregateFunction::Avg)
            .with_limit(100);

        assert_eq!(query.metric, "cpu_usage");
        assert!(query.aggregation.is_some());
        assert_eq!(query.limit, Some(100));
    }

    #[test]
    fn test_query_execution() {
        let series = create_test_series();
        let query = TimeSeriesQuery::last("test_metric", Duration::hours(2));

        let result = QueryExecutor::execute(&query, vec![series]);

        assert_eq!(result.series.len(), 1);
        assert!(result.points_returned > 0);
    }

    #[test]
    fn test_query_with_aggregation() {
        let series = create_test_series();
        let query = TimeSeriesQuery::last("test_metric", Duration::hours(2))
            .downsample(Duration::minutes(10), AggregateFunction::Avg);

        let result = QueryExecutor::execute(&query, vec![series]);

        assert_eq!(result.series.len(), 1);
        assert!(result.series[0].points.len() < 100);
    }
}
