//! A from-scratch HNSW (Hierarchical Navigable Small World) index for
//! approximate nearest-neighbor search.
//!
//! Implements the Malkov & Yashunin (2018) algorithm: a multi-layer
//! navigable-small-world graph with a level-decaying probability, greedy
//! descent through upper layers, and the diversity neighbor-selection heuristic
//! (their Algorithm 4) for high recall at low degree.
//!
//! Vectors are owned by the index (the engine normalizes them first for the
//! cosine metric). Deletes are soft (tombstoned): the node stays in the graph
//! for connectivity but is excluded from results.

use crate::types::Metric;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

/// Build/search parameters.
#[derive(Debug, Clone, Copy)]
pub struct HnswConfig {
    /// Max neighbors per node on layers > 0 (`M`).
    pub m: usize,
    /// Size of the dynamic candidate list during construction (`efConstruction`).
    pub ef_construction: usize,
    /// RNG seed for level assignment (reproducible builds).
    pub seed: u64,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            seed: 0x5EED_A19E_C701,
        }
    }
}

/// A search/insert candidate, ordered by distance so a `BinaryHeap` (max-heap)
/// yields the *farthest* first.
#[derive(Debug, Clone, Copy)]
struct Cand {
    dist: f32,
    node: u32,
}
impl PartialEq for Cand {
    fn eq(&self, o: &Self) -> bool {
        self.dist == o.dist && self.node == o.node
    }
}
impl Eq for Cand {}
impl Ord for Cand {
    fn cmp(&self, o: &Self) -> Ordering {
        self.dist
            .total_cmp(&o.dist)
            .then_with(|| self.node.cmp(&o.node))
    }
}
impl PartialOrd for Cand {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// An HNSW index over `dim`-dimensional vectors under a fixed [`Metric`].
pub struct HnswIndex {
    dim: usize,
    metric: Metric,
    vectors: Vec<Vec<f32>>,
    /// `links[node][layer]` = neighbor node ids of `node` on `layer`.
    links: Vec<Vec<Vec<u32>>>,
    deleted: Vec<bool>,
    entry_point: Option<u32>,
    max_layer: usize,
    m: usize,
    m_max: usize,
    m_max0: usize,
    ef_construction: usize,
    ml: f64,
    rng: StdRng,
    live: usize,
}

impl HnswIndex {
    pub fn new(dim: usize, metric: Metric, config: HnswConfig) -> Self {
        let m = config.m.max(2);
        Self {
            dim,
            metric,
            vectors: Vec::new(),
            links: Vec::new(),
            deleted: Vec::new(),
            entry_point: None,
            max_layer: 0,
            m,
            m_max: m,
            m_max0: m * 2,
            ef_construction: config.ef_construction.max(m),
            ml: 1.0 / (m as f64).ln(),
            rng: StdRng::seed_from_u64(config.seed),
            live: 0,
        }
    }

    pub fn dim(&self) -> usize {
        self.dim
    }
    /// Number of live (non-deleted) vectors.
    pub fn len(&self) -> usize {
        self.live
    }
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// The stored vector for a node (L2-normalized when the metric is cosine).
    #[inline]
    pub fn vector(&self, node: u32) -> &[f32] {
        &self.vectors[node as usize]
    }

    #[inline]
    fn dist(&self, node: u32, q: &[f32]) -> f32 {
        self.metric.distance(&self.vectors[node as usize], q)
    }

    fn random_level(&mut self) -> usize {
        let r: f64 = self.rng.gen::<f64>().max(1e-12);
        (-r.ln() * self.ml).floor() as usize
    }

    /// Insert a (pre-normalized for cosine) vector. Returns its node id.
    pub fn insert(&mut self, vector: Vec<f32>) -> u32 {
        let node = self.vectors.len() as u32;
        let level = self.random_level();
        self.vectors.push(vector);
        self.deleted.push(false);
        self.links.push(vec![Vec::new(); level + 1]);
        self.live += 1;

        let ep = match self.entry_point {
            None => {
                self.entry_point = Some(node);
                self.max_layer = level;
                return node;
            }
            Some(ep) => ep,
        };

        let q = self.vectors[node as usize].clone();
        let top = self.max_layer;
        let mut cur = ep;

        // Greedy descent through layers above `level` (ef = 1).
        let mut lc = top;
        while lc > level {
            let w = self.search_layer(&q, &[cur], 1, lc);
            if let Some(best) = w.first() {
                cur = best.node;
            }
            lc -= 1;
        }

        // Insert into every layer from min(top, level) down to 0.
        let start = top.min(level);
        let mut lc = start as isize;
        while lc >= 0 {
            let layer = lc as usize;
            let w = self.search_layer(&q, &[cur], self.ef_construction, layer);
            let m_max = if layer == 0 { self.m_max0 } else { self.m_max };
            let selected = self.select_neighbors(&w, self.m);

            self.links[node as usize][layer] = selected.clone();
            for &nb in &selected {
                self.links[nb as usize][layer].push(node);
                if self.links[nb as usize][layer].len() > m_max {
                    let nb_vec = self.vectors[nb as usize].clone();
                    let cands: Vec<Cand> = self.links[nb as usize][layer]
                        .iter()
                        .map(|&x| Cand {
                            dist: self.metric.distance(&nb_vec, &self.vectors[x as usize]),
                            node: x,
                        })
                        .collect();
                    let pruned = self.select_neighbors(&cands, m_max);
                    self.links[nb as usize][layer] = pruned;
                }
            }
            if let Some(best) = w.first() {
                cur = best.node;
            }
            lc -= 1;
        }

        if level > self.max_layer {
            self.max_layer = level;
            self.entry_point = Some(node);
        }
        node
    }

