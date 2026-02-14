use serde_json::json;

use crate::{models::CurrentUser, AppState};

pub fn render_shell(
    state: &AppState,
    initial_path: &str,
    user: Option<&CurrentUser>,
    open_funds_modal: bool,
) -> Result<String, String> {
    let (is_logged_in, user_json) = match user {
        Some(u) => (
            true,
            json!({
                "id": u.id.to_hex(),
                "email": u.email,
                "username": u.username,
            }),
        ),
        None => (false, serde_json::Value::Null),
    };

    let ctx = json!({
        "body": "",
        "is_logged_in": is_logged_in,
        "user": user_json,
        "initial_path": initial_path,
        "open_funds_modal": open_funds_modal,
    });

    state
        .hbs
        .render("layouts/base", &ctx)
        .map_err(|e| e.to_string())
}

pub fn render_full(
    state: &AppState,
    title: &str,
    body_html: String,
    user: Option<&CurrentUser>,
) -> Result<String, String> {
    let (is_logged_in, user_json) = match user {
        Some(u) => (
            true,
            json!({
                "id": u.id.to_hex(),
                "email": u.email,
                "username": u.username,
            }),
        ),
        None => (false, serde_json::Value::Null),
    };

    let ctx = json!({
        "title": title,
        "body": body_html,
        "is_logged_in": is_logged_in,
        "user": user_json,
    });

    state
        .hbs
        .render("layouts/base", &ctx)
        .map_err(|e| e.to_string())
}
