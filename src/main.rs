use actix::*;
use actix_web::{get, post, web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegisteredNode {
    id: Uuid,
    password: String,
    mac_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProxyNode {
    id: Uuid,
    name: String,
    ip: String,
    port: u16,
    active: bool,
    mac_id: String,
}

type RegisteredNodes = Arc<Mutex<HashMap<Uuid, RegisteredNode>>>;
type ActiveNodes = Arc<Mutex<HashMap<Uuid, ProxyNode>>>;

#[derive(Deserialize)]
struct RegisterRequest {
    id: Uuid,
    password: String,
    mac_id: String,
    api_key: String,
}

#[post("/register")]
async fn register(
    reg: web::Json<RegisterRequest>,
    data: web::Data<RegisteredNodes>,
) -> impl Responder {
    let expected_api_key = env::var("API_KEY").unwrap_or_default();
    if reg.api_key != expected_api_key {
        return HttpResponse::Unauthorized().body("Invalid API key");
    }

    let mut reg_nodes = data.lock().await;

    if reg_nodes.contains_key(&reg.id) {
        return HttpResponse::BadRequest().body("ID already registered");
    }

    let node = RegisteredNode {
        id: reg.id,
        password: reg.password.clone(),
        mac_id: reg.mac_id.clone(),
    };

    reg_nodes.insert(reg.id, node);
    HttpResponse::Ok().body("Registered successfully")
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum WsMessage {
    Auth { id: Uuid, password: String },
    SetAddress { ip: String, port: u16 },
}

struct ProxyWsSession {
    id: Uuid,
    nodes: ActiveNodes,
    reg_nodes: RegisteredNodes,
    authed: bool,
    mac_id: String,
}

impl Actor for ProxyWsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        if !self.authed {
            ctx.text("Authentication required");
            ctx.close(None);
            ctx.stop();
            return;
        }

        let proxy_node = ProxyNode {
            id: self.id,
            name: format!("node-{}", &self.id.to_string()[..8]),
            ip: "unknown".to_string(),
            port: 0,
            active: true,
            mac_id: self.mac_id.clone(),
        };

        let mut guard = self.nodes.try_lock();
        if let Ok(ref mut map) = guard {
            map.insert(self.id, proxy_node);
        }
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        let mut guard = self.nodes.try_lock();
        if let Ok(ref mut map) = guard {
            map.remove(&self.id);
        }
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ProxyWsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => match serde_json::from_str::<WsMessage>(&text) {
                Ok(WsMessage::Auth { id, password }) => {
                    if self.authed {
                        ctx.text("Already authenticated");
                        return;
                    }
                    let guard = self.reg_nodes.try_lock();
                    if let Ok(reg_nodes) = guard {
                        if let Some(reg_node) = reg_nodes.get(&id) {
                            if reg_node.password == password {
                                self.authed = true;
                                self.id = id;
                                self.mac_id = reg_node.mac_id.clone();
                                ctx.text("Authenticated");
                                return;
                            }
                        }
                    }
                    ctx.text("Authentication failed");
                    ctx.close(None);
                    ctx.stop();
                }
                Ok(WsMessage::SetAddress { ip, port }) => {
                    if self.authed {
                        let mut guard = self.nodes.try_lock();
                        if let Ok(ref mut map) = guard {
                            if let Some(node) = map.get_mut(&self.id) {
                                node.ip = ip;
                                node.port = port;
                                ctx.text("Address updated");
                                return;
                            }
                        }
                    } else {
                        ctx.text("Not authenticated");
                    }
                }
                Err(_) => {
                    ctx.text("Invalid message format");
                }
            },
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Pong(_)) => (),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => (),
        }
    }
}

#[get("/ws/")]
async fn ws_index(
    req: HttpRequest,
    stream: web::Payload,
    active_nodes: web::Data<ActiveNodes>,
    registered_nodes: web::Data<RegisteredNodes>,
) -> Result<HttpResponse, Error> {
    let session = ProxyWsSession {
        id: Uuid::new_v4(),
        nodes: active_nodes.get_ref().clone(),
        reg_nodes: registered_nodes.get_ref().clone(),
        authed: false,
        mac_id: String::new(),
    };

    ws::start(session, &req, stream)
}

#[get("/nodes")]
async fn nodes_endpoint(data: web::Data<ActiveNodes>) -> impl Responder {
    let guard = data.lock().await;
    let list: Vec<ProxyNode> = guard.values().cloned().collect();
    HttpResponse::Ok().json(list)
}

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

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
        <p>Available endpoints:</p>
        <ul>
            <li><code>GET /</code> - This page</li>
            <li><code>GET /health</code> - Health check</li>
            <li><code>POST /register</code> - Register proxy node (id, password, mac_id)</li>
            <li><code>GET /ws/</code> - WebSocket for proxy nodes (requires auth message)</li>
            <li><code>GET /nodes</code> - List active proxy nodes</li>
        </ul>
    </body>
    </html>
    "#;

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    let port = env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("Listening on: {}", addr);

    let registered_nodes: RegisteredNodes = Arc::new(Mutex::new(HashMap::new()));
    let active_nodes: ActiveNodes = Arc::new(Mutex::new(HashMap::new()));

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(registered_nodes.clone()))
            .app_data(web::Data::new(active_nodes.clone()))
            .service(index)
            .service(health)
            .service(register)
            .service(ws_index)
            .service(nodes_endpoint)
    })
    .bind(addr)?
    .run()
    .await
}
