use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, Clone)]
#[diesel(table_name = crate::db::schema::videos)]
pub struct Video {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub duration: Option<f64>,
    pub status: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, Clone)]
#[diesel(table_name = crate::db::schema::video_qualities)]
pub struct VideoQuality {
    pub id: Uuid,
    pub video_id: Uuid,
    pub resolution: String,
    pub bitrate: String,
    pub file_path: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct VideoWithMeta {
    #[serde(flatten)]
    pub video: Video,
    pub qualities: Vec<VideoQuality>,
    pub thumbnail_url: String,
    pub stream_url: String,
}
