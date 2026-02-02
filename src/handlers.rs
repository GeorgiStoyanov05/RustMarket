use crate::models::{CurrentUser, User};
use axum::{Form, response::Redirect};
use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse},
};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use bcrypt::{DEFAULT_COST, hash, verify};
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use mongodb::bson::{doc};
use serde::Deserialize;
use serde_json::json;
use regex::Regex;
use axum::extract::{Path, Query, Extension};
use crate::AppState;


#[derive(Deserialize)]
pub struct ChangeEmailForm {
    pub email: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordForm {
    pub password: String,

    #[serde(default, rename = "rePassword")]
    pub re_password: Option<String>,
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub username: String,
    pub email: String,
    pub password: String,

    #[serde(default, rename = "rePassword")]
    pub re_password: Option<String>,

    #[serde(default, rename = "rememberMe")]
    pub remember_me: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
    #[serde(default, rename = "rememberMe")]
    pub remember_me: Option<String>,
}

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

fn is_valid_email(s: &str) -> bool {
    let s = s.trim();
    let re = Regex::new(r"^[a-zA-Z0-9.!#$%&'*+/=?^_{|}~-]+@[a-zA-Z0-9-]+(\.[a-zA-Z0-9-]+)+$")
        .unwrap();
    re.is_match(s)
}

fn make_jwt_with_days(state: &AppState, user_id: &mongodb::bson::oid::ObjectId, days: i64) -> String {
    let exp = (Utc::now() + Duration::days(days)).timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_hex(),
        exp,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.settings.jwt_secret.as_bytes()),
    )
    .expect("jwt encode failed")
}

fn auth_cookie(state: &AppState, token: String) -> Cookie<'static> {
    let mut cookie = Cookie::new(state.settings.jwt_cookie_name.clone(), token);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    if state.settings.cookie_secure {
        cookie.set_secure(true);
    }
    cookie
}

fn clear_auth_cookie(state: &AppState) -> Cookie<'static> {
    // Expire cookie
    let mut cookie = Cookie::new(state.settings.jwt_cookie_name.clone(), "");
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.make_removal();
    cookie
}

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn htmx_redirect(path: &'static str) -> axum::response::Response {
    let mut res = StatusCode::NO_CONTENT.into_response();
    res.headers_mut()
        .insert("HX-Redirect", HeaderValue::from_static(path));
    res
}

fn render_full(
    state: &AppState,
    title: &str,
    body_html: String,
    user: Option<&CurrentUser>,
) -> Result<String, String> {
    let (is_logged_in, user_json) = match user {
        Some(u) => (
            true,
            json!({
                "id": u.id.to_hex(),
                "email": u.email,
                "username": u.username,
            }),
        ),
        None => (false, serde_json::Value::Null),
    };

    let ctx = json!({
        "title": title,
        "body": body_html,
        "is_logged_in": is_logged_in,
        "user": user_json,
    });

    state
        .hbs
        .render("layouts/base", &ctx)
        .map_err(|e| e.to_string())
}

