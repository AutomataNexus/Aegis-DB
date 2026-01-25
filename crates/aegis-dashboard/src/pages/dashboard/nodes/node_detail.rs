//! Node detail modal component

use leptos::*;
use crate::types::*;
use super::super::shared::{formatting::*, types::*};
use super::super::state::use_dashboard_context;

/// Node detail modal showing extended node information
#[component]
pub fn NodeDetailModal() -> impl IntoView {
    let ctx = use_dashboard_context();

    let close_modal = move |_| {
        ctx.active_modal.set(BrowserModal::None);
        ctx.selected_node.set(None);
        ctx.show_logs_view.set(false);
    };

    view! {
        <Show when=move || ctx.active_modal.get() == BrowserModal::NodeDetail && ctx.selected_node.get().is_some()>
            <div class="modal-overlay" on:click=close_modal>
                <div class="modal-content node-detail-modal" on:click=|e| e.stop_propagation()>
                    {move || ctx.selected_node.get().map(|node| {
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

                        view! {
                            <div class="modal-header">
                                <h2>{format!("Node: {}", node.id)}</h2>
                                <button class="modal-close" on:click=close_modal>"×"</button>
                            </div>
                            <div class="modal-body">
                                // Status and role badges
                                <div class="node-detail-header">
                                    <span class=format!("status-badge large {}", status_class)>
                                        {match node.status {
                                            NodeStatus::Healthy => "Healthy",
                                            NodeStatus::Degraded => "Degraded",
                                            NodeStatus::Offline => "Offline",
                                        }}
                                    </span>
                                    <span class="role-badge large">{role_text}</span>
                                </div>

                                // Basic info
                                <div class="detail-section">
                                    <h3>"Node Information"</h3>
                                    <div class="detail-grid">
                                        <div class="detail-item">
                                            <span class="detail-label">"Address"</span>
                                            <span class="detail-value">{node.address.clone()}</span>
                                        </div>
                                        <div class="detail-item">
                                            <span class="detail-label">"Region"</span>
                                            <span class="detail-value">{node.region.clone()}</span>
                                        </div>
                                        <div class="detail-item">
                                            <span class="detail-label">"Uptime"</span>
                                            <span class="detail-value">{format_uptime(node.uptime)}</span>
                                        </div>
                                        <div class="detail-item">
                                            <span class="detail-label">"Version"</span>
                                            <span class="detail-value">"0.1.7"</span>
                                        </div>
                                    </div>
                                </div>

                                // Metrics
                                <div class="detail-section">
                                    <h3>"Performance Metrics"</h3>
                                    <div class="metrics-grid">
                                        <div class="metric-card">
                                            <div class="metric-title">"CPU Usage"</div>
                                            <div class="metric-value">{format!("{:.1}%", node.metrics.cpu_usage)}</div>
                                            <div class="metric-bar">
                                                <div class="metric-fill" style=format!("width: {}%", node.metrics.cpu_usage as u32)></div>
                                            </div>
                                        </div>
                                        <div class="metric-card">
                                            <div class="metric-title">"Memory Usage"</div>
                                            <div class="metric-value">{format!("{:.1}%", node.metrics.memory_usage)}</div>
                                            <div class="metric-bar">
                                                <div class="metric-fill" style=format!("width: {}%", node.metrics.memory_usage as u32)></div>
                                            </div>
                                        </div>
                                        <div class="metric-card">
                                            <div class="metric-title">"Disk Usage"</div>
                                            <div class="metric-value">{format!("{:.1}%", node.metrics.disk_usage)}</div>
                                            <div class="metric-bar">
                                                <div class="metric-fill" style=format!("width: {}%", node.metrics.disk_usage as u32)></div>
                                            </div>
                                        </div>
                                        <div class="metric-card">
                                            <div class="metric-title">"Ops/second"</div>
                                            <div class="metric-value">{format!("{:.0}", node.metrics.ops_per_second)}</div>
                                        </div>
                                        <div class="metric-card">
                                            <div class="metric-title">"Latency (p99)"</div>
                                            <div class="metric-value">{format!("{:.2}ms", node.metrics.latency_p99)}</div>
                                        </div>
                                        <div class="metric-card">
                                            <div class="metric-title">"Connections"</div>
                                            <div class="metric-value">{node.metrics.connections}</div>
                                        </div>
                                    </div>
                                </div>

                                // Action message
                                {move || ctx.node_action_message.get().map(|msg| view! {
                                    <div class="action-message">{msg}</div>
                                })}

                                // Logs section
                                <Show when=move || ctx.show_logs_view.get()>
                                    <div class="detail-section">
                                        <h3>"Recent Logs"</h3>
                                        <div class="logs-container">
                                            {move || ctx.node_logs.get().into_iter().map(|log| {
                                                let level_class = match log.level.as_str() {
                                                    "ERROR" => "error",
                                                    "WARN" => "warn",
                                                    "INFO" => "info",
                                                    _ => "debug",
                                                };
                                                view! {
                                                    <div class=format!("log-entry {}", level_class)>
                                                        <span class="log-timestamp">{log.timestamp.clone()}</span>
                                                        <span class="log-level">{log.level.clone()}</span>
                                                        <span class="log-message">{log.message.clone()}</span>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                </Show>
                            </div>
                            <div class="modal-footer">
                                <button
                                    class="btn btn-secondary"
                                    on:click=move |_| ctx.show_logs_view.update(|v| *v = !*v)
                                >
                                    {move || if ctx.show_logs_view.get() { "Hide Logs" } else { "View Logs" }}
                                </button>
                                <button class="btn btn-primary" on:click=close_modal>"Close"</button>
                            </div>
                        }
                    })}
                </div>
            </div>
        </Show>
    }
}
