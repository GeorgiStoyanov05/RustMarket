use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tracing_subscriber;

use mongodb::Client;

mod auth;
mod config;
mod handlers;
mod models;
mod templates;



#[derive(Clone)]
pub struct AppState {
    pub hbs: templates::Hbs,
    pub db: mongodb::Database,
    pub settings: config::Settings,
}



#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let settings = config::load();

    // Mongo connection
    let client = Client::with_uri_str(&settings.mongodb_uri)
        .await
        .expect("Failed to connect to MongoDB");
    let db = client.database(&settings.mongodb_db);
    use axum::middleware::from_fn_with_state;

    let state = AppState {
        hbs: templates::build_handlebars(),
        db,
        settings: settings.clone(),
    };

    let app = Router::new()
        .route("/", get(handlers::home))
        .route("/health", get(handlers::health))
        .route("/health/db", get(handlers::health_db))
        .nest_service("/static", ServeDir::new("static"))
        .fallback(handlers::not_found)
        // middleware that injects User into request extensions if logged in
        .layer(from_fn_with_state(state.clone(), auth::inject_current_user))
        .with_state(state);

    let addr = SocketAddr::from((settings.host.parse::<std::net::IpAddr>().unwrap(), settings.port));
    tracing::info!("listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
