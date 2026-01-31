use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tracing_subscriber;

use mongodb::Client;

mod auth;
mod config;
mod handlers;
mod models;
mod templates;
mod finnhub;
mod ws;
mod stubs;

#[derive(Clone)]
pub struct AppState {
    pub hbs: templates::Hbs,
    pub db: mongodb::Database,
    pub settings: config::Settings,
    pub finnhub: finnhub::FinnhubClient,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();
    let settings = config::load();

    // Mongo connection
    let client = Client::with_uri_str(&settings.mongodb_uri)
        .await
        .expect("Failed to connect to MongoDB");
    let db = client.database(&settings.mongodb_db);

    use axum::middleware::from_fn_with_state;

    let finnhub = finnhub::FinnhubClient::new(settings.finnhub_api_key.clone());

    let state = AppState {
        hbs: templates::build_handlebars(),
        db,
        settings: settings.clone(),
        finnhub,
    };

    let app = Router::new()
        .route("/", get(handlers::home))
        .route("/health", get(handlers::health))
        .route("/health/db", get(handlers::health_db))
        .route("/login", get(handlers::get_login).post(handlers::post_login))
        .route("/register", get(handlers::get_register).post(handlers::post_register))
        .route("/logout", get(handlers::logout))
        .route("/me", get(handlers::me))
        .route("/search", get(handlers::get_search))
        .route("/search/results", get(handlers::get_search_results))
        .route("/details/:symbol", get(handlers::get_details))
        .route("/details/:symbol/quote", get(handlers::get_details_quote))
        // Real-time trades WS
        .route("/ws/trades", get(ws::ws_trades))
        // ---- STUBS to stop HTMX 404 spam (details sidebar) ----
        .route("/alerts/:symbol/list", get(stubs::get_alerts_list))
        .route("/alerts/:symbol", post(stubs::post_create_alert))
        .route("/alerts/:symbol/:id/delete", post(stubs::post_delete_alert))
        .route("/positions/:symbol", get(stubs::get_position_panel))
        .route("/trade/:symbol/buy", post(stubs::post_trade_buy))
        .route("/trade/:symbol/sell", post(stubs::post_trade_sell))
        // Static
        .nest_service("/static", ServeDir::new("static"))
        .fallback(handlers::not_found)
        // middleware that injects User into request extensions if logged in
        .layer(from_fn_with_state(state.clone(), auth::inject_current_user))
        .with_state(state);

    let addr = SocketAddr::from((
        settings.host.parse::<std::net::IpAddr>().unwrap(),
        settings.port,
    ));
    tracing::info!("listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
