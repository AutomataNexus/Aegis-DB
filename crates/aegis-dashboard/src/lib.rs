//! Aegis DB Dashboard - Leptos Web Interface
//!
//! A full-stack Rust dashboard for managing and monitoring Aegis DB clusters.

pub mod pages;
pub mod types;
pub mod api;
pub mod state;

use leptos::*;
use leptos_router::*;

use pages::{Landing, Login, Dashboard};
use state::provide_app_state;

/// Main application component
#[component]
pub fn App() -> impl IntoView {
    // Initialize app state
    provide_app_state();

    view! {
        <Router>
            <main>
                <Routes>
                    <Route path="/" view=Landing />
                    <Route path="/login" view=Login />
                    <Route path="/dashboard" view=Dashboard />
                    <Route path="/dashboard/*any" view=Dashboard />
                </Routes>
            </main>
        </Router>
    }
}

/// Mount the application to the DOM
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).expect("Failed to init logger");

    mount_to_body(App);
}
