use axum::{
    http::{header, Request, StatusCode},
    routing::post,
    Router,
};
use http_body_util::BodyExt;
use mongodb::Client;
use rustmarket::{controllers::auth_controller, config, services, templates, AppState};
use tower::ServiceExt;

async fn test_state() -> AppState {
    let mut settings = config::load();
    settings.finnhub_api_key = String::new();

    let client = Client::with_uri_str(&settings.mongodb_uri)
        .await
        .expect("mongodb client");
    let db = client.database(&settings.mongodb_db);

    let finnhub = services::finnhub::FinnhubClient::new(settings.finnhub_api_key.clone());
    let (events_tx, _events_rx) = tokio::sync::broadcast::channel::<String>(16);

    AppState {
        hbs: templates::build_handlebars(),
        db,
        settings,
        finnhub,
        events_tx,
    }
}

async fn response_body_string(res: axum::response::Response) -> String {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8_lossy(&bytes).to_string()
}

#[tokio::test]
async fn post_login_missing_fields_renders_errors() {
    let state = test_state().await;
    let app = Router::new()
        .route("/login", post(auth_controller::post_login))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("email=&password="))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Email is required."));
    assert!(body.contains("Password is required."));
}

#[tokio::test]
async fn post_login_invalid_email_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/login", post(auth_controller::post_login))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("email=not-an-email&password=123456"))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Invalid email."));
}

#[tokio::test]
async fn post_register_password_mismatch_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/register", post(auth_controller::post_register))
        .with_state(state);

    // rePassword mismatch
    let req = Request::builder()
        .method("POST")
        .uri("/register")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from(
            "username=TestUser&email=test%40example.com&password=123456&rePassword=654321",
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Passwords do not match."));
}

#[tokio::test]
async fn post_login_missing_password_only_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/login", post(auth_controller::post_login))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("email=test%40example.com&password="))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Password is required."));
    assert!(!body.contains("Invalid email."));
}

#[tokio::test]
async fn post_login_missing_email_only_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/login", post(auth_controller::post_login))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("email=&password=123456"))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Email is required."));
    assert!(!body.contains("Password is required."));
}

#[tokio::test]
async fn post_register_missing_username_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/register", post(auth_controller::post_register))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/register")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from(
            "username=&email=test%40example.com&password=123456&rePassword=123456",
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Username is required."));
}

#[tokio::test]
async fn post_register_short_username_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/register", post(auth_controller::post_register))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/register")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from(
            "username=a&email=test%40example.com&password=123456&rePassword=123456",
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("at least 2 characters"));
}

#[tokio::test]
async fn post_register_short_password_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/register", post(auth_controller::post_register))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/register")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from(
            "username=TestUser&email=test%40example.com&password=123&rePassword=123",
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("at least 6 characters"));
}

#[tokio::test]
async fn post_register_missing_repeat_password_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/register", post(auth_controller::post_register))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/register")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from(
            "username=TestUser&email=test%40example.com&password=123456&rePassword=",
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Repeat password is required."));
}
