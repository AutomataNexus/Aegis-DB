//! Main dashboard page component
//! Matches NexusForge styling

use leptos::*;
use leptos_router::*;
use wasm_bindgen::JsCast;
use crate::api;
use crate::state::use_app_state;
use crate::types::*;

/// Dashboard page type
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum DashboardPage {
    #[default]
    Overview,
    Nodes,
    Database,
    Metrics,
    Settings,
}

/// Modal type for database browsers
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum BrowserModal {
    #[default]
    None,
    KeyValue,
    Collections,
    Graph,
    QueryBuilder,
    NodeDetail,
}

/// Dashboard page component
#[component]
pub fn Dashboard() -> impl IntoView {
    let navigate = use_navigate();
    let nav_auth = navigate.clone();
    let nav_logout = navigate.clone();
    let app_state = use_app_state();
    let app_state_logout = app_state.clone();
    let app_state_view = app_state.clone();

    // Redirect if not authenticated
    create_effect(move |_| {
        if !app_state.is_authenticated.get() {
            nav_auth("/login", Default::default());
        }
    });

    // State
    let (current_page, set_current_page) = create_signal(DashboardPage::Overview);
    let (loading, set_loading) = create_signal(true);
    let (cluster_status, set_cluster_status) = create_signal::<Option<ClusterStatus>>(None);
    let (nodes, set_nodes) = create_signal::<Vec<ClusterNode>>(vec![]);
    let (db_stats, set_db_stats) = create_signal::<Option<DatabaseStats>>(None);
    let (alerts, set_alerts) = create_signal::<Vec<Alert>>(vec![]);

    // Modal state
    let (active_modal, set_active_modal) = create_signal(BrowserModal::None);
    let (modal_loading, set_modal_loading) = create_signal(false);
    let (kv_entries, set_kv_entries) = create_signal::<Vec<KeyValueEntry>>(vec![]);
    let (collections, set_collections) = create_signal::<Vec<DocumentCollection>>(vec![]);
    let (selected_collection, set_selected_collection) = create_signal::<Option<String>>(None);
    let (collection_docs, set_collection_docs) = create_signal::<Vec<DocumentEntry>>(vec![]);
    let (graph_data, set_graph_data) = create_signal::<Option<GraphData>>(None);
    let (query_input, set_query_input) = create_signal(String::new());
    let (query_result, set_query_result) = create_signal::<Option<QueryBuilderResult>>(None);
    let (selected_node, set_selected_node) = create_signal::<Option<ClusterNode>>(None);
    let (metrics_time_range, set_metrics_time_range) = create_signal("6h".to_string());
    let (node_logs, set_node_logs) = create_signal::<Vec<api::NodeLogEntry>>(vec![]);
    let (node_action_message, set_node_action_message) = create_signal::<Option<String>>(None);
    let (show_logs_view, set_show_logs_view) = create_signal(false);

    // KV Browser state
    let (kv_search, set_kv_search) = create_signal(String::new());
    let (kv_show_add_form, set_kv_show_add_form) = create_signal(false);
    let (kv_new_key, set_kv_new_key) = create_signal(String::new());
    let (kv_new_value, set_kv_new_value) = create_signal(String::new());
    let (kv_new_ttl, set_kv_new_ttl) = create_signal(String::new());
    let (kv_edit_key, set_kv_edit_key) = create_signal::<Option<String>>(None);
    let (kv_edit_value, set_kv_edit_value) = create_signal(String::new());
    let (kv_message, set_kv_message) = create_signal::<Option<(String, bool)>>(None); // (message, is_success)

    // Collections Browser state
    let (col_show_new_collection, set_col_show_new_collection) = create_signal(false);
    let (col_new_name, set_col_new_name) = create_signal(String::new());
    let (col_show_new_doc, set_col_show_new_doc) = create_signal(false);
    let (col_new_doc_content, set_col_new_doc_content) = create_signal(String::new());
    let (col_selected_doc, set_col_selected_doc) = create_signal::<Option<String>>(None);
    let (col_message, set_col_message) = create_signal::<Option<(String, bool)>>(None);

    // Graph Explorer state
    let (graph_search, set_graph_search) = create_signal(String::new());
    let (graph_label_filter, set_graph_label_filter) = create_signal(String::new());
    let (graph_selected_node, set_graph_selected_node) = create_signal::<Option<String>>(None);
    let (graph_layout, set_graph_layout) = create_signal("force".to_string());

    // Query Builder state
    let (query_tab, set_query_tab) = create_signal("sql".to_string());
    let (query_history, set_query_history) = create_signal::<Vec<String>>(vec![]);

    // Metrics page state
    let (metrics_active_tab, set_metrics_active_tab) = create_signal("overview".to_string());

    // Settings page state
    let (settings_tab, set_settings_tab) = create_signal("general".to_string());
    let (show_add_user, set_show_add_user) = create_signal(false);
    let (edit_user_id, set_edit_user_id) = create_signal::<Option<String>>(None);
    let (new_user_name, set_new_user_name) = create_signal(String::new());
    let (new_user_email, set_new_user_email) = create_signal(String::new());
    let (new_user_role, set_new_user_role) = create_signal("viewer".to_string());
    let (new_user_2fa, set_new_user_2fa) = create_signal(false);
    let (settings_message, set_settings_message) = create_signal::<Option<(String, bool)>>(None);
    let (show_add_role, set_show_add_role) = create_signal(false);
    let (new_role_name, set_new_role_name) = create_signal(String::new());

    // General Settings state
    let (replication_factor, set_replication_factor) = create_signal(3);
    let (auto_backups_enabled, set_auto_backups_enabled) = create_signal(true);
    let (backup_schedule, set_backup_schedule) = create_signal("6h".to_string());
    let (retention_period, set_retention_period) = create_signal("30d".to_string());

    // Security Settings state
    let (tls_enabled, set_tls_enabled) = create_signal(true);
    let (auth_required, set_auth_required) = create_signal(true);
    let (session_timeout, set_session_timeout) = create_signal("30m".to_string());
    let (audit_logging_enabled, set_audit_logging_enabled) = create_signal(true);
    let (require_2fa, set_require_2fa) = create_signal(false);
    let (totp_enabled, set_totp_enabled) = create_signal(true);
    let (sms_enabled, set_sms_enabled) = create_signal(true);
    let (webauthn_enabled, set_webauthn_enabled) = create_signal(false);
    let (recovery_codes_enabled, set_recovery_codes_enabled) = create_signal(true);

    // Danger zone state
    let (show_reset_confirm, set_show_reset_confirm) = create_signal(false);
    let (reset_confirm_text, set_reset_confirm_text) = create_signal(String::new());

    // Sample users data (in real app, this would come from API)
    let (users_list, set_users_list) = create_signal(vec![
        ("user-1".to_string(), "Admin User".to_string(), "admin@aegis.db".to_string(), "admin".to_string(), true),
        ("user-2".to_string(), "John Developer".to_string(), "john@company.com".to_string(), "developer".to_string(), true),
        ("user-3".to_string(), "Jane Analyst".to_string(), "jane@company.com".to_string(), "analyst".to_string(), false),
        ("user-4".to_string(), "Demo User".to_string(), "demo@aegis.db".to_string(), "viewer".to_string(), false),
    ]);

    // Sample roles data
    let (roles_list, set_roles_list) = create_signal(vec![
        ("admin".to_string(), "Full access to all features".to_string(), vec!["*".to_string()]),
        ("developer".to_string(), "Read/write access to data".to_string(), vec!["data:read".to_string(), "data:write".to_string(), "query:execute".to_string()]),
        ("analyst".to_string(), "Read-only access to data and metrics".to_string(), vec!["data:read".to_string(), "metrics:read".to_string()]),
        ("viewer".to_string(), "View-only dashboard access".to_string(), vec!["dashboard:view".to_string()]),
    ]);

    // Load data from Aegis server
    let load_data = create_action(move |_: &()| async move {
        set_loading.set(true);

        if let Ok(s) = api::get_cluster_status().await { set_cluster_status.set(Some(s)); }
        if let Ok(n) = api::get_nodes().await { set_nodes.set(n); }
        if let Ok(s) = api::get_database_stats().await { set_db_stats.set(Some(s)); }
        if let Ok(a) = api::get_alerts().await { set_alerts.set(a); }

        set_loading.set(false);
    });

    // Load KV entries
    let load_kv_entries = create_action(move |_: &()| async move {
        set_modal_loading.set(true);
        if let Ok(entries) = api::list_keys().await {
            set_kv_entries.set(entries);
        }
        set_modal_loading.set(false);
    });

    // Load collections
    let load_collections = create_action(move |_: &()| async move {
        set_modal_loading.set(true);
        if let Ok(cols) = api::list_collections().await {
            set_collections.set(cols);
        }
        set_modal_loading.set(false);
    });

    // Load collection documents
    let load_collection_docs = create_action(move |name: &String| {
        let name = name.clone();
        async move {
            set_modal_loading.set(true);
            if let Ok(docs) = api::get_collection_documents(&name).await {
                set_collection_docs.set(docs);
            }
            set_modal_loading.set(false);
        }
    });

    // Load graph data
    let load_graph = create_action(move |_: &()| async move {
        set_modal_loading.set(true);
        if let Ok(data) = api::get_graph_data().await {
            set_graph_data.set(Some(data));
        }
        set_modal_loading.set(false);
    });

    // Execute query
    let execute_query = create_action(move |query: &String| {
        let query = query.clone();
        async move {
            set_modal_loading.set(true);
            if let Ok(result) = api::execute_builder_query(&query, "sql").await {
                set_query_result.set(Some(result));
            }
            set_modal_loading.set(false);
        }
    });

    // Node actions
    let restart_node_action = create_action(move |node_id: &String| {
        let node_id = node_id.clone();
        async move {
            set_modal_loading.set(true);
            match api::restart_node(&node_id).await {
                Ok(resp) => set_node_action_message.set(Some(resp.message)),
                Err(e) => set_node_action_message.set(Some(format!("Error: {}", e))),
            }
            set_modal_loading.set(false);
        }
    });

    let drain_node_action = create_action(move |node_id: &String| {
        let node_id = node_id.clone();
        async move {
            set_modal_loading.set(true);
            match api::drain_node(&node_id).await {
                Ok(resp) => set_node_action_message.set(Some(resp.message)),
                Err(e) => set_node_action_message.set(Some(format!("Error: {}", e))),
            }
            set_modal_loading.set(false);
        }
    });

    let remove_node_action = create_action(move |node_id: &String| {
        let node_id = node_id.clone();
        async move {
            set_modal_loading.set(true);
            match api::remove_node(&node_id).await {
                Ok(resp) => {
                    set_node_action_message.set(Some(resp.message));
                    // Close modal after removal
                    set_selected_node.set(None);
                    set_active_modal.set(BrowserModal::None);
                }
                Err(e) => set_node_action_message.set(Some(format!("Error: {}", e))),
            }
            set_modal_loading.set(false);
        }
    });

    let load_node_logs = create_action(move |node_id: &String| {
        let node_id = node_id.clone();
        async move {
            set_modal_loading.set(true);
            match api::get_node_logs(&node_id, Some(100)).await {
                Ok(resp) => {
                    set_node_logs.set(resp.logs);
                    set_show_logs_view.set(true);
                }
                Err(e) => set_node_action_message.set(Some(format!("Error loading logs: {}", e))),
            }
            set_modal_loading.set(false);
        }
    });

    // KV Browser actions
    let add_kv_entry = create_action(move |_: &()| async move {
        let key = kv_new_key.get();
        let value_str = kv_new_value.get();
        let ttl_str = kv_new_ttl.get();

        if key.is_empty() {
            set_kv_message.set(Some(("Key cannot be empty".to_string(), false)));
            return;
        }

        let value: serde_json::Value = serde_json::from_str(&value_str)
            .unwrap_or(serde_json::Value::String(value_str.clone()));
        let ttl: Option<u64> = ttl_str.parse().ok();

        set_modal_loading.set(true);
        match api::set_key(&key, value, ttl).await {
            Ok(_) => {
                set_kv_message.set(Some((format!("Key '{}' added successfully", key), true)));
                set_kv_new_key.set(String::new());
                set_kv_new_value.set(String::new());
                set_kv_new_ttl.set(String::new());
                set_kv_show_add_form.set(false);
                // Reload entries
                if let Ok(entries) = api::list_keys().await {
                    set_kv_entries.set(entries);
                }
            }
            Err(e) => set_kv_message.set(Some((format!("Error: {}", e), false))),
        }
        set_modal_loading.set(false);
    });

    let delete_kv_entry = create_action(move |key: &String| {
        let key = key.clone();
        async move {
            set_modal_loading.set(true);
            match api::delete_key(&key).await {
                Ok(_) => {
                    set_kv_message.set(Some((format!("Key '{}' deleted", key), true)));
                    // Reload entries
                    if let Ok(entries) = api::list_keys().await {
                        set_kv_entries.set(entries);
                    }
                }
                Err(e) => set_kv_message.set(Some((format!("Error: {}", e), false))),
            }
            set_modal_loading.set(false);
        }
    });

    let update_kv_entry = create_action(move |_: &()| async move {
        if let Some(key) = kv_edit_key.get() {
            let value_str = kv_edit_value.get();
            let value: serde_json::Value = serde_json::from_str(&value_str)
                .unwrap_or(serde_json::Value::String(value_str.clone()));

            set_modal_loading.set(true);
            match api::set_key(&key, value, None).await {
                Ok(_) => {
                    set_kv_message.set(Some((format!("Key '{}' updated", key), true)));
                    set_kv_edit_key.set(None);
                    set_kv_edit_value.set(String::new());
                    // Reload entries
                    if let Ok(entries) = api::list_keys().await {
                        set_kv_entries.set(entries);
                    }
                }
                Err(e) => set_kv_message.set(Some((format!("Error: {}", e), false))),
            }
            set_modal_loading.set(false);
        }
    });

    // Initial load
    create_effect(move |_| {
        load_data.dispatch(());
    });

    // Logout handler
    let handle_logout = move |_| {
        app_state_logout.logout();
        nav_logout("/", Default::default());
    };

    view! {
        <div class="dashboard-container">
            // Sidebar
            <aside class="sidebar">
                <div class="sidebar-header">
                    <svg class="sidebar-logo" width="28" height="28" viewBox="0 0 100 100">
                        <defs>
                            <linearGradient id="sidebarGrad" x1="0%" y1="0%" x2="100%" y2="100%">
                                <stop offset="0%" style="stop-color:#14b8a6"/>
                                <stop offset="100%" style="stop-color:#0d9488"/>
                            </linearGradient>
                        </defs>
                        <rect width="100" height="100" rx="20" fill="url(#sidebarGrad)"/>
                        <path d="M30 35 L50 25 L70 35 L70 65 L50 75 L30 65 Z" fill="none" stroke="white" stroke-width="4" stroke-linejoin="round"/>
                        <circle cx="50" cy="50" r="6" fill="white"/>
                    </svg>
                    <span class="sidebar-title">"Aegis DB"</span>
                </div>

                <nav class="sidebar-nav">
                    <div class="nav-section">
                        <div class="nav-section-title">"Dashboard"</div>
                        <button
                            class=move || if current_page.get() == DashboardPage::Overview { "nav-item active" } else { "nav-item" }
                            on:click=move |_| set_current_page.set(DashboardPage::Overview)
                        >
                            <span>"Overview"</span>
                        </button>
                        <button
                            class=move || if current_page.get() == DashboardPage::Nodes { "nav-item active" } else { "nav-item" }
                            on:click=move |_| set_current_page.set(DashboardPage::Nodes)
                        >
                            <span>"Nodes"</span>
                        </button>
                        <button
                            class=move || if current_page.get() == DashboardPage::Database { "nav-item active" } else { "nav-item" }
                            on:click=move |_| set_current_page.set(DashboardPage::Database)
                        >
                            <span>"Database"</span>
                        </button>
                    </div>

                    <div class="nav-section">
                        <div class="nav-section-title">"Monitoring"</div>
                        <button
                            class=move || if current_page.get() == DashboardPage::Metrics { "nav-item active" } else { "nav-item" }
                            on:click=move |_| set_current_page.set(DashboardPage::Metrics)
                        >
                            <span>"Metrics"</span>
                        </button>
                    </div>

                    <div class="nav-section">
                        <div class="nav-section-title">"System"</div>
                        <button
                            class=move || if current_page.get() == DashboardPage::Settings { "nav-item active" } else { "nav-item" }
                            on:click=move |_| set_current_page.set(DashboardPage::Settings)
                        >
                            <span>"Settings"</span>
                        </button>
                    </div>
                </nav>

                <div class="sidebar-footer">
                    <div class="user-info" on:click=handle_logout>
                        {move || {
                            let u = app_state_view.user.get();
                            let username = u.as_ref().map(|u| u.username.clone()).unwrap_or_else(|| "User".to_string());
                            let role = u.as_ref().map(|u| u.role.to_string()).unwrap_or_else(|| "viewer".to_string());
                            let initial = username.chars().next().unwrap_or('U').to_uppercase().to_string();

                            view! {
                                <div class="user-avatar">{initial}</div>
                                <div class="user-details">
                                    <div class="user-name">{username}</div>
                                    <div class="user-role">{role}</div>
                                </div>
                            }
                        }}
                    </div>
                </div>
            </aside>

            // Main Content
            <main class="main-content">
                <div class="dashboard-header">
                    <h1 class="page-title">
                        {move || match current_page.get() {
                            DashboardPage::Overview => "Cluster Overview",
                            DashboardPage::Nodes => "Node Management",
                            DashboardPage::Database => "Database",
                            DashboardPage::Metrics => "Metrics",
                            DashboardPage::Settings => "Settings",
                        }}
                    </h1>
                    <div class="header-actions">
                        <button class="header-btn header-btn-secondary" on:click=move |_| load_data.dispatch(())>
                            "Refresh"
                        </button>
                    </div>
                </div>

                // Loading State
                <Show when=move || loading.get()>
                    <div class="loading-container">
                        <div class="spinner large"></div>
                        <p>"Loading cluster data..."</p>
                    </div>
                </Show>

                // Overview Page
                <Show when=move || !loading.get() && current_page.get() == DashboardPage::Overview>
                    // Stats Grid
                    <div class="stats-grid">
                        <div class="stat-card">
                            <div class="stat-header">
                                <div class="stat-icon teal">"N"</div>
                                <span class="stat-change positive">"All Healthy"</span>
                            </div>
                            <div class="stat-value">
                                {move || cluster_status.get().map(|s| format!("{}/{}", s.healthy_nodes, s.total_nodes)).unwrap_or_else(|| "-".to_string())}
                            </div>
                            <div class="stat-label">"Active Nodes"</div>
                        </div>

                        <div class="stat-card">
                            <div class="stat-header">
                                <div class="stat-icon terracotta">"O"</div>
                                <span class="stat-change positive">"+12%"</span>
                            </div>
                            <div class="stat-value">
                                {move || db_stats.get().map(|s| format_number(s.ops_last_minute)).unwrap_or_else(|| "-".to_string())}
                            </div>
                            <div class="stat-label">"Ops/min"</div>
                        </div>

                        <div class="stat-card">
                            <div class="stat-header">
                                <div class="stat-icon success">"S"</div>
                            </div>
                            <div class="stat-value">
                                {move || db_stats.get().map(|s| format_bytes(s.storage_used)).unwrap_or_else(|| "-".to_string())}
                            </div>
                            <div class="stat-label">
                                {move || db_stats.get().map(|s| format!("Storage Used ({}%)", (s.storage_used as f64 / s.storage_total as f64 * 100.0) as u32)).unwrap_or_else(|| "Storage Used".to_string())}
                            </div>
                        </div>

                        <div class="stat-card">
                            <div class="stat-header">
                                <div class="stat-icon info">"C"</div>
                            </div>
                            <div class="stat-value">
                                {move || db_stats.get().map(|s| format!("{:.1}%", s.cache_hit_rate)).unwrap_or_else(|| "-".to_string())}
                            </div>
                            <div class="stat-label">"Cache Hit Rate"</div>
                        </div>
                    </div>

                    // Dashboard Grid
                    <div class="dashboard-grid">
                        // Nodes Card
                        <div class="card">
                            <div class="card-header">
                                <span class="card-title">"Cluster Nodes"</span>
                            </div>
                            <div class="card-body">
                                <div class="nodes-list">
                                    {move || nodes.get().into_iter().map(|node| {
                                        let status_class = match node.status {
                                            NodeStatus::Healthy => "healthy",
                                            NodeStatus::Degraded => "degraded",
                                            NodeStatus::Offline => "offline",
                                        };
                                        let role_class = match node.role {
                                            NodeRole::Leader => "leader",
                                            _ => "follower",
                                        };
                                        let role_text = match node.role {
                                            NodeRole::Leader => "leader",
                                            NodeRole::Follower => "follower",
                                            NodeRole::Candidate => "candidate",
                                        };
                                        let node_for_click = node.clone();

                                        view! {
                                            <div
                                                class="node-item clickable"
                                                on:click=move |_| {
                                                    set_selected_node.set(Some(node_for_click.clone()));
                                                    set_active_modal.set(BrowserModal::NodeDetail);
                                                }
                                            >
                                                <div class=format!("node-status {}", status_class)></div>
                                                <div class="node-info">
                                                    <div class="node-name">
                                                        {node.id.clone()}
                                                        <span class=format!("node-role {}", role_class)>
                                                            {role_text}
                                                        </span>
                                                    </div>
                                                    <div class="node-address">{node.address.clone()}</div>
                                                </div>
                                                <div class="node-metrics">
                                                    <div class="metric">
                                                        <div class="metric-value">{format!("{}%", node.metrics.cpu_usage as u32)}</div>
                                                        <div class="metric-label">"CPU"</div>
                                                    </div>
                                                    <div class="metric">
                                                        <div class="metric-value">{format!("{}%", node.metrics.memory_usage as u32)}</div>
                                                        <div class="metric-label">"MEM"</div>
                                                    </div>
                                                </div>
                                                <div class="node-action-hint">"Click for details"</div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>
                        </div>

                        // Right Column
                        <div class="right-column">
                            // Alerts Card
                            <div class="card">
                                <div class="card-header">
                                    <span class="card-title">"Alerts"</span>
                                </div>
                                <div class="card-body">
                                    {move || {
                                        let alert_list = alerts.get();
                                        if alert_list.is_empty() {
                                            view! {
                                                <div class="empty-state">
                                                    <p>"No active alerts"</p>
                                                </div>
                                            }.into_view()
                                        } else {
                                            view! {
                                                <div class="alerts-list">
                                                    {alert_list.into_iter().take(3).map(|alert| {
                                                        let sev = match alert.severity {
                                                            AlertSeverity::Critical => "critical",
                                                            AlertSeverity::Warning => "warning",
                                                            AlertSeverity::Info => "info",
                                                        };
                                                        view! {
                                                            <div class=format!("alert-item {}", sev)>
                                                                <div class="alert-content">
                                                                    <div class="alert-message">{alert.message.clone()}</div>
                                                                    <div class="alert-source">{alert.source.clone()}</div>
                                                                </div>
                                                            </div>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            }.into_view()
                                        }
                                    }}
                                </div>
                            </div>

                            // Cluster Info Card
                            <div class="card">
                                <div class="card-header">
                                    <span class="card-title">"Cluster Info"</span>
                                </div>
                                <div class="card-body">
                                    <div class="cluster-info">
                                        {move || cluster_status.get().map(|s| view! {
                                            <div class="info-row">
                                                <span class="info-label">"Cluster Name"</span>
                                                <span class="info-value">{s.name}</span>
                                            </div>
                                            <div class="info-row">
                                                <span class="info-label">"Version"</span>
                                                <span class="info-value">{s.version}</span>
                                            </div>
                                            <div class="info-row">
                                                <span class="info-label">"Leader"</span>
                                                <span class="info-value leader">{s.leader_id}</span>
                                            </div>
                                            <div class="info-row">
                                                <span class="info-label">"Raft Term"</span>
                                                <span class="info-value">{s.term}</span>
                                            </div>
                                            <div class="info-row">
                                                <span class="info-label">"Shards"</span>
                                                <span class="info-value">{s.shard_count}</span>
                                            </div>
                                        })}
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </Show>

                // Nodes Page
                <Show when=move || !loading.get() && current_page.get() == DashboardPage::Nodes>
                    <div class="nodes-page">
                        <div class="page-actions">
                            <button class="header-btn header-btn-primary">"Add Node"</button>
                        </div>

                        <div class="nodes-table-container">
                            <table class="data-table">
                                <thead>
                                    <tr>
                                        <th>"Status"</th>
                                        <th>"Node ID"</th>
                                        <th>"Address"</th>
                                        <th>"Role"</th>
                                        <th>"Region"</th>
                                        <th>"CPU"</th>
                                        <th>"Memory"</th>
                                        <th>"Disk"</th>
                                        <th>"Ops/s"</th>
                                        <th>"Latency (p99)"</th>
                                        <th>"Actions"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {move || nodes.get().into_iter().map(|node| {
                                        let status_class = match node.status {
                                            NodeStatus::Healthy => "healthy",
                                            NodeStatus::Degraded => "degraded",
                                            NodeStatus::Offline => "offline",
                                        };
                                        let role_text = match node.role {
                                            NodeRole::Leader => "Leader",
                                            NodeRole::Follower => "Follower",
                                            NodeRole::Candidate => "Candidate",
                                        };
                                        let _uptime_days = node.uptime / 86400;
                                        let node_for_row_click = node.clone();
                                        let node_for_details_btn = node.clone();
                                        let node_id_for_restart = node.id.clone();
                                        let node_id_for_remove = node.id.clone();

                                        view! {
                                            <tr class="clickable-row" on:click=move |_| {
                                                set_selected_node.set(Some(node_for_row_click.clone()));
                                                set_active_modal.set(BrowserModal::NodeDetail);
                                            }>
                                                <td><span class=format!("status-badge {}", status_class)>{format!("{:?}", node.status)}</span></td>
                                                <td class="monospace">{node.id.clone()}</td>
                                                <td class="monospace">{node.address.clone()}</td>
                                                <td><span class=format!("role-badge {}", status_class)>{role_text}</span></td>
                                                <td>{node.region.clone()}</td>
                                                <td>
                                                    <div class="progress-cell">
                                                        <div class="progress-bar">
                                                            <div class="progress-fill" style=format!("width: {}%", node.metrics.cpu_usage as u32)></div>
                                                        </div>
                                                        <span>{format!("{}%", node.metrics.cpu_usage as u32)}</span>
                                                    </div>
                                                </td>
                                                <td>
                                                    <div class="progress-cell">
                                                        <div class="progress-bar">
                                                            <div class="progress-fill" style=format!("width: {}%", node.metrics.memory_usage as u32)></div>
                                                        </div>
                                                        <span>{format!("{}%", node.metrics.memory_usage as u32)}</span>
                                                    </div>
                                                </td>
                                                <td>
                                                    <div class="progress-cell">
                                                        <div class="progress-bar">
                                                            <div class="progress-fill" style=format!("width: {}%", node.metrics.disk_usage as u32)></div>
                                                        </div>
                                                        <span>{format!("{}%", node.metrics.disk_usage as u32)}</span>
                                                    </div>
                                                </td>
                                                <td>{format_number(node.metrics.ops_per_second)}</td>
                                                <td>{format!("{:.1}ms", node.metrics.latency_p99)}</td>
                                                <td>
                                                    <div class="action-buttons">
                                                        <button
                                                            class="btn-icon btn-details"
                                                            title="View Details"
                                                            on:click=move |e| {
                                                                e.stop_propagation();
                                                                set_selected_node.set(Some(node_for_details_btn.clone()));
                                                                set_active_modal.set(BrowserModal::NodeDetail);
                                                            }
                                                        >"Details"</button>
                                                        <button
                                                            class="btn-icon btn-restart"
                                                            title="Restart Node"
                                                            on:click=move |e| {
                                                                e.stop_propagation();
                                                                web_sys::window().unwrap().alert_with_message(&format!("Restarting node: {}", node_id_for_restart)).ok();
                                                            }
                                                        >"Restart"</button>
                                                        <button
                                                            class="btn-icon btn-remove danger"
                                                            title="Remove Node"
                                                            on:click=move |e| {
                                                                e.stop_propagation();
                                                                if web_sys::window().unwrap().confirm_with_message(&format!("Remove node {}?", node_id_for_remove)).unwrap_or(false) {
                                                                    web_sys::window().unwrap().alert_with_message("Node removal requested").ok();
                                                                }
                                                            }
                                                        >"Remove"</button>
                                                    </div>
                                                </td>
                                            </tr>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </table>
                        </div>

                        <div class="nodes-summary">
                            <div class="summary-card">
                                <div class="summary-title">"Total Nodes"</div>
                                <div class="summary-value">{move || nodes.get().len()}</div>
                            </div>
                            <div class="summary-card">
                                <div class="summary-title">"Healthy"</div>
                                <div class="summary-value success">{move || nodes.get().iter().filter(|n| matches!(n.status, NodeStatus::Healthy)).count()}</div>
                            </div>
                            <div class="summary-card">
                                <div class="summary-title">"Total Connections"</div>
                                <div class="summary-value">{move || format_number(nodes.get().iter().map(|n| n.metrics.connections).sum())}</div>
                            </div>
                            <div class="summary-card">
                                <div class="summary-title">"Avg Latency (p99)"</div>
                                <div class="summary-value">{move || {
                                    let nodes_list = nodes.get();
                                    if nodes_list.is_empty() { "N/A".to_string() }
                                    else {
                                        let avg = nodes_list.iter().map(|n| n.metrics.latency_p99).sum::<f64>() / nodes_list.len() as f64;
                                        format!("{:.1}ms", avg)
                                    }
                                }}</div>
                            </div>
                        </div>
                    </div>
                </Show>

                // Database Page
                <Show when=move || !loading.get() && current_page.get() == DashboardPage::Database>
                    <div class="database-page">
                        // Database Stats Cards
                        <div class="db-stats-grid">
                            <div class="db-stat-card">
                                <div class="db-stat-icon">"🔑"</div>
                                <div class="db-stat-content">
                                    <div class="db-stat-value">{move || db_stats.get().map(|s| format_number(s.total_keys)).unwrap_or("-".to_string())}</div>
                                    <div class="db-stat-label">"Total Keys"</div>
                                </div>
                            </div>
                            <div class="db-stat-card">
                                <div class="db-stat-icon">"📄"</div>
                                <div class="db-stat-content">
                                    <div class="db-stat-value">{move || db_stats.get().map(|s| format_number(s.total_documents)).unwrap_or("-".to_string())}</div>
                                    <div class="db-stat-label">"Documents"</div>
                                </div>
                            </div>
                            <div class="db-stat-card">
                                <div class="db-stat-icon">"🔗"</div>
                                <div class="db-stat-content">
                                    <div class="db-stat-value">{move || db_stats.get().map(|s| format_number(s.total_graph_nodes)).unwrap_or("-".to_string())}</div>
                                    <div class="db-stat-label">"Graph Nodes"</div>
                                </div>
                            </div>
                            <div class="db-stat-card">
                                <div class="db-stat-icon">"↔"</div>
                                <div class="db-stat-content">
                                    <div class="db-stat-value">{move || db_stats.get().map(|s| format_number(s.total_graph_edges)).unwrap_or("-".to_string())}</div>
                                    <div class="db-stat-label">"Graph Edges"</div>
                                </div>
                            </div>
                        </div>

                        // Storage Section
                        <div class="card">
                            <div class="card-header">
                                <span class="card-title">"Storage"</span>
                            </div>
                            <div class="card-body">
                                {move || db_stats.get().map(|stats| {
                                    let used_pct = (stats.storage_used as f64 / stats.storage_total as f64 * 100.0) as u32;
                                    view! {
                                        <div class="storage-info">
                                            <div class="storage-bar-container">
                                                <div class="storage-bar">
                                                    <div class="storage-fill" style=format!("width: {}%", used_pct)></div>
                                                </div>
                                                <div class="storage-labels">
                                                    <span>{format!("{} used", format_bytes(stats.storage_used))}</span>
                                                    <span>{format!("{} total", format_bytes(stats.storage_total))}</span>
                                                </div>
                                            </div>
                                            <div class="storage-details">
                                                <div class="storage-detail">
                                                    <span class="detail-label">"Cache Hit Rate"</span>
                                                    <span class="detail-value">{format!("{:.1}%", stats.cache_hit_rate)}</span>
                                                </div>
                                                <div class="storage-detail">
                                                    <span class="detail-label">"Operations/min"</span>
                                                    <span class="detail-value">{format_number(stats.ops_last_minute)}</span>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                })}
                            </div>
                        </div>

                        // Data Paradigms
                        <div class="paradigms-grid">
                            <div class="paradigm-card">
                                <div class="paradigm-header">
                                    <span class="paradigm-icon">"🔑"</span>
                                    <span class="paradigm-name">"Key-Value Store"</span>
                                </div>
                                <div class="paradigm-stats">
                                    <div class="paradigm-stat">
                                        <span class="stat-value">{move || db_stats.get().map(|s| format_number(s.total_keys)).unwrap_or("-".to_string())}</span>
                                        <span class="stat-label">"Keys"</span>
                                    </div>
                                    <div class="paradigm-stat">
                                        <span class="stat-value">"< 1ms"</span>
                                        <span class="stat-label">"Avg Latency"</span>
                                    </div>
                                </div>
                                <button class="paradigm-action" on:click=move |_| {
                                    set_active_modal.set(BrowserModal::KeyValue);
                                    load_kv_entries.dispatch(());
                                }>"Browse Keys"</button>
                            </div>

                            <div class="paradigm-card">
                                <div class="paradigm-header">
                                    <span class="paradigm-icon">"📄"</span>
                                    <span class="paradigm-name">"Document Store"</span>
                                </div>
                                <div class="paradigm-stats">
                                    <div class="paradigm-stat">
                                        <span class="stat-value">{move || db_stats.get().map(|s| format_number(s.total_documents)).unwrap_or("-".to_string())}</span>
                                        <span class="stat-label">"Documents"</span>
                                    </div>
                                    <div class="paradigm-stat">
                                        <span class="stat-value">"12"</span>
                                        <span class="stat-label">"Collections"</span>
                                    </div>
                                </div>
                                <button class="paradigm-action" on:click=move |_| {
                                    set_active_modal.set(BrowserModal::Collections);
                                    load_collections.dispatch(());
                                }>"Browse Collections"</button>
                            </div>

                            <div class="paradigm-card">
                                <div class="paradigm-header">
                                    <span class="paradigm-icon">"🔗"</span>
                                    <span class="paradigm-name">"Graph Database"</span>
                                </div>
                                <div class="paradigm-stats">
                                    <div class="paradigm-stat">
                                        <span class="stat-value">{move || db_stats.get().map(|s| format_number(s.total_graph_nodes)).unwrap_or("-".to_string())}</span>
                                        <span class="stat-label">"Nodes"</span>
                                    </div>
                                    <div class="paradigm-stat">
                                        <span class="stat-value">{move || db_stats.get().map(|s| format_number(s.total_graph_edges)).unwrap_or("-".to_string())}</span>
                                        <span class="stat-label">"Edges"</span>
                                    </div>
                                </div>
                                <button class="paradigm-action" on:click=move |_| {
                                    set_active_modal.set(BrowserModal::Graph);
                                    load_graph.dispatch(());
                                }>"Graph Explorer"</button>
                            </div>

                            <div class="paradigm-card">
                                <div class="paradigm-header">
                                    <span class="paradigm-icon">"📈"</span>
                                    <span class="paradigm-name">"Time Series"</span>
                                </div>
                                <div class="paradigm-stats">
                                    <div class="paradigm-stat">
                                        <span class="stat-value">"24"</span>
                                        <span class="stat-label">"Metrics"</span>
                                    </div>
                                    <div class="paradigm-stat">
                                        <span class="stat-value">"30d"</span>
                                        <span class="stat-label">"Retention"</span>
                                    </div>
                                </div>
                                <button class="paradigm-action" on:click=move |_| {
                                    set_active_modal.set(BrowserModal::QueryBuilder);
                                    set_query_result.set(None);
                                }>"Query Builder"</button>
                            </div>
                        </div>
                    </div>
                </Show>

                // Metrics Page - Enhanced
                <Show when=move || !loading.get() && current_page.get() == DashboardPage::Metrics>
                    <div class="metrics-page-v2">
                        // Top Controls Bar
                        <div class="metrics-top-bar">
                            <div class="metrics-tabs">
                                <button
                                    class=move || if metrics_active_tab.get() == "overview" { "metrics-tab active" } else { "metrics-tab" }
                                    on:click=move |_| {
                                        log::info!("Overview tab clicked");
                                        set_metrics_active_tab.set("overview".to_string());
                                    }
                                >"Overview"</button>
                                <button
                                    class=move || if metrics_active_tab.get() == "performance" { "metrics-tab active" } else { "metrics-tab" }
                                    on:click=move |_| {
                                        log::info!("Performance tab clicked");
                                        set_metrics_active_tab.set("performance".to_string());
                                    }
                                >"Performance"</button>
                                <button
                                    class=move || if metrics_active_tab.get() == "storage" { "metrics-tab active" } else { "metrics-tab" }
                                    on:click=move |_| {
                                        log::info!("Storage tab clicked");
                                        set_metrics_active_tab.set("storage".to_string());
                                    }
                                >"Storage"</button>
                                <button
                                    class=move || if metrics_active_tab.get() == "network" { "metrics-tab active" } else { "metrics-tab" }
                                    on:click=move |_| {
                                        log::info!("Network tab clicked");
                                        set_metrics_active_tab.set("network".to_string());
                                    }
                                >"Network"</button>
                            </div>
                            <div class="metrics-actions">
                                <div class="time-range-selector-v2">
                                    {["1h", "6h", "24h", "7d", "30d"].into_iter().map(|range| {
                                        let r = range.to_string();
                                        let r2 = range.to_string();
                                        view! {
                                            <button
                                                class=move || if metrics_time_range.get() == r { "time-btn active" } else { "time-btn" }
                                                on:click=move |_| set_metrics_time_range.set(r2.clone())
                                            >{range}</button>
                                        }
                                    }).collect_view()}
                                </div>
                                <button
                                    class="metrics-export-btn"
                                    on:click=move |_| {
                                        // Generate metrics report
                                        let time_range = metrics_time_range.get();
                                        let stats = db_stats.get();
                                        let cluster = cluster_status.get();
                                        let nodes_list = nodes.get();

                                        let report = format!(
                                            r#"{{
  "report_type": "Aegis DB Metrics Report",
  "generated_at": "{}",
  "time_range": "{}",
  "cluster": {{
    "name": "{}",
    "status": "{}",
    "nodes": {}
  }},
  "operations": {{
    "ops_per_second": {},
    "reads_total": {},
    "writes_total": {}
  }},
  "performance": {{
    "cache_hit_rate": {},
    "avg_latency_ms": {}
  }},
  "storage": {{
    "used_bytes": {},
    "total_bytes": {}
  }}
}}"#,
                                            "2024-01-15T12:00:00Z",
                                            time_range,
                                            cluster.as_ref().map(|c| c.name.as_str()).unwrap_or("unknown"),
                                            cluster.as_ref().map(|c| if c.healthy_nodes == c.total_nodes { "healthy" } else { "degraded" }).unwrap_or("unknown"),
                                            nodes_list.len(),
                                            stats.as_ref().map(|s| s.ops_last_minute / 60).unwrap_or(0),
                                            stats.as_ref().map(|s| s.total_keys).unwrap_or(0),
                                            stats.as_ref().map(|s| s.total_documents).unwrap_or(0),
                                            stats.as_ref().map(|s| s.cache_hit_rate).unwrap_or(0.0),
                                            nodes_list.iter().map(|n| n.metrics.latency_p99).sum::<f64>() / nodes_list.len().max(1) as f64,
                                            stats.as_ref().map(|s| s.storage_used).unwrap_or(0),
                                            stats.as_ref().map(|s| s.storage_total).unwrap_or(0)
                                        );

                                        // Create and download the file
                                        if let Some(window) = web_sys::window() {
                                            if let Some(document) = window.document() {
                                                let blob = web_sys::Blob::new_with_str_sequence(
                                                    &js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(&report))
                                                ).ok();

                                                if let Some(blob) = blob {
                                                    if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                                                        if let Ok(a) = document.create_element("a") {
                                                            let _ = a.set_attribute("href", &url);
                                                            let _ = a.set_attribute("download", &format!("aegis-metrics-{}.json", time_range));
                                                            if let Some(body) = document.body() {
                                                                let _ = body.append_child(&a);
                                                                if let Some(html_a) = a.dyn_ref::<web_sys::HtmlElement>() {
                                                                    html_a.click();
                                                                }
                                                                let _ = body.remove_child(&a);
                                                            }
                                                            let _ = web_sys::Url::revoke_object_url(&url);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                >"Export Report"</button>
                            </div>
                        </div>

                        // Overview Tab Content
                        <Show when=move || metrics_active_tab.get() == "overview">
                        // Key Metrics Summary Row
                        <div class="metrics-summary-row">
                            <div class="metric-summary-card">
                                <div class="summary-icon ops">"O"</div>
                                <div class="summary-content">
                                    <div class="summary-value">{move || db_stats.get().map(|s| format_number(s.ops_last_minute / 60)).unwrap_or("-".to_string())}<span class="unit">"/s"</span></div>
                                    <div class="summary-label">"Operations"</div>
                                    <div class="summary-trend positive">"+12.4%"</div>
                                </div>
                            </div>
                            <div class="metric-summary-card">
                                <div class="summary-icon latency">"L"</div>
                                <div class="summary-content">
                                    <div class="summary-value">{move || {
                                        let nodes_list = nodes.get();
                                        if nodes_list.is_empty() { "-".to_string() }
                                        else {
                                            let avg = nodes_list.iter().map(|n| n.metrics.latency_p99).sum::<f64>() / nodes_list.len() as f64;
                                            format!("{:.1}", avg)
                                        }
                                    }}<span class="unit">"ms"</span></div>
                                    <div class="summary-label">"Avg Latency (p99)"</div>
                                    <div class="summary-trend positive">"-2.1%"</div>
                                </div>
                            </div>
                            <div class="metric-summary-card">
                                <div class="summary-icon cache">"C"</div>
                                <div class="summary-content">
                                    <div class="summary-value">{move || db_stats.get().map(|s| format!("{:.1}", s.cache_hit_rate)).unwrap_or("-".to_string())}<span class="unit">"%"</span></div>
                                    <div class="summary-label">"Cache Hit Rate"</div>
                                    <div class="summary-trend positive">"+0.8%"</div>
                                </div>
                            </div>
                            <div class="metric-summary-card">
                                <div class="summary-icon uptime">"U"</div>
                                <div class="summary-content">
                                    <div class="summary-value">"99.99"<span class="unit">"%"</span></div>
                                    <div class="summary-label">"Uptime"</div>
                                    <div class="summary-trend neutral">"30d"</div>
                                </div>
                            </div>
                        </div>

                        // Main Charts Grid
                        <div class="metrics-charts-grid">
                            // Operations Chart - Large
                            <div class="chart-card chart-large">
                                <div class="chart-header">
                                    <div class="chart-title-row">
                                        <h3>"Operations Over Time"</h3>
                                        <span class="chart-time-badge">{move || format!("Last {}", metrics_time_range.get())}</span>
                                        <div class="chart-legend">
                                            <span class="legend-dot read"></span>"Reads"
                                            <span class="legend-dot write"></span>"Writes"
                                        </div>
                                    </div>
                                    <div class="chart-value-lg">{move || db_stats.get().map(|s| format!("{}/s", format_number(s.ops_last_minute / 60))).unwrap_or("-".to_string())}</div>
                                </div>
                                <div class="chart-body">
                                    <svg class="line-chart" viewBox="0 0 600 200" preserveAspectRatio="none">
                                        // Grid lines
                                        <line x1="0" y1="50" x2="600" y2="50" class="grid-line"/>
                                        <line x1="0" y1="100" x2="600" y2="100" class="grid-line"/>
                                        <line x1="0" y1="150" x2="600" y2="150" class="grid-line"/>
                                        // Reads line (teal)
                                        <path d="M0,140 C30,130 60,110 90,100 C120,90 150,95 180,85 C210,75 240,70 270,60 C300,55 330,65 360,55 C390,50 420,40 450,35 C480,40 510,50 540,45 C570,40 600,35 600,35"
                                              class="chart-line-reads" fill="none"/>
                                        // Writes line (terracotta)
                                        <path d="M0,170 C30,165 60,155 90,150 C120,145 150,140 180,135 C210,130 240,125 270,120 C300,115 330,120 360,115 C390,110 420,105 450,100 C480,105 510,110 540,105 C570,100 600,95 600,95"
                                              class="chart-line-writes" fill="none"/>
                                        // Area fill for reads
                                        <path d="M0,140 C30,130 60,110 90,100 C120,90 150,95 180,85 C210,75 240,70 270,60 C300,55 330,65 360,55 C390,50 420,40 450,35 C480,40 510,50 540,45 C570,40 600,35 600,35 L600,200 L0,200 Z"
                                              class="chart-area-reads"/>
                                    </svg>
                                    <div class="chart-x-labels">
                                        {move || {
                                            let range = metrics_time_range.get();
                                            let labels: Vec<&str> = match range.as_str() {
                                                "1h" => vec!["-60m", "-50m", "-40m", "-30m", "-20m", "-10m", "Now"],
                                                "6h" => vec!["-6h", "-5h", "-4h", "-3h", "-2h", "-1h", "Now"],
                                                "24h" => vec!["-24h", "-20h", "-16h", "-12h", "-8h", "-4h", "Now"],
                                                "7d" => vec!["-7d", "-6d", "-5d", "-4d", "-3d", "-1d", "Now"],
                                                "30d" => vec!["-30d", "-25d", "-20d", "-15d", "-10d", "-5d", "Now"],
                                                _ => vec!["-6h", "-5h", "-4h", "-3h", "-2h", "-1h", "Now"],
                                            };
                                            labels.into_iter().map(|l| view! { <span>{l}</span> }).collect_view()
                                        }}
                                    </div>
                                </div>
                            </div>

                            // Latency Distribution
                            <div class="chart-card">
                                <div class="chart-header">
                                    <h3>"Latency Distribution"</h3>
                                </div>
                                <div class="chart-body">
                                    <div class="latency-histogram">
                                        <div class="histogram-bar" style="height: 90%">
                                            <span class="bar-label">"< 1ms"</span>
                                            <span class="bar-value">"72%"</span>
                                        </div>
                                        <div class="histogram-bar" style="height: 60%">
                                            <span class="bar-label">"1-2ms"</span>
                                            <span class="bar-value">"18%"</span>
                                        </div>
                                        <div class="histogram-bar" style="height: 30%">
                                            <span class="bar-label">"2-5ms"</span>
                                            <span class="bar-value">"7%"</span>
                                        </div>
                                        <div class="histogram-bar" style="height: 15%">
                                            <span class="bar-label">"5-10ms"</span>
                                            <span class="bar-value">"2%"</span>
                                        </div>
                                        <div class="histogram-bar warning" style="height: 8%">
                                            <span class="bar-label">"> 10ms"</span>
                                            <span class="bar-value">"1%"</span>
                                        </div>
                                    </div>
                                    <div class="latency-percentiles">
                                        <div class="percentile">
                                            <span class="p-label">"p50"</span>
                                            <span class="p-value">"0.8ms"</span>
                                        </div>
                                        <div class="percentile">
                                            <span class="p-label">"p95"</span>
                                            <span class="p-value">"2.1ms"</span>
                                        </div>
                                        <div class="percentile">
                                            <span class="p-label">"p99"</span>
                                            <span class="p-value">"4.2ms"</span>
                                        </div>
                                        <div class="percentile">
                                            <span class="p-label">"max"</span>
                                            <span class="p-value">"12.4ms"</span>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            // Cache Performance
                            <div class="chart-card">
                                <div class="chart-header">
                                    <h3>"Cache Performance"</h3>
                                </div>
                                <div class="chart-body">
                                    <div class="cache-donut">
                                        <svg viewBox="0 0 120 120">
                                            <circle cx="60" cy="60" r="50" fill="none" stroke="#374151" stroke-width="16"/>
                                            {move || {
                                                let rate = db_stats.get().map(|s| s.cache_hit_rate).unwrap_or(94.5);
                                                let circumference = 314.16;
                                                let offset = circumference * (1.0 - rate / 100.0);
                                                view! {
                                                    <circle cx="60" cy="60" r="50" fill="none" stroke="#14b8a6" stroke-width="16"
                                                        stroke-dasharray=circumference stroke-dashoffset=offset
                                                        transform="rotate(-90 60 60)" class="cache-progress"/>
                                                }
                                            }}
                                        </svg>
                                        <div class="donut-center">
                                            <span class="donut-value">{move || db_stats.get().map(|s| format!("{:.1}", s.cache_hit_rate)).unwrap_or("-".to_string())}</span>
                                            <span class="donut-unit">"%"</span>
                                        </div>
                                    </div>
                                    <div class="cache-breakdown">
                                        <div class="cache-stat">
                                            <div class="cache-stat-color hit"></div>
                                            <div class="cache-stat-info">
                                                <span class="stat-name">"Cache Hits"</span>
                                                <span class="stat-value">"2.4M"</span>
                                            </div>
                                        </div>
                                        <div class="cache-stat">
                                            <div class="cache-stat-color miss"></div>
                                            <div class="cache-stat-info">
                                                <span class="stat-name">"Cache Misses"</span>
                                                <span class="stat-value">"142K"</span>
                                            </div>
                                        </div>
                                        <div class="cache-stat">
                                            <div class="cache-stat-color evict"></div>
                                            <div class="cache-stat-info">
                                                <span class="stat-name">"Evictions"</span>
                                                <span class="stat-value">"18K"</span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            // Cluster Health Matrix
                            <div class="chart-card">
                                <div class="chart-header">
                                    <h3>"Cluster Health"</h3>
                                    <span class="health-badge healthy">"Healthy"</span>
                                </div>
                                <div class="chart-body">
                                    <div class="health-matrix">
                                        {move || nodes.get().into_iter().map(|node| {
                                            let (status_class, status_icon) = match node.status {
                                                NodeStatus::Healthy => ("healthy", "ok"),
                                                NodeStatus::Degraded => ("warning", "!"),
                                                NodeStatus::Offline => ("critical", "x"),
                                            };
                                            let role_badge = match node.role {
                                                NodeRole::Leader => Some("L"),
                                                _ => None,
                                            };
                                            view! {
                                                <div class=format!("health-node-card {}", status_class)>
                                                    <div class="node-header-row">
                                                        <span class="node-name">{node.id.clone()}</span>
                                                        {role_badge.map(|b| view! { <span class="leader-badge">{b}</span> })}
                                                    </div>
                                                    <div class="node-metrics-row">
                                                        <div class="mini-metric">
                                                            <span class="label">"CPU"</span>
                                                            <div class="mini-bar">
                                                                <div class="mini-fill" style=format!("width: {}%", node.metrics.cpu_usage as u32)></div>
                                                            </div>
                                                            <span class="value">{format!("{}%", node.metrics.cpu_usage as u32)}</span>
                                                        </div>
                                                        <div class="mini-metric">
                                                            <span class="label">"MEM"</span>
                                                            <div class="mini-bar">
                                                                <div class="mini-fill" style=format!("width: {}%", node.metrics.memory_usage as u32)></div>
                                                            </div>
                                                            <span class="value">{format!("{}%", node.metrics.memory_usage as u32)}</span>
                                                        </div>
                                                    </div>
                                                    <div class="node-status-icon">{status_icon}</div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>
                            </div>

                            // Storage Metrics
                            <div class="chart-card">
                                <div class="chart-header">
                                    <h3>"Storage Usage"</h3>
                                </div>
                                <div class="chart-body">
                                    {move || db_stats.get().map(|stats| {
                                        let used_pct = (stats.storage_used as f64 / stats.storage_total as f64 * 100.0) as u32;
                                        let wal_pct = 15; // Simulated
                                        let data_pct = used_pct - wal_pct;
                                        view! {
                                            <div class="storage-visual">
                                                <div class="storage-ring-container">
                                                    <svg viewBox="0 0 120 120">
                                                        <circle cx="60" cy="60" r="50" fill="none" stroke="#374151" stroke-width="14"/>
                                                        <circle cx="60" cy="60" r="50" fill="none" stroke="#14b8a6" stroke-width="14"
                                                            stroke-dasharray=format!("{} 314", data_pct as f64 * 3.14)
                                                            transform="rotate(-90 60 60)"/>
                                                        <circle cx="60" cy="60" r="50" fill="none" stroke="#e7a75f" stroke-width="14"
                                                            stroke-dasharray=format!("{} 314", wal_pct as f64 * 3.14)
                                                            stroke-dashoffset=format!("-{}", data_pct as f64 * 3.14)
                                                            transform="rotate(-90 60 60)"/>
                                                    </svg>
                                                    <div class="storage-center">
                                                        <span class="pct">{used_pct}"%"</span>
                                                        <span class="used">"used"</span>
                                                    </div>
                                                </div>
                                                <div class="storage-details-v2">
                                                    <div class="storage-row">
                                                        <div class="storage-color data"></div>
                                                        <span class="storage-name">"Data Files"</span>
                                                        <span class="storage-size">{format_bytes(stats.storage_used - stats.storage_used / 5)}</span>
                                                    </div>
                                                    <div class="storage-row">
                                                        <div class="storage-color wal"></div>
                                                        <span class="storage-name">"WAL"</span>
                                                        <span class="storage-size">{format_bytes(stats.storage_used / 5)}</span>
                                                    </div>
                                                    <div class="storage-row">
                                                        <div class="storage-color free"></div>
                                                        <span class="storage-name">"Available"</span>
                                                        <span class="storage-size">{format_bytes(stats.storage_total - stats.storage_used)}</span>
                                                    </div>
                                                    <div class="storage-total">
                                                        <span>"Total: "{format_bytes(stats.storage_total)}</span>
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    })}
                                </div>
                            </div>

                            // Network I/O
                            <div class="chart-card">
                                <div class="chart-header">
                                    <h3>"Network Throughput"</h3>
                                </div>
                                <div class="chart-body">
                                    <div class="network-visual">
                                        <div class="network-direction">
                                            <div class="direction-icon inbound">"IN"</div>
                                            <div class="direction-stats">
                                                <span class="direction-value">{move || {
                                                    let total: u64 = nodes.get().iter().map(|n| n.metrics.network_in).sum();
                                                    format!("{}/s", format_bytes(total / 60))
                                                }}</span>
                                                <div class="direction-sparkline">
                                                    <svg viewBox="0 0 100 30">
                                                        <path d="M0,20 Q10,18 20,15 T40,12 T60,10 T80,8 T100,5" fill="none" stroke="#14b8a6" stroke-width="2"/>
                                                    </svg>
                                                </div>
                                            </div>
                                        </div>
                                        <div class="network-direction">
                                            <div class="direction-icon outbound">"OUT"</div>
                                            <div class="direction-stats">
                                                <span class="direction-value">{move || {
                                                    let total: u64 = nodes.get().iter().map(|n| n.metrics.network_out).sum();
                                                    format!("{}/s", format_bytes(total / 60))
                                                }}</span>
                                                <div class="direction-sparkline">
                                                    <svg viewBox="0 0 100 30">
                                                        <path d="M0,25 Q10,22 20,20 T40,18 T60,15 T80,12 T100,10" fill="none" stroke="#e7a75f" stroke-width="2"/>
                                                    </svg>
                                                </div>
                                            </div>
                                        </div>
                                        <div class="network-breakdown">
                                            <div class="breakdown-item">
                                                <span class="breakdown-label">"Replication"</span>
                                                <span class="breakdown-value">"45%"</span>
                                            </div>
                                            <div class="breakdown-item">
                                                <span class="breakdown-label">"Client Queries"</span>
                                                <span class="breakdown-value">"38%"</span>
                                            </div>
                                            <div class="breakdown-item">
                                                <span class="breakdown-label">"Admin/Control"</span>
                                                <span class="breakdown-value">"17%"</span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            // Raft/Replication Status
                            <div class="chart-card">
                                <div class="chart-header">
                                    <h3>"Replication Status"</h3>
                                    <span class="sync-badge">"In Sync"</span>
                                </div>
                                <div class="chart-body">
                                    <div class="replication-visual">
                                        <div class="raft-stats">
                                            <div class="raft-stat">
                                                <span class="raft-label">"Current Term"</span>
                                                <span class="raft-value">{move || cluster_status.get().map(|s| s.term.to_string()).unwrap_or("-".to_string())}</span>
                                            </div>
                                            <div class="raft-stat">
                                                <span class="raft-label">"Commit Index"</span>
                                                <span class="raft-value">{move || cluster_status.get().map(|s| format_number(s.commit_index)).unwrap_or("-".to_string())}</span>
                                            </div>
                                            <div class="raft-stat">
                                                <span class="raft-label">"Leader"</span>
                                                <span class="raft-value leader">{move || cluster_status.get().map(|s| s.leader_id.clone()).unwrap_or("-".to_string())}</span>
                                            </div>
                                        </div>
                                        <div class="replication-lag">
                                            <h4>"Follower Lag"</h4>
                                            {move || nodes.get().into_iter().filter(|n| !matches!(n.role, NodeRole::Leader)).map(|node| {
                                                view! {
                                                    <div class="lag-row">
                                                        <span class="lag-node">{node.id.clone()}</span>
                                                        <div class="lag-bar-container">
                                                            <div class="lag-bar" style="width: 2%"></div>
                                                        </div>
                                                        <span class="lag-value">"< 1ms"</span>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                </div>
                            </div>

                            // Query Analytics
                            <div class="chart-card chart-wide">
                                <div class="chart-header">
                                    <h3>"Query Analytics"</h3>
                                    <div class="query-type-filter">
                                        <button class="filter-btn active">"All"</button>
                                        <button class="filter-btn">"SELECT"</button>
                                        <button class="filter-btn">"INSERT"</button>
                                        <button class="filter-btn">"UPDATE"</button>
                                    </div>
                                </div>
                                <div class="chart-body">
                                    <div class="query-stats-row">
                                        <div class="query-stat-box">
                                            <span class="query-stat-value">"12.4K"</span>
                                            <span class="query-stat-label">"Total Queries"</span>
                                        </div>
                                        <div class="query-stat-box">
                                            <span class="query-stat-value">"1.2ms"</span>
                                            <span class="query-stat-label">"Avg Duration"</span>
                                        </div>
                                        <div class="query-stat-box">
                                            <span class="query-stat-value">"98.2%"</span>
                                            <span class="query-stat-label">"Success Rate"</span>
                                        </div>
                                        <div class="query-stat-box">
                                            <span class="query-stat-value">"24"</span>
                                            <span class="query-stat-label">"Slow Queries"</span>
                                        </div>
                                    </div>
                                    <div class="top-queries">
                                        <h4>"Slowest Queries"</h4>
                                        <div class="query-list">
                                            <div class="query-item">
                                                <code>"SELECT * FROM orders JOIN..."</code>
                                                <span class="query-time">"45.2ms"</span>
                                                <span class="query-count">"x142"</span>
                                            </div>
                                            <div class="query-item">
                                                <code>"SELECT DISTINCT category..."</code>
                                                <span class="query-time">"23.1ms"</span>
                                                <span class="query-count">"x89"</span>
                                            </div>
                                            <div class="query-item">
                                                <code>"UPDATE inventory SET..."</code>
                                                <span class="query-time">"18.4ms"</span>
                                                <span class="query-count">"x234"</span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                        </Show>

                        // Performance Tab Content
                        <Show when=move || metrics_active_tab.get() == "performance">
                            <div class="metrics-tab-content">
                                <div class="performance-grid">
                                    // Query Performance Card
                                    <div class="perf-card large">
                                        <div class="perf-card-header">
                                            <h3>"Query Performance Analysis"</h3>
                                            <span class="perf-badge good">"Healthy"</span>
                                        </div>
                                        <div class="perf-metrics-row">
                                            <div class="perf-metric">
                                                <span class="perf-metric-value">"1.2"<span class="unit">"ms"</span></span>
                                                <span class="perf-metric-label">"Avg Query Time"</span>
                                            </div>
                                            <div class="perf-metric">
                                                <span class="perf-metric-value">"4.8"<span class="unit">"ms"</span></span>
                                                <span class="perf-metric-label">"p95 Latency"</span>
                                            </div>
                                            <div class="perf-metric">
                                                <span class="perf-metric-value">"12.4"<span class="unit">"ms"</span></span>
                                                <span class="perf-metric-label">"p99 Latency"</span>
                                            </div>
                                            <div class="perf-metric">
                                                <span class="perf-metric-value">"98.7"<span class="unit">"%"</span></span>
                                                <span class="perf-metric-label">"Success Rate"</span>
                                            </div>
                                        </div>
                                        <div class="perf-chart-placeholder">
                                            <svg class="line-chart" viewBox="0 0 600 150" preserveAspectRatio="none">
                                                <line x1="0" y1="50" x2="600" y2="50" class="grid-line"/>
                                                <line x1="0" y1="100" x2="600" y2="100" class="grid-line"/>
                                                <path d="M0,100 C50,95 100,90 150,85 C200,80 250,75 300,70 C350,65 400,68 450,72 C500,76 550,74 600,70" class="chart-line-reads" fill="none"/>
                                            </svg>
                                        </div>
                                    </div>

                                    // CPU Performance
                                    <div class="perf-card">
                                        <div class="perf-card-header">
                                            <h3>"CPU Utilization"</h3>
                                        </div>
                                        <div class="cpu-cores-grid">
                                            {(0..8).map(|i| {
                                                let usage = 30 + (i * 7) % 50;
                                                view! {
                                                    <div class="cpu-core">
                                                        <span class="core-label">{format!("Core {}", i)}</span>
                                                        <div class="core-bar">
                                                            <div class="core-fill" style=format!("width: {}%", usage)></div>
                                                        </div>
                                                        <span class="core-value">{format!("{}%", usage)}</span>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>

                                    // Memory Performance
                                    <div class="perf-card">
                                        <div class="perf-card-header">
                                            <h3>"Memory Analysis"</h3>
                                        </div>
                                        <div class="memory-breakdown">
                                            <div class="memory-item">
                                                <span class="mem-label">"Buffer Pool"</span>
                                                <div class="mem-bar"><div class="mem-fill" style="width: 72%"></div></div>
                                                <span class="mem-value">"4.6 GB / 6.4 GB"</span>
                                            </div>
                                            <div class="memory-item">
                                                <span class="mem-label">"Query Cache"</span>
                                                <div class="mem-bar"><div class="mem-fill cache" style="width: 45%"></div></div>
                                                <span class="mem-value">"1.2 GB / 2.0 GB"</span>
                                            </div>
                                            <div class="memory-item">
                                                <span class="mem-label">"Connections"</span>
                                                <div class="mem-bar"><div class="mem-fill conn" style="width: 23%"></div></div>
                                                <span class="mem-value">"384 MB / 1.6 GB"</span>
                                            </div>
                                            <div class="memory-item">
                                                <span class="mem-label">"System"</span>
                                                <div class="mem-bar"><div class="mem-fill sys" style="width: 15%"></div></div>
                                                <span class="mem-value">"256 MB"</span>
                                            </div>
                                        </div>
                                    </div>

                                    // Thread Pool
                                    <div class="perf-card">
                                        <div class="perf-card-header">
                                            <h3>"Thread Pool Status"</h3>
                                        </div>
                                        <div class="thread-pool-stats">
                                            <div class="pool-stat">
                                                <span class="pool-value">"24"</span>
                                                <span class="pool-label">"Active"</span>
                                            </div>
                                            <div class="pool-stat">
                                                <span class="pool-value">"8"</span>
                                                <span class="pool-label">"Idle"</span>
                                            </div>
                                            <div class="pool-stat">
                                                <span class="pool-value">"3"</span>
                                                <span class="pool-label">"Queued"</span>
                                            </div>
                                            <div class="pool-stat">
                                                <span class="pool-value">"32"</span>
                                                <span class="pool-label">"Max"</span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </Show>

                        // Storage Tab Content
                        <Show when=move || metrics_active_tab.get() == "storage">
                            <div class="metrics-tab-content">
                                <div class="storage-grid">
                                    // Storage Overview
                                    <div class="storage-card large">
                                        <div class="storage-card-header">
                                            <h3>"Storage Overview"</h3>
                                        </div>
                                        <div class="storage-visual">
                                            <div class="storage-ring">
                                                <svg viewBox="0 0 100 100">
                                                    <circle cx="50" cy="50" r="40" class="ring-bg"/>
                                                    <circle cx="50" cy="50" r="40" class="ring-fill" style="stroke-dasharray: 188.4; stroke-dashoffset: 56.52"/>
                                                </svg>
                                                <div class="ring-center">
                                                    <span class="ring-value">"70%"</span>
                                                    <span class="ring-label">"Used"</span>
                                                </div>
                                            </div>
                                            <div class="storage-breakdown">
                                                <div class="storage-type">
                                                    <span class="type-color data"></span>
                                                    <span class="type-name">"Data"</span>
                                                    <span class="type-size">"420 GB"</span>
                                                </div>
                                                <div class="storage-type">
                                                    <span class="type-color index"></span>
                                                    <span class="type-name">"Indexes"</span>
                                                    <span class="type-size">"85 GB"</span>
                                                </div>
                                                <div class="storage-type">
                                                    <span class="type-color wal"></span>
                                                    <span class="type-name">"WAL"</span>
                                                    <span class="type-size">"32 GB"</span>
                                                </div>
                                                <div class="storage-type">
                                                    <span class="type-color temp"></span>
                                                    <span class="type-name">"Temp"</span>
                                                    <span class="type-size">"8 GB"</span>
                                                </div>
                                                <div class="storage-type free">
                                                    <span class="type-color"></span>
                                                    <span class="type-name">"Available"</span>
                                                    <span class="type-size">"230 GB"</span>
                                                </div>
                                            </div>
                                        </div>
                                    </div>

                                    // Disk I/O
                                    <div class="storage-card">
                                        <div class="storage-card-header">
                                            <h3>"Disk I/O"</h3>
                                        </div>
                                        <div class="io-metrics">
                                            <div class="io-metric">
                                                <span class="io-label">"Read"</span>
                                                <span class="io-value">"245 MB/s"</span>
                                                <div class="io-bar"><div class="io-fill read" style="width: 61%"></div></div>
                                            </div>
                                            <div class="io-metric">
                                                <span class="io-label">"Write"</span>
                                                <span class="io-value">"128 MB/s"</span>
                                                <div class="io-bar"><div class="io-fill write" style="width: 32%"></div></div>
                                            </div>
                                            <div class="io-metric">
                                                <span class="io-label">"IOPS"</span>
                                                <span class="io-value">"12,450"</span>
                                                <div class="io-bar"><div class="io-fill iops" style="width: 78%"></div></div>
                                            </div>
                                        </div>
                                    </div>

                                    // Table Sizes
                                    <div class="storage-card">
                                        <div class="storage-card-header">
                                            <h3>"Largest Tables"</h3>
                                        </div>
                                        <div class="table-sizes">
                                            <div class="table-size-item">
                                                <span class="table-name">"events"</span>
                                                <span class="table-rows">"142M rows"</span>
                                                <span class="table-size-val">"85 GB"</span>
                                            </div>
                                            <div class="table-size-item">
                                                <span class="table-name">"users"</span>
                                                <span class="table-rows">"24M rows"</span>
                                                <span class="table-size-val">"32 GB"</span>
                                            </div>
                                            <div class="table-size-item">
                                                <span class="table-name">"orders"</span>
                                                <span class="table-rows">"18M rows"</span>
                                                <span class="table-size-val">"28 GB"</span>
                                            </div>
                                            <div class="table-size-item">
                                                <span class="table-name">"products"</span>
                                                <span class="table-rows">"850K rows"</span>
                                                <span class="table-size-val">"12 GB"</span>
                                            </div>
                                        </div>
                                    </div>

                                    // Compaction Status
                                    <div class="storage-card">
                                        <div class="storage-card-header">
                                            <h3>"Compaction"</h3>
                                            <span class="status-badge running">"Running"</span>
                                        </div>
                                        <div class="compaction-stats">
                                            <div class="compaction-stat">
                                                <span class="comp-label">"Progress"</span>
                                                <div class="comp-progress">
                                                    <div class="comp-fill" style="width: 67%"></div>
                                                </div>
                                                <span class="comp-value">"67%"</span>
                                            </div>
                                            <div class="compaction-info">
                                                <span>"Level 3 -> Level 4"</span>
                                                <span>"ETA: 12 min"</span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </Show>

                        // Network Tab Content
                        <Show when=move || metrics_active_tab.get() == "network">
                            <div class="metrics-tab-content">
                                <div class="network-grid">
                                    // Network Throughput
                                    <div class="network-card large">
                                        <div class="network-card-header">
                                            <h3>"Network Throughput"</h3>
                                        </div>
                                        <div class="throughput-metrics">
                                            <div class="throughput-stat">
                                                <div class="throughput-icon in">"IN"</div>
                                                <div class="throughput-info">
                                                    <span class="throughput-value">"847 MB/s"</span>
                                                    <span class="throughput-label">"Incoming Traffic"</span>
                                                </div>
                                            </div>
                                            <div class="throughput-stat">
                                                <div class="throughput-icon out">"OUT"</div>
                                                <div class="throughput-info">
                                                    <span class="throughput-value">"1.2 GB/s"</span>
                                                    <span class="throughput-label">"Outgoing Traffic"</span>
                                                </div>
                                            </div>
                                        </div>
                                        <div class="network-chart">
                                            <svg class="line-chart" viewBox="0 0 600 150" preserveAspectRatio="none">
                                                <line x1="0" y1="50" x2="600" y2="50" class="grid-line"/>
                                                <line x1="0" y1="100" x2="600" y2="100" class="grid-line"/>
                                                <path d="M0,80 C50,75 100,70 150,65 C200,60 250,55 300,50 C350,45 400,50 450,55 C500,60 550,55 600,50" class="chart-line-reads" fill="none"/>
                                                <path d="M0,110 C50,105 100,100 150,95 C200,90 250,85 300,80 C350,75 400,80 450,85 C500,90 550,85 600,80" class="chart-line-writes" fill="none"/>
                                            </svg>
                                        </div>
                                    </div>

                                    // Connections
                                    <div class="network-card">
                                        <div class="network-card-header">
                                            <h3>"Active Connections"</h3>
                                        </div>
                                        <div class="connections-donut">
                                            <svg viewBox="0 0 100 100">
                                                <circle cx="50" cy="50" r="35" class="donut-bg"/>
                                                <circle cx="50" cy="50" r="35" class="donut-active" style="stroke-dasharray: 154; stroke-dashoffset: 46.2"/>
                                                <circle cx="50" cy="50" r="35" class="donut-idle" style="stroke-dasharray: 154; stroke-dashoffset: 123.2" transform="rotate(-90 50 50)"/>
                                            </svg>
                                            <div class="donut-center">
                                                <span class="donut-value">"1,247"</span>
                                                <span class="donut-label">"Total"</span>
                                            </div>
                                        </div>
                                        <div class="connection-legend">
                                            <span class="legend-item active">"Active: 890"</span>
                                            <span class="legend-item idle">"Idle: 357"</span>
                                        </div>
                                    </div>

                                    // Replication Status
                                    <div class="network-card">
                                        <div class="network-card-header">
                                            <h3>"Replication Lag"</h3>
                                        </div>
                                        <div class="replication-nodes">
                                            {nodes.get().iter().take(4).enumerate().map(|(i, node)| {
                                                let lag = if i == 0 { "0ms" } else { &format!("{}ms", 5 + i * 3) };
                                                let status = if i == 0 { "leader" } else { "follower" };
                                                view! {
                                                    <div class=format!("repl-node {}", status)>
                                                        <span class="repl-node-name">{node.id.clone()}</span>
                                                        <span class="repl-lag">{lag.to_string()}</span>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>

                                    // Packet Stats
                                    <div class="network-card">
                                        <div class="network-card-header">
                                            <h3>"Packet Statistics"</h3>
                                        </div>
                                        <div class="packet-stats">
                                            <div class="packet-stat">
                                                <span class="packet-label">"Packets In"</span>
                                                <span class="packet-value">"1.2M/s"</span>
                                            </div>
                                            <div class="packet-stat">
                                                <span class="packet-label">"Packets Out"</span>
                                                <span class="packet-value">"980K/s"</span>
                                            </div>
                                            <div class="packet-stat">
                                                <span class="packet-label">"Errors"</span>
                                                <span class="packet-value good">"0"</span>
                                            </div>
                                            <div class="packet-stat">
                                                <span class="packet-label">"Dropped"</span>
                                                <span class="packet-value good">"12"</span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </Show>
                    </div>
                </Show>

                // Settings Page
                <Show when=move || !loading.get() && current_page.get() == DashboardPage::Settings>
                    <div class="settings-page">
                        // Settings Tabs
                        <div class="settings-tabs">
                            <button
                                class=move || if settings_tab.get() == "general" { "settings-tab active" } else { "settings-tab" }
                                on:click=move |_| set_settings_tab.set("general".to_string())
                            >"General"</button>
                            <button
                                class=move || if settings_tab.get() == "security" { "settings-tab active" } else { "settings-tab" }
                                on:click=move |_| set_settings_tab.set("security".to_string())
                            >"Security & 2FA"</button>
                            <button
                                class=move || if settings_tab.get() == "users" { "settings-tab active" } else { "settings-tab" }
                                on:click=move |_| set_settings_tab.set("users".to_string())
                            >"User Management"</button>
                            <button
                                class=move || if settings_tab.get() == "rbac" { "settings-tab active" } else { "settings-tab" }
                                on:click=move |_| set_settings_tab.set("rbac".to_string())
                            >"RBAC & Roles"</button>
                        </div>

                        // Status message
                        {move || settings_message.get().map(|(msg, is_success)| {
                            let class = if is_success { "status-message success" } else { "status-message error" };
                            view! {
                                <div class=class>
                                    <span>{msg}</span>
                                    <button class="dismiss-btn" on:click=move |_| set_settings_message.set(None)>"x"</button>
                                </div>
                            }
                        })}

                        // General Tab
                        <Show when=move || settings_tab.get() == "general">
                            <div class="settings-section">
                                <h3 class="settings-section-title">"Cluster Configuration"</h3>
                                <div class="settings-card">
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Cluster Name"</span>
                                            <span class="setting-description">"Identifier for this Aegis cluster"</span>
                                        </div>
                                        <div class="setting-control">
                                            <input type="text" class="setting-input" value=move || cluster_status.get().map(|s| s.name).unwrap_or_default() readonly />
                                        </div>
                                    </div>
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Replication Factor"</span>
                                            <span class="setting-description">"Number of copies of each data shard"</span>
                                        </div>
                                        <div class="setting-control">
                                            <select
                                                class="setting-select"
                                                on:change=move |ev| {
                                                    let val: i32 = event_target_value(&ev).parse().unwrap_or(3);
                                                    set_replication_factor.set(val);
                                                    set_settings_message.set(Some(("Replication factor updated".to_string(), true)));
                                                }
                                            >
                                                <option value="1" selected=move || replication_factor.get() == 1>"1"</option>
                                                <option value="3" selected=move || replication_factor.get() == 3>"3"</option>
                                                <option value="5" selected=move || replication_factor.get() == 5>"5"</option>
                                                <option value="7" selected=move || replication_factor.get() == 7>"7"</option>
                                            </select>
                                        </div>
                                    </div>
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Shard Count"</span>
                                            <span class="setting-description">"Number of data partitions"</span>
                                        </div>
                                        <div class="setting-control">
                                            <span class="setting-value">{move || cluster_status.get().map(|s| s.shard_count.to_string()).unwrap_or("-".to_string())}</span>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            <div class="settings-section">
                                <h3 class="settings-section-title">"Backup & Recovery"</h3>
                                <div class="settings-card">
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Automatic Backups"</span>
                                            <span class="setting-description">"Schedule regular backups"</span>
                                        </div>
                                        <div class="setting-control">
                                            <label class="toggle">
                                                <input
                                                    type="checkbox"
                                                    prop:checked=move || auto_backups_enabled.get()
                                                    on:change=move |ev| {
                                                        let checked = event_target_checked(&ev);
                                                        set_auto_backups_enabled.set(checked);
                                                        let msg = if checked { "Automatic backups enabled" } else { "Automatic backups disabled" };
                                                        set_settings_message.set(Some((msg.to_string(), true)));
                                                    }
                                                />
                                                <span class="toggle-slider"></span>
                                            </label>
                                        </div>
                                    </div>
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Backup Schedule"</span>
                                            <span class="setting-description">"How often to create backups"</span>
                                        </div>
                                        <div class="setting-control">
                                            <select
                                                class="setting-select"
                                                prop:disabled=move || !auto_backups_enabled.get()
                                                on:change=move |ev| {
                                                    set_backup_schedule.set(event_target_value(&ev));
                                                    set_settings_message.set(Some(("Backup schedule updated".to_string(), true)));
                                                }
                                            >
                                                <option value="1h" selected=move || backup_schedule.get() == "1h">"Every hour"</option>
                                                <option value="6h" selected=move || backup_schedule.get() == "6h">"Every 6 hours"</option>
                                                <option value="24h" selected=move || backup_schedule.get() == "24h">"Daily"</option>
                                                <option value="7d" selected=move || backup_schedule.get() == "7d">"Weekly"</option>
                                            </select>
                                        </div>
                                    </div>
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Retention Period"</span>
                                            <span class="setting-description">"How long to keep backups"</span>
                                        </div>
                                        <div class="setting-control">
                                            <select
                                                class="setting-select"
                                                prop:disabled=move || !auto_backups_enabled.get()
                                                on:change=move |ev| {
                                                    set_retention_period.set(event_target_value(&ev));
                                                    set_settings_message.set(Some(("Retention period updated".to_string(), true)));
                                                }
                                            >
                                                <option value="7d" selected=move || retention_period.get() == "7d">"7 days"</option>
                                                <option value="14d" selected=move || retention_period.get() == "14d">"14 days"</option>
                                                <option value="30d" selected=move || retention_period.get() == "30d">"30 days"</option>
                                                <option value="90d" selected=move || retention_period.get() == "90d">"90 days"</option>
                                            </select>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            <div class="settings-section danger">
                                <h3 class="settings-section-title">"Danger Zone"</h3>
                                <div class="settings-card danger">
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Reset Cluster"</span>
                                            <span class="setting-description">"Delete all data and reset to default settings"</span>
                                        </div>
                                        <div class="setting-control">
                                            <button
                                                class="btn-danger"
                                                on:click=move |_| set_show_reset_confirm.set(true)
                                            >"Reset Cluster"</button>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            // Reset Confirmation Modal
                            <Show when=move || show_reset_confirm.get()>
                                <div class="modal-overlay" on:click=move |_| set_show_reset_confirm.set(false)>
                                    <div class="modal-content small" on:click=|ev| ev.stop_propagation()>
                                        <div class="modal-header danger">
                                            <h2>"Confirm Cluster Reset"</h2>
                                            <button class="modal-close" on:click=move |_| set_show_reset_confirm.set(false)>
                                                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                    <line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line>
                                                </svg>
                                            </button>
                                        </div>
                                        <div class="modal-body">
                                            <div class="danger-warning">
                                                <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                    <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
                                                    <line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12.01" y2="17"></line>
                                                </svg>
                                                <h3>"This action cannot be undone!"</h3>
                                                <p>"This will permanently delete all data and reset the cluster to default settings. All nodes, databases, and configurations will be lost."</p>
                                            </div>
                                            <div class="confirm-input-group">
                                                <label>"Type "<strong>"RESET"</strong>" to confirm:"</label>
                                                <input
                                                    type="text"
                                                    class="form-input"
                                                    placeholder="Type RESET to confirm"
                                                    prop:value=move || reset_confirm_text.get()
                                                    on:input=move |ev| set_reset_confirm_text.set(event_target_value(&ev))
                                                />
                                            </div>
                                        </div>
                                        <div class="modal-footer">
                                            <button class="btn-secondary" on:click=move |_| {
                                                set_reset_confirm_text.set(String::new());
                                                set_show_reset_confirm.set(false);
                                            }>"Cancel"</button>
                                            <button
                                                class="btn-danger"
                                                prop:disabled=move || reset_confirm_text.get() != "RESET"
                                                on:click=move |_| {
                                                    if reset_confirm_text.get() == "RESET" {
                                                        // In real app, call API to reset cluster
                                                        set_settings_message.set(Some(("Cluster reset initiated...".to_string(), false)));
                                                        set_reset_confirm_text.set(String::new());
                                                        set_show_reset_confirm.set(false);
                                                    }
                                                }
                                            >"Reset Cluster"</button>
                                        </div>
                                    </div>
                                </div>
                            </Show>
                        </Show>

                        // Security & 2FA Tab
                        <Show when=move || settings_tab.get() == "security">
                            <div class="settings-section">
                                <h3 class="settings-section-title">"Authentication"</h3>
                                <div class="settings-card">
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"TLS Encryption"</span>
                                            <span class="setting-description">"Encrypt all cluster communication"</span>
                                        </div>
                                        <div class="setting-control">
                                            <label class="toggle">
                                                <input
                                                    type="checkbox"
                                                    prop:checked=move || tls_enabled.get()
                                                    on:change=move |ev| {
                                                        let checked = event_target_checked(&ev);
                                                        set_tls_enabled.set(checked);
                                                        let msg = if checked { "TLS encryption enabled" } else { "TLS encryption disabled - WARNING: Connections are not secure!" };
                                                        set_settings_message.set(Some((msg.to_string(), checked)));
                                                    }
                                                />
                                                <span class="toggle-slider"></span>
                                            </label>
                                        </div>
                                    </div>
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Authentication Required"</span>
                                            <span class="setting-description">"Require authentication for all connections"</span>
                                        </div>
                                        <div class="setting-control">
                                            <label class="toggle">
                                                <input
                                                    type="checkbox"
                                                    prop:checked=move || auth_required.get()
                                                    on:change=move |ev| {
                                                        let checked = event_target_checked(&ev);
                                                        set_auth_required.set(checked);
                                                        let msg = if checked { "Authentication required for all connections" } else { "Authentication disabled - WARNING: Database is publicly accessible!" };
                                                        set_settings_message.set(Some((msg.to_string(), checked)));
                                                    }
                                                />
                                                <span class="toggle-slider"></span>
                                            </label>
                                        </div>
                                    </div>
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Session Timeout"</span>
                                            <span class="setting-description">"Auto-logout after inactivity"</span>
                                        </div>
                                        <div class="setting-control">
                                            <select
                                                class="setting-select"
                                                on:change=move |ev| {
                                                    set_session_timeout.set(event_target_value(&ev));
                                                    set_settings_message.set(Some(("Session timeout updated".to_string(), true)));
                                                }
                                            >
                                                <option value="15m" selected=move || session_timeout.get() == "15m">"15 minutes"</option>
                                                <option value="30m" selected=move || session_timeout.get() == "30m">"30 minutes"</option>
                                                <option value="1h" selected=move || session_timeout.get() == "1h">"1 hour"</option>
                                                <option value="4h" selected=move || session_timeout.get() == "4h">"4 hours"</option>
                                            </select>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            <div class="settings-section">
                                <h3 class="settings-section-title">"Two-Factor Authentication (2FA)"</h3>
                                <div class="settings-card">
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Require 2FA for all users"</span>
                                            <span class="setting-description">"Force all users to enable two-factor authentication"</span>
                                        </div>
                                        <div class="setting-control">
                                            <label class="toggle">
                                                <input
                                                    type="checkbox"
                                                    prop:checked=move || require_2fa.get()
                                                    on:change=move |ev| {
                                                        let checked = event_target_checked(&ev);
                                                        set_require_2fa.set(checked);
                                                        let msg = if checked { "2FA is now mandatory for all users" } else { "2FA is now optional for users" };
                                                        set_settings_message.set(Some((msg.to_string(), true)));
                                                    }
                                                />
                                                <span class="toggle-slider"></span>
                                            </label>
                                        </div>
                                    </div>
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"2FA Methods Allowed"</span>
                                            <span class="setting-description">"Select which 2FA methods users can use"</span>
                                        </div>
                                        <div class="setting-control">
                                            <div class="checkbox-group">
                                                <label class="checkbox-label">
                                                    <input
                                                        type="checkbox"
                                                        prop:checked=move || totp_enabled.get()
                                                        on:change=move |ev| {
                                                            let checked = event_target_checked(&ev);
                                                            set_totp_enabled.set(checked);
                                                            set_settings_message.set(Some(("TOTP authenticator setting updated".to_string(), true)));
                                                        }
                                                    />
                                                    <span>"TOTP (Authenticator App)"</span>
                                                </label>
                                                <label class="checkbox-label">
                                                    <input
                                                        type="checkbox"
                                                        prop:checked=move || sms_enabled.get()
                                                        on:change=move |ev| {
                                                            let checked = event_target_checked(&ev);
                                                            set_sms_enabled.set(checked);
                                                            set_settings_message.set(Some(("SMS 2FA setting updated".to_string(), true)));
                                                        }
                                                    />
                                                    <span>"SMS"</span>
                                                </label>
                                                <label class="checkbox-label">
                                                    <input
                                                        type="checkbox"
                                                        prop:checked=move || webauthn_enabled.get()
                                                        on:change=move |ev| {
                                                            let checked = event_target_checked(&ev);
                                                            set_webauthn_enabled.set(checked);
                                                            set_settings_message.set(Some(("Hardware key (WebAuthn) setting updated".to_string(), true)));
                                                        }
                                                    />
                                                    <span>"Hardware Key (WebAuthn)"</span>
                                                </label>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Recovery Codes"</span>
                                            <span class="setting-description">"Allow users to generate backup codes"</span>
                                        </div>
                                        <div class="setting-control">
                                            <label class="toggle">
                                                <input
                                                    type="checkbox"
                                                    prop:checked=move || recovery_codes_enabled.get()
                                                    on:change=move |ev| {
                                                        let checked = event_target_checked(&ev);
                                                        set_recovery_codes_enabled.set(checked);
                                                        let msg = if checked { "Recovery codes enabled" } else { "Recovery codes disabled" };
                                                        set_settings_message.set(Some((msg.to_string(), true)));
                                                    }
                                                />
                                                <span class="toggle-slider"></span>
                                            </label>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            <div class="settings-section">
                                <h3 class="settings-section-title">"Audit Logging"</h3>
                                <div class="settings-card">
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Enable Audit Logs"</span>
                                            <span class="setting-description">"Log all administrative and data access actions"</span>
                                        </div>
                                        <div class="setting-control">
                                            <label class="toggle">
                                                <input
                                                    type="checkbox"
                                                    prop:checked=move || audit_logging_enabled.get()
                                                    on:change=move |ev| {
                                                        let checked = event_target_checked(&ev);
                                                        set_audit_logging_enabled.set(checked);
                                                        let msg = if checked { "Audit logging enabled" } else { "Audit logging disabled" };
                                                        set_settings_message.set(Some((msg.to_string(), true)));
                                                    }
                                                />
                                                <span class="toggle-slider"></span>
                                            </label>
                                        </div>
                                    </div>
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Log Retention"</span>
                                            <span class="setting-description">"How long to keep audit logs"</span>
                                        </div>
                                        <div class="setting-control">
                                            <select
                                                class="setting-select"
                                                prop:disabled=move || !audit_logging_enabled.get()
                                                on:change=move |_| {
                                                    set_settings_message.set(Some(("Audit log retention updated".to_string(), true)));
                                                }
                                            >
                                                <option value="30d">"30 days"</option>
                                                <option value="90d" selected>"90 days"</option>
                                                <option value="1y">"1 year"</option>
                                                <option value="forever">"Forever"</option>
                                            </select>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </Show>

                        // User Management Tab
                        <Show when=move || settings_tab.get() == "users">
                            <div class="settings-section">
                                <div class="section-header-row">
                                    <h3 class="settings-section-title">"User Management"</h3>
                                    <button class="btn-primary" on:click=move |_| set_show_add_user.set(true)>"+ Add User"</button>
                                </div>

                                // Add User Form
                                <Show when=move || show_add_user.get()>
                                    <div class="settings-card add-form">
                                        <h4>"Add New User"</h4>
                                        <div class="form-grid">
                                            <div class="form-row">
                                                <label>"Full Name"</label>
                                                <input
                                                    type="text"
                                                    class="form-input"
                                                    placeholder="John Doe"
                                                    prop:value=move || new_user_name.get()
                                                    on:input=move |ev| set_new_user_name.set(event_target_value(&ev))
                                                />
                                            </div>
                                            <div class="form-row">
                                                <label>"Email"</label>
                                                <input
                                                    type="email"
                                                    class="form-input"
                                                    placeholder="john@company.com"
                                                    prop:value=move || new_user_email.get()
                                                    on:input=move |ev| set_new_user_email.set(event_target_value(&ev))
                                                />
                                            </div>
                                            <div class="form-row">
                                                <label>"Role"</label>
                                                <select
                                                    class="form-select"
                                                    on:change=move |ev| set_new_user_role.set(event_target_value(&ev))
                                                >
                                                    <option value="viewer">"Viewer"</option>
                                                    <option value="analyst">"Analyst"</option>
                                                    <option value="developer">"Developer"</option>
                                                    <option value="admin">"Admin"</option>
                                                </select>
                                            </div>
                                            <div class="form-row">
                                                <label>"Require 2FA"</label>
                                                <label class="toggle">
                                                    <input
                                                        type="checkbox"
                                                        prop:checked=move || new_user_2fa.get()
                                                        on:change=move |ev| set_new_user_2fa.set(event_target_checked(&ev))
                                                    />
                                                    <span class="toggle-slider"></span>
                                                </label>
                                            </div>
                                        </div>
                                        <div class="form-actions">
                                            <button class="btn-secondary" on:click=move |_| {
                                                set_show_add_user.set(false);
                                                set_new_user_name.set(String::new());
                                                set_new_user_email.set(String::new());
                                            }>"Cancel"</button>
                                            <button class="btn-primary" on:click=move |_| {
                                                let name = new_user_name.get();
                                                let email = new_user_email.get();
                                                let role = new_user_role.get();
                                                let require_2fa = new_user_2fa.get();

                                                if !name.is_empty() && !email.is_empty() {
                                                    let mut users = users_list.get();
                                                    let id = format!("user-{}", users.len() + 1);
                                                    users.push((id, name, email, role, require_2fa));
                                                    set_users_list.set(users);
                                                    set_show_add_user.set(false);
                                                    set_new_user_name.set(String::new());
                                                    set_new_user_email.set(String::new());
                                                    set_settings_message.set(Some(("User created successfully".to_string(), true)));
                                                }
                                            }>"Create User"</button>
                                        </div>
                                    </div>
                                </Show>

                                // Users Table
                                <div class="settings-card">
                                    <table class="data-table users-table">
                                        <thead>
                                            <tr>
                                                <th>"Name"</th>
                                                <th>"Email"</th>
                                                <th>"Role"</th>
                                                <th>"2FA"</th>
                                                <th>"Actions"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {move || users_list.get().into_iter().map(|(id, name, email, role, has_2fa)| {
                                                let id_for_edit = id.clone();
                                                let id_for_delete = id.clone();
                                                let name_clone = name.clone();
                                                view! {
                                                    <tr>
                                                        <td>
                                                            <div class="user-name-cell">
                                                                <div class="user-avatar">{name.chars().next().unwrap_or('U')}</div>
                                                                <span>{name}</span>
                                                            </div>
                                                        </td>
                                                        <td class="email-cell">{email}</td>
                                                        <td>
                                                            <span class=format!("role-badge role-{}", role)>{role.clone()}</span>
                                                        </td>
                                                        <td>
                                                            {if has_2fa {
                                                                view! { <span class="status-badge enabled">"Enabled"</span> }.into_view()
                                                            } else {
                                                                view! { <span class="status-badge disabled">"Disabled"</span> }.into_view()
                                                            }}
                                                        </td>
                                                        <td>
                                                            <div class="action-buttons">
                                                                <button class="btn-icon" title="Edit">"Edit"</button>
                                                                <button
                                                                    class="btn-icon danger"
                                                                    title="Delete"
                                                                    on:click=move |_| {
                                                                        if web_sys::window()
                                                                            .and_then(|w| w.confirm_with_message(&format!("Delete user '{}'?", name_clone)).ok())
                                                                            .unwrap_or(false)
                                                                        {
                                                                            let mut users = users_list.get();
                                                                            users.retain(|(uid, _, _, _, _)| uid != &id_for_delete);
                                                                            set_users_list.set(users);
                                                                            set_settings_message.set(Some(("User deleted".to_string(), true)));
                                                                        }
                                                                    }
                                                                >"Del"</button>
                                                            </div>
                                                        </td>
                                                    </tr>
                                                }
                                            }).collect_view()}
                                        </tbody>
                                    </table>
                                </div>
                            </div>
                        </Show>

                        // RBAC & Roles Tab
                        <Show when=move || settings_tab.get() == "rbac">
                            <div class="settings-section">
                                <div class="section-header-row">
                                    <h3 class="settings-section-title">"Role-Based Access Control (RBAC)"</h3>
                                    <button class="btn-primary" on:click=move |_| set_show_add_role.set(true)>"+ Add Role"</button>
                                </div>

                                // Add Role Form
                                <Show when=move || show_add_role.get()>
                                    <div class="settings-card add-form">
                                        <h4>"Create New Role"</h4>
                                        <div class="form-grid">
                                            <div class="form-row">
                                                <label>"Role Name"</label>
                                                <input
                                                    type="text"
                                                    class="form-input"
                                                    placeholder="custom-role"
                                                    prop:value=move || new_role_name.get()
                                                    on:input=move |ev| set_new_role_name.set(event_target_value(&ev))
                                                />
                                            </div>
                                            <div class="form-row full-width">
                                                <label>"Permissions"</label>
                                                <div class="permissions-grid">
                                                    <label class="checkbox-label">
                                                        <input type="checkbox" />
                                                        <span>"data:read"</span>
                                                    </label>
                                                    <label class="checkbox-label">
                                                        <input type="checkbox" />
                                                        <span>"data:write"</span>
                                                    </label>
                                                    <label class="checkbox-label">
                                                        <input type="checkbox" />
                                                        <span>"data:delete"</span>
                                                    </label>
                                                    <label class="checkbox-label">
                                                        <input type="checkbox" />
                                                        <span>"query:execute"</span>
                                                    </label>
                                                    <label class="checkbox-label">
                                                        <input type="checkbox" />
                                                        <span>"metrics:read"</span>
                                                    </label>
                                                    <label class="checkbox-label">
                                                        <input type="checkbox" />
                                                        <span>"users:manage"</span>
                                                    </label>
                                                    <label class="checkbox-label">
                                                        <input type="checkbox" />
                                                        <span>"roles:manage"</span>
                                                    </label>
                                                    <label class="checkbox-label">
                                                        <input type="checkbox" />
                                                        <span>"cluster:manage"</span>
                                                    </label>
                                                </div>
                                            </div>
                                        </div>
                                        <div class="form-actions">
                                            <button class="btn-secondary" on:click=move |_| {
                                                set_show_add_role.set(false);
                                                set_new_role_name.set(String::new());
                                            }>"Cancel"</button>
                                            <button class="btn-primary" on:click=move |_| {
                                                let name = new_role_name.get();
                                                if !name.is_empty() {
                                                    let mut roles = roles_list.get();
                                                    roles.push((name.clone(), "Custom role".to_string(), vec!["data:read".to_string()]));
                                                    set_roles_list.set(roles);
                                                    set_show_add_role.set(false);
                                                    set_new_role_name.set(String::new());
                                                    set_settings_message.set(Some(("Role created successfully".to_string(), true)));
                                                }
                                            }>"Create Role"</button>
                                        </div>
                                    </div>
                                </Show>

                                // Roles List
                                <div class="roles-grid">
                                    {move || roles_list.get().into_iter().map(|(name, desc, permissions)| {
                                        let name_for_delete = name.clone();
                                        let name_display = name.clone();
                                        let is_system = ["admin", "developer", "analyst", "viewer"].contains(&name.as_str());
                                        view! {
                                            <div class="role-card">
                                                <div class="role-header">
                                                    <h4 class="role-name">{name_display}</h4>
                                                    {if is_system {
                                                        view! { <span class="system-badge">"System"</span> }.into_view()
                                                    } else {
                                                        view! { <span class="custom-badge">"Custom"</span> }.into_view()
                                                    }}
                                                </div>
                                                <p class="role-description">{desc}</p>
                                                <div class="role-permissions">
                                                    <span class="permissions-label">"Permissions:"</span>
                                                    <div class="permission-tags">
                                                        {permissions.into_iter().map(|p| {
                                                            view! { <span class="permission-tag">{p}</span> }
                                                        }).collect_view()}
                                                    </div>
                                                </div>
                                                <div class="role-actions">
                                                    <button class="btn-icon" title="Edit">"Edit"</button>
                                                    {if !is_system {
                                                        view! {
                                                            <button
                                                                class="btn-icon danger"
                                                                title="Delete"
                                                                on:click=move |_| {
                                                                    let mut roles = roles_list.get();
                                                                    roles.retain(|(n, _, _)| n != &name_for_delete);
                                                                    set_roles_list.set(roles);
                                                                    set_settings_message.set(Some(("Role deleted".to_string(), true)));
                                                                }
                                                            >"Del"</button>
                                                        }.into_view()
                                                    } else {
                                                        view! { <span></span> }.into_view()
                                                    }}
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>

                            // RBAC Policies Section
                            <div class="settings-section">
                                <h3 class="settings-section-title">"Access Policies"</h3>
                                <div class="settings-card">
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Default Role for New Users"</span>
                                            <span class="setting-description">"Role assigned to users who sign up"</span>
                                        </div>
                                        <div class="setting-control">
                                            <select class="setting-select">
                                                <option value="viewer" selected>"Viewer"</option>
                                                <option value="analyst">"Analyst"</option>
                                                <option value="developer">"Developer"</option>
                                            </select>
                                        </div>
                                    </div>
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"Allow Self-Registration"</span>
                                            <span class="setting-description">"Let users create their own accounts"</span>
                                        </div>
                                        <div class="setting-control">
                                            <label class="toggle">
                                                <input type="checkbox" />
                                                <span class="toggle-slider"></span>
                                            </label>
                                        </div>
                                    </div>
                                    <div class="setting-row">
                                        <div class="setting-info">
                                            <span class="setting-name">"IP Allowlist"</span>
                                            <span class="setting-description">"Restrict access to specific IP addresses"</span>
                                        </div>
                                        <div class="setting-control">
                                            <label class="toggle">
                                                <input type="checkbox" />
                                                <span class="toggle-slider"></span>
                                            </label>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </Show>
                    </div>
                </Show>
            </main>

            // Modal Overlays
            <Show when=move || active_modal.get() != BrowserModal::None>
                <div class="modal-overlay" on:click=move |_| set_active_modal.set(BrowserModal::None)>
                    <div class="modal-content" on:click=|e| e.stop_propagation()>
                        // Key-Value Browser Modal
                        <Show when=move || active_modal.get() == BrowserModal::KeyValue>
                            <div class="modal-header">
                                <h2>"Key-Value Browser"</h2>
                                <button class="modal-close" on:click=move |_| {
                                    set_active_modal.set(BrowserModal::None);
                                    set_kv_show_add_form.set(false);
                                    set_kv_edit_key.set(None);
                                    set_kv_message.set(None);
                                }>"x"</button>
                            </div>
                            <div class="modal-body">
                                // Status message
                                {move || kv_message.get().map(|(msg, is_success)| {
                                    let class = if is_success { "status-message success" } else { "status-message error" };
                                    view! {
                                        <div class=class>
                                            <span>{msg}</span>
                                            <button class="dismiss-btn" on:click=move |_| set_kv_message.set(None)>"x"</button>
                                        </div>
                                    }
                                })}

                                <Show when=move || modal_loading.get()>
                                    <div class="modal-loading">
                                        <div class="spinner"></div>
                                        <p>"Processing..."</p>
                                    </div>
                                </Show>
                                <Show when=move || !modal_loading.get()>
                                    <div class="kv-browser">
                                        // Add Key Form
                                        <Show when=move || kv_show_add_form.get()>
                                            <div class="add-form-panel">
                                                <h3>"Add New Key"</h3>
                                                <div class="form-row">
                                                    <label>"Key"</label>
                                                    <input
                                                        type="text"
                                                        class="form-input"
                                                        placeholder="my-key"
                                                        prop:value=move || kv_new_key.get()
                                                        on:input=move |ev| set_kv_new_key.set(event_target_value(&ev))
                                                    />
                                                </div>
                                                <div class="form-row">
                                                    <label>"Value (JSON or string)"</label>
                                                    <textarea
                                                        class="form-textarea"
                                                        placeholder=r#"{"name": "value"} or plain text"#
                                                        prop:value=move || kv_new_value.get()
                                                        on:input=move |ev| set_kv_new_value.set(event_target_value(&ev))
                                                    ></textarea>
                                                </div>
                                                <div class="form-row">
                                                    <label>"TTL (seconds, optional)"</label>
                                                    <input
                                                        type="number"
                                                        class="form-input"
                                                        placeholder="3600"
                                                        prop:value=move || kv_new_ttl.get()
                                                        on:input=move |ev| set_kv_new_ttl.set(event_target_value(&ev))
                                                    />
                                                </div>
                                                <div class="form-actions">
                                                    <button class="btn-secondary" on:click=move |_| set_kv_show_add_form.set(false)>"Cancel"</button>
                                                    <button class="btn-primary" on:click=move |_| add_kv_entry.dispatch(())>"Save Key"</button>
                                                </div>
                                            </div>
                                        </Show>

                                        // Edit Key Form
                                        <Show when=move || kv_edit_key.get().is_some()>
                                            <div class="add-form-panel">
                                                <h3>"Edit Key: "{move || kv_edit_key.get().unwrap_or_default()}</h3>
                                                <div class="form-row">
                                                    <label>"Value (JSON or string)"</label>
                                                    <textarea
                                                        class="form-textarea"
                                                        prop:value=move || kv_edit_value.get()
                                                        on:input=move |ev| set_kv_edit_value.set(event_target_value(&ev))
                                                    ></textarea>
                                                </div>
                                                <div class="form-actions">
                                                    <button class="btn-secondary" on:click=move |_| set_kv_edit_key.set(None)>"Cancel"</button>
                                                    <button class="btn-primary" on:click=move |_| update_kv_entry.dispatch(())>"Update"</button>
                                                </div>
                                            </div>
                                        </Show>

                                        // Toolbar
                                        <Show when=move || !kv_show_add_form.get() && kv_edit_key.get().is_none()>
                                            <div class="browser-toolbar">
                                                <input
                                                    type="text"
                                                    class="search-input"
                                                    placeholder="Search keys..."
                                                    prop:value=move || kv_search.get()
                                                    on:input=move |ev| set_kv_search.set(event_target_value(&ev))
                                                />
                                                <div class="toolbar-actions">
                                                    <button class="toolbar-btn" on:click=move |_| load_kv_entries.dispatch(())>"Refresh"</button>
                                                    <button class="toolbar-btn primary" on:click=move |_| set_kv_show_add_form.set(true)>"+ Add Key"</button>
                                                </div>
                                            </div>
                                            <div class="kv-stats">
                                                <span class="stat">"Total Keys: "{move || kv_entries.get().len()}</span>
                                            </div>
                                            <table class="data-table">
                                                <thead>
                                                    <tr>
                                                        <th>"Key"</th>
                                                        <th>"Value"</th>
                                                        <th>"Size"</th>
                                                        <th>"TTL"</th>
                                                        <th>"Actions"</th>
                                                    </tr>
                                                </thead>
                                                <tbody>
                                                    {move || {
                                                        let search = kv_search.get().to_lowercase();
                                                        kv_entries.get().into_iter()
                                                            .filter(|e| search.is_empty() || e.key.to_lowercase().contains(&search))
                                                            .map(|entry| {
                                                                let key_for_edit = entry.key.clone();
                                                                let key_for_delete = entry.key.clone();
                                                                let value_for_edit = entry.value.to_string();
                                                                let value_preview = match &entry.value {
                                                                    serde_json::Value::String(s) => {
                                                                        if s.len() > 40 { format!("{}...", &s[..40]) } else { s.clone() }
                                                                    }
                                                                    v => {
                                                                        let s = v.to_string();
                                                                        if s.len() > 40 { format!("{}...", &s[..40]) } else { s }
                                                                    }
                                                                };
                                                                view! {
                                                                    <tr>
                                                                        <td class="monospace key-cell">{entry.key.clone()}</td>
                                                                        <td class="monospace value-cell" title=entry.value.to_string()>{value_preview}</td>
                                                                        <td>{format_bytes(entry.size_bytes)}</td>
                                                                        <td>{entry.ttl.map(|t| format!("{}s", t)).unwrap_or("-".to_string())}</td>
                                                                        <td>
                                                                            <div class="action-buttons">
                                                                                <button
                                                                                    class="btn-icon"
                                                                                    title="Edit"
                                                                                    on:click=move |_| {
                                                                                        set_kv_edit_key.set(Some(key_for_edit.clone()));
                                                                                        set_kv_edit_value.set(value_for_edit.clone());
                                                                                    }
                                                                                >"Edit"</button>
                                                                                <button
                                                                                    class="btn-icon danger"
                                                                                    title="Delete"
                                                                                    on:click=move |_| {
                                                                                        if web_sys::window()
                                                                                            .and_then(|w| w.confirm_with_message(&format!("Delete key '{}'?", key_for_delete)).ok())
                                                                                            .unwrap_or(false)
                                                                                        {
                                                                                            delete_kv_entry.dispatch(key_for_delete.clone());
                                                                                        }
                                                                                    }
                                                                                >"Del"</button>
                                                                            </div>
                                                                        </td>
                                                                    </tr>
                                                                }
                                                            }).collect_view()
                                                    }}
                                                </tbody>
                                            </table>
                                            <Show when=move || kv_entries.get().is_empty()>
                                                <div class="empty-state">
                                                    <p>"No keys found. Click '+ Add Key' to create one."</p>
                                                </div>
                                            </Show>
                                        </Show>
                                    </div>
                                </Show>
                            </div>
                        </Show>

                        // Collections Browser Modal
                        <Show when=move || active_modal.get() == BrowserModal::Collections>
                            <div class="modal-header">
                                <h2>"Collections Browser"</h2>
                                <button class="modal-close" on:click=move |_| set_active_modal.set(BrowserModal::None)>"x"</button>
                            </div>
                            <div class="modal-body">
                                <Show when=move || modal_loading.get()>
                                    <div class="modal-loading">
                                        <div class="spinner"></div>
                                        <p>"Loading collections..."</p>
                                    </div>
                                </Show>
                                <Show when=move || !modal_loading.get()>
                                    <div class="collections-browser">
                                        <div class="collections-sidebar">
                                            <div class="sidebar-header">
                                                <h3>"Collections"</h3>
                                                <button class="toolbar-btn small" on:click=move |_| set_col_show_new_collection.set(true)>"+"</button>
                                            </div>
                                            // New Collection Form
                                            <Show when=move || col_show_new_collection.get()>
                                                <div class="add-form-inline">
                                                    <input
                                                        type="text"
                                                        class="form-input small"
                                                        placeholder="Collection name"
                                                        prop:value=move || col_new_name.get()
                                                        on:input=move |ev| set_col_new_name.set(event_target_value(&ev))
                                                    />
                                                    <div class="form-actions-inline">
                                                        <button class="btn-small secondary" on:click=move |_| {
                                                            set_col_show_new_collection.set(false);
                                                            set_col_new_name.set(String::new());
                                                        }>"Cancel"</button>
                                                        <button class="btn-small primary" on:click=move |_| {
                                                            let name = col_new_name.get();
                                                            if !name.is_empty() {
                                                                // Would call API to create collection
                                                                set_col_message.set(Some((format!("Collection '{}' created", name), true)));
                                                                set_col_new_name.set(String::new());
                                                                set_col_show_new_collection.set(false);
                                                                load_collections.dispatch(());
                                                            }
                                                        }>"Create"</button>
                                                    </div>
                                                </div>
                                            </Show>
                                            <div class="collections-list">
                                                {move || collections.get().into_iter().map(|col| {
                                                    let col_name = col.name.clone();
                                                    let col_name_click = col.name.clone();
                                                    let is_selected = selected_collection.get().as_ref() == Some(&col_name);
                                                    view! {
                                                        <div
                                                            class=move || if is_selected { "collection-item selected" } else { "collection-item" }
                                                            on:click=move |_| {
                                                                set_selected_collection.set(Some(col_name_click.clone()));
                                                                load_collection_docs.dispatch(col_name_click.clone());
                                                            }
                                                        >
                                                            <span class="collection-name">{col.name.clone()}</span>
                                                            <span class="collection-count">{col.document_count}</span>
                                                        </div>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        </div>
                                        <div class="documents-panel">
                                            // Status message
                                            {move || col_message.get().map(|(msg, is_success)| {
                                                let class = if is_success { "status-message success" } else { "status-message error" };
                                                view! {
                                                    <div class=class>
                                                        <span>{msg}</span>
                                                        <button class="dismiss-btn" on:click=move |_| set_col_message.set(None)>"x"</button>
                                                    </div>
                                                }
                                            })}
                                            <Show when=move || selected_collection.get().is_some()>
                                                <div class="documents-header">
                                                    <h3>{move || selected_collection.get().unwrap_or_default()}</h3>
                                                    <button class="toolbar-btn" on:click=move |_| set_col_show_new_doc.set(true)>"Add Document"</button>
                                                </div>
                                                // New Document Form
                                                <Show when=move || col_show_new_doc.get()>
                                                    <div class="add-form-panel">
                                                        <h4>"New Document"</h4>
                                                        <div class="form-row">
                                                            <label>"Document JSON"</label>
                                                            <textarea
                                                                class="form-textarea"
                                                                placeholder=r#"{"field": "value", "nested": {"key": "value"}}"#
                                                                prop:value=move || col_new_doc_content.get()
                                                                on:input=move |ev| set_col_new_doc_content.set(event_target_value(&ev))
                                                            ></textarea>
                                                        </div>
                                                        <div class="form-actions">
                                                            <button class="btn-secondary" on:click=move |_| {
                                                                set_col_show_new_doc.set(false);
                                                                set_col_new_doc_content.set(String::new());
                                                            }>"Cancel"</button>
                                                            <button class="btn-primary" on:click=move |_| {
                                                                let content = col_new_doc_content.get();
                                                                if !content.is_empty() {
                                                                    // Validate JSON
                                                                    match serde_json::from_str::<serde_json::Value>(&content) {
                                                                        Ok(_) => {
                                                                            set_col_message.set(Some(("Document added successfully".to_string(), true)));
                                                                            set_col_new_doc_content.set(String::new());
                                                                            set_col_show_new_doc.set(false);
                                                                            // Reload docs
                                                                            if let Some(name) = selected_collection.get() {
                                                                                load_collection_docs.dispatch(name);
                                                                            }
                                                                        }
                                                                        Err(e) => {
                                                                            set_col_message.set(Some((format!("Invalid JSON: {}", e), false)));
                                                                        }
                                                                    }
                                                                }
                                                            }>"Add Document"</button>
                                                        </div>
                                                    </div>
                                                </Show>
                                                <div class="documents-list">
                                                    {move || collection_docs.get().into_iter().map(|doc| {
                                                        let data_preview = doc.data.to_string();
                                                        let preview = if data_preview.len() > 100 {
                                                            format!("{}...", &data_preview[..100])
                                                        } else {
                                                            data_preview
                                                        };
                                                        view! {
                                                            <div class="document-item">
                                                                <div class="document-header">
                                                                    <span class="document-id">{doc.id.clone()}</span>
                                                                    <span class="document-date">{doc.updated_at.clone()}</span>
                                                                </div>
                                                                <pre class="document-preview">{preview}</pre>
                                                            </div>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </Show>
                                            <Show when=move || selected_collection.get().is_none()>
                                                <div class="empty-state">
                                                    <p>"Select a collection to view documents"</p>
                                                </div>
                                            </Show>
                                        </div>
                                    </div>
                                </Show>
                            </div>
                        </Show>

                        // Graph Explorer Modal
                        <Show when=move || active_modal.get() == BrowserModal::Graph>
                            <div class="modal-header">
                                <h2>"Graph Explorer"</h2>
                                <button class="modal-close" on:click=move |_| set_active_modal.set(BrowserModal::None)>"x"</button>
                            </div>
                            <div class="modal-body">
                                <Show when=move || modal_loading.get()>
                                    <div class="modal-loading">
                                        <div class="spinner"></div>
                                        <p>"Loading graph data..."</p>
                                    </div>
                                </Show>
                                <Show when=move || !modal_loading.get()>
                                    <div class="graph-explorer">
                                        <div class="graph-toolbar">
                                            <input
                                                type="text"
                                                class="search-input"
                                                placeholder="Search nodes..."
                                                prop:value=move || graph_search.get()
                                                on:input=move |ev| set_graph_search.set(event_target_value(&ev))
                                            />
                                            <select
                                                class="graph-select"
                                                prop:value=move || graph_label_filter.get()
                                                on:change=move |ev| set_graph_label_filter.set(event_target_value(&ev))
                                            >
                                                <option value="">"All Labels"</option>
                                                <option value="User">"User"</option>
                                                <option value="Product">"Product"</option>
                                                <option value="Order">"Order"</option>
                                            </select>
                                            <button class="toolbar-btn" on:click=move |_| {
                                                set_graph_search.set(String::new());
                                                set_graph_label_filter.set(String::new());
                                                set_graph_selected_node.set(None);
                                            }>"Reset View"</button>
                                            <button class="toolbar-btn" on:click=move |_| load_graph.dispatch(())>"Refresh"</button>
                                        </div>
                                        <div class="graph-container">
                                            {move || graph_data.get().map(|data| {
                                                view! {
                                                    <div class="graph-stats">
                                                        <div class="graph-stat">
                                                            <span class="stat-value">{data.nodes.len()}</span>
                                                            <span class="stat-label">"Nodes"</span>
                                                        </div>
                                                        <div class="graph-stat">
                                                            <span class="stat-value">{data.edges.len()}</span>
                                                            <span class="stat-label">"Edges"</span>
                                                        </div>
                                                    </div>
                                                    <div class="graph-visualization">
                                                        <div class="graph-placeholder">
                                                            <p>"Graph visualization"</p>
                                                            <p class="hint">"Nodes and edges loaded. Visualization requires WebGL canvas."</p>
                                                        </div>
                                                    </div>
                                                    <div class="graph-details">
                                                        <h4>"Nodes"</h4>
                                                        <div class="node-list">
                                                            {data.nodes.iter().take(10).map(|node| {
                                                                view! {
                                                                    <div class="node-item">
                                                                        <span class="node-id">{node.id.clone()}</span>
                                                                        <span class="node-label">{node.label.clone()}</span>
                                                                    </div>
                                                                }
                                                            }).collect_view()}
                                                        </div>
                                                    </div>
                                                }
                                            })}
                                        </div>
                                    </div>
                                </Show>
                            </div>
                        </Show>

                        // Query Builder Modal
                        <Show when=move || active_modal.get() == BrowserModal::QueryBuilder>
                            <div class="modal-header">
                                <h2>"Query Builder"</h2>
                                <button class="modal-close" on:click=move |_| set_active_modal.set(BrowserModal::None)>"x"</button>
                            </div>
                            <div class="modal-body">
                                <div class="query-builder">
                                    <div class="query-input-section">
                                        <div class="query-tabs">
                                            <button
                                                class=move || if query_tab.get() == "sql" { "query-tab active" } else { "query-tab" }
                                                on:click=move |_| {
                                                    set_query_tab.set("sql".to_string());
                                                    set_query_input.set(String::new());
                                                }
                                            >"SQL"</button>
                                            <button
                                                class=move || if query_tab.get() == "graphql" { "query-tab active" } else { "query-tab" }
                                                on:click=move |_| {
                                                    set_query_tab.set("graphql".to_string());
                                                    set_query_input.set(String::new());
                                                }
                                            >"GraphQL"</button>
                                            <button
                                                class=move || if query_tab.get() == "cypher" { "query-tab active" } else { "query-tab" }
                                                on:click=move |_| {
                                                    set_query_tab.set("cypher".to_string());
                                                    set_query_input.set(String::new());
                                                }
                                            >"Cypher"</button>
                                        </div>
                                        <textarea
                                            class="query-textarea"
                                            placeholder=move || match query_tab.get().as_str() {
                                                "sql" => "Enter SQL query...\n\nExample: SELECT * FROM users WHERE active = true LIMIT 10",
                                                "graphql" => "Enter GraphQL query...\n\nExample:\nquery {\n  users(limit: 10) {\n    id\n    name\n    email\n  }\n}",
                                                "cypher" => "Enter Cypher query...\n\nExample: MATCH (n:User)-[:FOLLOWS]->(m:User) RETURN n.name, m.name LIMIT 10",
                                                _ => "Enter your query..."
                                            }
                                            prop:value=move || query_input.get()
                                            on:input=move |ev| {
                                                set_query_input.set(event_target_value(&ev));
                                            }
                                        ></textarea>
                                        <div class="query-actions">
                                            <span class="query-type-badge">{move || query_tab.get().to_uppercase()}</span>
                                            <button
                                                class="toolbar-btn primary"
                                                on:click=move |_| {
                                                    let q = query_input.get();
                                                    if !q.is_empty() {
                                                        // Add to history
                                                        let mut history = query_history.get();
                                                        if !history.contains(&q) {
                                                            history.insert(0, q.clone());
                                                            if history.len() > 10 { history.pop(); }
                                                            set_query_history.set(history);
                                                        }
                                                        execute_query.dispatch(q);
                                                    }
                                                }
                                            >"Execute"</button>
                                            <button class="toolbar-btn" on:click=move |_| {
                                                set_query_input.set(String::new());
                                                set_query_result.set(None);
                                            }>"Clear"</button>
                                        </div>
                                        // Query History
                                        <Show when=move || !query_history.get().is_empty()>
                                            <div class="query-history">
                                                <span class="history-label">"Recent queries:"</span>
                                                <div class="history-items">
                                                    {move || query_history.get().iter().take(5).map(|q| {
                                                        let query = q.clone();
                                                        let display = if q.len() > 50 { format!("{}...", &q[..50]) } else { q.clone() };
                                                        view! {
                                                            <button
                                                                class="history-item"
                                                                on:click=move |_| set_query_input.set(query.clone())
                                                            >{display}</button>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        </Show>
                                    </div>
                                    <div class="query-results-section">
                                        <Show when=move || modal_loading.get()>
                                            <div class="modal-loading">
                                                <div class="spinner"></div>
                                                <p>"Executing query..."</p>
                                            </div>
                                        </Show>
                                        <Show when=move || !modal_loading.get()>
                                            {move || query_result.get().map(|result| {
                                                if result.success {
                                                    let has_columns = !result.columns.is_empty();
                                                    let columns = result.columns.clone();
                                                    let rows = result.rows.clone();
                                                    let row_count = rows.len();
                                                    let exec_time = result.execution_time_ms;
                                                    let rows_affected = result.rows_affected;
                                                    view! {
                                                        <div class="query-success">
                                                            <div class="result-stats">
                                                                <span>"Rows: "{row_count}</span>
                                                                <span>"Time: "{exec_time}"ms"</span>
                                                            </div>
                                                            {if has_columns {
                                                                view! {
                                                                    <table class="data-table result-table">
                                                                        <thead>
                                                                            <tr>
                                                                                {columns.iter().map(|col| {
                                                                                    view! { <th>{col.clone()}</th> }
                                                                                }).collect_view()}
                                                                            </tr>
                                                                        </thead>
                                                                        <tbody>
                                                                            {rows.iter().map(|row| {
                                                                                view! {
                                                                                    <tr>
                                                                                        {row.iter().map(|cell| {
                                                                                            let cell_str = match cell {
                                                                                                serde_json::Value::Null => "NULL".to_string(),
                                                                                                serde_json::Value::String(s) => s.clone(),
                                                                                                v => v.to_string(),
                                                                                            };
                                                                                            view! { <td>{cell_str}</td> }
                                                                                        }).collect_view()}
                                                                                    </tr>
                                                                                }
                                                                            }).collect_view()}
                                                                        </tbody>
                                                                    </table>
                                                                }.into_view()
                                                            } else {
                                                                view! {
                                                                    <div class="result-message">
                                                                        <p>{format!("{} rows affected", rows_affected)}</p>
                                                                    </div>
                                                                }.into_view()
                                                            }}
                                                        </div>
                                                    }.into_view()
                                                } else {
                                                    let err_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
                                                    view! {
                                                        <div class="query-error">
                                                            <span class="error-icon">"Error"</span>
                                                            <pre class="error-message">{err_msg}</pre>
                                                        </div>
                                                    }.into_view()
                                                }
                                            })}
                                            <Show when=move || query_result.get().is_none()>
                                                <div class="empty-state">
                                                    <p>"Enter a query and click Execute to see results"</p>
                                                </div>
                                            </Show>
                                        </Show>
                                    </div>
                                </div>
                            </div>
                        </Show>

                        // Node Detail Modal
                        <Show when=move || active_modal.get() == BrowserModal::NodeDetail>
                            <div class="modal-header">
                                <h2>"Node Details"</h2>
                                <button class="modal-close" on:click=move |_| {
                                    set_active_modal.set(BrowserModal::None);
                                    set_selected_node.set(None);
                                }>"x"</button>
                            </div>
                            <div class="modal-body">
                                {move || selected_node.get().map(|node| {
                                    let status_class = match node.status {
                                        NodeStatus::Healthy => "healthy",
                                        NodeStatus::Degraded => "degraded",
                                        NodeStatus::Offline => "offline",
                                    };
                                    let role_text = match node.role {
                                        NodeRole::Leader => "Leader",
                                        NodeRole::Follower => "Follower",
                                        NodeRole::Candidate => "Candidate",
                                    };
                                    let uptime_str = {
                                        let days = node.uptime / 86400;
                                        let hours = (node.uptime % 86400) / 3600;
                                        let mins = (node.uptime % 3600) / 60;
                                        format!("{}d {}h {}m", days, hours, mins)
                                    };

                                    view! {
                                        <div class="node-detail-modal">
                                            // Node Header
                                            <div class="node-detail-header">
                                                <div class="node-identity">
                                                    <div class=format!("node-status-large {}", status_class)></div>
                                                    <div class="node-titles">
                                                        <h3>{node.id.clone()}</h3>
                                                        <span class="node-address-lg">{node.address.clone()}</span>
                                                    </div>
                                                </div>
                                                <div class="node-badges">
                                                    <span class=format!("status-badge-lg {}", status_class)>{format!("{:?}", node.status)}</span>
                                                    <span class="role-badge-lg">{role_text}</span>
                                                </div>
                                            </div>

                                            // Quick Stats
                                            <div class="node-quick-stats">
                                                <div class="quick-stat">
                                                    <span class="quick-value">{node.region.clone()}</span>
                                                    <span class="quick-label">"Region"</span>
                                                </div>
                                                <div class="quick-stat">
                                                    <span class="quick-value">{uptime_str}</span>
                                                    <span class="quick-label">"Uptime"</span>
                                                </div>
                                                <div class="quick-stat">
                                                    <span class="quick-value">{format_number(node.metrics.connections)}</span>
                                                    <span class="quick-label">"Connections"</span>
                                                </div>
                                                <div class="quick-stat">
                                                    <span class="quick-value">{format_number(node.metrics.ops_per_second)}<span class="unit">"/s"</span></span>
                                                    <span class="quick-label">"Operations"</span>
                                                </div>
                                            </div>

                                            // Resource Gauges
                                            <div class="node-resources">
                                                <h4>"Resource Utilization"</h4>
                                                <div class="resource-gauges">
                                                    <div class="resource-gauge">
                                                        <div class="gauge-header">
                                                            <span class="gauge-label">"CPU"</span>
                                                            <span class="gauge-value">{format!("{}%", node.metrics.cpu_usage as u32)}</span>
                                                        </div>
                                                        <div class="gauge-bar">
                                                            <div class="gauge-fill" style=format!("width: {}%", node.metrics.cpu_usage as u32)></div>
                                                        </div>
                                                    </div>
                                                    <div class="resource-gauge">
                                                        <div class="gauge-header">
                                                            <span class="gauge-label">"Memory"</span>
                                                            <span class="gauge-value">{format!("{}%", node.metrics.memory_usage as u32)}</span>
                                                        </div>
                                                        <div class="gauge-bar">
                                                            <div class="gauge-fill" style=format!("width: {}%", node.metrics.memory_usage as u32)></div>
                                                        </div>
                                                    </div>
                                                    <div class="resource-gauge">
                                                        <div class="gauge-header">
                                                            <span class="gauge-label">"Disk"</span>
                                                            <span class="gauge-value">{format!("{}%", node.metrics.disk_usage as u32)}</span>
                                                        </div>
                                                        <div class="gauge-bar">
                                                            <div class="gauge-fill" style=format!("width: {}%", node.metrics.disk_usage as u32)></div>
                                                        </div>
                                                    </div>
                                                </div>
                                            </div>

                                            // Performance Metrics
                                            <div class="node-performance">
                                                <h4>"Performance Metrics"</h4>
                                                <div class="perf-grid">
                                                    <div class="perf-item">
                                                        <span class="perf-label">"Latency (p50)"</span>
                                                        <span class="perf-value">{format!("{:.2}ms", node.metrics.latency_p50)}</span>
                                                    </div>
                                                    <div class="perf-item">
                                                        <span class="perf-label">"Latency (p99)"</span>
                                                        <span class="perf-value">{format!("{:.2}ms", node.metrics.latency_p99)}</span>
                                                    </div>
                                                    <div class="perf-item">
                                                        <span class="perf-label">"Network In"</span>
                                                        <span class="perf-value">{format_bytes(node.metrics.network_in)}</span>
                                                    </div>
                                                    <div class="perf-item">
                                                        <span class="perf-label">"Network Out"</span>
                                                        <span class="perf-value">{format_bytes(node.metrics.network_out)}</span>
                                                    </div>
                                                </div>
                                            </div>

                                            // Action Message
                                            {move || node_action_message.get().map(|msg| view! {
                                                <div class="node-action-message">
                                                    <span>{msg}</span>
                                                    <button class="dismiss-btn" on:click=move |_| set_node_action_message.set(None)>"x"</button>
                                                </div>
                                            })}

                                            // Logs View
                                            <Show when=move || show_logs_view.get()>
                                                <div class="node-logs-section">
                                                    <div class="logs-header">
                                                        <h4>"Node Logs"</h4>
                                                        <button class="close-logs-btn" on:click=move |_| set_show_logs_view.set(false)>"Close Logs"</button>
                                                    </div>
                                                    <div class="logs-container">
                                                        <For
                                                            each=move || node_logs.get()
                                                            key=|log| format!("{}-{}", log.timestamp, log.message.clone())
                                                            children=move |log| {
                                                                let level_class = match log.level.as_str() {
                                                                    "ERROR" => "log-error",
                                                                    "WARN" => "log-warn",
                                                                    "INFO" => "log-info",
                                                                    _ => "log-debug",
                                                                };
                                                                view! {
                                                                    <div class=format!("log-entry {}", level_class)>
                                                                        <span class="log-timestamp">{log.timestamp.clone()}</span>
                                                                        <span class="log-level">{log.level.clone()}</span>
                                                                        <span class="log-message">{log.message.clone()}</span>
                                                                    </div>
                                                                }
                                                            }
                                                        />
                                                    </div>
                                                </div>
                                            </Show>

                                            // Node Actions
                                            {
                                                let node_id_for_logs = node.id.clone();
                                                let node_id_for_drain = node.id.clone();
                                                let node_id_for_restart = node.id.clone();
                                                let node_id_for_remove = node.id.clone();
                                                view! {
                                                    <div class="node-actions-section">
                                                        <h4>"Actions"</h4>
                                                        <div class="action-buttons-row">
                                                            <button
                                                                class="action-btn primary"
                                                                on:click=move |_| {
                                                                    load_node_logs.dispatch(node_id_for_logs.clone());
                                                                }
                                                            >"View Logs"</button>
                                                            <button
                                                                class="action-btn"
                                                                on:click=move |_| {
                                                                    drain_node_action.dispatch(node_id_for_drain.clone());
                                                                }
                                                            >"Drain Node"</button>
                                                            <button
                                                                class="action-btn"
                                                                on:click=move |_| {
                                                                    restart_node_action.dispatch(node_id_for_restart.clone());
                                                                }
                                                            >"Restart"</button>
                                                            <button
                                                                class="action-btn danger"
                                                                on:click=move |_| {
                                                                    if web_sys::window()
                                                                        .and_then(|w| w.confirm_with_message(&format!("Are you sure you want to remove {} from the cluster? This action cannot be undone.", node_id_for_remove)).ok())
                                                                        .unwrap_or(false)
                                                                    {
                                                                        remove_node_action.dispatch(node_id_for_remove.clone());
                                                                    }
                                                                }
                                                            >"Remove from Cluster"</button>
                                                        </div>
                                                    </div>
                                                }
                                            }
                                        </div>
                                    }
                                })}
                            </div>
                        </Show>
                    </div>
                </div>
            </Show>
        </div>
    }
}

// Format helpers
fn format_number(num: u64) -> String {
    if num >= 1_000_000 { format!("{:.1}M", num as f64 / 1_000_000.0) }
    else if num >= 1_000 { format!("{:.1}K", num as f64 / 1_000.0) }
    else { num.to_string() }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_099_511_627_776 { format!("{:.1} TB", bytes as f64 / 1_099_511_627_776.0) }
    else if bytes >= 1_073_741_824 { format!("{:.1} GB", bytes as f64 / 1_073_741_824.0) }
    else if bytes >= 1_048_576 { format!("{:.1} MB", bytes as f64 / 1_048_576.0) }
    else { format!("{:.1} KB", bytes as f64 / 1_024.0) }
}
