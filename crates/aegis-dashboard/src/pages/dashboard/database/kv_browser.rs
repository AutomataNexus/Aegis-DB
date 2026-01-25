//! Key-Value browser modal component

use leptos::*;
use crate::api;
use super::super::shared::types::*;
use super::super::state::use_dashboard_context;

/// Key-Value browser modal with CRUD operations
#[component]
pub fn KvBrowserModal() -> impl IntoView {
    let ctx = use_dashboard_context();

    let close_modal = move |_| {
        ctx.active_modal.set(BrowserModal::None);
        ctx.kv_show_add_form.set(false);
        ctx.kv_edit_key.set(None);
        ctx.kv_message.set(None);
    };

    // Load KV entries when modal opens
    let load_entries = create_action(move |_: &()| async move {
        ctx.modal_loading.set(true);
        if let Ok(entries) = api::list_keys().await {
            ctx.kv_entries.set(entries);
        }
        ctx.modal_loading.set(false);
    });

    // Add new key
    let add_key = create_action(move |_: &()| async move {
        let key = ctx.kv_new_key.get();
        let value = ctx.kv_new_value.get();
        let ttl = ctx.kv_new_ttl.get().parse::<u64>().ok();

        if key.is_empty() {
            ctx.kv_message.set(Some(("Key cannot be empty".to_string(), false)));
            return;
        }

        let json_value = serde_json::Value::String(value);
        match api::set_key(&key, json_value, ttl).await {
            Ok(_) => {
                ctx.kv_message.set(Some(("Key added successfully".to_string(), true)));
                ctx.kv_new_key.set(String::new());
                ctx.kv_new_value.set(String::new());
                ctx.kv_new_ttl.set(String::new());
                ctx.kv_show_add_form.set(false);
                load_entries.dispatch(());
            }
            Err(e) => ctx.kv_message.set(Some((e, false))),
        }
    });

    // Delete key
    let delete_key = create_action(move |key: &String| {
        let key = key.clone();
        async move {
            match api::delete_key(&key).await {
                Ok(_) => {
                    ctx.kv_message.set(Some(("Key deleted".to_string(), true)));
                    load_entries.dispatch(());
                }
                Err(e) => ctx.kv_message.set(Some((e, false))),
            }
        }
    });

    // Update key
    let update_key = create_action(move |_: &()| async move {
        if let Some(key) = ctx.kv_edit_key.get() {
            let value = ctx.kv_edit_value.get();
            let json_value = serde_json::Value::String(value);
            match api::set_key(&key, json_value, None).await {
                Ok(_) => {
                    ctx.kv_message.set(Some(("Key updated".to_string(), true)));
                    ctx.kv_edit_key.set(None);
                    load_entries.dispatch(());
                }
                Err(e) => ctx.kv_message.set(Some((e, false))),
            }
        }
    });

    // Load entries when modal opens
    create_effect(move |_| {
        if ctx.active_modal.get() == BrowserModal::KeyValue {
            load_entries.dispatch(());
        }
    });

    view! {
        <Show when=move || ctx.active_modal.get() == BrowserModal::KeyValue>
            <div class="modal-overlay" on:click=close_modal>
                <div class="modal-content kv-browser-modal" on:click=|e| e.stop_propagation()>
                    <div class="modal-header">
                        <h2>"🔑 Key-Value Browser"</h2>
                        <button class="modal-close" on:click=close_modal>"×"</button>
                    </div>
                    <div class="modal-body">
                        // Message display
                        {move || ctx.kv_message.get().map(|(msg, is_success)| view! {
                            <div class=if is_success { "message success" } else { "message error" }>{msg}</div>
                        })}

                        // Search and add
                        <div class="kv-toolbar">
                            <input
                                type="text"
                                class="search-input"
                                placeholder="Search keys..."
                                prop:value=move || ctx.kv_search.get()
                                on:input=move |e| ctx.kv_search.set(event_target_value(&e))
                            />
                            <button
                                class="btn btn-primary"
                                on:click=move |_| ctx.kv_show_add_form.set(true)
                            >"+ Add Key"</button>
                        </div>

                        // Add form
                        <Show when=move || ctx.kv_show_add_form.get()>
                            <div class="kv-add-form">
                                <input
                                    type="text"
                                    placeholder="Key"
                                    prop:value=move || ctx.kv_new_key.get()
                                    on:input=move |e| ctx.kv_new_key.set(event_target_value(&e))
                                />
                                <textarea
                                    placeholder="Value (JSON)"
                                    prop:value=move || ctx.kv_new_value.get()
                                    on:input=move |e| ctx.kv_new_value.set(event_target_value(&e))
                                ></textarea>
                                <input
                                    type="text"
                                    placeholder="TTL (seconds, optional)"
                                    prop:value=move || ctx.kv_new_ttl.get()
                                    on:input=move |e| ctx.kv_new_ttl.set(event_target_value(&e))
                                />
                                <div class="form-actions">
                                    <button class="btn btn-secondary" on:click=move |_| ctx.kv_show_add_form.set(false)>"Cancel"</button>
                                    <button class="btn btn-primary" on:click=move |_| add_key.dispatch(())>"Add"</button>
                                </div>
                            </div>
                        </Show>

                        // Loading indicator
                        <Show when=move || ctx.modal_loading.get()>
                            <div class="loading">"Loading..."</div>
                        </Show>

                        // Entries list
                        <Show when=move || !ctx.modal_loading.get()>
                            <div class="kv-list">
                                {move || {
                                    let search = ctx.kv_search.get().to_lowercase();
                                    ctx.kv_entries.get().into_iter()
                                        .filter(|e| search.is_empty() || e.key.to_lowercase().contains(&search))
                                        .map(|entry| {
                                            let key_display = entry.key.clone();
                                            let key_for_edit = entry.key.clone();
                                            let key_for_delete = entry.key.clone();
                                            let key_for_check = entry.key.clone();
                                            let value_str = serde_json::to_string_pretty(&entry.value).unwrap_or_default();
                                            let value_for_fallback = value_str.clone();
                                            let value_for_edit = value_str.clone();

                                            view! {
                                                <div class="kv-entry">
                                                    <div class="kv-key">{key_display}</div>
                                                    <Show
                                                        when={
                                                            let k = key_for_check.clone();
                                                            move || ctx.kv_edit_key.get().as_ref() == Some(&k)
                                                        }
                                                        fallback={
                                                            let v = value_for_fallback.clone();
                                                            move || view! { <pre class="kv-value">{v.clone()}</pre> }
                                                        }
                                                    >
                                                        <textarea
                                                            class="kv-edit-value"
                                                            prop:value=move || ctx.kv_edit_value.get()
                                                            on:input=move |e| ctx.kv_edit_value.set(event_target_value(&e))
                                                        ></textarea>
                                                    </Show>
                                                    <div class="kv-actions">
                                                        <Show
                                                            when={
                                                                let k = key_for_edit.clone();
                                                                move || ctx.kv_edit_key.get().as_ref() == Some(&k)
                                                            }
                                                            fallback={
                                                                let key_clone = key_for_edit.clone();
                                                                let value_clone = value_for_edit.clone();
                                                                let key_del = key_for_delete.clone();
                                                                move || view! {
                                                                    <button
                                                                        class="btn btn-small"
                                                                        on:click={
                                                                            let k = key_clone.clone();
                                                                            let v = value_clone.clone();
                                                                            move |_| {
                                                                                ctx.kv_edit_key.set(Some(k.clone()));
                                                                                ctx.kv_edit_value.set(v.clone());
                                                                            }
                                                                        }
                                                                    >"Edit"</button>
                                                                    <button
                                                                        class="btn btn-small btn-danger"
                                                                        on:click={
                                                                            let k = key_del.clone();
                                                                            move |_| delete_key.dispatch(k.clone())
                                                                        }
                                                                    >"Delete"</button>
                                                                }
                                                            }
                                                        >
                                                            <button class="btn btn-small btn-primary" on:click=move |_| update_key.dispatch(())>"Save"</button>
                                                            <button class="btn btn-small" on:click=move |_| ctx.kv_edit_key.set(None)>"Cancel"</button>
                                                        </Show>
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()
                                }}
                            </div>
                        </Show>
                    </div>
                </div>
            </div>
        </Show>
    }
}
