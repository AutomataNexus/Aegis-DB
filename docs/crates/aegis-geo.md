---
layout: default
title: aegis-geo
parent: Crate Documentation
nav_order: 18
description: "Geospatial engine (grid index + Haversine)"
---

# aegis-geo

Geospatial engine — named collections of `{id, lat, lon, metadata}` features on a
uniform grid spatial index, with radius / bounding-box / nearest-k queries over
great-circle (Haversine) distance. The ninth data paradigm.

## Overview

`aegis-geo` buckets points into fixed-size lat/lon grid cells so spatial queries
only scan the cells overlapping the query region, then filters by exact Haversine
distance. Nearest-k is exact — the search radius expands geometrically until at
least `k` points are enclosed.

## Modules

| Module | Responsibility |
|--------|----------------|
| `types.rs` | `GeoFeature` / `GeoHit`, `GeoError`, `haversine_m`, coordinate validation |
| `grid.rs` | `GridIndex` — cell bucketing, `within_radius` / `within_bbox` / `nearest` |
| `engine.rs` | `GeoEngine` — named collections, upsert/get/delete, spatial queries, metadata filter, snapshot |

## Distance

Great-circle distance on a spherical Earth (mean radius 6 371 000 m):

```
a = sin²(Δφ/2) + cos φ₁ · cos φ₂ · sin²(Δλ/2)
d = 2 · R · asin(√a)
```

Each `GeoHit` carries `distance_m` from the query point (0 for bounding-box
queries, which have no reference point) plus the feature's metadata.

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET / POST | `/api/v1/geo/collections` | List / create (`{name}`) |
| GET / DELETE | `/api/v1/geo/collections/:name` | Stats / drop |
| POST | `/api/v1/geo/collections/:name/features` | Upsert `{id, lat, lon, metadata?}` |
| GET / DELETE | `/api/v1/geo/collections/:name/features/:id` | Get / delete |
| POST | `/api/v1/geo/collections/:name/radius` | Radius `{lat, lon, radius_m, filter?}` |
| POST | `/api/v1/geo/collections/:name/bbox` | Bbox `{min_lat, min_lon, max_lat, max_lon, filter?}` |
| POST | `/api/v1/geo/collections/:name/nearest` | Nearest-k `{lat, lon, k, filter?}` |

All endpoints require authentication.

## Persistence

A collection snapshot (features) is written to `geo.ncb` as a NexusCompress blob
frame on graceful shutdown and the grid index is rebuilt from it on startup.

## Tests

Workspace total includes Haversine accuracy, nearest-k vs brute force, radius +
bbox membership, metadata filter, CRUD with in-place moves, error paths, and
snapshot round-trip.
