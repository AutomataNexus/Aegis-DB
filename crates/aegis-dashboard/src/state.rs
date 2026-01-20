//! Application state management for Aegis Dashboard

use leptos::*;
use crate::types::User;
use web_sys::window;

/// Global application state
#[derive(Clone)]
pub struct AppState {
    pub user: RwSignal<Option<User>>,
    pub token: RwSignal<Option<String>>,
    pub is_authenticated: Signal<bool>,
}

impl AppState {
    pub fn new() -> Self {
        let user = create_rw_signal(None);
        let token = create_rw_signal(None);
        let is_authenticated = Signal::derive(move || token.get().is_some());

        Self {
            user,
            token,
            is_authenticated,
        }
    }

    /// Load session from storage
    pub fn load_from_storage(&self) {
        if let Some(window) = window() {
            // Try localStorage first, then sessionStorage
            let storage = window.local_storage().ok().flatten()
                .or_else(|| window.session_storage().ok().flatten());

            if let Some(storage) = storage {
                if let Ok(Some(token)) = storage.get_item("aegis_token") {
                    self.token.set(Some(token));
                }

                if let Ok(Some(user_json)) = storage.get_item("aegis_user") {
                    if let Ok(user) = serde_json::from_str(&user_json) {
                        self.user.set(Some(user));
                    }
                }
            }
        }
    }

    /// Save session to storage
    pub fn save_to_storage(&self, remember: bool) {
        if let Some(window) = window() {
            let storage = if remember {
                window.local_storage().ok().flatten()
            } else {
                window.session_storage().ok().flatten()
            };

            if let Some(storage) = storage {
                if let Some(token) = self.token.get_untracked() {
                    let _ = storage.set_item("aegis_token", &token);
                }

                if let Some(user) = self.user.get_untracked() {
                    if let Ok(user_json) = serde_json::to_string(&user) {
                        let _ = storage.set_item("aegis_user", &user_json);
                    }
                }
            }
        }
    }

    /// Clear session from storage and state
    pub fn logout(&self) {
        self.user.set(None);
        self.token.set(None);

        if let Some(window) = window() {
            if let Ok(Some(storage)) = window.local_storage() {
                let _ = storage.remove_item("aegis_token");
                let _ = storage.remove_item("aegis_user");
            }
            if let Ok(Some(storage)) = window.session_storage() {
                let _ = storage.remove_item("aegis_token");
                let _ = storage.remove_item("aegis_user");
            }
        }
    }

    /// Login with token and user
    pub fn login(&self, token: String, user: User, remember: bool) {
        self.token.set(Some(token));
        self.user.set(Some(user));
        self.save_to_storage(remember);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Provide application state context
pub fn provide_app_state() -> AppState {
    let state = AppState::new();
    state.load_from_storage();
    provide_context(state.clone());
    state
}

/// Use application state from context
pub fn use_app_state() -> AppState {
    expect_context::<AppState>()
}
