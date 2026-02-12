use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use serde_json::json;

use crate::{models::CurrentUser, render, services::stocks_service, AppState};

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub async fn get_search(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> axum::response::Response {
    let body = match state.hbs.render("pages/search", &json!({})) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("template error: {e}")),
            )
                .into_response()
        }
    };

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(body)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);
    match render::render_full(&state, "Search", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn get_search_results(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> axum::response::Response {
    let q = query.q.unwrap_or_default().trim().to_string();

    let data = stocks_service::search_results_ctx(&state, &q).await;

    let html = state
        .hbs
        .render("partials/search_results", &data)
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(html)).into_response()
}

pub async fn get_details(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
) -> axum::response::Response {
    let body = match state
        .hbs
        .render("pages/details", &json!({ "symbol": symbol }))
    {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("template error: {e}")),
            )
                .into_response()
        }
    };

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(body)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);
    match render::render_full(&state, "Details", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn get_details_quote(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> axum::response::Response {
    let data = stocks_service::quote_ctx(&state, &symbol).await;

    let html = state
        .hbs
        .render("partials/quote", &data)
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(html)).into_response()
}
