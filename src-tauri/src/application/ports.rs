#![allow(async_fn_in_trait)]

use crate::domain::models::{
    AiSettings, Directory, IndexingProgressEvent, Photo, PhotoSortOrder, ScoreResult,
};
use crate::error::Result;
use std::path::{Path, PathBuf};

pub trait PhotoRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<Photo>>;
    async fn find_all(&self, sort_order: Option<&PhotoSortOrder>) -> Result<Vec<Photo>>;
    async fn insert_or_ignore(&self, photo: &Photo) -> Result<()>;
    async fn update_thumbnail(&self, id: &str, path: &str) -> Result<()>;
    async fn update_score(&self, id: &str, score: f64) -> Result<()>;
    async fn update_embedding(&self, id: &str, version: i32) -> Result<()>;
    async fn reset_all_embeddings(&self) -> Result<()>;
    async fn update_export_status(&self, id: &str) -> Result<()>;
    async fn find_by_ids(&self, ids: &[String]) -> Result<Vec<Photo>>;
    async fn find_unindexed(&self) -> Result<Vec<Photo>>;
}

pub trait DirectoryRepository {
    async fn find_by_path(&self, path: &str) -> Result<Option<Directory>>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Directory>>;
    async fn find_all(&self) -> Result<Vec<Directory>>;
    async fn save(&self, directory: &Directory) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn find_by_ids(&self, ids: &[String]) -> Result<Vec<Directory>>;
    async fn update_path(&self, id: &str, path: &str) -> Result<()>;
}

pub trait SettingsRepository {
    async fn get(&self) -> Result<AiSettings>;
    async fn update(&self, settings: &AiSettings) -> Result<()>;
}

pub trait VectorRepository {
    async fn find_all(&self) -> Result<Vec<(String, String)>>;
    async fn save(&self, id: &str, photo_id: &str, vector_json: &str) -> Result<()>;
    async fn delete_all(&self) -> Result<()>;
}

pub trait PhotoFileGateway {
    async fn scan_directory(&self, dir_path: &str, dir_id: &str) -> Result<Vec<Photo>>;
    async fn generate_thumbnail(
        &self,
        photo_path: &str,
        cache_dir: &Path,
        max_dimension: u32,
    ) -> Result<String>;
    fn create_dir_all(&self, path: &Path) -> Result<()>;
    fn file_exists(&self, path: &Path) -> bool;
    fn copy_file(&self, source: &Path, destination: &Path) -> Result<u64>;
    fn resolve_conflict(&self, path: &Path) -> PathBuf;
}

pub trait BookmarkGateway {
    fn create_bookmark(&self, path: &str) -> Option<Vec<u8>>;
    fn resolve_bookmark(&self, data: &[u8]) -> std::result::Result<String, String>;
    fn release_access(&self, path: &str);
}

pub trait DirectoryMonitor {
    async fn start_monitoring(&self, path: &str, dir_id: &str) -> Result<()>;
    async fn stop_monitoring(&self, dir_id: &str);
}

pub trait EmbeddingService {
    fn has_local_model(&self) -> bool;
    async fn embed_image(&self, api_key: &str, image_path: &str) -> Result<Vec<f64>>;
    async fn embed_text(&self, api_key: &str, text: &str) -> Result<Vec<f64>>;
}

pub trait ScoringService {
    async fn score_image(&self, settings: &AiSettings, image_path: &str) -> Result<ScoreResult>;
    async fn validate_api_key(&self, settings: &AiSettings) -> Result<(bool, String)>;
}

pub trait VectorIndexStore {
    async fn add(&self, photo_id: String, vector: Vec<f64>);
    async fn clear(&self);
    async fn search(&self, query: &[f64], top_k: usize, min_similarity: f64) -> Vec<(String, f64)>;
    async fn stats(&self) -> (usize, Option<usize>);
}

pub trait ProgressReporter {
    fn indexing_progress(&self, event: IndexingProgressEvent);
    fn scoring_progress(&self, event: IndexingProgressEvent);
}

pub struct NoopProgressReporter;

impl ProgressReporter for NoopProgressReporter {
    fn indexing_progress(&self, _event: IndexingProgressEvent) {}

    fn scoring_progress(&self, _event: IndexingProgressEvent) {}
}
