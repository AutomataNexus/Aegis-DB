//! Aegis Vector — vector / KNN engine for the Aegis database.
//!
//! A multi-collection store of dense embeddings with approximate
//! nearest-neighbor search backed by a from-scratch HNSW index. Supports the
//! cosine, squared-L2, and inner-product (dot) metrics, per-record JSON
//! metadata with exact-match filtering, soft deletes, and a serializable
//! snapshot for persistence.

pub mod engine;
pub mod hnsw;
pub mod types;

pub use engine::{
    CollectionConfig, CollectionSnapshot, CollectionStats, EngineSnapshot, VectorEngine,
};
pub use hnsw::{HnswConfig, HnswIndex};
pub use types::{dot, normalize, Metric, SearchHit, VectorError, VectorRecord};

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    fn random_vectors(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
        let mut rng = StdRng::seed_from_u64(seed);
        (0..n)
            .map(|_| (0..dim).map(|_| rng.gen::<f32>() * 2.0 - 1.0).collect())
            .collect()
    }

    /// Exact KNN by linear scan (ground truth).
    fn brute_force(data: &[Vec<f32>], q: &[f32], k: usize, metric: Metric) -> Vec<usize> {
        let mut scored: Vec<(usize, f32)> = data
            .iter()
            .enumerate()
            .map(|(i, v)| (i, metric.distance(v, q)))
            .collect();
        scored.sort_by(|a, b| a.1.total_cmp(&b.1));
        scored.into_iter().take(k).map(|(i, _)| i).collect()
    }

    #[test]
    fn hnsw_recall_matches_bruteforce() {
        // Build an HNSW over 2000 vectors and check that its top-10 overlaps
        // heavily with the exact top-10 across many queries (recall@10).
        let dim = 32;
        let n = 2000;
        let k = 10;
        let metric = Metric::L2;
        let data = random_vectors(n, dim, 1);

        let mut idx = HnswIndex::new(dim, metric, HnswConfig::default());
        for v in &data {
            idx.insert(v.clone());
        }

        let queries = random_vectors(50, dim, 99);
        let mut hits = 0usize;
        let mut total = 0usize;
        for q in &queries {
            let truth: std::collections::HashSet<usize> =
                brute_force(&data, q, k, metric).into_iter().collect();
            let got = idx.search(q, k, 64);
            for (node, _) in got {
                if truth.contains(&(node as usize)) {
                    hits += 1;
                }
            }
            total += k;
        }
        let recall = hits as f32 / total as f32;
        assert!(recall > 0.90, "HNSW recall@{k} too low: {recall:.3}");
    }

    #[test]
    fn cosine_metric_normalizes_and_ranks() {
        let engine = VectorEngine::new();
        engine.create_collection("c", 3, Metric::Cosine).unwrap();
        // Same direction, different magnitude -> identical under cosine.
        engine
            .upsert("c", "a", &[1.0, 0.0, 0.0], serde_json::json!({"t": "x"}))
            .unwrap();
        engine
            .upsert("c", "b", &[0.0, 1.0, 0.0], serde_json::json!({"t": "y"}))
            .unwrap();
        engine
            .upsert("c", "c2", &[10.0, 0.0, 0.0], serde_json::json!({"t": "x"}))
            .unwrap();

        let hits = engine
            .search("c", &[2.0, 0.0, 0.0], 2, None, &serde_json::Value::Null)
            .unwrap();
        let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        // a and c2 both point along +x -> they are the two nearest, b is last.
        assert!(ids.contains(&"a") && ids.contains(&"c2"));
        assert!(!ids.contains(&"b"));
        // cosine score of an exactly-aligned vector ~ 1.0
        assert!(hits[0].score > 0.99);
    }

    #[test]
    fn upsert_get_delete_and_filter() {
        let engine = VectorEngine::new();
        engine.create_collection("docs", 4, Metric::L2).unwrap();
        for i in 0..20 {
            let cat = if i % 2 == 0 { "even" } else { "odd" };
            engine
                .upsert(
                    "docs",
                    format!("v{i}"),
                    &[i as f32, 0.0, 0.0, 0.0],
                    serde_json::json!({ "cat": cat }),
                )
                .unwrap();
        }
        assert_eq!(engine.collection_stats("docs").unwrap().count, 20);

        // get
        let r = engine.get("docs", "v3").unwrap().unwrap();
        assert_eq!(r.metadata["cat"], "odd");

        // filtered search returns only matching metadata
        let hits = engine
            .search(
                "docs",
                &[0.0; 4],
                5,
                None,
                &serde_json::json!({"cat": "even"}),
            )
            .unwrap();
        assert!(!hits.is_empty());
        assert!(hits.iter().all(|h| h.metadata["cat"] == "even"));

        // delete + re-stat
        assert!(engine.delete("docs", "v3").unwrap());
        assert!(engine.get("docs", "v3").unwrap().is_none());
        assert_eq!(engine.collection_stats("docs").unwrap().count, 19);

        // upsert overwrites in place (count unchanged)
        engine
            .upsert(
                "docs",
                "v4",
                &[99.0, 0.0, 0.0, 0.0],
                serde_json::json!({"cat": "even"}),
            )
            .unwrap();
        assert_eq!(engine.collection_stats("docs").unwrap().count, 19);
    }

    #[test]
    fn dimension_mismatch_and_missing_collection() {
        let engine = VectorEngine::new();
        engine.create_collection("c", 3, Metric::Dot).unwrap();
        assert!(matches!(
            engine.upsert("c", "a", &[1.0, 2.0], serde_json::Value::Null),
            Err(VectorError::DimensionMismatch {
                expected: 3,
                got: 2
            })
        ));
        assert!(matches!(
            engine.search("nope", &[1.0, 2.0, 3.0], 1, None, &serde_json::Value::Null),
            Err(VectorError::CollectionNotFound(_))
        ));
    }

    #[test]
    fn snapshot_roundtrip_rebuilds_index() {
        let engine = VectorEngine::new();
        engine.create_collection("c", 8, Metric::Cosine).unwrap();
        let data = random_vectors(200, 8, 7);
        for (i, v) in data.iter().enumerate() {
            engine
                .upsert("c", format!("id{i}"), v, serde_json::json!({"i": i}))
                .unwrap();
        }
        let snap = engine.snapshot();
        let json = serde_json::to_vec(&snap).unwrap();

        let restored = VectorEngine::new();
        restored.load_snapshot(serde_json::from_slice(&json).unwrap());
        assert_eq!(restored.collection_stats("c").unwrap().count, 200);
        // a search still returns the self vector as the top hit
        let hits = restored
            .search("c", &data[0], 1, None, &serde_json::Value::Null)
            .unwrap();
        assert_eq!(hits[0].id, "id0");
    }
}
