//! Graph explorer modal with SVG visualization

use leptos::*;
use crate::api;
use crate::types::*;
use super::super::shared::types::*;
use super::super::state::use_dashboard_context;

/// Graph explorer modal with force-directed layout visualization
#[component]
pub fn GraphExplorerModal() -> impl IntoView {
    let ctx = use_dashboard_context();

    let close_modal = move |_| {
        ctx.active_modal.set(BrowserModal::None);
        ctx.graph_selected_node.set(None);
    };

    // Load graph data
    let load_graph = create_action(move |_: &()| async move {
        ctx.modal_loading.set(true);
        if let Ok(data) = api::get_graph_data().await {
            ctx.graph_data.set(Some(data));
        }
        ctx.modal_loading.set(false);
    });

    // Load on open
    create_effect(move |_| {
        if ctx.active_modal.get() == BrowserModal::Graph {
            load_graph.dispatch(());
        }
    });

    view! {
        <Show when=move || ctx.active_modal.get() == BrowserModal::Graph>
            <div class="modal-overlay" on:click=close_modal>
                <div class="modal-content graph-modal" on:click=|e| e.stop_propagation()>
                    <div class="modal-header">
                        <h2>"🔗 Graph Explorer"</h2>
                        <div class="graph-controls">
                            <input
                                type="text"
                                class="search-input"
                                placeholder="Search nodes..."
                                prop:value=move || ctx.graph_search.get()
                                on:input=move |e| ctx.graph_search.set(event_target_value(&e))
                            />
                            <input
                                type="text"
                                class="filter-input"
                                placeholder="Filter by label..."
                                prop:value=move || ctx.graph_label_filter.get()
                                on:input=move |e| ctx.graph_label_filter.set(event_target_value(&e))
                            />
                            <select
                                class="layout-select"
                                prop:value=move || ctx.graph_layout.get()
                                on:change=move |e| ctx.graph_layout.set(event_target_value(&e))
                            >
                                <option value="force">"Force-Directed"</option>
                                <option value="circular">"Circular"</option>
                                <option value="grid">"Grid"</option>
                            </select>
                        </div>
                        <button class="modal-close" on:click=close_modal>"×"</button>
                    </div>
                    <div class="modal-body">
                        <Show when=move || ctx.modal_loading.get()>
                            <div class="loading">"Loading graph data..."</div>
                        </Show>

                        <Show when=move || !ctx.modal_loading.get() && ctx.graph_data.get().is_some()>
                            <div class="graph-container">
                                {move || ctx.graph_data.get().map(|data| {
                                    let layout = calculate_layout(&data.nodes, &data.edges, &ctx.graph_layout.get());
                                    let search = ctx.graph_search.get().to_lowercase();
                                    let filter = ctx.graph_label_filter.get().to_lowercase();
                                    // Clone nodes for use in the view
                                    let nodes_for_view: Vec<_> = data.nodes.clone();
                                    let edges_for_view: Vec<_> = data.edges.clone();

                                    view! {
                                        <svg class="graph-svg" viewBox="0 0 800 600">
                                            // Edges
                                            {edges_for_view.iter().map(|edge| {
                                                let from_pos = layout.iter().find(|(id, _, _)| id == &edge.source);
                                                let to_pos = layout.iter().find(|(id, _, _)| id == &edge.target);

                                                if let (Some((_, x1, y1)), Some((_, x2, y2))) = (from_pos, to_pos) {
                                                    view! {
                                                        <line
                                                            x1=*x1
                                                            y1=*y1
                                                            x2=*x2
                                                            y2=*y2
                                                            class="graph-edge"
                                                            stroke="#94a3b8"
                                                            stroke-width="2"
                                                        />
                                                    }.into_view()
                                                } else {
                                                    view! {}.into_view()
                                                }
                                            }).collect_view()}

                                            // Nodes - use layout directly with node info included
                                            {layout.into_iter().filter_map(|(id, x, y)| {
                                                let node = nodes_for_view.iter().find(|n| n.id == id)?;
                                                // Apply search filter (matches ID or label)
                                                if !search.is_empty() && !node.id.to_lowercase().contains(&search) && !node.label.to_lowercase().contains(&search) {
                                                    return None;
                                                }
                                                // Apply label type filter
                                                if !filter.is_empty() && !node.label.to_lowercase().contains(&filter) {
                                                    return None;
                                                }
                                                let color = get_label_color(&node.label);
                                                let node_id = node.id.clone();
                                                let node_label = node.label.clone();
                                                let first_char = node_label.chars().next().unwrap_or('?').to_string();
                                                let node_id_for_click = node_id.clone();
                                                let node_id_for_check = node_id.clone();

                                                Some(view! {
                                                    <g
                                                        class="graph-node"
                                                        transform=format!("translate({}, {})", x, y)
                                                        on:click=move |_| ctx.graph_selected_node.set(Some(node_id_for_click.clone()))
                                                    >
                                                        <circle
                                                            r={
                                                                let id = node_id_for_check.clone();
                                                                move || if ctx.graph_selected_node.get().as_ref() == Some(&id) { "24" } else { "20" }
                                                            }
                                                            fill=color
                                                            stroke={
                                                                let id = node_id.clone();
                                                                move || if ctx.graph_selected_node.get().as_ref() == Some(&id) { "#000" } else { "#fff" }
                                                            }
                                                            stroke-width="2"
                                                        />
                                                        <text
                                                            text-anchor="middle"
                                                            dy="5"
                                                            fill="white"
                                                            font-size="12"
                                                        >{first_char}</text>
                                                    </g>
                                                })
                                            }).collect_view()}
                                        </svg>
                                    }
                                })}

                                // Selected node details
                                <Show when=move || ctx.graph_selected_node.get().is_some()>
                                    {move || {
                                        let selected_id = ctx.graph_selected_node.get()?;
                                        let data = ctx.graph_data.get()?;
                                        let node = data.nodes.iter().find(|n| n.id == selected_id)?;

                                        Some(view! {
                                            <div class="node-details-panel">
                                                <h4>"Node Details"</h4>
                                                <div class="detail-row">
                                                    <span class="label">"ID:"</span>
                                                    <span class="value">{node.id.clone()}</span>
                                                </div>
                                                <div class="detail-row">
                                                    <span class="label">"Label:"</span>
                                                    <span class="value">{node.label.clone()}</span>
                                                </div>
                                                <div class="detail-row">
                                                    <span class="label">"Properties:"</span>
                                                    <pre class="value">{serde_json::to_string_pretty(&node.properties).unwrap_or_default()}</pre>
                                                </div>
                                                <button
                                                    class="btn btn-small"
                                                    on:click=move |_| ctx.graph_selected_node.set(None)
                                                >"Close"</button>
                                            </div>
                                        })
                                    }}
                                </Show>
                            </div>
                        </Show>
                    </div>
                </div>
            </div>
        </Show>
    }
}

