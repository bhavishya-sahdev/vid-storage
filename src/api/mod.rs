// src/api/mod.rs
pub mod health;
pub mod videos;

use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .configure(videos::configure)
            .configure(health::configure),
    );
}