pub async fn home(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let body = state.hbs.render("pages/home", &json!({})).unwrap();

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(body)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);

    match render_full(&state, "GoMarket", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn not_found(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let body = state.hbs.render("pages/not_found", &json!({})).unwrap();

    if is_htmx(&headers) {
        return (StatusCode::NOT_FOUND, Html(body)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);

    match render_full(&state, "404", body, user_ref) {
        Ok(page) => (StatusCode::NOT_FOUND, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, Html("ok".to_string()))
}

pub async fn health_db(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.run_command(doc! { "ping": 1 }, None).await {
        Ok(_) => (StatusCode::OK, Html("mongo: ok".to_string())).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("mongo error: {}", e)),
        )
            .into_response(),
    }
}

pub async fn get_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> axum::response::Response {
    if user.is_some() {
        return Redirect::to("/").into_response();
    }

    let ctx = json!({
        "values": { "email": "", "rememberMe": false },
        "errors": {}
    });

    let body = match state.hbs.render("pages/login", &ctx) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("template error: {e}"))).into_response(),
    };

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(body)).into_response();
    }

    match render_full(&state, "Login", body, None) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn post_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> axum::response::Response {
    let email = form.email.trim().to_string();
    let password = form.password.to_string();
    let remember = form.remember_me.is_some();

    let mut errors = serde_json::Map::new();

    if email.is_empty() {
        errors.insert("email".into(), json!("Email is required."));
    } else if !is_valid_email(&email) {
        errors.insert("email".into(), json!("Please enter a valid email address."));
    }

    if password.len() < 6 {
        errors.insert("password".into(), json!("Password should be at least 6 characters long!"));
    }

    if !errors.is_empty() {
        let ctx = json!({
            "values": { "email": email, "rememberMe": remember },
            "errors": errors
        });

        let body = state.hbs.render("pages/login", &ctx).unwrap_or_else(|e| format!("template error: {e}"));

        if is_htmx(&headers) {
            return (StatusCode::OK, Html(body)).into_response();
        }

        let page = render_full(&state, "Login", body, None).unwrap_or_else(|e| e);
        return (StatusCode::OK, Html(page)).into_response();
    }

    let users = state.db.collection::<crate::models::User>("users");

    let user = match users.find_one(doc! { "email": &email }, None).await {
        Ok(Some(u)) => u,
        _ => {
            let ctx = json!({
                "values": { "email": email, "rememberMe": remember },
                "errors": { "_form": "Invalid email or password." }
            });
            let body = state.hbs.render("pages/login", &ctx).unwrap_or_else(|e| format!("template error: {e}"));

            if is_htmx(&headers) {
                return (StatusCode::OK, Html(body)).into_response();
            }

            let page = render_full(&state, "Login", body, None).unwrap_or_else(|e| e);
            return (StatusCode::OK, Html(page)).into_response();
        }
    };

    if !verify(&password, &user.password_hash).unwrap_or(false) {
        let ctx = json!({
            "values": { "email": email, "rememberMe": remember },
            "errors": { "_form": "Invalid email or password." }
        });
        let body = state.hbs.render("pages/login", &ctx).unwrap_or_else(|e| format!("template error: {e}"));

        if is_htmx(&headers) {
            return (StatusCode::OK, Html(body)).into_response();
        }

        let page = render_full(&state, "Login", body, None).unwrap_or_else(|e| e);
        return (StatusCode::OK, Html(page)).into_response();
    }

    let days = if remember { 30 } else { state.settings.jwt_ttl_days };
    let token = make_jwt_with_days(&state, &user.id, days);

    let jar = jar.add(auth_cookie(&state, token));

    if is_htmx(&headers) {
        let mut res = StatusCode::NO_CONTENT.into_response();
        res.headers_mut().insert("HX-Redirect", axum::http::HeaderValue::from_static("/"));
        return (jar, res).into_response();
    }

    (jar, Redirect::to("/")).into_response()
}

