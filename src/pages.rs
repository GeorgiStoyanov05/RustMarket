use axum::{
    extract::{Extension, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
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
    render,
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
    state
        .hbs
        .render(tpl, &ctx)
        .unwrap_or_else(|e| format!("template error: {e}"))
}

fn fmt2(x: f64) -> String {
    format!("{:.2}", x)
}

async fn get_or_create_account(state: &AppState, user_id: ObjectId) -> Result<Account, String> {
    let accounts = state.db.collection::<Account>("accounts");
    if let Ok(Some(acc)) = accounts.find_one(doc! { "_id": user_id }, None).await {
        return Ok(acc);
    }
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

// ---------------- Pages ----------------

pub async fn get_portfolio_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    if is_htmx(&headers) {
        let html = render_page(&state, "pages/portfolio", json!({}));
        return (StatusCode::OK, Html(html)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);
    match render::render_shell(&state, "/portfolio", user_ref, false) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn get_alerts_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    if is_htmx(&headers) {
        let html = render_page(&state, "pages/alerts", json!({}));
        return (StatusCode::OK, Html(html)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);
    match render::render_shell(&state, "/alerts", user_ref, false) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn get_funds_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    if let Some(Extension(u)) = user.as_ref() {
        let _ = get_or_create_account(&state, u.id).await;
    }

    if is_htmx(&headers) {
        let html = render_page(&state, "pages/funds", json!({}));
        return (StatusCode::OK, Html(html)).into_response();
    }

    // Match the Go app: direct navigation to /funds returns the shell + auto-opens the modal.
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
        let _ = get_or_create_account(&state, u.id).await;
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

    let acc = match get_or_create_account(&state, u.id).await {
        Ok(a) => a,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))),
    };

    let html = render_page(
        &state,
        "partials/cash_badge",
        json!({ "cash": fmt2(acc.cash) }),
    );
    (StatusCode::OK, Html(html))
}

// ---------------- Partials ----------------

// GET /portfolio/positions
pub async fn get_portfolio_positions(
    State(state): State<AppState>,
    _headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let mut groups: Vec<serde_json::Value> = vec![];

    if let Some(Extension(u)) = user {
        let positions = state.db.collection::<Position>("positions");
        let find_opts = FindOptions::builder().sort(doc! { "updated_at": -1 }).build();

        let mut cursor = match positions.find(doc! { "user_id": u.id }, find_opts).await {
            Ok(c) => c,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(format!("db error: {e}")),
                )
                    .into_response()
            }
        };

        while let Some(res) = cursor.next().await {
            let p = match res {
                Ok(p) => p,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Html(format!("db error: {e}")),
                    )
                        .into_response()
                }
            };

            let symbol = p.symbol.to_uppercase();

            let q = state.finnhub.quote(&symbol).await.ok();
            let last = q.map(|x| x.c).unwrap_or(0.0);

            let pnl = (last - p.avg_price) * (p.qty as f64);

            groups.push(json!({
                "symbol": symbol,
                "qty": p.qty,
                "avg_price": fmt2(p.avg_price),
                "last": fmt2(last),
                "pnl": fmt2(pnl),
            }));
        }
    }

    let ctx = json!({
        "groups": if groups.is_empty() { serde_json::Value::Null } else { serde_json::Value::Array(groups) }
    });

    let body = state
        .hbs
        .render("partials/portfolio_positions", &ctx)
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(body)).into_response()
}


