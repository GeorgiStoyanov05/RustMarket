use std::collections::HashMap;

use axum_extra::extract::cookie::{Cookie, SameSite};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use mongodb::bson::{doc, oid::ObjectId};

use crate::{models::User, AppState};

pub type FieldErrors = HashMap<String, String>;

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

pub fn make_jwt_with_days(state: &AppState, user_id: &ObjectId, days: i64) -> Result<String, String> {
    let exp = (Utc::now() + Duration::days(days)).timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_hex(),
        exp,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.settings.jwt_secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

pub fn auth_cookie(state: &AppState, token: String) -> Cookie<'static> {
    let mut cookie = Cookie::new(state.settings.jwt_cookie_name.clone(), token);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    if state.settings.cookie_secure {
        cookie.set_secure(true);
    }
    cookie
}

pub fn clear_auth_cookie(state: &AppState) -> Cookie<'static> {
    let mut cookie = Cookie::new(state.settings.jwt_cookie_name.clone(), "");
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.make_removal();
    cookie
}

pub async fn login_user(state: &AppState, email: &str, password: &str) -> Result<User, FieldErrors> {
    let mut errs: FieldErrors = HashMap::new();

    let users = state.db.collection::<User>("users");

    let user = match users.find_one(doc! { "email": email }, None).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            errs.insert("_form".into(), "Invalid email or password.".into());
            return Err(errs);
        }
        Err(_) => {
            errs.insert("_form".into(), "Server error. Please try again.".into());
            return Err(errs);
        }
    };

    if !verify(password, &user.password_hash).unwrap_or(false) {
        errs.insert("_form".into(), "Invalid email or password.".into());
        return Err(errs);
    }

    Ok(user)
}

pub async fn register_user(
    state: &AppState,
    username: &str,
    email: &str,
    password: &str,
) -> Result<ObjectId, FieldErrors> {
    let mut errs: FieldErrors = HashMap::new();

    let users = state.db.collection::<User>("users");

    // unique email
    match users.find_one(doc! { "email": email }, None).await {
        Ok(Some(_)) => {
            errs.insert("email".into(), "Email has already been taken!".into());
            return Err(errs);
        }
        Ok(None) => {}
        Err(_) => {
            errs.insert("_form".into(), "There is a problem registering this user!".into());
            return Err(errs);
        }
    }

    // unique username
    match users.find_one(doc! { "username": username }, None).await {
        Ok(Some(_)) => {
            errs.insert("username".into(), "Username has already been taken!".into());
            return Err(errs);
        }
        Ok(None) => {}
        Err(_) => {
            errs.insert("_form".into(), "There is a problem registering this user!".into());
            return Err(errs);
        }
    }

    let pw_hash = match hash(password, DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => {
            errs.insert("_form".into(), "There is a problem registering this user!".into());
            return Err(errs);
        }
    };

    let insert = match state
        .db
        .collection("users")
        .insert_one(
            doc! {
                "email": email,
                "username": username,
                "password_hash": pw_hash,
            },
            None,
        )
        .await
    {
        Ok(r) => r,
        Err(_) => {
            errs.insert("_form".into(), "There is a problem registering this user!".into());
            return Err(errs);
        }
    };

    let new_id = insert
        .inserted_id
        .as_object_id()
        .ok_or_else(|| {
            let mut e = FieldErrors::new();
            e.insert("_form".into(), "There is a problem registering this user!".into());
            e
        })?;

    Ok(new_id)
}
