use axum::{Router, routing::get};
use crate::{AppState, controllers::home_controller};

pub fn add_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/", get(home_controller::home))
        .route("/health", get(home_controller::health))
        .route("/health/db", get(home_controller::health_db))
}
