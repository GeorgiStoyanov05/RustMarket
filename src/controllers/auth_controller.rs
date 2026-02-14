use axum::{
    extract::{State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Form,
};
use axum_extra::extract::cookie::CookieJar;
use regex::Regex;
use serde::Deserialize;
use serde_json::json;

use crate::{render, services::auth_service, AppState};

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn htmx_redirect(path: &str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert("HX-Redirect", path.parse().unwrap());
    (StatusCode::OK, headers, Html("".to_string())).into_response()
}

fn is_valid_email(email: &str) -> bool {
    let re = Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap();
    re.is_match(email)
}

// ---------------- LOGIN ----------------

pub async fn get_login(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let body = match state.hbs.render("pages/login", &json!({})) {
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

    match render::render_full(&state, "Login", body, None) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
}

pub async fn post_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Response {
    let email = form.email.trim().to_string();
    let password = form.password.trim().to_string();

    let mut errors = serde_json::Map::new();

    if email.is_empty() {
        errors.insert("email".into(), json!("Email is required."));
    } else if !is_valid_email(&email) {
        errors.insert("email".into(), json!("Invalid email."));
    }

    if password.is_empty() {
        errors.insert("password".into(), json!("Password is required."));
    }

    if !errors.is_empty() {
        let html = state
            .hbs
            .render(
                "pages/login",
                &json!({
                    "values": {"email": email, "password": password},
                    "errors": errors
                }),
            )
            .unwrap_or_else(|e| format!("template error: {e}"));
        return (StatusCode::OK, Html(html)).into_response();
    }

    let user = match auth_service::login_user(&state, &email, &password).await {
        Ok(u) => u,
        Err(errs) => {
            for (k, v) in errs {
                errors.insert(k, json!(v));
            }

            let html = state
                .hbs
                .render(
                    "pages/login",
                    &json!({
                        "values": {"email": email, "password": password},
                        "errors": errors
                    }),
                )
                .unwrap_or_else(|e| format!("template error: {e}"));
            return (StatusCode::OK, Html(html)).into_response();
        }
    };

    let token = match auth_service::make_jwt_with_days(&state, &user.id, 7) {
        Ok(t) => t,
        Err(e) => {
            errors.insert("_form".into(), json!(format!("Auth error: {e}")));
            let html = state
                .hbs
                .render(
                    "pages/login",
                    &json!({
                        "values": {"email": email, "password": password},
                        "errors": errors
                    }),
                )
                .unwrap_or_else(|e| format!("template error: {e}"));
            return (StatusCode::OK, Html(html)).into_response();
        }
    };

    let jar = jar.add(auth_service::auth_cookie(&state, token));

    if is_htmx(&headers) {
        return (jar, htmx_redirect("/")).into_response();
    }

    (
        jar,
        (StatusCode::SEE_OTHER, [("Location", "/")], Html("".to_string())),
    )
        .into_response()
}

// ---------------- REGISTER ----------------

pub async fn get_register(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let body = match state.hbs.render("pages/register", &json!({})) {
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

    match render::render_full(&state, "Register", body, None) {
        Ok(page) => (StatusCode::OK, Html(page)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e)).into_response(),
    }
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub username: String,
    pub email: String,
    pub password: String,

    #[serde(default, rename = "rePassword")]
    pub re_password: Option<String>,
}

pub async fn post_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Form(form): Form<RegisterForm>,
) -> Response {
    let username = form.username.trim().to_string();
    let email = form.email.trim().to_string();
    let password = form.password.trim().to_string();
    let re_password = form.re_password.as_deref().unwrap_or("").trim().to_string();

    let mut errors = serde_json::Map::new();

    if username.is_empty() {
        errors.insert("username".into(), json!("Username is required."));
    } else if username.len() < 2 {
        errors.insert("username".into(), json!("Username must be at least 2 characters."));
    }

    if email.is_empty() {
        errors.insert("email".into(), json!("Email is required."));
    } else if !is_valid_email(&email) {
        errors.insert("email".into(), json!("Invalid email."));
    }

    if password.is_empty() {
        errors.insert("password".into(), json!("Password is required."));
    } else if password.len() < 6 {
        errors.insert("password".into(), json!("Password must be at least 6 characters."));
    }

    if re_password.is_empty() {
        errors.insert("rePassword".into(), json!("Repeat password is required."));
    } else if password != re_password {
        errors.insert("rePassword".into(), json!("Passwords do not match."));
    }

    if !errors.is_empty() {
        let html = state
            .hbs
            .render(
                "pages/register",
                &json!({
                    "values": {"username": username, "email": email, "password": password, "rePassword": re_password},
                    "errors": errors
                }),
            )
            .unwrap_or_else(|e| format!("template error: {e}"));
        return (StatusCode::OK, Html(html)).into_response();
    }

    let user_id = match auth_service::register_user(&state, &username, &email, &password).await {
        Ok(id) => id,
        Err(errs) => {
            for (k, v) in errs {
                errors.insert(k, json!(v));
            }

            let html = state
                .hbs
                .render(
                    "pages/register",
                    &json!({
                        "values": {"username": username, "email": email, "password": password, "rePassword": re_password},
                        "errors": errors
                    }),
                )
                .unwrap_or_else(|e| format!("template error: {e}"));
            return (StatusCode::OK, Html(html)).into_response();
        }
    };

    let token = match auth_service::make_jwt_with_days(&state, &user_id, 7) {
        Ok(t) => t,
        Err(e) => {
            errors.insert("_form".into(), json!(format!("Auth error: {e}")));
            let html = state
                .hbs
                .render(
                    "pages/register",
                    &json!({
                        "values": {"username": username, "email": email, "password": password, "rePassword": re_password},
                        "errors": errors
                    }),
                )
                .unwrap_or_else(|e| format!("template error: {e}"));
            return (StatusCode::OK, Html(html)).into_response();
        }
    };

    let jar = jar.add(auth_service::auth_cookie(&state, token));

    if is_htmx(&headers) {
        return (jar, htmx_redirect("/")).into_response();
    }

    (
        jar,
        (StatusCode::SEE_OTHER, [("Location", "/")], Html("".to_string())),
    )
        .into_response()
}

// ---------------- LOGOUT ----------------

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let jar = jar.add(auth_service::clear_auth_cookie(&state));
    (jar, (StatusCode::SEE_OTHER, [("Location", "/")]))
}
