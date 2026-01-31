use std::env;

#[derive(Debug, Clone)]
pub struct Settings {
    pub mongodb_uri: String,
    pub mongodb_db: String,
    pub host: String,
    pub port: u16,

    pub jwt_secret: String,
    pub jwt_cookie_name: String,
}


pub fn load() -> Settings {
    // Loads .env if present (no crash if missing)
    dotenvy::dotenv().ok();

    let mongodb_uri = env::var("MONGODB_URI")
        .unwrap_or_else(|_| "mongodb://localhost:27017".to_string());

    let mongodb_db = env::var("MONGODB_DB")
        .unwrap_or_else(|_| "gomarket".to_string());

    let host = env::var("HOST")
        .unwrap_or_else(|_| "127.0.0.1".to_string());

    let port = env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(3000);

    let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "change-me-dev-secret".to_string());
    let jwt_cookie_name = env::var("JWT_COOKIE_NAME").unwrap_or_else(|_| "auth".to_string());

    Settings {
        mongodb_uri,
        mongodb_db,
        host,
        port,
        jwt_secret,
        jwt_cookie_name,
    }
}
