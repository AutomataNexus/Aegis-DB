//! Query builder modal with syntax highlighting

use leptos::*;
use crate::api;
use super::super::shared::types::*;
use super::super::state::use_dashboard_context;

/// Query builder modal with syntax highlighting
#[component]
pub fn QueryBuilderModal() -> impl IntoView {
    let ctx = use_dashboard_context();

    let close_modal = move |_| {
        ctx.active_modal.set(BrowserModal::None);
        ctx.query_result.set(None);
    };

    // Execute query
    let execute_query = create_action(move |_: &()| async move {
        let query = ctx.query_input.get();
        if query.is_empty() {
            return;
        }

        ctx.modal_loading.set(true);

        // Add to history
        ctx.query_history.update(|h| {
            if !h.contains(&query) {
                h.insert(0, query.clone());
                if h.len() > 10 {
                    h.pop();
                }
            }
        });

        let paradigm = ctx.query_tab.get();
        let result = api::execute_builder_query(&query, &paradigm).await;

        match result {
            Ok(res) => ctx.query_result.set(Some(res)),
            Err(e) => {
                ctx.query_result.set(Some(crate::types::QueryBuilderResult {
                    success: false,
                    error: Some(e),
                    columns: vec![],
                    rows: vec![],
                    execution_time_ms: 0,
                    rows_affected: 0,
                }));
            }
        }

        ctx.modal_loading.set(false);
    });

    view! {
        <Show when=move || ctx.active_modal.get() == BrowserModal::QueryBuilder>
            <div class="modal-overlay" on:click=close_modal>
                <div class="modal-content query-builder-modal" on:click=|e| e.stop_propagation()>
                    <div class="modal-header">
                        <h2>"📊 Query Builder"</h2>
                        <button class="modal-close" on:click=close_modal>"×"</button>
                    </div>
                    <div class="modal-body">
                        // Query type tabs
                        <div class="query-tabs">
                            <button
                                class=move || if ctx.query_tab.get() == "sql" { "tab-btn active" } else { "tab-btn" }
                                on:click=move |_| ctx.query_tab.set("sql".to_string())
                            >"SQL"</button>
                            <button
                                class=move || if ctx.query_tab.get() == "graphql" { "tab-btn active" } else { "tab-btn" }
                                on:click=move |_| ctx.query_tab.set("graphql".to_string())
                            >"GraphQL"</button>
                            <button
                                class=move || if ctx.query_tab.get() == "cypher" { "tab-btn active" } else { "tab-btn" }
                                on:click=move |_| ctx.query_tab.set("cypher".to_string())
                            >"Cypher"</button>
                        </div>

                        // Query editor with syntax highlighting
                        <div class="query-editor">
                            <div class="editor-container">
                                // Highlighted overlay
                                <pre class="syntax-highlight" aria-hidden="true">
                                    {move || {
                                        let query = ctx.query_input.get();
                                        let tab = ctx.query_tab.get();
                                        let tokens = tokenize_query(&query, &tab);
                                        tokens.into_iter().map(|(text, token_type)| {
                                            let class = token_class(token_type);
                                            view! { <span class=class>{text}</span> }
                                        }).collect_view()
                                    }}
                                </pre>
                                // Actual textarea
                                <textarea
                                    class="query-input"
                                    placeholder=move || match ctx.query_tab.get().as_str() {
                                        "sql" => "SELECT * FROM users WHERE id = 1;",
                                        "graphql" => "{ users { id name email } }",
                                        "cypher" => "MATCH (n:Person) RETURN n LIMIT 10",
                                        _ => "Enter query...",
                                    }
                                    prop:value=move || ctx.query_input.get()
                                    on:input=move |e| ctx.query_input.set(event_target_value(&e))
                                    spellcheck="false"
                                ></textarea>
                            </div>

                            <div class="editor-toolbar">
                                <button
                                    class="btn btn-primary"
                                    on:click=move |_| execute_query.dispatch(())
                                    disabled=move || ctx.modal_loading.get()
                                >
                                    {move || if ctx.modal_loading.get() { "Running..." } else { "▶ Run Query" }}
                                </button>
                                <button
                                    class="btn btn-secondary"
                                    on:click=move |_| ctx.query_input.set(String::new())
                                >"Clear"</button>
                            </div>
                        </div>

                        // Query history
                        <Show when=move || !ctx.query_history.get().is_empty()>
                            <div class="query-history">
                                <h4>"Recent Queries"</h4>
                                <div class="history-list">
                                    {move || ctx.query_history.get().into_iter().take(5).map(|q| {
                                        let query = q.clone();
                                        view! {
                                            <div
                                                class="history-item"
                                                on:click=move |_| ctx.query_input.set(query.clone())
                                            >
                                                {q}
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>
                        </Show>

                        // Results
                        <Show when=move || ctx.query_result.get().is_some()>
                            {move || ctx.query_result.get().map(|result| {
                                if let Some(error) = result.error {
                                    view! {
                                        <div class="query-error">
                                            <strong>"Error: "</strong>{error}
                                        </div>
                                    }.into_view()
                                } else {
                                    view! {
                                        <div class="query-results">
                                            <div class="results-meta">
                                                <span>{format!("{} rows", result.rows.len())}</span>
                                                <span>{format!("{}ms", result.execution_time_ms)}</span>
                                            </div>
                                            <div class="results-table-container">
                                                <table class="results-table">
                                                    <thead>
                                                        <tr>
                                                            {result.columns.iter().map(|col| view! { <th>{col}</th> }).collect_view()}
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {result.rows.iter().map(|row| view! {
                                                            <tr>
                                                                {row.iter().map(|cell| {
                                                                    let display = match cell {
                                                                        serde_json::Value::String(s) => s.clone(),
                                                                        serde_json::Value::Null => "NULL".to_string(),
                                                                        v => v.to_string(),
                                                                    };
                                                                    view! { <td>{display}</td> }
                                                                }).collect_view()}
                                                            </tr>
                                                        }).collect_view()}
                                                    </tbody>
                                                </table>
                                            </div>
                                        </div>
                                    }.into_view()
                                }
                            })}
                        </Show>
                    </div>
                </div>
            </div>
        </Show>
    }
}

/// Tokenize query for syntax highlighting
fn tokenize_query(query: &str, query_type: &str) -> Vec<(String, TokenType)> {
    match query_type {
        "sql" => tokenize_sql(query),
        "graphql" => tokenize_graphql(query),
        "cypher" => tokenize_cypher(query),
        _ => vec![(query.to_string(), TokenType::Identifier)],
    }
}

/// SQL tokenizer
fn tokenize_sql(query: &str) -> Vec<(String, TokenType)> {
    let keywords = ["SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "IN", "LIKE",
                    "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE",
                    "CREATE", "TABLE", "DROP", "ALTER", "INDEX", "JOIN",
                    "LEFT", "RIGHT", "INNER", "OUTER", "ON", "AS", "ORDER",
                    "BY", "ASC", "DESC", "LIMIT", "OFFSET", "GROUP", "HAVING",
                    "DISTINCT", "COUNT", "SUM", "AVG", "MIN", "MAX", "NULL",
                    "TRUE", "FALSE", "IS", "BETWEEN", "EXISTS", "CASE", "WHEN",
                    "THEN", "ELSE", "END", "UNION", "ALL", "PRIMARY", "KEY",
                    "FOREIGN", "REFERENCES", "DEFAULT", "CONSTRAINT", "CHECK"];

    let mut tokens = Vec::new();
    let chars: Vec<char> = query.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Whitespace
        if c.is_whitespace() {
            let mut ws = String::new();
            while i < chars.len() && chars[i].is_whitespace() {
                ws.push(chars[i]);
                i += 1;
            }
            tokens.push((ws, TokenType::Whitespace));
            continue;
        }

        // String literals
        if c == '\'' || c == '"' {
            let quote = c;
            let mut s = String::new();
            s.push(c);
            i += 1;
            while i < chars.len() && chars[i] != quote {
                s.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                s.push(chars[i]);
                i += 1;
            }
            tokens.push((s, TokenType::String));
            continue;
        }

        // Comments
        if c == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            let mut comment = String::new();
            while i < chars.len() && chars[i] != '\n' {
                comment.push(chars[i]);
                i += 1;
            }
            tokens.push((comment, TokenType::Comment));
            continue;
        }

        // Numbers
        if c.is_ascii_digit() {
            let mut num = String::new();
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                num.push(chars[i]);
                i += 1;
            }
            tokens.push((num, TokenType::Number));
            continue;
        }

        // Identifiers, keywords, functions, and types
        if c.is_alphabetic() || c == '_' {
            let mut ident = String::new();
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                ident.push(chars[i]);
                i += 1;
            }
            let upper = ident.to_uppercase();
            // SQL functions
            let functions = ["COUNT", "SUM", "AVG", "MIN", "MAX", "COALESCE", "CONCAT",
                            "LENGTH", "UPPER", "LOWER", "TRIM", "SUBSTRING", "NOW",
                            "DATE", "TIME", "YEAR", "MONTH", "DAY", "CAST", "CONVERT"];
            // SQL types
            let types = ["INT", "INTEGER", "BIGINT", "SMALLINT", "TINYINT", "FLOAT",
                        "DOUBLE", "DECIMAL", "NUMERIC", "VARCHAR", "CHAR", "TEXT",
                        "BOOLEAN", "BOOL", "DATE", "TIME", "TIMESTAMP", "DATETIME",
                        "BLOB", "JSON", "UUID", "SERIAL"];
            if functions.contains(&upper.as_str()) {
                tokens.push((ident, TokenType::Function));
            } else if types.contains(&upper.as_str()) {
                tokens.push((ident, TokenType::Type));
            } else if keywords.contains(&upper.as_str()) {
                tokens.push((ident, TokenType::Keyword));
            } else {
                tokens.push((ident, TokenType::Identifier));
            }
            continue;
        }

        // Operators and punctuation
        if "=<>!+-*/%".contains(c) {
            let mut op = String::new();
            op.push(c);
            i += 1;
            if i < chars.len() && "=<>".contains(chars[i]) {
                op.push(chars[i]);
                i += 1;
            }
            tokens.push((op, TokenType::Operator));
            continue;
        }

        // Punctuation
        if "(),;.".contains(c) {
            tokens.push((c.to_string(), TokenType::Punctuation));
            i += 1;
            continue;
        }

        // Unknown
        tokens.push((c.to_string(), TokenType::Identifier));
        i += 1;
    }

    tokens
}

