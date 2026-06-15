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

    // ---- Antimeridian (±180° longitude wrap) --------------------------------

    fn pacific() -> GeoEngine {
        let e = GeoEngine::new();
        e.create_collection("pac").unwrap();
        // east/west sit ~11 km on either side of the date line; far is ~9000 km.
        e.upsert("pac", "east", 0.0, 179.9, serde_json::Value::Null)
            .unwrap();
        e.upsert("pac", "west", 0.0, -179.9, serde_json::Value::Null)
            .unwrap();
        e.upsert("pac", "far", 0.0, 100.0, serde_json::Value::Null)
            .unwrap();
        e
    }

    #[test]
    fn antimeridian_radius_finds_both_sides() {
        let e = pacific();
        let hits = e
            .within_radius("pac", 0.0, 180.0, 50_000.0, &serde_json::Value::Null)
            .unwrap();
        let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        assert!(ids.contains(&"east"), "missed east of the date line");
        assert!(ids.contains(&"west"), "missed west of the date line");
        assert!(!ids.contains(&"far"));
    }

    #[test]
    fn antimeridian_nearest_crosses_line() {
        let e = pacific();
        // Query just west of the line; both east & west are the two nearest.
        let hits = e
            .nearest("pac", 0.0, -179.95, 2, &serde_json::Value::Null)
            .unwrap();
        let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        assert!(
            ids.contains(&"west") && ids.contains(&"east"),
            "got {ids:?}"
        );
        assert_eq!(hits[0].id, "west"); // nearer side first
    }

    #[test]
    fn antimeridian_bbox_crossing() {
        let e = pacific();
        // min_lon > max_lon => crosses the antimeridian (170°E .. 170°W).
        let hits = e
            .within_bbox("pac", -10.0, 170.0, 10.0, -170.0, &serde_json::Value::Null)
            .unwrap();
        let mut ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["east", "west"]);
    }

    #[test]
    fn near_pole_query_finds_far_longitude_points() {
        // Near a pole, points at very different longitudes are close via
        // over-the-pole paths. The scan band must not miss them.
        let e = GeoEngine::new();
        e.create_collection("arctic").unwrap();
        // All near the north pole but spread across longitudes.
        e.upsert("arctic", "a", 89.5, 0.0, serde_json::Value::Null)
            .unwrap();
        e.upsert("arctic", "b", 89.5, 90.0, serde_json::Value::Null)
            .unwrap();
        e.upsert("arctic", "c", 89.5, 180.0, serde_json::Value::Null)
            .unwrap();
        e.upsert("arctic", "d", 89.5, -90.0, serde_json::Value::Null)
            .unwrap();
        e.upsert("arctic", "equator", 0.0, 0.0, serde_json::Value::Null)
            .unwrap();
        // Query at (89, -179): all four arctic points are within a few hundred km.
        let hits = e
            .within_radius("arctic", 89.0, -179.0, 400_000.0, &serde_json::Value::Null)
            .unwrap();
        let mut ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        ids.sort();
        assert_eq!(
            ids,
            vec!["a", "b", "c", "d"],
            "near-pole radius missed far-longitude points"
        );
        // nearest-4 returns exactly the four arctic points (not the equator one).
        let n = e
            .nearest("arctic", 89.0, -179.0, 4, &serde_json::Value::Null)
            .unwrap();
        assert!(n.iter().all(|h| h.id != "equator"));
        assert_eq!(n.len(), 4);
    }

    // ---- nearest with a sparse filter (must still return k matches) ----------

    #[test]
    fn nearest_with_sparse_filter_returns_k() {
        let e = GeoEngine::new();
        e.create_collection("c").unwrap();
        // 60 'red' points clustered at the origin, 3 'blue' points ~9000 km away.
        for i in 0..60 {
            e.upsert(
                "c",
                format!("r{i}"),
                0.001 * i as f64,
                0.0,
                serde_json::json!({"color":"red"}),
            )
            .unwrap();
        }
        for (i, lat) in [80.0, 81.0, 82.0].iter().enumerate() {
            e.upsert(
                "c",
                format!("b{i}"),
                *lat,
                0.0,
                serde_json::json!({"color":"blue"}),
            )
            .unwrap();
        }
        // All 60 red points are nearer than any blue, yet we must still get 3 blue.
        let hits = e
            .nearest("c", 0.0, 0.0, 3, &serde_json::json!({"color":"blue"}))
            .unwrap();
        assert_eq!(hits.len(), 3, "sparse filter under-returned");
        assert!(hits.iter().all(|h| h.metadata["color"] == "blue"));
        // Ordered nearest-first by latitude.
        assert_eq!(hits[0].id, "b0");
    }

    // ---- Boundary / degenerate inputs ---------------------------------------

    #[test]
    fn k_zero_and_k_over_size() {
        let e = seeded();
        assert!(e
            .nearest("cities", NYC.0, NYC.1, 0, &serde_json::Value::Null)
            .unwrap()
            .is_empty());
        let all = e
            .nearest("cities", NYC.0, NYC.1, 100, &serde_json::Value::Null)
            .unwrap();
        assert_eq!(all.len(), 4); // never more than the collection holds
    }

    #[test]
    fn radius_zero_matches_only_exact_point() {
        let e = seeded();
        let hits = e
            .within_radius("cities", NYC.0, NYC.1, 0.0, &serde_json::Value::Null)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "nyc");
        assert_eq!(hits[0].distance_m, 0.0);
    }

    #[test]
    fn queries_on_empty_collection_are_empty() {
        let e = GeoEngine::new();
        e.create_collection("empty").unwrap();
        let z = &serde_json::Value::Null;
        assert!(e.nearest("empty", 0.0, 0.0, 5, z).unwrap().is_empty());
        assert!(e
            .within_radius("empty", 0.0, 0.0, 1e6, z)
            .unwrap()
            .is_empty());
        assert!(e
            .within_bbox("empty", -1.0, -1.0, 1.0, 1.0, z)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn coordinate_boundaries_are_validated() {
        let e = GeoEngine::new();
        e.create_collection("c").unwrap();
        let z = serde_json::Value::Null;
        // Exact bounds are valid.
        assert!(e.upsert("c", "np", 90.0, 180.0, z.clone()).is_ok());
        assert!(e.upsert("c", "sp", -90.0, -180.0, z.clone()).is_ok());
        // Just outside is rejected.
        for (lat, lon) in [(90.1, 0.0), (-90.1, 0.0), (0.0, 180.1), (0.0, -180.1)] {
            assert!(matches!(
                e.upsert("c", "bad", lat, lon, z.clone()),
                Err(GeoError::InvalidCoordinate)
            ));
        }
        // NaN / infinity rejected.
        assert!(matches!(
            e.upsert("c", "nan", f64::NAN, 0.0, z.clone()),
            Err(GeoError::InvalidCoordinate)
        ));
    }

    #[test]
    fn collection_lifecycle() {
        let e = GeoEngine::new();
        e.create_collection("a").unwrap();
        assert!(matches!(
            e.create_collection("a"),
            Err(GeoError::CollectionExists(_))
        ));
        e.create_collection("b").unwrap();
        assert_eq!(e.list_collections(), vec!["a", "b"]);
        assert!(e.collection_exists("a"));
        e.drop_collection("a").unwrap();
        assert!(!e.collection_exists("a"));
        assert!(matches!(
            e.drop_collection("a"),
            Err(GeoError::CollectionNotFound(_))
        ));
        // Operations on a missing collection error rather than panic.
        let z = &serde_json::Value::Null;
        assert!(matches!(
            e.within_radius("a", 0.0, 0.0, 1.0, z),
            Err(GeoError::CollectionNotFound(_))
        ));
        assert!(matches!(
            e.within_bbox("a", 0.0, 0.0, 1.0, 1.0, z),
            Err(GeoError::CollectionNotFound(_))
        ));
        assert!(matches!(
            e.get("a", "x"),
            Err(GeoError::CollectionNotFound(_))
        ));
        assert!(matches!(
            e.delete("a", "x"),
            Err(GeoError::CollectionNotFound(_))
        ));
    }

    #[test]
    fn delete_removes_from_spatial_results() {
        let e = seeded();
        assert!(e.delete("cities", "chicago").unwrap());
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
        assert_eq!(ids, vec!["nyc"]); // chicago no longer within radius
        assert!(!e.delete("cities", "chicago").unwrap()); // already gone
    }

    #[test]
    fn haversine_symmetry_and_zero() {
        assert_eq!(haversine_m(NYC.0, NYC.1, NYC.0, NYC.1), 0.0);
        let ab = haversine_m(NYC.0, NYC.1, LONDON.0, LONDON.1);
        let ba = haversine_m(LONDON.0, LONDON.1, NYC.0, NYC.1);
        assert!((ab - ba).abs() < 1e-6);
        // NYC -> London ~5570 km.
        assert!((ab - 5_570_000.0).abs() < 60_000.0, "got {ab}");
    }

    // ---- Larger randomized nearest vs exact brute force ---------------------

    #[test]
    fn nearest_matches_bruteforce_randomized() {
        let e = GeoEngine::new();
        e.create_collection("pts").unwrap();
        // Deterministic LCG over the whole globe (incl. high latitudes + the line).
        let mut s: u64 = 0x1234_5678;
        let mut next = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (s >> 33) as f64 / (1u64 << 31) as f64 // [0,1)
        };
        let mut pts: Vec<(f64, f64)> = (0..400)
            .map(|_| (next() * 180.0 - 90.0, next() * 360.0 - 180.0))
            .collect();
        // Plus deliberate extremes: both poles and points on the ±180 meridian.
        pts.extend([
            (90.0, 0.0),
            (-90.0, 0.0),
            (89.7, 180.0),
            (89.7, -180.0),
            (88.0, 30.0),
            (0.0, 180.0),
            (0.0, -180.0),
        ]);
        for (i, (lat, lon)) in pts.iter().enumerate() {
            e.upsert("pts", format!("p{i}"), *lat, *lon, serde_json::Value::Null)
                .unwrap();
        }
        let z = &serde_json::Value::Null;
        // Queries include both poles and the date line.
        for q in [
            (0.0, 179.99),
            (89.0, -179.0),
            (-45.0, 0.0),
            (10.0, -179.5),
            (89.9, 17.0),
            (-89.5, -120.0),
            (0.0, 180.0),
        ] {
            let got: Vec<String> = e
                .nearest("pts", q.0, q.1, 5, z)
                .unwrap()
                .iter()
                .map(|h| h.id.clone())
                .collect();
            // exact ground truth
            let mut all: Vec<(String, f64)> = pts
                .iter()
                .enumerate()
                .map(|(i, (la, lo))| (format!("p{i}"), haversine_m(q.0, q.1, *la, *lo)))
                .collect();
            all.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
            let truth: Vec<String> = all.iter().take(5).map(|(id, _)| id.clone()).collect();
            assert_eq!(got, truth, "nearest mismatch near {q:?}");
        }
    }
}
