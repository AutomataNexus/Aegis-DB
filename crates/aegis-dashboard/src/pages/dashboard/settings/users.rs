//! Users and roles management component

use leptos::*;
use crate::api;
use super::super::state::use_dashboard_context;

/// Users and roles settings tab
#[component]
pub fn UsersSettings() -> impl IntoView {
    let ctx = use_dashboard_context();

    // Load users from API
    let load_users = create_action(move |_: &()| async move {
        match api::list_users().await {
            Ok(users) => {
                let mapped: Vec<(String, String, String, String, bool)> = users
                    .into_iter()
                    .map(|u| (u.id, u.username, u.email, u.role, u.mfa_enabled))
                    .collect();
                ctx.users_list.set(mapped);
            }
            Err(e) => {
                ctx.settings_message.set(Some((format!("Failed to load users: {}", e), false)));
            }
        }
    });

    // Load roles from API
    let load_roles = create_action(move |_: &()| async move {
        match api::list_roles().await {
            Ok(roles) => {
                let mapped: Vec<(String, String, Vec<String>)> = roles
                    .into_iter()
                    .map(|r| (r.name, r.description, r.permissions))
                    .collect();
                ctx.roles_list.set(mapped);
            }
            Err(e) => {
                ctx.settings_message.set(Some((format!("Failed to load roles: {}", e), false)));
            }
        }
    });

    // Load data on mount
    create_effect(move |_| {
        load_users.dispatch(());
        load_roles.dispatch(());
    });

    // Add user action - calls create_user API
    let add_user = create_action(move |_: &()| {
        let name = ctx.new_user_name.get();
        let email = ctx.new_user_email.get();
        let password = ctx.new_user_password.get();
        let role = ctx.new_user_role.get();

        async move {
            if name.is_empty() || email.is_empty() {
                ctx.settings_message.set(Some(("Name and email are required".to_string(), false)));
                return;
            }
            if password.is_empty() {
                ctx.settings_message.set(Some(("Password is required".to_string(), false)));
                return;
            }

            match api::create_user(&name, &email, &password, &role).await {
                Ok(_) => {
                    ctx.new_user_name.set(String::new());
                    ctx.new_user_email.set(String::new());
                    ctx.new_user_password.set(String::new());
                    ctx.new_user_role.set("viewer".to_string());
                    ctx.new_user_2fa.set(false);
                    ctx.show_add_user.set(false);
                    ctx.settings_message.set(Some(("User added successfully".to_string(), true)));
                    load_users.dispatch(());
                }
                Err(e) => {
                    ctx.settings_message.set(Some((format!("Failed to add user: {}", e), false)));
                }
            }
        }
    });

    // Delete user action - calls delete_user API
    let delete_user = create_action(move |username: &String| {
        let username = username.clone();
        async move {
            match api::delete_user(&username).await {
                Ok(_) => {
                    ctx.settings_message.set(Some(("User deleted".to_string(), true)));
                    load_users.dispatch(());
                }
                Err(e) => {
                    ctx.settings_message.set(Some((format!("Failed to delete user: {}", e), false)));
                }
            }
        }
    });

    // Edit user action
    let start_edit = move |(id, name, email, role, has_2fa): (String, String, String, String, bool)| {
        ctx.edit_user_id.set(Some(id));
        ctx.edit_user_name.set(name);
        ctx.edit_user_email.set(email);
        ctx.edit_user_role.set(role);
        ctx.edit_user_2fa.set(has_2fa);
    };

    // Save edit action - calls update_user API
    let save_edit = create_action(move |_: &()| {
        let edit_id = ctx.edit_user_id.get();
        let name = ctx.edit_user_name.get();
        let email = ctx.edit_user_email.get();
        let role = ctx.edit_user_role.get();

        async move {
            if let Some(_edit_id) = edit_id {
                let updates = api::UserUpdate {
                    email: Some(email),
                    role: Some(role),
                    enabled: None,
                    password: None,
                };

                match api::update_user(&name, &updates).await {
                    Ok(_) => {
                        ctx.edit_user_id.set(None);
                        ctx.settings_message.set(Some(("User updated".to_string(), true)));
                        load_users.dispatch(());
                    }
                    Err(e) => {
                        ctx.settings_message.set(Some((format!("Failed to update user: {}", e), false)));
                    }
                }
            }
        }
    });

    // Add role action - calls create_role API
    let add_role = create_action(move |_: &()| {
        let name = ctx.new_role_name.get();
        let description = ctx.new_role_description.get();

        async move {
            if name.is_empty() {
                ctx.settings_message.set(Some(("Role name is required".to_string(), false)));
                return;
            }

            match api::create_role(&name, &description, &[]).await {
                Ok(_) => {
                    ctx.new_role_name.set(String::new());
                    ctx.new_role_description.set(String::new());
                    ctx.show_add_role.set(false);
                    ctx.settings_message.set(Some(("Role added".to_string(), true)));
                    load_roles.dispatch(());
                }
                Err(e) => {
                    ctx.settings_message.set(Some((format!("Failed to add role: {}", e), false)));
                }
            }
        }
    });

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
                            placeholder="Username"
                            prop:value=move || ctx.new_user_name.get()
                            on:input=move |e| ctx.new_user_name.set(event_target_value(&e))
                        />
                        <input
                            type="email"
                            placeholder="Email"
                            prop:value=move || ctx.new_user_email.get()
                            on:input=move |e| ctx.new_user_email.set(event_target_value(&e))
                        />
                        <input
                            type="password"
                            placeholder="Password"
                            prop:value=move || ctx.new_user_password.get()
                            on:input=move |e| ctx.new_user_password.set(event_target_value(&e))
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
                        <button class="btn btn-primary" on:click=move |_| add_user.dispatch(())>"Add User"</button>
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
                        let name_for_delete = name.clone();
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
                                                    let username = name_for_delete.clone();
                                                    move |_| delete_user.dispatch(username.clone())
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
                                        <button class="btn btn-small btn-primary" on:click=move |_| save_edit.dispatch(())>"Save"</button>
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
                    <input
                        type="text"
                        placeholder="Description"
                        prop:value=move || ctx.new_role_description.get()
                        on:input=move |e| ctx.new_role_description.set(event_target_value(&e))
                    />
                    <div class="form-actions">
                        <button class="btn btn-secondary" on:click=move |_| ctx.show_add_role.set(false)>"Cancel"</button>
                        <button class="btn btn-primary" on:click=move |_| add_role.dispatch(())>"Add Role"</button>
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