pub async fn get_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> axum::response::Response {
    if user.is_some() {
        return Redirect::to("/").into_response();
    }

    let ctx = json!({
        "values": { "username": "", "email": "", "rememberMe": false },
        "errors": {}
    });

    let body = match state.hbs.render("pages/register", &ctx) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("template error: {e}"))).into_response(),
    };

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(body)).into_response();
    }

    match render_full(&state, "Register", body, None) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn post_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Form(form): Form<RegisterForm>,
) -> axum::response::Response {
    let username = form.username.trim().to_string();
    let email = form.email.trim().to_string();
    let password = form.password.to_string();
    let re_password = form.re_password.clone().unwrap_or_default();
    let remember = form.remember_me.is_some();

    let mut errors = serde_json::Map::new();

    // username
    if username.len() < 3 {
        errors.insert("username".into(), json!("Username must be at least 3 characters long!"));
    }

    // email
    if email.is_empty() {
        errors.insert("email".into(), json!("Email is required."));
    } else if !is_valid_email(&email) {
        errors.insert("email".into(), json!("Please enter a valid email address."));
    }

    // password
    if password.len() < 6 {
        errors.insert("password".into(), json!("Password should be at least 6 characters long!"));
    }

    // IMPORTANT: rePassword is REQUIRED + must match
    if re_password.is_empty() {
        errors.insert("rePassword".into(), json!("Please repeat your password."));
    } else if re_password != password {
        errors.insert("rePassword".into(), json!("Passwords do not match."));
    }

    if !errors.is_empty() {
        let ctx = json!({
            "values": { "username": username, "email": email, "rememberMe": remember },
            "errors": errors
        });

        let body = state.hbs.render("pages/register", &ctx).unwrap_or_else(|e| format!("template error: {e}"));

        if is_htmx(&headers) {
            return (StatusCode::OK, Html(body)).into_response();
        }

        let page = render_full(&state, "Register", body, None).unwrap_or_else(|e| e);
        return (StatusCode::OK, Html(page)).into_response();
    }

    let users = state.db.collection::<crate::models::User>("users");

    // unique email
    if let Ok(Some(_)) = users.find_one(doc! { "email": &email }, None).await {
        let ctx = json!({
            "values": { "username": username, "email": email, "rememberMe": remember },
            "errors": { "email": "Email has already been taken!" }
        });

        let body = state.hbs.render("pages/register", &ctx).unwrap_or_else(|e| format!("template error: {e}"));
        if is_htmx(&headers) {
            return (StatusCode::OK, Html(body)).into_response();
        }
        let page = render_full(&state, "Register", body, None).unwrap_or_else(|e| e);
        return (StatusCode::OK, Html(page)).into_response();
    }

    // unique username
    if let Ok(Some(_)) = users.find_one(doc! { "username": &username }, None).await {
        let ctx = json!({
            "values": { "username": username, "email": email, "rememberMe": remember },
            "errors": { "username": "Username has already been taken!" }
        });

        let body = state.hbs.render("pages/register", &ctx).unwrap_or_else(|e| format!("template error: {e}"));
        if is_htmx(&headers) {
            return (StatusCode::OK, Html(body)).into_response();
        }
        let page = render_full(&state, "Register", body, None).unwrap_or_else(|e| e);
        return (StatusCode::OK, Html(page)).into_response();
    }

    let pw_hash = match hash(&password, DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => {
            let ctx = json!({
                "values": { "username": username, "email": email, "rememberMe": remember },
                "errors": { "_form": "There is a problem registering this user!" }
            });

            let body = state.hbs.render("pages/register", &ctx).unwrap_or_else(|e| format!("template error: {e}"));
            if is_htmx(&headers) {
                return (StatusCode::OK, Html(body)).into_response();
            }
            let page = render_full(&state, "Register", body, None).unwrap_or_else(|e| e);
            return (StatusCode::OK, Html(page)).into_response();
        }
    };

    let insert = match state.db.collection("users").insert_one(
        doc! {
            "email": &email,
            "username": &username,
            "password_hash": pw_hash,
        },
        None
    ).await {
        Ok(r) => r,
        Err(_) => {
            let ctx = json!({
                "values": { "username": username, "email": email, "rememberMe": remember },
                "errors": { "_form": "There is a problem registering this user!" }
            });

            let body = state.hbs.render("pages/register", &ctx).unwrap_or_else(|e| format!("template error: {e}"));
            if is_htmx(&headers) {
                return (StatusCode::OK, Html(body)).into_response();
            }
            let page = render_full(&state, "Register", body, None).unwrap_or_else(|e| e);
            return (StatusCode::OK, Html(page)).into_response();
        }
    };

    let new_id = insert.inserted_id.as_object_id().expect("inserted_id not ObjectId");

    let days = if remember { 30 } else { state.settings.jwt_ttl_days };
    let token = make_jwt_with_days(&state, &new_id, days);

    let jar = jar.add(auth_cookie(&state, token));

    if is_htmx(&headers) {
        let mut res = StatusCode::NO_CONTENT.into_response();
        res.headers_mut().insert("HX-Redirect", axum::http::HeaderValue::from_static("/"));
        return (jar, res).into_response();
    }

    (jar, Redirect::to("/")).into_response()
}


pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let jar = jar.add(clear_auth_cookie(&state));
    (jar, Redirect::to("/")).into_response()
}


pub async fn me(user: Option<Extension<CurrentUser>>) -> impl IntoResponse {
    match user {
        Some(Extension(u)) => (StatusCode::OK, axum::Json(u)).into_response(),
        None => (StatusCode::UNAUTHORIZED, Html("not logged in".to_string())).into_response(),
    }
}

