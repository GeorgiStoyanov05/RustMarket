use axum::{
    http::{header, Request, StatusCode},
    routing::post,
    Router,
};
use http_body_util::BodyExt;
use mongodb::{bson::oid::ObjectId, Client};
use rustmarket::{controllers::trading_controller, config, services, templates, AppState};
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
async fn post_trade_buy_unauthorized_returns_401() {
    let state = test_state().await;
    let app = Router::new()
        .route("/trade/:symbol/buy", post(trading_controller::post_trade_buy))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/trade/AAPL/buy")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("qty=1"))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let body = response_body_string(res).await;
    assert!(body.to_lowercase().contains("unauthorized"));
}

#[tokio::test]
async fn post_trade_buy_invalid_qty_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/trade/:symbol/buy", post(trading_controller::post_trade_buy))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/trade/AAPL/buy")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("qty=notanumber"))
        .unwrap();

    // Add authenticated user (so we hit the qty parse branch, not unauthorized).
    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Enter a valid quantity"));
}

#[tokio::test]
async fn post_trade_sell_unauthorized_returns_401() {
    let state = test_state().await;
    let app = Router::new()
        .route("/trade/:symbol/sell", post(trading_controller::post_trade_sell))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/trade/AAPL/sell")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("qty=1"))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let body = response_body_string(res).await;
    assert!(body.to_lowercase().contains("unauthorized"));
}

#[tokio::test]
async fn post_trade_sell_invalid_qty_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/trade/:symbol/sell", post(trading_controller::post_trade_sell))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/trade/AAPL/sell")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("qty=abc"))
        .unwrap();

    // Add authenticated user (so we hit qty parse branch, not unauthorized)
    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Enter a valid quantity"));
}

#[tokio::test]
async fn post_trade_buy_zero_qty_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/trade/:symbol/buy", post(trading_controller::post_trade_buy))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/trade/AAPL/buy")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("qty=0"))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Enter a valid quantity"));
}

#[tokio::test]
async fn post_trade_buy_negative_qty_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/trade/:symbol/buy", post(trading_controller::post_trade_buy))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/trade/AAPL/buy")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("qty=-5"))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Enter a valid quantity"));
}

#[tokio::test]
async fn post_trade_sell_zero_qty_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/trade/:symbol/sell", post(trading_controller::post_trade_sell))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/trade/AAPL/sell")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("qty=0"))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Enter a valid quantity"));
}

#[tokio::test]
async fn post_trade_sell_negative_qty_renders_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/trade/:symbol/sell", post(trading_controller::post_trade_sell))
        .with_state(state);

    let mut req = Request::builder()
        .method("POST")
        .uri("/trade/AAPL/sell")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("qty=-1"))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Enter a valid quantity"));
}

#[tokio::test]
async fn post_trade_buy_missing_symbol_returns_generic_error() {
    let state = test_state().await;
    let app = Router::new()
        .route("/trade/:symbol/buy", post(trading_controller::post_trade_buy))
        .with_state(state);

    // Symbol is whitespace ("%20"), which the service treats as missing.
    let mut req = Request::builder()
        .method("POST")
        .uri("/trade/%20/buy")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from("qty=1"))
        .unwrap();

    req.extensions_mut().insert(CurrentUser {
        id: ObjectId::new(),
        email: "test@example.com".to_string(),
        username: "test".to_string(),
    });

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = response_body_string(res).await;
    assert!(body.contains("Could not buy"));
}
