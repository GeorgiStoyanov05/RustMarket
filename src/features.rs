use futures_util::StreamExt;

use axum::{
    Form,
    extract::{Extension, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use chrono::Utc;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::options::{FindOptions, UpdateOptions};
use serde::Deserialize;
use serde_json::json;

use crate::{
    AppState,
    models::{Account, Alert, CurrentUser, Position},
};

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn unauthorized_snippet() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Html(r#"<div class="text-danger">Unauthorized</div>"#.to_string()),
    )
        .into_response()
}

fn hx_trigger_value(events: &[&str]) -> HeaderValue {
    // HX-Trigger supports either:
    //  - a string: "eventName"
    //  - a JSON object: {"event1":true,"event2":true}
    if events.len() == 1 {
        return HeaderValue::from_str(events[0]).unwrap_or_else(|_| HeaderValue::from_static(""));
    }

    let mut map = serde_json::Map::new();
    for &e in events {
        map.insert(e.to_string(), serde_json::Value::Bool(true));
    }

    let json = serde_json::Value::Object(map).to_string();
    HeaderValue::from_str(&json).unwrap_or_else(|_| HeaderValue::from_static(""))
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

async fn get_position(
    state: &AppState,
    user_id: ObjectId,
    symbol: &str,
) -> Result<Option<Position>, String> {
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
            mongodb::options::UpdateOptions::builder()
                .upsert(true)
                .build(),
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
    Html(
        r#"<div class="text-muted small">Log in to view and manage your position.</div>"#
            .to_string(),
    )
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
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            )
                .into_response();
        }
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
    pub qty: String,
}

pub async fn post_trade_buy(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<TradeForm>,
) -> Response {
    let Some(Extension(u)) = user else {
        return unauthorized_snippet();
    };

    let sym = symbol.to_uppercase();

    let qty_str = form.qty.trim();
    let qty: i64 = match qty_str.parse() {
        Ok(q) => q,
        Err(_) => {
            return (
                StatusCode::OK,
                Html(r#"<div class="text-danger">Enter a valid quantity.</div>"#.to_string()),
            )
                .into_response();
        }
    };

    if qty <= 0 {
        return (
            StatusCode::OK,
            Html(r#"<div class="text-danger">Enter a valid quantity.</div>"#.to_string()),
        )
            .into_response();
    }

    let quote = match state.finnhub.quote(&sym).await {
        Ok(q) => q,
        Err(e) => {
            return (
                StatusCode::OK,
                Html(format!(
                    r#"<div class="text-danger">Quote error: {e}</div>"#
                )),
            )
                .into_response();
        }
    };

    let price = quote.c;
    let total = price * (qty as f64);

    let mut acc = match get_or_create_account(&state, u.id).await {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            )
                .into_response();
        }
    };

    if acc.cash < total {
        return (
            StatusCode::OK,
            Html(r#"<div class="text-danger">Not enough funds.</div>"#.to_string()),
        )
            .into_response();
    }

    // update cash
    acc.cash -= total;
    acc.updated_at = Utc::now().timestamp();

    let accounts = state.db.collection::<Account>("accounts");
    if let Err(e) = accounts
        .update_one(
            doc! { "_id": u.id },
            doc! { "$set": { "cash": acc.cash, "updated_at": acc.updated_at } },
            None,
        )
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("db error: {e}")),
        )
            .into_response();
    }

    // upsert position
    let positions = state.db.collection::<Position>("positions");
    let existing = positions
        .find_one(doc! { "user_id": u.id, "symbol": &sym }, None)
        .await
        .ok()
        .flatten();

    let now = Utc::now().timestamp();

    let (new_qty, new_avg) = if let Some(p) = existing {
        let total_qty = p.qty + qty;
        let total_cost = (p.avg_price * (p.qty as f64)) + (price * (qty as f64));
        let avg = if total_qty > 0 {
            total_cost / (total_qty as f64)
        } else {
            price
        };
        (total_qty, avg)
    } else {
        (qty, price)
    };

    if let Err(e) = positions
        .update_one(
            doc! { "user_id": u.id, "symbol": &sym },
            doc! {
                "$set": {
                    "qty": new_qty,
                    "avg_price": new_avg,
                    "updated_at": now
                },
                "$setOnInsert": {
                    "created_at": now,
                    "user_id": u.id,
                    "symbol": &sym
                }
            },
            UpdateOptions::builder().upsert(true).build(),
        )
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("db error: {e}")),
        )
            .into_response();
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        hx_trigger_value(&["cashUpdated", "positionUpdated"]),
    );

    (
        StatusCode::OK,
        headers,
        Html(r#"<div class="text-success">Buy successful.</div>"#.to_string()),
    )
        .into_response()
}

