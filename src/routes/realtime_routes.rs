use axum::{Router, routing::get};
use crate::{AppState, controllers::realtime_controller};

pub fn add_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/ws/trades", get(realtime_controller::ws_trades))
        .route("/ws/trades_multi", get(realtime_controller::ws_trades_multi))
        .route("/events", get(realtime_controller::sse_events))
}
