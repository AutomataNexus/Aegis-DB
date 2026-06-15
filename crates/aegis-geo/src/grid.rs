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
        // Canonicalize the longitude column into the window the scan visits, so a
        // point at lon == 180.0 lands in the same cell as -180.0 (the same
        // meridian) rather than an out-of-window column that scans never reach.
        let total = (360.0 / self.cell_deg).round() as i32;
        let lon_raw = (lon / self.cell_deg).floor() as i32;
        let lon_cell = if total > 0 {
            (lon_raw + total / 2).rem_euclid(total) - total / 2
        } else {
            lon_raw
        };
        ((lat / self.cell_deg).floor() as i32, lon_cell)
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
        // Longitude wraps at the antimeridian: each scanned column is mapped into
        // the canonical column window, so a query straddling ±180° still finds
        // points on the far side. Latitude does not wrap. The longitude span is
        // capped at a full globe so wrapped columns are never visited twice.
        let total_cols = (360.0 / self.cell_deg).round() as i32;
        let half = total_cols / 2;
        let c_lat0 = (min_lat / self.cell_deg).floor() as i32;
        let c_lat1 = (max_lat / self.cell_deg).floor() as i32;
        let c_lon0 = (min_lon / self.cell_deg).floor() as i32;
        let c_lon1 = (max_lon / self.cell_deg).floor() as i32;
        let span = (c_lon1 - c_lon0).clamp(0, total_cols.max(1) - 1);
        for cl in c_lat0..=c_lat1 {
            for k in 0..=span {
                let cn = if total_cols > 0 {
                    (c_lon0 + k + half).rem_euclid(total_cols) - half
                } else {
                    c_lon0 + k
                };
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
        // The longitude half-width must be wide enough that the rectangular
        // lat/lon scan band is a superset of the great-circle disk. Two effects:
        // (1) the band is widest (smallest cos) at its edge nearest the pole, so
        // size dlon from that extreme latitude, not the query latitude; (2) if the
        // band reaches a pole, over-the-pole paths make *every* longitude reachable,
        // so the band must span the whole globe.
        let reaches_pole = (lat + dlat) >= 90.0 || (lat - dlat) <= -90.0;
        let lat_extreme = (lat + dlat).abs().max((lat - dlat).abs()).min(90.0);
        let cos_extreme = lat_extreme.to_radians().cos();
        let dlon = if reaches_pole || cos_extreme < 1e-9 {
            180.0
        } else {
            (radius_m / (M_PER_DEG_LAT * cos_extreme)).min(180.0)
        };
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

    /// All point ids inside the bounding box. A box with `min_lon > max_lon` is
    /// treated as crossing the antimeridian and split into two halves.
    pub fn within_bbox(&self, min_lat: f64, min_lon: f64, max_lat: f64, max_lon: f64) -> Vec<u32> {
        if min_lon > max_lon {
            let mut out = self.within_bbox(min_lat, min_lon, max_lat, 180.0);
            out.extend(self.within_bbox(min_lat, -180.0, max_lat, max_lon));
            return out;
        }
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
