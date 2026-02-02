use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Extension, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::Stream;
use tokio::sync::broadcast::error::RecvError;

use crate::{models::CurrentUser, AppState};

pub async fn sse_events(
    State(state): State<AppState>,
    Extension(_u): Extension<CurrentUser>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.events_tx.subscribe();

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
            .interval(Duration::from_secs(20))
            .text("keep-alive"),
    )
}
