use std::path::PathBuf;
use std::str::FromStr;

use crate::api::shared::{parse_error, ResponseType};
use crate::db::models::{VideoQuality, VideoWithMeta};
use crate::db::{models::Video, DbPool};
use crate::services::video_processor;
use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use diesel::{BoolExpressionMethods, ExpressionMethods, QueryDsl};
use diesel_async::RunQueryDsl;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/videos")
            .route("", web::post().to(upload_video))
            .route("/{id}", web::get().to(video_details))
            .route("/{id}/master.m3u8", web::get().to(serve_master_playlist))
            .route(
                "/{id}/{quality}/playlist.m3u8",
                web::get().to(serve_quality_playlist),
            )
            .route(
                "/{video_id}/{quality}/{segment}",
                web::get().to(serve_segment),
            )
            .route("", web::get().to(list_videos)),
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

#[derive(Debug, Serialize)]
struct VideoWithThumbnail {
    #[serde(flatten)]
    pub video: Video,
    pub thumbnail_url: String,
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
    use crate::db::schema::videos::dsl::*;
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

    let videos_with_thumbnail: Vec<VideoWithThumbnail> = video_list
        .into_iter()
        .map(|video| {
            let video_id = video.id;
            VideoWithThumbnail {
                video,
                thumbnail_url: format!("{}/uploads/{}/thumbnails/thumb_0.jpg", base_url, video_id),
            }
        })
        .collect();

    let total_count: i64 = videos.count().get_result(conn).await.map_err(|e| {
        eprintln!("Error getting total count: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(json!({
        "videos": videos_with_thumbnail,
        "meta": {
            "total": total_count,
            "page": page,
            "per_page": per_page,
            "total_pages": (total_count as f64 / per_page as f64).ceil() as i64,
            "base": base_url,
        }
    })))
}

pub async fn video_details(
    req: HttpRequest,
    path: web::Path<String>,
    pool: web::Data<DbPool>,
) -> Result<HttpResponse, Error> {
    use crate::db::schema::{video_qualities, videos};
    let conn = &mut pool.get().await.expect("Failed to get DB connection");
    let video_id = match Uuid::from_str(&path.into_inner()) {
        Ok(v) => v,
        Err(_) => {
            return Err(parse_error(
                "video_id".to_string(),
                "Failed to parse video id".to_string(),
            ))
        }
    };
    let base_url = format!(
        "{}://{}",
        req.connection_info().scheme(),
        req.connection_info().host()
    );

    let video = match videos::table
        .filter(videos::id.eq(video_id).and(videos::status.eq("processed")))
        .first::<Video>(conn)
        .await
    {
        Ok(v) => v,
        Err(_) => {
            return Err(parse_error(
                "db_video_data".to_string(),
                "Failed to load video data".to_string(),
            ))
        }
    };

    let video_qualities = video_qualities::table
        .filter(video_qualities::video_id.eq(video_id))
        .load::<VideoQuality>(conn)
        .await
        .map_err(|e| {
            eprintln!("Error loading video qualities: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    Ok(
        HttpResponse::Ok().json(json!(ResponseType::<VideoWithMeta> {
            data: Some(VideoWithMeta {
                video,
                qualities: video_qualities,
                thumbnail_url: format!("{}/uploads/{}/thumbnails/thumb_0.jpg", base_url, video_id),
                stream_url: format!("{}/uploads/{}/hls/master.m3u8", base_url, video_id),
            }),
            error: None
        })),
    )
}

pub async fn serve_master_playlist(video_id: web::Path<Uuid>) -> Result<NamedFile, Error> {
    let path = PathBuf::from("uploads")
        .join(video_id.to_string())
        .join("hls")
        .join("master.m3u8");

    Ok(NamedFile::open(path)
        .map_err(|_| actix_web::error::ErrorNotFound("Playlist not found"))?
        // .set_content_type("application/vnd.apple.mpegurl")
        .use_last_modified(true))
}

pub async fn serve_quality_playlist(params: web::Path<(Uuid, String)>) -> Result<NamedFile, Error> {
    let (video_id, quality) = params.into_inner();
    let path = PathBuf::from("uploads")
        .join(video_id.to_string())
        .join("hls")
        .join(quality)
        .join("playlist.m3u8");

    Ok(NamedFile::open(path)
        .map_err(|_| actix_web::error::ErrorNotFound("Playlist not found"))?
        // .set_content_type("application/vnd.apple.mpegurl")
        .use_last_modified(true))
}

pub async fn serve_segment(params: web::Path<(Uuid, String, String)>) -> Result<NamedFile, Error> {
    let (video_id, quality, segment) = params.into_inner();
    let path = PathBuf::from("uploads")
        .join(video_id.to_string())
        .join("hls")
        .join(quality)
        .join(segment);

    Ok(NamedFile::open(path)
        .map_err(|_| actix_web::error::ErrorNotFound("Segment not found"))?
        .use_last_modified(true))
}
