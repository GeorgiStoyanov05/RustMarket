use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    #[serde(rename = "_id")]
    pub id: ObjectId,

    pub user_id: ObjectId,
    pub symbol: String,

    pub qty: i64,
    pub avg_price: f64,

    pub updated_at: i64,
}