/// GraphQL tokenizer (simplified)
fn tokenize_graphql(query: &str) -> Vec<(String, TokenType)> {
    let keywords = ["query", "mutation", "subscription", "fragment", "on", "type",
                    "interface", "union", "enum", "input", "scalar", "directive"];

    let mut tokens = Vec::new();
    let chars: Vec<char> = query.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() {
            let mut ws = String::new();
            while i < chars.len() && chars[i].is_whitespace() {
                ws.push(chars[i]);
                i += 1;
            }
            tokens.push((ws, TokenType::Whitespace));
        } else if c == '"' {
            let mut s = String::new();
            s.push(c);
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                s.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                s.push(chars[i]);
                i += 1;
            }
            tokens.push((s, TokenType::String));
        } else if c == '#' {
            let mut comment = String::new();
            while i < chars.len() && chars[i] != '\n' {
                comment.push(chars[i]);
                i += 1;
            }
            tokens.push((comment, TokenType::Comment));
        } else if c.is_alphabetic() || c == '_' {
            let mut ident = String::new();
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                ident.push(chars[i]);
                i += 1;
            }
            if keywords.contains(&ident.to_lowercase().as_str()) {
                tokens.push((ident, TokenType::Keyword));
            } else {
                tokens.push((ident, TokenType::Identifier));
            }
        } else if "{}[]():!$@".contains(c) {
            tokens.push((c.to_string(), TokenType::Punctuation));
            i += 1;
        } else {
            tokens.push((c.to_string(), TokenType::Identifier));
            i += 1;
        }
    }

    tokens
}

