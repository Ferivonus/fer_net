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
    let html = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Ferivonus Proxy API</title>
        <style>
            body {
                background-color: #0d0d0d;
                color: #00ffcc;
                font-family: monospace;
                padding: 40px;
            }
            h1 {
                color: #ff00ff;
            }
            ul {
                list-style-type: square;
            }
            li {
                margin-bottom: 10px;
            }
            code {
                background: #1a1a1a;
                padding: 2px 6px;
                border-radius: 4px;
                color: #00ffcc;
            }
        </style>
    </head>
    <body>
        <h1>Ferivonus Proxy Network API</h1>
        <p>Welcome to the API hub. Here are the available endpoints:</p>
        <ul>
            <li><code>GET /</code> – This help page</li>
            <li><code>GET /health</code> – Health check (returns OK)</li>
            <li><code>POST /register</code> – Register a new proxy node</li>
            <li><code>GET /stacked</code> – List all proxies in LIFO (stack) order</li>
            <li><code>GET /all-proxy-points</code> – Show all proxy endpoints as ip:port</li>
        </ul>
        <p style="margin-top: 40px; font-size: 12px;">Ferivonus Proxy System - powered by Rust + Actix Web</p>
    </body>
    </html>
    "#;

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
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
