//! Aegis Geo — geospatial engine for the Aegis database.
//!
//! Named collections of geo points (`{id, lat, lon, metadata}`) backed by a
//! uniform grid spatial index, with radius / bounding-box / nearest-k queries
//! over great-circle (Haversine) distance, metadata filtering, and snapshot
//! persistence.

pub mod engine;
pub mod grid;
pub mod types;

pub use engine::{CollectionSnapshot, CollectionStats, EngineSnapshot, GeoEngine};
pub use grid::GridIndex;
pub use types::{haversine_m, valid_coord, GeoError, GeoFeature, GeoHit, EARTH_RADIUS_M};

#[cfg(test)]
mod tests {
    use super::*;

    // (lat, lon) of a few cities.
    const NYC: (f64, f64) = (40.7128, -74.0060);
    const CHICAGO: (f64, f64) = (41.8781, -87.6298);
    const LA: (f64, f64) = (34.0522, -118.2437);
    const LONDON: (f64, f64) = (51.5074, -0.1278);

    fn seeded() -> GeoEngine {
        let e = GeoEngine::new();
        e.create_collection("cities").unwrap();
        for (id, (lat, lon), country) in [
            ("nyc", NYC, "us"),
            ("chicago", CHICAGO, "us"),
            ("la", LA, "us"),
            ("london", LONDON, "uk"),
        ] {
            e.upsert(
                "cities",
                id,
                lat,
                lon,
                serde_json::json!({ "country": country }),
            )
            .unwrap();
        }
        e
    }

    #[test]
    fn haversine_known_distance() {
        // NYC -> LA is ~3,936 km.
        let d = haversine_m(NYC.0, NYC.1, LA.0, LA.1);
        assert!((d - 3_936_000.0).abs() < 30_000.0, "got {d} m");
    }

    #[test]
    fn within_radius_and_bbox() {
        let e = seeded();
        // 2000 km of NYC: NYC (0) + Chicago (~1145 km), not LA/London.
        let hits = e
            .within_radius(
                "cities",
                NYC.0,
                NYC.1,
                2_000_000.0,
                &serde_json::Value::Null,
            )
            .unwrap();
        let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["nyc", "chicago"]);
        assert!(hits[0].distance_m < hits[1].distance_m);

        // bbox roughly over the continental US excludes London.
        let bbox = e
            .within_bbox(
                "cities",
                25.0,
                -125.0,
                50.0,
                -65.0,
                &serde_json::Value::Null,
            )
            .unwrap();
        let mut ids: Vec<&str> = bbox.iter().map(|h| h.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["chicago", "la", "nyc"]);
    }

    #[test]
    fn nearest_matches_bruteforce() {
        let e = seeded();
        let q = (39.0, -77.0); // near Washington DC
        let hits = e
            .nearest("cities", q.0, q.1, 3, &serde_json::Value::Null)
            .unwrap();

        // brute-force ground truth
        let mut all = [
            ("nyc", NYC),
            ("chicago", CHICAGO),
            ("la", LA),
            ("london", LONDON),
        ]
        .map(|(id, c)| (id, haversine_m(q.0, q.1, c.0, c.1)));
        all.sort_by(|a, b| a.1.total_cmp(&b.1));
        let truth: Vec<&str> = all.iter().take(3).map(|(id, _)| *id).collect();

        let got: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(got, truth);
    }

    #[test]
    fn metadata_filter() {
        let e = seeded();
        let hits = e
            .nearest(
                "cities",
                NYC.0,
                NYC.1,
                5,
                &serde_json::json!({"country": "uk"}),
            )
            .unwrap();
        let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["london"]);
    }

    #[test]
    fn upsert_move_get_delete() {
        let e = seeded();
        assert_eq!(e.collection_stats("cities").unwrap().count, 4);
        // move 'nyc' to London's coordinates; it should now be near London.
        e.upsert(
            "cities",
            "nyc",
            LONDON.0,
            LONDON.1,
            serde_json::json!({"country": "moved"}),
        )
        .unwrap();
        assert_eq!(e.collection_stats("cities").unwrap().count, 4);
        let f = e.get("cities", "nyc").unwrap().unwrap();
        assert!((f.lat - LONDON.0).abs() < 1e-9);

        assert!(e.delete("cities", "la").unwrap());
        assert_eq!(e.collection_stats("cities").unwrap().count, 3);
        assert!(e.get("cities", "la").unwrap().is_none());
    }

    #[test]
    fn invalid_coord_and_missing_collection() {
        let e = GeoEngine::new();
        e.create_collection("c").unwrap();
        assert!(matches!(
            e.upsert("c", "x", 200.0, 0.0, serde_json::Value::Null),
            Err(GeoError::InvalidCoordinate)
        ));
        assert!(matches!(
            e.nearest("nope", 0.0, 0.0, 1, &serde_json::Value::Null),
            Err(GeoError::CollectionNotFound(_))
        ));
    }

    #[test]
    fn snapshot_roundtrip() {
        let e = seeded();
        let bytes = serde_json::to_vec(&e.snapshot()).unwrap();
        let restored = GeoEngine::new();
        restored.load_snapshot(serde_json::from_slice(&bytes).unwrap());
        assert_eq!(restored.collection_stats("cities").unwrap().count, 4);
        let hits = restored
            .nearest("cities", NYC.0, NYC.1, 1, &serde_json::Value::Null)
            .unwrap();
        assert_eq!(hits[0].id, "nyc");
    }
}
