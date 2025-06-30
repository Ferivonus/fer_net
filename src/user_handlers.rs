use crate::auth::create_jwt;
use crate::{
    db::USERS,
    models::{LoginRequest, LoginResponse},
};
use actix_web::{get, post, web, HttpResponse, Responder};
use bcrypt::verify;

#[post("/login")]
pub async fn login(data: web::Json<LoginRequest>) -> impl Responder {
    let users = USERS.lock().await;
    if let Some(user) = users.get(&data.username) {
        if verify(&data.password, &user.password_hash).unwrap_or(false) {
            let token = create_jwt(&user.username);
            return HttpResponse::Ok().json(LoginResponse { token });
        }
    }
    HttpResponse::Unauthorized().body("Invalid username or password")
}

#[get("/hello")]
pub async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello! You are authenticated.")
}