/// Cypher tokenizer (simplified)
fn tokenize_cypher(query: &str) -> Vec<(String, TokenType)> {
    let keywords = ["MATCH", "WHERE", "RETURN", "CREATE", "DELETE", "SET", "REMOVE",
                    "MERGE", "WITH", "ORDER", "BY", "LIMIT", "SKIP", "UNWIND",
                    "AND", "OR", "NOT", "IN", "AS", "OPTIONAL", "DETACH", "CALL"];

    let mut tokens = Vec::new();
    let chars: Vec<char> = query.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() {
            let mut ws = String::new();
            while i < chars.len() && chars[i].is_whitespace() {
                ws.push(chars[i]);
                i += 1;
            }
            tokens.push((ws, TokenType::Whitespace));
        } else if c == '\'' || c == '"' {
            let quote = c;
            let mut s = String::new();
            s.push(c);
            i += 1;
            while i < chars.len() && chars[i] != quote {
                s.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                s.push(chars[i]);
                i += 1;
            }
            tokens.push((s, TokenType::String));
        } else if c.is_alphabetic() || c == '_' {
            let mut ident = String::new();
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                ident.push(chars[i]);
                i += 1;
            }
            if keywords.contains(&ident.to_uppercase().as_str()) {
                tokens.push((ident, TokenType::Keyword));
            } else {
                tokens.push((ident, TokenType::Identifier));
            }
        } else if "()[]{}:.-><".contains(c) {
            tokens.push((c.to_string(), TokenType::Punctuation));
            i += 1;
        } else {
            tokens.push((c.to_string(), TokenType::Identifier));
            i += 1;
        }
    }

    tokens
}

/// Get CSS class for token type
fn token_class(token_type: TokenType) -> &'static str {
    match token_type {
        TokenType::Keyword => "token-keyword",
        TokenType::String => "token-string",
        TokenType::Number => "token-number",
        TokenType::Operator => "token-operator",
        TokenType::Identifier => "token-identifier",
        TokenType::Punctuation => "token-punctuation",
        TokenType::Comment => "token-comment",
        TokenType::Function => "token-function",
        TokenType::Type => "token-type",
        TokenType::Whitespace => "token-whitespace",
    }
}
