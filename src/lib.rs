pub mod config;
pub mod models;
#[path = "middleware/auth.rs"]
pub mod auth;

pub mod services;

#[path = "views/render.rs"]
pub mod render;
#[path = "views/templates.rs"]
pub mod templates;

pub mod controllers;
pub mod routes;

#[derive(Clone)]
pub struct AppState {
    pub hbs: templates::Hbs,
    pub db: mongodb::Database,
    pub settings: config::Settings,
    pub finnhub: services::finnhub::FinnhubClient,
    pub events_tx: tokio::sync::broadcast::Sender<String>,
}
