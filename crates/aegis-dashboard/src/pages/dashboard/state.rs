//! Dashboard state management using Leptos signals and context

use leptos::*;
use crate::types::*;
use super::shared::types::*;

/// Dashboard context containing all shared state
#[derive(Clone)]
pub struct DashboardContext {
    // Navigation
    pub current_page: RwSignal<DashboardPage>,

    // Loading state
    pub loading: RwSignal<bool>,

    // Core data
    pub cluster_status: RwSignal<Option<ClusterStatus>>,
    pub nodes: RwSignal<Vec<ClusterNode>>,
    pub db_stats: RwSignal<Option<DatabaseStats>>,
    pub alerts: RwSignal<Vec<Alert>>,

    // Modal state
    pub active_modal: RwSignal<BrowserModal>,
    pub modal_loading: RwSignal<bool>,
    pub selected_node: RwSignal<Option<ClusterNode>>,

    // KV Browser state
    pub kv_entries: RwSignal<Vec<KeyValueEntry>>,
    pub kv_search: RwSignal<String>,
    pub kv_show_add_form: RwSignal<bool>,
    pub kv_new_key: RwSignal<String>,
    pub kv_new_value: RwSignal<String>,
    pub kv_new_ttl: RwSignal<String>,
    pub kv_edit_key: RwSignal<Option<String>>,
    pub kv_edit_value: RwSignal<String>,
    pub kv_message: RwSignal<Option<(String, bool)>>,

    // Collections Browser state
    pub collections: RwSignal<Vec<DocumentCollection>>,
    pub selected_collection: RwSignal<Option<String>>,
    pub collection_docs: RwSignal<Vec<DocumentEntry>>,
    pub col_show_new_collection: RwSignal<bool>,
    pub col_new_name: RwSignal<String>,
    pub col_show_new_doc: RwSignal<bool>,
    pub col_new_doc_content: RwSignal<String>,
    pub col_selected_doc: RwSignal<Option<String>>,
    pub col_message: RwSignal<Option<(String, bool)>>,

    // Graph Explorer state
    pub graph_data: RwSignal<Option<GraphData>>,
    pub graph_search: RwSignal<String>,
    pub graph_label_filter: RwSignal<String>,
    pub graph_selected_node: RwSignal<Option<String>>,
    pub graph_layout: RwSignal<String>,

    // Query Builder state
    pub query_input: RwSignal<String>,
    pub query_result: RwSignal<Option<QueryBuilderResult>>,
    pub query_tab: RwSignal<String>,
    pub query_history: RwSignal<Vec<String>>,

    // Data Visualizer state
    pub viz_chart_type: RwSignal<DataChartType>,
    pub viz_query: RwSignal<String>,
    pub viz_data: RwSignal<Option<QueryBuilderResult>>,
    pub viz_x_column: RwSignal<String>,
    pub viz_y_column: RwSignal<String>,
    pub viz_z_column: RwSignal<String>,
    pub viz_color_column: RwSignal<String>,
    pub viz_size_column: RwSignal<String>,
    pub viz_loading: RwSignal<bool>,

    // Metrics page state
    pub metrics_time_range: RwSignal<String>,

    // Node detail state
    pub node_logs: RwSignal<Vec<crate::api::NodeLogEntry>>,
    pub node_action_message: RwSignal<Option<String>>,
    pub show_logs_view: RwSignal<bool>,

    // Settings page state
    pub settings_tab: RwSignal<String>,
    pub settings_message: RwSignal<Option<(String, bool)>>,

    // User management state
    pub show_add_user: RwSignal<bool>,
    pub edit_user_id: RwSignal<Option<String>>,
    pub edit_user_name: RwSignal<String>,
    pub edit_user_email: RwSignal<String>,
    pub edit_user_role: RwSignal<String>,
    pub edit_user_2fa: RwSignal<bool>,
    pub new_user_name: RwSignal<String>,
    pub new_user_email: RwSignal<String>,
    pub new_user_role: RwSignal<String>,
    pub new_user_2fa: RwSignal<bool>,
    pub users_list: RwSignal<Vec<(String, String, String, String, bool)>>,

    // Role management state
    pub show_add_role: RwSignal<bool>,
    pub new_role_name: RwSignal<String>,
    pub roles_list: RwSignal<Vec<(String, String, Vec<String>)>>,

    // General Settings state
    pub replication_factor: RwSignal<i32>,
    pub auto_backups_enabled: RwSignal<bool>,
    pub backup_schedule: RwSignal<String>,
    pub retention_period: RwSignal<String>,

    // Security Settings state
    pub tls_enabled: RwSignal<bool>,
    pub auth_required: RwSignal<bool>,
    pub session_timeout: RwSignal<String>,
    pub audit_logging_enabled: RwSignal<bool>,
    pub require_2fa: RwSignal<bool>,
    pub totp_enabled: RwSignal<bool>,
    pub sms_enabled: RwSignal<bool>,
    pub webauthn_enabled: RwSignal<bool>,
    pub recovery_codes_enabled: RwSignal<bool>,

    // Danger zone state
    pub show_reset_confirm: RwSignal<bool>,
    pub reset_confirm_text: RwSignal<String>,
}

