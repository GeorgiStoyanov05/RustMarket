use axum::{
    extract::{Extension, Form, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use mongodb::bson::oid::ObjectId;
use serde::Deserialize;
use serde_json::json;

use crate::{
    models::CurrentUser,
    render,
    services::alerts_service,
    AppState,
};

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn hx_trigger_value(events: &[&str]) -> HeaderValue {
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

fn unauthorized_snippet() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Html(r#"<div class=\"text-danger\">Unauthorized</div>"#.to_string()),
    )
        .into_response()
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

// ---------------- Pages ----------------

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

// ---------------- Partials ----------------

#[derive(Deserialize)]
pub struct CreateAlertForm {
    #[serde(rename = "targetPrice")]
    pub target_price: String,
    pub condition: String,
}

// GET /alerts/:symbol/list
pub async fn get_alerts_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(symbol): Path<String>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (
            StatusCode::OK,
            Html(r#"<div class=\"text-muted small\">Log in to manage alerts.</div>"#.to_string()),
        )
            .into_response();
    };

    let sym = symbol.to_uppercase();
    let alerts = match alerts_service::list_user_symbol_alerts(&state, u.id, &sym).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            )
                .into_response();
        }
    };

    let items: Vec<serde_json::Value> = alerts
        .into_iter()
        .map(|a| {
            json!({
              "id": a.id.to_hex(),
              "symbol": a.symbol,
              "condition": a.condition,
              "target_price": fmt2(a.target_price),
              "target_price_raw": a.target_price,
              "triggered": a.triggered,
            })
        })
        .collect();

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
        return unauthorized_snippet();
    };

    let sym = symbol.to_uppercase();

    let cond = form.condition.to_lowercase();
    if cond != "above" && cond != "below" {
        return (
            StatusCode::OK,
            Html(r#"<div class=\"text-danger\">Please choose a valid condition.</div>"#.to_string()),
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
                    r#"<div class=\"text-danger\">Please enter a valid target price.</div>"#
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
                r#"<div class=\"text-danger\">Please enter a valid target price.</div>"#.to_string(),
            ),
        )
            .into_response();
    }

    if let Err(e) = alerts_service::create_alert(&state, u.id, &sym, &cond, target).await {
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
        Html(r#"<div class=\"text-success\">Alert created.</div>"#.to_string()),
    )
        .into_response()
}

// POST /alerts/:symbol/:id/delete
pub async fn post_delete_alert(
    State(state): State<AppState>,
    Path((symbol, id)): Path<(String, String)>,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let Some(Extension(u)) = user else {
        return (
            StatusCode::UNAUTHORIZED,
            Html(r#"<div class=\"text-danger\">Unauthorized</div>"#.to_string()),
        )
            .into_response();
    };

    let oid = match ObjectId::parse_str(&id) {
        Ok(x) => x,
        Err(_) => return (StatusCode::BAD_REQUEST, Html("bad id".to_string())).into_response(),
    };

    if let Err(e) = alerts_service::delete_alert_for_symbol(&state, u.id, &symbol, oid).await {
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

// POST /alerts/:id/delete
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

    if let Err(e) = alerts_service::delete_alert_global(&state, u.id, oid).await {
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

// POST /alerts/:id/trigger
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

    let triggered_now = match alerts_service::trigger_alert(&state, u.id, oid).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("db error: {e}")),
            )
                .into_response();
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert("HX-Trigger", hx_trigger_value(&["alertsUpdated"]));

    let msg = if !triggered_now {
        r#"<div class=\"text-muted\">Alert already triggered.</div>"#
    } else {
        r#"<div class=\"text-warning\">⚠️ Alert triggered!</div>"#
    };

    (StatusCode::OK, headers, Html(msg.to_string())).into_response()
}

// GET /watchlist/alerts
pub async fn get_watchlist_alerts(
    State(state): State<AppState>,
    _headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> Response {
    let mut groups: Vec<serde_json::Value> = vec![];

    if let Some(Extension(u)) = user {
        let map = match alerts_service::list_user_alerts_grouped(&state, u.id).await {
            Ok(m) => m,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(format!("db error: {e}")),
                )
                    .into_response()
            }
        };

        for (symbol, alerts) in map {
            let alerts_json: Vec<serde_json::Value> = alerts
                .into_iter()
                .map(|a| {
                    json!({
                        "id": a.id.to_hex(),
                        "condition": a.condition,
                        "target_price": fmt2(a.target_price),
                        "created_at": a.created_at,
                        "triggered": a.triggered,
                        "triggered_at": a.triggered_at,
                    })
                })
                .collect();

            groups.push(json!({
                "symbol": symbol,
                "alerts": alerts_json
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
