use axum::{
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde_json::json;

use crate::{
    models::CurrentUser,
    render,
    services::portfolio_service,
    AppState,
};

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn fmt2(v: f64) -> String {
    format!("{:.2}", v)
}

// GET /portfolio (SSR page)
pub async fn get_portfolio_page(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let body = match state.hbs.render("pages/portfolio", &json!({})) {
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

    match render::render_full(&state, "Portfolio", body, None) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

// GET /portfolio/positions (HTMX partial)
pub async fn get_portfolio_positions(
    State(state): State<AppState>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        let html = state
            .hbs
            .render(
                "partials/portfolio_positions",
                &json!({ "groups": [] }),
            )
            .unwrap_or_else(|e| format!("template error: {e}"));
        return (StatusCode::OK, Html(html)).into_response();
    };

    let views = match portfolio_service::list_portfolio_position_views(&state, u.id).await {
        Ok(v) => v,
        Err(_) => vec![],
    };

    let groups: Vec<serde_json::Value> = views
        .into_iter()
        .map(|v| {
            json!({
                "symbol": v.symbol,
                "qty": v.qty,
                "avg": fmt2(v.avg_price),
                "avg_raw": v.avg_price,
                "current_price": fmt2(v.last_price),
                "pnl": fmt2(v.pnl),
                "pnl_pct": fmt2(v.pnl_pct),
                "pnl_class": v.pnl_class,
            })
        })
        .collect();

    let html = state
        .hbs
        .render("partials/portfolio_positions", &json!({ "groups": groups }))
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(html)).into_response()
}

// GET /portfolio/position/:symbol (HTMX partial)
pub async fn get_portfolio_position_card(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::UNAUTHORIZED, Html("Unauthorized".to_string())).into_response();
    };

    let view_opt = match portfolio_service::get_portfolio_position_view(&state, u.id, &symbol).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            )
                .into_response();
        }
    };

    let Some(view) = view_opt else {
        return (StatusCode::NOT_FOUND, Html("Not found".to_string())).into_response();
    };

    let html = state
        .hbs
        .render(
            "partials/portfolio_position_card",
            &json!({
                "symbol": view.symbol,
                "qty": view.qty,
                "avg": fmt2(view.avg_price),
                "current_price": fmt2(view.last_price),
                "pnl": fmt2(view.pnl),
                "pnl_pct": fmt2(view.pnl_pct),
                "pnl_class": view.pnl_class,
            }),
        )
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(html)).into_response()
}

// GET /portfolio/orders (HTMX partial)
pub async fn get_portfolio_orders(
    State(state): State<AppState>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        let html = state
            .hbs
            .render("partials/orders_list", &json!({ "items": [] }))
            .unwrap_or_else(|e| format!("template error: {e}"));
        return (StatusCode::OK, Html(html)).into_response();
    };

    let views = portfolio_service::list_recent_order_views(&state, u.id, 50)
        .await
        .unwrap_or_default();

    let items: Vec<serde_json::Value> = views
        .into_iter()
        .map(|o| {
            json!({
                "created_at": o.created_at,
                "symbol": o.symbol,
                "side": o.side,
                "qty": o.qty,
                "price": fmt2(o.price),
                "total": fmt2(o.total),
            })
        })
        .collect();

    let html = state
        .hbs
        .render("partials/orders_list", &json!({ "items": items }))
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(html)).into_response()
}
