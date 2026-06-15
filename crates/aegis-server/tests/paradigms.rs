//! End-to-end integration test for the five newest data paradigms — geospatial,
//! columnar/OLAP, object/blob, wide-column, and ledger.
//!
//! It boots the real `aegis-server` binary on an ephemeral port with a temp data
//! directory, authenticates as an admin, and drives every paradigm over HTTP:
//! happy paths, error status codes, auth enforcement, concurrent writes, and
//! data survival across a graceful restart. Nothing is mocked.

use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const USER: &str = "admin";
const PASS: &str = "Test1234!secure";

/// A spawned server process, killed on drop.
struct Server {
    child: Child,
    base: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn spawn(dir: &Path, port: u16) -> Child {
    Command::new(env!("CARGO_BIN_EXE_aegis-server"))
        .args([
            "--port",
            &port.to_string(),
            "--data-dir",
            dir.to_str().unwrap(),
        ])
        .env("AEGIS_ADMIN_USERNAME", USER)
        .env("AEGIS_ADMIN_PASSWORD", PASS)
        .env("AEGIS_RATE_LIMIT_PER_MINUTE", "1000000")
        .env("AEGIS_LOGIN_RATE_LIMIT_PER_MINUTE", "100000")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn aegis-server")
}

fn boot(dir: &Path, port: u16) -> Server {
    // Wrap the child in the guard up front so even the timeout panic path reaps it.
    let server = Server {
        child: spawn(dir, port),
        base: format!("http://127.0.0.1:{port}"),
    };
    let client = Client::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if client
            .get(format!("{}/health", server.base))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
        {
            return server;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("server did not become healthy on {}", server.base);
}

/// Gracefully stop the server (SIGTERM) so it flushes snapshots, then wait for
/// it to exit before returning. The `Server`'s `Drop` reaps the child.
fn graceful_stop(mut s: Server) {
    let pid = s.child.id();
    let _ = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if matches!(s.child.try_wait(), Ok(Some(_))) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // `s` drops here: Drop kills (no-op if already exited) and waits, reaping it.
}

fn login(base: &str) -> String {
    let client = Client::new();
    let resp: Value = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&json!({ "username": USER, "password": PASS }))
        .send()
        .unwrap()
        .json()
        .unwrap();
    resp["token"]
        .as_str()
        .expect("login returned no token")
        .to_string()
}

/// Tiny authed HTTP helper returning (status, body-json).
struct Api {
    client: Client,
    base: String,
    token: String,
}

impl Api {
    fn new(base: &str) -> Self {
        Api {
            client: Client::new(),
            base: base.to_string(),
            token: login(base),
        }
    }
    fn req(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut r = self
            .client
            .request(method, format!("{}{}", self.base, path))
            .bearer_auth(&self.token);
        if let Some(b) = body {
            r = r.json(&b);
        }
        let resp = r.send().unwrap();
        let status = resp.status();
        let json = resp.json().unwrap_or(Value::Null);
        (status, json)
    }
    fn get(&self, p: &str) -> (StatusCode, Value) {
        self.req(reqwest::Method::GET, p, None)
    }
    fn post(&self, p: &str, b: Value) -> (StatusCode, Value) {
        self.req(reqwest::Method::POST, p, Some(b))
    }
    fn put(&self, p: &str, b: Value) -> (StatusCode, Value) {
        self.req(reqwest::Method::PUT, p, Some(b))
    }
    fn delete(&self, p: &str) -> (StatusCode, Value) {
        self.req(reqwest::Method::DELETE, p, None)
    }
}

#[test]
fn all_new_paradigms_operational_over_http() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let server = boot(dir.path(), port);
    let api = Api::new(&server.base);

    // ---- Auth is enforced --------------------------------------------------
    let unauth = Client::new()
        .get(format!("{}/api/v1/geo/collections", server.base))
        .send()
        .unwrap();
    assert_eq!(
        unauth.status(),
        StatusCode::UNAUTHORIZED,
        "unauthenticated access must be 401"
    );

    geo(&api);
    columnar(&api);
    object(&server.base, &api.token);
    widecolumn(&api);
    ledger(&api);
    auto_create(&api);

    // ---- Persistence: graceful restart on the same data dir ----------------
    graceful_stop(server);
    let port2 = free_port();
    let server2 = boot(dir.path(), port2);
    let api2 = Api::new(&server2.base);
    verify_persisted(&api2);
}

