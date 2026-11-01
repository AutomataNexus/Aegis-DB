//! Role enforcement: a valid session is not enough. Viewers are read-only,
//! writes need Operator or Admin, destructive and cluster routes need Admin.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

use aegis_server::{create_router, AppState, ServerConfig};

struct Ctx {
    app: axum::Router,
    admin: String,
    operator: String,
    viewer: String,
}

fn setup() -> Ctx {
    let state = AppState::new(ServerConfig::default());
    for (u, role) in [
        ("rbacadmin", "admin"),
        ("rbacop", "operator"),
        ("rbacview", "viewer"),
    ] {
        state
            .auth
            .create_user(u, &format!("{u}@test.local"), "RbacPass123!", role)
            .unwrap();
    }
    let token = |u: &str| state.auth.login(u, "RbacPass123!").token.unwrap();
    let (admin, operator, viewer) = (token("rbacadmin"), token("rbacop"), token("rbacview"));
    Ctx {
        app: create_router(state),
        admin,
        operator,
        viewer,
    }
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> StatusCode {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"));
    let body = match body {
        Some(b) => {
            req = req.header("Content-Type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    app.clone()
        .oneshot(req.body(body).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn viewer_reads_but_cannot_write() {
    let c = setup();
    let doc = json!({"document": {"a": 1}});
    assert_eq!(
        call(
            &c.app,
            "POST",
            "/api/v1/documents/collections/rbac/documents",
            &c.operator,
            Some(doc.clone())
        )
        .await,
        StatusCode::CREATED
    );
    assert_eq!(
        call(
            &c.app,
            "GET",
            "/api/v1/documents/collections/rbac/documents",
            &c.viewer,
            None
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &c.app,
            "POST",
            "/api/v1/documents/collections/rbac/query",
            &c.viewer,
            Some(json!({}))
        )
        .await,
        StatusCode::OK
    );
    for (method, uri, body) in [
        (
            "POST",
            "/api/v1/documents/collections/rbac/documents",
            Some(doc.clone()),
        ),
        (
            "POST",
            "/api/v1/kv/keys",
            Some(json!({"key": "k", "value": "v"})),
        ),
        ("DELETE", "/api/v1/kv/keys/k", None),
        (
            "POST",
            "/api/v1/timeseries/write",
            Some(json!({"metric": "m", "value": 1.0})),
        ),
        (
            "POST",
            "/api/v1/graph/nodes",
            Some(json!({"label": "x", "properties": {}})),
        ),
        (
            "POST",
            "/api/v1/vector/collections",
            Some(json!({"name": "v", "dim": 3})),
        ),
    ] {
        assert_eq!(
            call(&c.app, method, uri, &c.viewer, body).await,
            StatusCode::FORBIDDEN,
            "{method} {uri}"
        );
    }
}

#[tokio::test]
async fn viewer_sql_is_select_only() {
    let c = setup();
    let q = |sql: &str| Some(json!({"sql": sql}));
    assert_eq!(
        call(
            &c.app,
            "POST",
            "/api/v1/query",
            &c.operator,
            q("CREATE TABLE r (id INT)")
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &c.app,
            "POST",
            "/api/v1/query",
            &c.viewer,
            q("SELECT * FROM r")
        )
        .await,
        StatusCode::OK
    );
    for sql in [
        "INSERT INTO r VALUES (1)",
        "DROP TABLE r",
        "SELECT * FROM r; DELETE FROM r",
        "UPDATE r SET id = 2",
    ] {
        assert_eq!(
            call(&c.app, "POST", "/api/v1/query", &c.viewer, q(sql)).await,
            StatusCode::FORBIDDEN,
            "{sql}"
        );
        assert_eq!(
            call(
                &c.app,
                "POST",
                "/api/v1/query-builder/execute",
                &c.viewer,
                Some(json!({"query": sql}))
            )
            .await,
            StatusCode::FORBIDDEN,
            "{sql}"
        );
        assert_eq!(
            call(&c.app, "POST", "/api/v1/prepare", &c.viewer, q(sql)).await,
            StatusCode::FORBIDDEN,
            "{sql}"
        );
    }
}

#[tokio::test]
async fn prepared_statements_belong_to_their_owner() {
    let c = setup();
    let resp = c
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/prepare")
                .header("Authorization", format!("Bearer {}", c.operator))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"sql": "CREATE TABLE p (id INT)"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let id = serde_json::from_slice::<Value>(&body).unwrap()["statement_id"]
        .as_str()
        .unwrap()
        .to_string();
    let exec = json!({"statement_id": id});
    // Another user can neither run nor free it.
    assert_eq!(
        call(
            &c.app,
            "POST",
            "/api/v1/prepared/execute",
            &c.viewer,
            Some(exec.clone())
        )
        .await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &c.app,
            "DELETE",
            &format!("/api/v1/prepared/{id}"),
            &c.viewer,
            None
        )
        .await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &c.app,
            "POST",
            "/api/v1/prepared/execute",
            &c.operator,
            Some(exec)
        )
        .await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn destructive_and_cluster_routes_need_admin() {
    let c = setup();
    assert_eq!(
        call(
            &c.app,
            "POST",
            "/api/v1/documents/collections",
            &c.operator,
            Some(json!({"name": "dropme"}))
        )
        .await,
        StatusCode::CREATED
    );
    let join = json!({"node_id": "evil", "address": "169.254.169.254:80"});
    for token in [&c.viewer, &c.operator] {
        assert_eq!(
            call(
                &c.app,
                "DELETE",
                "/api/v1/documents/collections/dropme",
                token,
                None
            )
            .await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(
                &c.app,
                "POST",
                "/api/v1/cluster/join",
                token,
                Some(join.clone())
            )
            .await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(&c.app, "GET", "/api/v1/admin/backups", token, None).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(&c.app, "GET", "/api/v1/admin/users", token, None).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(
                &c.app,
                "POST",
                "/api/v1/admin/backup",
                token,
                Some(json!({}))
            )
            .await,
            StatusCode::FORBIDDEN
        );
    }
    // Admin: a malformed peer address is refused, the drop goes through.
    assert_eq!(
        call(
            &c.app,
            "POST",
            "/api/v1/cluster/join",
            &c.admin,
            Some(json!({"node_id": "n", "address": "http://169.254.169.254/latest"}))
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &c.app,
            "DELETE",
            "/api/v1/documents/collections/dropme",
            &c.admin,
            None
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        call(&c.app, "GET", "/api/v1/admin/users", &c.admin, None).await,
        StatusCode::OK
    );
    // Reads any role can do.
    assert_eq!(
        call(&c.app, "GET", "/api/v1/admin/stats", &c.viewer, None).await,
        StatusCode::OK
    );
}
