use actix_web::{get, App, HttpServer, Responder, HttpResponse};

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Railway'de Actix çalışıyor!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Railway `PORT` ortam değişkeni verir
    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("Listening on: {}", addr);

    HttpServer::new(|| App::new().service(hello))
        .bind(addr)?
        .run()
        .await
}
