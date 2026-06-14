---
layout: default
title: aegis-object
parent: Crate Documentation
nav_order: 20
description: "Object / blob store (S3-style buckets, content-addressed ETags)"
---

# aegis-object

Object / blob store — S3-style buckets of binary objects, each with a content
type, a content-addressed ETag, and JSON metadata. The eleventh data paradigm.

## Overview

`aegis-object` stores arbitrary binary objects in named buckets, keyed by
string. Each object carries a content type, arbitrary JSON metadata, and an ETag
computed from its bytes (FNV-1a), so the ETag changes only when the content
does. Objects within a bucket are kept sorted by key, making prefix listing a
range scan.

## Modules

| Module | Responsibility |
|--------|----------------|
| `types.rs` | `ObjectMeta`, `ObjectError`, `etag_of` (FNV-1a), bucket-name validation |
| `engine.rs` | `ObjectEngine` — buckets, put/get/head/delete, prefix list, stats, snapshot |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET / POST | `/api/v1/objects/buckets` | List / create (`{name}`) |
| GET / DELETE | `/api/v1/objects/buckets/:bucket` | Stats / drop |
| GET | `/api/v1/objects/buckets/:bucket/objects` | List metadata (`?prefix=&limit=`) |
| PUT | `/api/v1/objects/buckets/:bucket/object/*key` | Store (raw body) |
| GET | `/api/v1/objects/buckets/:bucket/object/*key` | Fetch bytes (or `?meta=1`) |
| DELETE | `/api/v1/objects/buckets/:bucket/object/*key` | Delete |

The object key is a wildcard, so keys may contain `/`. On `PUT`, the request
body is the object content, the `Content-Type` header sets the stored content
type, and an optional `X-Aegis-Meta` header (JSON) sets custom metadata. On
`GET`, the raw bytes are returned with the stored `Content-Type` and a quoted
`ETag` header; `?meta=1` returns the JSON metadata instead. All endpoints require
authentication.

## Persistence

The full bucket set is written to `objects.ncb` as a NexusCompress blob frame on
graceful shutdown and reloaded on startup.

## Tests

Workspace total includes put/get round-trip + ETag stability, head, overwrite,
sorted prefix listing with limit, delete + stats, bucket-name validation, and
snapshot round-trip.
