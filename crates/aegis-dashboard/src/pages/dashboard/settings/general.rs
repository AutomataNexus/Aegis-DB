//! General settings component

use leptos::*;
use crate::api::{self, ServerSettings};
use super::super::state::use_dashboard_context;

/// Parse a retention period string like "7d", "30d" into days as u32.
fn parse_retention_days(period: &str) -> u32 {
    period
        .strip_suffix('d')
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(30)
}

/// Convert retention days (u32) back to the UI period string like "30d".
fn retention_days_to_period(days: u32) -> String {
    format!("{}d", days)
}

/// Parse a session timeout string like "30m", "60m" into minutes as u32.
fn parse_session_timeout_minutes(timeout: &str) -> u32 {
    timeout
        .strip_suffix('m')
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(60)
}

/// General settings tab
#[component]
pub fn GeneralSettings() -> impl IntoView {
    let ctx = use_dashboard_context();

    // Load settings from API on mount
    let load_settings = create_action({
        let ctx = ctx.clone();
        move |_: &()| {
            let ctx = ctx.clone();
            async move {
                match api::get_settings().await {
                    Ok(settings) => {
                        ctx.replication_factor.set(settings.replication_factor as i32);
                        ctx.auto_backups_enabled.set(settings.auto_backups_enabled);
                        ctx.backup_schedule.set(settings.backup_schedule);
                        ctx.retention_period.set(retention_days_to_period(settings.retention_days));
                        ctx.tls_enabled.set(settings.tls_enabled);
                        ctx.auth_required.set(settings.auth_required);
                        ctx.session_timeout.set(format!("{}m", settings.session_timeout_minutes));
                        ctx.require_2fa.set(settings.require_2fa);
                        ctx.audit_logging_enabled.set(settings.audit_logging_enabled);
                    }
                    Err(e) => {
                        ctx.settings_message.set(Some((format!("Failed to load settings: {}", e), false)));
                    }
                }
            }
        }
    });

    // Fetch settings on mount
    create_effect(move |_| {
        load_settings.dispatch(());
    });

    // Save settings via API
    let save_settings = create_action({
        let ctx = ctx.clone();
        move |_: &()| {
            let ctx = ctx.clone();
            async move {
                let settings = ServerSettings {
                    replication_factor: ctx.replication_factor.get() as u8,
                    auto_backups_enabled: ctx.auto_backups_enabled.get(),
                    backup_schedule: ctx.backup_schedule.get(),
                    retention_days: parse_retention_days(&ctx.retention_period.get()),
                    tls_enabled: ctx.tls_enabled.get(),
                    auth_required: ctx.auth_required.get(),
                    session_timeout_minutes: parse_session_timeout_minutes(&ctx.session_timeout.get()),
                    require_2fa: ctx.require_2fa.get(),
                    audit_logging_enabled: ctx.audit_logging_enabled.get(),
                };
                match api::update_settings(&settings).await {
                    Ok(()) => {
                        ctx.settings_message.set(Some(("Settings saved successfully".to_string(), true)));
                    }
                    Err(e) => {
                        ctx.settings_message.set(Some((format!("Failed to save settings: {}", e), false)));
                    }
                }
            }
        }
    });

    view! {
        <div class="settings-section">
            <h3>"Cluster Configuration"</h3>

            <div class="settings-group">
                <div class="setting-item">
                    <label>"Replication Factor"</label>
                    <select
                        prop:value=move || ctx.replication_factor.get().to_string()
                        on:change=move |e| {
                            if let Ok(v) = event_target_value(&e).parse() {
                                ctx.replication_factor.set(v);
                            }
                        }
                    >
                        <option value="1">"1 (No replication)"</option>
                        <option value="2">"2"</option>
                        <option value="3">"3 (Recommended)"</option>
                        <option value="5">"5"</option>
                    </select>
                    <p class="setting-help">"Number of copies of each data shard"</p>
                </div>
            </div>

            <h3>"Backup Configuration"</h3>

            <div class="settings-group">
                <div class="setting-item">
                    <label class="toggle-label">
                        <input
                            type="checkbox"
                            prop:checked=move || ctx.auto_backups_enabled.get()
                            on:change=move |e| ctx.auto_backups_enabled.set(event_target_checked(&e))
                        />
                        <span>"Enable Automatic Backups"</span>
                    </label>
                </div>

                <Show when=move || ctx.auto_backups_enabled.get()>
                    <div class="setting-item">
                        <label>"Backup Schedule"</label>
                        <select
                            prop:value=move || ctx.backup_schedule.get()
                            on:change=move |e| ctx.backup_schedule.set(event_target_value(&e))
                        >
                            <option value="1h">"Every hour"</option>
                            <option value="6h">"Every 6 hours"</option>
                            <option value="12h">"Every 12 hours"</option>
                            <option value="24h">"Daily"</option>
                        </select>
                    </div>

                    <div class="setting-item">
                        <label>"Retention Period"</label>
                        <select
                            prop:value=move || ctx.retention_period.get()
                            on:change=move |e| ctx.retention_period.set(event_target_value(&e))
                        >
                            <option value="7d">"7 days"</option>
                            <option value="14d">"14 days"</option>
                            <option value="30d">"30 days"</option>
                            <option value="90d">"90 days"</option>
                        </select>
                    </div>
                </Show>
            </div>

            <div class="settings-actions">
                <button class="btn btn-primary" on:click=move |_| save_settings.dispatch(())>"Save Changes"</button>
            </div>

            // Danger Zone
            <div class="danger-zone">
                <h3>"Danger Zone"</h3>
                <div class="danger-item">
                    <div class="danger-info">
                        <h4>"Reset Cluster"</h4>
                        <p>"This will delete all data and reset the cluster to its initial state."</p>
                    </div>
                    <button
                        class="btn btn-danger"
                        on:click=move |_| ctx.show_reset_confirm.set(true)
                    >"Reset Cluster"</button>
                </div>
            </div>

            // Reset confirmation modal
            <Show when=move || ctx.show_reset_confirm.get()>
                <div class="modal-overlay" on:click=move |_| ctx.show_reset_confirm.set(false)>
                    <div class="modal-content danger-modal" on:click=|e| e.stop_propagation()>
                        <h3>"Confirm Cluster Reset"</h3>
                        <p>"This action is irreversible. Type 'RESET' to confirm:"</p>
                        <input
                            type="text"
                            placeholder="Type RESET"
                            prop:value=move || ctx.reset_confirm_text.get()
                            on:input=move |e| ctx.reset_confirm_text.set(event_target_value(&e))
                        />
                        <div class="modal-actions">
                            <button
                                class="btn btn-secondary"
                                on:click=move |_| {
                                    ctx.show_reset_confirm.set(false);
                                    ctx.reset_confirm_text.set(String::new());
                                }
                            >"Cancel"</button>
                            <button
                                class="btn btn-danger"
                                disabled=move || ctx.reset_confirm_text.get() != "RESET"
                                on:click=move |_| {
                                    ctx.settings_message.set(Some(("Cluster reset initiated".to_string(), true)));
                                    ctx.show_reset_confirm.set(false);
                                    ctx.reset_confirm_text.set(String::new());
                                }
                            >"Reset Cluster"</button>
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}
