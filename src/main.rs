use mongodb::Client;
use std::net::SocketAddr;
use tracing_subscriber;

use rustmarket::{config, routes, services, templates, AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let settings = config::load();

    let client = Client::with_uri_str(&settings.mongodb_uri)
        .await
        .expect("Failed to connect to MongoDB");
    let db = client.database(&settings.mongodb_db);

    services::db_init::ensure_indexes(&db)
        .await
        .expect("Failed to ensure MongoDB indexes");

    let finnhub = services::finnhub::FinnhubClient::new(settings.finnhub_api_key.clone());
    let (events_tx, _events_rx) = tokio::sync::broadcast::channel::<String>(256);

    let state = AppState {
        hbs: templates::build_handlebars(),
        db,
        settings: settings.clone(),
        finnhub,
        events_tx,
    };

    // Background alert monitoring
    services::alert_monitor::spawn_price_alert_monitor(state.clone());

    // Build router from feature routers
    let app = routes::app(state);

    let addr = SocketAddr::from((
        settings.host.parse::<std::net::IpAddr>().unwrap(),
        settings.port,
    ));
    tracing::info!("listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
