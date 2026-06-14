//! The object / blob engine: S3-style buckets of binary objects with
//! content-addressed ETags, prefix listing, and snapshot persistence.

use crate::types::{etag_of, valid_bucket_name, ObjectError, ObjectMeta, DEFAULT_CONTENT_TYPE};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct BucketStats {
    pub name: String,
    pub objects: usize,
    pub bytes: usize,
}

/// A stored object: bytes plus its metadata.
#[derive(Clone, Serialize, Deserialize)]
struct StoredObject {
    data: Vec<u8>,
    content_type: String,
    etag: String,
    metadata: serde_json::Value,
}

impl StoredObject {
    fn meta(&self, key: &str) -> ObjectMeta {
        ObjectMeta {
            key: key.to_string(),
            size: self.data.len(),
            content_type: self.content_type.clone(),
            etag: self.etag.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

/// A bucket keeps its objects sorted by key so prefix listing is a range scan.
#[derive(Default, Clone, Serialize, Deserialize)]
struct Bucket {
    objects: BTreeMap<String, StoredObject>,
}

/// Multi-bucket object store.
pub struct ObjectEngine {
    buckets: RwLock<HashMap<String, Bucket>>,
}

impl Default for ObjectEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectEngine {
    pub fn new() -> Self {
        Self {
            buckets: RwLock::new(HashMap::new()),
        }
    }

    pub fn create_bucket(&self, name: impl Into<String>) -> Result<(), ObjectError> {
        let name = name.into();
        if !valid_bucket_name(&name) {
            return Err(ObjectError::InvalidBucketName(name));
        }
        let mut buckets = self.buckets.write();
        if buckets.contains_key(&name) {
            return Err(ObjectError::BucketExists(name));
        }
        buckets.insert(name, Bucket::default());
        Ok(())
    }

    pub fn drop_bucket(&self, name: &str) -> Result<(), ObjectError> {
        self.buckets
            .write()
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| ObjectError::BucketNotFound(name.to_string()))
    }

    pub fn list_buckets(&self) -> Vec<String> {
        let mut v: Vec<String> = self.buckets.read().keys().cloned().collect();
        v.sort();
        v
    }

    pub fn bucket_stats(&self, name: &str) -> Option<BucketStats> {
        let buckets = self.buckets.read();
        let b = buckets.get(name)?;
        Some(BucketStats {
            name: name.to_string(),
            objects: b.objects.len(),
            bytes: b.objects.values().map(|o| o.data.len()).sum(),
        })
    }

    /// Store (or replace) an object. Returns its metadata, including the ETag.
    pub fn put(
        &self,
        bucket: &str,
        key: impl Into<String>,
        data: Vec<u8>,
        content_type: Option<String>,
        metadata: serde_json::Value,
    ) -> Result<ObjectMeta, ObjectError> {
        let key = key.into();
        let mut buckets = self.buckets.write();
        let b = buckets
            .get_mut(bucket)
            .ok_or_else(|| ObjectError::BucketNotFound(bucket.to_string()))?;
        let obj = StoredObject {
            etag: etag_of(&data),
            content_type: content_type.unwrap_or_else(|| DEFAULT_CONTENT_TYPE.to_string()),
            metadata,
            data,
        };
        let meta = obj.meta(&key);
        b.objects.insert(key, obj);
        Ok(meta)
    }

    /// Fetch an object's bytes + metadata, or `None` if absent.
    pub fn get(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<Option<(Vec<u8>, ObjectMeta)>, ObjectError> {
        let buckets = self.buckets.read();
        let b = buckets
            .get(bucket)
            .ok_or_else(|| ObjectError::BucketNotFound(bucket.to_string()))?;
        Ok(b.objects.get(key).map(|o| (o.data.clone(), o.meta(key))))
    }

    /// Fetch only an object's metadata (HEAD), or `None` if absent.
    pub fn head(&self, bucket: &str, key: &str) -> Result<Option<ObjectMeta>, ObjectError> {
        let buckets = self.buckets.read();
        let b = buckets
            .get(bucket)
            .ok_or_else(|| ObjectError::BucketNotFound(bucket.to_string()))?;
        Ok(b.objects.get(key).map(|o| o.meta(key)))
    }

    /// Delete an object; returns whether it existed.
    pub fn delete(&self, bucket: &str, key: &str) -> Result<bool, ObjectError> {
        let mut buckets = self.buckets.write();
        let b = buckets
            .get_mut(bucket)
            .ok_or_else(|| ObjectError::BucketNotFound(bucket.to_string()))?;
        Ok(b.objects.remove(key).is_some())
    }

    /// List object metadata in a bucket, optionally filtered by key prefix,
    /// in lexical key order, capped at `limit`.
    pub fn list(
        &self,
        bucket: &str,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Vec<ObjectMeta>, ObjectError> {
        let buckets = self.buckets.read();
        let b = buckets
            .get(bucket)
            .ok_or_else(|| ObjectError::BucketNotFound(bucket.to_string()))?;
        let cap = limit.unwrap_or(usize::MAX);
        // BTreeMap range from the prefix; stop as soon as a key stops matching.
        let out = b
            .objects
            .range(prefix.to_string()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .take(cap)
            .map(|(k, o)| o.meta(k))
            .collect();
        Ok(out)
    }

    // ---- Persistence --------------------------------------------------------

    pub fn snapshot(&self) -> EngineSnapshot {
        EngineSnapshot {
            buckets: self.buckets.read().clone(),
        }
    }

    pub fn load_snapshot(&self, snap: EngineSnapshot) {
        *self.buckets.write() = snap.buckets;
    }
}

#[derive(Serialize, Deserialize)]
pub struct EngineSnapshot {
    buckets: HashMap<String, Bucket>,
}
