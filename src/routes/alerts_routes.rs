use axum::{Router, routing::{get, post}};
use crate::{AppState, controllers::alerts_controller};

pub fn add_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/alerts", get(alerts_controller::get_alerts_page))
        .route("/alerts/list", get(alerts_controller::get_watchlist_alerts))
        .route("/alerts/:symbol/list", get(alerts_controller::get_alerts_list))
        .route("/alerts/:symbol", post(alerts_controller::post_create_alert))
        .route("/alerts/:symbol/:id/delete", post(alerts_controller::post_delete_alert))
        .route("/alerts/by-id/:id/delete", post(alerts_controller::post_delete_alert_global))
        .route("/alerts/by-id/:id/trigger", post(alerts_controller::post_trigger_alert))
}
