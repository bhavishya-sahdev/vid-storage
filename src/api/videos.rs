use crate::db::models::{VideoQuality, VideoWithMeta};
use crate::db::{models::Video, DbPool};
use crate::services::video_processor;
use actix_multipart::Multipart;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use diesel::{ExpressionMethods, QueryDsl};
use diesel_async::RunQueryDsl;
use futures::TryStreamExt;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/videos")
            .route("", web::post().to(upload_video))
            .route("", web::get().to(list_videos)), // .route("/{id}", web::get().to(get_video)),
    );
}

#[derive(Deserialize, Debug)]
pub struct VideoMetadata {
    title: String,
    description: Option<String>,
}

pub async fn upload_video(
    payload: Multipart,
    pool: web::Data<DbPool>,
) -> Result<HttpResponse, Error> {
    let video_id = Uuid::new_v4();
    let conn = &mut pool.get().await.expect("Failed to get DB connection");

    let mut video_file: Option<(String, Vec<u8>)> = None;
    let mut metadata = VideoMetadata {
        title: "Untitled".to_string(),
        description: None,
    };

    let mut payload = payload;
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field
            .content_disposition()
            .expect("Failed to get content disposition");
        let field_name = content_disposition
            .get_name()
            .ok_or_else(|| actix_web::error::ErrorBadRequest("No field name"))?;

        match field_name {
            "video" => {
                let filename = content_disposition
                    .get_filename()
                    .ok_or_else(|| actix_web::error::ErrorBadRequest("No filename"))?
                    .to_owned();

                let mut video_data = Vec::new();
                while let Some(chunk) = field.try_next().await? {
                    video_data.extend_from_slice(&chunk);
                }
                video_file = Some((filename, video_data));
            }
            "title" => {
                let mut title = String::new();
                while let Some(chunk) = field.try_next().await? {
                    title.push_str(std::str::from_utf8(&chunk)?);
                }
                metadata.title = title;
            }
            "description" => {
                let mut description = String::new();
                while let Some(chunk) = field.try_next().await? {
                    description.push_str(std::str::from_utf8(&chunk)?);
                }
                metadata.description = Some(description);
            }
            _ => {
                // Skip unknown fields
                while (field.try_next().await?).is_some() {}
            }
        }
    }

    let (_filename, video_data) =
        video_file.ok_or_else(|| actix_web::error::ErrorBadRequest("No video file provided"))?;

    let video = Video {
        id: video_id,
        title: metadata.title,
        description: metadata.description,
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

    match video_processor::handle_upload(video_data, video_id, pool).await {
        Ok(_) => {
            diesel::update(crate::db::schema::videos::table)
                .filter(crate::db::schema::videos::id.eq(video_id))
                .set(crate::db::schema::videos::status.eq("processing"))
                .execute(conn)
                .await
                .map_err(|_e| actix_web::error::ErrorInternalServerError("Database error"))?;
        }
        Err(e) => {
            log::error!("Failed to handle upload: {}", e);
            diesel::update(crate::db::schema::videos::table)
                .filter(crate::db::schema::videos::id.eq(video_id))
                .set(crate::db::schema::videos::status.eq("failed"))
                .execute(conn)
                .await
                .map_err(|_e| actix_web::error::ErrorInternalServerError("Database error"))?;
            return Err(e);
        }
    }

    Ok(HttpResponse::Ok().json(video))
}

#[derive(Debug, Deserialize)]
pub struct ListQueryParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list_videos(
    req: HttpRequest,
    query: web::Query<ListQueryParams>,
    pool: web::Data<DbPool>,
) -> Result<HttpResponse, Error> {
    use crate::db::schema::{video_qualities, videos::dsl::*};
    let conn = &mut pool.get().await.expect("Failed to get DB connection");
    let base_url = format!(
        "{}://{}",
        req.connection_info().scheme(),
        req.connection_info().host()
    );

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(10).min(100); // Maximum 100 items per page
    let offset = (page - 1) * per_page;

    let video_list = videos
        .filter(status.eq("processed"))
        .order_by(created_at.desc())
        .offset(offset)
        .limit(per_page)
        .load::<Video>(conn)
        .await
        .map_err(|e| {
            eprintln!("Error loading videos: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    let video_ids: Vec<Uuid> = video_list.iter().map(|v| v.id).collect();
    let qualities = video_qualities::table
        .filter(video_qualities::video_id.eq_any(video_ids))
        .load::<VideoQuality>(conn)
        .await
        .map_err(|e| {
            eprintln!("Error loading video qualities: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    let videos_with_qualities: Vec<VideoWithMeta> = video_list
        .into_iter()
        .map(|video| {
            let video_qualities: Vec<VideoQuality> = qualities
                .iter()
                .filter(|q| q.video_id == video.id)
                .cloned()
                .collect();
            let video_id = video.id;
            VideoWithMeta {
                video,
                qualities: video_qualities,
                thumbnail_url: format!("{}/uploads/{}/thumbnails/thumb_0.jpg", base_url, video_id),
                stream_url: format!("{}/uploads/{}/hls/master.m3u8", base_url, video_id),
            }
        })
        .collect();

    let total_count: i64 = videos.count().get_result(conn).await.map_err(|e| {
        eprintln!("Error getting total count: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(json!({
        "videos": videos_with_qualities,
        "meta": {
            "total": total_count,
            "page": page,
            "per_page": per_page,
            "total_pages": (total_count as f64 / per_page as f64).ceil() as i64,
            "base": base_url,
        }
    })))
}
