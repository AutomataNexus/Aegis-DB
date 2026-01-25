//! Dashboard sidebar navigation component

use leptos::*;
use leptos_router::*;
use crate::state::use_app_state;
use super::shared::types::DashboardPage;
use super::state::use_dashboard_context;

/// Sidebar navigation component
#[component]
pub fn Sidebar() -> impl IntoView {
    let ctx = use_dashboard_context();
    let navigate = use_navigate();
    let app_state = use_app_state();

    let logout = move |_| {
        app_state.logout();
        navigate("/login", Default::default());
    };

    view! {
        <aside class="sidebar">
            <div class="sidebar-header">
                <img src="AegisDB-logo.png" alt="Aegis DB Logo" class="sidebar-logo" width="36" height="36"/>
                <span class="sidebar-title">"Aegis DB"</span>
            </div>

            <nav class="sidebar-nav">
                <NavItem
                    page=DashboardPage::Overview
                    icon="📊"
                    label="Overview"
                    current_page=ctx.current_page
                />
                <NavItem
                    page=DashboardPage::Nodes
                    icon="🖥️"
                    label="Nodes"
                    current_page=ctx.current_page
                />
                <NavItem
                    page=DashboardPage::Database
                    icon="🗄️"
                    label="Database"
                    current_page=ctx.current_page
                />
                <NavItem
                    page=DashboardPage::Metrics
                    icon="📈"
                    label="Metrics"
                    current_page=ctx.current_page
                />
                <NavItem
                    page=DashboardPage::Settings
                    icon="⚙️"
                    label="Settings"
                    current_page=ctx.current_page
                />
            </nav>

            <div class="sidebar-footer">
                <button class="logout-btn" on:click=logout>
                    <span class="logout-icon">"🚪"</span>
                    <span>"Logout"</span>
                </button>
            </div>
        </aside>
    }
}

/// Individual navigation item
#[component]
fn NavItem(
    page: DashboardPage,
    icon: &'static str,
    label: &'static str,
    current_page: RwSignal<DashboardPage>,
) -> impl IntoView {
    let is_active = move || current_page.get() == page;

    view! {
        <button
            class=move || if is_active() { "nav-item active" } else { "nav-item" }
            on:click=move |_| current_page.set(page)
        >
            <span class="nav-icon">{icon}</span>
            <span>{label}</span>
        </button>
    }
}
