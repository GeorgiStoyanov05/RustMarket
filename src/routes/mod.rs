use axum::Router;
use axum::middleware::from_fn_with_state;
use tower_http::services::ServeDir;

use crate::{AppState, controllers::home_controller};

pub mod home_routes;
pub mod auth_routes;
pub mod user_routes;
pub mod stocks_routes;
pub mod trading_routes;
pub mod portfolio_routes;
pub mod alerts_routes;
pub mod realtime_routes;

pub fn app(state: AppState) -> Router {
    let router = Router::<AppState>::new();

    let router = home_routes::add_routes(router);
    let router = auth_routes::add_routes(router);
    let router = user_routes::add_routes(router);
    let router = stocks_routes::add_routes(router);
    let router = trading_routes::add_routes(router);
    let router = portfolio_routes::add_routes(router);
    let router = alerts_routes::add_routes(router);
    let router = realtime_routes::add_routes(router);

    router
        .nest_service("/static", ServeDir::new("static"))
        .fallback(home_controller::not_found)
        .layer(from_fn_with_state(state.clone(), crate::auth::require_auth))
        .layer(from_fn_with_state(state.clone(), crate::auth::inject_current_user))
        .with_state(state)
}