pub async fn post_trade_sell(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<TradeForm>,
) -> Response {
    let Some(Extension(u)) = user else {
        return unauthorized_snippet();
    };

    let sym = symbol.to_uppercase();

    let qty_str = form.qty.trim();
    let qty: i64 = match qty_str.parse() {
        Ok(q) => q,
        Err(_) => {
            return (
                StatusCode::OK,
                Html(r#"<div class="text-danger">Enter a valid quantity.</div>"#.to_string()),
            )
                .into_response();
        }
    };

    if qty <= 0 {
        return (
            StatusCode::OK,
            Html(r#"<div class="text-danger">Enter a valid quantity.</div>"#.to_string()),
        )
            .into_response();
    }

    let quote = match state.finnhub.quote(&sym).await {
        Ok(q) => q,
        Err(e) => {
            return (
                StatusCode::OK,
                Html(format!(
                    r#"<div class="text-danger">Quote error: {e}</div>"#
                )),
            )
                .into_response();
        }
    };

    let price = quote.c;

    // find position
    let positions = state.db.collection::<Position>("positions");
    let pos = match positions
        .find_one(doc! { "user_id": u.id, "symbol": &sym }, None)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            )
                .into_response();
        }
    };

    let Some(mut p) = pos else {
        return (
            StatusCode::OK,
            Html(r#"<div class="text-danger">You don't own this stock.</div>"#.to_string()),
        )
            .into_response();
    };

    if qty > p.qty {
        return (
            StatusCode::OK,
            Html(r#"<div class="text-danger">Not enough shares.</div>"#.to_string()),
        )
            .into_response();
    }

    // update cash
    let mut acc = match get_or_create_account(&state, u.id).await {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            )
                .into_response();
        }
    };

    let proceeds = price * (qty as f64);
    acc.cash += proceeds;
    acc.updated_at = Utc::now().timestamp();

    let accounts = state.db.collection::<Account>("accounts");
    if let Err(e) = accounts
        .update_one(
            doc! { "_id": u.id },
            doc! { "$set": { "cash": acc.cash, "updated_at": acc.updated_at } },
            None,
        )
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("db error: {e}")),
        )
            .into_response();
    }

    // update / delete position
    p.qty -= qty;
    p.updated_at = Utc::now().timestamp();

    if p.qty <= 0 {
        if let Err(e) = positions
            .delete_one(doc! { "user_id": u.id, "symbol": &sym }, None)
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            )
                .into_response();
        }
    } else {
        if let Err(e) = positions
            .update_one(
                doc! { "user_id": u.id, "symbol": &sym },
                doc! { "$set": { "qty": p.qty, "updated_at": p.updated_at } },
                None,
            )
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            )
                .into_response();
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        hx_trigger_value(&["cashUpdated", "positionUpdated"]),
    );

    (
        StatusCode::OK,
        headers,
        Html(r#"<div class="text-success">Sell successful.</div>"#.to_string()),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct CreateAlertForm {
    #[serde(rename = "targetPrice")]
    pub target_price: String,
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
        return (
            StatusCode::OK,
            Html(r#"<div class="text-muted small">Log in to manage alerts.</div>"#.to_string()),
        )
            .into_response();
    };

    let sym = symbol.to_uppercase();
    let alerts = state.db.collection::<Alert>("alerts");

    let find_opts = FindOptions::builder()
        .sort(doc! { "created_at": -1 })
        .build();

    let mut cursor = match alerts
        .find(doc! { "user_id": u.id, "symbol": &sym }, find_opts)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            )
                .into_response();
        }
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
                  "target_price_raw": a.target_price,
                  "triggered": a.triggered,
                }));
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(format!("db error: {e}")),
                )
                    .into_response();
            }
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

