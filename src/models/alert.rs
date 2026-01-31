use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    #[serde(rename = "_id")]
    pub id: ObjectId,

    pub user_id: ObjectId,
    pub symbol: String,

    // "above" | "below"
    pub condition: String,
    pub target_price: f64,

    pub created_at: i64,

    // reserved for later
    pub triggered: bool,
    pub triggered_at: Option<i64>,
}
