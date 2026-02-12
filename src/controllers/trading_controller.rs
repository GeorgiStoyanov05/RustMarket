use axum::{
    extract::{Extension, Form, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    models::CurrentUser,
    services::{portfolio_service, trading_service},
    AppState,
};

fn hx_trigger_value(events: &[&str]) -> HeaderValue {
    // HX-Trigger expects JSON: {"evt":true,...}
    let mut s = String::from("{");
    for (i, ev) in events.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('"');
        s.push_str(ev);
        s.push_str("\":true");
    }
    s.push('}');
    HeaderValue::from_str(&s).unwrap_or_else(|_| HeaderValue::from_static("{}"))
}

fn unauthorized_snippet() -> Response {
    (StatusCode::UNAUTHORIZED, Html(r#"<div class=\"text-danger\">Unauthorized</div>"#.to_string())).into_response()
}

fn fmt2(v: f64) -> String {
    format!("{:.2}", v)
}

// GET /position/:symbol (HTMX partial)
pub async fn get_position_panel(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        let html = state
            .hbs
            .render(
                "partials/position_panel",
                &json!({
                    "has_position": false,
                    "symbol": symbol.to_uppercase(),
                }),
            )
            .unwrap_or_else(|e| format!("template error: {e}"));
        return (StatusCode::OK, Html(html)).into_response();
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
        let html = state
            .hbs
            .render(
                "partials/position_panel",
                &json!({
                    "has_position": false,
                    "symbol": symbol.to_uppercase(),
                }),
            )
            .unwrap_or_else(|e| format!("template error: {e}"));
        return (StatusCode::OK, Html(html)).into_response();
    };

    let html = state
        .hbs
        .render(
            "partials/position_panel",
            &json!({
                "has_position": true,
                "symbol": view.symbol,
                "qty": view.qty,
                "avg_price": fmt2(view.avg_price),
                "avg_price_raw": view.avg_price,
                "last_price": fmt2(view.last_price),
                "pnl": fmt2(view.pnl),
                "pnl_pct": fmt2(view.pnl_pct),
                "pnl_class": view.pnl_class,
            }),
        )
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(html)).into_response()
}

#[derive(Deserialize)]
pub struct TradeForm {
    pub qty: String,
}

// POST /trade/:symbol/buy
pub async fn post_trade_buy(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<TradeForm>,
) -> Response {
    let Some(Extension(u)) = user else {
        return unauthorized_snippet();
    };

    let qty_str = form.qty.trim();
    let qty: i64 = match qty_str.parse() {
        Ok(q) => q,
        Err(_) => {
            return (
                StatusCode::OK,
                Html(r#"<div class=\"text-danger\">Enter a valid quantity.</div>"#.to_string()),
            )
                .into_response();
        }
    };

    let result = match trading_service::market_buy(&state, u.id, &symbol, qty).await {
        Ok(r) => r,
        Err(errs) => {
            if let Some(v) = errs.get("balance") {
                return (StatusCode::OK, Html(format!(r#"<div class=\"text-danger\">{}</div>"#, v))).into_response();
            }
            if let Some(v) = errs.get("qty") {
                return (StatusCode::OK, Html(format!(r#"<div class=\"text-danger\">{}</div>"#, v))).into_response();
            }
            if let Some(v) = errs.get("_form") {
                return (StatusCode::OK, Html(format!(r#"<div class=\"text-danger\">{}</div>"#, v))).into_response();
            }
            return (
                StatusCode::OK,
                Html(r#"<div class=\"text-danger\">Could not buy.</div>"#.to_string()),
            )
                .into_response();
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        hx_trigger_value(&["cashUpdated", "positionUpdated", "ordersUpdated"]),
    );

    (
        StatusCode::OK,
        headers,
        Html(format!(
            r#"<div class=\"text-success\">Bought {} {} @ {} (Cost: {}, New balance: {})</div>"#,
            result.qty,
            result.symbol,
            fmt2(result.fill_price),
            fmt2(result.cost),
            fmt2(result.new_cash)
        )),
    )
        .into_response()
}

// POST /trade/:symbol/sell
pub async fn post_trade_sell(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<TradeForm>,
) -> Response {
    let Some(Extension(u)) = user else {
        return unauthorized_snippet();
    };

    let qty_str = form.qty.trim();
    let qty: i64 = match qty_str.parse() {
        Ok(q) => q,
        Err(_) => {
            return (
                StatusCode::OK,
                Html(r#"<div class=\"text-danger\">Enter a valid quantity.</div>"#.to_string()),
            )
                .into_response();
        }
    };

    let result = match trading_service::market_sell(&state, u.id, &symbol, qty).await {
        Ok(r) => r,
        Err(errs) => {
            if let Some(v) = errs.get("qty") {
                return (StatusCode::OK, Html(format!(r#"<div class=\"text-danger\">{}</div>"#, v))).into_response();
            }
            if let Some(v) = errs.get("_form") {
                return (StatusCode::OK, Html(format!(r#"<div class=\"text-danger\">{}</div>"#, v))).into_response();
            }
            return (
                StatusCode::OK,
                Html(r#"<div class=\"text-danger\">Could not sell.</div>"#.to_string()),
            )
                .into_response();
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        hx_trigger_value(&["cashUpdated", "positionUpdated", "ordersUpdated"]),
    );

    (
        StatusCode::OK,
        headers,
        Html(format!(
            r#"<div class=\"text-success\">Sold {} {} @ {} (Proceeds: {}, New balance: {})</div>"#,
            result.qty,
            result.symbol,
            fmt2(result.fill_price),
            fmt2(result.proceeds),
            fmt2(result.new_cash)
        )),
    )
        .into_response()
}
