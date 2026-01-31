use axum::{
    extract::State,
    http::{header, HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use mongodb::bson::{doc, oid::ObjectId};
use serde::{Deserialize, Serialize};

use crate::{models::User, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    // user id as hex string
    pub sub: String,
    // expiry (unix timestamp seconds)
    pub exp: usize,
}

fn get_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;

    for part in raw.split(';') {
        let part = part.trim();
        let mut it = part.splitn(2, '=');
        let k = it.next()?.trim();
        let v = it.next()?.trim();
        if k == name {
            return Some(v.to_string());
        }
    }
    None
}

pub async fn inject_current_user(
    State(state): State<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let cookie_name = state.settings.jwt_cookie_name.as_str();

    if let Some(token) = get_cookie(req.headers(), cookie_name) {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        let decoded = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(state.settings.jwt_secret.as_bytes()),
            &validation,
        );

        if let Ok(data) = decoded {
            if let Ok(user_id) = ObjectId::parse_str(&data.claims.sub) {
                let users = state.db.collection::<User>("users");

                if let Ok(Some(user)) = users.find_one(doc! { "_id": user_id }, None).await {
                    // Store user in request extensions so handlers can access it
                    req.extensions_mut().insert(user);
                }
            }
        }
    }

    next.run(req).await
}
