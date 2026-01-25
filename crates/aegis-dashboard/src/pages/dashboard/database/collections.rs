//! Collections browser modal component

use leptos::*;
use crate::api;
use super::super::shared::types::*;
use super::super::state::use_dashboard_context;

/// Collections browser modal with CRUD operations
#[component]
pub fn CollectionsModal() -> impl IntoView {
    let ctx = use_dashboard_context();

    let close_modal = move |_| {
        ctx.active_modal.set(BrowserModal::None);
        ctx.selected_collection.set(None);
        ctx.col_show_new_collection.set(false);
        ctx.col_show_new_doc.set(false);
        ctx.col_message.set(None);
    };

    // Load collections
    let load_collections = create_action(move |_: &()| async move {
        ctx.modal_loading.set(true);
        if let Ok(cols) = api::list_collections().await {
            ctx.collections.set(cols);
        }
        ctx.modal_loading.set(false);
    });

    // Load documents for selected collection
    let load_documents = create_action(move |name: &String| {
        let name = name.clone();
        async move {
            if let Ok(docs) = api::get_collection_documents(&name).await {
                ctx.collection_docs.set(docs);
            }
        }
    });

    // Create collection
    let create_collection = create_action(move |_: &()| async move {
        let name = ctx.col_new_name.get();
        if name.is_empty() {
            ctx.col_message.set(Some(("Collection name required".to_string(), false)));
            return;
        }
        match api::create_collection(&name).await {
            Ok(_) => {
                ctx.col_message.set(Some(("Collection created".to_string(), true)));
                ctx.col_new_name.set(String::new());
                ctx.col_show_new_collection.set(false);
                load_collections.dispatch(());
            }
            Err(e) => ctx.col_message.set(Some((e, false))),
        }
    });

    // Insert document
    let insert_document = create_action(move |_: &()| async move {
        if let Some(collection) = ctx.selected_collection.get() {
            let content = ctx.col_new_doc_content.get();
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(doc) => {
                    match api::insert_document(&collection, doc).await {
                        Ok(_) => {
                            ctx.col_message.set(Some(("Document inserted".to_string(), true)));
                            ctx.col_new_doc_content.set(String::new());
                            ctx.col_show_new_doc.set(false);
                            load_documents.dispatch(collection);
                        }
                        Err(e) => ctx.col_message.set(Some((e, false))),
                    }
                }
                Err(e) => ctx.col_message.set(Some((format!("Invalid JSON: {}", e), false))),
            }
        }
    });

    // Delete document
    let delete_document = create_action(move |(collection, doc_id): &(String, String)| {
        let collection = collection.clone();
        let doc_id = doc_id.clone();
        async move {
            match api::delete_document(&collection, &doc_id).await {
                Ok(_) => {
                    ctx.col_message.set(Some(("Document deleted".to_string(), true)));
                    load_documents.dispatch(collection);
                }
                Err(e) => ctx.col_message.set(Some((e, false))),
            }
        }
    });

    // Load on open
    create_effect(move |_| {
        if ctx.active_modal.get() == BrowserModal::Collections {
            load_collections.dispatch(());
        }
    });

    view! {
        <Show when=move || ctx.active_modal.get() == BrowserModal::Collections>
            <div class="modal-overlay" on:click=close_modal>
                <div class="modal-content collections-modal" on:click=|e| e.stop_propagation()>
                    <div class="modal-header">
                        <h2>"📄 Document Collections"</h2>
                        <button class="modal-close" on:click=close_modal>"×"</button>
                    </div>
                    <div class="modal-body">
                        // Message
                        {move || ctx.col_message.get().map(|(msg, is_success)| view! {
                            <div class=if is_success { "message success" } else { "message error" }>{msg}</div>
                        })}

                        <div class="collections-layout">
                            // Collections sidebar
                            <div class="collections-sidebar">
                                <div class="sidebar-header">
                                    <h3>"Collections"</h3>
                                    <button class="btn btn-small" on:click=move |_| ctx.col_show_new_collection.set(true)>"+"</button>
                                </div>

                                // New collection form
                                <Show when=move || ctx.col_show_new_collection.get()>
                                    <div class="new-collection-form">
                                        <input
                                            type="text"
                                            placeholder="Collection name"
                                            prop:value=move || ctx.col_new_name.get()
                                            on:input=move |e| ctx.col_new_name.set(event_target_value(&e))
                                        />
                                        <div class="form-actions">
                                            <button class="btn btn-small" on:click=move |_| ctx.col_show_new_collection.set(false)>"Cancel"</button>
                                            <button class="btn btn-small btn-primary" on:click=move |_| create_collection.dispatch(())>"Create"</button>
                                        </div>
                                    </div>
                                </Show>

                                // Collections list
                                <div class="collections-list">
                                    {move || ctx.collections.get().into_iter().map(|col| {
                                        let name = col.name.clone();
                                        let name_for_click = col.name.clone();
                                        let is_selected = move || ctx.selected_collection.get().as_ref() == Some(&name);

                                        view! {
                                            <div
                                                class=move || if is_selected() { "collection-item selected" } else { "collection-item" }
                                                on:click=move |_| {
                                                    ctx.selected_collection.set(Some(name_for_click.clone()));
                                                    load_documents.dispatch(name_for_click.clone());
                                                }
                                            >
                                                <span class="collection-name">{col.name}</span>
                                                <span class="collection-count">{col.document_count}</span>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>

                            // Documents panel
                            <div class="documents-panel">
                                <Show
                                    when=move || ctx.selected_collection.get().is_some()
                                    fallback=|| view! { <div class="empty-state">"Select a collection"</div> }
                                >
                                    <div class="documents-header">
                                        <h3>{move || ctx.selected_collection.get().unwrap_or_default()}</h3>
                                        <button class="btn btn-primary" on:click=move |_| ctx.col_show_new_doc.set(true)>"+ Add Document"</button>
                                    </div>

                                    // New document form
                                    <Show when=move || ctx.col_show_new_doc.get()>
                                        <div class="new-doc-form">
                                            <textarea
                                                placeholder=r#"{"field": "value"}"#
                                                prop:value=move || ctx.col_new_doc_content.get()
                                                on:input=move |e| ctx.col_new_doc_content.set(event_target_value(&e))
                                            ></textarea>
                                            <div class="form-actions">
                                                <button class="btn btn-secondary" on:click=move |_| ctx.col_show_new_doc.set(false)>"Cancel"</button>
                                                <button class="btn btn-primary" on:click=move |_| insert_document.dispatch(())>"Insert"</button>
                                            </div>
                                        </div>
                                    </Show>

                                    // Documents list
                                    <div class="documents-list">
                                        {move || ctx.collection_docs.get().into_iter().map(|doc| {
                                            let doc_id = doc.id.clone();
                                            let doc_id_select = doc.id.clone();
                                            let doc_id_class = doc.id.clone();
                                            let doc_id_content = doc.id.clone();
                                            let collection = ctx.selected_collection.get().unwrap_or_default();
                                            let doc_json = serde_json::to_string_pretty(&doc.data).unwrap_or_default();
                                            let doc_json_full = doc_json.clone();
                                            let doc_preview = if doc_json.len() > 100 {
                                                format!("{}...", doc_json.chars().take(100).collect::<String>())
                                            } else {
                                                doc_json.clone()
                                            };

                                            view! {
                                                <div
                                                    class=move || if ctx.col_selected_doc.get().as_ref() == Some(&doc_id_class) { "document-item selected" } else { "document-item" }
                                                    on:click={
                                                        let id = doc_id_select.clone();
                                                        move |_| {
                                                            if ctx.col_selected_doc.get().as_ref() == Some(&id) {
                                                                ctx.col_selected_doc.set(None);
                                                            } else {
                                                                ctx.col_selected_doc.set(Some(id.clone()));
                                                            }
                                                        }
                                                    }
                                                >
                                                    <div class="doc-header">
                                                        <span class="doc-id">{doc.id.clone()}</span>
                                                        <button
                                                            class="btn btn-small btn-danger"
                                                            on:click={
                                                                let c = collection.clone();
                                                                let d = doc_id.clone();
                                                                move |e| {
                                                                    e.stop_propagation();
                                                                    delete_document.dispatch((c.clone(), d.clone()));
                                                                }
                                                            }
                                                        >"Delete"</button>
                                                    </div>
                                                    {move || {
                                                        let is_selected = ctx.col_selected_doc.get().as_ref() == Some(&doc_id_content);
                                                        if is_selected {
                                                            view! { <pre class="doc-content expanded">{doc_json_full.clone()}</pre> }.into_view()
                                                        } else {
                                                            view! { <pre class="doc-content collapsed">{doc_preview.clone()}</pre> }.into_view()
                                                        }
                                                    }}
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Show>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </Show>
    }
}
