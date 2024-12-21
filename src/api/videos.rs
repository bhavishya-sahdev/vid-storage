use crate::db::{models::Video, DbPool};
use crate::services::video_processor;
use actix_multipart::Multipart;
use actix_web::{web, Error, HttpResponse};
use diesel::{ExpressionMethods, QueryDsl};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/videos")
            .route("", web::post().to(upload_video))
            .route("", web::get().to(list_videos)), // .route("/{id}", web::get().to(get_video)),
    );
}

pub async fn upload_video(
    mut payload: Multipart,
    pool: web::Data<DbPool>,
) -> Result<HttpResponse, Error> {
    let video_id = Uuid::new_v4();
    let conn = &mut pool.get().await.expect("Failed to get DB connection");

    let video = Video {
        id: video_id,
        title: "Untitled".to_string(),
        description: None,
        duration: None,
        status: "uploading".to_string(),
        created_at: chrono::Utc::now().naive_utc(),
        updated_at: chrono::Utc::now().naive_utc(),
    };

    diesel::insert_into(crate::db::schema::videos::table)
        .values(&video)
        .execute(conn)
        .await
        .map_err(|_e| actix_web::error::ErrorInternalServerError("Database error"))?;

    video_processor::handle_upload(payload, video_id, pool).await?;

    Ok(HttpResponse::Ok().json(video))
}

pub async fn list_videos(pool: web::Data<DbPool>) -> Result<HttpResponse, Error> {
    use crate::db::schema::videos::dsl::*;
    let conn = &mut pool.get().await.expect("Failed to get DB connection");

    let video_list = videos
        .order_by(created_at.desc())
        .load::<Video>(conn)
        .await
        .expect("Failed to get video list");

    Ok(HttpResponse::Ok().json(video_list))
}
