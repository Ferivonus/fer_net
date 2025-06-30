use actix::*; // Core Actix actor framework
use actix_web::{get, post, web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws; // WebSocket support for Actix Web
use serde::{Deserialize, Serialize}; // For serialization/deserialization
use std::collections::HashMap; // For HashMap data structure
use std::sync::{Arc, Mutex}; // For thread-safe shared state
use uuid::Uuid; // For unique identifiers // Required for StreamHandler trait

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegisteredNode {
    id: Uuid,
    password: String,
    mc_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProxyNode {
    id: Uuid,
    name: String,
    ip: String,
    port: u16,
    active: bool,
    mc_id: String,
}

// Type aliases for shared, thread-safe data structures
type RegisteredNodes = Arc<Mutex<HashMap<Uuid, RegisteredNode>>>;
type ActiveNodes = Arc<Mutex<HashMap<Uuid, ProxyNode>>>;

/// Data model for registration requests (id and password are required)
#[derive(Deserialize)]
struct RegisterRequest {
    id: Uuid,
    password: String,
    mc_id: String,
}

/// Handles the registration of new proxy nodes.
/// Expects a JSON payload with id, password, and mc_id.
#[post("/register")]
async fn register(
    reg: web::Json<RegisterRequest>,
    data: web::Data<RegisteredNodes>,
) -> impl Responder {
    // Acquire a lock on the registered nodes HashMap
    let mut reg_nodes = data.lock().unwrap();

    // Check if the ID is already registered
    if reg_nodes.contains_key(&reg.id) {
        return HttpResponse::BadRequest().body("ID already registered");
    }

    // Create a new RegisteredNode
    let node = RegisteredNode {
        id: reg.id,
        password: reg.password.clone(),
        mc_id: reg.mc_id.clone(),
    };

    // Insert the new node into the HashMap
    reg_nodes.insert(reg.id, node);
    HttpResponse::Ok().body("Registered successfully")
}

/// WebSocket handler for proxy nodes.
/// Each connected node will have an instance of this actor.
struct ProxyWsSession {
    id: Uuid,                   // Unique ID for this session/node
    nodes: ActiveNodes,         // Shared state for active nodes
    reg_nodes: RegisteredNodes, // Shared state for registered nodes
    authed: bool,               // Authentication status of the session
    mc_id: String,              // Minecraft ID associated with the node
}

impl Actor for ProxyWsSession {
    type Context = ws::WebsocketContext<Self>;

    /// Called when the actor starts (WebSocket connection established).
    fn started(&mut self, ctx: &mut Self::Context) {
        // If not authenticated upon starting, close the connection immediately.
        if !self.authed {
            println!("Unauthenticated WS connection, closing");
            ctx.text("Authentication required");
            ctx.close(None);
            ctx.stop();
            return;
        }

        println!("WebSocket connected: {}", self.id);

        // Create a ProxyNode entry for the newly connected and authenticated node
        let proxy_node = ProxyNode {
            id: self.id,
            name: format!("node-{}", &self.id.to_string()[..8]), // Generate a short name
            ip: "unknown".to_string(), // IP will be updated later if provided
            port: 0,                   // Port will be updated later if provided
            active: true,
            mc_id: self.mc_id.clone(),
        };

        // Insert the active node into the shared HashMap
        self.nodes.lock().unwrap().insert(self.id, proxy_node);
    }

    /// Called when the actor stops (WebSocket connection closed).
    fn stopped(&mut self, _: &mut Self::Context) {
        println!("WebSocket disconnected: {}", self.id);
        // Remove the disconnected node from the active nodes HashMap
        self.nodes.lock().unwrap().remove(&self.id);
    }
}

// Implements message handling for the WebSocket session.
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ProxyWsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                println!("WS message from {}: {}", self.id, text);

                // Handle authentication for the first message received
                if !self.authed {
                    if let Ok(auth) = serde_json::from_str::<AuthMessage>(&text) {
                        let reg_nodes = self.reg_nodes.lock().unwrap();
                        if let Some(reg_node) = reg_nodes.get(&auth.id) {
                            // Check if the provided password matches the registered node's password
                            if reg_node.password == auth.password {
                                self.authed = true;
                                ctx.text("Authenticated");
                                println!("Node {} authenticated", auth.id);
                                self.id = auth.id; // Assign the actual authenticated ID
                                self.mc_id = reg_node.mc_id.clone(); // Assign the mc_id
                                                                     // Additional node information (like IP/port) could be set here based on the auth message
                                return; // Stop processing this message, authentication is done
                            }
                        }
                        // If authentication fails (ID not found or password mismatch)
                        ctx.text("Authentication failed");
                        ctx.close(None);
                        ctx.stop();
                    } else {
                        // If the first message is not a valid AuthMessage
                        ctx.text("Invalid authentication message");
                        ctx.close(None);
                        ctx.stop();
                    }
                    return; // Stop processing if authentication is still pending
                }

                // Process other messages from authenticated clients here
                // Example: ctx.text(format!("Echo: {}", text));
            }
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg), // Respond to Ping with Pong
            Ok(ws::Message::Pong(_)) => (),               // Ignore Pong messages
            Ok(ws::Message::Binary(_)) => {
                // Handle binary messages if needed
                println!("Received binary message from {}", self.id);
            }
            Ok(ws::Message::Close(reason)) => {
                println!("WS closed {}: {:?}", self.id, reason);
                ctx.close(reason);
                ctx.stop();
            }
            _ => (), // Ignore other message types
        }
    }
}

