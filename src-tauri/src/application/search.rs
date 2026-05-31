use crate::application::ports::{
    EmbeddingRepository, EmbeddingService, PhotoRepository, ProgressReporter, SettingsRepository,
    VectorIndexStore, VectorRepository,
};
use crate::domain::models::{IndexingProgressEvent, Photo, SearchResult};
use crate::error::{PhotoCurateError, Result};
use crate::infrastructure;
use crate::infrastructure::vector::SharedVectorIndex;
use futures_util::stream::{self, StreamExt};
use sqlx::{Pool, Sqlite};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

const EMBEDDING_VERSION: i32 = 1;
const SEARCH_LIMIT: usize = 50;
const SEARCH_MIN_SIMILARITY: f64 = 0.2;
const LOCAL_INDEX_CONCURRENCY: usize = 2;
const REMOTE_INDEX_CONCURRENCY: usize = 4;

pub async fn build_index(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    app_handle: &tauri::AppHandle,
    indexing: &crate::application::bootstrap::IndexingCoordinator,
    photo_ids: Vec<String>,
    allow_keychain_read: bool,
) -> Result<usize> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let vector_repo = infrastructure::repositories::SqliteVectorRepository::new(db.clone());
    let embedding_repo = infrastructure::repositories::SqliteEmbeddingRepository::new(db.clone());
    let embeddings = infrastructure::adapters::AiGateway::new(chinese_clip.clone());
    let progress = infrastructure::adapters::TauriProgressReporter::new(app_handle.clone());

    build_index_with(
        &settings_repo,
        &photo_repo,
        &vector_repo,
        &embedding_repo,
        vector_index,
        &embeddings,
        &progress,
        Some(indexing),
        photo_ids,
        allow_keychain_read,
    )
    .await
}

pub async fn build_index_with(
    settings: &impl SettingsRepository,
    photos: &impl PhotoRepository,
    vectors: &impl VectorRepository,
    embedding_store: &impl EmbeddingRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
    progress: &impl ProgressReporter,
    indexing: Option<&crate::application::bootstrap::IndexingCoordinator>,
    photo_ids: Vec<String>,
    allow_keychain_read: bool,
) -> Result<usize> {
    let selected_photos = find_photos_by_ids(photos, photo_ids).await?;
    index_photos(
        settings,
        photos,
        vectors,
        embedding_store,
        index,
        embeddings,
        progress,
        indexing,
        selected_photos,
        true,
        allow_keychain_read,
    )
    .await
}

pub async fn rebuild_all_index(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    app_handle: &tauri::AppHandle,
    indexing: &crate::application::bootstrap::IndexingCoordinator,
    allow_keychain_read: bool,
) -> Result<usize> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let vector_repo = infrastructure::repositories::SqliteVectorRepository::new(db.clone());
    let embedding_repo = infrastructure::repositories::SqliteEmbeddingRepository::new(db.clone());
    let embeddings = infrastructure::adapters::AiGateway::new(chinese_clip.clone());
    let progress = infrastructure::adapters::TauriProgressReporter::new(app_handle.clone());

    rebuild_all_index_with(
        &settings_repo,
        &photo_repo,
        &vector_repo,
        &embedding_repo,
        vector_index,
        &embeddings,
        &progress,
        indexing,
        allow_keychain_read,
    )
    .await
}

