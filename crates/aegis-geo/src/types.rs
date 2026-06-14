//! Core types for the geospatial engine.

use serde::{Deserialize, Serialize};

/// Mean Earth radius in metres (spherical model).
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;
/// Metres per degree of latitude (≈ constant on a sphere).
pub const M_PER_DEG_LAT: f64 = 111_320.0;

/// A stored geo feature: a point `(lat, lon)` with an id and JSON metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoFeature {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// A query result: a feature with its distance from the query point (metres).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoHit {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    /// Great-circle distance from the query point, in metres. `0` for bbox
    /// queries (no reference point).
    pub distance_m: f64,
    pub metadata: serde_json::Value,
}

/// Errors returned by the geospatial engine.
#[derive(Debug, thiserror::Error)]
pub enum GeoError {
    #[error("collection '{0}' not found")]
    CollectionNotFound(String),
    #[error("collection '{0}' already exists")]
    CollectionExists(String),
    #[error("feature '{0}' not found")]
    FeatureNotFound(String),
    #[error("invalid coordinate: lat must be in [-90, 90], lon in [-180, 180]")]
    InvalidCoordinate,
}

/// Great-circle (Haversine) distance between two lat/lon points, in metres.
pub fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * a.sqrt().asin()
}

/// Validate a lat/lon pair.
pub fn valid_coord(lat: f64, lon: f64) -> bool {
    (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lon)
        && lat.is_finite()
        && lon.is_finite()
}
