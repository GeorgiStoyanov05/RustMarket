use chrono::Utc;
use mongodb::bson::{doc, oid::ObjectId};

use crate::{models::Account, AppState};

/// Gets the user's account document. If missing, creates it with the default starting balance.
pub async fn get_or_create_account(state: &AppState, user_id: ObjectId) -> Result<Account, String> {
    let accounts = state.db.collection::<Account>("accounts");

    if let Ok(Some(acc)) = accounts.find_one(doc! { "_id": user_id }, None).await {
        return Ok(acc);
    }

    let acc = Account {
        id: user_id,
        cash: 10_000.0,
        updated_at: Utc::now().timestamp(),
    };

    accounts
        .insert_one(&acc, None)
        .await
        .map_err(|e| e.to_string())?;

    Ok(acc)
}

pub async fn set_cash(state: &AppState, user_id: ObjectId, cash: f64, updated_at: i64) -> Result<(), String> {
    let accounts = state.db.collection::<Account>("accounts");
    accounts
        .update_one(
            doc! { "_id": user_id },
            doc! { "$set": { "cash": cash, "updated_at": updated_at } },
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
