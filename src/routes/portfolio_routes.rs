use axum::{Router, routing::get};

use crate::{AppState, controllers::portfolio_controller};

pub fn add_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/portfolio", get(portfolio_controller::get_portfolio_page))
        .route("/portfolio/positions", get(portfolio_controller::get_portfolio_positions))
        .route("/portfolio/position/:symbol", get(portfolio_controller::get_portfolio_position_card))
        .route("/portfolio/orders", get(portfolio_controller::get_portfolio_orders))
}
