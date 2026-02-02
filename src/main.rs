use axum::{
    Router,
    routing::{get, post},
};
use mongodb::Client;
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tracing_subscriber;

mod auth;
mod config;
mod features;
mod finnhub;
mod handlers;
mod models;
mod pages;
mod render;
mod templates;
mod ws;
mod alert_monitor;
mod events;
mod db_init;

#[derive(Clone)]
pub struct AppState {
    pub hbs: templates::Hbs,
    pub db: mongodb::Database,
    pub settings: config::Settings,
    pub finnhub: finnhub::FinnhubClient,
    pub events_tx: tokio::sync::broadcast::Sender<String>,
}


#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let settings = config::load();

    let client = Client::with_uri_str(&settings.mongodb_uri)
        .await
        .expect("Failed to connect to MongoDB");
    let db = client.database(&settings.mongodb_db);
    db_init::ensure_indexes(&db)
        .await
        .expect("Failed to ensure MongoDB indexes");

    let finnhub = finnhub::FinnhubClient::new(settings.finnhub_api_key.clone());
    let (events_tx, _events_rx) = tokio::sync::broadcast::channel::<String>(256);
    let state = AppState {
        hbs: templates::build_handlebars(),
        db,
        settings: settings.clone(),
        finnhub,
        events_tx,
    };

    alert_monitor::spawn_price_alert_monitor(state.clone());

    use axum::middleware::from_fn_with_state;

    let app = Router::new()
        .route("/", get(handlers::home))
        .route("/health", get(handlers::health))
        .route("/health/db", get(handlers::health_db))
        .route(
            "/login",
            get(handlers::get_login).post(handlers::post_login),
        )
        .route(
            "/register",
            get(handlers::get_register).post(handlers::post_register),
        )
        .route("/logout", get(handlers::logout))
        .route("/me", get(handlers::me))
        .route("/search", get(handlers::get_search))
        .route("/search/results", get(handlers::get_search_results))
        .route("/details/:symbol", get(handlers::get_details))
        .route("/details/:symbol/quote", get(handlers::get_details_quote))
        .route("/positions/:symbol", get(features::get_position_panel))
        .route("/trade/:symbol/buy", post(features::post_trade_buy))
        .route("/trade/:symbol/sell", post(features::post_trade_sell))
        .route("/alerts/:symbol/list", get(features::get_alerts_list))
        .route("/alerts/:symbol", post(features::post_create_alert))
        .route(
            "/alerts/:symbol/:id/delete",
            post(features::post_delete_alert),
        )
        .route(
            "/alerts/by-id/:id/delete",
            post(features::post_delete_alert_global),
        )
        .route("/portfolio", get(pages::get_portfolio_page))
        .route("/portfolio/positions", get(pages::get_portfolio_positions))
        .route(
            "/portfolio/position/:symbol",
            get(pages::get_portfolio_position_card),
        )
        .route("/alerts", get(pages::get_alerts_page))
        .route("/alerts/list", get(pages::get_watchlist_alerts))
        .route("/funds", get(pages::get_funds_page).post(pages::post_funds))
        .route("/funds/modal", get(pages::get_funds_modal))
        .route("/cash", get(pages::get_cash_badge))
        .route("/ws/trades", get(ws::ws_trades))
        .route("/settings", get(handlers::get_settings))
        .route(
            "/settings/email",
            get(handlers::get_settings_email).post(handlers::post_settings_email),
        )
        .route(
            "/settings/password",
            get(handlers::get_settings_password).post(handlers::post_settings_password),
        )
        .route("/alerts/by-id/:id/trigger", post(features::post_trigger_alert))
        .route("/events", get(events::sse_events))
        .route("/portfolio/orders", get(pages::get_portfolio_orders))
        .nest_service("/static", ServeDir::new("static"))
        .fallback(handlers::not_found)
        .layer(from_fn_with_state(state.clone(), auth::require_auth))
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
