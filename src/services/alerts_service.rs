use std::collections::BTreeMap;

use chrono::Utc;
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::options::FindOptions;

use crate::{models::Alert, AppState};

pub async fn list_user_symbol_alerts(
    state: &AppState,
    user_id: ObjectId,
    symbol: &str,
) -> Result<Vec<Alert>, String> {
    let sym = symbol.to_uppercase();
    let alerts = state.db.collection::<Alert>("alerts");

    let find_opts = FindOptions::builder()
        .sort(doc! { "created_at": -1 })
        .build();

    let mut cursor = alerts
        .find(doc! { "user_id": user_id, "symbol": &sym }, find_opts)
        .await
        .map_err(|e| e.to_string())?;

    let mut items: Vec<Alert> = Vec::new();
    while let Some(res) = cursor.next().await {
        items.push(res.map_err(|e| e.to_string())?);
    }

    Ok(items)
}

pub async fn create_alert(
    state: &AppState,
    user_id: ObjectId,
    symbol: &str,
    condition: &str,
    target_price: f64,
) -> Result<Alert, String> {
    let sym = symbol.to_uppercase();
    let alerts = state.db.collection::<Alert>("alerts");
    let now = Utc::now().timestamp();

    let alert = Alert {
        id: ObjectId::new(),
        user_id,
        symbol: sym,
        condition: condition.to_lowercase(),
        target_price,
        created_at: now,
        triggered: false,
        triggered_at: None,
    };

    alerts
        .insert_one(&alert, None)
        .await
        .map_err(|e| e.to_string())?;

    let _ = state.events_tx.send("alertsUpdated".to_string());

    Ok(alert)
}

pub async fn delete_alert_for_symbol(
    state: &AppState,
    user_id: ObjectId,
    symbol: &str,
    alert_id: ObjectId,
) -> Result<(), String> {
    let sym = symbol.to_uppercase();
    let alerts = state.db.collection::<Alert>("alerts");

    alerts
        .delete_one(doc! { "_id": alert_id, "user_id": user_id, "symbol": &sym }, None)
        .await
        .map_err(|e| e.to_string())?;

    let _ = state.events_tx.send("alertsUpdated".to_string());

    Ok(())
}

pub async fn delete_alert_global(
    state: &AppState,
    user_id: ObjectId,
    alert_id: ObjectId,
) -> Result<(), String> {
    let alerts = state.db.collection::<Alert>("alerts");

    alerts
        .delete_one(doc! { "_id": alert_id, "user_id": user_id }, None)
        .await
        .map_err(|e| e.to_string())?;

    let _ = state.events_tx.send("alertsUpdated".to_string());

    Ok(())
}

/// Returns true if the alert was newly triggered, false if it was already triggered.
pub async fn trigger_alert(
    state: &AppState,
    user_id: ObjectId,
    alert_id: ObjectId,
) -> Result<bool, String> {
    let alerts = state.db.collection::<Alert>("alerts");
    let now = Utc::now().timestamp();

    let res = alerts
        .update_one(
            doc! { "_id": alert_id, "user_id": user_id, "triggered": false },
            doc! { "$set": { "triggered": true, "triggered_at": now } },
            None,
        )
        .await
        .map_err(|e| e.to_string())?;

    let _ = state.events_tx.send("alertsUpdated".to_string());

    Ok(res.matched_count > 0)
}

pub async fn list_user_alerts_grouped(
    state: &AppState,
    user_id: ObjectId,
) -> Result<BTreeMap<String, Vec<Alert>>, String> {
    let alerts = state.db.collection::<Alert>("alerts");
    let find_opts = FindOptions::builder().sort(doc! { "created_at": -1 }).build();

    let mut cursor = alerts
        .find(doc! { "user_id": user_id }, find_opts)
        .await
        .map_err(|e| e.to_string())?;

    let mut map: BTreeMap<String, Vec<Alert>> = BTreeMap::new();
    while let Some(res) = cursor.next().await {
        let a = res.map_err(|e| e.to_string())?;
        map.entry(a.symbol.to_uppercase()).or_default().push(a);
    }

    Ok(map)
}
