//! Users and roles management component

use leptos::*;
use super::super::state::use_dashboard_context;

/// Users and roles settings tab
#[component]
pub fn UsersSettings() -> impl IntoView {
    let ctx = use_dashboard_context();

    // Add user action
    let add_user = move |_| {
        let name = ctx.new_user_name.get();
        let email = ctx.new_user_email.get();
        let role = ctx.new_user_role.get();
        let has_2fa = ctx.new_user_2fa.get();

        if name.is_empty() || email.is_empty() {
            ctx.settings_message.set(Some(("Name and email are required".to_string(), false)));
            return;
        }

        // Generate a simple unique ID using timestamp
        let id = format!("user-{}", js_sys::Date::now() as u64);

        ctx.users_list.update(|users| {
            users.push((id, name.clone(), email.clone(), role.clone(), has_2fa));
        });

        ctx.new_user_name.set(String::new());
        ctx.new_user_email.set(String::new());
        ctx.new_user_role.set("viewer".to_string());
        ctx.new_user_2fa.set(false);
        ctx.show_add_user.set(false);
        ctx.settings_message.set(Some(("User added successfully".to_string(), true)));
    };

    // Delete user action
    let delete_user = move |user_id: String| {
        ctx.users_list.update(|users| {
            users.retain(|(id, _, _, _, _)| id != &user_id);
        });
        ctx.settings_message.set(Some(("User deleted".to_string(), true)));
    };

    // Edit user action
    let start_edit = move |(id, name, email, role, has_2fa): (String, String, String, String, bool)| {
        ctx.edit_user_id.set(Some(id));
        ctx.edit_user_name.set(name);
        ctx.edit_user_email.set(email);
        ctx.edit_user_role.set(role);
        ctx.edit_user_2fa.set(has_2fa);
    };

    let save_edit = move |_| {
        if let Some(edit_id) = ctx.edit_user_id.get() {
            let name = ctx.edit_user_name.get();
            let email = ctx.edit_user_email.get();
            let role = ctx.edit_user_role.get();
            let has_2fa = ctx.edit_user_2fa.get();

            ctx.users_list.update(|users| {
                if let Some(user) = users.iter_mut().find(|(id, _, _, _, _)| id == &edit_id) {
                    *user = (edit_id.clone(), name, email, role, has_2fa);
                }
            });

            ctx.edit_user_id.set(None);
            ctx.settings_message.set(Some(("User updated".to_string(), true)));
        }
    };

    // Add role action
    let add_role = move |_| {
        let name = ctx.new_role_name.get();
        if name.is_empty() {
            ctx.settings_message.set(Some(("Role name is required".to_string(), false)));
            return;
        }

        ctx.roles_list.update(|roles| {
            roles.push((name.clone(), "Custom role".to_string(), vec![]));
        });

        ctx.new_role_name.set(String::new());
        ctx.show_add_role.set(false);
        ctx.settings_message.set(Some(("Role added".to_string(), true)));
    };

    view! {
        <div class="settings-section">
            // Users section
            <div class="section-header">
                <h3>"Users"</h3>
                <button
                    class="btn btn-primary"
                    on:click=move |_| ctx.show_add_user.set(true)
                >"+ Add User"</button>
            </div>

            // Add user form
            <Show when=move || ctx.show_add_user.get()>
                <div class="add-form">
                    <h4>"Add New User"</h4>
                    <div class="form-grid">
                        <input
                            type="text"
                            placeholder="Name"
                            prop:value=move || ctx.new_user_name.get()
                            on:input=move |e| ctx.new_user_name.set(event_target_value(&e))
                        />
                        <input
                            type="email"
                            placeholder="Email"
                            prop:value=move || ctx.new_user_email.get()
                            on:input=move |e| ctx.new_user_email.set(event_target_value(&e))
                        />
                        <select
                            prop:value=move || ctx.new_user_role.get()
                            on:change=move |e| ctx.new_user_role.set(event_target_value(&e))
                        >
                            {move || ctx.roles_list.get().into_iter().map(|(name, _, _)| {
                                view! { <option value=name.clone()>{name}</option> }
                            }).collect_view()}
                        </select>
                        <label class="toggle-label">
                            <input
                                type="checkbox"
                                prop:checked=move || ctx.new_user_2fa.get()
                                on:change=move |e| ctx.new_user_2fa.set(event_target_checked(&e))
                            />
                            <span>"Require 2FA"</span>
                        </label>
                    </div>
                    <div class="form-actions">
                        <button class="btn btn-secondary" on:click=move |_| ctx.show_add_user.set(false)>"Cancel"</button>
                        <button class="btn btn-primary" on:click=add_user>"Add User"</button>
                    </div>
                </div>
            </Show>

            // Users table
            <table class="data-table">
                <thead>
                    <tr>
                        <th>"Name"</th>
                        <th>"Email"</th>
                        <th>"Role"</th>
                        <th>"2FA"</th>
                        <th>"Actions"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || ctx.users_list.get().into_iter().map(|(id, name, email, role, has_2fa)| {
                        let id_for_edit = id.clone();
                        let id_for_delete = id.clone();
                        let edit_data = (id.clone(), name.clone(), email.clone(), role.clone(), has_2fa);
                        let is_editing = move || ctx.edit_user_id.get().as_ref() == Some(&id_for_edit);

                        view! {
                            <tr>
                                <Show
                                    when=is_editing
                                    fallback=move || view! {
                                        <td>{name.clone()}</td>
                                        <td>{email.clone()}</td>
                                        <td><span class="role-badge">{role.clone()}</span></td>
                                        <td>{if has_2fa { "✓" } else { "✗" }}</td>
                                        <td class="actions-cell">
                                            <button
                                                class="btn btn-small"
                                                on:click={
                                                    let data = edit_data.clone();
                                                    move |_| start_edit(data.clone())
                                                }
                                            >"Edit"</button>
                                            <button
                                                class="btn btn-small btn-danger"
                                                on:click={
                                                    let id = id_for_delete.clone();
                                                    move |_| delete_user(id.clone())
                                                }
                                            >"Delete"</button>
                                        </td>
                                    }
                                >
                                    <td>
                                        <input
                                            type="text"
                                            prop:value=move || ctx.edit_user_name.get()
                                            on:input=move |e| ctx.edit_user_name.set(event_target_value(&e))
                                        />
                                    </td>
                                    <td>
                                        <input
                                            type="email"
                                            prop:value=move || ctx.edit_user_email.get()
                                            on:input=move |e| ctx.edit_user_email.set(event_target_value(&e))
                                        />
                                    </td>
                                    <td>
                                        <select
                                            prop:value=move || ctx.edit_user_role.get()
                                            on:change=move |e| ctx.edit_user_role.set(event_target_value(&e))
                                        >
                                            {move || ctx.roles_list.get().into_iter().map(|(name, _, _)| {
                                                view! { <option value=name.clone()>{name}</option> }
                                            }).collect_view()}
                                        </select>
                                    </td>
                                    <td>
                                        <input
                                            type="checkbox"
                                            prop:checked=move || ctx.edit_user_2fa.get()
                                            on:change=move |e| ctx.edit_user_2fa.set(event_target_checked(&e))
                                        />
                                    </td>
                                    <td class="actions-cell">
                                        <button class="btn btn-small btn-primary" on:click=save_edit>"Save"</button>
                                        <button class="btn btn-small" on:click=move |_| ctx.edit_user_id.set(None)>"Cancel"</button>
                                    </td>
                                </Show>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>

            // Roles section
            <div class="section-header">
                <h3>"Roles"</h3>
                <button
                    class="btn btn-primary"
                    on:click=move |_| ctx.show_add_role.set(true)
                >"+ Add Role"</button>
            </div>

            // Add role form
            <Show when=move || ctx.show_add_role.get()>
                <div class="add-form">
                    <h4>"Add New Role"</h4>
                    <input
                        type="text"
                        placeholder="Role name"
                        prop:value=move || ctx.new_role_name.get()
                        on:input=move |e| ctx.new_role_name.set(event_target_value(&e))
                    />
                    <div class="form-actions">
                        <button class="btn btn-secondary" on:click=move |_| ctx.show_add_role.set(false)>"Cancel"</button>
                        <button class="btn btn-primary" on:click=add_role>"Add Role"</button>
                    </div>
                </div>
            </Show>

            // Roles list
            <div class="roles-list">
                {move || ctx.roles_list.get().into_iter().map(|(name, desc, perms)| {
                    view! {
                        <div class="role-card">
                            <div class="role-header">
                                <h4>{name}</h4>
                            </div>
                            <p class="role-desc">{desc}</p>
                            <div class="role-perms">
                                {perms.into_iter().map(|p| view! {
                                    <span class="perm-badge">{p}</span>
                                }).collect_view()}
                            </div>
                        </div>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}
