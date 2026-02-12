use axum::{Router, routing::get};
use crate::{AppState, controllers::user_controller};

pub fn add_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/me", get(user_controller::me))
        .route("/settings", get(user_controller::get_settings))
        .route(
            "/settings/email",
            get(user_controller::get_settings_email).post(user_controller::post_settings_email),
        )
        .route(
            "/settings/password",
            get(user_controller::get_settings_password).post(user_controller::post_settings_password),
        )
        .route("/funds", get(user_controller::get_funds_page).post(user_controller::post_funds))
        .route("/funds/modal", get(user_controller::get_funds_modal))
        .route("/cash", get(user_controller::get_cash_badge))
}
