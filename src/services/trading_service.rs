use std::collections::HashMap;

use chrono::Utc;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::options::UpdateOptions;

use crate::{
    models::{Order, Position},
    AppState,
};

use super::{account_service, auth_service::FieldErrors};

#[derive(Debug, Clone)]
pub struct BuyResult {
    pub symbol: String,
    pub qty: i64,
    pub fill_price: f64,
    pub cost: f64,
    pub new_cash: f64,
    pub position: Position,
}

#[derive(Debug, Clone)]
pub struct SellResult {
    pub symbol: String,
    pub qty: i64,
    pub fill_price: f64,
    pub proceeds: f64,
    pub new_cash: f64,
    pub remaining: Option<Position>,
}

async fn get_position(state: &AppState, user_id: ObjectId, symbol: &str) -> Result<Option<Position>, String> {
    let positions = state.db.collection::<Position>("positions");
    positions
        .find_one(doc! { "user_id": user_id, "symbol": symbol }, None)
        .await
        .map_err(|e| e.to_string())
}

async fn upsert_position(state: &AppState, pos: &Position) -> Result<(), String> {
    let positions = state.db.collection::<Position>("positions");
    positions
        .update_one(
            doc! { "_id": pos.id },
            doc! {
                "$set": {
                    "user_id": pos.user_id,
                    "symbol": &pos.symbol,
                    "qty": pos.qty,
                    "avg_price": pos.avg_price,
                    "updated_at": pos.updated_at,
                }
            },
            UpdateOptions::builder().upsert(true).build(),
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn delete_position(state: &AppState, id: ObjectId) -> Result<(), String> {
    let positions = state.db.collection::<Position>("positions");
    positions
        .delete_one(doc! { "_id": id }, None)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_user_position(state: &AppState, user_id: ObjectId, symbol: &str) -> Result<Option<Position>, String> {
    let sym = symbol.to_uppercase();
    get_position(state, user_id, &sym).await
}

pub async fn market_buy(state: &AppState, user_id: ObjectId, symbol: &str, qty: i64) -> Result<BuyResult, FieldErrors> {
    let mut errs: FieldErrors = HashMap::new();

    let sym = symbol.to_uppercase();

    if sym.trim().is_empty() {
        errs.insert("symbol".into(), "Missing symbol.".into());
    }
    if qty <= 0 {
        errs.insert("qty".into(), "Enter a valid quantity.".into());
    }
    if !errs.is_empty() {
        return Err(errs);
    }

    let quote = match state.finnhub.quote(&sym).await {
        Ok(q) => q,
        Err(e) => {
            errs.insert("_form".into(), format!("Quote error: {e}"));
            return Err(errs);
        }
    };

    let price = quote.c;
    let total = price * (qty as f64);

    let mut acc = match account_service::get_or_create_account(state, user_id).await {
        Ok(a) => a,
        Err(e) => {
            errs.insert("_form".into(), format!("db error: {e}"));
            return Err(errs);
        }
    };

    if acc.cash < total {
        errs.insert("balance".into(), "Not enough cash.".into());
        return Err(errs);
    }

    let pos_opt = match get_position(state, user_id, &sym).await {
        Ok(p) => p,
        Err(e) => {
            errs.insert("_form".into(), format!("db error: {e}"));
            return Err(errs);
        }
    };

    let now = Utc::now().timestamp();

    let new_pos = match pos_opt {
        Some(mut p) => {
            let new_qty = p.qty + qty;
            let new_avg = ((p.avg_price * (p.qty as f64)) + total) / (new_qty as f64);
            p.qty = new_qty;
            p.avg_price = new_avg;
            p.updated_at = now;
            p
        }
        None => Position {
            id: ObjectId::new(),
            user_id,
            symbol: sym.clone(),
            qty,
            avg_price: price,
            updated_at: now,
        },
    };

    if let Err(e) = upsert_position(state, &new_pos).await {
        errs.insert("_form".into(), format!("db error: {e}"));
        return Err(errs);
    }

    // deduct cash
    acc.cash -= total;
    acc.updated_at = now;

    if let Err(e) = account_service::set_cash(state, user_id, acc.cash, acc.updated_at).await {
        errs.insert("_form".into(), format!("db error: {e}"));
        return Err(errs);
    }

    // store order
    let orders = state.db.collection::<Order>("orders");
    let order = Order {
        id: ObjectId::new(),
        user_id,
        symbol: sym.clone(),
        side: "buy".to_string(),
        qty,
        price,
        total,
        created_at: now,
    };
    let _ = orders.insert_one(order, None).await;

    // broadcast so other tabs/pages update
    let _ = state.events_tx.send("ordersUpdated".to_string());
    let _ = state.events_tx.send("positionUpdated".to_string());
    let _ = state.events_tx.send("cashUpdated".to_string());

    Ok(BuyResult {
        symbol: sym,
        qty,
        fill_price: price,
        cost: total,
        new_cash: acc.cash,
        position: new_pos,
    })
}

pub async fn market_sell(state: &AppState, user_id: ObjectId, symbol: &str, qty: i64) -> Result<SellResult, FieldErrors> {
    let mut errs: FieldErrors = HashMap::new();

    let sym = symbol.to_uppercase();

    if sym.trim().is_empty() {
        errs.insert("symbol".into(), "Missing symbol.".into());
    }
    if qty <= 0 {
        errs.insert("qty".into(), "Enter a valid quantity.".into());
    }
    if !errs.is_empty() {
        return Err(errs);
    }

    let quote = match state.finnhub.quote(&sym).await {
        Ok(q) => q,
        Err(e) => {
            errs.insert("_form".into(), format!("Quote error: {e}"));
            return Err(errs);
        }
    };

    let price = quote.c;

    let pos_opt = match get_position(state, user_id, &sym).await {
        Ok(p) => p,
        Err(e) => {
            errs.insert("_form".into(), format!("db error: {e}"));
            return Err(errs);
        }
    };

    let Some(mut pos) = pos_opt else {
        errs.insert("qty".into(), "You have no position to sell.".into());
        return Err(errs);
    };

    if qty > pos.qty {
        errs.insert("qty".into(), "You don't have that many shares.".into());
        return Err(errs);
    }

    let proceeds = price * (qty as f64);
    let now = Utc::now().timestamp();

    pos.qty -= qty;
    pos.updated_at = now;

    let remaining = if pos.qty == 0 {
        let _ = delete_position(state, pos.id).await;
        None
    } else {
        if let Err(e) = upsert_position(state, &pos).await {
            errs.insert("_form".into(), format!("db error: {e}"));
            return Err(errs);
        }
        Some(pos.clone())
    };

    let mut acc = match account_service::get_or_create_account(state, user_id).await {
        Ok(a) => a,
        Err(e) => {
            errs.insert("_form".into(), format!("db error: {e}"));
            return Err(errs);
        }
    };

    acc.cash += proceeds;
    acc.updated_at = now;

    if let Err(e) = account_service::set_cash(state, user_id, acc.cash, acc.updated_at).await {
        errs.insert("_form".into(), format!("db error: {e}"));
        return Err(errs);
    }

    // store order
    let orders = state.db.collection::<Order>("orders");
    let order = Order {
        id: ObjectId::new(),
        user_id,
        symbol: sym.clone(),
        side: "sell".to_string(),
        qty,
        price,
        total: proceeds,
        created_at: now,
    };
    let _ = orders.insert_one(order, None).await;

    let _ = state.events_tx.send("ordersUpdated".to_string());
    let _ = state.events_tx.send("positionUpdated".to_string());
    let _ = state.events_tx.send("cashUpdated".to_string());

    Ok(SellResult {
        symbol: sym,
        qty,
        fill_price: price,
        proceeds,
        new_cash: acc.cash,
        remaining,
    })
}
