use std::{convert::Infallible, time::Duration as StdDuration};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State, Extension,
    },
    http::StatusCode,
    response::{IntoResponse, sse::{Event, KeepAlive, Sse}},
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::time::{interval, Duration as TokioDuration};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as TMessage};
use tokio::sync::broadcast::error::RecvError;

use crate::{models::CurrentUser, AppState};

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
    let url = format!("wss://ws.finnhub.io/?token={}", token);

    tracing::info!("WS client connected: symbol={}", symbol);
    tracing::info!("Connecting to Finnhub WS...");

    let (fh_ws, _) = match connect_async(url.as_str()).await {
        Ok(x) => x,
        Err(err) => {
            tracing::error!("Finnhub WS connect failed: {}", err);
            let _ = client_ws
                .send(Message::Text(format!(
                    r#"{{\"type\":\"error\",\"message\":\"Finnhub WS connect failed: {}\"}}"#,
                    err
                )))
                .await;
            let _ = client_ws.close().await;
            return;
        }
    };

    tracing::info!("Finnhub WS connected OK");

    let (mut fh_write, mut fh_read) = fh_ws.split();

    let sub = serde_json::json!({ "type": "subscribe", "symbol": symbol });
    let _ = fh_write.send(TMessage::Text(sub.to_string())).await;

    let mut ping = interval(TokioDuration::from_secs(25));

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

#[derive(Deserialize)]
pub struct TradesMultiWsQuery {
    pub symbols: String,
}

// GET /ws/trades_multi?symbols=AAPL,MSFT,TSLA
pub async fn ws_trades_multi(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(q): Query<TradesMultiWsQuery>,
) -> impl IntoResponse {
    let token = state.settings.finnhub_api_key.trim().to_string();
    if token.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "missing FINNHUB_API_KEY",
        )
            .into_response();
    }

    let mut syms: Vec<String> = q
        .symbols
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();

    syms.sort();
    syms.dedup();

    if syms.is_empty() {
        return (StatusCode::BAD_REQUEST, "missing symbols").into_response();
    }

    if syms.len() > 50 {
        syms.truncate(50);
    }

    ws.on_upgrade(move |socket| handle_trades_multi_socket(socket, syms, token))
}

async fn handle_trades_multi_socket(mut client_ws: WebSocket, symbols: Vec<String>, token: String) {
    let url = format!("wss://ws.finnhub.io/?token={}", token);

    tracing::info!("WS multi client connected: symbols={:?}", symbols);
    tracing::info!("Connecting to Finnhub WS...");

    let (fh_ws, _) = match connect_async(url.as_str()).await {
        Ok(x) => x,
        Err(err) => {
            tracing::error!("Finnhub WS connect failed: {}", err);
            let _ = client_ws
                .send(Message::Text(format!(
                    r#"{{\"type\":\"error\",\"message\":\"Finnhub WS connect failed: {}\"}}"#,
                    err
                )))
                .await;
            let _ = client_ws.close().await;
            return;
        }
    };

    let (mut fh_write, mut fh_read) = fh_ws.split();

    for s in &symbols {
        let sub = serde_json::json!({ "type": "subscribe", "symbol": s });
        let _ = fh_write.send(TMessage::Text(sub.to_string())).await;
    }

    let mut ping = interval(TokioDuration::from_secs(25));

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

// GET /events  (SSE)
pub async fn sse_events(
    State(state): State<AppState>,
    Extension(_u): Extension<CurrentUser>,
) -> Sse<impl futures_util::stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.events_tx.subscribe();

    let stream = futures_util::stream::unfold(rx, |mut rx| async {
        let evt = match rx.recv().await {
            Ok(name) => Event::default().event(name).data("1"),
            Err(RecvError::Lagged(_)) => Event::default().event("ping").data("lagged"),
            Err(RecvError::Closed) => Event::default().event("ping").data("closed"),
        };

        Some((Ok(evt), rx))
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(StdDuration::from_secs(20))
            .text("keep-alive"),
    )
}
