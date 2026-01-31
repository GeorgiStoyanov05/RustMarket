use crate::models::User;
use axum::extract::Extension;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
};
use mongodb::bson::doc;
use serde_json::json;

use crate::AppState;

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn render_full(
    state: &AppState,
    title: &str,
    body_html: String,
    user: Option<&User>,
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

pub async fn home(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<User>>,
) -> impl IntoResponse {
    // Render the fragment (body)
    let body = match state.hbs.render("pages/home", &json!({})) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("template error: {}", e)),
            )
                .into_response();
        }
    };

    // If HTMX request -> return fragment only
    if is_htmx(&headers) {
        return (StatusCode::OK, Html(body)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);

    // Normal request -> wrap in layout
    match render_full(&state, "GoMarket", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("render error: {}", e)),
        )
            .into_response(),
    }
}

pub async fn not_found(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<User>>,
) -> impl IntoResponse {
    let body = match state.hbs.render("pages/not_found", &json!({})) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("template error: {}", e)),
            )
                .into_response();
        }
    };

    if is_htmx(&headers) {
        return (StatusCode::NOT_FOUND, Html(body)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);
    match render_full(&state, "404", body, user_ref) {
        Ok(page) => (StatusCode::NOT_FOUND, Html(page)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("render error: {}", e)),
        )
            .into_response(),
    }
}

pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, Html("ok".to_string()))
}

pub async fn health_db(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.run_command(doc! { "ping": 1 }, None).await {
        Ok(_) => (StatusCode::OK, Html("mongo: ok".to_string())).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("mongo error: {}", e)),
        )
            .into_response(),
    }
}
