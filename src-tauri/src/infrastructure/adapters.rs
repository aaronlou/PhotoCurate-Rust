use crate::application::ports::{
    BookmarkGateway, DirectoryMonitor, EmbeddingService, PhotoFileGateway, ProgressReporter,
    ScoringService,
};
use crate::domain::models::{IndexingProgressEvent, Photo, ScoreResult};
use crate::error::Result;
use crate::infrastructure;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Mutex;

pub struct LocalPhotoFileGateway;

impl PhotoFileGateway for LocalPhotoFileGateway {
    async fn scan_directory(&self, dir_path: &str, dir_id: &str) -> Result<Vec<Photo>> {
        infrastructure::fs::scan_directory(dir_path, dir_id).await
    }

    async fn generate_thumbnail(
        &self,
        photo_path: &str,
        cache_dir: &Path,
        max_dimension: u32,
    ) -> Result<String> {
        infrastructure::fs::generate_thumbnail(photo_path, cache_dir, max_dimension).await
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)?;
        Ok(())
    }

    fn file_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn copy_file(&self, source: &Path, destination: &Path) -> Result<u64> {
        Ok(std::fs::copy(source, destination)?)
    }

    fn resolve_conflict(&self, path: &Path) -> PathBuf {
        crate::domain::services::resolve_conflict(path)
    }
}

pub struct BookmarkService;

impl BookmarkGateway for BookmarkService {
    fn create_bookmark(&self, path: &str) -> Option<Vec<u8>> {
        infrastructure::bookmarks::create_bookmark(path).ok()
    }

    fn resolve_bookmark(&self, data: &[u8]) -> std::result::Result<String, String> {
        infrastructure::bookmarks::resolve_bookmark(data)
    }

    fn release_access(&self, path: &str) {
        infrastructure::bookmarks::release_access(path);
    }
}

#[derive(Clone)]
pub struct NotifyDirectoryMonitor {
    monitors: Arc<Mutex<HashMap<String, (String, notify::RecommendedWatcher)>>>,
}

impl NotifyDirectoryMonitor {
    pub fn new(
        monitors: Arc<Mutex<HashMap<String, (String, notify::RecommendedWatcher)>>>,
    ) -> Self {
        Self { monitors }
    }
}

impl DirectoryMonitor for NotifyDirectoryMonitor {
    async fn start_monitoring(&self, path: &str, dir_id: &str) -> Result<()> {
        use notify::{RecursiveMode, Watcher};

        let path = path.to_string();
        let dir_id = dir_id.to_string();
        let mut watcher = notify::recommended_watcher(move |res| match res {
            Ok(event) => tracing::debug!("FS event: {:?}", event),
            Err(e) => tracing::error!("Watch error: {:?}", e),
        })?;

        watcher.watch(Path::new(&path), RecursiveMode::Recursive)?;

        let mut monitors = self.monitors.lock().await;
        monitors.insert(dir_id, (path, watcher));
        Ok(())
    }

    async fn stop_monitoring(&self, dir_id: &str) {
        let mut monitors = self.monitors.lock().await;
        if let Some((_, watcher)) = monitors.remove(dir_id) {
            drop(watcher);
        }
    }
}

#[derive(Clone)]
pub struct AiGateway {
    chinese_clip: Option<Arc<infrastructure::ai::ChineseClipService>>,
}

impl AiGateway {
    pub fn new(chinese_clip: Option<Arc<infrastructure::ai::ChineseClipService>>) -> Self {
        Self { chinese_clip }
    }
}

impl EmbeddingService for AiGateway {
    fn has_local_model(&self) -> bool {
        self.chinese_clip.is_some()
    }

    async fn embed_image(&self, api_key: &str, image_path: &str) -> Result<Vec<f64>> {
        infrastructure::ai::embed_image(api_key, image_path, self.chinese_clip.clone()).await
    }

    async fn embed_text(&self, api_key: &str, text: &str) -> Result<Vec<f64>> {
        infrastructure::ai::embed_text(api_key, text, self.chinese_clip.clone()).await
    }
}

impl ScoringService for AiGateway {
    async fn score_image(&self, api_key: &str, image_path: &str) -> Result<ScoreResult> {
        infrastructure::ai::score_image(api_key, image_path).await
    }

    async fn validate_api_key(&self, api_key: &str) -> Result<(bool, String)> {
        infrastructure::ai::validate_api_key(api_key).await
    }
}

#[derive(Clone)]
pub struct TauriProgressReporter {
    app_handle: tauri::AppHandle,
}

impl TauriProgressReporter {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }
}

impl ProgressReporter for TauriProgressReporter {
    fn indexing_progress(&self, event: IndexingProgressEvent) {
        let _ = self.app_handle.emit("indexing-progress", event);
    }

    fn scoring_progress(&self, event: IndexingProgressEvent) {
        let _ = self.app_handle.emit("scoring-progress", event);
    }
}