fn geo(api: &Api) {
    assert_eq!(
        api.post("/api/v1/geo/collections", json!({"name":"cities"}))
            .0,
        StatusCode::CREATED
    );
    for (id, lat, lon, c) in [
        ("nyc", 40.7128, -74.0060, "us"),
        ("chicago", 41.8781, -87.6298, "us"),
        ("la", 34.0522, -118.2437, "us"),
        ("london", 51.5074, -0.1278, "uk"),
    ] {
        let (s, _) = api.post(
            "/api/v1/geo/collections/cities/features",
            json!({"id": id, "lat": lat, "lon": lon, "metadata": {"country": c}}),
        );
        assert_eq!(s, StatusCode::OK);
    }
    // nearest, radius, bbox, get, list, stats
    let (s, v) = api.post(
        "/api/v1/geo/collections/cities/nearest",
        json!({"lat":39.0,"lon":-77.0,"k":2}),
    );
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["hits"][0]["id"], "nyc");
    let (_, v) = api.post(
        "/api/v1/geo/collections/cities/radius",
        json!({"lat":40.7,"lon":-74.0,"radius_m":2_000_000}),
    );
    assert_eq!(v["count"], 2);
    let (_, v) = api.post(
        "/api/v1/geo/collections/cities/bbox",
        json!({"min_lat":25,"min_lon":-125,"max_lat":50,"max_lon":-65}),
    );
    assert_eq!(v["count"], 3); // continental US, excludes london
                               // metadata filter
    let (_, v) = api.post(
        "/api/v1/geo/collections/cities/nearest",
        json!({"lat":40.7,"lon":-74.0,"k":5,"filter":{"country":"uk"}}),
    );
    assert_eq!(v["count"], 1);
    assert_eq!(v["hits"][0]["id"], "london");
    assert_eq!(
        api.get("/api/v1/geo/collections/cities/features/nyc").0,
        StatusCode::OK
    );
    assert_eq!(api.get("/api/v1/geo/collections/cities").1["count"], 4);
    assert!(api.get("/api/v1/geo/collections").1["collections"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c == "cities"));
    assert_eq!(
        api.delete("/api/v1/geo/collections/cities/features/la").0,
        StatusCode::OK
    );
    assert_eq!(api.get("/api/v1/geo/collections/cities").1["count"], 3);
    // error: query a missing collection
    assert_eq!(
        api.post(
            "/api/v1/geo/collections/nope/nearest",
            json!({"lat":0,"lon":0,"k":1})
        )
        .0,
        StatusCode::NOT_FOUND
    );
}

fn columnar(api: &Api) {
    assert_eq!(
        api.post("/api/v1/columnar/tables", json!({"name":"sales","columns":[{"name":"region","type":"text"},{"name":"amount","type":"float"}]})).0,
        StatusCode::CREATED
    );
    let (s, v) = api.post(
        "/api/v1/columnar/tables/sales/rows",
        json!({"rows":[{"region":"east","amount":100},{"region":"west","amount":200},{"region":"east","amount":75}]}),
    );
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["inserted"], 3);
    let (_, v) = api.post(
        "/api/v1/columnar/tables/sales/aggregate",
        json!({"group_by":["region"],"aggregates":[{"func":"sum","column":"amount"},{"func":"count","column":"*"}]}),
    );
    let groups = v["groups"].as_array().unwrap();
    let east = groups.iter().find(|g| g["keys"][0] == "east").unwrap();
    assert_eq!(east["values"][0], 175.0);
    assert_eq!(east["values"][1], 2);
    // scan, distinct, stats
    assert_eq!(
        api.post(
            "/api/v1/columnar/tables/sales/scan",
            json!({"filter":[{"column":"region","op":"eq","value":"west"}]})
        )
        .1["count"],
        1
    );
    assert_eq!(
        api.get("/api/v1/columnar/tables/sales/distinct/region").1["count"],
        2
    );
    assert_eq!(api.get("/api/v1/columnar/tables/sales").1["rows"], 3);
    // errors: duplicate table => 409, type mismatch => 400
    assert_eq!(
        api.post(
            "/api/v1/columnar/tables",
            json!({"name":"sales","columns":[{"name":"x","type":"int"}]})
        )
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        api.post(
            "/api/v1/columnar/tables/sales/rows",
            json!({"rows":[{"amount":"notanumber"}]})
        )
        .0,
        StatusCode::BAD_REQUEST
    );
}

