use bcrypt::verify;
use chrono::Utc;
use mongodb::bson::{doc, oid::ObjectId};

use crate::{models::{Account, User}, AppState};

use super::{account_service, auth_service::FieldErrors};

pub async fn change_email(state: &AppState, user_id: ObjectId, new_email: &str) -> Result<(), FieldErrors> {
    let mut errs = FieldErrors::new();

    let users = state.db.collection::<User>("users");

    if let Err(e) = users
        .update_one(doc! { "_id": user_id }, doc! { "$set": { "email": new_email } }, None)
        .await
    {
        let msg = e.to_string();
        if msg.contains("E11000") {
            errs.insert("email".into(), "This email is already in use.".into());
        } else {
            errs.insert("_form".into(), format!("db error: {e}"));
        }
        return Err(errs);
    }

    Ok(())
}

pub async fn change_password(state: &AppState, user_id: ObjectId, new_password: &str) -> Result<(), FieldErrors> {
    let mut errs = FieldErrors::new();

    let users = state.db.collection::<User>("users");

    let db_user = match users.find_one(doc! { "_id": user_id }, None).await {
        Ok(Some(u)) => u,
        _ => {
            errs.insert("_form".into(), "User not found.".into());
            return Err(errs);
        }
    };

    if verify(new_password, &db_user.password_hash).unwrap_or(false) {
        errs.insert(
            "password".into(),
            "New password must be different from your current password.".into(),
        );
        return Err(errs);
    }

    let pw_hash = match bcrypt::hash(new_password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => {
            errs.insert("_form".into(), "Failed to hash password.".into());
            return Err(errs);
        }
    };

    if let Err(e) = users
        .update_one(doc! { "_id": user_id }, doc! { "$set": { "password_hash": pw_hash } }, None)
        .await
    {
        errs.insert("_form".into(), format!("db error: {e}"));
        return Err(errs);
    }

    Ok(())
}

pub async fn deposit_funds(state: &AppState, user_id: ObjectId, amount: f64) -> Result<Account, FieldErrors> {
    let mut errs = FieldErrors::new();

    let mut acc = match account_service::get_or_create_account(state, user_id).await {
        Ok(a) => a,
        Err(e) => {
            errs.insert("_form".into(), format!("db error: {e}"));
            return Err(errs);
        }
    };

    acc.cash += amount;
    acc.updated_at = Utc::now().timestamp();

    if let Err(e) = account_service::set_cash(state, user_id, acc.cash, acc.updated_at).await {
        errs.insert("_form".into(), format!("db error: {e}"));
        return Err(errs);
    }

    let _ = state.events_tx.send("cashUpdated".to_string());

    Ok(acc)
}
