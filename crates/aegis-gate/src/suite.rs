//! The query and document-store suite run against the booted `aegis-server`: every data route
//! refuses a caller without a token, a wrong password is refused and the one-run admin signs in,
//! SQL DDL / DML / WHERE / ORDER BY / LIMIT / params / aggregates / GROUP BY / HAVING answer the
//! expected rows, bad SQL is refused without taking the server down, collections and documents
//! are created, read, filtered, sorted, paged, patched, replaced and deleted, then the server is
//! stopped with SIGTERM, booted again on the same data dir and the same rows and documents read back.

use crate::node::{self, head, Node, Run};
use crate::Step;
use serde_json::{json, Value};

// ── fixtures ──

const TABLE: &str = "gate_parts";
const COLLECTION: &str = "gate_units";

const UNAUTHED: &[(&str, &str)] = &[
    ("GET", "/api/v1/tables"),
    ("POST", "/api/v1/query"),
    ("GET", "/api/v1/documents/collections"),
    ("POST", "/api/v1/documents/collections/gate_units/query"),
    (
        "GET",
        "/api/v1/documents/collections/gate_units/documents/a",
    ),
    ("GET", "/api/v1/kv/keys"),
];

fn rows_after_dml() -> Value {
    json!([[1, "bolt", 4], [3, "bolt", 2], [4, "gear", 10]])
}

// ── http helpers ──

struct Api<'a> {
    node: &'a Node,
    token: Option<String>,
}

