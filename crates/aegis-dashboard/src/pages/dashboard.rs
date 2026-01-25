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
    DataVisualizer,
}

/// Chart types for data visualization
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum DataChartType {
    #[default]
    Scatter3D,
    ScatterPlot,
    LineChart,
    BarChart,
    AreaChart,
    RadarChart,
    HeatMap,
    BubbleChart,
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

    // Data Visualizer state
    let (viz_chart_type, set_viz_chart_type) = create_signal(DataChartType::Scatter3D);
    let (viz_query, set_viz_query) = create_signal(String::new());
    let (_viz_data, set_viz_data) = create_signal::<Option<QueryBuilderResult>>(None);
    let (viz_x_column, set_viz_x_column) = create_signal(String::new());
    let (viz_y_column, set_viz_y_column) = create_signal(String::new());
    let (viz_z_column, set_viz_z_column) = create_signal(String::new());
    let (_viz_color_column, set_viz_color_column) = create_signal(String::new());
    let (_viz_size_column, set_viz_size_column) = create_signal(String::new());
    let (viz_loading, set_viz_loading) = create_signal(false);

    // Metrics page state
    let (metrics_active_tab, set_metrics_active_tab) = create_signal("overview".to_string());

    // Settings page state
    let (settings_tab, set_settings_tab) = create_signal("general".to_string());
    let (show_add_user, set_show_add_user) = create_signal(false);
    let (edit_user_id, set_edit_user_id) = create_signal::<Option<String>>(None);
    let (edit_user_name, set_edit_user_name) = create_signal(String::new());
    let (edit_user_email, set_edit_user_email) = create_signal(String::new());
    let (edit_user_role, set_edit_user_role) = create_signal(String::new());
    let (edit_user_2fa, set_edit_user_2fa) = create_signal(false);
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
                    <img src="AegisDB-logo.png" alt="Aegis DB Logo" class="sidebar-logo" width="36" height="36"/>
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
                                        <th>"Uptime"</th>
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
                                        let uptime_display = format_uptime(node.uptime);
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
                                                <td class="monospace">{uptime_display.clone()}</td>
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

                            <div class="paradigm-card featured">
                                <div class="paradigm-header">
                                    <span class="paradigm-icon">"📊"</span>
                                    <span class="paradigm-name">"Data Visualizer"</span>
                                    <span class="paradigm-badge">"NEW"</span>
                                </div>
                                <div class="paradigm-stats">
                                    <div class="paradigm-stat">
                                        <span class="stat-value">"8"</span>
                                        <span class="stat-label">"Chart Types"</span>
                                    </div>
                                    <div class="paradigm-stat">
                                        <span class="stat-value">"3D"</span>
                                        <span class="stat-label">"Scatter"</span>
                                    </div>
                                </div>
                                <button class="paradigm-action primary" on:click=move |_| {
                                    set_active_modal.set(BrowserModal::DataVisualizer);
                                    set_viz_data.set(None);
                                }>"Visualize Data"</button>
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
                                        let used_pct = if stats.storage_total > 0 {
                                            (stats.storage_used as f64 / stats.storage_total as f64 * 100.0) as u32
                                        } else { 0 };
                                        let wal_pct = if stats.storage_used > 0 {
                                            (stats.wal_bytes as f64 / stats.storage_used as f64 * used_pct as f64) as u32
                                        } else { 0 };
                                        let data_pct = used_pct.saturating_sub(wal_pct);
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
                                                        <span class="storage-size">{format_bytes(stats.data_bytes)}</span>
                                                    </div>
                                                    <div class="storage-row">
                                                        <div class="storage-color wal"></div>
                                                        <span class="storage-name">"WAL"</span>
                                                        <span class="storage-size">{format_bytes(stats.wal_bytes)}</span>
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

                        // Performance Tab Content - STUNNING CHARTS
                        <Show when=move || metrics_active_tab.get() == "performance">
                            <div class="metrics-tab-content performance-showcase">
                                <div class="performance-grid-advanced">

                                    // STACKED WAVE AREA CHART - Full Width
                                    <div class="chart-card chart-fullwidth glass-effect">
                                        <div class="chart-header premium">
                                            <div class="chart-title-group">
                                                <h3>"Real-Time Operations Flow"</h3>
                                                <span class="live-indicator"><span class="pulse-dot"></span>"LIVE"</span>
                                            </div>
                                            <div class="chart-legend-inline">
                                                <span class="legend-item"><span class="dot reads"></span>"Reads"</span>
                                                <span class="legend-item"><span class="dot writes"></span>"Writes"</span>
                                                <span class="legend-item"><span class="dot deletes"></span>"Deletes"</span>
                                                <span class="legend-item"><span class="dot scans"></span>"Scans"</span>
                                            </div>
                                        </div>
                                        <div class="stacked-wave-container">
                                            <svg class="stacked-wave-chart" viewBox="0 0 800 280" preserveAspectRatio="none">
                                                <defs>
                                                    // Gradients for each layer
                                                    <linearGradient id="readGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                                                        <stop offset="0%" stop-color="#14b8a6" stop-opacity="0.9"/>
                                                        <stop offset="100%" stop-color="#14b8a6" stop-opacity="0.3"/>
                                                    </linearGradient>
                                                    <linearGradient id="writeGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                                                        <stop offset="0%" stop-color="#8b5cf6" stop-opacity="0.85"/>
                                                        <stop offset="100%" stop-color="#8b5cf6" stop-opacity="0.25"/>
                                                    </linearGradient>
                                                    <linearGradient id="deleteGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                                                        <stop offset="0%" stop-color="#f97316" stop-opacity="0.8"/>
                                                        <stop offset="100%" stop-color="#f97316" stop-opacity="0.2"/>
                                                    </linearGradient>
                                                    <linearGradient id="scanGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                                                        <stop offset="0%" stop-color="#3b82f6" stop-opacity="0.75"/>
                                                        <stop offset="100%" stop-color="#3b82f6" stop-opacity="0.15"/>
                                                    </linearGradient>
                                                    // Glow filter
                                                    <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
                                                        <feGaussianBlur stdDeviation="3" result="blur"/>
                                                        <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
                                                    </filter>
                                                </defs>
                                                // Stacked areas with smooth wave effect
                                                <path d="M0,260 C40,255 80,245 120,235 C160,225 200,215 240,200 C280,185 320,165 360,155 C400,145 440,140 480,135 C520,130 560,128 600,125 C640,122 680,120 720,118 C760,116 800,115 800,115 L800,280 L0,280 Z" fill="url(#scanGrad)" class="wave-layer scan"/>
                                                <path d="M0,240 C40,230 80,215 120,195 C160,175 200,160 240,150 C280,140 320,130 360,120 C400,110 440,105 480,100 C520,95 560,92 600,90 C640,88 680,86 720,85 C760,84 800,83 800,83 L800,280 L0,280 Z" fill="url(#deleteGrad)" class="wave-layer delete"/>
                                                <path d="M0,200 C40,185 80,160 120,140 C160,120 200,105 240,95 C280,85 320,78 360,72 C400,66 440,62 480,58 C520,54 560,52 600,50 C640,48 680,47 720,46 C760,45 800,45 800,45 L800,280 L0,280 Z" fill="url(#writeGrad)" class="wave-layer write"/>
                                                <path d="M0,160 C40,140 80,110 120,85 C160,60 200,45 240,38 C280,31 320,28 360,26 C400,24 440,23 480,22 C520,21 560,20 600,20 C640,20 680,20 720,20 C760,20 800,20 800,20 L800,280 L0,280 Z" fill="url(#readGrad)" class="wave-layer read"/>
                                                // Glowing top lines
                                                <path d="M0,160 C40,140 80,110 120,85 C160,60 200,45 240,38 C280,31 320,28 360,26 C400,24 440,23 480,22 C520,21 560,20 600,20 C640,20 680,20 720,20 C760,20 800,20" fill="none" stroke="#14b8a6" stroke-width="2" filter="url(#glow)" class="glow-line"/>
                                            </svg>
                                            <div class="wave-y-axis">
                                                <span>"100K"</span><span>"75K"</span><span>"50K"</span><span>"25K"</span><span>"0"</span>
                                            </div>
                                        </div>
                                    </div>

                                    // RADAR CHART - Performance Dimensions
                                    <div class="chart-card glass-effect">
                                        <div class="chart-header premium">
                                            <h3>"Performance Radar"</h3>
                                            <span class="chart-subtitle">"Multi-dimensional analysis"</span>
                                        </div>
                                        <div class="radar-chart-container">
                                            <svg class="radar-chart" viewBox="0 0 300 300">
                                                <defs>
                                                    <radialGradient id="radarFill" cx="50%" cy="50%" r="50%">
                                                        <stop offset="0%" stop-color="#14b8a6" stop-opacity="0.6"/>
                                                        <stop offset="100%" stop-color="#14b8a6" stop-opacity="0.1"/>
                                                    </radialGradient>
                                                </defs>
                                                // Background rings
                                                <circle cx="150" cy="150" r="120" fill="none" stroke="#374151" stroke-width="1" opacity="0.3"/>
                                                <circle cx="150" cy="150" r="90" fill="none" stroke="#374151" stroke-width="1" opacity="0.3"/>
                                                <circle cx="150" cy="150" r="60" fill="none" stroke="#374151" stroke-width="1" opacity="0.3"/>
                                                <circle cx="150" cy="150" r="30" fill="none" stroke="#374151" stroke-width="1" opacity="0.3"/>
                                                // Axis lines (6 dimensions)
                                                <line x1="150" y1="150" x2="150" y2="30" stroke="#4b5563" stroke-width="1"/>
                                                <line x1="150" y1="150" x2="254" y2="90" stroke="#4b5563" stroke-width="1"/>
                                                <line x1="150" y1="150" x2="254" y2="210" stroke="#4b5563" stroke-width="1"/>
                                                <line x1="150" y1="150" x2="150" y2="270" stroke="#4b5563" stroke-width="1"/>
                                                <line x1="150" y1="150" x2="46" y2="210" stroke="#4b5563" stroke-width="1"/>
                                                <line x1="150" y1="150" x2="46" y2="90" stroke="#4b5563" stroke-width="1"/>
                                                // Data polygon - current values
                                                <polygon points="150,42 238,98 238,198 150,258 62,198 62,98" fill="url(#radarFill)" stroke="#14b8a6" stroke-width="2"/>
                                                // Data points with glow
                                                <circle cx="150" cy="42" r="5" fill="#14b8a6" filter="url(#glow)"/>
                                                <circle cx="238" cy="98" r="5" fill="#14b8a6" filter="url(#glow)"/>
                                                <circle cx="238" cy="198" r="5" fill="#14b8a6" filter="url(#glow)"/>
                                                <circle cx="150" cy="258" r="5" fill="#14b8a6" filter="url(#glow)"/>
                                                <circle cx="62" cy="198" r="5" fill="#14b8a6" filter="url(#glow)"/>
                                                <circle cx="62" cy="98" r="5" fill="#14b8a6" filter="url(#glow)"/>
                                                // Labels
                                                <text x="150" y="18" text-anchor="middle" fill="#9ca3af" font-size="11" font-weight="500">"Throughput"</text>
                                                <text x="270" y="92" text-anchor="start" fill="#9ca3af" font-size="11" font-weight="500">"Latency"</text>
                                                <text x="270" y="215" text-anchor="start" fill="#9ca3af" font-size="11" font-weight="500">"Cache"</text>
                                                <text x="150" y="290" text-anchor="middle" fill="#9ca3af" font-size="11" font-weight="500">"Availability"</text>
                                                <text x="30" y="215" text-anchor="end" fill="#9ca3af" font-size="11" font-weight="500">"Efficiency"</text>
                                                <text x="30" y="92" text-anchor="end" fill="#9ca3af" font-size="11" font-weight="500">"Scalability"</text>
                                            </svg>
                                        </div>
                                    </div>

                                    // CORRELATION HEATMAP
                                    <div class="chart-card glass-effect">
                                        <div class="chart-header premium">
                                            <h3>"Metric Correlation Matrix"</h3>
                                            <span class="chart-subtitle">"Cross-correlation analysis"</span>
                                        </div>
                                        <div class="heatmap-container">
                                            <svg class="heatmap-chart" viewBox="0 0 280 280">
                                                // Column headers
                                                <text x="70" y="20" text-anchor="middle" fill="#9ca3af" font-size="9" transform="rotate(-45,70,20)">"CPU"</text>
                                                <text x="110" y="20" text-anchor="middle" fill="#9ca3af" font-size="9" transform="rotate(-45,110,20)">"Memory"</text>
                                                <text x="150" y="20" text-anchor="middle" fill="#9ca3af" font-size="9" transform="rotate(-45,150,20)">"Latency"</text>
                                                <text x="190" y="20" text-anchor="middle" fill="#9ca3af" font-size="9" transform="rotate(-45,190,20)">"Ops/s"</text>
                                                <text x="230" y="20" text-anchor="middle" fill="#9ca3af" font-size="9" transform="rotate(-45,230,20)">"Cache"</text>
                                                // Row headers
                                                <text x="45" y="70" text-anchor="end" fill="#9ca3af" font-size="10">"CPU"</text>
                                                <text x="45" y="110" text-anchor="end" fill="#9ca3af" font-size="10">"Memory"</text>
                                                <text x="45" y="150" text-anchor="end" fill="#9ca3af" font-size="10">"Latency"</text>
                                                <text x="45" y="190" text-anchor="end" fill="#9ca3af" font-size="10">"Ops/s"</text>
                                                <text x="45" y="230" text-anchor="end" fill="#9ca3af" font-size="10">"Cache"</text>
                                                // Heatmap cells (5x5 grid)
                                                // Row 1 - CPU correlations
                                                <rect x="50" y="50" width="35" height="35" fill="#14b8a6" rx="4"/><text x="68" y="72" text-anchor="middle" fill="#fff" font-size="10" font-weight="600">"1.0"</text>
                                                <rect x="90" y="50" width="35" height="35" fill="#22c55e" rx="4"/><text x="108" y="72" text-anchor="middle" fill="#fff" font-size="10">"0.8"</text>
                                                <rect x="130" y="50" width="35" height="35" fill="#f97316" rx="4"/><text x="148" y="72" text-anchor="middle" fill="#fff" font-size="10">"0.6"</text>
                                                <rect x="170" y="50" width="35" height="35" fill="#22c55e" rx="4"/><text x="188" y="72" text-anchor="middle" fill="#fff" font-size="10">"0.7"</text>
                                                <rect x="210" y="50" width="35" height="35" fill="#eab308" rx="4"/><text x="228" y="72" text-anchor="middle" fill="#fff" font-size="10">"0.4"</text>
                                                // Row 2 - Memory correlations
                                                <rect x="50" y="90" width="35" height="35" fill="#22c55e" rx="4"/><text x="68" y="112" text-anchor="middle" fill="#fff" font-size="10">"0.8"</text>
                                                <rect x="90" y="90" width="35" height="35" fill="#14b8a6" rx="4"/><text x="108" y="112" text-anchor="middle" fill="#fff" font-size="10" font-weight="600">"1.0"</text>
                                                <rect x="130" y="90" width="35" height="35" fill="#ef4444" rx="4"/><text x="148" y="112" text-anchor="middle" fill="#fff" font-size="10">"0.9"</text>
                                                <rect x="170" y="90" width="35" height="35" fill="#22c55e" rx="4"/><text x="188" y="112" text-anchor="middle" fill="#fff" font-size="10">"0.6"</text>
                                                <rect x="210" y="90" width="35" height="35" fill="#f97316" rx="4"/><text x="228" y="112" text-anchor="middle" fill="#fff" font-size="10">"0.5"</text>
                                                // Row 3 - Latency correlations
                                                <rect x="50" y="130" width="35" height="35" fill="#f97316" rx="4"/><text x="68" y="152" text-anchor="middle" fill="#fff" font-size="10">"0.6"</text>
                                                <rect x="90" y="130" width="35" height="35" fill="#ef4444" rx="4"/><text x="108" y="152" text-anchor="middle" fill="#fff" font-size="10">"0.9"</text>
                                                <rect x="130" y="130" width="35" height="35" fill="#14b8a6" rx="4"/><text x="148" y="152" text-anchor="middle" fill="#fff" font-size="10" font-weight="600">"1.0"</text>
                                                <rect x="170" y="130" width="35" height="35" fill="#ef4444" rx="4"/><text x="188" y="152" text-anchor="middle" fill="#fff" font-size="10">"-0.7"</text>
                                                <rect x="210" y="130" width="35" height="35" fill="#ef4444" rx="4"/><text x="228" y="152" text-anchor="middle" fill="#fff" font-size="10">"-0.8"</text>
                                                // Row 4 - Ops/s correlations
                                                <rect x="50" y="170" width="35" height="35" fill="#22c55e" rx="4"/><text x="68" y="192" text-anchor="middle" fill="#fff" font-size="10">"0.7"</text>
                                                <rect x="90" y="170" width="35" height="35" fill="#22c55e" rx="4"/><text x="108" y="192" text-anchor="middle" fill="#fff" font-size="10">"0.6"</text>
                                                <rect x="130" y="170" width="35" height="35" fill="#ef4444" rx="4"/><text x="148" y="192" text-anchor="middle" fill="#fff" font-size="10">"-0.7"</text>
                                                <rect x="170" y="170" width="35" height="35" fill="#14b8a6" rx="4"/><text x="188" y="192" text-anchor="middle" fill="#fff" font-size="10" font-weight="600">"1.0"</text>
                                                <rect x="210" y="170" width="35" height="35" fill="#22c55e" rx="4"/><text x="228" y="192" text-anchor="middle" fill="#fff" font-size="10">"0.8"</text>
                                                // Row 5 - Cache correlations
                                                <rect x="50" y="210" width="35" height="35" fill="#eab308" rx="4"/><text x="68" y="232" text-anchor="middle" fill="#fff" font-size="10">"0.4"</text>
                                                <rect x="90" y="210" width="35" height="35" fill="#f97316" rx="4"/><text x="108" y="232" text-anchor="middle" fill="#fff" font-size="10">"0.5"</text>
                                                <rect x="130" y="210" width="35" height="35" fill="#ef4444" rx="4"/><text x="148" y="232" text-anchor="middle" fill="#fff" font-size="10">"-0.8"</text>
                                                <rect x="170" y="210" width="35" height="35" fill="#22c55e" rx="4"/><text x="188" y="232" text-anchor="middle" fill="#fff" font-size="10">"0.8"</text>
                                                <rect x="210" y="210" width="35" height="35" fill="#14b8a6" rx="4"/><text x="228" y="232" text-anchor="middle" fill="#fff" font-size="10" font-weight="600">"1.0"</text>
                                            </svg>
                                            <div class="heatmap-legend">
                                                <span class="legend-label">"-1"</span>
                                                <div class="legend-gradient"></div>
                                                <span class="legend-label">"+1"</span>
                                            </div>
                                        </div>
                                    </div>

                                    // ANIMATED GAUGE CLUSTER
                                    <div class="chart-card glass-effect">
                                        <div class="chart-header premium">
                                            <h3>"System Health Gauges"</h3>
                                        </div>
                                        <div class="gauge-cluster">
                                            // CPU Gauge
                                            <div class="gauge-item">
                                                <svg class="gauge-svg" viewBox="0 0 120 80">
                                                    <defs>
                                                        <linearGradient id="gaugeGreen" x1="0%" y1="0%" x2="100%" y2="0%">
                                                            <stop offset="0%" stop-color="#22c55e"/>
                                                            <stop offset="50%" stop-color="#eab308"/>
                                                            <stop offset="100%" stop-color="#ef4444"/>
                                                        </linearGradient>
                                                    </defs>
                                                    // Background arc
                                                    <path d="M15,70 A45,45 0 0,1 105,70" fill="none" stroke="#374151" stroke-width="10" stroke-linecap="round"/>
                                                    // Value arc (animated)
                                                    {move || {
                                                        let cpu = nodes.get().first().map(|n| n.metrics.cpu_usage).unwrap_or(42.0);
                                                        let arc_length = cpu / 100.0 * 141.37; // Half circumference
                                                        view! {
                                                            <path d="M15,70 A45,45 0 0,1 105,70" fill="none" stroke="url(#gaugeGreen)" stroke-width="10" stroke-linecap="round"
                                                                stroke-dasharray=format!("{} 200", arc_length) class="gauge-arc"/>
                                                        }
                                                    }}
                                                    // Needle
                                                    {move || {
                                                        let cpu = nodes.get().first().map(|n| n.metrics.cpu_usage).unwrap_or(42.0);
                                                        let angle = -90.0 + (cpu / 100.0 * 180.0);
                                                        view! {
                                                            <line x1="60" y1="70" x2="60" y2="30" stroke="#fff" stroke-width="2" stroke-linecap="round"
                                                                transform=format!("rotate({} 60 70)", angle) class="gauge-needle"/>
                                                        }
                                                    }}
                                                    <circle cx="60" cy="70" r="6" fill="#1f2937"/>
                                                    <circle cx="60" cy="70" r="3" fill="#14b8a6"/>
                                                </svg>
                                                <div class="gauge-label">"CPU"</div>
                                                <div class="gauge-value">{move || format!("{:.0}%", nodes.get().first().map(|n| n.metrics.cpu_usage).unwrap_or(42.0))}</div>
                                            </div>
                                            // Memory Gauge
                                            <div class="gauge-item">
                                                <svg class="gauge-svg" viewBox="0 0 120 80">
                                                    <path d="M15,70 A45,45 0 0,1 105,70" fill="none" stroke="#374151" stroke-width="10" stroke-linecap="round"/>
                                                    {move || {
                                                        let mem = nodes.get().first().map(|n| n.metrics.memory_usage).unwrap_or(68.0);
                                                        let arc_length = mem / 100.0 * 141.37;
                                                        view! {
                                                            <path d="M15,70 A45,45 0 0,1 105,70" fill="none" stroke="url(#gaugeGreen)" stroke-width="10" stroke-linecap="round"
                                                                stroke-dasharray=format!("{} 200", arc_length) class="gauge-arc"/>
                                                        }
                                                    }}
                                                    {move || {
                                                        let mem = nodes.get().first().map(|n| n.metrics.memory_usage).unwrap_or(68.0);
                                                        let angle = -90.0 + (mem / 100.0 * 180.0);
                                                        view! {
                                                            <line x1="60" y1="70" x2="60" y2="30" stroke="#fff" stroke-width="2" stroke-linecap="round"
                                                                transform=format!("rotate({} 60 70)", angle) class="gauge-needle"/>
                                                        }
                                                    }}
                                                    <circle cx="60" cy="70" r="6" fill="#1f2937"/>
                                                    <circle cx="60" cy="70" r="3" fill="#8b5cf6"/>
                                                </svg>
                                                <div class="gauge-label">"Memory"</div>
                                                <div class="gauge-value">{move || format!("{:.0}%", nodes.get().first().map(|n| n.metrics.memory_usage).unwrap_or(68.0))}</div>
                                            </div>
                                            // Disk Gauge
                                            <div class="gauge-item">
                                                <svg class="gauge-svg" viewBox="0 0 120 80">
                                                    <path d="M15,70 A45,45 0 0,1 105,70" fill="none" stroke="#374151" stroke-width="10" stroke-linecap="round"/>
                                                    {move || {
                                                        let disk = nodes.get().first().map(|n| n.metrics.disk_usage).unwrap_or(54.0);
                                                        let arc_length = disk / 100.0 * 141.37;
                                                        view! {
                                                            <path d="M15,70 A45,45 0 0,1 105,70" fill="none" stroke="url(#gaugeGreen)" stroke-width="10" stroke-linecap="round"
                                                                stroke-dasharray=format!("{} 200", arc_length) class="gauge-arc"/>
                                                        }
                                                    }}
                                                    {move || {
                                                        let disk = nodes.get().first().map(|n| n.metrics.disk_usage).unwrap_or(54.0);
                                                        let angle = -90.0 + (disk / 100.0 * 180.0);
                                                        view! {
                                                            <line x1="60" y1="70" x2="60" y2="30" stroke="#fff" stroke-width="2" stroke-linecap="round"
                                                                transform=format!("rotate({} 60 70)", angle) class="gauge-needle"/>
                                                        }
                                                    }}
                                                    <circle cx="60" cy="70" r="6" fill="#1f2937"/>
                                                    <circle cx="60" cy="70" r="3" fill="#f97316"/>
                                                </svg>
                                                <div class="gauge-label">"Disk"</div>
                                                <div class="gauge-value">{move || format!("{:.0}%", nodes.get().first().map(|n| n.metrics.disk_usage).unwrap_or(54.0))}</div>
                                            </div>
                                        </div>
                                    </div>

                                    // SPARKLINE GRID - Real-time micro-charts
                                    <div class="chart-card chart-wide glass-effect">
                                        <div class="chart-header premium">
                                            <h3>"Real-Time Metrics Sparklines"</h3>
                                            <span class="chart-subtitle">"Last 60 seconds"</span>
                                        </div>
                                        <div class="sparkline-grid">
                                            <div class="sparkline-item">
                                                <div class="sparkline-header">
                                                    <span class="sparkline-label">"Query Rate"</span>
                                                    <span class="sparkline-value positive">"12.4K/s"</span>
                                                </div>
                                                <svg class="sparkline" viewBox="0 0 200 40">
                                                    <defs>
                                                        <linearGradient id="sparkGrad1" x1="0%" y1="0%" x2="0%" y2="100%">
                                                            <stop offset="0%" stop-color="#14b8a6" stop-opacity="0.4"/>
                                                            <stop offset="100%" stop-color="#14b8a6" stop-opacity="0"/>
                                                        </linearGradient>
                                                    </defs>
                                                    <path d="M0,35 L10,32 L20,28 L30,30 L40,25 L50,22 L60,24 L70,20 L80,18 L90,22 L100,15 L110,12 L120,14 L130,10 L140,8 L150,12 L160,6 L170,8 L180,5 L190,8 L200,5 L200,40 L0,40 Z" fill="url(#sparkGrad1)"/>
                                                    <path d="M0,35 L10,32 L20,28 L30,30 L40,25 L50,22 L60,24 L70,20 L80,18 L90,22 L100,15 L110,12 L120,14 L130,10 L140,8 L150,12 L160,6 L170,8 L180,5 L190,8 L200,5" fill="none" stroke="#14b8a6" stroke-width="2"/>
                                                </svg>
                                            </div>
                                            <div class="sparkline-item">
                                                <div class="sparkline-header">
                                                    <span class="sparkline-label">"Response Time"</span>
                                                    <span class="sparkline-value">"2.1ms"</span>
                                                </div>
                                                <svg class="sparkline" viewBox="0 0 200 40">
                                                    <defs>
                                                        <linearGradient id="sparkGrad2" x1="0%" y1="0%" x2="0%" y2="100%">
                                                            <stop offset="0%" stop-color="#8b5cf6" stop-opacity="0.4"/>
                                                            <stop offset="100%" stop-color="#8b5cf6" stop-opacity="0"/>
                                                        </linearGradient>
                                                    </defs>
                                                    <path d="M0,20 L10,18 L20,22 L30,25 L40,20 L50,15 L60,18 L70,12 L80,15 L90,10 L100,14 L110,12 L120,16 L130,14 L140,10 L150,12 L160,8 L170,10 L180,6 L190,9 L200,8 L200,40 L0,40 Z" fill="url(#sparkGrad2)"/>
                                                    <path d="M0,20 L10,18 L20,22 L30,25 L40,20 L50,15 L60,18 L70,12 L80,15 L90,10 L100,14 L110,12 L120,16 L130,14 L140,10 L150,12 L160,8 L170,10 L180,6 L190,9 L200,8" fill="none" stroke="#8b5cf6" stroke-width="2"/>
                                                </svg>
                                            </div>
                                            <div class="sparkline-item">
                                                <div class="sparkline-header">
                                                    <span class="sparkline-label">"Error Rate"</span>
                                                    <span class="sparkline-value positive">"0.02%"</span>
                                                </div>
                                                <svg class="sparkline" viewBox="0 0 200 40">
                                                    <defs>
                                                        <linearGradient id="sparkGrad3" x1="0%" y1="0%" x2="0%" y2="100%">
                                                            <stop offset="0%" stop-color="#22c55e" stop-opacity="0.4"/>
                                                            <stop offset="100%" stop-color="#22c55e" stop-opacity="0"/>
                                                        </linearGradient>
                                                    </defs>
                                                    <path d="M0,38 L10,37 L20,38 L30,36 L40,38 L50,37 L60,38 L70,36 L80,38 L90,37 L100,38 L110,36 L120,38 L130,37 L140,38 L150,36 L160,38 L170,37 L180,38 L190,36 L200,38 L200,40 L0,40 Z" fill="url(#sparkGrad3)"/>
                                                    <path d="M0,38 L10,37 L20,38 L30,36 L40,38 L50,37 L60,38 L70,36 L80,38 L90,37 L100,38 L110,36 L120,38 L130,37 L140,38 L150,36 L160,38 L170,37 L180,38 L190,36 L200,38" fill="none" stroke="#22c55e" stroke-width="2"/>
                                                </svg>
                                            </div>
                                            <div class="sparkline-item">
                                                <div class="sparkline-header">
                                                    <span class="sparkline-label">"Connections"</span>
                                                    <span class="sparkline-value">"847"</span>
                                                </div>
                                                <svg class="sparkline" viewBox="0 0 200 40">
                                                    <defs>
                                                        <linearGradient id="sparkGrad4" x1="0%" y1="0%" x2="0%" y2="100%">
                                                            <stop offset="0%" stop-color="#f97316" stop-opacity="0.4"/>
                                                            <stop offset="100%" stop-color="#f97316" stop-opacity="0"/>
                                                        </linearGradient>
                                                    </defs>
                                                    <path d="M0,30 L10,28 L20,25 L30,28 L40,22 L50,20 L60,22 L70,18 L80,20 L90,15 L100,18 L110,14 L120,16 L130,12 L140,15 L150,10 L160,12 L170,8 L180,10 L190,6 L200,8 L200,40 L0,40 Z" fill="url(#sparkGrad4)"/>
                                                    <path d="M0,30 L10,28 L20,25 L30,28 L40,22 L50,20 L60,22 L70,18 L80,20 L90,15 L100,18 L110,14 L120,16 L130,12 L140,15 L150,10 L160,12 L170,8 L180,10 L190,6 L200,8" fill="none" stroke="#f97316" stroke-width="2"/>
                                                </svg>
                                            </div>
                                        </div>
                                    </div>

                                    // 3D BAR CHART EFFECT
                                    <div class="chart-card glass-effect">
                                        <div class="chart-header premium">
                                            <h3>"Query Distribution by Type"</h3>
                                        </div>
                                        <div class="bar3d-container">
                                            <svg class="bar3d-chart" viewBox="0 0 280 200">
                                                <defs>
                                                    <linearGradient id="bar3d1" x1="0%" y1="0%" x2="100%" y2="0%">
                                                        <stop offset="0%" stop-color="#14b8a6"/>
                                                        <stop offset="100%" stop-color="#0d9488"/>
                                                    </linearGradient>
                                                    <linearGradient id="bar3dTop1" x1="0%" y1="0%" x2="100%" y2="100%">
                                                        <stop offset="0%" stop-color="#2dd4bf"/>
                                                        <stop offset="100%" stop-color="#14b8a6"/>
                                                    </linearGradient>
                                                    <linearGradient id="bar3dSide1" x1="0%" y1="0%" x2="100%" y2="0%">
                                                        <stop offset="0%" stop-color="#0f766e"/>
                                                        <stop offset="100%" stop-color="#115e59"/>
                                                    </linearGradient>
                                                </defs>
                                                // SELECT bar (tallest)
                                                <polygon points="30,180 30,50 60,50 60,180" fill="url(#bar3d1)"/>
                                                <polygon points="60,50 75,40 75,170 60,180" fill="url(#bar3dSide1)"/>
                                                <polygon points="30,50 45,40 75,40 60,50" fill="url(#bar3dTop1)"/>
                                                <text x="52" y="195" text-anchor="middle" fill="#9ca3af" font-size="10">"SELECT"</text>
                                                <text x="52" y="42" text-anchor="middle" fill="#fff" font-size="10" font-weight="600">"45%"</text>
                                                // INSERT bar
                                                <polygon points="95,180 95,80 125,80 125,180" fill="#8b5cf6"/>
                                                <polygon points="125,80 140,70 140,170 125,180" fill="#6d28d9"/>
                                                <polygon points="95,80 110,70 140,70 125,80" fill="#a78bfa"/>
                                                <text x="117" y="195" text-anchor="middle" fill="#9ca3af" font-size="10">"INSERT"</text>
                                                <text x="117" y="72" text-anchor="middle" fill="#fff" font-size="10" font-weight="600">"28%"</text>
                                                // UPDATE bar
                                                <polygon points="160,180 160,110 190,110 190,180" fill="#f97316"/>
                                                <polygon points="190,110 205,100 205,170 190,180" fill="#c2410c"/>
                                                <polygon points="160,110 175,100 205,100 190,110" fill="#fb923c"/>
                                                <text x="182" y="195" text-anchor="middle" fill="#9ca3af" font-size="10">"UPDATE"</text>
                                                <text x="182" y="102" text-anchor="middle" fill="#fff" font-size="10" font-weight="600">"18%"</text>
                                                // DELETE bar (shortest)
                                                <polygon points="225,180 225,140 255,140 255,180" fill="#ef4444"/>
                                                <polygon points="255,140 270,130 270,170 255,180" fill="#b91c1c"/>
                                                <polygon points="225,140 240,130 270,130 255,140" fill="#f87171"/>
                                                <text x="247" y="195" text-anchor="middle" fill="#9ca3af" font-size="10">"DELETE"</text>
                                                <text x="247" y="132" text-anchor="middle" fill="#fff" font-size="10" font-weight="600">"9%"</text>
                                            </svg>
                                        </div>
                                    </div>

                                    // 3D SCATTER PLOT - Cross-Correlation Analysis
                                    <div class="chart-card chart-wide glass-effect">
                                        <div class="chart-header premium">
                                            <div class="chart-title-group">
                                                <h3>"3D Cross-Correlation Scatter"</h3>
                                                <span class="chart-subtitle">"CPU vs Memory vs Latency"</span>
                                            </div>
                                            <div class="scatter-legend">
                                                <span class="scatter-legend-item"><span class="dot healthy"></span>"Healthy"</span>
                                                <span class="scatter-legend-item"><span class="dot warning"></span>"Warning"</span>
                                                <span class="scatter-legend-item"><span class="dot critical"></span>"Critical"</span>
                                            </div>
                                        </div>
                                        <div class="scatter3d-container">
                                            <svg class="scatter3d-chart" viewBox="0 0 500 350">
                                                <defs>
                                                    // Gradients for depth perception
                                                    <radialGradient id="scatter3dBg" cx="50%" cy="50%" r="70%">
                                                        <stop offset="0%" stop-color="#1e293b" stop-opacity="0.8"/>
                                                        <stop offset="100%" stop-color="#0f172a" stop-opacity="1"/>
                                                    </radialGradient>
                                                    // Point gradients
                                                    <radialGradient id="pointGreen" cx="30%" cy="30%">
                                                        <stop offset="0%" stop-color="#4ade80"/>
                                                        <stop offset="100%" stop-color="#22c55e"/>
                                                    </radialGradient>
                                                    <radialGradient id="pointYellow" cx="30%" cy="30%">
                                                        <stop offset="0%" stop-color="#fde047"/>
                                                        <stop offset="100%" stop-color="#eab308"/>
                                                    </radialGradient>
                                                    <radialGradient id="pointRed" cx="30%" cy="30%">
                                                        <stop offset="0%" stop-color="#f87171"/>
                                                        <stop offset="100%" stop-color="#ef4444"/>
                                                    </radialGradient>
                                                    <radialGradient id="pointTeal" cx="30%" cy="30%">
                                                        <stop offset="0%" stop-color="#5eead4"/>
                                                        <stop offset="100%" stop-color="#14b8a6"/>
                                                    </radialGradient>
                                                    // Glow filter for points
                                                    <filter id="pointGlow" x="-100%" y="-100%" width="300%" height="300%">
                                                        <feGaussianBlur stdDeviation="2" result="blur"/>
                                                        <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
                                                    </filter>
                                                    // Shadow filter
                                                    <filter id="dropShadow3d" x="-50%" y="-50%" width="200%" height="200%">
                                                        <feDropShadow dx="2" dy="4" stdDeviation="3" flood-color="#000" flood-opacity="0.4"/>
                                                    </filter>
                                                </defs>

                                                // Background with subtle gradient
                                                <rect x="50" y="20" width="400" height="280" rx="8" fill="url(#scatter3dBg)"/>

                                                // 3D Grid - Back planes (isometric perspective)
                                                // Back wall (XY plane at Z=0)
                                                <g class="grid-plane back-plane" opacity="0.4">
                                                    <line x1="80" y1="260" x2="80" y2="60" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="140" y1="260" x2="140" y2="60" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="200" y1="260" x2="200" y2="60" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="260" y1="260" x2="260" y2="60" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="80" y1="260" x2="260" y2="260" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="80" y1="210" x2="260" y2="210" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="80" y1="160" x2="260" y2="160" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="80" y1="110" x2="260" y2="110" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="80" y1="60" x2="260" y2="60" stroke="#4b5563" stroke-width="1"/>
                                                </g>

                                                // Floor plane (XZ at Y=0) with perspective
                                                <g class="grid-plane floor-plane" opacity="0.3">
                                                    <line x1="80" y1="260" x2="280" y2="300" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="140" y1="260" x2="340" y2="300" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="200" y1="260" x2="400" y2="300" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="260" y1="260" x2="420" y2="280" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="80" y1="260" x2="260" y2="260" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="130" y1="270" x2="310" y2="270" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="180" y1="280" x2="360" y2="280" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="230" y1="290" x2="410" y2="290" stroke="#4b5563" stroke-width="1"/>
                                                </g>

                                                // Right wall (YZ plane) with perspective
                                                <g class="grid-plane right-plane" opacity="0.25">
                                                    <line x1="260" y1="260" x2="420" y2="280" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="260" y1="210" x2="420" y2="230" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="260" y1="160" x2="420" y2="180" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="260" y1="110" x2="420" y2="130" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="260" y1="60" x2="420" y2="80" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="260" y1="60" x2="260" y2="260" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="300" y1="65" x2="300" y2="265" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="340" y1="70" x2="340" y2="270" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="380" y1="75" x2="380" y2="275" stroke="#4b5563" stroke-width="1"/>
                                                    <line x1="420" y1="80" x2="420" y2="280" stroke="#4b5563" stroke-width="1"/>
                                                </g>

                                                // 3D Axes with arrows
                                                <g class="axes-3d">
                                                    // X axis (CPU)
                                                    <line x1="80" y1="260" x2="280" y2="260" stroke="#14b8a6" stroke-width="2"/>
                                                    <polygon points="280,260 270,255 270,265" fill="#14b8a6"/>
                                                    // Y axis (Memory)
                                                    <line x1="80" y1="260" x2="80" y2="50" stroke="#8b5cf6" stroke-width="2"/>
                                                    <polygon points="80,50 75,60 85,60" fill="#8b5cf6"/>
                                                    // Z axis (Latency) - diagonal for 3D effect
                                                    <line x1="80" y1="260" x2="200" y2="310" stroke="#f97316" stroke-width="2"/>
                                                    <polygon points="200,310 188,308 192,298" fill="#f97316"/>
                                                </g>

                                                // Axis labels
                                                <text x="280" y="250" fill="#14b8a6" font-size="11" font-weight="600">"CPU %"</text>
                                                <text x="60" y="45" fill="#8b5cf6" font-size="11" font-weight="600">"Memory %"</text>
                                                <text x="205" y="320" fill="#f97316" font-size="11" font-weight="600">"Latency ms"</text>

                                                // Scale labels
                                                <text x="75" y="265" fill="#6b7280" font-size="8" text-anchor="end">"0"</text>
                                                <text x="75" y="165" fill="#6b7280" font-size="8" text-anchor="end">"50"</text>
                                                <text x="75" y="65" fill="#6b7280" font-size="8" text-anchor="end">"100"</text>
                                                <text x="260" y="275" fill="#6b7280" font-size="8" text-anchor="middle">"100"</text>
                                                <text x="170" y="275" fill="#6b7280" font-size="8" text-anchor="middle">"50"</text>

                                                // DATA POINTS - 3D positioned scatter points
                                                // Each point has: main circle + shadow + connecting line to floor

                                                // Cluster 1: Healthy - low CPU, low memory, low latency (front-left, bottom)
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="112" cy="285" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="110" y1="220" x2="112" y2="282" stroke="#22c55e" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="110" cy="220" r="8" fill="url(#pointGreen)" class="scatter-point"/>
                                                </g>
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="127" cy="278" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="125" y1="205" x2="127" y2="275" stroke="#22c55e" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="125" cy="205" r="7" fill="url(#pointGreen)" class="scatter-point"/>
                                                </g>
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="137" cy="282" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="135" y1="215" x2="137" y2="279" stroke="#22c55e" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="135" cy="215" r="6" fill="url(#pointGreen)" class="scatter-point"/>
                                                </g>
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="102" cy="280" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="100" y1="230" x2="102" y2="277" stroke="#22c55e" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="100" cy="230" r="7" fill="url(#pointGreen)" class="scatter-point"/>
                                                </g>
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="147" cy="275" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="145" y1="198" x2="147" y2="272" stroke="#22c55e" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="145" cy="198" r="9" fill="url(#pointGreen)" class="scatter-point"/>
                                                </g>

                                                // Cluster 2: Medium - moderate values (center)
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="192" cy="272" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="175" y1="165" x2="192" y2="269" stroke="#14b8a6" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="175" cy="165" r="8" fill="url(#pointTeal)" class="scatter-point"/>
                                                </g>
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="207" cy="275" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="190" y1="150" x2="207" y2="272" stroke="#14b8a6" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="190" cy="150" r="7" fill="url(#pointTeal)" class="scatter-point"/>
                                                </g>
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="182" cy="268" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="165" y1="155" x2="182" y2="265" stroke="#14b8a6" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="165" cy="155" r="6" fill="url(#pointTeal)" class="scatter-point"/>
                                                </g>
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="217" cy="278" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="200" y1="140" x2="217" y2="275" stroke="#14b8a6" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="200" cy="140" r="8" fill="url(#pointTeal)" class="scatter-point"/>
                                                </g>

                                                // Cluster 3: Warning - high CPU/memory, medium latency (upper-right)
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="267" cy="282" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="230" y1="95" x2="267" y2="279" stroke="#eab308" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="230" cy="95" r="9" fill="url(#pointYellow)" class="scatter-point"/>
                                                </g>
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="282" cy="285" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="245" y1="85" x2="282" y2="282" stroke="#eab308" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="245" cy="85" r="8" fill="url(#pointYellow)" class="scatter-point"/>
                                                </g>
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="257" cy="278" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="220" y1="105" x2="257" y2="275" stroke="#eab308" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="220" cy="105" r="7" fill="url(#pointYellow)" class="scatter-point"/>
                                                </g>

                                                // Cluster 4: Critical - extreme values (far back-right, high latency)
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="350" cy="285" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="255" y1="75" x2="350" y2="282" stroke="#ef4444" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="255" cy="75" r="10" fill="url(#pointRed)" class="scatter-point critical-pulse"/>
                                                </g>
                                                <g class="scatter-point-group" filter="url(#pointGlow)">
                                                    <ellipse cx="380" cy="290" rx="4" ry="2" fill="#000" opacity="0.3"/>
                                                    <line x1="248" y1="68" x2="380" y2="287" stroke="#ef4444" stroke-width="1" stroke-dasharray="2,2" opacity="0.5"/>
                                                    <circle cx="248" cy="68" r="8" fill="url(#pointRed)" class="scatter-point critical-pulse"/>
                                                </g>

                                                // Correlation trend plane (semi-transparent)
                                                <polygon points="95,235 240,90 410,110 265,255" fill="#14b8a6" opacity="0.08" class="correlation-plane"/>
                                                <line x1="95" y1="235" x2="410" y2="110" stroke="#14b8a6" stroke-width="1" stroke-dasharray="4,4" opacity="0.4"/>

                                                // Legend/Stats overlay
                                                <g class="scatter-stats">
                                                    <rect x="330" y="30" width="110" height="70" rx="6" fill="#1e293b" opacity="0.9"/>
                                                    <text x="340" y="48" fill="#9ca3af" font-size="9">"Correlation"</text>
                                                    <text x="340" y="62" fill="#22c55e" font-size="11" font-weight="600">"r = 0.847"</text>
                                                    <text x="340" y="78" fill="#9ca3af" font-size="9">"Data Points"</text>
                                                    <text x="340" y="92" fill="#f1f5f9" font-size="11" font-weight="600">"n = 14"</text>
                                                </g>
                                            </svg>
                                        </div>
                                        <div class="scatter3d-controls">
                                            <span class="control-hint">"Hover over points for details"</span>
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

                                // Edit User Form
                                <Show when=move || edit_user_id.get().is_some()>
                                    <div class="settings-card add-form">
                                        <h4>"Edit User: "{move || edit_user_name.get()}</h4>
                                        <div class="form-grid">
                                            <div class="form-row">
                                                <label>"Full Name"</label>
                                                <input
                                                    type="text"
                                                    class="form-input"
                                                    prop:value=move || edit_user_name.get()
                                                    on:input=move |ev| set_edit_user_name.set(event_target_value(&ev))
                                                />
                                            </div>
                                            <div class="form-row">
                                                <label>"Email"</label>
                                                <input
                                                    type="email"
                                                    class="form-input"
                                                    prop:value=move || edit_user_email.get()
                                                    on:input=move |ev| set_edit_user_email.set(event_target_value(&ev))
                                                />
                                            </div>
                                            <div class="form-row">
                                                <label>"Role"</label>
                                                <select
                                                    class="form-select"
                                                    prop:value=move || edit_user_role.get()
                                                    on:change=move |ev| set_edit_user_role.set(event_target_value(&ev))
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
                                                        prop:checked=move || edit_user_2fa.get()
                                                        on:change=move |ev| set_edit_user_2fa.set(event_target_checked(&ev))
                                                    />
                                                    <span class="toggle-slider"></span>
                                                </label>
                                            </div>
                                        </div>
                                        <div class="form-actions">
                                            <button class="btn-secondary" on:click=move |_| {
                                                set_edit_user_id.set(None);
                                            }>"Cancel"</button>
                                            <button class="btn-primary" on:click=move |_| {
                                                if let Some(uid) = edit_user_id.get() {
                                                    let name = edit_user_name.get();
                                                    let email = edit_user_email.get();
                                                    let role = edit_user_role.get();
                                                    let has_2fa = edit_user_2fa.get();

                                                    let mut users = users_list.get();
                                                    if let Some(user) = users.iter_mut().find(|(id, _, _, _, _)| id == &uid) {
                                                        user.1 = name;
                                                        user.2 = email;
                                                        user.3 = role;
                                                        user.4 = has_2fa;
                                                    }
                                                    set_users_list.set(users);
                                                    set_edit_user_id.set(None);
                                                    set_settings_message.set(Some(("User updated successfully".to_string(), true)));
                                                }
                                            }>"Save Changes"</button>
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
                                                let name_for_edit = name.clone();
                                                let email_for_edit = email.clone();
                                                let role_for_edit = role.clone();
                                                let has_2fa_for_edit = has_2fa;
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
                                                                <button
                                                                    class="btn-icon"
                                                                    title="Edit"
                                                                    on:click=move |_| {
                                                                        set_edit_user_id.set(Some(id_for_edit.clone()));
                                                                        set_edit_user_name.set(name_for_edit.clone());
                                                                        set_edit_user_email.set(email_for_edit.clone());
                                                                        set_edit_user_role.set(role_for_edit.clone());
                                                                        set_edit_user_2fa.set(has_2fa_for_edit);
                                                                    }
                                                                >"Edit"</button>
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
                                                        let doc_id = doc.id.clone();
                                                        let doc_id_click = doc.id.clone();
                                                        let full_data = doc.data.to_string();
                                                        let data_preview = full_data.clone();
                                                        let preview = if data_preview.len() > 100 {
                                                            format!("{}...", &data_preview[..100])
                                                        } else {
                                                            data_preview
                                                        };
                                                        let doc_id_sel1 = doc_id.clone();
                                                        let doc_id_sel2 = doc_id.clone();
                                                        view! {
                                                            <div
                                                                class=move || {
                                                                    if col_selected_doc.get().as_ref() == Some(&doc_id_sel1) {
                                                                        "document-item selected"
                                                                    } else {
                                                                        "document-item"
                                                                    }
                                                                }
                                                                on:click=move |_| {
                                                                    if col_selected_doc.get().as_ref() == Some(&doc_id_click) {
                                                                        set_col_selected_doc.set(None);
                                                                    } else {
                                                                        set_col_selected_doc.set(Some(doc_id_click.clone()));
                                                                    }
                                                                }
                                                            >
                                                                <div class="document-header">
                                                                    <span class="document-id">{doc.id.clone()}</span>
                                                                    <span class="document-date">{doc.updated_at.clone()}</span>
                                                                </div>
                                                                <pre class="document-preview">
                                                                    {move || if col_selected_doc.get().as_ref() == Some(&doc_id_sel2) { full_data.clone() } else { preview.clone() }}
                                                                </pre>
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
                                            <select
                                                class="graph-select"
                                                prop:value=move || graph_layout.get()
                                                on:change=move |ev| set_graph_layout.set(event_target_value(&ev))
                                            >
                                                <option value="force">"Force-Directed"</option>
                                                <option value="circular">"Circular"</option>
                                                <option value="grid">"Grid"</option>
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
                                                        {
                                                            // Calculate layout based on selected type
                                                            let nodes = data.nodes.clone();
                                                            let edges = data.edges.clone();
                                                            let layout_type = graph_layout.get();
                                                            let layout = calculate_graph_layout(&nodes, &edges, &layout_type);

                                                            let selected = graph_selected_node.clone();
                                                            let set_selected = set_graph_selected_node.clone();

                                                            view! {
                                                                <svg
                                                                    class="graph-svg"
                                                                    viewBox="-100 -100 800 600"
                                                                    preserveAspectRatio="xMidYMid meet"
                                                                >
                                                                    // Draw edges first (behind nodes)
                                                                    {edges.iter().map(|edge| {
                                                                        let source_pos = layout.iter().find(|(id, _, _)| id == &edge.source).map(|(_, x, y)| (*x, *y)).unwrap_or((0.0, 0.0));
                                                                        let target_pos = layout.iter().find(|(id, _, _)| id == &edge.target).map(|(_, x, y)| (*x, *y)).unwrap_or((0.0, 0.0));

                                                                        // Calculate arrow head
                                                                        let dx = target_pos.0 - source_pos.0;
                                                                        let dy = target_pos.1 - source_pos.1;
                                                                        let len = (dx * dx + dy * dy).sqrt();
                                                                        let node_radius = 20.0;
                                                                        let end_x = target_pos.0 - (dx / len) * node_radius;
                                                                        let end_y = target_pos.1 - (dy / len) * node_radius;

                                                                        view! {
                                                                            <g class="graph-edge">
                                                                                <line
                                                                                    x1=source_pos.0
                                                                                    y1=source_pos.1
                                                                                    x2=end_x
                                                                                    y2=end_y
                                                                                    stroke="#8b8b8b"
                                                                                    stroke-width="2"
                                                                                    marker-end="url(#arrowhead)"
                                                                                />
                                                                                <text
                                                                                    x=(source_pos.0 + end_x) / 2.0
                                                                                    y=(source_pos.1 + end_y) / 2.0 - 5.0
                                                                                    class="edge-label"
                                                                                    text-anchor="middle"
                                                                                    font-size="10"
                                                                                    fill="#666"
                                                                                >
                                                                                    {edge.label.clone()}
                                                                                </text>
                                                                            </g>
                                                                        }
                                                                    }).collect_view()}

                                                                    // Draw nodes
                                                                    {layout.iter().map(|(id, x, y)| {
                                                                        let node = nodes.iter().find(|n| &n.id == id);
                                                                        let label = node.map(|n| n.label.clone()).unwrap_or_default();
                                                                        let node_id_click = id.clone();
                                                                        let node_id_display = id.clone();
                                                                        let node_id_sel1 = id.clone();
                                                                        let node_id_sel2 = id.clone();
                                                                        let node_id_sel3 = id.clone();

                                                                        let selected1 = selected.clone();
                                                                        let selected2 = selected.clone();
                                                                        let selected3 = selected.clone();
                                                                        let color = get_label_color(&label);

                                                                        view! {
                                                                            <g
                                                                                class="graph-node"
                                                                                transform=format!("translate({}, {})", x, y)
                                                                                on:click={
                                                                                    let set_sel = set_selected.clone();
                                                                                    let nid = node_id_click.clone();
                                                                                    move |_| {
                                                                                        set_sel.set(Some(nid.clone()));
                                                                                    }
                                                                                }
                                                                                style="cursor: pointer"
                                                                            >
                                                                                <circle
                                                                                    r={
                                                                                        let nid = node_id_sel1.clone();
                                                                                        move || if selected1.get().as_ref() == Some(&nid) { 24 } else { 20 }
                                                                                    }
                                                                                    fill=color
                                                                                    stroke={
                                                                                        let nid = node_id_sel2.clone();
                                                                                        move || if selected2.get().as_ref() == Some(&nid) { "#000" } else { "#fff" }
                                                                                    }
                                                                                    stroke-width={
                                                                                        let nid = node_id_sel3.clone();
                                                                                        move || if selected3.get().as_ref() == Some(&nid) { 3 } else { 2 }
                                                                                    }
                                                                                />
                                                                                <text
                                                                                    text-anchor="middle"
                                                                                    dy="4"
                                                                                    fill="white"
                                                                                    font-size="12"
                                                                                    font-weight="bold"
                                                                                >
                                                                                    {label.chars().next().unwrap_or('?')}
                                                                                </text>
                                                                                <text
                                                                                    text-anchor="middle"
                                                                                    y="35"
                                                                                    fill="#333"
                                                                                    font-size="10"
                                                                                >
                                                                                    {node_id_display}
                                                                                </text>
                                                                            </g>
                                                                        }
                                                                    }).collect_view()}

                                                                    // Arrow marker definition
                                                                    <defs>
                                                                        <marker
                                                                            id="arrowhead"
                                                                            markerWidth="10"
                                                                            markerHeight="7"
                                                                            refX="9"
                                                                            refY="3.5"
                                                                            orient="auto"
                                                                        >
                                                                            <polygon points="0 0, 10 3.5, 0 7" fill="#8b8b8b" />
                                                                        </marker>
                                                                    </defs>
                                                                </svg>
                                                            }
                                                        }
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
                                        <div class="query-editor-container">
                                            <div class="syntax-highlight-overlay" aria-hidden="true">
                                                {move || {
                                                    let input = query_input.get();
                                                    let tab = query_tab.get();
                                                    if input.is_empty() {
                                                        view! {
                                                            <span class="placeholder">{match tab.as_str() {
                                                                "sql" => "Enter SQL query...",
                                                                "graphql" => "Enter GraphQL query...",
                                                                "cypher" => "Enter Cypher query...",
                                                                _ => "Enter your query..."
                                                            }}</span>
                                                        }.into_view()
                                                    } else {
                                                        // Tokenize based on query type
                                                        let tokens = match tab.as_str() {
                                                            "sql" | "cypher" => tokenize_sql(&input),
                                                            _ => tokenize_sql(&input), // Use SQL tokenizer as fallback
                                                        };
                                                        tokens.into_iter().map(|(text, token_type)| {
                                                            let class = token_class(token_type);
                                                            view! {
                                                                <span class=class>{text}</span>
                                                            }
                                                        }).collect_view()
                                                    }
                                                }}
                                            </div>
                                            <textarea
                                                class="query-textarea"
                                                spellcheck="false"
                                                prop:value=move || query_input.get()
                                                on:input=move |ev| {
                                                    set_query_input.set(event_target_value(&ev));
                                                }
                                            ></textarea>
                                        </div>
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

                        // Data Visualizer Modal - For visualizing actual database data
                        <Show when=move || active_modal.get() == BrowserModal::DataVisualizer>
                            <div class="modal-header premium-header">
                                <div class="header-title-group">
                                    <h2>"Data Visualizer"</h2>
                                    <span class="header-badge">"BETA"</span>
                                </div>
                                <button class="modal-close" on:click=move |_| set_active_modal.set(BrowserModal::None)>"x"</button>
                            </div>
                            <div class="modal-body visualizer-body">
                                <div class="visualizer-layout">
                                    // Left Panel - Query & Config
                                    <div class="visualizer-config-panel">
                                        <div class="config-section">
                                            <h3>"1. Query Your Data"</h3>
                                            <textarea
                                                class="viz-query-input"
                                                placeholder="SELECT price, quantity, rating, category FROM products LIMIT 100"
                                                prop:value=move || viz_query.get()
                                                on:input=move |ev| set_viz_query.set(event_target_value(&ev))
                                            ></textarea>
                                            <button
                                                class="btn-primary fetch-btn"
                                                on:click=move |_| {
                                                    let q = viz_query.get();
                                                    if !q.is_empty() {
                                                        set_viz_loading.set(true);
                                                        execute_query.dispatch(q);
                                                    }
                                                }
                                            >"Fetch Data"</button>
                                        </div>

                                        <div class="config-section">
                                            <h3>"2. Chart Type"</h3>
                                            <div class="chart-type-grid">
                                                <button
                                                    class=move || if viz_chart_type.get() == DataChartType::Scatter3D { "chart-type-btn active" } else { "chart-type-btn" }
                                                    on:click=move |_| set_viz_chart_type.set(DataChartType::Scatter3D)
                                                >
                                                    <span class="chart-icon">"⬡"</span>
                                                    <span>"3D Scatter"</span>
                                                </button>
                                                <button
                                                    class=move || if viz_chart_type.get() == DataChartType::ScatterPlot { "chart-type-btn active" } else { "chart-type-btn" }
                                                    on:click=move |_| set_viz_chart_type.set(DataChartType::ScatterPlot)
                                                >
                                                    <span class="chart-icon">"◉"</span>
                                                    <span>"Scatter"</span>
                                                </button>
                                                <button
                                                    class=move || if viz_chart_type.get() == DataChartType::BubbleChart { "chart-type-btn active" } else { "chart-type-btn" }
                                                    on:click=move |_| set_viz_chart_type.set(DataChartType::BubbleChart)
                                                >
                                                    <span class="chart-icon">"◎"</span>
                                                    <span>"Bubble"</span>
                                                </button>
                                                <button
                                                    class=move || if viz_chart_type.get() == DataChartType::LineChart { "chart-type-btn active" } else { "chart-type-btn" }
                                                    on:click=move |_| set_viz_chart_type.set(DataChartType::LineChart)
                                                >
                                                    <span class="chart-icon">"📈"</span>
                                                    <span>"Line"</span>
                                                </button>
                                                <button
                                                    class=move || if viz_chart_type.get() == DataChartType::BarChart { "chart-type-btn active" } else { "chart-type-btn" }
                                                    on:click=move |_| set_viz_chart_type.set(DataChartType::BarChart)
                                                >
                                                    <span class="chart-icon">"📊"</span>
                                                    <span>"Bar"</span>
                                                </button>
                                                <button
                                                    class=move || if viz_chart_type.get() == DataChartType::RadarChart { "chart-type-btn active" } else { "chart-type-btn" }
                                                    on:click=move |_| set_viz_chart_type.set(DataChartType::RadarChart)
                                                >
                                                    <span class="chart-icon">"⬢"</span>
                                                    <span>"Radar"</span>
                                                </button>
                                                <button
                                                    class=move || if viz_chart_type.get() == DataChartType::HeatMap { "chart-type-btn active" } else { "chart-type-btn" }
                                                    on:click=move |_| set_viz_chart_type.set(DataChartType::HeatMap)
                                                >
                                                    <span class="chart-icon">"▦"</span>
                                                    <span>"Heatmap"</span>
                                                </button>
                                                <button
                                                    class=move || if viz_chart_type.get() == DataChartType::AreaChart { "chart-type-btn active" } else { "chart-type-btn" }
                                                    on:click=move |_| set_viz_chart_type.set(DataChartType::AreaChart)
                                                >
                                                    <span class="chart-icon">"▤"</span>
                                                    <span>"Area"</span>
                                                </button>
                                            </div>
                                        </div>

                                        <div class="config-section">
                                            <h3>"3. Map Columns"</h3>
                                            {move || {
                                                let data = query_result.get();
                                                let columns = data.as_ref().map(|d| d.columns.clone()).unwrap_or_default();
                                                if columns.is_empty() {
                                                    view! {
                                                        <p class="config-hint">"Run a query to see available columns"</p>
                                                    }.into_view()
                                                } else {
                                                    let cols_x = columns.clone();
                                                    let cols_y = columns.clone();
                                                    let cols_z = columns.clone();
                                                    let cols_color = columns.clone();
                                                    let cols_size = columns.clone();
                                                    view! {
                                                        <div class="column-mapping">
                                                            <div class="mapping-row">
                                                                <label>"X Axis"</label>
                                                                <select class="mapping-select" on:change=move |ev| set_viz_x_column.set(event_target_value(&ev))>
                                                                    <option value="">"Select column..."</option>
                                                                    {cols_x.iter().map(|c| view! { <option value=c.clone()>{c.clone()}</option> }).collect_view()}
                                                                </select>
                                                            </div>
                                                            <div class="mapping-row">
                                                                <label>"Y Axis"</label>
                                                                <select class="mapping-select" on:change=move |ev| set_viz_y_column.set(event_target_value(&ev))>
                                                                    <option value="">"Select column..."</option>
                                                                    {cols_y.iter().map(|c| view! { <option value=c.clone()>{c.clone()}</option> }).collect_view()}
                                                                </select>
                                                            </div>
                                                            <Show when=move || viz_chart_type.get() == DataChartType::Scatter3D>
                                                                <div class="mapping-row">
                                                                    <label>"Z Axis"</label>
                                                                    <select class="mapping-select" on:change=move |ev| set_viz_z_column.set(event_target_value(&ev))>
                                                                        <option value="">"Select column..."</option>
                                                                        {cols_z.iter().map(|c| view! { <option value=c.clone()>{c.clone()}</option> }).collect_view()}
                                                                    </select>
                                                                </div>
                                                            </Show>
                                                            <Show when=move || matches!(viz_chart_type.get(), DataChartType::Scatter3D | DataChartType::ScatterPlot | DataChartType::BubbleChart)>
                                                                <div class="mapping-row">
                                                                    <label>"Color By"</label>
                                                                    <select class="mapping-select" on:change=move |ev| set_viz_color_column.set(event_target_value(&ev))>
                                                                        <option value="">"None"</option>
                                                                        {cols_color.iter().map(|c| view! { <option value=c.clone()>{c.clone()}</option> }).collect_view()}
                                                                    </select>
                                                                </div>
                                                            </Show>
                                                            <Show when=move || viz_chart_type.get() == DataChartType::BubbleChart>
                                                                <div class="mapping-row">
                                                                    <label>"Size By"</label>
                                                                    <select class="mapping-select" on:change=move |ev| set_viz_size_column.set(event_target_value(&ev))>
                                                                        <option value="">"Fixed"</option>
                                                                        {cols_size.iter().map(|c| view! { <option value=c.clone()>{c.clone()}</option> }).collect_view()}
                                                                    </select>
                                                                </div>
                                                            </Show>
                                                        </div>
                                                    }.into_view()
                                                }
                                            }}
                                        </div>
                                    </div>

                                    // Right Panel - Chart Visualization
                                    <div class="visualizer-chart-panel">
                                        <Show when=move || viz_loading.get()>
                                            <div class="viz-loading">
                                                <div class="spinner"></div>
                                                <p>"Loading data..."</p>
                                            </div>
                                        </Show>
                                        <Show when=move || !viz_loading.get()>
                                            {move || {
                                                let data = query_result.get();
                                                let x_col = viz_x_column.get();
                                                let y_col = viz_y_column.get();
                                                let z_col = viz_z_column.get();
                                                let chart_type = viz_chart_type.get();

                                                if data.is_none() || x_col.is_empty() || y_col.is_empty() {
                                                    return view! {
                                                        <div class="viz-empty-state">
                                                            <div class="empty-icon">"📊"</div>
                                                            <h3>"Ready to Visualize"</h3>
                                                            <p>"1. Run a SQL query to fetch your data"</p>
                                                            <p>"2. Select X and Y columns to map"</p>
                                                            <p>"3. Your chart will render here"</p>
                                                        </div>
                                                    }.into_view();
                                                }

                                                let result = data.unwrap();
                                                if !result.success {
                                                    return view! {
                                                        <div class="viz-error">
                                                            <span>"Query failed"</span>
                                                        </div>
                                                    }.into_view();
                                                }

                                                // Extract data points
                                                let columns = result.columns.clone();
                                                let rows = result.rows.clone();
                                                let x_idx = columns.iter().position(|c| c == &x_col);
                                                let y_idx = columns.iter().position(|c| c == &y_col);
                                                let z_idx = if !z_col.is_empty() { columns.iter().position(|c| c == &z_col) } else { None };

                                                if x_idx.is_none() || y_idx.is_none() {
                                                    return view! { <div class="viz-error">"Column not found"</div> }.into_view();
                                                }

                                                let x_idx = x_idx.unwrap();
                                                let y_idx = y_idx.unwrap();

                                                // Parse numeric values
                                                let points: Vec<(f64, f64, f64)> = rows.iter().filter_map(|row| {
                                                    let x = row.get(x_idx).and_then(|v| parse_numeric(v))?;
                                                    let y = row.get(y_idx).and_then(|v| parse_numeric(v))?;
                                                    let z = z_idx.and_then(|zi| row.get(zi).and_then(|v| parse_numeric(v))).unwrap_or(0.0);
                                                    Some((x, y, z))
                                                }).collect();

                                                if points.is_empty() {
                                                    return view! { <div class="viz-error">"No numeric data found"</div> }.into_view();
                                                }

                                                // Calculate bounds
                                                let (min_x, max_x) = points.iter().fold((f64::MAX, f64::MIN), |(min, max), (x, _, _)| (min.min(*x), max.max(*x)));
                                                let (min_y, max_y) = points.iter().fold((f64::MAX, f64::MIN), |(min, max), (_, y, _)| (min.min(*y), max.max(*y)));
                                                let (min_z, max_z) = points.iter().fold((f64::MAX, f64::MIN), |(min, max), (_, _, z)| (min.min(*z), max.max(*z)));

                                                let range_x = (max_x - min_x).max(0.001);
                                                let range_y = (max_y - min_y).max(0.001);
                                                let range_z = (max_z - min_z).max(0.001);

                                                match chart_type {
                                                    DataChartType::Scatter3D => {
                                                        // 3D Scatter Plot
                                                        view! {
                                                            <div class="viz-chart-container">
                                                                <div class="viz-chart-title">
                                                                    <h3>{format!("{} vs {} vs {}", x_col, y_col, if z_col.is_empty() { "—" } else { &z_col })}</h3>
                                                                    <span class="point-count">{format!("{} points", points.len())}</span>
                                                                </div>
                                                                <svg class="viz-scatter3d" viewBox="0 0 600 450">
                                                                    <defs>
                                                                        <radialGradient id="vizPointGrad" cx="30%" cy="30%">
                                                                            <stop offset="0%" stop-color="#5eead4"/>
                                                                            <stop offset="100%" stop-color="#14b8a6"/>
                                                                        </radialGradient>
                                                                        <filter id="vizGlow" x="-50%" y="-50%" width="200%" height="200%">
                                                                            <feGaussianBlur stdDeviation="2" result="blur"/>
                                                                            <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
                                                                        </filter>
                                                                        <linearGradient id="vizFloorGrad" x1="0%" y1="0%" x2="100%" y2="100%">
                                                                            <stop offset="0%" stop-color="#1e293b" stop-opacity="0.3"/>
                                                                            <stop offset="100%" stop-color="#0f172a" stop-opacity="0.1"/>
                                                                        </linearGradient>
                                                                    </defs>

                                                                    // 3D coordinate system
                                                                    <g class="viz-3d-axes">
                                                                        // Floor grid
                                                                        <polygon points="80,350 80,280 400,220 520,280 520,350 200,410" fill="url(#vizFloorGrad)" stroke="#374151" stroke-width="1"/>
                                                                        // Grid lines on floor
                                                                        {(0..5).map(|i| {
                                                                            let y_start = 280.0 + i as f64 * 17.5;
                                                                            let y_end = 220.0 + i as f64 * 17.5;
                                                                            let x1 = 80.0 + i as f64 * 30.0;
                                                                            let x2_val = x1 + 320.0;
                                                                            view! {
                                                                                <line x1=x1 y1=y_start x2=x2_val y2=y_end stroke="#374151" stroke-width="1" opacity="0.3"/>
                                                                            }
                                                                        }).collect_view()}

                                                                        // Back wall
                                                                        <polygon points="80,280 80,80 400,20 400,220" fill="#1e293b" fill-opacity="0.2" stroke="#374151" stroke-width="1"/>

                                                                        // X axis (red)
                                                                        <line x1="80" y1="280" x2="400" y2="220" stroke="#ef4444" stroke-width="2"/>
                                                                        <text x="410" y="225" fill="#ef4444" font-size="12" font-weight="600">{x_col.clone()}</text>

                                                                        // Y axis (green)
                                                                        <line x1="80" y1="280" x2="80" y2="80" stroke="#22c55e" stroke-width="2"/>
                                                                        <text x="60" y="70" fill="#22c55e" font-size="12" font-weight="600">{y_col.clone()}</text>

                                                                        // Z axis (blue)
                                                                        <line x1="80" y1="280" x2="200" y2="350" stroke="#3b82f6" stroke-width="2"/>
                                                                        <text x="210" y="365" fill="#3b82f6" font-size="12" font-weight="600">{if z_col.is_empty() { "Z".to_string() } else { z_col.clone() }}</text>
                                                                    </g>

                                                                    // Data points with 3D projection
                                                                    {points.iter().enumerate().map(|(_i, (x, y, z))| {
                                                                        // Normalize to 0-1
                                                                        let nx = (x - min_x) / range_x;
                                                                        let ny = (y - min_y) / range_y;
                                                                        let nz = (z - min_z) / range_z;

                                                                        // Project to 2D with isometric-ish transformation
                                                                        let px = 80.0 + nx * 320.0 + nz * 120.0;
                                                                        let py = 280.0 - ny * 200.0 - nx * 60.0 + nz * 70.0;

                                                                        // Size based on depth (closer = bigger)
                                                                        let size = 4.0 + nz * 6.0;

                                                                        // Color based on Y value
                                                                        let hue = 160.0 + ny * 60.0; // teal to green

                                                                        let shadow_cx = 80.0 + nx * 320.0 + nz * 120.0;
                                                                        let shadow_cy = 350.0 - nx * 60.0 + nz * 70.0;
                                                                        let shadow_rx = size * 0.8;
                                                                        let shadow_ry = size * 0.3;
                                                                        view! {
                                                                            <g class="viz-point-group">
                                                                                // Shadow on floor
                                                                                <ellipse
                                                                                    cx=shadow_cx
                                                                                    cy=shadow_cy
                                                                                    rx=shadow_rx
                                                                                    ry=shadow_ry
                                                                                    fill="#000"
                                                                                    opacity="0.2"
                                                                                />
                                                                                // Connecting line to floor
                                                                                <line
                                                                                    x1=px
                                                                                    y1=py
                                                                                    x2=shadow_cx
                                                                                    y2=shadow_cy
                                                                                    stroke="#14b8a6"
                                                                                    stroke-width="1"
                                                                                    stroke-dasharray="2,2"
                                                                                    opacity="0.3"
                                                                                />
                                                                                // Data point
                                                                                <circle
                                                                                    cx=px
                                                                                    cy=py
                                                                                    r=size
                                                                                    fill=format!("hsl({}, 70%, 50%)", hue)
                                                                                    filter="url(#vizGlow)"
                                                                                    class="viz-data-point"
                                                                                >
                                                                                    <title>{format!("({:.2}, {:.2}, {:.2})", x, y, z)}</title>
                                                                                </circle>
                                                                            </g>
                                                                        }
                                                                    }).collect_view()}

                                                                    // Stats overlay
                                                                    <g class="viz-stats">
                                                                        <rect x="440" y="30" width="140" height="90" rx="6" fill="#1e293b" opacity="0.9"/>
                                                                        <text x="450" y="50" fill="#9ca3af" font-size="10">"X Range"</text>
                                                                        <text x="450" y="65" fill="#f1f5f9" font-size="11" font-weight="500">{format!("{:.1} - {:.1}", min_x, max_x)}</text>
                                                                        <text x="450" y="82" fill="#9ca3af" font-size="10">"Y Range"</text>
                                                                        <text x="450" y="97" fill="#f1f5f9" font-size="11" font-weight="500">{format!("{:.1} - {:.1}", min_y, max_y)}</text>
                                                                        <text x="450" y="114" fill="#9ca3af" font-size="10">"Points"</text>
                                                                    </g>
                                                                </svg>
                                                            </div>
                                                        }.into_view()
                                                    },
                                                    DataChartType::ScatterPlot => {
                                                        // 2D Scatter Plot
                                                        view! {
                                                            <div class="viz-chart-container">
                                                                <div class="viz-chart-title">
                                                                    <h3>{format!("{} vs {}", x_col, y_col)}</h3>
                                                                    <span class="point-count">{format!("{} points", points.len())}</span>
                                                                </div>
                                                                <svg class="viz-scatter2d" viewBox="0 0 600 400">
                                                                    <defs>
                                                                        <linearGradient id="scatter2dBg" x1="0%" y1="0%" x2="100%" y2="100%">
                                                                            <stop offset="0%" stop-color="#1e293b"/>
                                                                            <stop offset="100%" stop-color="#0f172a"/>
                                                                        </linearGradient>
                                                                    </defs>
                                                                    <rect x="60" y="20" width="520" height="340" rx="8" fill="url(#scatter2dBg)"/>

                                                                    // Grid
                                                                    {(0..6).map(|i| {
                                                                        let x = 60.0 + i as f64 * 104.0;
                                                                        view! { <line x1=x y1="20" x2=x y2="360" stroke="#374151" stroke-width="1" opacity="0.3"/> }
                                                                    }).collect_view()}
                                                                    {(0..6).map(|i| {
                                                                        let y = 20.0 + i as f64 * 68.0;
                                                                        view! { <line x1="60" y1=y x2="580" y2=y stroke="#374151" stroke-width="1" opacity="0.3"/> }
                                                                    }).collect_view()}

                                                                    // Axes
                                                                    <line x1="60" y1="360" x2="580" y2="360" stroke="#6b7280" stroke-width="2"/>
                                                                    <line x1="60" y1="20" x2="60" y2="360" stroke="#6b7280" stroke-width="2"/>

                                                                    // Labels
                                                                    <text x="320" y="390" fill="#9ca3af" font-size="12" text-anchor="middle">{x_col.clone()}</text>
                                                                    <text x="25" y="190" fill="#9ca3af" font-size="12" text-anchor="middle" transform="rotate(-90, 25, 190)">{y_col.clone()}</text>

                                                                    // Data points
                                                                    {points.iter().map(|(x, y, _)| {
                                                                        let nx = (x - min_x) / range_x;
                                                                        let ny = (y - min_y) / range_y;
                                                                        let px = 60.0 + nx * 520.0;
                                                                        let py = 360.0 - ny * 340.0;
                                                                        view! {
                                                                            <circle cx=px cy=py r="6" fill="#14b8a6" opacity="0.8" class="viz-data-point">
                                                                                <title>{format!("({:.2}, {:.2})", x, y)}</title>
                                                                            </circle>
                                                                        }
                                                                    }).collect_view()}

                                                                    // Trend line (simple linear regression visual)
                                                                    <line x1="60" y1="340" x2="560" y2="40" stroke="#f97316" stroke-width="2" stroke-dasharray="6,4" opacity="0.6"/>
                                                                </svg>
                                                            </div>
                                                        }.into_view()
                                                    },
                                                    DataChartType::BarChart => {
                                                        // Bar Chart - aggregate by X, sum Y
                                                        view! {
                                                            <div class="viz-chart-container">
                                                                <div class="viz-chart-title">
                                                                    <h3>{format!("{} by {}", y_col, x_col)}</h3>
                                                                </div>
                                                                <svg class="viz-bar-chart" viewBox="0 0 600 400">
                                                                    <rect x="60" y="20" width="520" height="340" rx="8" fill="#1e293b"/>

                                                                    // Y axis
                                                                    <line x1="60" y1="20" x2="60" y2="360" stroke="#6b7280" stroke-width="2"/>
                                                                    <line x1="60" y1="360" x2="580" y2="360" stroke="#6b7280" stroke-width="2"/>

                                                                    // Bars
                                                                    {points.iter().take(10).enumerate().map(|(i, (_, y, _))| {
                                                                        let ny = (y - min_y) / range_y;
                                                                        let bar_width = 45.0;
                                                                        let gap = 8.0;
                                                                        let px = 70.0 + i as f64 * (bar_width + gap);
                                                                        let height = ny * 300.0;
                                                                        let py = 360.0 - height;

                                                                        let hue = 160.0 + (i as f64 * 20.0);
                                                                        view! {
                                                                            <g>
                                                                                <rect
                                                                                    x=px
                                                                                    y=py
                                                                                    width=bar_width
                                                                                    height=height
                                                                                    rx="4"
                                                                                    fill=format!("hsl({}, 70%, 50%)", hue)
                                                                                    class="viz-bar"
                                                                                >
                                                                                    <title>{format!("{:.2}", y)}</title>
                                                                                </rect>
                                                                                {
                                                                                    let text_x = px + bar_width / 2.0;
                                                                                    let text_y = py - 8.0;
                                                                                    view! { <text x=text_x y=text_y fill="#f1f5f9" font-size="10" text-anchor="middle">{format!("{:.0}", y)}</text> }
                                                                                }
                                                                            </g>
                                                                        }
                                                                    }).collect_view()}
                                                                </svg>
                                                            </div>
                                                        }.into_view()
                                                    },
                                                    DataChartType::RadarChart => {
                                                        // Radar chart - use first few columns as dimensions
                                                        let num_axes = points.len().min(6);
                                                        view! {
                                                            <div class="viz-chart-container">
                                                                <div class="viz-chart-title">
                                                                    <h3>"Radar Analysis"</h3>
                                                                </div>
                                                                <svg class="viz-radar" viewBox="0 0 400 400">
                                                                    <defs>
                                                                        <radialGradient id="radarDataFill" cx="50%" cy="50%" r="50%">
                                                                            <stop offset="0%" stop-color="#14b8a6" stop-opacity="0.6"/>
                                                                            <stop offset="100%" stop-color="#14b8a6" stop-opacity="0.2"/>
                                                                        </radialGradient>
                                                                    </defs>

                                                                    // Background rings
                                                                    <circle cx="200" cy="200" r="150" fill="none" stroke="#374151" stroke-width="1"/>
                                                                    <circle cx="200" cy="200" r="112" fill="none" stroke="#374151" stroke-width="1" opacity="0.5"/>
                                                                    <circle cx="200" cy="200" r="75" fill="none" stroke="#374151" stroke-width="1" opacity="0.3"/>
                                                                    <circle cx="200" cy="200" r="37" fill="none" stroke="#374151" stroke-width="1" opacity="0.2"/>

                                                                    // Axis lines
                                                                    {(0..num_axes).map(|i| {
                                                                        let angle = (i as f64 / num_axes as f64) * 2.0 * std::f64::consts::PI - std::f64::consts::PI / 2.0;
                                                                        let x2 = 200.0 + 150.0 * angle.cos();
                                                                        let y2 = 200.0 + 150.0 * angle.sin();
                                                                        view! {
                                                                            <line x1="200" y1="200" x2=x2 y2=y2 stroke="#4b5563" stroke-width="1"/>
                                                                        }
                                                                    }).collect_view()}

                                                                    // Data polygon
                                                                    {
                                                                        let polygon_points: String = points.iter().take(num_axes).enumerate().map(|(i, (_x, y, _))| {
                                                                            let angle = (i as f64 / num_axes as f64) * 2.0 * std::f64::consts::PI - std::f64::consts::PI / 2.0;
                                                                            let normalized = ((y - min_y) / range_y).min(1.0);
                                                                            let r = normalized * 140.0 + 10.0;
                                                                            let px = 200.0 + r * angle.cos();
                                                                            let py = 200.0 + r * angle.sin();
                                                                            format!("{:.1},{:.1}", px, py)
                                                                        }).collect::<Vec<_>>().join(" ");

                                                                        view! {
                                                                            <polygon points=polygon_points fill="url(#radarDataFill)" stroke="#14b8a6" stroke-width="2"/>
                                                                        }
                                                                    }

                                                                    // Data points on polygon
                                                                    {points.iter().take(num_axes).enumerate().map(|(i, (_, y, _))| {
                                                                        let angle = (i as f64 / num_axes as f64) * 2.0 * std::f64::consts::PI - std::f64::consts::PI / 2.0;
                                                                        let normalized = ((y - min_y) / range_y).min(1.0);
                                                                        let r = normalized * 140.0 + 10.0;
                                                                        let px = 200.0 + r * angle.cos();
                                                                        let py = 200.0 + r * angle.sin();
                                                                        view! {
                                                                            <circle cx=px cy=py r="5" fill="#14b8a6" filter="url(#vizGlow)"/>
                                                                        }
                                                                    }).collect_view()}
                                                                </svg>
                                                            </div>
                                                        }.into_view()
                                                    },
                                                    _ => {
                                                        view! {
                                                            <div class="viz-coming-soon">
                                                                <p>"This chart type is coming soon!"</p>
                                                            </div>
                                                        }.into_view()
                                                    }
                                                }
                                            }}
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

/// Parse a JSON value to a numeric f64 for charting.
fn parse_numeric(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
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

/// Format uptime in seconds to human-readable string.
fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let mins = (seconds % 3600) / 60;

    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

/// Calculate force-directed graph layout.
/// Returns a vector of (node_id, x, y) positions.
fn calculate_force_layout(nodes: &[crate::types::GraphNode], edges: &[crate::types::GraphEdge]) -> Vec<(String, f64, f64)> {
    use std::collections::HashMap;

    if nodes.is_empty() {
        return vec![];
    }

    // Layout parameters
    const REPULSION: f64 = 8000.0;
    const ATTRACTION: f64 = 0.015;
    const DAMPING: f64 = 0.85;
    const ITERATIONS: usize = 80;
    const MIN_DIST: f64 = 50.0;

    // Initialize positions in a circle
    let mut positions: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();
    let center_x = 300.0;
    let center_y = 200.0;
    let radius = 150.0;

    for (i, node) in nodes.iter().enumerate() {
        let angle = (i as f64 / nodes.len() as f64) * 2.0 * std::f64::consts::PI;
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();
        positions.insert(node.id.clone(), (x, y, 0.0, 0.0)); // x, y, vx, vy
    }

    // Build adjacency list
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    for node in nodes {
        adjacency.insert(node.id.clone(), vec![]);
    }
    for edge in edges {
        if let Some(neighbors) = adjacency.get_mut(&edge.source) {
            neighbors.push(edge.target.clone());
        }
        if let Some(neighbors) = adjacency.get_mut(&edge.target) {
            neighbors.push(edge.source.clone());
        }
    }

    // Run force simulation
    for _ in 0..ITERATIONS {
        // Calculate repulsion forces between all node pairs
        let node_ids: Vec<String> = positions.keys().cloned().collect();
        let mut forces: HashMap<String, (f64, f64)> = HashMap::new();
        for id in &node_ids {
            forces.insert(id.clone(), (0.0, 0.0));
        }

        for i in 0..node_ids.len() {
            for j in (i + 1)..node_ids.len() {
                let id_i = &node_ids[i];
                let id_j = &node_ids[j];

                let (xi, yi, _, _) = positions[id_i];
                let (xj, yj, _, _) = positions[id_j];

                let dx = xj - xi;
                let dy = yj - yi;
                let dist = (dx * dx + dy * dy).sqrt().max(MIN_DIST);

                // Coulomb repulsion
                let force = REPULSION / (dist * dist);
                let fx = (dx / dist) * force;
                let fy = (dy / dist) * force;

                if let Some((ref mut ffx, ref mut ffy)) = forces.get_mut(id_i) {
                    *ffx -= fx;
                    *ffy -= fy;
                }
                if let Some((ref mut ffx, ref mut ffy)) = forces.get_mut(id_j) {
                    *ffx += fx;
                    *ffy += fy;
                }
            }
        }

        // Calculate attraction forces along edges (spring force)
        for edge in edges {
            if let (Some(&(xi, yi, _, _)), Some(&(xj, yj, _, _))) =
                (positions.get(&edge.source), positions.get(&edge.target))
            {
                let dx = xj - xi;
                let dy = yj - yi;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);

                // Hooke's law
                let force = dist * ATTRACTION;
                let fx = (dx / dist) * force;
                let fy = (dy / dist) * force;

                if let Some((ref mut ffx, ref mut ffy)) = forces.get_mut(&edge.source) {
                    *ffx += fx;
                    *ffy += fy;
                }
                if let Some((ref mut ffx, ref mut ffy)) = forces.get_mut(&edge.target) {
                    *ffx -= fx;
                    *ffy -= fy;
                }
            }
        }

        // Update velocities and positions
        for (id, (ref mut x, ref mut y, ref mut vx, ref mut vy)) in positions.iter_mut() {
            if let Some(&(fx, fy)) = forces.get(id) {
                *vx = (*vx + fx) * DAMPING;
                *vy = (*vy + fy) * DAMPING;

                // Limit velocity
                let speed = (*vx * *vx + *vy * *vy).sqrt();
                if speed > 50.0 {
                    *vx = *vx / speed * 50.0;
                    *vy = *vy / speed * 50.0;
                }

                *x += *vx;
                *y += *vy;

                // Keep within bounds
                *x = x.clamp(20.0, 580.0);
                *y = y.clamp(20.0, 380.0);
            }
        }
    }

    // Return final positions
    positions.into_iter().map(|(id, (x, y, _, _))| (id, x, y)).collect()
}

/// Calculate circular layout.
fn calculate_circular_layout(nodes: &[crate::types::GraphNode]) -> Vec<(String, f64, f64)> {
    if nodes.is_empty() {
        return vec![];
    }

    let center_x = 300.0;
    let center_y = 200.0;
    let radius = 150.0;

    nodes.iter().enumerate().map(|(i, node)| {
        let angle = (i as f64 / nodes.len() as f64) * 2.0 * std::f64::consts::PI;
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();
        (node.id.clone(), x, y)
    }).collect()
}

/// Calculate grid layout.
fn calculate_grid_layout(nodes: &[crate::types::GraphNode]) -> Vec<(String, f64, f64)> {
    if nodes.is_empty() {
        return vec![];
    }

    let cols = (nodes.len() as f64).sqrt().ceil() as usize;
    let cell_width = 500.0 / cols.max(1) as f64;
    let cell_height = 350.0 / ((nodes.len() + cols - 1) / cols).max(1) as f64;

    nodes.iter().enumerate().map(|(i, node)| {
        let col = i % cols;
        let row = i / cols;
        let x = 50.0 + col as f64 * cell_width + cell_width / 2.0;
        let y = 50.0 + row as f64 * cell_height + cell_height / 2.0;
        (node.id.clone(), x, y)
    }).collect()
}

/// Calculate graph layout based on layout type.
fn calculate_graph_layout(nodes: &[crate::types::GraphNode], edges: &[crate::types::GraphEdge], layout_type: &str) -> Vec<(String, f64, f64)> {
    match layout_type {
        "circular" => calculate_circular_layout(nodes),
        "grid" => calculate_grid_layout(nodes),
        _ => calculate_force_layout(nodes, edges), // Default to force-directed
    }
}

/// Get color for a node label.
fn get_label_color(label: &str) -> &'static str {
    match label.to_lowercase().as_str() {
        "person" | "user" => "#14b8a6", // teal
        "company" | "organization" => "#8b5cf6", // purple
        "project" | "task" => "#f97316", // orange
        "product" | "item" => "#3b82f6", // blue
        "order" | "transaction" => "#ef4444", // red
        "location" | "place" => "#22c55e", // green
        _ => "#6b7280", // gray
    }
}

/// Token types for syntax highlighting.
#[derive(Clone, Copy, PartialEq, Debug)]
enum TokenType {
    Keyword,
    String,
    Number,
    Operator,
    Identifier,
    Comment,
    Punctuation,
    Whitespace,
}

/// Tokenize SQL for syntax highlighting.
fn tokenize_sql(query: &str) -> Vec<(String, TokenType)> {
    let sql_keywords = [
        "SELECT", "FROM", "WHERE", "INSERT", "INTO", "VALUES", "UPDATE", "SET",
        "DELETE", "CREATE", "TABLE", "DROP", "ALTER", "INDEX", "JOIN", "LEFT",
        "RIGHT", "INNER", "OUTER", "ON", "AND", "OR", "NOT", "IN", "IS", "NULL",
        "AS", "ORDER", "BY", "ASC", "DESC", "LIMIT", "OFFSET", "GROUP", "HAVING",
        "DISTINCT", "COUNT", "SUM", "AVG", "MIN", "MAX", "BETWEEN", "LIKE", "UNION",
        "ALL", "EXISTS", "CASE", "WHEN", "THEN", "ELSE", "END", "PRIMARY", "KEY",
        "FOREIGN", "REFERENCES", "CONSTRAINT", "DEFAULT", "UNIQUE", "CHECK",
    ];

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

        // Comment (-- style)
        if c == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            let mut comment = String::new();
            while i < chars.len() && chars[i] != '\n' {
                comment.push(chars[i]);
                i += 1;
            }
            tokens.push((comment, TokenType::Comment));
            continue;
        }

        // String literal
        if c == '\'' || c == '"' {
            let quote = c;
            let mut s = String::new();
            s.push(c);
            i += 1;
            while i < chars.len() {
                let ch = chars[i];
                s.push(ch);
                i += 1;
                if ch == quote {
                    // Check for escaped quote
                    if i < chars.len() && chars[i] == quote {
                        s.push(chars[i]);
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
            tokens.push((s, TokenType::String));
            continue;
        }

        // Number
        if c.is_ascii_digit() || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) {
            let mut num = String::new();
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                num.push(chars[i]);
                i += 1;
            }
            tokens.push((num, TokenType::Number));
            continue;
        }

        // Identifier or keyword
        if c.is_alphabetic() || c == '_' {
            let mut word = String::new();
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                word.push(chars[i]);
                i += 1;
            }
            let upper = word.to_uppercase();
            if sql_keywords.contains(&upper.as_str()) {
                tokens.push((word, TokenType::Keyword));
            } else {
                tokens.push((word, TokenType::Identifier));
            }
            continue;
        }

        // Operators
        if matches!(c, '=' | '<' | '>' | '!' | '+' | '-' | '*' | '/' | '%') {
            let mut op = String::new();
            op.push(c);
            i += 1;
            // Check for multi-char operators
            if i < chars.len() && matches!(chars[i], '=' | '>') {
                op.push(chars[i]);
                i += 1;
            }
            tokens.push((op, TokenType::Operator));
            continue;
        }

        // Punctuation
        if matches!(c, '(' | ')' | ',' | ';' | '.') {
            tokens.push((c.to_string(), TokenType::Punctuation));
            i += 1;
            continue;
        }

        // Unknown character
        tokens.push((c.to_string(), TokenType::Identifier));
        i += 1;
    }

    tokens
}

/// Get CSS class for token type.
fn token_class(token_type: TokenType) -> &'static str {
    match token_type {
        TokenType::Keyword => "token-keyword",
        TokenType::String => "token-string",
        TokenType::Number => "token-number",
        TokenType::Operator => "token-operator",
        TokenType::Identifier => "token-identifier",
        TokenType::Comment => "token-comment",
        TokenType::Punctuation => "token-punctuation",
        TokenType::Whitespace => "token-whitespace",
    }
}
