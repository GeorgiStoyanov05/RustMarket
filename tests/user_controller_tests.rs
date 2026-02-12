use axum::{
    http::{header, Request, StatusCode},
    routing::post,
    Router,
};
use http_body_util::BodyExt;
use mongodb::{bson::oid::ObjectId, Client};
use rustmarket::{controllers::user_controller, config, services, templates, AppState};
use rustmarket::models::CurrentUser;
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
async fn post_funds_unauthorized_renders_error_banner() {
    let state = test_state().await;
    let app = Router::new()
        .route("/funds", post(user_controller::post_funds))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/funds")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("amount=10"))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.to_lowercase().contains("error getting user"));
}

#[tokio::test]
async fn post_funds_invalid_amount_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/funds", post(user_controller::post_funds))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/funds")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("amount=notanumber"))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.to_lowercase().contains("error with the amount"));
}

#[tokio::test]
async fn post_settings_email_same_as_current_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/settings/email", post(user_controller::post_settings_email))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/settings/email")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("email=test%40example.com"))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.to_lowercase().contains("must be different"));
}

#[tokio::test]
async fn post_funds_zero_amount_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/funds", post(user_controller::post_funds))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/funds")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("amount=0"))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await.to_lowercase();
    assert!(body.contains("bigger than zero"));
}

#[tokio::test]
async fn post_funds_negative_amount_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/funds", post(user_controller::post_funds))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/funds")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("amount=-10"))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await.to_lowercase();
    assert!(body.contains("bigger than zero"));
}

#[tokio::test]
async fn post_settings_email_unauthorized_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/settings/email", post(user_controller::post_settings_email))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/settings/email")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("email=test%40example.com"))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await.to_lowercase();
    assert!(body.contains("error getting user"));
}

#[tokio::test]
async fn post_settings_email_invalid_format_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/settings/email", post(user_controller::post_settings_email))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/settings/email")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("email=not-an-email"))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "old@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await.to_lowercase();
    assert!(body.contains("valid email"));
}

#[tokio::test]
async fn post_settings_email_empty_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/settings/email", post(user_controller::post_settings_email))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/settings/email")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("email="))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "old@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Email is required."));
}

#[tokio::test]
async fn post_settings_password_mismatch_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/settings/password", post(user_controller::post_settings_password))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/settings/password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("password=123456&rePassword=654321"))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Passwords do not match."));
}
