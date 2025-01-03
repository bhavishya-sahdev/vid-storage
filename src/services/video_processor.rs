// src/services/video_processor.rs
use crate::db::models::VideoQuality;
use crate::db::DbPool;
use actix_web::{web, Error};
use anyhow::{Context, Result};
use chrono::Utc;
use diesel::ExpressionMethods;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use uuid::Uuid;

const CHUNK_DURATION: u32 = 6; // Duration of each HLS segment in seconds
const QUALITIES: &[(&str, &str)] = &[
    ("1080p", "5000k"),
    ("720p", "2800k"),
    ("480p", "1400k"),
    ("360p", "800k"),
];

pub async fn handle_upload(
    video_data: Vec<u8>,
    v_id: Uuid,
    pool: web::Data<DbPool>,
) -> Result<(), Error> {
    let upload_dir = get_video_dir(v_id);
    fs::create_dir_all(&upload_dir).await.map_err(|e| {
        log::error!("Failed to create upload directory: {}", e);
        actix_web::error::ErrorInternalServerError("Storage error")
    })?;

    let filepath = upload_dir.join("original.mp4");
    // Write the video data to file
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&filepath)
        .await
        .map_err(|e| {
            log::error!("Failed to open file: {}", e);
            actix_web::error::ErrorInternalServerError("Storage error")
        })?;

    f.write_all(&video_data).await.map_err(|e| {
        log::error!("Error writing file: {}", e);
        actix_web::error::ErrorInternalServerError("Storage error")
    })?;

    f.sync_all().await.map_err(|e| {
        log::error!("Error syncing file: {}", e);
        actix_web::error::ErrorInternalServerError("Storage error")
    })?;

    // Get video duration before processing
    if let Ok(duration) = get_video_duration(&filepath.to_string_lossy()).await {
        let conn = &mut pool.get().await.expect("Failed to get DB connection");
        diesel::update(crate::db::schema::videos::table)
            .filter(crate::db::schema::videos::id.eq(v_id))
            .set(crate::db::schema::videos::duration.eq(duration))
            .execute(conn)
            .await
            .map_err(|e| {
                log::error!("Error updating video duration: {}", e);
                actix_web::error::ErrorInternalServerError("Database error")
            })?;
    }

    // Spawn video processing
    let video_id_str = v_id.to_string();

    tokio::spawn(async move {
        let mut conn = pool.get().await.expect("Failed to get DB connection");
        if let Err(e) = process_video(&video_id_str, &mut conn).await {
            log::error!("Error processing video {}: {}", video_id_str, e);

            // Update status to failed if processing fails
            if let Err(db_err) = diesel::update(crate::db::schema::videos::table)
                .filter(crate::db::schema::videos::id.eq(Uuid::parse_str(&video_id_str).unwrap()))
                .set(crate::db::schema::videos::status.eq("failed"))
                .execute(&mut conn)
                .await
            {
                log::error!("Error updating video status: {}", db_err);
            }
        }
    });

    Ok(())
}

async fn process_video(v_id: &str, conn: &mut AsyncPgConnection) -> Result<()> {
    use crate::db::schema::videos;

    let video_dir = get_video_dir(Uuid::parse_str(v_id)?);
    let input_path = video_dir.join("original.mp4");
    let hls_dir = video_dir.join("hls");
    fs::create_dir_all(&hls_dir).await?;

    let mut master_playlist = String::from("#EXTM3U\n#EXT-X-VERSION:3\n");

    // Process each quality
    for &(quality, bitrate) in QUALITIES {
        let quality_dir = hls_dir.join(quality);
        fs::create_dir_all(&quality_dir).await?;
        let output_path = quality_dir.join("stream.m3u8");

        // Transcode to HLS
        match transcode_to_hls(&input_path, &output_path, bitrate, quality, CHUNK_DURATION).await {
            Ok(_) => {
                // Store successful transcoding in database
                let video_quality = VideoQuality {
                    id: Uuid::new_v4(),
                    video_id: Uuid::parse_str(v_id)?,
                    resolution: quality.to_string(),
                    bitrate: bitrate.to_string(),
                    file_path: format!("hls/{}/stream.m3u8", quality),
                    created_at: Utc::now().naive_utc(),
                };

                match diesel::insert_into(crate::db::schema::video_qualities::table)
                    .values(&video_quality)
                    .execute(conn)
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("Failed to update quality {e}")
                    }
                }

                // Add to master playlist
                let bandwidth = parse_bitrate(bitrate)?;
                let resolution = get_resolution(quality);
                master_playlist.push_str(&format!(
                    "#EXT-X-STREAM-INF:BANDWIDTH={},RESOLUTION={}\n{}/stream.m3u8\n",
                    bandwidth, resolution, quality
                ));
            }
            Err(e) => {
                log::error!("Failed to transcode quality {}: {}", quality, e);
                // Continue with other qualities even if one fails
                continue;
            }
        }
    }

    let uuid_vid_id = Uuid::parse_str(v_id).expect("Failed to parse video id into uuid");
    let path_str = input_path
        .as_os_str()
        .to_str()
        .expect("Failed to convert input path to string");
    let duration = get_video_duration(path_str)
        .await
        .expect("failed to get video duration");
    match diesel::update(videos::table)
        .filter(videos::id.eq(uuid_vid_id))
        .set((
            videos::status.eq("processed"),
            videos::duration.eq(Some(duration)),
        ))
        .execute(conn)
        .await
    {
        Ok(_) => {}
        Err(e) => {
            log::error!("Failed to update video status: {e}");
        }
    };

    // Write master playlist
    fs::write(hls_dir.join("master.m3u8"), master_playlist).await?;

    // Generate thumbnails
    generate_thumbnails(&input_path, &video_dir).await?;

    Ok(())
}

