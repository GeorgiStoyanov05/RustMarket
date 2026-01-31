use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    // use user id as primary key
    #[serde(rename = "_id")]
    pub id: ObjectId,

    pub cash: f64,
    pub updated_at: i64,
}
