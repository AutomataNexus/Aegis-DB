//! Security settings component

use leptos::*;
use super::super::state::use_dashboard_context;

/// Security settings tab
#[component]
pub fn SecuritySettings() -> impl IntoView {
    let ctx = use_dashboard_context();

    let save_settings = move |_| {
        ctx.settings_message.set(Some(("Security settings saved".to_string(), true)));
    };

    view! {
        <div class="settings-section">
            <h3>"Authentication"</h3>

            <div class="settings-group">
                <div class="setting-item">
                    <label class="toggle-label">
                        <input
                            type="checkbox"
                            prop:checked=move || ctx.tls_enabled.get()
                            on:change=move |e| ctx.tls_enabled.set(event_target_checked(&e))
                        />
                        <span>"Enable TLS/SSL"</span>
                    </label>
                    <p class="setting-help">"Encrypt all network communications"</p>
                </div>

                <div class="setting-item">
                    <label class="toggle-label">
                        <input
                            type="checkbox"
                            prop:checked=move || ctx.auth_required.get()
                            on:change=move |e| ctx.auth_required.set(event_target_checked(&e))
                        />
                        <span>"Require Authentication"</span>
                    </label>
                </div>

                <div class="setting-item">
                    <label>"Session Timeout"</label>
                    <select
                        prop:value=move || ctx.session_timeout.get()
                        on:change=move |e| ctx.session_timeout.set(event_target_value(&e))
                    >
                        <option value="15m">"15 minutes"</option>
                        <option value="30m">"30 minutes"</option>
                        <option value="1h">"1 hour"</option>
                        <option value="8h">"8 hours"</option>
                        <option value="24h">"24 hours"</option>
                    </select>
                </div>
            </div>

            <h3>"Two-Factor Authentication"</h3>

            <div class="settings-group">
                <div class="setting-item">
                    <label class="toggle-label">
                        <input
                            type="checkbox"
                            prop:checked=move || ctx.require_2fa.get()
                            on:change=move |e| ctx.require_2fa.set(event_target_checked(&e))
                        />
                        <span>"Require 2FA for all users"</span>
                    </label>
                </div>

                <div class="mfa-methods">
                    <h4>"Allowed MFA Methods"</h4>
                    <div class="setting-item">
                        <label class="toggle-label">
                            <input
                                type="checkbox"
                                prop:checked=move || ctx.totp_enabled.get()
                                on:change=move |e| ctx.totp_enabled.set(event_target_checked(&e))
                            />
                            <span>"TOTP (Authenticator App)"</span>
                        </label>
                    </div>
                    <div class="setting-item">
                        <label class="toggle-label">
                            <input
                                type="checkbox"
                                prop:checked=move || ctx.sms_enabled.get()
                                on:change=move |e| ctx.sms_enabled.set(event_target_checked(&e))
                            />
                            <span>"SMS"</span>
                        </label>
                    </div>
                    <div class="setting-item">
                        <label class="toggle-label">
                            <input
                                type="checkbox"
                                prop:checked=move || ctx.webauthn_enabled.get()
                                on:change=move |e| ctx.webauthn_enabled.set(event_target_checked(&e))
                            />
                            <span>"WebAuthn / Security Keys"</span>
                        </label>
                    </div>
                    <div class="setting-item">
                        <label class="toggle-label">
                            <input
                                type="checkbox"
                                prop:checked=move || ctx.recovery_codes_enabled.get()
                                on:change=move |e| ctx.recovery_codes_enabled.set(event_target_checked(&e))
                            />
                            <span>"Recovery Codes"</span>
                        </label>
                    </div>
                </div>
            </div>

            <h3>"Audit Logging"</h3>

            <div class="settings-group">
                <div class="setting-item">
                    <label class="toggle-label">
                        <input
                            type="checkbox"
                            prop:checked=move || ctx.audit_logging_enabled.get()
                            on:change=move |e| ctx.audit_logging_enabled.set(event_target_checked(&e))
                        />
                        <span>"Enable Audit Logging"</span>
                    </label>
                    <p class="setting-help">"Log all administrative actions and data access"</p>
                </div>
            </div>

            <div class="settings-actions">
                <button class="btn btn-primary" on:click=save_settings>"Save Security Settings"</button>
            </div>
        </div>
    }
}
