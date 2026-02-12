use axum::{Router, routing::{get, post}};

use crate::{AppState, controllers::trading_controller};

pub fn add_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/positions/:symbol", get(trading_controller::get_position_panel))
        .route("/trade/:symbol/buy", post(trading_controller::post_trade_buy))
        .route("/trade/:symbol/sell", post(trading_controller::post_trade_sell))
}
