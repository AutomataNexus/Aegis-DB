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

    // ---- Bucket-name validation --------------------------------------------

    #[test]
    fn bucket_name_rules() {
        assert!(valid_bucket_name("my-bucket.1"));
        assert!(valid_bucket_name("a"));
        assert!(!valid_bucket_name("")); // empty
        assert!(!valid_bucket_name("Upper")); // uppercase
        assert!(!valid_bucket_name("has space"));
        assert!(!valid_bucket_name("under_score")); // underscore not allowed
        assert!(!valid_bucket_name(&"x".repeat(64))); // too long (>63)
        assert!(valid_bucket_name(&"x".repeat(63)));
    }

    // ---- Payload edge cases -------------------------------------------------

    #[test]
    fn empty_and_binary_payloads() {
        let e = ObjectEngine::new();
        e.create_bucket("b").unwrap();
        // zero-byte object
        let m = e
            .put("b", "empty", vec![], None, serde_json::Value::Null)
            .unwrap();
        assert_eq!(m.size, 0);
        assert_eq!(m.etag, etag_of(&[]));
        let (d, _) = e.get("b", "empty").unwrap().unwrap();
        assert!(d.is_empty());
        // arbitrary non-UTF8 bytes survive intact
        let raw: Vec<u8> = (0u16..=255).map(|x| x as u8).collect();
        e.put(
            "b",
            "blob",
            raw.clone(),
            Some("application/octet-stream".into()),
            serde_json::Value::Null,
        )
        .unwrap();
        let (back, _) = e.get("b", "blob").unwrap().unwrap();
        assert_eq!(back, raw);
    }

    #[test]
    fn default_content_type_when_none() {
        let e = ObjectEngine::new();
        e.create_bucket("b").unwrap();
        let m = e
            .put("b", "k", b"x".to_vec(), None, serde_json::Value::Null)
            .unwrap();
        assert_eq!(m.content_type, DEFAULT_CONTENT_TYPE);
    }

    #[test]
    fn etag_is_content_addressed() {
        // Same bytes => same etag regardless of key/bucket; different bytes differ.
        let e = ObjectEngine::new();
        e.create_bucket("a").unwrap();
        e.create_bucket("b").unwrap();
        let m1 = e
            .put(
                "a",
                "k1",
                b"identical".to_vec(),
                None,
                serde_json::Value::Null,
            )
            .unwrap();
        let m2 = e
            .put(
                "b",
                "k2",
                b"identical".to_vec(),
                None,
                serde_json::Value::Null,
            )
            .unwrap();
        assert_eq!(m1.etag, m2.etag);
        let m3 = e
            .put(
                "a",
                "k3",
                b"different".to_vec(),
                None,
                serde_json::Value::Null,
            )
            .unwrap();
        assert_ne!(m1.etag, m3.etag);
    }

    // ---- Keys, isolation, listing ------------------------------------------

    #[test]
    fn keys_with_slashes_and_bucket_isolation() {
        let e = ObjectEngine::new();
        e.create_bucket("one").unwrap();
        e.create_bucket("two").unwrap();
        e.put(
            "one",
            "a/b/c/deep.txt",
            b"1".to_vec(),
            None,
            serde_json::Value::Null,
        )
        .unwrap();
        e.put(
            "two",
            "a/b/c/deep.txt",
            b"2".to_vec(),
            None,
            serde_json::Value::Null,
        )
        .unwrap();
        // identical keys in different buckets are independent
        assert_eq!(e.get("one", "a/b/c/deep.txt").unwrap().unwrap().0, b"1");
        assert_eq!(e.get("two", "a/b/c/deep.txt").unwrap().unwrap().0, b"2");
    }

    #[test]
    fn prefix_listing_variants() {
        let e = ObjectEngine::new();
        e.create_bucket("b").unwrap();
        for k in ["a/1", "a/2", "a/3", "b/1", "c"] {
            e.put("b", k, b"x".to_vec(), None, serde_json::Value::Null)
                .unwrap();
        }
        assert_eq!(e.list("b", "a/", None).unwrap().len(), 3);
        assert_eq!(e.list("b", "a/", Some(2)).unwrap().len(), 2); // limit
        assert_eq!(e.list("b", "", None).unwrap().len(), 5); // all
        assert!(e.list("b", "zzz", None).unwrap().is_empty()); // no match
                                                               // results are in lexical key order
        let keys: Vec<String> = e
            .list("b", "", None)
            .unwrap()
            .into_iter()
            .map(|m| m.key)
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn get_and_delete_missing() {
        let e = ObjectEngine::new();
        e.create_bucket("b").unwrap();
        assert!(e.get("b", "missing").unwrap().is_none());
        assert!(e.head("b", "missing").unwrap().is_none());
        assert!(!e.delete("b", "missing").unwrap());
    }

    #[test]
    fn bucket_lifecycle_and_stats() {
        let e = ObjectEngine::new();
        assert!(matches!(
            e.create_bucket("UP"),
            Err(ObjectError::InvalidBucketName(_))
        ));
        e.create_bucket("a").unwrap();
        e.create_bucket("b").unwrap();
        assert!(matches!(
            e.create_bucket("a"),
            Err(ObjectError::BucketExists(_))
        ));
        assert_eq!(e.list_buckets(), vec!["a", "b"]);
        e.put("a", "k", b"hello".to_vec(), None, serde_json::Value::Null)
            .unwrap();
        e.put("a", "k2", b"hi".to_vec(), None, serde_json::Value::Null)
            .unwrap();
        let s = e.bucket_stats("a").unwrap();
        assert_eq!(s.objects, 2);
        assert_eq!(s.bytes, 7); // 5 + 2
        e.drop_bucket("a").unwrap();
        assert!(matches!(
            e.drop_bucket("a"),
            Err(ObjectError::BucketNotFound(_))
        ));
        assert!(e.bucket_stats("a").is_none());
        // operations on a missing bucket error rather than panic
        assert!(matches!(
            e.get("a", "k"),
            Err(ObjectError::BucketNotFound(_))
        ));
        assert!(matches!(
            e.list("a", "", None),
            Err(ObjectError::BucketNotFound(_))
        ));
    }

    #[test]
    fn metadata_is_preserved() {
        let e = ObjectEngine::new();
        e.create_bucket("b").unwrap();
        let meta = serde_json::json!({"author": "andrew", "tags": ["a", "b"]});
        e.put(
            "b",
            "k",
            b"x".to_vec(),
            Some("text/plain".into()),
            meta.clone(),
        )
        .unwrap();
        assert_eq!(e.head("b", "k").unwrap().unwrap().metadata, meta);
    }
}
