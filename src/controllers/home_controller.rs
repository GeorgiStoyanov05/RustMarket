use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
};
use mongodb::bson::doc;
use serde_json::json;

use crate::{models::CurrentUser, render, AppState};

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub async fn home(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let body = state.hbs.render("pages/home", &json!({})).unwrap();

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(body)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);

    match render::render_full(&state, "GoMarket", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn not_found(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let body = state.hbs.render("pages/not_found", &json!({})).unwrap();

    if is_htmx(&headers) {
        return (StatusCode::NOT_FOUND, Html(body)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);

    match render::render_full(&state, "404", body, user_ref) {
        Ok(page) => (StatusCode::NOT_FOUND, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
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
