use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub storage: StorageConfig,
    pub ffmpeg: FfmpegConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageConfig {
    pub upload_path: String,
    pub max_file_size: usize, // in bytes
}

#[derive(Debug, Deserialize, Clone)]
pub struct FfmpegConfig {
    pub thread_count: usize,
    pub preset: String,
}

impl AppConfig {
    pub fn new() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let s = Config::builder()
            // Start with default values
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 8080)?
            .set_default("database.max_connections", 5)?
            .set_default("storage.max_file_size", 1024 * 1024 * 1024)? // 1GB
            .set_default("ffmpeg.thread_count", 2)?
            .set_default("ffmpeg.preset", "fast")?
            // Layer on the environment-specific values
            .add_source(File::with_name(&format!("config/{}", run_mode)).required(false))
            // Add in settings from the environment
            // E.g. `SERVER__PORT=5001 ./target/app` would set `server.port`
            .add_source(Environment::with_prefix("APP").separator("__"))
            .build()?;

        // Deserialize the configuration
        s.try_deserialize()
    }

    pub fn from_env() -> Result<Self, ConfigError> {
        Self::new()
    }
}

// Add default implementation for configs
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://postgres:postgres@localhost/video_streaming".to_string(),
            max_connections: 5,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            upload_path: "uploads".to_string(),
            max_file_size: 1024 * 1024 * 1024, // 1GB
        }
    }
}

impl Default for FfmpegConfig {
    fn default() -> Self {
        Self {
            thread_count: 2,
            preset: "fast".to_string(),
        }
    }
}
