use futures_util::StreamExt;

use axum::{
    extract::{Extension, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    Form,
};
use chrono::Utc;
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

fn htmx_trigger(mut res: Response, event: &str) -> Response {
    // triggers a client-side event on the <body>
    if let Ok(v) = HeaderValue::from_str(event) {
        res.headers_mut().insert("HX-Trigger", v);
    }
    res
}

fn fmt2(x: f64) -> String {
    format!("{:.2}", x)
}

async fn get_or_create_account(state: &AppState, user_id: ObjectId) -> Result<Account, String> {
    let accounts = state.db.collection::<Account>("accounts");

    if let Ok(Some(acc)) = accounts.find_one(doc! { "_id": user_id }, None).await {
        return Ok(acc);
    }

    // Start users with some demo cash so you can immediately test trading.
    let acc = Account {
        id: user_id,
        cash: 10_000.0,
        updated_at: Utc::now().timestamp(),
    };

    accounts
        .insert_one(&acc, None)
        .await
        .map_err(|e| e.to_string())?;

    Ok(acc)
}

async fn get_position(state: &AppState, user_id: ObjectId, symbol: &str) -> Result<Option<Position>, String> {
    let positions = state.db.collection::<Position>("positions");
    positions
        .find_one(doc! { "user_id": user_id, "symbol": symbol }, None)
        .await
        .map_err(|e| e.to_string())
}

async fn upsert_position(state: &AppState, pos: &Position) -> Result<(), String> {
    let positions = state.db.collection::<Position>("positions");
    positions
        .update_one(
            doc! { "_id": pos.id },
            doc! {
                "$set": {
                    "user_id": pos.user_id,
                    "symbol": &pos.symbol,
                    "qty": pos.qty,
                    "avg_price": pos.avg_price,
                    "updated_at": pos.updated_at,
                }
            },
            mongodb::options::UpdateOptions::builder().upsert(true).build(),
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn delete_position(state: &AppState, id: ObjectId) -> Result<(), String> {
    let positions = state.db.collection::<Position>("positions");
    positions
        .delete_one(doc! { "_id": id }, None)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn not_logged_in_panel() -> Html<String> {
    Html(r#"<div class="text-muted small">Log in to view and manage your position.</div>"#.to_string())
}

// GET /positions/:symbol  (HTMX partial)
pub async fn get_position_panel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::OK, not_logged_in_panel()).into_response();
    };

    let sym = symbol.to_uppercase();

    let pos = match get_position(&state, u.id, &sym).await {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
    };

    // current price (best effort)
    let current_price = match state.finnhub.quote(&sym).await {
        Ok(q) => q.c,
        Err(_) => 0.0,
    };

    let ctx = match pos {
        Some(p) => {
            let pnl = (current_price - p.avg_price) * (p.qty as f64);
            let pct = if p.avg_price > 0.0 {
                ((current_price - p.avg_price) / p.avg_price) * 100.0
            } else {
                0.0
            };
            let pnl_class = if pnl > 0.0 {
                "text-success"
            } else if pnl < 0.0 {
                "text-danger"
            } else {
                "text-muted"
            };

            json!({
                "has_position": true,
                "symbol": sym,
                "qty": p.qty,
                "avg_price": fmt2(p.avg_price),
                "avg_price_raw": p.avg_price,
                "last_price": fmt2(current_price),
                "pnl": (if pnl>0.0 {"+"} else {""}).to_string() + &fmt2(pnl),
                "pnl_pct": (if pct>0.0 {"+"} else {""}).to_string() + &fmt2(pct),
                "pnl_class": pnl_class,
            })
        }
        None => json!({ "has_position": false, "symbol": sym }),
    };

    let html = state
        .hbs
        .render("partials/position_panel", &ctx)
        .unwrap_or_else(|e| format!("template error: {e}"));

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(html)).into_response();
    }
    (StatusCode::OK, Html(html)).into_response()
}

#[derive(Deserialize)]
pub struct TradeForm {
    pub qty: i64,
}