// ✅ NEW: GET /portfolio/position/:symbol
// Returns ONE card (outerHTML swap target). If the position no longer exists, returns empty string (removes the card).
pub async fn get_portfolio_position_card(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (StatusCode::OK, Html("".to_string())).into_response();
    };

    let sym = symbol.to_uppercase();
    let positions = state.db.collection::<Position>("positions");

    let p = match positions
        .find_one(doc! { "user_id": u.id, "symbol": &sym }, None)
        .await
    {
        Ok(x) => x,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("db error: {e}"))).into_response(),
    };

    let Some(p) = p else {
        // Returning empty body + 200 means outerHTML swap will remove the card.
        return (StatusCode::OK, Html("".to_string())).into_response();
    };

    let q = state.finnhub.quote(&sym).await.ok();
    let last = q.map(|x| x.c).unwrap_or(0.0);

    let pnl = (last - p.avg_price) * (p.qty as f64);
    let pct = if p.avg_price > 0.0 { ((last - p.avg_price) / p.avg_price) * 100.0 } else { 0.0 };
    let pnl_class = if pnl > 0.0 { "text-success" } else if pnl < 0.0 { "text-danger" } else { "text-muted" };

    let ctx = json!({
        "symbol": sym,
        "qty": p.qty,
        "avg": fmt2(p.avg_price),
        "current_price": fmt2(last),
        "pnl": (if pnl>0.0 { "+" } else { "" }).to_string() + &fmt2(pnl),
        "pnl_pct": (if pct>0.0 { "+" } else { "" }).to_string() + &fmt2(pct),
        "pnl_class": pnl_class
    });

    let html = state.hbs
        .render("partials/portfolio_position_card", &ctx)
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(html)).into_response()
}

pub async fn get_watchlist_alerts(
    State(state): State<AppState>,
    _headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    use std::collections::BTreeMap;

    let mut groups: Vec<serde_json::Value> = vec![];

    if let Some(Extension(u)) = user {
        let alerts = state.db.collection::<Alert>("alerts");
        let find_opts = FindOptions::builder().sort(doc! { "created_at": -1 }).build();

        let mut cursor = match alerts.find(doc! { "user_id": u.id }, find_opts).await {
            Ok(c) => c,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(format!("db error: {e}")),
                )
                    .into_response()
            }
        };

        let mut map: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();

        while let Some(res) = cursor.next().await {
            let a = match res {
                Ok(a) => a,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Html(format!("db error: {e}")),
                    )
                        .into_response()
                }
            };

            let sym = a.symbol.to_uppercase();
            map.entry(sym).or_default().push(json!({
                "id": a.id.to_hex(),
                "condition": a.condition,
                "target_price": fmt2(a.target_price),
                "created_at": a.created_at,
                "triggered": a.triggered,
                "triggered_at": a.triggered_at,
            }));
        }

        for (symbol, alerts) in map {
            groups.push(json!({
                "symbol": symbol,
                "alerts": alerts
            }));
        }
    }

    let ctx = json!({
        "groups": if groups.is_empty() { serde_json::Value::Null } else { serde_json::Value::Array(groups) }
    });

    let body = state
        .hbs
        .render("partials/watchlist_alerts", &ctx)
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(body)).into_response()
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
            Html(r#"<div class="alert alert-danger mb-0">There was an error getting user</div>"#.to_string()),
        )
            .into_response();
    };

    let amount_str = form.amount.trim();
    let amount: f64 = match amount_str.parse() {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::OK,
                Html(r#"<div class="alert alert-danger mb-0">There was an error with the amount!</div>"#.to_string()),
            )
                .into_response();
        }
    };

    if !amount.is_finite() || amount <= 0.0 {
        return (
            StatusCode::OK,
            Html(r#"<div class="alert alert-danger mb-0">Amount must be bigger than zero!</div>"#.to_string()),
        )
            .into_response();
    }

    let mut acc = match get_or_create_account(&state, u.id).await {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            )
                .into_response()
        }
    };

    acc.cash += amount;
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

    let msg =
        r#"<div class="alert alert-success mb-0">The deposit was successful!</div>"#.to_string();

    let mut headers = HeaderMap::new();
    headers.insert("HX-Trigger", HeaderValue::from_static(r#"{"cashUpdated":true}"#));

    (StatusCode::OK, headers, Html(msg)).into_response()
}
