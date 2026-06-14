//! The vector engine: named collections of embeddings, each backed by an HNSW
//! index, with upsert / get / delete / KNN-search (+ metadata filter) and a
//! serializable snapshot for persistence.

use crate::hnsw::{HnswConfig, HnswIndex};
use crate::types::{normalize, Metric, SearchHit, VectorError, VectorRecord};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-collection configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CollectionConfig {
    pub dim: usize,
    pub metric: Metric,
}

/// Summary stats for a collection.
#[derive(Debug, Clone, Serialize)]
pub struct CollectionStats {
    pub name: String,
    pub dim: usize,
    pub metric: Metric,
    pub count: usize,
}

struct Collection {
    config: CollectionConfig,
    index: HnswIndex,
    id_to_node: HashMap<String, u32>,
    node_meta: Vec<serde_json::Value>,
    node_id: Vec<Option<String>>,
}

impl Collection {
    fn new(config: CollectionConfig, hnsw: HnswConfig) -> Self {
        Self {
            index: HnswIndex::new(config.dim, config.metric, hnsw),
            config,
            id_to_node: HashMap::new(),
            node_meta: Vec::new(),
            node_id: Vec::new(),
        }
    }

    fn prepared(&self, vector: &[f32]) -> Result<Vec<f32>, VectorError> {
        if vector.len() != self.config.dim {
            return Err(VectorError::DimensionMismatch {
                expected: self.config.dim,
                got: vector.len(),
            });
        }
        let mut v = vector.to_vec();
        if self.config.metric.normalizes() {
            normalize(&mut v);
        }
        Ok(v)
    }

    fn upsert(
        &mut self,
        id: String,
        vector: &[f32],
        metadata: serde_json::Value,
    ) -> Result<(), VectorError> {
        let prepared = self.prepared(vector)?;
        if let Some(&old) = self.id_to_node.get(&id) {
            self.index.mark_deleted(old);
            self.node_id[old as usize] = None;
            self.node_meta[old as usize] = serde_json::Value::Null;
        }
        let node = self.index.insert(prepared);
        debug_assert_eq!(node as usize, self.node_meta.len());
        self.node_meta.push(metadata);
        self.node_id.push(Some(id.clone()));
        self.id_to_node.insert(id, node);
        Ok(())
    }

    fn get(&self, id: &str) -> Option<VectorRecord> {
        let &node = self.id_to_node.get(id)?;
        Some(VectorRecord {
            id: id.to_string(),
            vector: self.index.vector(node).to_vec(),
            metadata: self.node_meta[node as usize].clone(),
        })
    }

    fn delete(&mut self, id: &str) -> bool {
        match self.id_to_node.remove(id) {
            Some(node) => {
                self.index.mark_deleted(node);
                self.node_id[node as usize] = None;
                self.node_meta[node as usize] = serde_json::Value::Null;
                true
            }
            None => false,
        }
    }

    fn search(
        &self,
        query: &[f32],
        k: usize,
        ef: Option<usize>,
        filter: &serde_json::Value,
    ) -> Result<Vec<SearchHit>, VectorError> {
        let prepared = self.prepared(query)?;
        let has_filter = filter.as_object().map(|m| !m.is_empty()).unwrap_or(false);
        // Over-fetch when a metadata filter is applied so post-filtering still
        // returns up to k results.
        let fetch = if has_filter {
            (k * 10).min(self.index.len()).max(k)
        } else {
            k
        };
        let ef = ef.unwrap_or_else(|| (k * 4).max(64));
        let raw = self.index.search(&prepared, fetch, ef);

        let metric = self.config.metric;
        let mut hits = Vec::with_capacity(k);
        for (node, dist) in raw {
            let meta = &self.node_meta[node as usize];
            if has_filter && !matches_filter(meta, filter) {
                continue;
            }
            let id = match &self.node_id[node as usize] {
                Some(id) => id.clone(),
                None => continue,
            };
            hits.push(SearchHit {
                id,
                score: score_for(metric, dist),
                distance: dist,
                metadata: meta.clone(),
            });
            if hits.len() >= k {
                break;
            }
        }
        Ok(hits)
    }

    fn records(&self) -> Vec<VectorRecord> {
        self.id_to_node
            .iter()
            .map(|(id, &node)| VectorRecord {
                id: id.clone(),
                vector: self.index.vector(node).to_vec(),
                metadata: self.node_meta[node as usize].clone(),
            })
            .collect()
    }
}

/// A similarity-style score (higher = more similar) derived from a metric
/// distance (lower = closer).
fn score_for(metric: Metric, dist: f32) -> f32 {
    match metric {
        Metric::Cosine => 1.0 - dist, // dist = 1 - cosine_sim
        Metric::L2 => -dist,          // dist = squared L2
        Metric::Dot => -dist,         // dist = -dot
    }
}

