use actix_files::Files;
use actix_web::{web, App, HttpServer};
use dotenv::dotenv;
use std::sync::Arc;

mod api;
mod config;
mod db;
mod services;
// mod utils;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env file if it exists
    dotenv().ok();

    // Initialize logger
    env_logger::init();

    // Load configuration
    let config = config::AppConfig::new().expect("Failed to load configuration");
    let config = Arc::new(config);

    log::info!(
        "Starting server on {}:{}",
        config.server.host,
        config.server.port
    );

    // Create upload directory if it doesn't exist
    tokio::fs::create_dir_all(&config.storage.upload_path)
        .await
        .expect("Failed to create upload directory");

    // Create DB pool
    let pool = db::create_pool(&config.database.url).await;

    let c = config.clone();
    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .service(Files::new("/uploads", "uploads/").show_files_listing())
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(c.clone()))
            .wrap(actix_cors::Cors::permissive()) // Configure properly in production
            .configure(api::configure)
    })
    .bind((config.server.host.clone(), config.server.port))?
    .run()
    .await
}
