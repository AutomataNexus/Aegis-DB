//! Login page component with MFA support
//! Matches NexusForge styling

use leptos::*;
use leptos_router::*;
use crate::api;
use crate::state::use_app_state;
use crate::types::MfaSetupData;

/// Authentication step
#[derive(Clone, Copy, PartialEq)]
enum AuthStep {
    Login,
    MfaVerify,
    MfaSetup,
}

/// Login page component
#[component]
pub fn Login() -> impl IntoView {
    let navigate = use_navigate();
    let nav_login = navigate.clone();
    let nav_mfa = navigate.clone();
    let nav_back = store_value(navigate.clone());
    let app_state = use_app_state();
    let app_state_login = app_state.clone();
    let app_state_mfa = app_state.clone();

    // State
    let (auth_step, set_auth_step) = create_signal(AuthStep::Login);
    let (show_form, set_show_form) = create_signal(false);
    let (show_password, set_show_password) = create_signal(false);
    let (remember_me, set_remember_me) = create_signal(false);
    let (loading, set_loading) = create_signal(false);
    let (message, set_message) = create_signal::<Option<(String, String)>>(None); // (type, text)

    let (username, set_username) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    let (mfa_code, set_mfa_code) = create_signal(String::new());
    let (temp_token, set_temp_token) = create_signal(String::new());
    let (mfa_setup, set_mfa_setup) = create_signal::<Option<MfaSetupData>>(None);

    // Show form after animation delay
    set_timeout(move || set_show_form.set(true), std::time::Duration::from_millis(500));

    // Handle login
    let handle_login = create_action(move |_: &()| {
        let username_val = username.get();
        let password_val = password.get();
        let navigate = nav_login.clone();
        let app = app_state_login.clone();

        async move {
            set_loading.set(true);
            set_message.set(None);

            match api::login(&username_val, &password_val).await {
                Ok(response) => {
                    if response.requires_mfa == Some(true) {
                        set_temp_token.set(response.token.unwrap_or_default());
                        set_auth_step.set(AuthStep::MfaVerify);
                        set_message.set(Some(("info".to_string(), "Please enter your authenticator code.".to_string())));
                    } else if response.requires_mfa_setup == Some(true) {
                        set_temp_token.set(response.token.unwrap_or_default());
                        set_mfa_setup.set(response.mfa_setup_data);
                        set_auth_step.set(AuthStep::MfaSetup);
                        set_message.set(Some(("info".to_string(), "MFA is required. Please set up your authenticator app.".to_string())));
                    } else if let (Some(token), Some(user)) = (response.token, response.user) {
                        app.login(token, user, remember_me.get());
                        navigate("/dashboard", Default::default());
                    }
                }
                Err(e) => {
                    set_message.set(Some(("error".to_string(), e)));
                }
            }

            set_loading.set(false);
        }
    });

    // Handle MFA verification
    let handle_mfa_verify = create_action(move |_: &()| {
        let code = mfa_code.get();
        let token = temp_token.get();
        let navigate = nav_mfa.clone();
        let app = app_state_mfa.clone();

        async move {
            set_loading.set(true);
            set_message.set(None);

            match api::verify_mfa(&code, &token).await {
                Ok(response) => {
                    if let (Some(token), Some(user)) = (response.token, response.user) {
                        set_message.set(Some(("success".to_string(), "MFA verified! Welcome to Aegis DB.".to_string())));

                        // Delay navigation for success message
                        set_timeout(move || {
                            app.login(token, user, remember_me.get_untracked());
                            navigate("/dashboard", Default::default());
                        }, std::time::Duration::from_millis(1000));
                    }
                }
                Err(e) => {
                    set_message.set(Some(("error".to_string(), e)));
                    set_mfa_code.set(String::new());
                }
            }

            set_loading.set(false);
        }
    });

    // Reset to login
    let reset_to_login = move |_| {
        set_auth_step.set(AuthStep::Login);
        set_mfa_code.set(String::new());
        set_temp_token.set(String::new());
        set_mfa_setup.set(None);
        set_message.set(None);
    };


    view! {
        <div class="login-container">
            <div class="login-content">
                // Logo Section
                <div class="logo-section">
                    <div class="logo">
                        <svg viewBox="0 0 100 100" width="64" height="64">
                            <defs>
                                <linearGradient id="logoGrad" x1="0%" y1="0%" x2="100%" y2="100%">
                                    <stop offset="0%" style="stop-color:#14b8a6"/>
                                    <stop offset="100%" style="stop-color:#0d9488"/>
                                </linearGradient>
                            </defs>
                            <rect width="100" height="100" rx="20" fill="url(#logoGrad)"/>
                            <path d="M30 35 L50 25 L70 35 L70 65 L50 75 L30 65 Z" fill="none" stroke="white" stroke-width="4" stroke-linejoin="round"/>
                            <path d="M30 45 L50 35 L70 45" fill="none" stroke="white" stroke-width="3"/>
                            <path d="M30 55 L50 45 L70 55" fill="none" stroke="white" stroke-width="3"/>
                            <circle cx="50" cy="50" r="6" fill="white"/>
                        </svg>
                    </div>
                    <h1 class="welcome-text">"Aegis DB"</h1>
                    <div class="design-line"></div>
                    <div class="secured-text">
                        <svg class="shield-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                        </svg>
                        <span>"Secured by AutomataNexus"</span>
                    </div>
                </div>

                // Form Section
                <Show when=move || show_form.get()>
                    <div class="form-section">
                        // Message display
                        <Show when=move || message.get().is_some()>
                            {move || {
                                let (msg_type, msg_text) = message.get().unwrap_or_default();
                                view! {
                                    <div class=format!("message {}", msg_type)>
                                        {match msg_type.as_str() {
                                            "success" => view! {
                                                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                                    <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
                                                    <polyline points="22 4 12 14.01 9 11.01"/>
                                                </svg>
                                            }.into_view(),
                                            "error" => view! {
                                                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                                    <circle cx="12" cy="12" r="10"/>
                                                    <line x1="12" y1="8" x2="12" y2="12"/>
                                                    <line x1="12" y1="16" x2="12.01" y2="16"/>
                                                </svg>
                                            }.into_view(),
                                            _ => view! {
                                                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                                    <rect x="5" y="2" width="14" height="20" rx="2" ry="2"/>
                                                    <line x1="12" y1="18" x2="12.01" y2="18"/>
                                                </svg>
                                            }.into_view(),
                                        }}
                                        {msg_text}
                                    </div>
                                }
                            }}
                        </Show>

                        // Login Form
                        <Show when=move || auth_step.get() == AuthStep::Login>
                            <div class="form-header">
                                <span>"Sign in to Dashboard"</span>
                            </div>

                            <form class="auth-form" on:submit=move |ev| {
                                ev.prevent_default();
                                handle_login.dispatch(());
                            }>
                                <div class="input-group">
                                    <svg class="input-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                        <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
                                        <circle cx="12" cy="7" r="4"/>
                                    </svg>
                                    <input
                                        type="text"
                                        placeholder="Username"
                                        prop:value=username
                                        on:input=move |ev| set_username.set(event_target_value(&ev))
                                        autocomplete="username"
                                        required
                                    />
                                </div>

                                <div class="input-group">
                                    <svg class="input-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                        <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
                                        <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
                                    </svg>
                                    <input
                                        type=move || if show_password.get() { "text" } else { "password" }
                                        placeholder="Password"
                                        prop:value=password
                                        on:input=move |ev| set_password.set(event_target_value(&ev))
                                        autocomplete="current-password"
                                        required
                                    />
                                    <button
                                        type="button"
                                        class="password-toggle"
                                        on:click=move |_| set_show_password.update(|v| *v = !*v)
                                        tabindex=-1
                                    >
                                        <Show
                                            when=move || show_password.get()
                                            fallback=|| view! {
                                                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                                                    <circle cx="12" cy="12" r="3"/>
                                                </svg>
                                            }
                                        >
                                            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                                <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/>
                                                <line x1="1" y1="1" x2="23" y2="23"/>
                                            </svg>
                                        </Show>
                                    </button>
                                </div>

                                <div class="remember-me">
                                    <input
                                        type="checkbox"
                                        id="rememberMe"
                                        prop:checked=remember_me
                                        on:change=move |ev| set_remember_me.set(event_target_checked(&ev))
                                    />
                                    <label for="rememberMe">"Remember me for 30 days"</label>
                                </div>

                                <button type="submit" class="auth-button" disabled=move || loading.get()>
                                    <Show when=move || loading.get() fallback=|| view! { "Sign In" }>
                                        <div class="spinner" style="width: 20px; height: 20px;"></div>
                                        <span>"Signing In..."</span>
                                    </Show>
                                </button>

                                <button type="button" class="back-button" on:click=move |_| nav_back.get_value()("/", Default::default())>
                                    "Back to Home"
                                </button>
                            </form>

                            <div class="demo-hint">
                                <strong>"Demo credentials:"</strong><br/>
                                "admin / admin (with MFA)"<br/>
                                "demo / demo (no MFA)"
                            </div>
                        </Show>

                        // MFA Verification
                        <Show when=move || auth_step.get() == AuthStep::MfaVerify>
                            <div class="mfa-header">
                                <svg class="mfa-icon" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                    <rect x="5" y="2" width="14" height="20" rx="2" ry="2"/>
                                    <line x1="12" y1="18" x2="12.01" y2="18"/>
                                </svg>
                                <h2>"Two-Factor Authentication"</h2>
                                <p>"Enter the code from your authenticator app"</p>
                            </div>

                            <form class="auth-form" on:submit=move |ev| {
                                ev.prevent_default();
                                handle_mfa_verify.dispatch(());
                            }>
                                <div class="input-group">
                                    <svg class="input-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                        <path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/>
                                    </svg>
                                    <input
                                        type="text"
                                        placeholder="Enter 6-digit code"
                                        prop:value=mfa_code
                                        on:input=move |ev| {
                                            let value: String = event_target_value(&ev)
                                                .chars()
                                                .filter(|c| c.is_ascii_digit())
                                                .take(6)
                                                .collect();
                                            set_mfa_code.set(value);
                                        }
                                        maxlength="6"
                                        autocomplete="one-time-code"
                                        required
                                    />
                                </div>

                                <button
                                    type="submit"
                                    class="auth-button"
                                    disabled=move || loading.get() || mfa_code.get().len() != 6
                                >
                                    <Show when=move || loading.get() fallback=|| view! { "Verify Code" }>
                                        <div class="spinner" style="width: 20px; height: 20px;"></div>
                                        <span>"Verifying..."</span>
                                    </Show>
                                </button>

                                <button type="button" class="back-button" on:click=reset_to_login>
                                    "Back to Login"
                                </button>
                            </form>

                            <div class="demo-hint">
                                <strong>"Demo:"</strong>" Enter 123456 to verify"
                            </div>
                        </Show>

                        // MFA Setup
                        <Show when=move || auth_step.get() == AuthStep::MfaSetup>
                            <div class="mfa-header">
                                <svg class="mfa-icon" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                    <rect x="5" y="2" width="14" height="20" rx="2" ry="2"/>
                                    <line x1="12" y1="18" x2="12.01" y2="18"/>
                                </svg>
                                <h2>"Set Up Two-Factor Authentication"</h2>
                                <p>"Scan this QR code with your authenticator app"</p>
                            </div>

                            {move || {
                                mfa_setup.get().map(|data| {
                                    view! {
                                        <div class="mfa-setup">
                                            <div class="qr-code">
                                                <img src=data.qr_code.clone() alt="MFA QR Code"/>
                                            </div>

                                            <div class="manual-code">
                                                <p>"Or enter this code manually:"</p>
                                                <code>{data.secret.clone()}</code>
                                            </div>

                                            <div class="backup-codes">
                                                <p><strong>"Save these backup codes:"</strong></p>
                                                <div class="codes-grid">
                                                    {data.backup_codes.iter().map(|code| {
                                                        view! { <code>{code.clone()}</code> }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        </div>
                                    }
                                })
                            }}
                        </Show>
                    </div>
                </Show>
            </div>

            // Footer
            <div class="login-footer">
                <span>"Copyright 2025 AutomataNexus, LLC"</span>
            </div>
        </div>
    }
}