pub async fn rebuild_all_index_with(
    settings: &impl SettingsRepository,
    photos: &impl PhotoRepository,
    vectors: &impl VectorRepository,
    embedding_store: &impl EmbeddingRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
    progress: &impl ProgressReporter,
    indexing: &crate::application::bootstrap::IndexingCoordinator,
    allow_keychain_read: bool,
) -> Result<usize> {
    let all_photos = photos.find_all(None).await?;
    if all_photos.is_empty() {
        index.clear().await;
        vectors.delete_all().await?;
        photos.reset_all_embeddings().await?;
        return Ok(0);
    }

    let generated = generate_embeddings(
        settings,
        embeddings,
        progress,
        &all_photos,
        true,
        Some(indexing),
        allow_keychain_read,
    )
    .await?;

    if indexing.is_cancelled().await {
        emit_indexing_cancelled(progress, all_photos.len(), true);
        return Ok(generated.len());
    }

    if generated.is_empty() {
        emit_indexing_failed(progress, all_photos.len(), true);
        return Err(PhotoCurateError::Other(
            "failed to generate any embeddings; existing index was left unchanged".into(),
        ));
    }

    let persisted: Vec<(String, String)> = generated
        .iter()
        .map(|entry| (entry.photo.id.clone(), entry.vector_json.clone()))
        .collect();

    if let Err(e) = embedding_store
        .replace_all_embeddings(&persisted, EMBEDDING_VERSION)
        .await
    {
        emit_indexing_failed(progress, all_photos.len(), true);
        return Err(e);
    }

    index.clear().await;
    for entry in &generated {
        index
            .add(entry.photo.id.clone(), entry.embedding.clone())
            .await;
    }

    emit_indexing_complete(progress, all_photos.len(), true);
    Ok(generated.len())
}

pub async fn natural_language_search(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    query: String,
    allow_keychain_read: bool,
) -> Result<Vec<SearchResult>> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let embeddings = infrastructure::adapters::AiGateway::new(chinese_clip.clone());

    natural_language_search_with(
        &settings_repo,
        &photo_repo,
        vector_index,
        &embeddings,
        query,
        allow_keychain_read,
    )
    .await
}

pub async fn reload_vector_index(db: &Pool<Sqlite>, index: &impl VectorIndexStore) -> Result<()> {
    let vector_repo = infrastructure::repositories::SqliteVectorRepository::new(db.clone());
    reload_vector_index_with(&vector_repo, index).await
}

pub async fn reload_vector_index_with(
    vectors: &impl VectorRepository,
    index: &impl VectorIndexStore,
) -> Result<()> {
    let rows = vectors.find_all().await?;
    index.clear().await;

    for (photo_id, vector_json) in rows {
        match serde_json::from_str::<Vec<f64>>(&vector_json) {
            Ok(vector) => index.add(photo_id, vector).await,
            Err(e) => tracing::warn!("Skipping invalid vector while reloading index: {}", e),
        }
    }

    Ok(())
}

pub async fn natural_language_search_with(
    settings: &impl SettingsRepository,
    photos: &impl PhotoRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
    query: String,
    allow_keychain_read: bool,
) -> Result<Vec<SearchResult>> {
    let api_key = resolve_embedding_api_key(settings, embeddings, allow_keychain_read).await?;

    let query_embedding = embeddings.embed_text(&api_key, &query).await?;
    let matches = index
        .search(&query_embedding, SEARCH_LIMIT, SEARCH_MIN_SIMILARITY)
        .await;

    let mut search_results = Vec::new();
    for (photo_id, similarity) in matches {
        if let Some(photo) = photos.find_by_id(&photo_id).await? {
            search_results.push(SearchResult { photo, similarity });
        }
    }

    Ok(search_results)
}

struct GeneratedEmbedding {
    photo: Photo,
    embedding: Vec<f64>,
    vector_json: String,
}

