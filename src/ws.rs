use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::time::{interval, Duration};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as TMessage};

use crate::AppState;

#[derive(Deserialize)]
pub struct TradesWsQuery {
    pub symbol: String,
}

// GET /ws/trades?symbol=AAPL
pub async fn ws_trades(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(q): Query<TradesWsQuery>,
) -> impl IntoResponse {
    let symbol = q.symbol.trim().to_string();
    let token = state.settings.finnhub_api_key.trim().to_string();

    if symbol.is_empty() {
        return (StatusCode::BAD_REQUEST, "missing symbol").into_response();
    }
    if token.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "missing FINNHUB_API_KEY",
        )
            .into_response();
    }

    ws.on_upgrade(move |socket| handle_trades_socket(socket, symbol, token))
}

async fn handle_trades_socket(mut client_ws: WebSocket, symbol: String, token: String) {
    // IMPORTANT: add the "/" before ?token=  (prevents 400 in some WS stacks)
    let url = format!("wss://ws.finnhub.io/?token={}", token);

    tracing::info!("WS client connected: symbol={}", symbol);
    tracing::info!("Connecting to Finnhub WS...");

    let (fh_ws, _) = match connect_async(url.as_str()).await {
        Ok(x) => x,
        Err(err) => {
            tracing::error!("Finnhub WS connect failed: {}", err);
            let _ = client_ws
                .send(Message::Text(format!(
                    r#"{{"type":"error","message":"Finnhub WS connect failed: {}"}}"#,
                    err
                )))
                .await;
            let _ = client_ws.close().await;
            return;
        }
    };

    tracing::info!("Finnhub WS connected OK");

    let (mut fh_write, mut fh_read) = fh_ws.split();

    // Subscribe to symbol
    let sub = serde_json::json!({ "type": "subscribe", "symbol": symbol });
    let _ = fh_write.send(TMessage::Text(sub.to_string())).await;

    // Ping browser to keep alive
    let mut ping = interval(Duration::from_secs(25));

    loop {
        tokio::select! {
            _ = ping.tick() => {
                if client_ws.send(Message::Ping(b"ping".to_vec())).await.is_err() {
                    break;
                }
            }

            fh_msg = fh_read.next() => {
                match fh_msg {
                    Some(Ok(TMessage::Text(txt))) => {
                        // forward EVERYTHING (including Finnhub errors)
                        if client_ws.send(Message::Text(txt)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(TMessage::Binary(bin))) => {
                        if client_ws.send(Message::Binary(bin)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(TMessage::Ping(payload))) => {
                        let _ = fh_write.send(TMessage::Pong(payload)).await;
                    }
                    Some(Ok(TMessage::Pong(_))) => {}
                    Some(Ok(TMessage::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }

            client_msg = client_ws.recv() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
        }
    }

    let _ = client_ws.close().await;
}