impl Api<'_> {
    fn call(&self, method: &str, path: &str, body: &Value) -> Result<(u16, Value), String> {
        let text = if body.is_null() {
            String::new()
        } else {
            body.to_string()
        };
        let r = node::request(self.node.port, method, path, &text, self.token.as_deref())?;
        let v = r.json().unwrap_or(Value::String(head(&r.body)));
        Ok((r.status, v))
    }

    fn expect(
        &self,
        method: &str,
        path: &str,
        body: Value,
        want: u16,
        p: &mut Vec<String>,
    ) -> Option<Value> {
        match self.call(method, path, &body) {
            Ok((s, v)) if s == want => Some(v),
            Ok((s, v)) => {
                p.push(format!(
                    "{method} {path} answered {s} (want {want}): {}",
                    head(&v.to_string())
                ));
                None
            }
            Err(e) => {
                p.push(format!("{method} {e}"));
                None
            }
        }
    }

    fn sql(&self, sql: &str, params: Value, p: &mut Vec<String>) -> Option<Value> {
        let body = json!({ "sql": sql, "params": params });
        let v = self.expect("POST", "/api/v1/query", body, 200, p)?;
        if v["success"] != true {
            p.push(format!(
                "{sql}: success is not true: {}",
                head(&v.to_string())
            ));
            return None;
        }
        Some(v["data"].clone())
    }

    fn rows(&self, sql: &str, params: Value, want: Value, p: &mut Vec<String>) {
        if let Some(d) = self.sql(sql, params, p) {
            if d["rows"] != want {
                p.push(format!("{sql}: rows {} (want {want})", d["rows"]));
            }
        }
    }

    fn affected(&self, sql: &str, want: u64, p: &mut Vec<String>) {
        if let Some(d) = self.sql(sql, json!([]), p) {
            if d["rows_affected"] != want {
                p.push(format!(
                    "{sql}: rows_affected {} (want {want})",
                    d["rows_affected"]
                ));
            }
        }
    }

    fn refused_sql(&self, sql: &str, p: &mut Vec<String>) {
        match self.call("POST", "/api/v1/query", &json!({ "sql": sql })) {
            Ok((s, v)) if s >= 400 && v["success"] == false && v["error"].is_string() => {}
            Ok((s, v)) => p.push(format!(
                "{sql}: answered {s} {} (want a refusal with success false and an error)",
                head(&v.to_string())
            )),
            Err(e) => p.push(format!("{sql}: {e}")),
        }
    }

    fn doc(&self, id: &str, p: &mut Vec<String>) -> Option<Value> {
        let path = format!("/api/v1/documents/collections/{COLLECTION}/documents/{id}");
        self.expect("GET", &path, Value::Null, 200, p)
            .map(|v| v["data"].clone())
    }

    fn query_ids(&self, body: Value, p: &mut Vec<String>) -> Option<(Vec<String>, Value)> {
        let path = format!("/api/v1/documents/collections/{COLLECTION}/query");
        let v = self.expect("POST", &path, body, 200, p)?;
        let ids = v["documents"]
            .as_array()
            .map(|d| {
                d.iter()
                    .filter_map(|x| x["id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        Some((ids, v["next_cursor"].clone()))
    }
}

fn want_ids(label: &str, got: Option<(Vec<String>, Value)>, want: &[&str], p: &mut Vec<String>) {
    if let Some((ids, _)) = got {
        if ids != want {
            p.push(format!("{label}: ids {ids:?} (want {want:?})"));
        }
    }
}

fn want_field(id: &str, doc: &Option<Value>, ptr: &str, want: Value, p: &mut Vec<String>) {
    if let Some(d) = doc {
        let got = d.pointer(ptr).cloned().unwrap_or(Value::Null);
        if got != want {
            p.push(format!("document {id}{ptr} = {got} (want {want})"));
        }
    }
}

fn check(name: &str, problems: Vec<String>) -> Step {
    Step {
        name: name.to_string(),
        problems,
    }
}

// ── auth ──

fn unauthed(n: &Node) -> Step {
    let api = Api {
        node: n,
        token: None,
    };
    let mut p = Vec::new();
    for (m, path) in UNAUTHED {
        let body = if *m == "POST" {
            json!({ "sql": "SELECT 1", "filter": {} })
        } else {
            Value::Null
        };
        api.expect(m, path, body, 401, &mut p);
    }
    check("every data route refuses a caller without a token", p)
}

fn sign_in(run: &Run, n: &Node) -> (Step, Option<String>) {
    let api = Api {
        node: n,
        token: None,
    };
    let mut p = Vec::new();
    let wrong = json!({ "username": run.user, "password": format!("{}x", run.pass) });
    if let Some(v) = api.expect("POST", "/api/v1/auth/login", wrong, 401, &mut p) {
        if !v["token"].is_null() {
            p.push("a wrong password got a token".into());
        }
    }
    let right = json!({ "username": run.user, "password": run.pass });
    let token = api
        .expect("POST", "/api/v1/auth/login", right, 200, &mut p)
        .and_then(|v| {
            if v["user"]["role"] != "admin" {
                p.push(format!(
                    "signed-in role is {} (want admin)",
                    v["user"]["role"]
                ));
            }
            v["token"].as_str().map(str::to_string)
        });
    if token.is_none() && p.is_empty() {
        p.push("login answered 200 without a token".into());
    }
    (
        check("a wrong password is refused; the one-run admin signs in", p),
        token,
    )
}

// ── sql ──

fn sql_writes(api: &Api) -> Step {
    let mut p = Vec::new();
    api.sql(
        &format!("CREATE TABLE {TABLE} (id INTEGER NOT NULL, kind VARCHAR(16), qty INTEGER, price FLOAT)"),
        json!([]),
        &mut p,
    );
    api.affected(
        &format!("INSERT INTO {TABLE} (id, kind, qty, price) VALUES (1, 'bolt', 4, 0.25), (2, 'nut', 6, 0.1), (3, 'bolt', 2, 1.5), (4, 'gear', 1, 12.5)"),
        4,
        &mut p,
    );
    api.affected(
        &format!("UPDATE {TABLE} SET qty = 10 WHERE id = 4"),
        1,
        &mut p,
    );
    api.affected(&format!("DELETE FROM {TABLE} WHERE id = 2"), 1, &mut p);
    api.rows(
        &format!("SELECT id, kind, qty FROM {TABLE} ORDER BY id"),
        json!([]),
        rows_after_dml(),
        &mut p,
    );
    check("SQL CREATE TABLE / INSERT / UPDATE / DELETE", p)
}

fn sql_reads(api: &Api) -> Step {
    let mut p = Vec::new();
    api.rows(
        &format!("SELECT id, kind FROM {TABLE} WHERE qty > 2 ORDER BY qty DESC"),
        json!([]),
        json!([[4, "gear"], [1, "bolt"]]),
        &mut p,
    );
    api.rows(
        &format!("SELECT id FROM {TABLE} ORDER BY id LIMIT 2"),
        json!([]),
        json!([[1], [3]]),
        &mut p,
    );
    api.rows(
        &format!("SELECT kind, price FROM {TABLE} WHERE id = $1"),
        json!([3]),
        json!([["bolt", 1.5]]),
        &mut p,
    );
    if let Some(v) = api.expect("GET", "/api/v1/tables", Value::Null, 200, &mut p) {
        let t = v["tables"]
            .as_array()
            .and_then(|ts| ts.iter().find(|t| t["name"] == TABLE));
        match t {
            Some(t) if t["row_count"] == 3 && t["columns"].as_array().map(Vec::len) == Some(4) => {}
            Some(t) => p.push(format!("/api/v1/tables {TABLE}: {}", head(&t.to_string()))),
            None => p.push(format!("/api/v1/tables does not list {TABLE}")),
        }
    }
    check(
        "SQL WHERE / ORDER BY / LIMIT / $1 params; /tables lists it",
        p,
    )
}

fn sql_aggregates(api: &Api) -> Step {
    let mut p = Vec::new();
    for (sql, want) in [
        (format!("SELECT COUNT(*) FROM {TABLE}"), json!([[3]])),
        (
            format!("SELECT COUNT(*) AS n, SUM(qty) AS total, MIN(qty), MAX(qty) FROM {TABLE}"),
            json!([[3, 16.0, 2, 10]]),
        ),
        (
            format!("SELECT kind, COUNT(*) AS n FROM {TABLE} GROUP BY kind ORDER BY kind"),
            json!([["bolt", 2], ["gear", 1]]),
        ),
        (
            format!("SELECT kind FROM {TABLE} GROUP BY kind HAVING COUNT(*) > 1"),
            json!([["bolt"]]),
        ),
    ] {
        api.rows(&sql, json!([]), want, &mut p);
    }
    check("SQL COUNT / SUM / MIN / MAX / GROUP BY / HAVING", p)
}

fn sql_errors(api: &Api) -> Step {
    let mut p = Vec::new();
    api.refused_sql("SELEC * FORM nowhere", &mut p);
    api.refused_sql("SELECT * FROM gate_no_such_table", &mut p);
    api.refused_sql(&format!("CREATE TABLE {TABLE} (id INTEGER)"), &mut p);
    api.rows(
        &format!("SELECT COUNT(*) FROM {TABLE}"),
        json!([]),
        json!([[3]]),
        &mut p,
    );
    check(
        "bad SQL (syntax, unknown table, duplicate table) is refused; the server keeps serving",
        p,
    )
}

// ── documents ──

fn collections(api: &Api) -> Step {
    let mut p = Vec::new();
    let body = || json!({ "name": COLLECTION });
    api.expect("POST", "/api/v1/documents/collections", body(), 201, &mut p);
    match api.call("POST", "/api/v1/documents/collections", &body()) {
        Ok((s, v)) if (400..500).contains(&s) && v["success"] == false => {}
        Ok((s, v)) => p.push(format!(
            "duplicate collection answered {s} {}",
            head(&v.to_string())
        )),
        Err(e) => p.push(e),
    }
    check("collection create; a duplicate is refused", p)
}

fn documents(api: &Api) -> Step {
    let mut p = Vec::new();
    let path = format!("/api/v1/documents/collections/{COLLECTION}/documents");
    for (id, doc) in [
        (
            "a",
            json!({ "name": "ahu-1", "qty": 1, "tags": ["x", "y"], "site": { "zone": "n" } }),
        ),
        ("b", json!({ "name": "ahu-2", "qty": 5 })),
        ("c", json!({ "name": "rtu-1", "qty": 3 })),
    ] {
        if let Some(v) = api.expect(
            "POST",
            &path,
            json!({ "id": id, "document": doc }),
            201,
            &mut p,
        ) {
            if v["id"] != id {
                p.push(format!("insert {id} answered id {}", v["id"]));
            }
        }
    }
    match api.call(
        "POST",
        &path,
        &json!({ "id": "a", "document": { "name": "dup" } }),
    ) {
        Ok((s, _)) if (400..500).contains(&s) => {}
        Ok((s, v)) => p.push(format!(
            "duplicate id a answered {s} {}",
            head(&v.to_string())
        )),
        Err(e) => p.push(e),
    }
    let a = api.doc("a", &mut p);
    want_field("a", &a, "/name", json!("ahu-1"), &mut p);
    want_field("a", &a, "/tags", json!(["x", "y"]), &mut p);
    want_field("a", &a, "/site/zone", json!("n"), &mut p);
    check(
        "documents insert with ids; a duplicate id is refused; nested fields round-trip",
        p,
    )
}

fn document_queries(api: &Api) -> Step {
    let mut p = Vec::new();
    want_ids(
        "$gt + sort desc",
        api.query_ids(
            json!({ "filter": { "qty": { "$gt": 1 } }, "sort": { "field": "qty", "ascending": false } }),
            &mut p,
        ),
        &["b", "c"],
        &mut p,
    );
    want_ids(
        "$in + sort",
        api.query_ids(
            json!({ "filter": { "name": { "$in": ["ahu-1", "rtu-1"] } }, "sort": { "field": "name", "ascending": true } }),
            &mut p,
        ),
        &["a", "c"],
        &mut p,
    );
    let page = |cursor: Value| json!({ "filter": {}, "sort": { "field": "qty", "ascending": true }, "limit": 2, "cursor": cursor });
    if let Some((ids, next)) = api.query_ids(page(Value::Null), &mut p) {
        if ids != ["a", "c"] {
            p.push(format!("page 1: ids {ids:?} (want [\"a\", \"c\"])"));
        }
        if next.is_string() {
            if let Some((ids, last)) = api.query_ids(page(next), &mut p) {
                if ids != ["b"] || !last.is_null() {
                    p.push(format!(
                        "page 2: ids {ids:?}, next_cursor {last} (want [\"b\"], none)"
                    ));
                }
            }
        } else {
            p.push(format!("page 1 has no next_cursor ({next})"));
        }
    }
    check(
        "document queries: $gt / $in filters, sort, limit + cursor paging",
        p,
    )
}

fn document_writes(api: &Api) -> Step {
    let mut p = Vec::new();
    let path = |id: &str| format!("/api/v1/documents/collections/{COLLECTION}/documents/{id}");
    api.expect(
        "PATCH",
        &path("a"),
        json!({ "document": { "qty": 7 } }),
        200,
        &mut p,
    );
    let a = api.doc("a", &mut p);
    want_field("a", &a, "/qty", json!(7), &mut p);
    want_field("a", &a, "/name", json!("ahu-1"), &mut p);
    want_field("a", &a, "/site/zone", json!("n"), &mut p);
    api.expect(
        "PUT",
        &path("b"),
        json!({ "document": { "name": "ahu-2b" } }),
        200,
        &mut p,
    );
    let b = api.doc("b", &mut p);
    want_field("b", &b, "/name", json!("ahu-2b"), &mut p);
    want_field("b", &b, "/qty", Value::Null, &mut p);
    api.expect("DELETE", &path("c"), Value::Null, 200, &mut p);
    api.expect("GET", &path("c"), Value::Null, 404, &mut p);
    if let Some(v) = api.expect(
        "GET",
        "/api/v1/documents/collections",
        Value::Null,
        200,
        &mut p,
    ) {
        let found = v
            .as_array()
            .and_then(|cs| cs.iter().find(|c| c["name"] == COLLECTION))
            .map(|c| c["document_count"].clone());
        if found != Some(json!(2)) {
            p.push(format!(
                "collection list: {COLLECTION} document_count {found:?} (want 2)"
            ));
        }
    }
    check(
        "document PATCH merges, PUT replaces, DELETE then 404; the collection counts 2",
        p,
    )
}

// ── boots ──

pub fn first_boot(run: &Run, emit: &mut dyn FnMut(Step)) -> bool {
    let n = match run.boot("first") {
        Ok(n) => n,
        Err(e) => {
            emit(check(
                "aegis-server boots on 127.0.0.1 and /health answers",
                vec![e],
            ));
            return false;
        }
    };
    emit(check(
        "aegis-server boots on 127.0.0.1 and /health answers",
        Vec::new(),
    ));
    emit(unauthed(&n));
    let (s, token) = sign_in(run, &n);
    emit(s);
    let Some(token) = token else {
        return false;
    };
    {
        let api = Api {
            node: &n,
            token: Some(token),
        };
        for step in [
            sql_writes,
            sql_reads,
            sql_aggregates,
            sql_errors,
            collections,
            documents,
            document_queries,
            document_writes,
        ] {
            emit(step(&api));
        }
    }
    let mut p = node::log_problems(&n.log);
    if let Err(e) = n.stop() {
        p.push(e);
    }
    let clean = p.is_empty();
    emit(check(
        "no panic in the server log; SIGTERM stops it cleanly (data saved)",
        p,
    ));
    clean
}

pub fn second_boot(run: &Run, emit: &mut dyn FnMut(Step)) {
    let mut n = match run.boot("second") {
        Ok(n) => n,
        Err(e) => {
            emit(check("boots again on the same data dir", vec![e]));
            return;
        }
    };
    let (s, token) = sign_in(run, &n);
    emit(check(
        "boots again on the same data dir; the admin still signs in",
        s.problems,
    ));
    if let Some(token) = token {
        let api = Api {
            node: &n,
            token: Some(token),
        };
        let mut p = Vec::new();
        api.rows(
            &format!("SELECT id, kind, qty FROM {TABLE} ORDER BY id"),
            json!([]),
            rows_after_dml(),
            &mut p,
        );
        api.rows(
            &format!("SELECT COUNT(*) FROM {TABLE}"),
            json!([]),
            json!([[3]]),
            &mut p,
        );
        emit(check("SQL rows survive the restart", p));

        let mut p = Vec::new();
        let a = api.doc("a", &mut p);
        want_field("a", &a, "/qty", json!(7), &mut p);
        want_field("a", &a, "/tags", json!(["x", "y"]), &mut p);
        let b = api.doc("b", &mut p);
        want_field("b", &b, "/name", json!("ahu-2b"), &mut p);
        let gone = format!("/api/v1/documents/collections/{COLLECTION}/documents/c");
        api.expect("GET", &gone, Value::Null, 404, &mut p);
        emit(check(
            "documents survive the restart (patched, replaced, deleted)",
            p,
        ));
    }
    let mut p = node::log_problems(&n.log);
    if !n.alive() {
        p.push(format!("exited (see {})", n.log.display()));
    }
    if let Err(e) = n.stop() {
        p.push(e);
    }
    emit(check(
        "no panic in the restarted server's log; it stops cleanly",
        p,
    ));
}