impl DashboardContext {
    /// Create a new dashboard context with default values
    pub fn new() -> Self {
        Self {
            // Navigation
            current_page: create_rw_signal(DashboardPage::Overview),

            // Loading state
            loading: create_rw_signal(true),

            // Core data
            cluster_status: create_rw_signal(None),
            nodes: create_rw_signal(vec![]),
            db_stats: create_rw_signal(None),
            alerts: create_rw_signal(vec![]),

            // Modal state
            active_modal: create_rw_signal(BrowserModal::None),
            modal_loading: create_rw_signal(false),
            selected_node: create_rw_signal(None),

            // KV Browser state
            kv_entries: create_rw_signal(vec![]),
            kv_search: create_rw_signal(String::new()),
            kv_show_add_form: create_rw_signal(false),
            kv_new_key: create_rw_signal(String::new()),
            kv_new_value: create_rw_signal(String::new()),
            kv_new_ttl: create_rw_signal(String::new()),
            kv_edit_key: create_rw_signal(None),
            kv_edit_value: create_rw_signal(String::new()),
            kv_message: create_rw_signal(None),

            // Collections Browser state
            collections: create_rw_signal(vec![]),
            selected_collection: create_rw_signal(None),
            collection_docs: create_rw_signal(vec![]),
            col_show_new_collection: create_rw_signal(false),
            col_new_name: create_rw_signal(String::new()),
            col_show_new_doc: create_rw_signal(false),
            col_new_doc_content: create_rw_signal(String::new()),
            col_selected_doc: create_rw_signal(None),
            col_message: create_rw_signal(None),

            // Graph Explorer state
            graph_data: create_rw_signal(None),
            graph_search: create_rw_signal(String::new()),
            graph_label_filter: create_rw_signal(String::new()),
            graph_selected_node: create_rw_signal(None),
            graph_layout: create_rw_signal("force".to_string()),

            // Query Builder state
            query_input: create_rw_signal(String::new()),
            query_result: create_rw_signal(None),
            query_tab: create_rw_signal("sql".to_string()),
            query_history: create_rw_signal(vec![]),

            // Data Visualizer state
            viz_chart_type: create_rw_signal(DataChartType::Scatter3D),
            viz_query: create_rw_signal(String::new()),
            viz_data: create_rw_signal(None),
            viz_x_column: create_rw_signal(String::new()),
            viz_y_column: create_rw_signal(String::new()),
            viz_z_column: create_rw_signal(String::new()),
            viz_color_column: create_rw_signal(String::new()),
            viz_size_column: create_rw_signal(String::new()),
            viz_loading: create_rw_signal(false),

            // Metrics page state
            metrics_time_range: create_rw_signal("6h".to_string()),

            // Node detail state
            node_logs: create_rw_signal(vec![]),
            node_action_message: create_rw_signal(None),
            show_logs_view: create_rw_signal(false),

            // Settings page state
            settings_tab: create_rw_signal("general".to_string()),
            settings_message: create_rw_signal(None),

            // User management state
            show_add_user: create_rw_signal(false),
            edit_user_id: create_rw_signal(None),
            edit_user_name: create_rw_signal(String::new()),
            edit_user_email: create_rw_signal(String::new()),
            edit_user_role: create_rw_signal(String::new()),
            edit_user_2fa: create_rw_signal(false),
            new_user_name: create_rw_signal(String::new()),
            new_user_email: create_rw_signal(String::new()),
            new_user_role: create_rw_signal("viewer".to_string()),
            new_user_2fa: create_rw_signal(false),
            users_list: create_rw_signal(vec![
                ("user-1".to_string(), "Admin User".to_string(), "admin@aegis.db".to_string(), "admin".to_string(), true),
                ("user-2".to_string(), "John Developer".to_string(), "john@company.com".to_string(), "developer".to_string(), true),
                ("user-3".to_string(), "Jane Analyst".to_string(), "jane@company.com".to_string(), "analyst".to_string(), false),
                ("user-4".to_string(), "Demo User".to_string(), "demo@aegis.db".to_string(), "viewer".to_string(), false),
            ]),

            // Role management state
            show_add_role: create_rw_signal(false),
            new_role_name: create_rw_signal(String::new()),
            roles_list: create_rw_signal(vec![
                ("admin".to_string(), "Full access to all features".to_string(), vec!["*".to_string()]),
                ("developer".to_string(), "Read/write access to data".to_string(), vec!["data:read".to_string(), "data:write".to_string(), "query:execute".to_string()]),
                ("analyst".to_string(), "Read-only access to data and metrics".to_string(), vec!["data:read".to_string(), "metrics:read".to_string()]),
                ("viewer".to_string(), "View-only dashboard access".to_string(), vec!["dashboard:view".to_string()]),
            ]),

            // General Settings state
            replication_factor: create_rw_signal(3),
            auto_backups_enabled: create_rw_signal(true),
            backup_schedule: create_rw_signal("6h".to_string()),
            retention_period: create_rw_signal("30d".to_string()),

            // Security Settings state
            tls_enabled: create_rw_signal(true),
            auth_required: create_rw_signal(true),
            session_timeout: create_rw_signal("30m".to_string()),
            audit_logging_enabled: create_rw_signal(true),
            require_2fa: create_rw_signal(false),
            totp_enabled: create_rw_signal(true),
            sms_enabled: create_rw_signal(true),
            webauthn_enabled: create_rw_signal(false),
            recovery_codes_enabled: create_rw_signal(true),

            // Danger zone state
            show_reset_confirm: create_rw_signal(false),
            reset_confirm_text: create_rw_signal(String::new()),
        }
    }
}

impl Default for DashboardContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Provide the dashboard context to child components
pub fn provide_dashboard_context() -> DashboardContext {
    let ctx = DashboardContext::new();
    provide_context(ctx.clone());
    ctx
}

/// Get the dashboard context from the component tree
pub fn use_dashboard_context() -> DashboardContext {
    expect_context::<DashboardContext>()
}
