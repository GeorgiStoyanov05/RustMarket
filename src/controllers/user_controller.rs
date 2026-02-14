use axum::{
    Form,
    extract::{Extension, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;

use crate::{
    AppState,
    models::CurrentUser,
    render,
    services::{account_service, user_service},
};

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn render_page(state: &AppState, tpl: &str, ctx: serde_json::Value) -> String {
    state
        .hbs
        .render(tpl, &ctx)
        .unwrap_or_else(|e| format!("template error: {e}"))
}

fn fmt2(x: f64) -> String {
    format!("{:.2}", x)
}

pub async fn me(user: Option<Extension<CurrentUser>>) -> impl IntoResponse {
    match user {
        Some(Extension(u)) => (StatusCode::OK, axum::Json(u)).into_response(),
        None => (StatusCode::UNAUTHORIZED, Html("not logged in".to_string())).into_response(),
    }
}

// ---------------- Settings ----------------

pub async fn get_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let body = match state.hbs.render("pages/settings", &json!({})) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("template error: {e}")),
            )
                .into_response();
        }
    };

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(body)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);

    match render::render_full(&state, "Settings", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

#[derive(Deserialize)]
pub struct ChangeEmailForm {
    pub email: String,
}

pub async fn get_settings_email(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let current_email = user
        .as_ref()
        .map(|Extension(u)| u.email.as_str())
        .unwrap_or("");

    let partial = state
        .hbs
        .render(
            "partials/change_email",
            &json!({
                "values": { "email": current_email },
                "errors": {},
                "succ": ""
            }),
        )
        .unwrap_or_else(|e| format!("template error: {e}"));

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(partial)).into_response();
    }

    let shell = state
        .hbs
        .render("pages/settings", &json!({}))
        .unwrap_or_else(|e| format!("template error: {e}"));

    let autoload = r##"<div hx-get=\"/settings/email\" hx-trigger=\"load\" hx-target=\"#rightPane\" hx-swap=\"innerHTML\"></div>"##;
    let body = format!("{}{}", shell, autoload);

    let user_ref = user.as_ref().map(|Extension(u)| u);

    match render::render_full(&state, "Settings", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn post_settings_email(
    State(state): State<AppState>,
    _headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<ChangeEmailForm>,
) -> Response {
    let new_email = form.email.trim().to_string();
    let mut errors = serde_json::Map::new();

    let Some(Extension(u)) = user else {
        errors.insert("_form".into(), json!("There was an error getting user"));

        let partial = state
            .hbs
            .render(
                "partials/change_email",
                &json!({
                    "values": { "email": new_email },
                    "errors": errors,
                    "succ": ""
                }),
            )
            .unwrap_or_else(|e| format!("template error: {e}"));

        return (StatusCode::OK, Html(partial)).into_response();
    };

    // validate email
    if new_email.is_empty() {
        errors.insert("email".into(), json!("Email is required."));
    } else {
        let re = Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap();
        if !re.is_match(&new_email) {
            errors.insert("email".into(), json!("Please enter a valid email address."));
        }
    }

    // must differ from current email
    if errors.is_empty() && new_email.eq_ignore_ascii_case(&u.email) {
        errors.insert(
            "email".into(),
            json!("New email must be different from your current email."),
        );
    }

    if errors.is_empty() {
        match user_service::change_email(&state, u.id, &new_email).await {
            Ok(()) => {}
            Err(errs) => {
                for (k, v) in errs {
                    errors.insert(k, json!(v));
                }
            }
        }
    }

    let succ = if errors.is_empty() {
        "You have changed your email successfully!"
    } else {
        ""
    };

    let partial = state
        .hbs
        .render(
            "partials/change_email",
            &json!({
                "values": { "email": if succ.is_empty() { new_email } else { String::new() } },
                "errors": errors,
                "succ": succ
            }),
        )
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(partial)).into_response()
}

#[derive(Deserialize)]
pub struct ChangePasswordForm {
    pub password: String,

    #[serde(default, rename = "rePassword")]
    pub re_password: Option<String>,
}

