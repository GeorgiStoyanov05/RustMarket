use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Form,
};
use chrono::Utc;
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::options::FindOptions;
use serde::Deserialize;
use serde_json::json;

use crate::{
    models::{Account, Alert, CurrentUser, Position},
    AppState,
};

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn render_page(state: &AppState, tpl: &str, ctx: serde_json::Value) -> String {
    // NOTE: your base layout may wrap differently; adapt if your handlers use a "layouts/base" render helper
    state.hbs.render(tpl, &ctx).unwrap_or_else(|e| format!("template error: {e}"))
}

fn fmt2(x: f64) -> String {
    format!("{:.2}", x)
}

async fn get_or_create_account(state: &AppState, user_id: ObjectId) -> Result<Account, String> {
    let accounts = state.db.collection::<Account>("accounts");
    if let Ok(Some(acc)) = accounts.find_one(doc! { "_id": user_id }, None).await {
        return Ok(acc);
    }
    let acc = Account { id: user_id, cash: 10_000.0, updated_at: Utc::now().timestamp() };
    accounts.insert_one(&acc, None).await.map_err(|e| e.to_string())?;
    Ok(acc)
}

// ---------------- Pages ----------------

pub async fn get_portfolio_page(State(state): State<AppState>) -> impl IntoResponse {
    let html = render_page(&state, "pages/portfolio", json!({}));
    (StatusCode::OK, Html(html))
}

pub async fn get_alerts_page(State(state): State<AppState>) -> impl IntoResponse {
    let html = render_page(&state, "pages/alerts", json!({}));
    (StatusCode::OK, Html(html))
}

pub async fn get_funds_page(State(state): State<AppState>, user: Option<Extension<CurrentUser>>) -> impl IntoResponse {
    // ensure account exists so depositing never fails
    if let Some(Extension(u)) = user {
        let _ = get_or_create_account(&state, u.id).await;
    }
    let html = render_page(&state, "pages/funds", json!({}));
    (StatusCode::OK, Html(html))
}

// ---------------- Partials ----------------

// GET /portfolio/positions
pub async fn get_portfolio_positions(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::OK, Html(r#"<div class="text-muted">Log in to see your portfolio.</div>"#.to_string())).into_response();
    };

    let positions = state.db.collection::<Position>("positions");
    let find_opts = FindOptions::builder().sort(doc! { "updated_at": -1 }).build();

    let mut cursor = match positions.find(doc! { "user_id": u.id }, find_opts).await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
    };

    let mut groups: Vec<serde_json::Value> = vec![];
    let mut i = 0;

    while let Some(res) = cursor.next().await {
        let p = match res {
            Ok(p) => p,
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
        };

        let q = state.finnhub.quote(&p.symbol).await.ok();
        let last = q.map(|x| x.c).unwrap_or(0.0);

        let pnl = (last - p.avg_price) * (p.qty as f64);
        let pct = if p.avg_price > 0.0 { ((last - p.avg_price) / p.avg_price) * 100.0 } else { 0.0 };

        let pnl_class = if pnl > 0.0 { "text-success" } else if pnl < 0.0 { "text-danger" } else { "text-muted" };

        groups.push(json!({
            "key": i,
            "symbol": p.symbol,
            "qty": p.qty,
            "avg": fmt2(p.avg_price),
            "current_price": fmt2(last),
            "pnl": (if pnl>0.0 { "+" } else { "" }).to_string() + &fmt2(pnl),
            "pnl_pct": (if pct>0.0 { "+" } else { "" }).to_string() + &fmt2(pct),
            "pnl_class": pnl_class
        }));

        i += 1;
    }

    let ctx = json!({ "groups": if groups.is_empty() { serde_json::Value::Null } else { serde_json::Value::Array(groups) } });
    let html = state.hbs.render("partials/portfolio_positions", &ctx).unwrap_or_else(|e| format!("template error: {e}"));

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(html)).into_response();
    }
    (StatusCode::OK, Html(html)).into_response()
}

// GET /alerts/list
pub async fn get_watchlist_alerts(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::OK, Html(r#"<div class="text-muted">Log in to see your alerts.</div>"#.to_string())).into_response();
    };

    let alerts = state.db.collection::<Alert>("alerts");
    let find_opts = FindOptions::builder().sort(doc! { "created_at": -1 }).build();

    let mut cursor = match alerts.find(doc! { "user_id": u.id }, find_opts).await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
    };

    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();

    while let Some(res) = cursor.next().await {
        let a = match res {
            Ok(a) => a,
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
        };

        map.entry(a.symbol.clone())
            .or_default()
            .push(json!({
                "id": a.id.to_hex(),
                "condition": a.condition,
                "target_price": fmt2(a.target_price),
            }));
    }

    let mut groups: Vec<serde_json::Value> = vec![];
    for (symbol, items) in map {
        groups.push(json!({ "symbol": symbol, "alerts": items }));
    }

    let ctx = json!({ "groups": if groups.is_empty() { serde_json::Value::Null } else { serde_json::Value::Array(groups) }});
    let html = state.hbs.render("partials/watchlist_alerts", &ctx).unwrap_or_else(|e| format!("template error: {e}"));

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(html)).into_response();
    }
    (StatusCode::OK, Html(html)).into_response()
}

// POST /funds
#[derive(Deserialize)]
pub struct DepositForm {
    pub amount: f64,
}

pub async fn post_funds(
    State(state): State<AppState>,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<DepositForm>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::UNAUTHORIZED, Html("Please log in.".to_string())).into_response();
    };

    if !form.amount.is_finite() || form.amount <= 0.0 {
        return (StatusCode::BAD_REQUEST, Html("Amount must be > 0".to_string())).into_response();
    }

    let mut acc = match get_or_create_account(&state, u.id).await {
        Ok(a) => a,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
    };

    acc.cash += form.amount;
    acc.updated_at = Utc::now().timestamp();

    let accounts = state.db.collection::<Account>("accounts");
    if let Err(e) = accounts.update_one(
        doc! { "_id": acc.id },
        doc! { "$set": { "cash": acc.cash, "updated_at": acc.updated_at } },
        None
    ).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response();
    }

    let msg = format!(r#"<div class="alert alert-success">Deposited <b>${}</b>. New cash: <b>${}</b></div>"#, fmt2(form.amount), fmt2(acc.cash));
    (StatusCode::OK, Html(msg)).into_response()
}