fn object(base: &str, token: &str) {
    let c = Client::new();
    let url = |p: &str| format!("{base}{p}");
    // create bucket
    assert_eq!(
        c.post(url("/api/v1/objects/buckets"))
            .bearer_auth(token)
            .json(&json!({"name":"media"}))
            .send()
            .unwrap()
            .status(),
        StatusCode::CREATED
    );
    // PUT raw bytes
    let put = c
        .put(url("/api/v1/objects/buckets/media/object/notes/a.txt"))
        .bearer_auth(token)
        .header("Content-Type", "text/plain")
        .header("X-Aegis-Meta", r#"{"author":"andrew"}"#)
        .body("hello aegis blob")
        .send()
        .unwrap();
    assert_eq!(put.status(), StatusCode::OK);
    // GET raw bytes + headers
    let got = c
        .get(url("/api/v1/objects/buckets/media/object/notes/a.txt"))
        .bearer_auth(token)
        .send()
        .unwrap();
    assert_eq!(got.status(), StatusCode::OK);
    assert_eq!(got.headers().get("content-type").unwrap(), "text/plain");
    assert!(got.headers().contains_key("etag"));
    assert_eq!(got.text().unwrap(), "hello aegis blob");
    // meta carries the custom metadata
    let meta: Value = c
        .get(url(
            "/api/v1/objects/buckets/media/object/notes/a.txt?meta=1",
        ))
        .bearer_auth(token)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(meta["size"], 16);
    assert_eq!(meta["metadata"]["author"], "andrew");
    // list + stats
    let list: Value = c
        .get(url("/api/v1/objects/buckets/media/objects?prefix=notes/"))
        .bearer_auth(token)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(list["count"], 1);
    let stats: Value = c
        .get(url("/api/v1/objects/buckets/media"))
        .bearer_auth(token)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(stats["objects"], 1);
    assert_eq!(stats["bytes"], 16);
    // missing object => 404; invalid bucket name => 400
    assert_eq!(
        c.get(url("/api/v1/objects/buckets/media/object/ghost"))
            .bearer_auth(token)
            .send()
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        c.post(url("/api/v1/objects/buckets"))
            .bearer_auth(token)
            .json(&json!({"name":"BAD NAME"}))
            .send()
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
}

fn widecolumn(api: &Api) {
    assert_eq!(
        api.post("/api/v1/widecolumn/tables", json!({"name":"users"}))
            .0,
        StatusCode::CREATED
    );
    api.put(
        "/api/v1/widecolumn/tables/users/rows/user:1",
        json!({"columns":{"name":"Alice","age":30}}),
    );
    api.put(
        "/api/v1/widecolumn/tables/users/rows/user:1",
        json!({"columns":{"city":"NYC"}}),
    ); // merge
    api.put(
        "/api/v1/widecolumn/tables/users/rows/user:2",
        json!({"columns":{"name":"Bob"}}),
    );
    let (_, v) = api.get("/api/v1/widecolumn/tables/users/rows/user:1");
    assert_eq!(v["columns"]["name"], "Alice");
    assert_eq!(v["columns"]["city"], "NYC"); // merged, not replaced
                                             // explicit-timestamp last-write-wins: older write loses
    api.put(
        "/api/v1/widecolumn/tables/users/rows/r",
        json!({"columns":{"v":"first"},"timestamp":100}),
    );
    api.put(
        "/api/v1/widecolumn/tables/users/rows/r",
        json!({"columns":{"v":"stale"},"timestamp":50}),
    );
    assert_eq!(
        api.get("/api/v1/widecolumn/tables/users/rows/r?columns=v")
            .1["columns"]["v"],
        "first"
    );
    // prefix scan ordered
    let (_, v) = api.post(
        "/api/v1/widecolumn/tables/users/scan",
        json!({"prefix":"user:"}),
    );
    assert_eq!(v["count"], 2);
    assert_eq!(v["rows"][0]["key"], "user:1");
    // delete a cell, then a row
    assert_eq!(
        api.delete("/api/v1/widecolumn/tables/users/rows/user:1/columns/age")
            .0,
        StatusCode::OK
    );
    assert_eq!(
        api.delete("/api/v1/widecolumn/tables/users/rows/user:2").0,
        StatusCode::OK
    );
    assert_eq!(
        api.get("/api/v1/widecolumn/tables/users/rows/user:2").0,
        StatusCode::NOT_FOUND
    );
}

fn ledger(api: &Api) {
    assert_eq!(
        api.post("/api/v1/ledger/ledgers", json!({"name":"audit"}))
            .0,
        StatusCode::CREATED
    );
    for ev in ["create", "update", "delete"] {
        let (s, _) = api.post(
            "/api/v1/ledger/ledgers/audit/entries",
            json!({"payload":{"event":ev}}),
        );
        assert_eq!(s, StatusCode::OK);
    }
    let (_, v) = api.get("/api/v1/ledger/ledgers/audit/verify");
    assert_eq!(v["valid"], true);
    assert_eq!(v["entries"], 3);
    assert_eq!(
        api.get("/api/v1/ledger/ledgers/audit/entries/1").1["seq"],
        1
    );
    assert_eq!(
        api.get("/api/v1/ledger/ledgers/audit/entries?start=1&limit=1")
            .1["count"],
        1
    );
    // missing entry => 404
    assert_eq!(
        api.get("/api/v1/ledger/ledgers/audit/entries/999").0,
        StatusCode::NOT_FOUND
    );

    // Concurrency: many parallel appends must keep the hash chain valid.
    let base = api.base.clone();
    let token = api.token.clone();
    std::thread::scope(|scope| {
        for i in 0..40 {
            let (base, token) = (base.clone(), token.clone());
            scope.spawn(move || {
                Client::new()
                    .post(format!("{base}/api/v1/ledger/ledgers/audit/entries"))
                    .bearer_auth(&token)
                    .json(&json!({"payload":{"concurrent": i}}))
                    .send()
                    .unwrap();
            });
        }
    });
    let (_, v) = api.get("/api/v1/ledger/ledgers/audit/verify");
    assert_eq!(v["entries"], 43, "concurrent appends lost entries");
    assert_eq!(v["valid"], true, "concurrent appends corrupted the chain");
}

/// Writing to a never-created container must auto-create it (no 404) across the
/// paradigms — the usability win, exercised end to end over HTTP.
fn auto_create(api: &Api) {
    // geo: upsert a feature into a collection that was never created
    assert_eq!(
        api.post(
            "/api/v1/geo/collections/autogeo/features",
            json!({"id":"p","lat":1.0,"lon":2.0})
        )
        .0,
        StatusCode::OK
    );
    assert_eq!(api.get("/api/v1/geo/collections/autogeo").1["count"], 1);
    // columnar: insert with inferred schema
    assert_eq!(
        api.post(
            "/api/v1/columnar/tables/autocol/rows",
            json!({"rows":[{"k":"a","n":5}]})
        )
        .1["inserted"],
        1
    );
    assert_eq!(api.get("/api/v1/columnar/tables/autocol").1["rows"], 1);
    // object: PUT into a never-created bucket
    let put = Client::new()
        .put(format!(
            "{}/api/v1/objects/buckets/autobucket/object/k",
            api.base
        ))
        .bearer_auth(&api.token)
        .body("x")
        .send()
        .unwrap();
    assert_eq!(put.status(), StatusCode::OK);
    assert_eq!(
        api.get("/api/v1/objects/buckets/autobucket").1["objects"],
        1
    );
    // widecolumn: put a row into a never-created table
    assert_eq!(
        api.put(
            "/api/v1/widecolumn/tables/autowc/rows/r1",
            json!({"columns":{"v":1}})
        )
        .0,
        StatusCode::OK
    );
    assert_eq!(api.get("/api/v1/widecolumn/tables/autowc").1["rows"], 1);
    // ledger: append to a never-created ledger
    assert_eq!(
        api.post(
            "/api/v1/ledger/ledgers/autoledger/entries",
            json!({"payload":{"x":1}})
        )
        .0,
        StatusCode::OK
    );
    assert_eq!(
        api.get("/api/v1/ledger/ledgers/autoledger/verify").1["valid"],
        true
    );
}

/// After a graceful restart, all five engines must have reloaded their data.
fn verify_persisted(api: &Api) {
    // geo: la was deleted before shutdown, so 3 remain
    assert_eq!(api.get("/api/v1/geo/collections/cities").1["count"], 3);
    assert_eq!(
        api.post(
            "/api/v1/geo/collections/cities/nearest",
            json!({"lat":40.7,"lon":-74.0,"k":1})
        )
        .1["hits"][0]["id"],
        "nyc"
    );
    // columnar
    let (_, v) = api.post(
        "/api/v1/columnar/tables/sales/aggregate",
        json!({"group_by":[],"aggregates":[{"func":"sum","column":"amount"}]}),
    );
    assert_eq!(v["groups"][0]["values"][0], 375.0); // 100+200+75
                                                    // object
    let bytes = Client::new()
        .get(format!(
            "{}/api/v1/objects/buckets/media/object/notes/a.txt",
            api.base
        ))
        .bearer_auth(&api.token)
        .send()
        .unwrap()
        .text()
        .unwrap();
    assert_eq!(bytes, "hello aegis blob");
    // widecolumn
    assert_eq!(
        api.get("/api/v1/widecolumn/tables/users/rows/user:1").1["columns"]["name"],
        "Alice"
    );
    // ledger: chain re-verifies after deserialization
    let (_, v) = api.get("/api/v1/ledger/ledgers/audit/verify");
    assert_eq!(v["valid"], true);
    assert_eq!(v["entries"], 43);
}
