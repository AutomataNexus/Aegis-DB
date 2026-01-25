//! Nodes page components

mod node_detail;

pub use node_detail::NodeDetailModal;

use leptos::*;
use crate::types::*;
use super::shared::{formatting::*, types::*};
use super::state::use_dashboard_context;

/// Nodes management page
#[component]
pub fn NodesPage() -> impl IntoView {
    let ctx = use_dashboard_context();

    view! {
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
                        {move || ctx.nodes.get().into_iter().map(|node| {
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
                            let node_id = node.id.clone();
                            let node_for_detail = node.clone();
                            let _node_for_restart = node.clone();
                            let _node_for_remove = node.clone();

                            view! {
                                <tr>
                                    <td>
                                        <span class=format!("status-badge {}", status_class)>
                                            {match node.status {
                                                NodeStatus::Healthy => "Healthy",
                                                NodeStatus::Degraded => "Degraded",
                                                NodeStatus::Offline => "Offline",
                                            }}
                                        </span>
                                    </td>
                                    <td class="node-id-cell">{node_id}</td>
                                    <td>{node.address.clone()}</td>
                                    <td><span class="role-badge">{role_text}</span></td>
                                    <td>{node.region.clone()}</td>
                                    <td>{format_uptime(node.uptime)}</td>
                                    <td>
                                        <div class="metric-bar">
                                            <div class="metric-fill" style=format!("width: {}%", node.metrics.cpu_usage as u32)></div>
                                            <span class="metric-text">{format!("{}%", node.metrics.cpu_usage as u32)}</span>
                                        </div>
                                    </td>
                                    <td>
                                        <div class="metric-bar">
                                            <div class="metric-fill" style=format!("width: {}%", node.metrics.memory_usage as u32)></div>
                                            <span class="metric-text">{format!("{}%", node.metrics.memory_usage as u32)}</span>
                                        </div>
                                    </td>
                                    <td>
                                        <div class="metric-bar">
                                            <div class="metric-fill" style=format!("width: {}%", node.metrics.disk_usage as u32)></div>
                                            <span class="metric-text">{format!("{}%", node.metrics.disk_usage as u32)}</span>
                                        </div>
                                    </td>
                                    <td>{format!("{:.0}", node.metrics.ops_per_second)}</td>
                                    <td>{format!("{:.1}ms", node.metrics.latency_p99)}</td>
                                    <td class="actions-cell">
                                        <button
                                            class="action-btn"
                                            title="View Details"
                                            on:click=move |_| {
                                                ctx.selected_node.set(Some(node_for_detail.clone()));
                                                ctx.active_modal.set(BrowserModal::NodeDetail);
                                            }
                                        >"Details"</button>
                                        <button class="action-btn" title="Restart Node">"Restart"</button>
                                        <button class="action-btn danger" title="Remove Node">"Remove"</button>
                                    </td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>

            // Summary cards
            <div class="nodes-summary">
                <div class="summary-card">
                    <div class="summary-value">{move || ctx.nodes.get().len()}</div>
                    <div class="summary-title">"Total Nodes"</div>
                </div>
                <div class="summary-card">
                    <div class="summary-value">
                        {move || ctx.nodes.get().iter().filter(|n| matches!(n.status, NodeStatus::Healthy)).count()}
                    </div>
                    <div class="summary-title">"Healthy"</div>
                </div>
                <div class="summary-card">
                    <div class="summary-value">
                        {move || ctx.nodes.get().iter().filter(|n| matches!(n.role, NodeRole::Leader)).count()}
                    </div>
                    <div class="summary-title">"Leaders"</div>
                </div>
            </div>
        </div>
    }
}
