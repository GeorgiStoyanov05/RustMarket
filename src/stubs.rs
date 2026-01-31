use axum::{
    extract::Path,
    http::{HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};

fn esc(s: &str) -> String {
    // Minimal HTML escaping (symbols are usually safe, but this avoids surprises)
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub async fn get_alerts_list(Path(symbol): Path<String>) -> impl IntoResponse {
    let sym = esc(&symbol);
    (StatusCode::OK, Html(format!(r#"<div class="text-muted small">No alerts for {sym} yet.</div>"#)))
}

pub async fn get_position_panel(Path(symbol): Path<String>) -> impl IntoResponse {
    let sym = esc(&symbol);
    (StatusCode::OK, Html(format!(r#"<div class="text-muted small">You don’t own {sym} yet.</div>"#)))
}

pub async fn post_create_alert(Path(symbol): Path<String>) -> Response {
    let sym = esc(&symbol);
    let mut res = (StatusCode::OK, Html(format!(
        r#"<div class="text-warning small">Alerts for {sym} not implemented yet (stub).</div>"#
    ))).into_response();
    res.headers_mut().insert("HX-Trigger", HeaderValue::from_static("alertsUpdated"));
    res
}

// POST /alerts/:symbol/:id/delete
pub async fn post_delete_alert(Path((_symbol, _id)): Path<(String, String)>) -> Response {
    // No DB yet => always “empty list”
    let html = r#"<div class="text-muted small">No alerts yet.</div>"#.to_string();

    let mut res = (StatusCode::OK, Html(html)).into_response();
    res.headers_mut()
        .insert("HX-Trigger", HeaderValue::from_static("alertsUpdated"));
    res
}

// POST /trade/:symbol/buy
pub async fn post_trade_buy(Path(symbol): Path<String>) -> Response {
    let sym = esc(&symbol);
    let html = format!(
        r#"<div class="text-warning small">Paper trading for {sym} is not implemented yet (stub).</div>"#
    );

    let mut res = (StatusCode::OK, Html(html)).into_response();
    res.headers_mut()
        .insert("HX-Trigger", HeaderValue::from_static("positionUpdated"));
    res
}

// POST /trade/:symbol/sell
pub async fn post_trade_sell(Path(symbol): Path<String>) -> Response {
    let sym = esc(&symbol);
    let html = format!(
        r#"<div class="text-warning small">Paper trading for {sym} is not implemented yet (stub).</div>"#
    );

    let mut res = (StatusCode::OK, Html(html)).into_response();
    res.headers_mut()
        .insert("HX-Trigger", HeaderValue::from_static("positionUpdated"));
    res
}
