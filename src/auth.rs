use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, Request, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use mongodb::bson::{doc, oid::ObjectId};
use serde::{Deserialize, Serialize};

use crate::{models::{User, CurrentUser}, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    // user id as hex string
    pub sub: String,
    // expiry (unix timestamp seconds)
    pub exp: usize,
}

fn get_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;

    for part in raw.split(';') {
        let part = part.trim();
        let mut it = part.splitn(2, '=');
        let k = it.next()?.trim();
        let v = it.next()?.trim();
        if k == name {
            return Some(v.to_string());
        }
    }
    None
}

pub async fn inject_current_user(
    State(state): State<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let cookie_name = state.settings.jwt_cookie_name.as_str();

    if let Some(token) = get_cookie(req.headers(), cookie_name) {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        let decoded = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(state.settings.jwt_secret.as_bytes()),
            &validation,
        );

        if let Ok(data) = decoded {
            if let Ok(user_id) = ObjectId::parse_str(&data.claims.sub) {
                let users = state.db.collection::<User>("users");

                if let Ok(Some(user)) = users.find_one(doc! { "_id": user_id }, None).await {
                    // Store user in request extensions so handlers can access it
                    req.extensions_mut().insert(CurrentUser::from(user));
                }
            }
        }
    }

    next.run(req).await
}
fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn is_websocket(headers: &HeaderMap) -> bool {
    headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
}

fn is_public_path(path: &str) -> bool {
    path == "/"
        || path == "/login"
        || path == "/register"
        || path == "/logout"
        || path == "/favicon.ico"
        || path.starts_with("/static/")
}

pub async fn require_auth(
    State(_state): State<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let path = req.uri().path();

    // Allow only home/login/register/static (and logout)
    if is_public_path(path) {
        return next.run(req).await;
    }

    // If inject_current_user already put CurrentUser in extensions => authenticated
    if req.extensions().get::<CurrentUser>().is_some() {
        return next.run(req).await;
    }

    // Not logged in:
    // - HTMX: force full redirect to /login
    // - Normal: 302 redirect to /login
    // - WebSocket: 401
    if is_websocket(req.headers()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    if is_htmx(req.headers()) {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Redirect", HeaderValue::from_static("/login"));
        return (StatusCode::OK, headers, Html("".to_string())).into_response();
    }

    Redirect::to("/login").into_response()
}
