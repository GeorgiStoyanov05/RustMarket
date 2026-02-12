//! Library entrypoint for RustMarket.
//!
//! This file exists mainly to make controller tests easy (integration tests
//! under `tests/` can import the app state, routers, controllers, services).

pub mod config;
pub mod models;

// Keep these modules at crate root because the codebase already references
// them as `crate::auth`, `crate::render`, and `crate::templates`.
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
