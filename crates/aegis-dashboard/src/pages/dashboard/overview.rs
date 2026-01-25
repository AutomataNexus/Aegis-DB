//! Dashboard overview page component

use leptos::*;
use crate::types::*;
use super::shared::{formatting::*, types::*};
use super::state::use_dashboard_context;

/// Overview page showing cluster stats and summary
#[component]
pub fn OverviewPage() -> impl IntoView {
    let ctx = use_dashboard_context();

    view! {
        // Stats Grid
        <div class="stats-grid">
            <StatCard
                icon="N"
                icon_class="teal"
                label="Active Nodes"
                value=Signal::derive(move || {
                    ctx.cluster_status.get()
                        .map(|s| format!("{}/{}", s.healthy_nodes, s.total_nodes))
                        .unwrap_or_else(|| "-".to_string())
                })
                change=Some("All Healthy".to_string())
                positive=true
            />
            <StatCard
                icon="O"
                icon_class="terracotta"
                label="Ops/min"
                value=Signal::derive(move || {
                    ctx.db_stats.get()
                        .map(|s| format_number(s.ops_last_minute))
                        .unwrap_or_else(|| "-".to_string())
                })
                change=Some("+12%".to_string())
                positive=true
            />
            <StatCard
                icon="S"
                icon_class="success"
                label="Storage Used"
                value=Signal::derive(move || {
                    ctx.db_stats.get()
                        .map(|s| {
                            let pct = (s.storage_used as f64 / s.storage_total as f64 * 100.0) as u32;
                            format!("{} ({}%)", format_bytes(s.storage_used), pct)
                        })
                        .unwrap_or_else(|| "-".to_string())
                })
                change=None
                positive=true
            />
            <StatCard
                icon="C"
                icon_class="info"
                label="Cache Hit Rate"
                value=Signal::derive(move || {
                    ctx.db_stats.get()
                        .map(|s| format!("{:.1}%", s.cache_hit_rate))
                        .unwrap_or_else(|| "-".to_string())
                })
                change=None
                positive=true
            />
        </div>

        // Dashboard Grid
        <div class="dashboard-grid">
            // Nodes Card
            <NodesCard />

            // Right Column
            <div class="right-column">
                <AlertsCard />
                <ClusterInfoCard />
            </div>
        </div>
    }
}

/// Stat card component
#[component]
fn StatCard(
    icon: &'static str,
    icon_class: &'static str,
    #[prop(into)] label: MaybeSignal<String>,
    value: Signal<String>,
    change: Option<String>,
    positive: bool,
) -> impl IntoView {
    view! {
        <div class="stat-card">
            <div class="stat-header">
                <div class=format!("stat-icon {}", icon_class)>{icon}</div>
                {change.map(|c| view! {
                    <span class=if positive { "stat-change positive" } else { "stat-change negative" }>{c}</span>
                })}
            </div>
            <div class="stat-value">{value}</div>
            <div class="stat-label">{label}</div>
        </div>
    }
}

/// Nodes summary card
#[component]
fn NodesCard() -> impl IntoView {
    let ctx = use_dashboard_context();

    view! {
        <div class="card">
            <div class="card-header">
                <span class="card-title">"Cluster Nodes"</span>
            </div>
            <div class="card-body">
                <div class="nodes-list">
                    {move || ctx.nodes.get().into_iter().map(|node| {
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
                                    ctx.selected_node.set(Some(node_for_click.clone()));
                                    ctx.active_modal.set(BrowserModal::NodeDetail);
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
    }
}

/// Alerts card
#[component]
fn AlertsCard() -> impl IntoView {
    let ctx = use_dashboard_context();

    view! {
        <div class="card">
            <div class="card-header">
                <span class="card-title">"Alerts"</span>
            </div>
            <div class="card-body">
                {move || {
                    let alert_list = ctx.alerts.get();
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
    }
}

/// Cluster info card
#[component]
fn ClusterInfoCard() -> impl IntoView {
    let ctx = use_dashboard_context();

    view! {
        <div class="card">
            <div class="card-header">
                <span class="card-title">"Cluster Info"</span>
            </div>
            <div class="card-body">
                <div class="cluster-info">
                    {move || ctx.cluster_status.get().map(|s| view! {
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
    }
}
