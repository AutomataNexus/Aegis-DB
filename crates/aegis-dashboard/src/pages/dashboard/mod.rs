//! Dashboard module - modular components for the main dashboard

pub mod shared;
pub mod state;
pub mod sidebar;
pub mod overview;
pub mod nodes;
pub mod database;
pub mod metrics;
pub mod settings;

pub use shared::types::*;
pub use state::provide_dashboard_context;
pub use sidebar::Sidebar;
pub use overview::OverviewPage;
pub use nodes::{NodesPage, NodeDetailModal};
pub use database::{DatabasePage, KvBrowserModal, CollectionsModal, GraphExplorerModal, QueryBuilderModal, DataVisualizerModal};
pub use metrics::MetricsPage;
pub use settings::SettingsPage;

use leptos::*;
use leptos_router::*;
use crate::api;
use crate::state::use_app_state;

/// Main dashboard component - slim container (~120 lines)
#[component]
pub fn Dashboard() -> impl IntoView {
    let navigate = use_navigate();
    let app_state = use_app_state();

    // Redirect if not authenticated
    create_effect({
        let nav = navigate.clone();
        move |_| {
            if !app_state.is_authenticated.get() {
                nav("/login", Default::default());
            }
        }
    });

    // Provide dashboard context to all child components
    let ctx = provide_dashboard_context();

    // Load initial data
    let load_data = create_action({
        let ctx = ctx.clone();
        move |_: &()| {
            let ctx = ctx.clone();
            async move {
                ctx.loading.set(true);

                if let Ok(s) = api::get_cluster_status().await {
                    ctx.cluster_status.set(Some(s));
                }
                if let Ok(n) = api::get_nodes().await {
                    ctx.nodes.set(n);
                }
                if let Ok(s) = api::get_database_stats().await {
                    ctx.db_stats.set(Some(s));
                }
                if let Ok(a) = api::get_alerts().await {
                    ctx.alerts.set(a);
                }

                ctx.loading.set(false);
            }
        }
    });

    // Load data on mount
    create_effect(move |_| {
        load_data.dispatch(());
    });

    // Get page title
    let page_title = move || match ctx.current_page.get() {
        DashboardPage::Overview => "Cluster Overview",
        DashboardPage::Nodes => "Node Management",
        DashboardPage::Database => "Database",
        DashboardPage::Metrics => "Metrics",
        DashboardPage::Settings => "Settings",
    };

    view! {
        <div class="dashboard-layout">
            // Sidebar
            <Sidebar />

            // Main content
            <main class="main-content">
                // Header
                <header class="content-header">
                    <h1>{page_title}</h1>
                    <div class="header-actions">
                        <button
                            class="refresh-btn"
                            on:click=move |_| load_data.dispatch(())
                            disabled=move || ctx.loading.get()
                        >
                            {move || if ctx.loading.get() { "Refreshing..." } else { "Refresh" }}
                        </button>
                    </div>
                </header>

                // Loading indicator
                <Show when=move || ctx.loading.get()>
                    <div class="loading-overlay">
                        <div class="spinner"></div>
                        <p>"Loading..."</p>
                    </div>
                </Show>

                // Page content
                <div class="page-content">
                    <Show when=move || !ctx.loading.get() && ctx.current_page.get() == DashboardPage::Overview>
                        <OverviewPage />
                    </Show>

                    <Show when=move || !ctx.loading.get() && ctx.current_page.get() == DashboardPage::Nodes>
                        <NodesPage />
                    </Show>

                    <Show when=move || !ctx.loading.get() && ctx.current_page.get() == DashboardPage::Database>
                        <DatabasePage />
                    </Show>

                    <Show when=move || !ctx.loading.get() && ctx.current_page.get() == DashboardPage::Metrics>
                        <MetricsPage />
                    </Show>

                    <Show when=move || !ctx.loading.get() && ctx.current_page.get() == DashboardPage::Settings>
                        <SettingsPage />
                    </Show>
                </div>
            </main>

            // Modals
            <NodeDetailModal />
            <KvBrowserModal />
            <CollectionsModal />
            <GraphExplorerModal />
            <QueryBuilderModal />
            <DataVisualizerModal />
        </div>
    }
}
