use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use dotenv::dotenv;
use std::env;

// GET /
#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Actix is running on Railway!")
}

// GET /ping
#[get("/ping")]
async fn ping() -> impl Responder {
    HttpResponse::Ok().body("pong")
}

// GET /greet/{name}
#[get("/greet/{name}")]
async fn greet(path: web::Path<String>) -> impl Responder {
    let name = path.into_inner();
    HttpResponse::Ok().body(format!("Hello, {}!", name))
}

// POST /echo
#[post("/echo")]
async fn echo(body: String) -> impl Responder {
    HttpResponse::Ok().body(format!("Received: {}", body))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load variables from .env file
    dotenv().ok();

    // Get the PORT from environment or use default
    let port = env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("Server listening on: {}", addr);

    HttpServer::new(|| {
        App::new()
            .service(hello)
            .service(ping)
            .service(greet)
            .service(echo)
    })
    .bind(addr)?
    .run()
    .await
}
