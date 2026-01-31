use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "_id")]
    pub id: ObjectId,

    pub email: String,
    pub username: String,

    pub password_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentUser {
    pub id: ObjectId,
    pub email: String,
    pub username: String,
}

impl From<User> for CurrentUser {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            username: u.username,
        }
    }
}
