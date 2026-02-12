use axum::{Router, routing::get};
use crate::{AppState, controllers::stocks_controller};

pub fn add_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/search", get(stocks_controller::get_search))
        .route("/search/results", get(stocks_controller::get_search_results))
        .route("/details/:symbol", get(stocks_controller::get_details))
        .route("/details/:symbol/quote", get(stocks_controller::get_details_quote))
}