// POST /trade/:symbol/buy
pub async fn post_trade_buy(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<TradeForm>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::UNAUTHORIZED, Html("Please log in.".to_string())).into_response();
    };

    let sym = symbol.to_uppercase();
    if form.qty <= 0 {
        return (StatusCode::BAD_REQUEST, Html("Quantity must be > 0".to_string())).into_response();
    }

    let quote = match state.finnhub.quote(&sym).await {
        Ok(q) => q,
        Err(e) => return (StatusCode::BAD_REQUEST, Html(format!("Quote error: {e}"))).into_response(),
    };
    let price = quote.c;
    let total = price * (form.qty as f64);

    // Account
    let mut acc = match get_or_create_account(&state, u.id).await {
        Ok(a) => a,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
    };

    if acc.cash + 1e-9 < total {
        let msg = format!(
            r#"<div class="alert alert-danger mb-2">Not enough funds. Cash: <b>${}</b>, Required: <b>${}</b></div>"#,
            fmt2(acc.cash), fmt2(total)
        );
        return (StatusCode::OK, Html(msg)).into_response();
    }

    // Update cash
    acc.cash -= total;
    acc.updated_at = Utc::now().timestamp();

    let accounts = state.db.collection::<Account>("accounts");
    if let Err(e) = accounts
        .update_one(
            doc! { "_id": acc.id },
            doc! { "$set": { "cash": acc.cash, "updated_at": acc.updated_at } },
            None,
        )
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response();
    }

    // Update/create position
    let now = Utc::now().timestamp();
    let existing = match get_position(&state, u.id, &sym).await {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
    };

    let new_pos = if let Some(mut p) = existing {
        let old_qty = p.qty as f64;
        let add_qty = form.qty as f64;
        let new_qty_i = p.qty + form.qty;
        let new_avg = if new_qty_i > 0 {
            ((p.avg_price * old_qty) + (price * add_qty)) / (new_qty_i as f64)
        } else {
            price
        };
        p.qty = new_qty_i;
        p.avg_price = new_avg;
        p.updated_at = now;
        p
    } else {
        Position {
            id: ObjectId::new(),
            user_id: u.id,
            symbol: sym.clone(),
            qty: form.qty,
            avg_price: price,
            updated_at: now,
        }
    };

    if let Err(e) = upsert_position(&state, &new_pos).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response();
    }

    let msg = format!(
        r#"<div class="alert alert-success mb-2">Bought <b>{}</b> shares of <b>{}</b> @ <b>${}</b></div>"#,
        form.qty,
        sym,
        fmt2(price)
    );

    let res = (StatusCode::OK, Html(msg)).into_response();
    htmx_trigger(res, r#"{"positionUpdated":true}"#)
}

// POST /trade/:symbol/sell
pub async fn post_trade_sell(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<TradeForm>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::UNAUTHORIZED, Html("Please log in.".to_string())).into_response();
    };

    let sym = symbol.to_uppercase();
    if form.qty <= 0 {
        return (StatusCode::BAD_REQUEST, Html("Quantity must be > 0".to_string())).into_response();
    }

    let mut pos = match get_position(&state, u.id, &sym).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (StatusCode::OK, Html(r#"<div class="alert alert-danger mb-2">You don’t own this stock.</div>"#.to_string()))
                .into_response()
        }
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
    };

    if form.qty > pos.qty {
        return (StatusCode::OK, Html(format!(
            r#"<div class="alert alert-danger mb-2">Not enough shares. You have <b>{}</b>.</div>"#,
            pos.qty
        )))
        .into_response();
    }

    let quote = match state.finnhub.quote(&sym).await {
        Ok(q) => q,
        Err(e) => return (StatusCode::BAD_REQUEST, Html(format!("Quote error: {e}"))).into_response(),
    };
    let price = quote.c;
    let total = price * (form.qty as f64);

    // Account
    let mut acc = match get_or_create_account(&state, u.id).await {
        Ok(a) => a,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
    };

    acc.cash += total;
    acc.updated_at = Utc::now().timestamp();

    let accounts = state.db.collection::<Account>("accounts");
    if let Err(e) = accounts
        .update_one(
            doc! { "_id": acc.id },
            doc! { "$set": { "cash": acc.cash, "updated_at": acc.updated_at } },
            None,
        )
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response();
    }

    // Update/delete position
    pos.qty -= form.qty;
    pos.updated_at = Utc::now().timestamp();

    if pos.qty <= 0 {
        if let Err(e) = delete_position(&state, pos.id).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response();
        }
    } else if let Err(e) = upsert_position(&state, &pos).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response();
    }

    let msg = format!(
        r#"<div class="alert alert-success mb-2">Sold <b>{}</b> shares of <b>{}</b> @ <b>${}</b></div>"#,
        form.qty,
        sym,
        fmt2(price)
    );

    let res = (StatusCode::OK, Html(msg)).into_response();
    htmx_trigger(res, r#"{"positionUpdated":true}"#)
}

