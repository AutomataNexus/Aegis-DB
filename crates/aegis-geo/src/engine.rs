//! The geospatial engine: named collections of geo points, each backed by a
//! grid index, with radius / bounding-box / nearest-k queries, metadata
//! filtering, and snapshot persistence.

use crate::grid::GridIndex;
use crate::types::{valid_coord, GeoError, GeoFeature, GeoHit};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Default grid cell size in degrees (~55 km of latitude).
const DEFAULT_CELL_DEG: f64 = 0.5;

#[derive(Debug, Clone, Serialize)]
pub struct CollectionStats {
    pub name: String,
    pub count: usize,
}

struct Collection {
    grid: GridIndex,
    id_to_node: HashMap<String, u32>,
    node_meta: Vec<serde_json::Value>,
    node_id: Vec<Option<String>>,
}

impl Collection {
    fn new(cell_deg: f64) -> Self {
        Self {
            grid: GridIndex::new(cell_deg),
            id_to_node: HashMap::new(),
            node_meta: Vec::new(),
            node_id: Vec::new(),
        }
    }

    fn upsert(
        &mut self,
        id: String,
        lat: f64,
        lon: f64,
        metadata: serde_json::Value,
    ) -> Result<(), GeoError> {
        if !valid_coord(lat, lon) {
            return Err(GeoError::InvalidCoordinate);
        }
        if let Some(&old) = self.id_to_node.get(&id) {
            self.grid.remove(old);
            self.node_id[old as usize] = None;
            self.node_meta[old as usize] = serde_json::Value::Null;
        }
        let node = self.grid.insert(lat, lon);
        debug_assert_eq!(node as usize, self.node_meta.len());
        self.node_meta.push(metadata);
        self.node_id.push(Some(id.clone()));
        self.id_to_node.insert(id, node);
        Ok(())
    }

    fn feature(&self, node: u32) -> Option<GeoFeature> {
        let (lat, lon) = self.grid.coord(node)?;
        let id = self.node_id[node as usize].clone()?;
        Some(GeoFeature {
            id,
            lat,
            lon,
            metadata: self.node_meta[node as usize].clone(),
        })
    }

    fn get(&self, id: &str) -> Option<GeoFeature> {
        let &node = self.id_to_node.get(id)?;
        self.feature(node)
    }

    fn delete(&mut self, id: &str) -> bool {
        match self.id_to_node.remove(id) {
            Some(node) => {
                self.grid.remove(node);
                self.node_id[node as usize] = None;
                self.node_meta[node as usize] = serde_json::Value::Null;
                true
            }
            None => false,
        }
    }

    fn hit(&self, node: u32, distance_m: f64) -> Option<GeoHit> {
        let (lat, lon) = self.grid.coord(node)?;
        let id = self.node_id[node as usize].clone()?;
        Some(GeoHit {
            id,
            lat,
            lon,
            distance_m,
            metadata: self.node_meta[node as usize].clone(),
        })
    }

    fn passes(&self, node: u32, has_filter: bool, filter: &serde_json::Value) -> bool {
        !has_filter || matches_filter(&self.node_meta[node as usize], filter)
    }
}

fn matches_filter(meta: &serde_json::Value, filter: &serde_json::Value) -> bool {
    match (meta.as_object(), filter.as_object()) {
        (Some(m), Some(f)) => f.iter().all(|(k, v)| m.get(k) == Some(v)),
        (_, Some(f)) => f.is_empty(),
        _ => true,
    }
}

fn has_filter(filter: &serde_json::Value) -> bool {
    filter.as_object().map(|m| !m.is_empty()).unwrap_or(false)
}

// ============================================================================
// Engine
// ============================================================================

/// Multi-collection geospatial engine.
pub struct GeoEngine {
    collections: RwLock<HashMap<String, Collection>>,
    cell_deg: f64,
}

impl Default for GeoEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GeoEngine {
    pub fn new() -> Self {
        Self {
            collections: RwLock::new(HashMap::new()),
            cell_deg: DEFAULT_CELL_DEG,
        }
    }

    pub fn create_collection(&self, name: impl Into<String>) -> Result<(), GeoError> {
        let name = name.into();
        let mut cols = self.collections.write();
        if cols.contains_key(&name) {
            return Err(GeoError::CollectionExists(name));
        }
        cols.insert(name, Collection::new(self.cell_deg));
        Ok(())
    }