/// WebSocket authentication message structure
#[derive(Deserialize)]
struct AuthMessage {
    id: Uuid,
    password: String,
}

/// Handles the initial WebSocket connection request.
#[get("/ws/")]
async fn ws_index(
    req: HttpRequest,
    stream: web::Payload,
    active_nodes: web::Data<ActiveNodes>,
    registered_nodes: web::Data<RegisteredNodes>,
) -> Result<HttpResponse, Error> {
    // Assign a temporary ID initially; the real ID will be set upon authentication
    let temp_id = Uuid::new_v4();

    let session = ProxyWsSession {
        id: temp_id,
        nodes: active_nodes.get_ref().clone(),
        reg_nodes: registered_nodes.get_ref().clone(),
        authed: false, // Session starts unauthenticated
        mc_id: String::new(),
    };

    // Start the WebSocket handshake and actor
    ws::start(session, &req, stream)
}

/// Retrieves a list of all currently active proxy nodes.
#[get("/nodes")]
async fn nodes(data: web::Data<ActiveNodes>) -> impl Responder {
    let nodes = data.lock().unwrap();
    let list: Vec<ProxyNode> = nodes.values().cloned().collect(); // Collect active nodes into a Vec
    HttpResponse::Ok().json(list) // Return as JSON
}

/// Health check endpoint.
#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

/// The root endpoint, serving an informational HTML page.
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
            <li><code>POST /register</code> - Register proxy node (id, password, mc_id)</li>
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

/// Main function to set up and run the Actix Web server.
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables from a .env file
    dotenv::dotenv().ok();
    // Get the port from environment variables, default to 8000
    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("Listening on: {}", addr);

    // Initialize shared state for registered and active nodes
    let registered_nodes: RegisteredNodes = Arc::new(Mutex::new(HashMap::new()));
    let active_nodes: ActiveNodes = Arc::new(Mutex::new(HashMap::new()));

    // Create and run the HTTP server
    HttpServer::new(move || {
        App::new()
            // Register shared application data
            .app_data(web::Data::new(registered_nodes.clone()))
            .app_data(web::Data::new(active_nodes.clone()))
            // Register all service endpoints
            .service(index)
            .service(health)
            .service(register)
            .service(ws_index)
            .service(nodes)
    })
    .bind(addr)? // Bind the server to the specified address
    .run() // Run the server
    .await // Wait for the server to shut down
}
