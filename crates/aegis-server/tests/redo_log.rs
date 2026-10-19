//! Redo log recovery: KV, document and timeseries writes made after the last
//! checkpoint come back when the server restarts without a clean shutdown.

use aegis_document::DocumentId;
use aegis_server::{AppState, ServerConfig};
use aegis_timeseries::{DataPoint, Tags, TimeSeriesQuery};
use chrono::{Duration, Utc};

fn open(dir: &std::path::Path) -> AppState {
    AppState::new(ServerConfig {
        data_dir: Some(dir.to_string_lossy().into_owned()),
        ..ServerConfig::default()
    })
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("aegis-redo-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn point_count(state: &AppState, metric: &str) -> usize {
    let now = Utc::now();
    let q = TimeSeriesQuery::new(metric, now - Duration::hours(1), now + Duration::hours(1));
    state
        .timeseries_engine
        .query(&q)
        .series
        .iter()
        .map(|s| s.points.len())
        .sum()
}

fn segments(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir.join("oplog"))
        .map(|r| r.count())
        .unwrap_or(0)
}

fn write_doc(state: &AppState, collection: &str, id: &str, n: i64) {
    let mut doc = aegis_document::Document::with_id(id);
    doc.set("n", aegis_document::Value::Int(n));
    let _ = state.document_engine.create_collection(collection);
    let doc_id = DocumentId::new(id);
    if matches!(state.document_engine.get(collection, &doc_id), Ok(Some(_))) {
        state
            .document_engine
            .update(collection, &doc_id, doc)
            .expect("update");
    } else {
        state
            .document_engine
            .insert(collection, doc)
            .expect("insert");
    }
    state.persist_docs(collection, &[doc_id]);
}

#[tokio::test(flavor = "multi_thread")]
async fn writes_after_the_last_checkpoint_survive_a_kill() {
    let dir = temp_dir("kill");
    {
        let state = open(&dir);
        state
            .kv_store
            .set("org/1/license".into(), serde_json::json!("abc"), None);
        state
            .kv_store
            .set("gone".into(), serde_json::json!(1), None);
        state.kv_store.delete("gone");
        write_doc(&state, "controllers", "c1", 1);
        write_doc(&state, "controllers", "c1", 2);
        write_doc(&state, "controllers", "c2", 5);
        state
            .document_engine
            .delete("controllers", &DocumentId::new("c2"))
            .expect("delete");
        state.persist_docs("controllers", &[DocumentId::new("c2")]);
        let mut tags = Tags::new();
        tags.insert("controller_id", "c1");
        for i in 0..3 {
            state
                .write_points(
                    "temp",
                    tags.clone(),
                    vec![DataPoint::new(Utc::now() - Duration::seconds(i), 70.0)],
                )
                .expect("write");
        }
        // No save_to_disk and no shutdown: dropped like a killed process.
    }

    let state = open(&dir);
    assert_eq!(
        state.kv_store.get("org/1/license").map(|e| e.value),
        Some(serde_json::json!("abc"))
    );
    assert!(state.kv_store.get("gone").is_none());
    let c1 = state
        .document_engine
        .get("controllers", &DocumentId::new("c1"))
        .expect("get")
        .expect("c1 recovered");
    assert_eq!(c1.get("n"), Some(&aegis_document::Value::Int(2)));
    assert!(matches!(
        state
            .document_engine
            .get("controllers", &DocumentId::new("c2")),
        Ok(None)
    ));
    assert_eq!(point_count(&state, "temp"), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_checkpoint_retires_the_log_without_duplicating_points() {
    let dir = temp_dir("checkpoint");
    {
        let state = open(&dir);
        let mut tags = Tags::new();
        tags.insert("controller_id", "c1");
        state
            .write_points("temp", tags.clone(), vec![DataPoint::new(Utc::now(), 1.0)])
            .expect("write");
        state.save_to_disk().expect("checkpoint");
        // Only the fresh generation the cut opened is left.
        assert_eq!(segments(&dir), 1);
        state
            .write_points(
                "temp",
                tags,
                vec![DataPoint::new(Utc::now() - Duration::seconds(5), 2.0)],
            )
            .expect("write");
    }
    let state = open(&dir);
    // One point from the checkpoint, one replayed: neither doubled.
    assert_eq!(point_count(&state, "temp"), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dropped_collection_stays_dropped() {
    let dir = temp_dir("drop");
    {
        let state = open(&dir);
        write_doc(&state, "scratch", "d1", 1);
        state.save_to_disk().expect("checkpoint");
        state
            .document_engine
            .drop_collection("scratch")
            .expect("drop");
        state.persist_collection_drop("scratch");
    }
    let state = open(&dir);
    assert!(!state
        .document_engine
        .list_collections()
        .contains(&"scratch".to_string()));
    let _ = std::fs::remove_dir_all(&dir);
}
