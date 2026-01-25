//! Data Visualizer modal with advanced charting

use leptos::*;
use crate::api;
use super::super::shared::types::*;
use super::super::state::use_dashboard_context;

/// Data Visualizer modal with chart type selection and column mapping
#[component]
pub fn DataVisualizerModal() -> impl IntoView {
    let ctx = use_dashboard_context();

    let close_modal = move |_| {
        ctx.active_modal.set(BrowserModal::None);
    };

    // Execute visualization query
    let execute_viz_query = create_action(move |_: &()| async move {
        let query = ctx.viz_query.get();
        if query.is_empty() {
            return;
        }
        ctx.viz_loading.set(true);
        match api::execute_builder_query(&query, "sql").await {
            Ok(result) => {
                ctx.viz_data.set(Some(result));
            }
            Err(_) => {
                ctx.viz_data.set(None);
            }
        }
        ctx.viz_loading.set(false);
    });

    view! {
        <Show when=move || ctx.active_modal.get() == BrowserModal::DataVisualizer>
            <div class="modal-overlay" on:click=close_modal>
                <div class="modal-content visualizer-modal" on:click=|e| e.stop_propagation()>
                    <div class="modal-header premium-header">
                        <div class="header-title-group">
                            <h2>"Data Visualizer"</h2>
                            <span class="header-badge">"BETA"</span>
                        </div>
                        <button class="modal-close" on:click=close_modal>"×"</button>
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
                                        prop:value=move || ctx.viz_query.get()
                                        on:input=move |ev| ctx.viz_query.set(event_target_value(&ev))
                                    ></textarea>
                                    <button
                                        class="btn btn-primary fetch-btn"
                                        on:click=move |_| execute_viz_query.dispatch(())
                                        disabled=move || ctx.viz_loading.get()
                                    >
                                        {move || if ctx.viz_loading.get() { "Loading..." } else { "Fetch Data" }}
                                    </button>
                                </div>

                                <div class="config-section">
                                    <h3>"2. Chart Type"</h3>
                                    <div class="chart-type-grid">
                                        <button
                                            class=move || if ctx.viz_chart_type.get() == DataChartType::Scatter3D { "chart-type-btn active" } else { "chart-type-btn" }
                                            on:click=move |_| ctx.viz_chart_type.set(DataChartType::Scatter3D)
                                        >
                                            <span class="chart-icon">"⬡"</span>
                                            <span>"3D Scatter"</span>
                                        </button>
                                        <button
                                            class=move || if ctx.viz_chart_type.get() == DataChartType::ScatterPlot { "chart-type-btn active" } else { "chart-type-btn" }
                                            on:click=move |_| ctx.viz_chart_type.set(DataChartType::ScatterPlot)
                                        >
                                            <span class="chart-icon">"◉"</span>
                                            <span>"Scatter"</span>
                                        </button>
                                        <button
                                            class=move || if ctx.viz_chart_type.get() == DataChartType::BubbleChart { "chart-type-btn active" } else { "chart-type-btn" }
                                            on:click=move |_| ctx.viz_chart_type.set(DataChartType::BubbleChart)
                                        >
                                            <span class="chart-icon">"◎"</span>
                                            <span>"Bubble"</span>
                                        </button>
                                        <button
                                            class=move || if ctx.viz_chart_type.get() == DataChartType::LineChart { "chart-type-btn active" } else { "chart-type-btn" }
                                            on:click=move |_| ctx.viz_chart_type.set(DataChartType::LineChart)
                                        >
                                            <span class="chart-icon">"📈"</span>
                                            <span>"Line"</span>
                                        </button>
                                        <button
                                            class=move || if ctx.viz_chart_type.get() == DataChartType::BarChart { "chart-type-btn active" } else { "chart-type-btn" }
                                            on:click=move |_| ctx.viz_chart_type.set(DataChartType::BarChart)
                                        >
                                            <span class="chart-icon">"📊"</span>
                                            <span>"Bar"</span>
                                        </button>
                                        <button
                                            class=move || if ctx.viz_chart_type.get() == DataChartType::RadarChart { "chart-type-btn active" } else { "chart-type-btn" }
                                            on:click=move |_| ctx.viz_chart_type.set(DataChartType::RadarChart)
                                        >
                                            <span class="chart-icon">"⬢"</span>
                                            <span>"Radar"</span>
                                        </button>
                                        <button
                                            class=move || if ctx.viz_chart_type.get() == DataChartType::HeatMap { "chart-type-btn active" } else { "chart-type-btn" }
                                            on:click=move |_| ctx.viz_chart_type.set(DataChartType::HeatMap)
                                        >
                                            <span class="chart-icon">"▦"</span>
                                            <span>"Heatmap"</span>
                                        </button>
                                        <button
                                            class=move || if ctx.viz_chart_type.get() == DataChartType::AreaChart { "chart-type-btn active" } else { "chart-type-btn" }
                                            on:click=move |_| ctx.viz_chart_type.set(DataChartType::AreaChart)
                                        >
                                            <span class="chart-icon">"▤"</span>
                                            <span>"Area"</span>
                                        </button>
                                    </div>
                                </div>

                                <div class="config-section">
                                    <h3>"3. Map Columns"</h3>
                                    {move || {
                                        let data = ctx.viz_data.get();
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
                                                        <select
                                                            class="mapping-select"
                                                            prop:value=move || ctx.viz_x_column.get()
                                                            on:change=move |ev| ctx.viz_x_column.set(event_target_value(&ev))
                                                        >
                                                            <option value="">"Select column..."</option>
                                                            {cols_x.iter().map(|c| view! { <option value=c.clone()>{c.clone()}</option> }).collect_view()}
                                                        </select>
                                                    </div>
                                                    <div class="mapping-row">
                                                        <label>"Y Axis"</label>
                                                        <select
                                                            class="mapping-select"
                                                            prop:value=move || ctx.viz_y_column.get()
                                                            on:change=move |ev| ctx.viz_y_column.set(event_target_value(&ev))
                                                        >
                                                            <option value="">"Select column..."</option>
                                                            {cols_y.iter().map(|c| view! { <option value=c.clone()>{c.clone()}</option> }).collect_view()}
                                                        </select>
                                                    </div>
                                                    <Show when=move || ctx.viz_chart_type.get() == DataChartType::Scatter3D>
                                                        <div class="mapping-row">
                                                            <label>"Z Axis"</label>
                                                            <select
                                                                class="mapping-select"
                                                                prop:value=move || ctx.viz_z_column.get()
                                                                on:change=move |ev| ctx.viz_z_column.set(event_target_value(&ev))
                                                            >
                                                                <option value="">"Select column..."</option>
                                                                {cols_z.iter().map(|c| view! { <option value=c.clone()>{c.clone()}</option> }).collect_view()}
                                                            </select>
                                                        </div>
                                                    </Show>
                                                    <Show when=move || matches!(ctx.viz_chart_type.get(), DataChartType::Scatter3D | DataChartType::ScatterPlot | DataChartType::BubbleChart)>
                                                        <div class="mapping-row">
                                                            <label>"Color By"</label>
                                                            <select
                                                                class="mapping-select"
                                                                prop:value=move || ctx.viz_color_column.get()
                                                                on:change=move |ev| ctx.viz_color_column.set(event_target_value(&ev))
                                                            >
                                                                <option value="">"None"</option>
                                                                {cols_color.iter().map(|c| view! { <option value=c.clone()>{c.clone()}</option> }).collect_view()}
                                                            </select>
                                                        </div>
                                                    </Show>
                                                    <Show when=move || ctx.viz_chart_type.get() == DataChartType::BubbleChart>
                                                        <div class="mapping-row">
                                                            <label>"Size By"</label>
                                                            <select
                                                                class="mapping-select"
                                                                prop:value=move || ctx.viz_size_column.get()
                                                                on:change=move |ev| ctx.viz_size_column.set(event_target_value(&ev))
                                                            >
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

                            // Right Panel - Chart Preview
                            <div class="visualizer-chart-panel">
                                <div class="chart-preview-header">
                                    <h3>"Chart Preview"</h3>
                                    <div class="chart-controls">
                                        <button class="chart-control-btn">"↻ Reset"</button>
                                        <button class="chart-control-btn">"⤓ Export"</button>
                                    </div>
                                </div>
                                <div class="chart-preview-area">
                                    {move || {
                                        let data = ctx.viz_data.get();
                                        let x_col = ctx.viz_x_column.get();
                                        let y_col = ctx.viz_y_column.get();
                                        let chart_type = ctx.viz_chart_type.get();

                                        if data.is_none() || x_col.is_empty() || y_col.is_empty() {
                                            view! {
                                                <div class="chart-placeholder">
                                                    <div class="placeholder-icon">"📊"</div>
                                                    <p>"Query data and select columns to visualize"</p>
                                                </div>
                                            }.into_view()
                                        } else {
                                            // Render chart based on type
                                            render_chart(data.unwrap(), chart_type, &x_col, &y_col).into_view()
                                        }
                                    }}
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </Show>
    }
}

/// Render a chart based on the selected type and data
fn render_chart(
    data: crate::types::QueryBuilderResult,
    chart_type: DataChartType,
    x_col: &str,
    y_col: &str,
) -> impl IntoView {
    let x_idx = data.columns.iter().position(|c| c == x_col);
    let y_idx = data.columns.iter().position(|c| c == y_col);

    if x_idx.is_none() || y_idx.is_none() {
        return view! { <div class="chart-error">"Invalid column selection"</div> }.into_view();
    }

    let x_idx = x_idx.unwrap();
    let y_idx = y_idx.unwrap();

    // Extract data points
    let points: Vec<(f64, f64)> = data.rows.iter().filter_map(|row| {
        let x = row.get(x_idx).and_then(|v| parse_value(v))?;
        let y = row.get(y_idx).and_then(|v| parse_value(v))?;
        Some((x, y))
    }).collect();

    if points.is_empty() {
        return view! { <div class="chart-error">"No numeric data to display"</div> }.into_view();
    }

    // Calculate bounds
    let x_min = points.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
    let x_max = points.iter().map(|(x, _)| *x).fold(f64::NEG_INFINITY, f64::max);
    let y_min = points.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
    let y_max = points.iter().map(|(_, y)| *y).fold(f64::NEG_INFINITY, f64::max);

    let width = 600.0;
    let height = 400.0;
    let padding = 40.0;

    match chart_type {
        DataChartType::ScatterPlot | DataChartType::Scatter3D => {
            let circles: Vec<_> = points.iter().map(|(x, y)| {
                let cx = padding + (x - x_min) / (x_max - x_min) * (width - 2.0 * padding);
                let cy = height - padding - (y - y_min) / (y_max - y_min) * (height - 2.0 * padding);
                view! {
                    <circle cx=cx cy=cy r="5" fill="#14b8a6" opacity="0.7"/>
                }
            }).collect();

            view! {
                <svg class="viz-chart" viewBox=format!("0 0 {} {}", width, height)>
                    // Axes
                    <line x1=padding y1=height-padding x2=width-padding y2=height-padding stroke="#94a3b8" stroke-width="2"/>
                    <line x1=padding y1=padding x2=padding y2=height-padding stroke="#94a3b8" stroke-width="2"/>
                    // Points
                    {circles}
                </svg>
            }.into_view()
        }
        DataChartType::LineChart => {
            let path_d = points.iter().enumerate().map(|(i, (x, y))| {
                let px = padding + (x - x_min) / (x_max - x_min) * (width - 2.0 * padding);
                let py = height - padding - (y - y_min) / (y_max - y_min) * (height - 2.0 * padding);
                if i == 0 { format!("M{},{}", px, py) } else { format!("L{},{}", px, py) }
            }).collect::<Vec<_>>().join(" ");

            view! {
                <svg class="viz-chart" viewBox=format!("0 0 {} {}", width, height)>
                    <line x1=padding y1=height-padding x2=width-padding y2=height-padding stroke="#94a3b8" stroke-width="2"/>
                    <line x1=padding y1=padding x2=padding y2=height-padding stroke="#94a3b8" stroke-width="2"/>
                    <path d=path_d fill="none" stroke="#14b8a6" stroke-width="2"/>
                </svg>
            }.into_view()
        }
        DataChartType::BarChart => {
            let bar_width = (width - 2.0 * padding) / points.len() as f64 * 0.8;
            let bars: Vec<_> = points.iter().enumerate().map(|(i, (_, y))| {
                let x = padding + i as f64 * (width - 2.0 * padding) / points.len() as f64;
                let bar_height = (y - y_min) / (y_max - y_min) * (height - 2.0 * padding);
                let bar_y = height - padding - bar_height;
                view! {
                    <rect x=x y=bar_y width=bar_width height=bar_height fill="#14b8a6"/>
                }
            }).collect();

            view! {
                <svg class="viz-chart" viewBox=format!("0 0 {} {}", width, height)>
                    <line x1=padding y1=height-padding x2=width-padding y2=height-padding stroke="#94a3b8" stroke-width="2"/>
                    <line x1=padding y1=padding x2=padding y2=height-padding stroke="#94a3b8" stroke-width="2"/>
                    {bars}
                </svg>
            }.into_view()
        }
        DataChartType::AreaChart => {
            let path_d = points.iter().enumerate().map(|(i, (x, y))| {
                let px = padding + (x - x_min) / (x_max - x_min) * (width - 2.0 * padding);
                let py = height - padding - (y - y_min) / (y_max - y_min) * (height - 2.0 * padding);
                if i == 0 { format!("M{},{}", px, py) } else { format!("L{},{}", px, py) }
            }).collect::<Vec<_>>().join(" ");

            let area_path = format!("{} L{},{} L{},{} Z",
                path_d,
                width - padding, height - padding,
                padding, height - padding
            );

            view! {
                <svg class="viz-chart" viewBox=format!("0 0 {} {}", width, height)>
                    <line x1=padding y1=height-padding x2=width-padding y2=height-padding stroke="#94a3b8" stroke-width="2"/>
                    <line x1=padding y1=padding x2=padding y2=height-padding stroke="#94a3b8" stroke-width="2"/>
                    <path d=area_path fill="#14b8a6" fill-opacity="0.3"/>
                    <path d=path_d fill="none" stroke="#14b8a6" stroke-width="2"/>
                </svg>
            }.into_view()
        }
        _ => {
            view! {
                <div class="chart-placeholder">
                    <p>"Chart type not yet implemented"</p>
                </div>
            }.into_view()
        }
    }
}

/// Parse a JSON value to f64
fn parse_value(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}
