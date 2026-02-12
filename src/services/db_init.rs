use mongodb::{
    bson::doc,
    options::IndexOptions,
    Database, IndexModel,
};

pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    // users: unique email
    {
        let col = db.collection::<mongodb::bson::Document>("users");
        let model = IndexModel::builder()
            .keys(doc! { "email": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        col.create_index(model, None)
            .await
            .map_err(|e| e.to_string())?;
    }

    // positions: unique per (user_id, symbol)
    {
        let col = db.collection::<mongodb::bson::Document>("positions");
        let model = IndexModel::builder()
            .keys(doc! { "user_id": 1, "symbol": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        col.create_index(model, None)
            .await
            .map_err(|e| e.to_string())?;
    }

    // orders: query by user quickly and sort by created_at desc
    {
        let col = db.collection::<mongodb::bson::Document>("orders");
        let model = IndexModel::builder()
            .keys(doc! { "user_id": 1, "created_at": -1 })
            .build();

        col.create_index(model, None)
            .await
            .map_err(|e| e.to_string())?;
    }

    // alerts: helpful for monitor scan (triggered + symbol)
    {
        let col = db.collection::<mongodb::bson::Document>("alerts");
        let model = IndexModel::builder()
            .keys(doc! { "triggered": 1, "symbol": 1 })
            .build();

        let _ = col.create_index(model, None).await;
    }

    Ok(())
}
