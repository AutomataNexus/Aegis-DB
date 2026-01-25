//! Database page and modal components

mod kv_browser;
mod collections;
mod graph;
mod query_builder;
mod data_visualizer;

pub use kv_browser::KvBrowserModal;
pub use collections::CollectionsModal;
pub use graph::GraphExplorerModal;
pub use query_builder::QueryBuilderModal;
pub use data_visualizer::DataVisualizerModal;

use leptos::*;
use super::shared::{formatting::*, types::*};
use super::state::use_dashboard_context;

/// Database management page
#[component]
pub fn DatabasePage() -> impl IntoView {
    let ctx = use_dashboard_context();

    view! {
        // Database Stats Cards
        <div class="db-stats-grid">
            <div class="db-stat-card">
                <div class="db-stat-icon">"🔑"</div>
                <div class="db-stat-value">
                    {move || ctx.db_stats.get().map(|s| format_number(s.total_keys as u64)).unwrap_or_else(|| "0".to_string())}
                </div>
                <div class="db-stat-label">"Total Keys"</div>
            </div>
            <div class="db-stat-card">
                <div class="db-stat-icon">"📄"</div>
                <div class="db-stat-value">
                    {move || ctx.db_stats.get().map(|s| format_number(s.total_documents as u64)).unwrap_or_else(|| "0".to_string())}
                </div>
                <div class="db-stat-label">"Documents"</div>
            </div>
            <div class="db-stat-card">
                <div class="db-stat-icon">"🔗"</div>
                <div class="db-stat-value">
                    {move || ctx.db_stats.get().map(|s| format_number(s.total_graph_nodes as u64)).unwrap_or_else(|| "0".to_string())}
                </div>
                <div class="db-stat-label">"Graph Nodes"</div>
            </div>
            <div class="db-stat-card">
                <div class="db-stat-icon">"↔"</div>
                <div class="db-stat-value">
                    {move || ctx.db_stats.get().map(|s| format_number(s.total_graph_edges as u64)).unwrap_or_else(|| "0".to_string())}
                </div>
                <div class="db-stat-label">"Graph Edges"</div>
            </div>
        </div>

        // Storage info
        <div class="storage-info-card">
            <h3>"Storage"</h3>
            <div class="storage-bar">
                <div
                    class="storage-fill"
                    style=move || ctx.db_stats.get()
                        .map(|s| format!("width: {}%", (s.storage_used as f64 / s.storage_total as f64 * 100.0) as u32))
                        .unwrap_or_else(|| "width: 0%".to_string())
                ></div>
            </div>
            <div class="storage-text">
                {move || ctx.db_stats.get().map(|s| format!("{} used / {} total",
                    format_bytes(s.storage_used),
                    format_bytes(s.storage_total)
                )).unwrap_or_else(|| "- / -".to_string())}
            </div>
            <div class="storage-stats">
                <div class="storage-stat">
                    <span class="stat-label">"Cache Hit Rate"</span>
                    <span class="stat-value">
                        {move || ctx.db_stats.get().map(|s| format!("{:.1}%", s.cache_hit_rate)).unwrap_or_else(|| "-".to_string())}
                    </span>
                </div>
                <div class="storage-stat">
                    <span class="stat-label">"Operations/min"</span>
                    <span class="stat-value">
                        {move || ctx.db_stats.get().map(|s| format_number(s.ops_last_minute)).unwrap_or_else(|| "-".to_string())}
                    </span>
                </div>
            </div>
        </div>

        // Data paradigms grid
        <div class="paradigms-grid">
            <ParadigmCard
                icon="🔑"
                name="Key-Value Store"
                stat1_label="Keys"
                stat1_value=Signal::derive(move || ctx.db_stats.get().map(|s| s.total_keys.to_string()).unwrap_or_else(|| "0".to_string()))
                stat2_label="Avg Latency"
                stat2_value="< 1ms".to_string()
                action_label="Browse Keys"
                on_action=move |_| ctx.active_modal.set(BrowserModal::KeyValue)
            />
            <ParadigmCard
                icon="📄"
                name="Document Store"
                stat1_label="Documents"
                stat1_value=Signal::derive(move || ctx.db_stats.get().map(|s| s.total_documents.to_string()).unwrap_or_else(|| "0".to_string()))
                stat2_label="Collections"
                stat2_value=Signal::derive(move || ctx.collections.get().len().to_string())
                action_label="Browse Collections"
                on_action=move |_| ctx.active_modal.set(BrowserModal::Collections)
            />
            <ParadigmCard
                icon="🔗"
                name="Graph Database"
                stat1_label="Nodes"
                stat1_value=Signal::derive(move || ctx.db_stats.get().map(|s| s.total_graph_nodes.to_string()).unwrap_or_else(|| "0".to_string()))
                stat2_label="Edges"
                stat2_value=Signal::derive(move || ctx.db_stats.get().map(|s| s.total_graph_edges.to_string()).unwrap_or_else(|| "0".to_string()))
                action_label="Graph Explorer"
                on_action=move |_| ctx.active_modal.set(BrowserModal::Graph)
            />
            <ParadigmCard
                icon="📈"
                name="Time Series"
                stat1_label="Metrics"
                stat1_value="24".to_string()
                stat2_label="Retention"
                stat2_value="30d".to_string()
                action_label="Query Builder"
                on_action=move |_| ctx.active_modal.set(BrowserModal::QueryBuilder)
            />
            <ParadigmCard
                icon="📊"
                name="Data Visualizer"
                stat1_label="Chart Types"
                stat1_value="8".to_string()
                stat2_label="3D"
                stat2_value="Scatter".to_string()
                action_label="Visualize Data"
                on_action=move |_| ctx.active_modal.set(BrowserModal::DataVisualizer)
            />
        </div>
    }
}

/// Data paradigm card component
#[component]
fn ParadigmCard<F>(
    icon: &'static str,
    name: &'static str,
    stat1_label: &'static str,
    #[prop(into)] stat1_value: MaybeSignal<String>,
    stat2_label: &'static str,
    #[prop(into)] stat2_value: MaybeSignal<String>,
    action_label: &'static str,
    on_action: F,
) -> impl IntoView
where
    F: Fn(web_sys::MouseEvent) + 'static,
{
    view! {
        <div class="paradigm-card">
            <div class="paradigm-header">
                <span class="paradigm-icon">{icon}</span>
                <span class="paradigm-name">{name}</span>
            </div>
            <div class="paradigm-stats">
                <div class="paradigm-stat">
                    <span class="stat-value">{stat1_value}</span>
                    <span class="stat-label">{stat1_label}</span>
                </div>
                <div class="paradigm-stat">
                    <span class="stat-value">{stat2_value}</span>
                    <span class="stat-label">{stat2_label}</span>
                </div>
            </div>
            <button class="paradigm-action" on:click=on_action>{action_label}</button>
        </div>
    }
}
