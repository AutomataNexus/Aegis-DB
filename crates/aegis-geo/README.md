<p align="center">
  <img src="https://img.shields.io/badge/crate-0.5.0-green.svg" alt="Version">
</p>

# aegis-geo

Geospatial engine for Aegis-DB — named collections of `{id, lat, lon, metadata}`
features on a uniform grid spatial index, with radius / bounding-box / nearest-k
queries over great-circle (**Haversine**) distance. The ninth data paradigm in
the Aegis engine.

## Features

- **Grid spatial index** — points bucketed into fixed-size lat/lon cells, so
  radius and bbox queries only scan the cells overlapping the query region.
- **Haversine distance** — great-circle distance in metres (spherical Earth).
- **Radius query** — every feature within `radius_m` of a point, nearest first.
- **Bounding-box query** — every feature inside a lat/lon rectangle.
- **Nearest-k** — exact k-nearest; the search radius expands geometrically until
  ≥ k points are enclosed, so the true k nearest are guaranteed.
- **Metadata filtering** — exact-match JSON filter on results.
- **Snapshot persistence** — a serializable snapshot the server stores and
  rebuilds the index from on load.

## Example

```rust
use aegis_geo::GeoEngine;

let geo = GeoEngine::new();
geo.create_collection("cities")?;
geo.upsert("cities", "nyc", 40.7128, -74.0060, serde_json::json!({"country": "us"}))?;
geo.upsert("cities", "chicago", 41.8781, -87.6298, serde_json::json!({"country": "us"}))?;

// Everything within 2000 km of New York, nearest first.
let hits = geo.within_radius("cities", 40.7128, -74.0060, 2_000_000.0, &serde_json::Value::Null)?;
for hit in hits {
    println!("{}  {:.0} km", hit.id, hit.distance_m / 1000.0);
}

// The 3 nearest cities to a point near Washington DC.
let near = geo.nearest("cities", 39.0, -77.0, 3, &serde_json::Value::Null)?;
```

## HTTP API (via `aegis-server`)

| Method | Path | Description |
|--------|------|-------------|
| GET/POST | `/api/v1/geo/collections` | List / create a collection (`{name}`) |
| GET/DELETE | `/api/v1/geo/collections/:name` | Stats / drop |
| POST | `/api/v1/geo/collections/:name/features` | Upsert `{id, lat, lon, metadata?}` |
| GET/DELETE | `/api/v1/geo/collections/:name/features/:id` | Get / delete a feature |
| POST | `/api/v1/geo/collections/:name/radius` | Radius query `{lat, lon, radius_m, filter?}` |
| POST | `/api/v1/geo/collections/:name/bbox` | Bbox query `{min_lat, min_lon, max_lat, max_lon, filter?}` |
| POST | `/api/v1/geo/collections/:name/nearest` | Nearest-k `{lat, lon, k, filter?}` |

## Tests

Workspace total includes Haversine accuracy vs a known city-pair distance,
nearest-k vs brute force, radius + bbox membership, metadata filter, CRUD with
in-place moves, and snapshot round-trip.