/// Calculate layout positions for nodes
fn calculate_layout(nodes: &[GraphNode], edges: &[GraphEdge], layout_type: &str) -> Vec<(String, f64, f64)> {
    match layout_type {
        "circular" => calculate_circular_layout(nodes),
        "grid" => calculate_grid_layout(nodes),
        _ => calculate_force_layout(nodes, edges),
    }
}

/// Force-directed layout using simple spring simulation
fn calculate_force_layout(nodes: &[GraphNode], edges: &[GraphEdge]) -> Vec<(String, f64, f64)> {
    let width = 800.0;
    let height = 600.0;
    let center_x = width / 2.0;
    let center_y = height / 2.0;

    if nodes.is_empty() {
        return vec![];
    }

    // Initialize positions in a circle
    let mut positions: Vec<(String, f64, f64)> = nodes.iter().enumerate().map(|(i, node)| {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (nodes.len() as f64);
        let radius = 150.0;
        (
            node.id.clone(),
            center_x + radius * angle.cos(),
            center_y + radius * angle.sin(),
        )
    }).collect();

    // Simple force simulation (10 iterations)
    for _ in 0..10 {
        let mut forces: Vec<(f64, f64)> = vec![(0.0, 0.0); nodes.len()];

        // Repulsion between all nodes
        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                let dx = positions[j].1 - positions[i].1;
                let dy = positions[j].2 - positions[i].2;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let force = 5000.0 / (dist * dist);

                let fx = force * dx / dist;
                let fy = force * dy / dist;

                forces[i].0 -= fx;
                forces[i].1 -= fy;
                forces[j].0 += fx;
                forces[j].1 += fy;
            }
        }

        // Attraction along edges
        for edge in edges {
            let from_idx = positions.iter().position(|(id, _, _)| id == &edge.source);
            let to_idx = positions.iter().position(|(id, _, _)| id == &edge.target);

            if let (Some(i), Some(j)) = (from_idx, to_idx) {
                let dx = positions[j].1 - positions[i].1;
                let dy = positions[j].2 - positions[i].2;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let force = dist * 0.01;

                let fx = force * dx / dist;
                let fy = force * dy / dist;

                forces[i].0 += fx;
                forces[i].1 += fy;
                forces[j].0 -= fx;
                forces[j].1 -= fy;
            }
        }

        // Apply forces
        for (i, (_, x, y)) in positions.iter_mut().enumerate() {
            *x += forces[i].0 * 0.1;
            *y += forces[i].1 * 0.1;

            // Keep within bounds
            *x = x.max(50.0).min(width - 50.0);
            *y = y.max(50.0).min(height - 50.0);
        }
    }

    positions
}

/// Circular layout
fn calculate_circular_layout(nodes: &[GraphNode]) -> Vec<(String, f64, f64)> {
    let center_x = 400.0;
    let center_y = 300.0;
    let radius = 200.0;

    nodes.iter().enumerate().map(|(i, node)| {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (nodes.len().max(1) as f64);
        (
            node.id.clone(),
            center_x + radius * angle.cos(),
            center_y + radius * angle.sin(),
        )
    }).collect()
}

/// Grid layout
fn calculate_grid_layout(nodes: &[GraphNode]) -> Vec<(String, f64, f64)> {
    let cols = ((nodes.len() as f64).sqrt().ceil() as usize).max(1);
    let spacing = 100.0;
    let start_x = 100.0;
    let start_y = 100.0;

    nodes.iter().enumerate().map(|(i, node)| {
        let col = i % cols;
        let row = i / cols;
        (
            node.id.clone(),
            start_x + (col as f64) * spacing,
            start_y + (row as f64) * spacing,
        )
    }).collect()
}

/// Get color for node label
fn get_label_color(label: &str) -> &'static str {
    match label.to_lowercase().as_str() {
        "user" | "person" => "#3b82f6",
        "product" | "item" => "#10b981",
        "order" | "transaction" => "#f59e0b",
        "category" | "tag" => "#8b5cf6",
        "location" | "place" => "#ef4444",
        _ => "#6b7280",
    }
}