async fn generate_embeddings(
    settings_repo: &impl SettingsRepository,
    embeddings: &impl EmbeddingService,
    progress: &impl ProgressReporter,
    photos_to_index: &[Photo],
    emit_progress: bool,
    indexing: Option<&crate::application::bootstrap::IndexingCoordinator>,
    allow_keychain_read: bool,
) -> Result<Vec<GeneratedEmbedding>> {
    let api_key = resolve_embedding_api_key(settings_repo, embeddings, allow_keychain_read).await?;

    let total = photos_to_index.len();
    emit_indexing_started(progress, total, emit_progress);

    let completed = AtomicUsize::new(0);
    let concurrency = index_concurrency(embeddings);
    let generated = stream::iter(photos_to_index.iter().cloned())
        .map(|photo| {
            let api_key = api_key.as_str();
            let completed = &completed;
            async move {
                if let Some(indexing) = indexing {
                    if indexing.is_cancelled().await {
                        return None;
                    }
                }

                let result = generate_embedding(api_key, &photo, embeddings).await;
                let current = completed.fetch_add(1, Ordering::Relaxed) + 1;
                if emit_progress {
                    progress.indexing_progress(IndexingProgressEvent {
                        current,
                        total,
                        status: "indexing".into(),
                    });
                }

                Some((current, photo, result))
            }
        })
        .buffer_unordered(concurrency)
        .filter_map(|outcome| async move {
            let (current, photo, result) = outcome?;
            match result {
                Ok(entry) => {
                    tracing::info!(
                        "Generated embedding {}/{}: {}",
                        current,
                        total,
                        photo.file_name
                    );
                    Some(entry)
                }
                Err(e) => {
                    tracing::warn!("Embedding failed for {}: {}", photo.file_name, e);
                    None
                }
            }
        })
        .collect()
        .await;

    Ok(generated)
}

async fn generate_embedding(
    api_key: &str,
    photo: &Photo,
    embeddings: &impl EmbeddingService,
) -> Result<GeneratedEmbedding> {
    let embedding = embeddings.embed_image(api_key, &photo.file_path).await?;
    let vector_json = serde_json::to_string(&embedding)
        .map_err(|e| PhotoCurateError::InvalidData(e.to_string()))?;

    Ok(GeneratedEmbedding {
        photo: photo.clone(),
        embedding,
        vector_json,
    })
}

fn index_concurrency(embeddings: &impl EmbeddingService) -> usize {
    if embeddings.has_local_model() {
        LOCAL_INDEX_CONCURRENCY
    } else {
        REMOTE_INDEX_CONCURRENCY
    }
}

async fn indexing_cancelled(
    indexing: Option<&crate::application::bootstrap::IndexingCoordinator>,
) -> bool {
    if let Some(indexing) = indexing {
        indexing.is_cancelled().await
    } else {
        false
    }
}

async fn persist_generated_embedding(
    embedding_store: &impl EmbeddingRepository,
    index: &impl VectorIndexStore,
    entry: &GeneratedEmbedding,
) -> bool {
    match embedding_store
        .upsert_embedding(&entry.photo.id, &entry.vector_json, EMBEDDING_VERSION)
        .await
    {
        Ok(()) => {
            index
                .add(entry.photo.id.clone(), entry.embedding.clone())
                .await;
            true
        }
        Err(e) => {
            tracing::warn!(
                "Failed to persist embedding for {}: {}",
                entry.photo.file_name,
                e
            );
            false
        }
    }
}

async fn index_photos(
    settings_repo: &impl SettingsRepository,
    _photos: &impl PhotoRepository,
    _vectors: &impl VectorRepository,
    embedding_store: &impl EmbeddingRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
    progress: &impl ProgressReporter,
    indexing: Option<&crate::application::bootstrap::IndexingCoordinator>,
    photos_to_index: Vec<Photo>,
    emit_progress: bool,
    allow_keychain_read: bool,
) -> Result<usize> {
    let api_key = resolve_embedding_api_key(settings_repo, embeddings, allow_keychain_read).await?;
    let total = photos_to_index.len();
    emit_indexing_started(progress, total, emit_progress);

    let completed = AtomicUsize::new(0);
    let concurrency = index_concurrency(embeddings);
    let mut generated = stream::iter(photos_to_index.into_iter())
        .map(|photo| {
            let api_key = api_key.as_str();
            let completed = &completed;
            async move {
                if let Some(indexing) = indexing {
                    if indexing.is_cancelled().await {
                        return None;
                    }
                }

                let result = generate_embedding(api_key, &photo, embeddings).await;
                let current = completed.fetch_add(1, Ordering::Relaxed) + 1;
                if emit_progress {
                    progress.indexing_progress(IndexingProgressEvent {
                        current,
                        total,
                        status: "indexing".into(),
                    });
                }

                Some((current, photo, result))
            }
        })
        .buffer_unordered(concurrency);
    let mut indexed_count = 0usize;
    while let Some(outcome) = generated.next().await {
        let Some((current, photo, result)) = outcome else {
            continue;
        };

        match result {
            Ok(entry) => {
                tracing::info!(
                    "Generated embedding {}/{}: {}",
                    current,
                    total,
                    photo.file_name
                );
                if persist_generated_embedding(embedding_store, index, &entry).await {
                    indexed_count += 1;
                }
            }
            Err(e) => {
                tracing::warn!("Embedding failed for {}: {}", photo.file_name, e);
            }
        }
    }

    if indexing_cancelled(indexing).await {
        emit_indexing_cancelled(progress, total, emit_progress);
    } else {
        emit_indexing_complete(progress, total, emit_progress);
    }
    Ok(indexed_count)
}

