use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub user_id: ObjectId,
    pub symbol: String,
    pub side: String,
    pub qty: i64,
    pub price: f64,
    pub total: f64,
    pub created_at: i64,
}
