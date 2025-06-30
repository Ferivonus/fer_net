use crate::models::User;
use bcrypt::{hash, DEFAULT_COST};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub type UserDB = Arc<Mutex<HashMap<String, User>>>;

lazy_static! {
    pub static ref USERS: UserDB = Arc::new(Mutex::new(HashMap::new()));
}

pub async fn add_user(username: &str, password: &str) {
    let hashed = hash(password, DEFAULT_COST).unwrap();
    let user = User {
        username: username.to_string(),
        password_hash: hashed,
    };
    USERS.lock().await.insert(username.to_string(), user);
}
