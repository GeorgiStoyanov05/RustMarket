use serde_json::json;

use crate::{AppState};

/// Build the context used by the `partials/search_results` template.
///
/// This mirrors the behavior implemented directly in the controller:
/// - empty query => no results, no error
/// - non-empty query => call Finnhub search, filter empty symbols, limit 10
/// - Finnhub failure => generic error string
pub async fn search_results_ctx(state: &AppState, query: &str) -> serde_json::Value {
    let q = query.trim().to_string();

    if q.is_empty() {
        return json!({
            "query": "",
            "results": serde_json::Value::Null,
            "error": serde_json::Value::Null
        });
    }

    match state.finnhub.search(&q).await {
        Ok(resp) => {
            let results: Vec<_> = resp
                .result
                .into_iter()
                .filter(|it| !it.symbol.trim().is_empty())
                .take(10)
                .map(|it| {
                    json!({
                        "symbol": it.symbol,
                        "display_symbol": it.display_symbol,
                        "description": it.description,
                        "type": it.kind
                    })
                })
                .collect();

            let results_val = if results.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::Array(results)
            };

            json!({
                "query": q,
                "results": results_val,
                "error": serde_json::Value::Null
            })
        }
        Err(_err) => json!({
            "query": q,
            "results": serde_json::Value::Null,
            "error": "Search unavailable right now."
        }),
    }
}

/// Build the context used by the `partials/quote` template.
pub async fn quote_ctx(state: &AppState, symbol: &str) -> serde_json::Value {
    match state.finnhub.quote(symbol).await {
        Ok(q) => json!({ "quote": q, "error": serde_json::Value::Null }),
        Err(err) => json!({ "quote": serde_json::Value::Null, "error": err }),
    }
}
