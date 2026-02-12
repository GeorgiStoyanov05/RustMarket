use futures_util::StreamExt;

use mongodb::bson::{doc, oid::ObjectId};
use mongodb::options::FindOptions;

use crate::{models::{Order, Position}, AppState};

#[derive(Debug, Clone)]
pub struct PositionView {
    pub symbol: String,
    pub qty: i64,
    pub avg_price: f64,
    pub last_price: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub pnl_class: &'static str,
}

#[derive(Debug, Clone)]
pub struct OrderView {
    pub created_at: String,
    pub symbol: String,
    pub side: String,
    pub qty: i64,
    pub price: f64,
    pub total: f64,
}

fn pnl_class(pnl: f64) -> &'static str {
    if pnl > 0.0 {
        "text-success"
    } else if pnl < 0.0 {
        "text-danger"
    } else {
        "text-muted"
    }
}

pub async fn list_user_positions(state: &AppState, user_id: ObjectId) -> Result<Vec<Position>, String> {
    let positions = state.db.collection::<Position>("positions");
    let find_opts = FindOptions::builder().sort(doc! { "updated_at": -1 }).build();

    let mut cursor = positions
        .find(doc! { "user_id": user_id }, find_opts)
        .await
        .map_err(|e| e.to_string())?;

    let mut out: Vec<Position> = vec![];
    while let Some(res) = cursor.next().await {
        out.push(res.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub async fn get_user_position(state: &AppState, user_id: ObjectId, symbol: &str) -> Result<Option<Position>, String> {
    let sym = symbol.to_uppercase();
    let positions = state.db.collection::<Position>("positions");
    positions
        .find_one(doc! { "user_id": user_id, "symbol": &sym }, None)
        .await
        .map_err(|e| e.to_string())
}

pub async fn list_portfolio_position_views(state: &AppState, user_id: ObjectId) -> Result<Vec<PositionView>, String> {
    let positions = list_user_positions(state, user_id).await?;

    let mut views: Vec<PositionView> = vec![];
    for p in positions {
        let symbol = p.symbol.to_uppercase();
        let last = state.finnhub.quote(&symbol).await.ok().map(|x| x.c).unwrap_or(0.0);

        let pnl = (last - p.avg_price) * (p.qty as f64);
        let pct = if p.avg_price > 0.0 {
            ((last - p.avg_price) / p.avg_price) * 100.0
        } else {
            0.0
        };

        views.push(PositionView {
            symbol,
            qty: p.qty,
            avg_price: p.avg_price,
            last_price: last,
            pnl,
            pnl_pct: pct,
            pnl_class: pnl_class(pnl),
        });
    }

    Ok(views)
}

pub async fn get_portfolio_position_view(state: &AppState, user_id: ObjectId, symbol: &str) -> Result<Option<PositionView>, String> {
    let Some(p) = get_user_position(state, user_id, symbol).await? else {
        return Ok(None);
    };

    let sym = p.symbol.to_uppercase();
    let last = state.finnhub.quote(&sym).await.ok().map(|x| x.c).unwrap_or(0.0);
    let pnl = (last - p.avg_price) * (p.qty as f64);
    let pct = if p.avg_price > 0.0 {
        ((last - p.avg_price) / p.avg_price) * 100.0
    } else {
        0.0
    };

    Ok(Some(PositionView {
        symbol: sym,
        qty: p.qty,
        avg_price: p.avg_price,
        last_price: last,
        pnl,
        pnl_pct: pct,
        pnl_class: pnl_class(pnl),
    }))
}

pub async fn list_recent_orders(state: &AppState, user_id: ObjectId, limit: i64) -> Result<Vec<Order>, String> {
    let orders = state.db.collection::<Order>("orders");
    let find_opts = FindOptions::builder().sort(doc! { "created_at": -1 }).limit(limit).build();

    let mut cursor = orders
        .find(doc! { "user_id": user_id }, find_opts)
        .await
        .map_err(|e| e.to_string())?;

    let mut out: Vec<Order> = vec![];
    while let Some(res) = cursor.next().await {
        out.push(res.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub async fn list_recent_order_views(state: &AppState, user_id: ObjectId, limit: i64) -> Result<Vec<OrderView>, String> {
    let orders = list_recent_orders(state, user_id, limit).await?;

    let mut out: Vec<OrderView> = vec![];
    for o in orders {
        let dt = chrono::DateTime::from_timestamp(o.created_at, 0)
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| o.created_at.to_string());

        out.push(OrderView {
            created_at: dt,
            symbol: o.symbol.to_uppercase(),
            side: o.side,
            qty: o.qty,
            price: o.price,
            total: o.total,
        });
    }

    Ok(out)
}