pub async fn get_settings_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let partial = state
        .hbs
        .render(
            "partials/change_password",
            &json!({
                "errors": {},
                "succ": ""
            }),
        )
        .unwrap_or_else(|e| format!("template error: {e}"));

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(partial)).into_response();
    }

    let shell = state
        .hbs
        .render("pages/settings", &json!({}))
        .unwrap_or_else(|e| format!("template error: {e}"));

    let autoload = r##"<div hx-get=\"/settings/password\" hx-trigger=\"load\" hx-target=\"#rightPane\" hx-swap=\"innerHTML\"></div>"##;
    let body = format!("{}{}", shell, autoload);

    let user_ref = user.as_ref().map(|Extension(u)| u);

    match render::render_full(&state, "Settings", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn post_settings_password(
    State(state): State<AppState>,
    _headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<ChangePasswordForm>,
) -> Response {
    let mut errors = serde_json::Map::new();

    let Some(Extension(u)) = user else {
        errors.insert("_form".into(), json!("There was an error getting user"));

        let partial = state
            .hbs
            .render(
                "partials/change_password",
                &json!({
                    "errors": errors,
                    "succ": ""
                }),
            )
            .unwrap_or_else(|e| format!("template error: {e}"));

        return (StatusCode::OK, Html(partial)).into_response();
    };

    let password = form.password.trim().to_string();
    let re_password = form.re_password.as_deref().unwrap_or("").trim().to_string();

    if password.is_empty() {
        errors.insert("password".into(), json!("Password is required."));
    }
    if re_password.is_empty() {
        errors.insert("rePassword".into(), json!("Repeat password is required."));
    }
    if errors.is_empty() && password != re_password {
        errors.insert("rePassword".into(), json!("Passwords do not match."));
    }
    if errors.is_empty() && password.len() < 6 {
        errors.insert(
            "password".into(),
            json!("Password must be at least 6 characters."),
        );
    }

    if errors.is_empty() {
        match user_service::change_password(&state, u.id, &password).await {
            Ok(()) => {}
            Err(errs) => {
                for (k, v) in errs {
                    errors.insert(k, json!(v));
                }
            }
        }
    }

    let partial = state
        .hbs
        .render(
            "partials/change_password",
            &json!({
                "errors": errors,
                "succ": if errors.is_empty() { "You have changed your password successfully!" } else { "" }
            }),
        )
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(partial)).into_response()
}

// ---------------- Funds ----------------

pub async fn get_funds_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    if let Some(Extension(u)) = user.as_ref() {
        let _ = account_service::get_or_create_account(&state, u.id).await;
    }

    if is_htmx(&headers) {
        let html = render_page(&state, "pages/funds", json!({}));
        return (StatusCode::OK, Html(html)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);
    match render::render_shell(&state, "/", user_ref, true) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

// GET /funds/modal
pub async fn get_funds_modal(
    State(state): State<AppState>,
    user: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    if let Some(Extension(u)) = user {
        let _ = account_service::get_or_create_account(&state, u.id).await;
    }

    let html = render_page(&state, "partials/funds_modal", json!({}));
    (StatusCode::OK, Html(html))
}

// GET /cash
pub async fn get_cash_badge(
    State(state): State<AppState>,
    user: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let Some(Extension(u)) = user else {
        return (StatusCode::OK, Html("".to_string()));
    };

    let acc = match account_service::get_or_create_account(&state, u.id).await {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            );
        }
    };

    let html = render_page(
        &state,
        "partials/cash_badge",
        json!({ "cash": fmt2(acc.cash) }),
    );
    (StatusCode::OK, Html(html))
}

// POST /funds
#[derive(Deserialize)]
pub struct DepositForm {
    pub amount: String,
}

pub async fn post_funds(
    State(state): State<AppState>,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<DepositForm>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (
            StatusCode::OK,
            Html(
                r#"<div class=\"alert alert-danger mb-0\">There was an error getting user</div>"#
                    .to_string(),
            ),
        )
            .into_response();
    };

    let amount_str = form.amount.trim();
    let amount: f64 = match amount_str.parse() {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::OK,
                Html(r#"<div class=\"alert alert-danger mb-0\">There was an error with the amount!</div>"#.to_string()),
            )
                .into_response();
        }
    };

    if !amount.is_finite() || amount <= 0.0 {
        return (
            StatusCode::OK,
            Html(
                r#"<div class=\"alert alert-danger mb-0\">Amount must be bigger than zero!</div>"#
                    .to_string(),
            ),
        )
            .into_response();
    }

    match user_service::deposit_funds(&state, u.id, amount).await {
        Ok(_acc) => {}
        Err(errs) => {
            let msg = errs
                .get("_form")
                .cloned()
                .unwrap_or_else(|| "Deposit failed.".to_string());
            return (
                StatusCode::OK,
                Html(format!(
                    r#"<div class=\"alert alert-danger mb-0\">{}</div>"#,
                    msg
                )),
            )
                .into_response();
        }
    }

    let msg =
        r#"<div class=\"alert alert-success mb-0\">The deposit was successful!</div>"#.to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        HeaderValue::from_static(r#"{\"cashUpdated\":true}"#),
    );

    // also broadcast to other tabs
    let _ = state.events_tx.send("cashUpdated".to_string());

    (StatusCode::OK, headers, Html(msg)).into_response()
}