#[derive(Deserialize)]
pub struct CreateAlertForm {
    #[serde(rename = "targetPrice")]
    pub target_price: f64,
    pub condition: String,
}

// GET /alerts/:symbol/list  (HTMX partial)
pub async fn get_alerts_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::OK, Html(r#"<div class="text-muted small">Log in to manage alerts.</div>"#.to_string())).into_response();
    };

    let sym = symbol.to_uppercase();
    let alerts = state.db.collection::<Alert>("alerts");

    let find_opts = FindOptions::builder().sort(doc! { "created_at": -1 }).build();

    let mut cursor = match alerts
        .find(doc! { "user_id": u.id, "symbol": &sym }, find_opts)
        .await
    {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
    };

    let mut items: Vec<serde_json::Value> = Vec::new();
    while let Some(res) = cursor.next().await {
        match res {
            Ok(a) => {
                items.push(json!({
                    "id": a.id.to_hex(),
                    "symbol": a.symbol,
                    "condition": a.condition,
                    "target_price": fmt2(a.target_price),
                    "triggered": a.triggered,
                }));
            }
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
        }
    }

    let ctx = json!({ "symbol": sym, "alerts": items, "has_alerts": !items.is_empty() });

    let html = state
        .hbs
        .render("partials/alerts_list", &ctx)
        .unwrap_or_else(|e| format!("template error: {e}"));

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(html)).into_response();
    }
    (StatusCode::OK, Html(html)).into_response()
}

// POST /alerts/:symbol
pub async fn post_create_alert(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<CreateAlertForm>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::UNAUTHORIZED, Html("Please log in.".to_string())).into_response();
    };

    let sym = symbol.to_uppercase();

    let cond = form.condition.to_lowercase();
    if cond != "above" && cond != "below" {
        return (StatusCode::BAD_REQUEST, Html("Invalid condition".to_string())).into_response();
    }
    if !form.target_price.is_finite() || form.target_price <= 0.0 {
        return (StatusCode::BAD_REQUEST, Html("Target price must be > 0".to_string())).into_response();
    }

    let alert = Alert {
        id: ObjectId::new(),
        user_id: u.id,
        symbol: sym.clone(),
        condition: cond,
        target_price: form.target_price,
        created_at: Utc::now().timestamp(),
        triggered: false,
        triggered_at: None,
    };

    let alerts = state.db.collection::<Alert>("alerts");
    if let Err(e) = alerts.insert_one(&alert, None).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response();
    }

    let msg = Html(r#"<div class="alert alert-success mb-2">Alert created.</div>"#.to_string());
    let res = (StatusCode::OK, msg).into_response();
    htmx_trigger(res, r#"{"alertsUpdated":true}"#)
}

// POST /alerts/:symbol/:id/delete
pub async fn post_delete_alert(
    State(state): State<AppState>,
    Path((symbol, id)): Path<(String, String)>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::UNAUTHORIZED, Html("Please log in.".to_string())).into_response();
    };

    let sym = symbol.to_uppercase();
    let oid = match ObjectId::parse_str(&id) {
        Ok(x) => x,
        Err(_) => return (StatusCode::BAD_REQUEST, Html("Invalid id".to_string())).into_response(),
    };

    let alerts = state.db.collection::<Alert>("alerts");
    if let Err(e) = alerts
        .delete_one(doc! { "_id": oid, "user_id": u.id, "symbol": &sym }, None)
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response();
    }

    let res = (StatusCode::OK, Html(r#"<div class="alert alert-success mb-2">Alert deleted.</div>"#.to_string())).into_response();
    htmx_trigger(res, r#"{"alertsUpdated":true}"#)
}

// POST /alerts/by-id/:id/delete  (for pages that don't have symbol handy)
pub async fn post_delete_alert_global(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::UNAUTHORIZED, Html("Please log in.".to_string())).into_response();
    };

    let oid = match ObjectId::parse_str(&id) {
        Ok(x) => x,
        Err(_) => return (StatusCode::BAD_REQUEST, Html("Invalid id".to_string())).into_response(),
    };

    let alerts = state.db.collection::<Alert>("alerts");
    if let Err(e) = alerts.delete_one(doc! { "_id": oid, "user_id": u.id }, None).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response();
    }

    let res = (StatusCode::OK, Html(r#"<div class="alert alert-success mb-2">Alert deleted.</div>"#.to_string())).into_response();
    htmx_trigger(res, r#"{"alertsUpdated":true}"#)
}