async fn find_photos_by_ids(
    photos: &impl PhotoRepository,
    photo_ids: Vec<String>,
) -> Result<Vec<Photo>> {
    let mut selected = Vec::new();
    for photo_id in photo_ids {
        if let Some(photo) = photos.find_by_id(&photo_id).await? {
            selected.push(photo);
        }
    }
    Ok(selected)
}

async fn resolve_embedding_api_key(
    settings: &impl SettingsRepository,
    embeddings: &impl EmbeddingService,
    allow_keychain_read: bool,
) -> Result<String> {
    if embeddings.has_local_model() {
        return Ok(String::new());
    }

    let api_key =
        crate::application::scoring::resolve_api_key_from_settings(settings, allow_keychain_read)
            .await?;
    if api_key.is_empty() {
        return Err(PhotoCurateError::EmbeddingServiceMissing);
    }
    Ok(api_key)
}

fn emit_indexing_complete(progress: &impl ProgressReporter, total: usize, emit_progress: bool) {
    if emit_progress {
        progress.indexing_progress(IndexingProgressEvent {
            current: total,
            total,
            status: "complete".into(),
        });
    }
}

fn emit_indexing_started(progress: &impl ProgressReporter, total: usize, emit_progress: bool) {
    if emit_progress {
        progress.indexing_progress(IndexingProgressEvent {
            current: 0,
            total,
            status: if total == 0 { "complete" } else { "started" }.into(),
        });
    }
}

fn emit_indexing_cancelled(progress: &impl ProgressReporter, total: usize, emit_progress: bool) {
    if emit_progress {
        progress.indexing_progress(IndexingProgressEvent {
            current: total,
            total,
            status: "cancelled".into(),
        });
    }
}

