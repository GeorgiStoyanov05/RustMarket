use mongodb::bson::doc;
use std::time::Duration;
use tokio::time;

use crate::{AppState, models::Alert};

pub fn spawn_price_alert_monitor(state: AppState) {
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            if let Err(e) = run_tick(&state).await {
                eprintln!("[alert-monitor] tick error: {}", e);
            }
        }
    });
}

async fn run_tick(state: &AppState) -> Result<(), String> {
    use std::collections::HashMap;

    use futures_util::StreamExt;
    use mongodb::bson::doc;

    let alerts = state.db.collection::<Alert>("alerts");

    let mut cursor = alerts
        .find(doc! { "triggered": false }, None)
        .await
        .map_err(|e| e.to_string())?;

    let mut by_symbol: HashMap<String, Vec<Alert>> = HashMap::new();
    while let Some(item) = cursor.next().await {
        let a = item.map_err(|e| e.to_string())?;
        by_symbol.entry(a.symbol.clone()).or_default().push(a);
    }

    if by_symbol.is_empty() {
        return Ok(());
    }

    let mut triggered_any = false;
    let now = chrono::Utc::now().timestamp();

    for (sym, group) in by_symbol {
        let quote = match state.finnhub.quote(&sym).await {
            Ok(q) => q,
            Err(_) => continue,
        };

        let price = quote.c;
        if !price.is_finite() || price <= 0.0 {
            continue;
        }

        for a in group {
            let hit = (a.condition == "above" && price >= a.target_price)
                || (a.condition == "below" && price <= a.target_price);

            if !hit {
                continue;
            }

            let res = alerts
                .update_one(
                    doc! { "_id": a.id, "triggered": false },
                    doc! { "$set": { "triggered": true, "triggered_at": now } },
                    None,
                )
                .await;

            if res.is_ok() {
                triggered_any = true;
            }
        }
    }

    if triggered_any {
        let _ = state.events_tx.send("alertsUpdated".to_string());
    }

    Ok(())
}
