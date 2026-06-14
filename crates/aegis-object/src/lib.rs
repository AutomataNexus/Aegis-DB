//! Aegis Object — object / blob store for the Aegis database.
//!
//! S3-style **buckets** of binary objects, each with a content type,
//! content-addressed **ETag** (FNV-1a fingerprint of the bytes), and JSON
//! metadata. Supports put / get / head / delete and lexical prefix listing,
//! with snapshot persistence.

pub mod engine;
pub mod types;

pub use engine::{BucketStats, EngineSnapshot, ObjectEngine};
pub use types::{etag_of, valid_bucket_name, ObjectError, ObjectMeta, DEFAULT_CONTENT_TYPE};

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> ObjectEngine {
        let e = ObjectEngine::new();
        e.create_bucket("media").unwrap();
        e.put(
            "media",
            "a.txt",
            b"hello".to_vec(),
            Some("text/plain".into()),
            serde_json::json!({"k": 1}),
        )
        .unwrap();
        e.put(
            "media",
            "img/1.png",
            b"\x89PNG\x00".to_vec(),
            Some("image/png".into()),
            serde_json::Value::Null,
        )
        .unwrap();
        e.put(
            "media",
            "img/2.png",
            b"\x89PNG\x01".to_vec(),
            Some("image/png".into()),
            serde_json::Value::Null,
        )
        .unwrap();
        e
    }

    #[test]
    fn put_get_roundtrip_and_etag() {
        let e = seeded();
        let (data, meta) = e.get("media", "a.txt").unwrap().unwrap();
        assert_eq!(data, b"hello");
        assert_eq!(meta.content_type, "text/plain");
        assert_eq!(meta.size, 5);
        assert_eq!(meta.etag, etag_of(b"hello"));
        assert_eq!(meta.metadata, serde_json::json!({"k": 1}));
        // Different content => different etag; identical content => identical.
        assert_ne!(etag_of(b"hello"), etag_of(b"world"));
        assert_eq!(etag_of(b"hello"), etag_of(b"hello"));
    }

    #[test]
    fn head_without_body() {
        let e = seeded();
        let meta = e.head("media", "img/1.png").unwrap().unwrap();
        assert_eq!(meta.content_type, "image/png");
        assert_eq!(meta.size, 5);
        assert!(e.head("media", "nope").unwrap().is_none());
    }

    #[test]
    fn overwrite_updates_etag_and_size() {
        let e = seeded();
        let first = e.head("media", "a.txt").unwrap().unwrap().etag;
        e.put(
            "media",
            "a.txt",
            b"hello world".to_vec(),
            None,
            serde_json::Value::Null,
        )
        .unwrap();
        let meta = e.head("media", "a.txt").unwrap().unwrap();
        assert_eq!(meta.size, 11);
        assert_ne!(meta.etag, first);
        // No content type supplied on overwrite => default.
        assert_eq!(meta.content_type, DEFAULT_CONTENT_TYPE);
        assert_eq!(e.bucket_stats("media").unwrap().objects, 3);
    }

    #[test]
    fn prefix_listing_is_sorted() {
        let e = seeded();
        let imgs = e.list("media", "img/", None).unwrap();
        let keys: Vec<&str> = imgs.iter().map(|m| m.key.as_str()).collect();
        assert_eq!(keys, vec!["img/1.png", "img/2.png"]);

        let all = e.list("media", "", None).unwrap();
        assert_eq!(all.len(), 3);
        let limited = e.list("media", "", Some(1)).unwrap();
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].key, "a.txt");
    }

    #[test]
    fn delete_and_stats() {
        let e = seeded();
        let stats = e.bucket_stats("media").unwrap();
        assert_eq!(stats.objects, 3);
        assert_eq!(stats.bytes, 5 + 5 + 5);
        assert!(e.delete("media", "a.txt").unwrap());
        assert!(!e.delete("media", "a.txt").unwrap());
        assert_eq!(e.bucket_stats("media").unwrap().objects, 2);
    }

    #[test]
    fn bucket_validation_and_errors() {
        let e = ObjectEngine::new();
        assert!(matches!(
            e.create_bucket("BadName"),
            Err(ObjectError::InvalidBucketName(_))
        ));
        e.create_bucket("ok").unwrap();
        assert!(matches!(
            e.create_bucket("ok"),
            Err(ObjectError::BucketExists(_))
        ));
        assert!(matches!(
            e.put("nope", "k", vec![], None, serde_json::Value::Null),
            Err(ObjectError::BucketNotFound(_))
        ));
    }

    #[test]
    fn snapshot_roundtrip() {
        let e = seeded();
        let bytes = serde_json::to_vec(&e.snapshot()).unwrap();
        let restored = ObjectEngine::new();
        restored.load_snapshot(serde_json::from_slice(&bytes).unwrap());
        let (data, meta) = restored.get("media", "img/2.png").unwrap().unwrap();
        assert_eq!(data, b"\x89PNG\x01");
        assert_eq!(meta.etag, etag_of(b"\x89PNG\x01"));
        assert_eq!(restored.bucket_stats("media").unwrap().objects, 3);
    }
}
