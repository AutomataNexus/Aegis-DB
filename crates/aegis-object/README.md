<p align="center">
  <img src="https://img.shields.io/badge/crate-0.4.4-green.svg" alt="Version">
</p>

# aegis-object

Object / blob store for Aegis-DB — S3-style **buckets** of binary objects, each
with a content type, a content-addressed **ETag** (FNV-1a fingerprint of the
bytes), and JSON metadata. The eleventh data paradigm in the Aegis engine.

## Features

- **Buckets + objects** — named buckets (`[a-z0-9.-]`) holding arbitrary binary
  objects keyed by string (keys may contain `/`).
- **Content-addressed ETags** — each object's ETag is an FNV-1a fingerprint of
  its bytes, so it changes iff the content changes.
- **Content type + metadata** — per-object content type and arbitrary JSON
  metadata travel with the bytes.
- **Prefix listing** — objects are stored sorted, so listing by key prefix is a
  range scan; supports a result limit.
- **put / get / head / delete** — `head` returns metadata without the bytes.
- **Snapshot persistence** — the whole bucket set serializes to a snapshot the
  server stores and reloads on startup.

## Example

```rust
use aegis_object::ObjectEngine;

let store = ObjectEngine::new();
store.create_bucket("media")?;

let meta = store.put(
    "media", "img/logo.png",
    std::fs::read("logo.png").unwrap(),
    Some("image/png".into()),
    serde_json::json!({"alt": "logo"}),
)?;
println!("etag = {}", meta.etag);

let (bytes, _meta) = store.get("media", "img/logo.png")?.unwrap();
let listing = store.list("media", "img/", None)?;   // prefix scan
```

## HTTP API (via `aegis-server`)

| Method | Path | Description |
|--------|------|-------------|
| GET/POST | `/api/v1/objects/buckets` | List / create a bucket (`{name}`) |
| GET/DELETE | `/api/v1/objects/buckets/:bucket` | Stats (count + bytes) / drop |
| GET | `/api/v1/objects/buckets/:bucket/objects` | List object metadata (`?prefix=&limit=`) |
| PUT | `/api/v1/objects/buckets/:bucket/object/*key` | Store (raw body; `Content-Type` + optional `X-Aegis-Meta`) |
| GET | `/api/v1/objects/buckets/:bucket/object/*key` | Fetch bytes (or `?meta=1` for JSON metadata) |
| DELETE | `/api/v1/objects/buckets/:bucket/object/*key` | Delete |

`GET` returns the raw bytes with the stored `Content-Type` and a quoted `ETag`
header.

## Tests

Workspace total includes put/get round-trip + ETag stability, head-without-body,
overwrite (etag/size/content-type), sorted prefix listing with limit, delete +
stats, bucket-name validation, and snapshot round-trip.
