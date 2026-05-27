use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Directory {
    pub id: String,
    pub path: String,
    pub is_monitoring: bool,
    pub date_added: DateTime<Utc>,
    pub bookmark_data: Option<Vec<u8>>,
}

impl Directory {
    pub fn new(id: String, path: String, bookmark_data: Option<Vec<u8>>) -> Self {
        Self {
            id,
            path,
            is_monitoring: true,
            date_added: Utc::now(),
            bookmark_data,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: i64,
    pub date_created: Option<DateTime<Utc>>,
    pub date_modified: DateTime<Utc>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub focal_length: Option<f64>,
    pub aperture: Option<f64>,
    pub shutter_speed: Option<f64>,
    pub iso: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub aesthetic_score: Option<f64>,
    pub has_been_scored: bool,
    pub score_date: Option<DateTime<Utc>>,
    pub has_embedding: bool,
    pub embedding_version: Option<i32>,
    pub thumbnail_path: Option<String>,
    pub directory_id: Option<String>,
    pub has_been_exported: bool,
    pub export_date: Option<DateTime<Utc>>,
}

impl Photo {
    pub fn extension(&self) -> Option<String> {
        self.file_path.rsplit('.').next().map(|s| s.to_lowercase())
    }

    pub fn is_raw(&self) -> bool {
        matches!(
            self.extension().as_deref(),
            Some("cr2" | "cr3" | "nef" | "arw" | "dng" | "raw")
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    pub id: String,
    pub provider: String,
    pub api_key: String,
    pub has_api_key: bool,
    pub key_storage: String,
    pub scoring_provider: String,
    pub scoring_model: String,
    pub scoring_base_url: String,
    pub scoring_api_key: String,
    pub has_scoring_api_key: bool,
    pub scoring_key_storage: String,
    pub ollama_base_url: String,
    pub ollama_embed_model: String,
    pub ollama_vision_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreResult {
    pub score: f64,
    pub review: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub photo: Photo,
    pub similarity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub exported_count: usize,
    pub failed_count: usize,
    pub failed_photos: Vec<ExportFailure>,
}

impl ExportResult {
    pub fn empty() -> Self {
        Self {
            exported_count: 0,
            failed_count: 0,
            failed_photos: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFailure {
    pub id: String,
    pub file_name: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingProgressEvent {
    pub current: usize,
    pub total: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhotoSortOrder {
    #[serde(rename = "date_desc")]
    DateDesc,
    #[serde(rename = "score_desc")]
    ScoreDesc,
    #[serde(rename = "score_asc")]
    ScoreAsc,
}