    /// SEARCH-LAYER: return up to `ef` closest nodes to `q` on `layer`, sorted
    /// nearest-first. Traverses through tombstoned nodes (for connectivity) but
    /// still returns them; callers filter deletes at the top level.
    fn search_layer(&self, q: &[f32], entry: &[u32], ef: usize, layer: usize) -> Vec<Cand> {
        let mut visited: HashSet<u32> = HashSet::with_capacity(ef * 4);
        let mut candidates: BinaryHeap<std::cmp::Reverse<Cand>> = BinaryHeap::new();
        let mut results: BinaryHeap<Cand> = BinaryHeap::new();

        for &ep in entry {
            let d = self.dist(ep, q);
            visited.insert(ep);
            candidates.push(std::cmp::Reverse(Cand { dist: d, node: ep }));
            results.push(Cand { dist: d, node: ep });
        }

        while let Some(std::cmp::Reverse(c)) = candidates.pop() {
            let farthest = results.peek().map(|x| x.dist).unwrap_or(f32::INFINITY);
            if c.dist > farthest {
                break;
            }
            if let Some(neighbors) = self.links[c.node as usize].get(layer) {
                for &e in neighbors {
                    if visited.insert(e) {
                        let d = self.dist(e, q);
                        let farthest = results.peek().map(|x| x.dist).unwrap_or(f32::INFINITY);
                        if d < farthest || results.len() < ef {
                            candidates.push(std::cmp::Reverse(Cand { dist: d, node: e }));
                            results.push(Cand { dist: d, node: e });
                            if results.len() > ef {
                                results.pop();
                            }
                        }
                    }
                }
            }
        }

        let mut out = results.into_vec();
        out.sort();
        out
    }

    /// Algorithm 4: pick up to `m` diverse neighbors — keep a candidate only if
    /// it is closer to the query than to any already-selected neighbor, then
    /// top up with the nearest remaining to preserve degree/connectivity.
    fn select_neighbors(&self, candidates: &[Cand], m: usize) -> Vec<u32> {
        let mut sorted = candidates.to_vec();
        sorted.sort();
        let mut result: Vec<u32> = Vec::with_capacity(m);
        for c in &sorted {
            if result.len() >= m {
                break;
            }
            let e_vec = &self.vectors[c.node as usize];
            let mut keep = true;
            for &r in &result {
                if self.metric.distance(e_vec, &self.vectors[r as usize]) < c.dist {
                    keep = false;
                    break;
                }
            }
            if keep {
                result.push(c.node);
            }
        }
        if result.len() < m {
            for c in &sorted {
                if result.len() >= m {
                    break;
                }
                if !result.contains(&c.node) {
                    result.push(c.node);
                }
            }
        }
        result
    }

    /// Soft-delete the node holding the given vector id mapping. No-op if out of
    /// range or already deleted.
    pub fn mark_deleted(&mut self, node: u32) {
        let i = node as usize;
        if i < self.deleted.len() && !self.deleted[i] {
            self.deleted[i] = true;
            self.live -= 1;
            if self.entry_point == Some(node) {
                // Re-seat the entry point on any live node (rare; keeps search valid).
                self.entry_point =
                    (0..self.vectors.len() as u32).find(|&n| !self.deleted[n as usize]);
                self.max_layer = self
                    .entry_point
                    .map(|n| self.links[n as usize].len() - 1)
                    .unwrap_or(0);
            }
        }
    }

    /// Top-`k` nearest live nodes to `q` (already normalized for cosine).
    /// Returns `(node_id, distance)` nearest-first.
    pub fn search(&self, q: &[f32], k: usize, ef: usize) -> Vec<(u32, f32)> {
        let ep = match self.entry_point {
            None => return Vec::new(),
            Some(ep) => ep,
        };
        let mut cur = ep;
        let mut lc = self.max_layer;
        while lc > 0 {
            let w = self.search_layer(q, &[cur], 1, lc);
            if let Some(best) = w.first() {
                cur = best.node;
            }
            lc -= 1;
        }
        let ef0 = ef.max(k).max(1);
        let w = self.search_layer(q, &[cur], ef0, 0);
        w.into_iter()
            .filter(|c| !self.deleted[c.node as usize])
            .take(k)
            .map(|c| (c.node, c.dist))
            .collect()
    }
}