/// Exact-match metadata filter: every key in `filter` must equal the same key
/// in the document's metadata. An empty filter matches everything.
fn matches_filter(meta: &serde_json::Value, filter: &serde_json::Value) -> bool {
    match (meta.as_object(), filter.as_object()) {
        (Some(m), Some(f)) => f.iter().all(|(k, v)| m.get(k) == Some(v)),
        (_, Some(f)) => f.is_empty(),
        _ => true,
    }
}

// ============================================================================
// Engine
// ============================================================================

/// Multi-collection vector engine.
pub struct VectorEngine {
    collections: RwLock<HashMap<String, Collection>>,
    hnsw: HnswConfig,
}

impl Default for VectorEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorEngine {
    pub fn new() -> Self {
        Self {
            collections: RwLock::new(HashMap::new()),
            hnsw: HnswConfig::default(),
        }
    }

    pub fn with_hnsw_config(hnsw: HnswConfig) -> Self {
        Self {
            collections: RwLock::new(HashMap::new()),
            hnsw,
        }
    }

    pub fn create_collection(
        &self,
        name: impl Into<String>,
        dim: usize,
        metric: Metric,
    ) -> Result<(), VectorError> {
        if dim == 0 {
            return Err(VectorError::InvalidDimension);
        }
        let name = name.into();
        let mut cols = self.collections.write();
        if cols.contains_key(&name) {
            return Err(VectorError::CollectionExists(name));
        }
        cols.insert(
            name,
            Collection::new(CollectionConfig { dim, metric }, self.hnsw),
        );
        Ok(())
    }

    pub fn drop_collection(&self, name: &str) -> Result<(), VectorError> {
        self.collections
            .write()
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| VectorError::CollectionNotFound(name.to_string()))
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
            dim: c.config.dim,
            metric: c.config.metric,
            count: c.index.len(),
        })
    }

    pub fn upsert(
        &self,
        collection: &str,
        id: impl Into<String>,
        vector: &[f32],
        metadata: serde_json::Value,
    ) -> Result<(), VectorError> {
        let mut cols = self.collections.write();
        let c = cols
            .get_mut(collection)
            .ok_or_else(|| VectorError::CollectionNotFound(collection.to_string()))?;
        c.upsert(id.into(), vector, metadata)
    }

    pub fn upsert_many(
        &self,
        collection: &str,
        records: Vec<VectorRecord>,
    ) -> Result<usize, VectorError> {
        let mut cols = self.collections.write();
        let c = cols
            .get_mut(collection)
            .ok_or_else(|| VectorError::CollectionNotFound(collection.to_string()))?;
        let mut n = 0;
        for r in records {
            c.upsert(r.id, &r.vector, r.metadata)?;
            n += 1;
        }
        Ok(n)
    }

    pub fn get(&self, collection: &str, id: &str) -> Result<Option<VectorRecord>, VectorError> {
        let cols = self.collections.read();
        let c = cols
            .get(collection)
            .ok_or_else(|| VectorError::CollectionNotFound(collection.to_string()))?;
        Ok(c.get(id))
    }

    pub fn delete(&self, collection: &str, id: &str) -> Result<bool, VectorError> {
        let mut cols = self.collections.write();
        let c = cols
            .get_mut(collection)
            .ok_or_else(|| VectorError::CollectionNotFound(collection.to_string()))?;
        Ok(c.delete(id))
    }

    pub fn search(
        &self,
        collection: &str,
        query: &[f32],
        k: usize,
        ef: Option<usize>,
        filter: &serde_json::Value,
    ) -> Result<Vec<SearchHit>, VectorError> {
        let cols = self.collections.read();
        let c = cols
            .get(collection)
            .ok_or_else(|| VectorError::CollectionNotFound(collection.to_string()))?;
        c.search(query, k, ef, filter)
    }

    // ---- Persistence ----------------------------------------------------

    /// Serializable snapshot of every collection (config + live records). The
    /// HNSW graph is not serialized; it is rebuilt from records on load.
    pub fn snapshot(&self) -> EngineSnapshot {
        let cols = self.collections.read();
        EngineSnapshot {
            collections: cols
                .iter()
                .map(|(name, c)| CollectionSnapshot {
                    name: name.clone(),
                    dim: c.config.dim,
                    metric: c.config.metric,
                    records: c.records(),
                })
                .collect(),
        }
    }

    /// Rebuild the engine state from a snapshot (re-inserts all records,
    /// reconstructing each HNSW index).
    pub fn load_snapshot(&self, snap: EngineSnapshot) {
        let mut cols = self.collections.write();
        cols.clear();
        for cs in snap.collections {
            let mut c = Collection::new(
                CollectionConfig {
                    dim: cs.dim,
                    metric: cs.metric,
                },
                self.hnsw,
            );
            for r in cs.records {
                let _ = c.upsert(r.id, &r.vector, r.metadata);
            }
            cols.insert(cs.name, c);
        }
    }
}

/// On-disk snapshot of the whole engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub collections: Vec<CollectionSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSnapshot {
    pub name: String,
    pub dim: usize,
    pub metric: Metric,
    pub records: Vec<VectorRecord>,
}
