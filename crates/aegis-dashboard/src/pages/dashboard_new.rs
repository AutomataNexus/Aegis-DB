//! Main dashboard page - modular version
//!
//! This is the slim container that composes all dashboard components.

use leptos::*;
use leptos_router::*;
use crate::api;
use crate::state::use_app_state;

// Import from dashboard module
mod dashboard;
use dashboard::*;

/// Main dashboard component - slim container
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

    // Load data on mount and periodically
    create_effect(move |_| {
        load_data.dispatch(());
    });

    // Auto-refresh every 30 seconds
    #[cfg(target_arch = "wasm32")]
    {
        use gloo_timers::callback::Interval;
        let _interval = Interval::new(30_000, move || {
            load_data.dispatch(());
        });
        // Keep interval alive - in production use on_cleanup
        std::mem::forget(_interval);
    }

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
                    // Overview page
                    <Show when=move || !ctx.loading.get() && ctx.current_page.get() == DashboardPage::Overview>
                        <OverviewPage />
                    </Show>

                    // Nodes page
                    <Show when=move || !ctx.loading.get() && ctx.current_page.get() == DashboardPage::Nodes>
                        <NodesPage />
                    </Show>

                    // Database page
                    <Show when=move || !ctx.loading.get() && ctx.current_page.get() == DashboardPage::Database>
                        <DatabasePage />
                    </Show>

                    // Metrics page
                    <Show when=move || !ctx.loading.get() && ctx.current_page.get() == DashboardPage::Metrics>
                        <MetricsPage />
                    </Show>

                    // Settings page
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
        </div>
    }
}