pub async fn get_search(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> axum::response::Response {
    let body = match state.hbs.render("pages/search", &json!({})) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("template error: {e}"))).into_response(),
    };

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(body)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);
    match render_full(&state, "Search", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn get_search_results(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> axum::response::Response {
    let q = query.q.unwrap_or_default().trim().to_string();

    // GoMarket behavior: empty query => show "Start typing..."
    if q.is_empty() {
        let html = state
            .hbs
            .render(
                "partials/search_results",
                &json!({
                    "query": "",
                    "results": serde_json::Value::Null,
                    "error": serde_json::Value::Null
                }),
            )
            .unwrap_or_else(|e| format!("template error: {e}"));
        return (StatusCode::OK, Html(html)).into_response();
    }

    let data = match state.finnhub.search(&q).await {
        Ok(resp) => {
            // GoMarket behavior: clean empty symbols + limit 10
            let mut results: Vec<_> = resp
                .result
                .into_iter()
                .filter(|it| !it.symbol.trim().is_empty())
                .take(10)
                .map(|it| {
                    json!({
                        "symbol": it.symbol,
                        "display_symbol": it.display_symbol,
                        "description": it.description,
                        "type": it.kind
                    })
                })
                .collect();

            // If nothing found, send results=null so template hits "No results"
            let results_val = if results.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::Array(results)
            };

            json!({
                "query": q,
                "results": results_val,
                "error": serde_json::Value::Null
            })
        }
        Err(_err) => {
            // GoMarket: generic message (don’t expose raw Finnhub errors)
            json!({
                "query": q,
                "results": serde_json::Value::Null,
                "error": "Search unavailable right now."
            })
        }
    };

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
    let body = match state.hbs.render("pages/details", &json!({ "symbol": symbol })) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("template error: {e}"))).into_response(),
    };

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(body)).into_response();
    }

    let user_ref = user.as_ref().map(|Extension(u)| u);
    match render_full(&state, "Details", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

pub async fn get_details_quote(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> axum::response::Response {
    let data = match state.finnhub.quote(&symbol).await {
        Ok(q) => json!({ "quote": q, "error": serde_json::Value::Null }),
        Err(err) => json!({ "quote": serde_json::Value::Null, "error": err }),
    };

    let html = state.hbs.render("partials/quote", &data)
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(html)).into_response()
}

// ---------------- Settings ----------------

pub async fn get_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> axum::response::Response {
    let body = match state.hbs.render("pages/settings", &json!({})) {
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

    match render_full(&state, "Settings", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}


pub async fn get_settings_email(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> axum::response::Response {
    let current_email = user
        .as_ref()
        .map(|Extension(u)| u.email.as_str())
        .unwrap_or("");

    let partial = state
        .hbs
        .render(
            "partials/change_email",
            &json!({
                "values": { "email": current_email },
                "errors": {},
                "succ": ""
            }),
        )
        .unwrap_or_else(|e| format!("template error: {e}"));

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(partial)).into_response();
    }

    let shell = state
        .hbs
        .render("pages/settings", &json!({}))
        .unwrap_or_else(|e| format!("template error: {e}"));

    let autoload = r##"<div hx-get="/settings/email" hx-trigger="load" hx-target="#rightPane" hx-swap="innerHTML"></div>"##;
    let body = format!("{}{}", shell, autoload);

    let user_ref = user.as_ref().map(|Extension(u)| u);

    match render_full(&state, "Settings", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}


pub async fn post_settings_email(
    State(state): State<AppState>,
    _headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<ChangeEmailForm>,
) -> axum::response::Response {
    let new_email = form.email.trim().to_string();
    let mut errors = serde_json::Map::new();

    let Some(Extension(u)) = user else {
        errors.insert("_form".into(), json!("There was an error getting user"));

        let partial = state
            .hbs
            .render(
                "partials/change_email",
                &json!({
                    "values": { "email": new_email },
                    "errors": errors,
                    "succ": ""
                }),
            )
            .unwrap_or_else(|e| format!("template error: {e}"));

        return (StatusCode::OK, Html(partial)).into_response();
    };

    // validate email
    if new_email.is_empty() {
        errors.insert("email".into(), json!("Email is required."));
    } else {
        let re = Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap();
        if !re.is_match(&new_email) {
            errors.insert("email".into(), json!("Please enter a valid email address."));
        }
    }

    // NEW: must differ from current email
    if errors.is_empty() && new_email.eq_ignore_ascii_case(&u.email) {
        errors.insert("email".into(), json!("New email must be different from your current email."));
    }

    let users = state.db.collection::<User>("users");

    // uniqueness check (fast path)
    if errors.is_empty() {
        if let Ok(Some(_)) = users.find_one(doc! { "email": &new_email }, None).await {
            errors.insert("email".into(), json!("This email is already in use."));
        }
    }

    // update
    if errors.is_empty() {
        if let Err(e) = users
            .update_one(doc! { "_id": u.id }, doc! { "$set": { "email": &new_email } }, None)
            .await
        {
            // NEW: handle duplicate-key gracefully (because now we have an index)
            let msg = e.to_string();
            if msg.contains("E11000") {
                errors.insert("email".into(), json!("This email is already in use."));
            } else {
                errors.insert("_form".into(), json!(format!("db error: {e}")));
            }
        }
    }

    let succ = if errors.is_empty() {
        "You have changed your email successfully!"
    } else {
        ""
    };

    let partial = state
        .hbs
        .render(
            "partials/change_email",
            &json!({
                "values": { "email": if succ.is_empty() { new_email } else { String::new() } },
                "errors": errors,
                "succ": succ
            }),
        )
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(partial)).into_response()
}



pub async fn get_settings_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
) -> axum::response::Response {
    let partial = state
        .hbs
        .render(
            "partials/change_password",
            &json!({
                "errors": {},
                "succ": ""
            }),
        )
        .unwrap_or_else(|e| format!("template error: {e}"));

    if is_htmx(&headers) {
        return (StatusCode::OK, Html(partial)).into_response();
    }

    let shell = state
        .hbs
        .render("pages/settings", &json!({}))
        .unwrap_or_else(|e| format!("template error: {e}"));

    let autoload = r##"<div hx-get="/settings/password" hx-trigger="load" hx-target="#rightPane" hx-swap="innerHTML"></div>"##;
    let body = format!("{}{}", shell, autoload);

    let user_ref = user.as_ref().map(|Extension(u)| u);

    match render_full(&state, "Settings", body, user_ref) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}


pub async fn post_settings_password(
    State(state): State<AppState>,
    _headers: HeaderMap,
    user: Option<Extension<CurrentUser>>,
    Form(form): Form<ChangePasswordForm>,
) -> axum::response::Response {
    let mut errors = serde_json::Map::new();

    let Some(Extension(u)) = user else {
        errors.insert("_form".into(), json!("There was an error getting user"));

        let partial = state
            .hbs
            .render(
                "partials/change_password",
                &json!({
                    "errors": errors,
                    "succ": ""
                }),
            )
            .unwrap_or_else(|e| format!("template error: {e}"));

        return (StatusCode::OK, Html(partial)).into_response();
    };

    let password = form.password.trim().to_string();
    let re_password = form.re_password.as_deref().unwrap_or("").trim().to_string();

    if password.is_empty() {
        errors.insert("password".into(), json!("Password is required."));
    }
    if re_password.is_empty() {
        errors.insert("rePassword".into(), json!("Repeat password is required."));
    }
    if errors.is_empty() && password != re_password {
        errors.insert("rePassword".into(), json!("Passwords do not match."));
    }
    if errors.is_empty() && password.len() < 6 {
        errors.insert("password".into(), json!("Password must be at least 6 characters."));
    }

    let users = state.db.collection::<User>("users");

    // NEW: cannot be same as current password
    if errors.is_empty() {
        let db_user = match users.find_one(doc! { "_id": u.id }, None).await {
            Ok(Some(x)) => x,
            _ => {
                errors.insert("_form".into(), json!("User not found."));
                let partial = state
                    .hbs
                    .render(
                        "partials/change_password",
                        &json!({ "errors": errors, "succ": "" }),
                    )
                    .unwrap_or_else(|e| format!("template error: {e}"));
                return (StatusCode::OK, Html(partial)).into_response();
            }
        };

        if verify(&password, &db_user.password_hash).unwrap_or(false) {
            errors.insert(
                "password".into(),
                json!("New password must be different from your current password."),
            );
        }
    }

    // update password_hash
    if errors.is_empty() {
        let pw_hash = match hash(&password, DEFAULT_COST) {
            Ok(h) => h,
            Err(_) => {
                errors.insert("_form".into(), json!("Failed to hash password."));
                String::new()
            }
        };

        if errors.is_empty() {
            if let Err(e) = users
                .update_one(
                    doc! { "_id": u.id },
                    doc! { "$set": { "password_hash": pw_hash } },
                    None,
                )
                .await
            {
                errors.insert("_form".into(), json!(format!("db error: {e}")));
            }
        }
    }

    let partial = state
        .hbs
        .render(
            "partials/change_password",
            &json!({
                "errors": errors,
                "succ": if errors.is_empty() { "You have changed your password successfully!" } else { "" }
            }),
        )
        .unwrap_or_else(|e| format!("template error: {e}"));

    (StatusCode::OK, Html(partial)).into_response()
}