pub async fn post_create_alert(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<CreateAlertForm>,
) -> Response {
    let Some(Extension(u)) = user else {
        return unauthorized_snippet();
    };

    let sym = symbol.to_uppercase();

    let cond = form.condition.to_lowercase();
    if cond != "above" && cond != "below" {
        return (
            StatusCode::OK,
            Html(r#"<div class="text-danger">Please choose a valid condition.</div>"#.to_string()),
        )
            .into_response();
    }

    let target_str = form.target_price.trim();
    let target: f64 = match target_str.parse() {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::OK,
                Html(
                    r#"<div class="text-danger">Please enter a valid target price.</div>"#
                        .to_string(),
                ),
            )
                .into_response();
        }
    };

    if !target.is_finite() || target <= 0.0 {
        return (
            StatusCode::OK,
            Html(
                r#"<div class="text-danger">Please enter a valid target price.</div>"#.to_string(),
            ),
        )
            .into_response();
    }

    let alerts = state.db.collection::<Alert>("alerts");
    let now = Utc::now().timestamp();

    let alert = Alert {
        id: ObjectId::new(),
        user_id: u.id,
        symbol: sym,
        condition: cond,
        target_price: target,
        created_at: now,
        triggered: false,
        triggered_at: None,
    };

    if let Err(e) = alerts.insert_one(alert, None).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("db error: {e}")),
        )
            .into_response();
    }

    let mut headers = HeaderMap::new();
    headers.insert("HX-Trigger", hx_trigger_value(&["alertsUpdated"]));

    (
        StatusCode::OK,
        headers,
        Html(r#"<div class="text-success">Alert created.</div>"#.to_string()),
    )
        .into_response()
}

pub async fn post_delete_alert(
    State(state): State<AppState>,
    Path((symbol, id)): Path<(String, String)>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (
            StatusCode::UNAUTHORIZED,
            Html(r#"<div class="text-danger">Unauthorized</div>"#.to_string()),
        )
            .into_response();
    };

    let oid = match ObjectId::parse_str(&id) {
        Ok(x) => x,
        Err(_) => return (StatusCode::BAD_REQUEST, Html("bad id".to_string())).into_response(),
    };

    let sym = symbol.to_uppercase();
    let alerts = state.db.collection::<Alert>("alerts");

    if let Err(e) = alerts
        .delete_one(doc! { "_id": oid, "user_id": u.id, "symbol": &sym }, None)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("db error: {e}")),
        )
            .into_response();
    }

    // Notify all open pages/tabs
    let _ = state.events_tx.send("alertsUpdated".to_string());

    let mut headers = HeaderMap::new();
    headers.insert("HX-Trigger", HeaderValue::from_static("alertsUpdated"));

    (
        StatusCode::OK,
        headers,
        Html(r#"<div class="text-success">Alert deleted.</div>"#.to_string()),
    )
        .into_response()
}


pub async fn post_delete_alert_global(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return unauthorized_snippet();
    };

    let oid = match ObjectId::parse_str(&id) {
        Ok(x) => x,
        Err(_) => return (StatusCode::BAD_REQUEST, Html("bad id".to_string())).into_response(),
    };

    let alerts = state.db.collection::<Alert>("alerts");

    if let Err(e) = alerts
        .delete_one(doc! { "_id": oid, "user_id": u.id }, None)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("db error: {e}")),
        )
            .into_response();
    }

    let mut headers = HeaderMap::new();
    let _ = state.events_tx.send("alertsUpdated".to_string());
    headers.insert("HX-Trigger", hx_trigger_value(&["alertsUpdated"]));

    (StatusCode::OK, headers, Html("".to_string())).into_response()
}

pub async fn post_trigger_alert(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return unauthorized_snippet();
    };

    let oid = match ObjectId::parse_str(&id) {
        Ok(x) => x,
        Err(_) => return (StatusCode::BAD_REQUEST, Html("bad id".to_string())).into_response(),
    };

    let alerts = state.db.collection::<Alert>("alerts");

    // delete alert after trigger
    if let Err(e) = alerts
        .delete_one(doc! { "_id": oid, "user_id": u.id }, None)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("db error: {e}")),
        )
            .into_response();
    }

    let mut headers = HeaderMap::new();
    headers.insert("HX-Trigger", hx_trigger_value(&["alertsUpdated"]));

    (StatusCode::OK, headers, Html("".to_string())).into_response()
}