    pub fn drop_collection(&self, name: &str) -> Result<(), GeoError> {
        self.collections
            .write()
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| GeoError::CollectionNotFound(name.to_string()))
    }

    pub fn list_collections(&self) -> Vec<String> {
        let mut v: Vec<String> = self.collections.read().keys().cloned().collect();
        v.sort();
        v
    }

    pub fn collection_exists(&self, name: &str) -> bool {
        self.collections.read().contains_key(name)
    }

    pub fn collection_stats(&self, name: &str) -> Option<CollectionStats> {
        let cols = self.collections.read();
        let c = cols.get(name)?;
        Some(CollectionStats {
            name: name.to_string(),
            count: c.grid.len(),
        })
    }

    pub fn upsert(
        &self,
        collection: &str,
        id: impl Into<String>,
        lat: f64,
        lon: f64,
        metadata: serde_json::Value,
    ) -> Result<(), GeoError> {
        let mut cols = self.collections.write();
        let c = cols
            .get_mut(collection)
            .ok_or_else(|| GeoError::CollectionNotFound(collection.to_string()))?;
        c.upsert(id.into(), lat, lon, metadata)
    }

    pub fn get(&self, collection: &str, id: &str) -> Result<Option<GeoFeature>, GeoError> {
        let cols = self.collections.read();
        let c = cols
            .get(collection)
            .ok_or_else(|| GeoError::CollectionNotFound(collection.to_string()))?;
        Ok(c.get(id))
    }

    pub fn delete(&self, collection: &str, id: &str) -> Result<bool, GeoError> {
        let mut cols = self.collections.write();
        let c = cols
            .get_mut(collection)
            .ok_or_else(|| GeoError::CollectionNotFound(collection.to_string()))?;
        Ok(c.delete(id))
    }

    /// Features within `radius_m` of `(lat, lon)`, nearest first.
    pub fn within_radius(
        &self,
        collection: &str,
        lat: f64,
        lon: f64,
        radius_m: f64,
        filter: &serde_json::Value,
    ) -> Result<Vec<GeoHit>, GeoError> {
        let cols = self.collections.read();
        let c = cols
            .get(collection)
            .ok_or_else(|| GeoError::CollectionNotFound(collection.to_string()))?;
        let hf = has_filter(filter);
        let mut hits: Vec<GeoHit> = c
            .grid
            .within_radius(lat, lon, radius_m)
            .into_iter()
            .filter(|(node, _)| c.passes(*node, hf, filter))
            .filter_map(|(node, d)| c.hit(node, d))
            .collect();
        hits.sort_by(|a, b| {
            a.distance_m
                .total_cmp(&b.distance_m)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(hits)
    }

    /// Features inside a bounding box.
    pub fn within_bbox(
        &self,
        collection: &str,
        min_lat: f64,
        min_lon: f64,
        max_lat: f64,
        max_lon: f64,
        filter: &serde_json::Value,
    ) -> Result<Vec<GeoHit>, GeoError> {
        let cols = self.collections.read();
        let c = cols
            .get(collection)
            .ok_or_else(|| GeoError::CollectionNotFound(collection.to_string()))?;
        let hf = has_filter(filter);
        let hits = c
            .grid
            .within_bbox(min_lat, min_lon, max_lat, max_lon)
            .into_iter()
            .filter(|node| c.passes(*node, hf, filter))
            .filter_map(|node| c.hit(node, 0.0))
            .collect();
        Ok(hits)
    }

    /// The `k` nearest features to `(lat, lon)`.
    pub fn nearest(
        &self,
        collection: &str,
        lat: f64,
        lon: f64,
        k: usize,
        filter: &serde_json::Value,
    ) -> Result<Vec<GeoHit>, GeoError> {
        let cols = self.collections.read();
        let c = cols
            .get(collection)
            .ok_or_else(|| GeoError::CollectionNotFound(collection.to_string()))?;
        let hf = has_filter(filter);
        // Without a filter, k nearest is exactly k. With a filter, matches may be
        // sparse and far away, so expand the candidate set geometrically until we
        // have k matches or the whole collection has been scanned — otherwise a
        // fixed over-fetch can silently return fewer than k.
        let live = c.grid.len();
        let mut fetch = if hf { (k * 4).max(k) } else { k };
        loop {
            let hits: Vec<GeoHit> = c
                .grid
                .nearest(lat, lon, fetch)
                .into_iter()
                .filter(|(node, _)| c.passes(*node, hf, filter))
                .filter_map(|(node, d)| c.hit(node, d))
                .take(k)
                .collect();
            if !hf || hits.len() >= k || fetch >= live {
                return Ok(hits);
            }
            fetch = (fetch * 4).min(live);
        }
    }

    // ---- Persistence ----------------------------------------------------

    pub fn snapshot(&self) -> EngineSnapshot {
        let cols = self.collections.read();
        EngineSnapshot {
            collections: cols
                .iter()
                .map(|(name, c)| CollectionSnapshot {
                    name: name.clone(),
                    features: c
                        .id_to_node
                        .values()
                        .filter_map(|&node| c.feature(node))
                        .collect(),
                })
                .collect(),
        }
    }

    pub fn load_snapshot(&self, snap: EngineSnapshot) {
        let mut cols = self.collections.write();
        cols.clear();
        for cs in snap.collections {
            let mut c = Collection::new(self.cell_deg);
            for f in cs.features {
                let _ = c.upsert(f.id, f.lat, f.lon, f.metadata);
            }
            cols.insert(cs.name, c);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub collections: Vec<CollectionSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSnapshot {
    pub name: String,
    pub features: Vec<GeoFeature>,
}