async fn transcode_to_hls(
    input: &Path,
    output: &Path,
    bitrate: &str,
    quality: &str,
    segment_duration: u32,
) -> Result<()> {
    let resolution = match quality {
        "1080p" => "1920x1080",
        "720p" => "1280x720",
        "480p" => "854x480",
        "360p" => "640x360",
        _ => return Err(anyhow::anyhow!("Invalid quality")),
    };

    let status = Command::new("ffmpeg")
        .arg("-i")
        .arg(input)
        .arg("-c:v")
        .arg("libx264")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:v")
        .arg(bitrate)
        .arg("-b:a")
        .arg("128k")
        .arg("-s")
        .arg(resolution)
        .arg("-preset")
        .arg("fast")
        .arg("-g")
        .arg("48")
        .arg("-sc_threshold")
        .arg("0")
        .arg("-keyint_min")
        .arg("48")
        .arg("-hls_time")
        .arg(segment_duration.to_string())
        .arg("-hls_playlist_type")
        .arg("vod")
        .arg("-loglevel")
        .arg("quiet")
        .arg("-hls_segment_filename")
        .arg(output.parent().unwrap().join("segment_%03d.ts"))
        .arg(output)
        .status()
        .await?;

    if !status.success() {
        return Err(anyhow::anyhow!("FFmpeg transcoding failed"));
    }

    Ok(())
}

async fn generate_thumbnails(input: &Path, output_dir: &Path) -> Result<()> {
    let thumbnails_dir = output_dir.join("thumbnails");
    fs::create_dir_all(&thumbnails_dir).await?;

    // Generate thumbnail every 10 seconds
    let status = Command::new("ffmpeg")
        .arg("-i")
        .arg(input)
        .arg("-vf")
        .arg("fps=1/10")
        .arg("-frame_pts")
        .arg("1")
        .arg("-vf")
        .arg("scale=320:-1")
        .arg("-loglevel")
        .arg("quiet")
        .arg(thumbnails_dir.join("thumb_%d.jpg"))
        .status()
        .await?;

    if !status.success() {
        return Err(anyhow::anyhow!("Thumbnail generation failed"));
    }

    Ok(())
}

fn get_video_dir(v_id: Uuid) -> PathBuf {
    PathBuf::from("uploads").join(v_id.to_string())
}

async fn get_video_duration(file_path: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            file_path,
        ])
        .output()
        .await?;

    let json: Value = serde_json::from_slice(&output.stdout)?;
    let duration = json["format"]["duration"]
        .as_str()
        .ok_or("No duration found")?
        .parse::<f64>()?;

    Ok(duration)
}

fn parse_bitrate(bitrate: &str) -> Result<u32> {
    let num = bitrate
        .trim_end_matches('k')
        .parse::<u32>()
        .context("Invalid bitrate format")?;
    Ok(num * 1000) // Convert to bits per second
}

fn get_resolution(quality: &str) -> String {
    match quality {
        "1080p" => "1920x1080",
        "720p" => "1280x720",
        "480p" => "854x480",
        "360p" => "640x360",
        _ => "640x360", // default
    }
    .to_string()
}
