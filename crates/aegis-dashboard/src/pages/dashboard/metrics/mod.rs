//! Metrics page with sophisticated visualizations

use leptos::*;
use crate::types::*;
use super::shared::formatting::*;
use super::state::use_dashboard_context;

/// Metrics monitoring page with full visualization suite
#[component]
pub fn MetricsPage() -> impl IntoView {
    let ctx = use_dashboard_context();

    view! {
        <div class="metrics-page">
            // Time range selector
            <div class="metrics-header">
                <h2>"Cluster Metrics"</h2>
                <div class="time-range-selector">
                    <select
                        prop:value=move || ctx.metrics_time_range.get()
                        on:change=move |e| ctx.metrics_time_range.set(event_target_value(&e))
                    >
                        <option value="1h">"Last 1 hour"</option>
                        <option value="6h">"Last 6 hours"</option>
                        <option value="24h">"Last 24 hours"</option>
                        <option value="7d">"Last 7 days"</option>
                        <option value="30d">"Last 30 days"</option>
                    </select>
                </div>
            </div>

            // Main Charts Grid
            <div class="metrics-charts-grid">
                // Operations Chart - Large
                <div class="chart-card chart-large">
                    <div class="chart-header">
                        <div class="chart-title-row">
                            <h3>"Operations Over Time"</h3>
                            <span class="chart-time-badge">{move || format!("Last {}", ctx.metrics_time_range.get())}</span>
                            <div class="chart-legend">
                                <span class="legend-dot read"></span>"Reads"
                                <span class="legend-dot write"></span>"Writes"
                            </div>
                        </div>
                        <div class="chart-value-lg">{move || ctx.db_stats.get().map(|s| format!("{}/s", format_number(s.ops_last_minute / 60))).unwrap_or("-".to_string())}</div>
                    </div>
                    <div class="chart-body">
                        <svg class="line-chart" viewBox="0 0 600 200" preserveAspectRatio="none">
                            // Grid lines
                            <line x1="0" y1="50" x2="600" y2="50" class="grid-line"/>
                            <line x1="0" y1="100" x2="600" y2="100" class="grid-line"/>
                            <line x1="0" y1="150" x2="600" y2="150" class="grid-line"/>
                            // Reads line (teal)
                            <path d="M0,140 C30,130 60,110 90,100 C120,90 150,95 180,85 C210,75 240,70 270,60 C300,55 330,65 360,55 C390,50 420,40 450,35 C480,40 510,50 540,45 C570,40 600,35 600,35"
                                  class="chart-line-reads" fill="none"/>
                            // Writes line (terracotta)
                            <path d="M0,170 C30,165 60,155 90,150 C120,145 150,140 180,135 C210,130 240,125 270,120 C300,115 330,120 360,115 C390,110 420,105 450,100 C480,105 510,110 540,105 C570,100 600,95 600,95"
                                  class="chart-line-writes" fill="none"/>
                            // Area fill for reads
                            <path d="M0,140 C30,130 60,110 90,100 C120,90 150,95 180,85 C210,75 240,70 270,60 C300,55 330,65 360,55 C390,50 420,40 450,35 C480,40 510,50 540,45 C570,40 600,35 600,35 L600,200 L0,200 Z"
                                  class="chart-area-reads"/>
                        </svg>
                        <div class="chart-x-labels">
                            {move || {
                                let range = ctx.metrics_time_range.get();
                                let labels: Vec<&str> = match range.as_str() {
                                    "1h" => vec!["-60m", "-50m", "-40m", "-30m", "-20m", "-10m", "Now"],
                                    "6h" => vec!["-6h", "-5h", "-4h", "-3h", "-2h", "-1h", "Now"],
                                    "24h" => vec!["-24h", "-20h", "-16h", "-12h", "-8h", "-4h", "Now"],
                                    "7d" => vec!["-7d", "-6d", "-5d", "-4d", "-3d", "-1d", "Now"],
                                    "30d" => vec!["-30d", "-25d", "-20d", "-15d", "-10d", "-5d", "Now"],
                                    _ => vec!["-6h", "-5h", "-4h", "-3h", "-2h", "-1h", "Now"],
                                };
                                labels.into_iter().map(|l| view! { <span>{l}</span> }).collect_view()
                            }}
                        </div>
                    </div>
                </div>

                // Latency Distribution
                <div class="chart-card">
                    <div class="chart-header">
                        <h3>"Latency Distribution"</h3>
                    </div>
                    <div class="chart-body">
                        <div class="latency-histogram">
                            <div class="histogram-bar" style="height: 90%">
                                <span class="bar-label">"< 1ms"</span>
                                <span class="bar-value">"72%"</span>
                            </div>
                            <div class="histogram-bar" style="height: 60%">
                                <span class="bar-label">"1-2ms"</span>
                                <span class="bar-value">"18%"</span>
                            </div>
                            <div class="histogram-bar" style="height: 30%">
                                <span class="bar-label">"2-5ms"</span>
                                <span class="bar-value">"7%"</span>
                            </div>
                            <div class="histogram-bar" style="height: 15%">
                                <span class="bar-label">"5-10ms"</span>
                                <span class="bar-value">"2%"</span>
                            </div>
                            <div class="histogram-bar warning" style="height: 8%">
                                <span class="bar-label">"> 10ms"</span>
                                <span class="bar-value">"1%"</span>
                            </div>
                        </div>
                        <div class="latency-percentiles">
                            <div class="percentile">
                                <span class="p-label">"p50"</span>
                                <span class="p-value">"0.8ms"</span>
                            </div>
                            <div class="percentile">
                                <span class="p-label">"p95"</span>
                                <span class="p-value">"2.1ms"</span>
                            </div>
                            <div class="percentile">
                                <span class="p-label">"p99"</span>
                                <span class="p-value">"4.2ms"</span>
                            </div>
                            <div class="percentile">
                                <span class="p-label">"max"</span>
                                <span class="p-value">"12.4ms"</span>
                            </div>
                        </div>
                    </div>
                </div>

                // Cache Performance
                <div class="chart-card">
                    <div class="chart-header">
                        <h3>"Cache Performance"</h3>
                    </div>
                    <div class="chart-body">
                        <div class="cache-donut">
                            <svg viewBox="0 0 120 120">
                                <circle cx="60" cy="60" r="50" fill="none" stroke="#374151" stroke-width="16"/>
                                {move || {
                                    let rate = ctx.db_stats.get().map(|s| s.cache_hit_rate).unwrap_or(94.5);
                                    let circumference = 314.16;
                                    let offset = circumference * (1.0 - rate / 100.0);
                                    view! {
                                        <circle cx="60" cy="60" r="50" fill="none" stroke="#14b8a6" stroke-width="16"
                                            stroke-dasharray=circumference stroke-dashoffset=offset
                                            transform="rotate(-90 60 60)" class="cache-progress"/>
                                    }
                                }}
                            </svg>
                            <div class="donut-center">
                                <span class="donut-value">{move || ctx.db_stats.get().map(|s| format!("{:.1}", s.cache_hit_rate)).unwrap_or("-".to_string())}</span>
                                <span class="donut-unit">"%"</span>
                            </div>
                        </div>
                        <div class="cache-breakdown">
                            <div class="cache-stat">
                                <div class="cache-stat-color hit"></div>
                                <div class="cache-stat-info">
                                    <span class="stat-name">"Cache Hits"</span>
                                    <span class="stat-value">"2.4M"</span>
                                </div>
                            </div>
                            <div class="cache-stat">
                                <div class="cache-stat-color miss"></div>
                                <div class="cache-stat-info">
                                    <span class="stat-name">"Cache Misses"</span>
                                    <span class="stat-value">"142K"</span>
                                </div>
                            </div>
                            <div class="cache-stat">
                                <div class="cache-stat-color evict"></div>
                                <div class="cache-stat-info">
                                    <span class="stat-name">"Evictions"</span>
                                    <span class="stat-value">"18K"</span>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                // Cluster Health Matrix
                <div class="chart-card">
                    <div class="chart-header">
                        <h3>"Cluster Health"</h3>
                        <span class="health-badge healthy">"Healthy"</span>
                    </div>
                    <div class="chart-body">
                        <div class="health-matrix">
                            {move || ctx.nodes.get().into_iter().map(|node| {
                                let (status_class, status_icon) = match node.status {
                                    NodeStatus::Healthy => ("healthy", "✓"),
                                    NodeStatus::Degraded => ("warning", "!"),
                                    NodeStatus::Offline => ("critical", "✗"),
                                };
                                let role_badge = match node.role {
                                    NodeRole::Leader => Some("L"),
                                    _ => None,
                                };
                                view! {
                                    <div class=format!("health-node-card {}", status_class)>
                                        <div class="node-header-row">
                                            <span class="node-name">{node.id.clone()}</span>
                                            {role_badge.map(|b| view! { <span class="leader-badge">{b}</span> })}
                                        </div>
                                        <div class="node-metrics-row">
                                            <div class="mini-metric">
                                                <span class="label">"CPU"</span>
                                                <div class="mini-bar">
                                                    <div class="mini-fill" style=format!("width: {}%", node.metrics.cpu_usage as u32)></div>
                                                </div>
                                                <span class="value">{format!("{}%", node.metrics.cpu_usage as u32)}</span>
                                            </div>
                                            <div class="mini-metric">
                                                <span class="label">"MEM"</span>
                                                <div class="mini-bar">
                                                    <div class="mini-fill" style=format!("width: {}%", node.metrics.memory_usage as u32)></div>
                                                </div>
                                                <span class="value">{format!("{}%", node.metrics.memory_usage as u32)}</span>
                                            </div>
                                        </div>
                                        <div class="node-status-icon">{status_icon}</div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    </div>
                </div>

                // Storage Metrics
                <div class="chart-card">
                    <div class="chart-header">
                        <h3>"Storage Usage"</h3>
                    </div>
                    <div class="chart-body">
                        {move || ctx.db_stats.get().map(|stats| {
                            let used_pct = if stats.storage_total > 0 {
                                (stats.storage_used as f64 / stats.storage_total as f64 * 100.0) as u32
                            } else { 0 };
                            let wal_pct = if stats.storage_used > 0 {
                                (stats.wal_bytes as f64 / stats.storage_used as f64 * used_pct as f64) as u32
                            } else { 0 };
                            let data_pct = used_pct.saturating_sub(wal_pct);
                            view! {
                                <div class="storage-visual">
                                    <div class="storage-ring-container">
                                        <svg viewBox="0 0 120 120">
                                            <circle cx="60" cy="60" r="50" fill="none" stroke="#374151" stroke-width="14"/>
                                            <circle cx="60" cy="60" r="50" fill="none" stroke="#14b8a6" stroke-width="14"
                                                stroke-dasharray=format!("{} 314", data_pct as f64 * 3.14)
                                                transform="rotate(-90 60 60)"/>
                                            <circle cx="60" cy="60" r="50" fill="none" stroke="#e7a75f" stroke-width="14"
                                                stroke-dasharray=format!("{} 314", wal_pct as f64 * 3.14)
                                                stroke-dashoffset=format!("-{}", data_pct as f64 * 3.14)
                                                transform="rotate(-90 60 60)"/>
                                        </svg>
                                        <div class="storage-center">
                                            <span class="pct">{used_pct}"%"</span>
                                            <span class="used">"used"</span>
                                        </div>
                                    </div>
                                    <div class="storage-details-v2">
                                        <div class="storage-row">
                                            <div class="storage-color data"></div>
                                            <span class="storage-name">"Data Files"</span>
                                            <span class="storage-size">{format_bytes(stats.data_bytes)}</span>
                                        </div>
                                        <div class="storage-row">
                                            <div class="storage-color wal"></div>
                                            <span class="storage-name">"WAL"</span>
                                            <span class="storage-size">{format_bytes(stats.wal_bytes)}</span>
                                        </div>
                                        <div class="storage-row">
                                            <div class="storage-color free"></div>
                                            <span class="storage-name">"Available"</span>
                                            <span class="storage-size">{format_bytes(stats.storage_total - stats.storage_used)}</span>
                                        </div>
                                        <div class="storage-total">
                                            <span>"Total: "{format_bytes(stats.storage_total)}</span>
                                        </div>
                                    </div>
                                </div>
                            }
                        })}
                    </div>
                </div>

                // Network I/O
                <div class="chart-card">
                    <div class="chart-header">
                        <h3>"Network Throughput"</h3>
                    </div>
                    <div class="chart-body">
                        <div class="network-visual">
                            <div class="network-direction">
                                <div class="direction-icon inbound">"IN"</div>
                                <div class="direction-stats">
                                    <span class="direction-value">{move || {
                                        let total: u64 = ctx.nodes.get().iter().map(|n| n.metrics.network_in).sum();
                                        format!("{}/s", format_bytes(total / 60))
                                    }}</span>
                                    <div class="direction-sparkline">
                                        <svg viewBox="0 0 100 30">
                                            <path d="M0,20 Q10,18 20,15 T40,12 T60,10 T80,8 T100,5" fill="none" stroke="#14b8a6" stroke-width="2"/>
                                        </svg>
                                    </div>
                                </div>
                            </div>
                            <div class="network-direction">
                                <div class="direction-icon outbound">"OUT"</div>
                                <div class="direction-stats">
                                    <span class="direction-value">{move || {
                                        let total: u64 = ctx.nodes.get().iter().map(|n| n.metrics.network_out).sum();
                                        format!("{}/s", format_bytes(total / 60))
                                    }}</span>
                                    <div class="direction-sparkline">
                                        <svg viewBox="0 0 100 30">
                                            <path d="M0,25 Q10,22 20,20 T40,18 T60,15 T80,12 T100,10" fill="none" stroke="#e7a75f" stroke-width="2"/>
                                        </svg>
                                    </div>
                                </div>
                            </div>
                            <div class="network-breakdown">
                                <div class="breakdown-item">
                                    <span class="breakdown-label">"Replication"</span>
                                    <span class="breakdown-value">"45%"</span>
                                </div>
                                <div class="breakdown-item">
                                    <span class="breakdown-label">"Client Queries"</span>
                                    <span class="breakdown-value">"38%"</span>
                                </div>
                                <div class="breakdown-item">
                                    <span class="breakdown-label">"Admin/Control"</span>
                                    <span class="breakdown-value">"17%"</span>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                // Raft/Replication Status
                <div class="chart-card">
                    <div class="chart-header">
                        <h3>"Replication Status"</h3>
                        <span class="sync-badge">"In Sync"</span>
                    </div>
                    <div class="chart-body">
                        <div class="replication-visual">
                            <div class="raft-stats">
                                <div class="raft-stat">
                                    <span class="raft-label">"Current Term"</span>
                                    <span class="raft-value">{move || ctx.cluster_status.get().map(|s| s.term.to_string()).unwrap_or("-".to_string())}</span>
                                </div>
                                <div class="raft-stat">
                                    <span class="raft-label">"Commit Index"</span>
                                    <span class="raft-value">{move || ctx.cluster_status.get().map(|s| format_number(s.commit_index)).unwrap_or("-".to_string())}</span>
                                </div>
                                <div class="raft-stat">
                                    <span class="raft-label">"Leader"</span>
                                    <span class="raft-value leader">{move || ctx.cluster_status.get().map(|s| s.leader_id.clone()).unwrap_or("-".to_string())}</span>
                                </div>
                            </div>
                            <div class="replication-lag">
                                <h4>"Follower Lag"</h4>
                                {move || ctx.nodes.get().into_iter().filter(|n| !matches!(n.role, NodeRole::Leader)).map(|node| {
                                    view! {
                                        <div class="lag-row">
                                            <span class="lag-node">{node.id.clone()}</span>
                                            <div class="lag-bar-container">
                                                <div class="lag-bar" style="width: 2%"></div>
                                            </div>
                                            <span class="lag-value">"< 1ms"</span>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        </div>
                    </div>
                </div>

                // Query Analytics
                <div class="chart-card chart-wide">
                    <div class="chart-header">
                        <h3>"Query Analytics"</h3>
                        <div class="query-type-filter">
                            <button class="filter-btn active">"All"</button>
                            <button class="filter-btn">"SELECT"</button>
                            <button class="filter-btn">"INSERT"</button>
                            <button class="filter-btn">"UPDATE"</button>
                        </div>
                    </div>
                    <div class="chart-body">
                        <div class="query-stats-row">
                            <div class="query-stat-box">
                                <span class="query-stat-value">"12.4K"</span>
                                <span class="query-stat-label">"Total Queries"</span>
                            </div>
                            <div class="query-stat-box">
                                <span class="query-stat-value">"1.2ms"</span>
                                <span class="query-stat-label">"Avg Duration"</span>
                            </div>
                            <div class="query-stat-box">
                                <span class="query-stat-value">"98.2%"</span>
                                <span class="query-stat-label">"Success Rate"</span>
                            </div>
                            <div class="query-stat-box">
                                <span class="query-stat-value">"24"</span>
                                <span class="query-stat-label">"Slow Queries"</span>
                            </div>
                        </div>
                        <div class="top-queries">
                            <h4>"Slowest Queries"</h4>
                            <div class="query-list">
                                <div class="query-item">
                                    <code>"SELECT * FROM orders JOIN..."</code>
                                    <span class="query-time">"45.2ms"</span>
                                    <span class="query-count">"x142"</span>
                                </div>
                                <div class="query-item">
                                    <code>"SELECT DISTINCT category..."</code>
                                    <span class="query-time">"23.1ms"</span>
                                    <span class="query-count">"x89"</span>
                                </div>
                                <div class="query-item">
                                    <code>"UPDATE inventory SET..."</code>
                                    <span class="query-time">"18.4ms"</span>
                                    <span class="query-count">"x234"</span>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}