fn emit_indexing_failed(progress: &impl ProgressReporter, total: usize, emit_progress: bool) {
    if emit_progress {
        progress.indexing_progress(IndexingProgressEvent {
            current: total,
            total,
            status: "failed".into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::NoopProgressReporter;
    use crate::domain::models::{AiSettings, Photo};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };
    use std::time::Duration;

    struct InMemorySettingsRepository {
        settings: AiSettings,
    }

    impl SettingsRepository for InMemorySettingsRepository {
        async fn get(&self) -> Result<AiSettings> {
            Ok(self.settings.clone())
        }

        async fn update(&self, _settings: &AiSettings) -> Result<()> {
            Ok(())
        }
    }

    struct InMemoryPhotoRepository {
        photos: HashMap<String, Photo>,
    }

    impl InMemoryPhotoRepository {
        fn new(photos: Vec<Photo>) -> Self {
            Self {
                photos: photos
                    .into_iter()
                    .map(|photo| (photo.id.clone(), photo))
                    .collect(),
            }
        }
    }

    impl PhotoRepository for InMemoryPhotoRepository {
        async fn find_by_id(&self, id: &str) -> Result<Option<Photo>> {
            Ok(self.photos.get(id).cloned())
        }

        async fn find_all(
            &self,
            _sort_order: Option<&crate::domain::models::PhotoSortOrder>,
        ) -> Result<Vec<Photo>> {
            Ok(self.photos.values().cloned().collect())
        }

        async fn insert_or_ignore(&self, _photo: &Photo) -> Result<()> {
            Ok(())
        }

        async fn update_thumbnail(&self, _id: &str, _path: &str) -> Result<()> {
            Ok(())
        }

        async fn update_score(&self, _id: &str, _score: f64) -> Result<()> {
            Ok(())
        }

        async fn update_embedding(&self, _id: &str, _version: i32) -> Result<()> {
            Ok(())
        }

        async fn reset_all_embeddings(&self) -> Result<()> {
            Ok(())
        }

        async fn update_export_status(&self, _id: &str) -> Result<()> {
            Ok(())
        }

        async fn find_by_ids(&self, ids: &[String]) -> Result<Vec<Photo>> {
            Ok(ids
                .iter()
                .filter_map(|id| self.photos.get(id).cloned())
                .collect())
        }

        async fn find_unindexed(&self) -> Result<Vec<Photo>> {
            Ok(self.photos.values().cloned().collect())
        }
    }

    struct InMemoryEmbeddingRepository {
        saved: Mutex<Vec<String>>,
    }

    impl InMemoryEmbeddingRepository {
        fn new() -> Self {
            Self {
                saved: Mutex::new(vec![]),
            }
        }

        fn saved_count(&self) -> usize {
            self.saved.lock().unwrap().len()
        }
    }

    impl EmbeddingRepository for InMemoryEmbeddingRepository {
        async fn upsert_embedding(
            &self,
            photo_id: &str,
            _vector_json: &str,
            _version: i32,
        ) -> Result<()> {
            self.saved.lock().unwrap().push(photo_id.to_string());
            Ok(())
        }

        async fn replace_all_embeddings(
            &self,
            embeddings: &[(String, String)],
            _version: i32,
        ) -> Result<()> {
            self.saved
                .lock()
                .unwrap()
                .extend(embeddings.iter().map(|(photo_id, _)| photo_id.clone()));
            Ok(())
        }
    }

    struct NoopVectorRepository;

    impl VectorRepository for NoopVectorRepository {
        async fn find_all(&self) -> Result<Vec<(String, String)>> {
            Ok(vec![])
        }

        async fn save(&self, _id: &str, _photo_id: &str, _vector_json: &str) -> Result<()> {
            Ok(())
        }

        async fn delete_all(&self) -> Result<()> {
            Ok(())
        }
    }

    struct InMemoryVectorIndex {
        ids: Mutex<Vec<String>>,
    }

    impl InMemoryVectorIndex {
        fn new() -> Self {
            Self {
                ids: Mutex::new(vec![]),
            }
        }
    }

    impl VectorIndexStore for InMemoryVectorIndex {
        async fn add(&self, photo_id: String, _vector: Vec<f64>) {
            self.ids.lock().unwrap().push(photo_id);
        }

        async fn clear(&self) {
            self.ids.lock().unwrap().clear();
        }

        async fn search(
            &self,
            _query: &[f64],
            _top_k: usize,
            _min_similarity: f64,
        ) -> Vec<(String, f64)> {
            vec![]
        }

        async fn stats(&self) -> (usize, Option<usize>) {
            (self.ids.lock().unwrap().len(), Some(2))
        }
    }

    struct ConcurrentEmbeddingService {
        in_flight: AtomicUsize,
        max_in_flight: AtomicUsize,
    }

    impl ConcurrentEmbeddingService {
        fn new() -> Self {
            Self {
                in_flight: AtomicUsize::new(0),
                max_in_flight: AtomicUsize::new(0),
            }
        }

        fn max_in_flight(&self) -> usize {
            self.max_in_flight.load(Ordering::SeqCst)
        }
    }

    impl EmbeddingService for ConcurrentEmbeddingService {
        fn has_local_model(&self) -> bool {
            false
        }

        async fn embed_image(&self, _api_key: &str, image_path: &str) -> Result<Vec<f64>> {
            let current = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_in_flight.fetch_max(current, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(25)).await;
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            Ok(vec![image_path.len() as f64, 1.0])
        }

        async fn embed_text(&self, _api_key: &str, _text: &str) -> Result<Vec<f64>> {
            Ok(vec![1.0, 0.0])
        }
    }

    fn settings() -> AiSettings {
        AiSettings {
            id: "default".to_string(),
            provider: "gemini".to_string(),
            api_key: "secret".to_string(),
            has_api_key: true,
            key_storage: "memory".to_string(),
            scoring_provider: "gemini".to_string(),
            scoring_model: "gemini-test".to_string(),
            scoring_base_url: String::new(),
            scoring_api_key: String::new(),
            has_scoring_api_key: false,
            scoring_key_storage: "memory".to_string(),
            ollama_base_url: String::new(),
            ollama_embed_model: String::new(),
            ollama_vision_model: String::new(),
        }
    }

    fn photo(id: &str) -> Photo {
        Photo {
            id: id.to_string(),
            file_path: format!("/tmp/{id}.jpg"),
            file_name: format!("{id}.jpg"),
            file_size: 100,
            date_created: None,
            date_modified: Utc::now(),
            camera_make: None,
            camera_model: None,
            lens_model: None,
            focal_length: None,
            aperture: None,
            shutter_speed: None,
            iso: None,
            width: None,
            height: None,
            aesthetic_score: None,
            has_been_scored: false,
            score_date: None,
            has_embedding: false,
            embedding_version: None,
            thumbnail_path: None,
            directory_id: Some("dir".to_string()),
            has_been_exported: false,
            export_date: None,
            latest_evaluation: None,
        }
    }

    #[tokio::test]
    async fn build_index_runs_with_bounded_concurrency_and_persists_each_result() {
        let photos = (0..8)
            .map(|index| photo(&format!("photo-{index}")))
            .collect::<Vec<_>>();
        let photo_ids = photos.iter().map(|photo| photo.id.clone()).collect();
        let settings_repo = InMemorySettingsRepository {
            settings: settings(),
        };
        let photo_repo = InMemoryPhotoRepository::new(photos);
        let vector_repo = NoopVectorRepository;
        let embedding_repo = InMemoryEmbeddingRepository::new();
        let vector_index = InMemoryVectorIndex::new();
        let embeddings = ConcurrentEmbeddingService::new();

        let indexed = build_index_with(
            &settings_repo,
            &photo_repo,
            &vector_repo,
            &embedding_repo,
            &vector_index,
            &embeddings,
            &NoopProgressReporter,
            None,
            photo_ids,
            false,
        )
        .await
        .expect("indexing should succeed");

        assert_eq!(indexed, 8);
        assert_eq!(embedding_repo.saved_count(), 8);
        assert!(embeddings.max_in_flight() > 1);
        assert!(embeddings.max_in_flight() <= REMOTE_INDEX_CONCURRENCY);
    }

    #[tokio::test]
    async fn build_index_reports_clear_message_when_embedding_service_is_unavailable() {
        let selected_photo = photo("photo-without-service");
        let settings_repo = InMemorySettingsRepository {
            settings: AiSettings {
                api_key: String::new(),
                has_api_key: false,
                ..settings()
            },
        };
        let photo_repo = InMemoryPhotoRepository::new(vec![selected_photo.clone()]);
        let vector_repo = NoopVectorRepository;
        let embedding_repo = InMemoryEmbeddingRepository::new();
        let vector_index = InMemoryVectorIndex::new();
        let embeddings = ConcurrentEmbeddingService::new();

        let error = build_index_with(
            &settings_repo,
            &photo_repo,
            &vector_repo,
            &embedding_repo,
            &vector_index,
            &embeddings,
            &NoopProgressReporter,
            None,
            vec![selected_photo.id],
            false,
        )
        .await
        .expect_err("indexing should explain missing service");

        let message = error.to_string();
        assert!(message.contains(
            "Smart Search needs either the local Chinese-CLIP model or a Gemini API Key"
        ));
        assert_eq!(embedding_repo.saved_count(), 0);
    }
}
