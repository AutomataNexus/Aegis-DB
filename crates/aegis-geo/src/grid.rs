//! A uniform lat/lon grid spatial index. Points are bucketed into fixed-size
//! cells so that radius and bounding-box queries only examine the cells that
//! overlap the query region, then exact Haversine distance filters the results.
//!
//! Nearest-k is exact: it expands a radius query until at least `k` points are
//! found — every point inside that radius is returned, so the true k nearest
//! are guaranteed to be among them.

use crate::types::{haversine_m, M_PER_DEG_LAT};
use std::collections::HashMap;

/// Grid index over point ids. Coordinates are stored parallel to node ids.
pub struct GridIndex {
    cell_deg: f64,
    cells: HashMap<(i32, i32), Vec<u32>>,
    coords: Vec<Option<(f64, f64)>>,
    live: usize,
}

impl GridIndex {
    /// `cell_deg` is the cell size in degrees (smaller = finer buckets).
    pub fn new(cell_deg: f64) -> Self {
        Self {
            cell_deg: cell_deg.max(0.001),
            cells: HashMap::new(),
            coords: Vec::new(),
            live: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.live
    }
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    #[inline]
    fn cell_of(&self, lat: f64, lon: f64) -> (i32, i32) {
        (
            (lat / self.cell_deg).floor() as i32,
            (lon / self.cell_deg).floor() as i32,
        )
    }

    pub fn coord(&self, id: u32) -> Option<(f64, f64)> {
        self.coords.get(id as usize).copied().flatten()
    }

    /// Insert a point, returning its node id.
    pub fn insert(&mut self, lat: f64, lon: f64) -> u32 {
        let id = self.coords.len() as u32;
        self.cells
            .entry(self.cell_of(lat, lon))
            .or_default()
            .push(id);
        self.coords.push(Some((lat, lon)));
        self.live += 1;
        id
    }

    /// Remove a point by node id.
    pub fn remove(&mut self, id: u32) {
        if let Some(Some((lat, lon))) = self.coords.get(id as usize).copied() {
            let cell = self.cell_of(lat, lon);
            if let Some(bucket) = self.cells.get_mut(&cell) {
                bucket.retain(|&x| x != id);
                if bucket.is_empty() {
                    self.cells.remove(&cell);
                }
            }
            self.coords[id as usize] = None;
            self.live -= 1;
        }
    }

    fn scan_cells<F: FnMut(u32, f64, f64)>(
        &self,
        min_lat: f64,
        min_lon: f64,
        max_lat: f64,
        max_lon: f64,
        mut visit: F,
    ) {
        let (c_lat0, c_lon0) = self.cell_of(min_lat, min_lon);
        let (c_lat1, c_lon1) = self.cell_of(max_lat, max_lon);
        for cl in c_lat0..=c_lat1 {
            for cn in c_lon0..=c_lon1 {
                if let Some(bucket) = self.cells.get(&(cl, cn)) {
                    for &id in bucket {
                        if let Some((lat, lon)) = self.coord(id) {
                            visit(id, lat, lon);
                        }
                    }
                }
            }
        }
    }

    /// All points within `radius_m` metres of `(lat, lon)`, as `(id, distance)`.
    pub fn within_radius(&self, lat: f64, lon: f64, radius_m: f64) -> Vec<(u32, f64)> {
        let dlat = radius_m / M_PER_DEG_LAT;
        let dlon = radius_m / (M_PER_DEG_LAT * lat.to_radians().cos().abs().max(1e-9));
        let mut out = Vec::new();
        self.scan_cells(
            lat - dlat,
            lon - dlon,
            lat + dlat,
            lon + dlon,
            |id, plat, plon| {
                let d = haversine_m(lat, lon, plat, plon);
                if d <= radius_m {
                    out.push((id, d));
                }
            },
        );
        out
    }

    /// All point ids inside the bounding box.
    pub fn within_bbox(&self, min_lat: f64, min_lon: f64, max_lat: f64, max_lon: f64) -> Vec<u32> {
        let mut out = Vec::new();
        self.scan_cells(min_lat, min_lon, max_lat, max_lon, |id, plat, plon| {
            if plat >= min_lat && plat <= max_lat && plon >= min_lon && plon <= max_lon {
                out.push(id);
            }
        });
        out
    }

    /// The `k` nearest points to `(lat, lon)`, nearest first, as `(id, distance)`.
    /// Exact: expands the search radius until at least `k` points are enclosed.
    pub fn nearest(&self, lat: f64, lon: f64, k: usize) -> Vec<(u32, f64)> {
        if k == 0 || self.live == 0 {
            return Vec::new();
        }
        // Start around one cell's worth of metres and grow geometrically.
        let mut radius = (self.cell_deg * M_PER_DEG_LAT).max(1000.0);
        // Cap so a query in an empty region terminates (half Earth circumference).
        let max_radius = 20_037_500.0;
        loop {
            let mut hits = self.within_radius(lat, lon, radius);
            if hits.len() >= k || radius >= max_radius {
                hits.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
                hits.truncate(k);
                return hits;
            }
            radius *= 2.0;
        }
    }
}
