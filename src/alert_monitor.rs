use std::{collections::HashMap, time::Duration};

use chrono::Utc;
use futures_util::StreamExt;
use mongodb::bson::doc;
use tokio::time;

use crate::{models::Alert, AppState};

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

    // 1) Fetch all untriggered alerts
    let mut cursor = alerts
        .find(doc! { "triggered": false }, None)
        .await
        .map_err(|e| e.to_string())?;

    // 2) Group by symbol => only 1 quote request per symbol per tick
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

    // 3) Check each symbol once
    for (sym, group) in by_symbol {
        let quote = match state.finnhub.quote(&sym).await {
            Ok(q) => q,
            Err(_) => continue, // ignore symbol if API fails this tick
        };

        let price = quote.c;
        if !price.is_finite() || price <= 0.0 {
            continue;
        }

        // 4) Trigger matching alerts
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

    // 5) Notify all open pages/tabs to refresh alerts UI
    if triggered_any {
        let _ = state.events_tx.send("alertsUpdated".to_string());
    }

    Ok(())
}
