//! General settings component

use leptos::*;
use super::super::state::use_dashboard_context;

/// General settings tab
#[component]
pub fn GeneralSettings() -> impl IntoView {
    let ctx = use_dashboard_context();

    let save_settings = move |_| {
        ctx.settings_message.set(Some(("Settings saved successfully".to_string(), true)));
        // In production, this would call the API
    };

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
                <button class="btn btn-primary" on:click=save_settings>"Save Changes"</button>
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
