//! Settings page components

mod general;
mod security;
mod users;

pub use general::GeneralSettings;
pub use security::SecuritySettings;
pub use users::UsersSettings;

use leptos::*;
use super::state::use_dashboard_context;

/// Settings page with tabs
#[component]
pub fn SettingsPage() -> impl IntoView {
    let ctx = use_dashboard_context();

    view! {
        <div class="settings-page">
            // Settings tabs
            <div class="settings-tabs">
                <button
                    class=move || if ctx.settings_tab.get() == "general" { "tab-btn active" } else { "tab-btn" }
                    on:click=move |_| ctx.settings_tab.set("general".to_string())
                >"General"</button>
                <button
                    class=move || if ctx.settings_tab.get() == "security" { "tab-btn active" } else { "tab-btn" }
                    on:click=move |_| ctx.settings_tab.set("security".to_string())
                >"Security"</button>
                <button
                    class=move || if ctx.settings_tab.get() == "users" { "tab-btn active" } else { "tab-btn" }
                    on:click=move |_| ctx.settings_tab.set("users".to_string())
                >"Users & Roles"</button>
            </div>

            // Message display
            {move || ctx.settings_message.get().map(|(msg, is_success)| view! {
                <div class=if is_success { "message success" } else { "message error" }>{msg}</div>
            })}

            // Tab content
            <div class="settings-content">
                <Show when=move || ctx.settings_tab.get() == "general">
                    <GeneralSettings />
                </Show>
                <Show when=move || ctx.settings_tab.get() == "security">
                    <SecuritySettings />
                </Show>
                <Show when=move || ctx.settings_tab.get() == "users">
                    <UsersSettings />
                </Show>
            </div>
        </div>
    }
}
