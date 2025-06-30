use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProxyNode {
    id: Uuid,
    name: String,
    ip: String,
    port: u16,
    active: bool,
}

type ProxyStack = Arc<Mutex<Vec<ProxyNode>>>;

#[get("/")]
async fn index() -> impl Responder {
    let endpoints = serde_json::json!({
        "endpoints": {
            "GET /": "This help message",
            "GET /health": "Health check (returns OK)",
            "POST /register": "Register a new proxy node",
            "GET /stacked": "List all proxies in LIFO (stack) order",
            "GET /all-proxy-points": "List all proxy IP:PORT combinations"
        }
    });

    HttpResponse::Ok().json(endpoints)
}

/// Simple health check
#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

/// Register a new proxy node
#[post("/register")]
async fn register(proxy: web::Json<ProxyNode>, data: web::Data<ProxyStack>) -> impl Responder {
    let mut stack = data.lock().unwrap();
    let mut new_proxy = proxy.into_inner();
    new_proxy.id = Uuid::new_v4();
    stack.push(new_proxy);
    HttpResponse::Ok().body("Proxy registered successfully")
}

/// Return the stack of all proxy nodes (LIFO order)
#[get("/stacked")]
async fn stacked(data: web::Data<ProxyStack>) -> impl Responder {
    let stack = data.lock().unwrap();
    let reversed: Vec<ProxyNode> = stack.iter().rev().cloned().collect();
    HttpResponse::Ok().json(reversed)
}

/// Show all proxy points as "ip:port"
#[get("/all-proxy-points")]
async fn proxy_points(data: web::Data<ProxyStack>) -> impl Responder {
    let stack = data.lock().unwrap();
    let points: Vec<String> = stack
        .iter()
        .map(|p| format!("{}:{}", p.ip, p.port))
        .collect();
    HttpResponse::Ok().json(points)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("Listening on: {}", addr);

    let stack: ProxyStack = Arc::new(Mutex::new(Vec::new()));

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(stack.clone()))
            .service(index)
            .service(health)
            .service(register)
            .service(stacked)
            .service(proxy_points)
    })
    .bind(addr)?
    .run()
    .await
}
