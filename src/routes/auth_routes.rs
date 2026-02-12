use axum::{Router, routing::get};
use crate::{AppState, controllers::auth_controller};

pub fn add_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/login", get(auth_controller::get_login).post(auth_controller::post_login))
        .route("/register", get(auth_controller::get_register).post(auth_controller::post_register))
        .route("/logout", get(auth_controller::logout))
}
