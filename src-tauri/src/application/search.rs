use crate::application::ports::{
    EmbeddingRepository, EmbeddingService, PhotoRepository, ProgressReporter, SettingsRepository,
    VectorIndexStore, VectorRepository,
};
use crate::domain::models::{IndexingProgressEvent, Photo, SearchResult};
use crate::error::{PhotoCurateError, Result};
use crate::infrastructure;
use crate::infrastructure::vector::SharedVectorIndex;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;

const EMBEDDING_VERSION: i32 = 1;
const SEARCH_LIMIT: usize = 50;
const SEARCH_MIN_SIMILARITY: f64 = 0.2;

pub async fn build_index(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    photo_ids: Vec<String>,
    allow_keychain_read: bool,
) -> Result<()> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let vector_repo = infrastructure::repositories::SqliteVectorRepository::new(db.clone());
    let embedding_repo = infrastructure::repositories::SqliteEmbeddingRepository::new(db.clone());
    let embeddings = infrastructure::adapters::AiGateway::new(chinese_clip.clone());
    let progress = crate::application::ports::NoopProgressReporter;

    build_index_with(
        &settings_repo,
        &photo_repo,
        &vector_repo,
        &embedding_repo,
        vector_index,
        &embeddings,
        &progress,
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
    photo_ids: Vec<String>,
    allow_keychain_read: bool,
) -> Result<()> {
    let selected_photos = find_photos_by_ids(photos, photo_ids).await?;
    index_photos(
        settings,
        photos,
        vectors,
        embedding_store,
        index,
        embeddings,
        progress,
        selected_photos,
        false,
        allow_keychain_read,
    )
    .await
    .map(|_| ())
}

pub async fn auto_index_unindexed(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    app_handle: &tauri::AppHandle,
) -> Result<()> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let vector_repo = infrastructure::repositories::SqliteVectorRepository::new(db.clone());
    let embedding_repo = infrastructure::repositories::SqliteEmbeddingRepository::new(db.clone());
    let embeddings = infrastructure::adapters::AiGateway::new(chinese_clip.clone());
    let progress = infrastructure::adapters::TauriProgressReporter::new(app_handle.clone());

    auto_index_unindexed_with(
        &settings_repo,
        &photo_repo,
        &vector_repo,
        &embedding_repo,
        vector_index,
        &embeddings,
        &progress,
        false,
    )
    .await
}

pub async fn auto_index_unindexed_with(
    settings: &impl SettingsRepository,
    photos: &impl PhotoRepository,
    vectors: &impl VectorRepository,
    embedding_store: &impl EmbeddingRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
    progress: &impl ProgressReporter,
    allow_keychain_read: bool,
) -> Result<()> {
    if automatic_embedding_unavailable(embeddings) {
        progress.indexing_progress(IndexingProgressEvent {
            current: 0,
            total: 0,
            status: "unavailable".into(),
        });
        return Ok(());
    }

    let unindexed = photos.find_unindexed().await?;
    index_photos(
        settings,
        photos,
        vectors,
        embedding_store,
        index,
        embeddings,
        progress,
        unindexed,
        true,
        allow_keychain_read,
    )
    .await
    .map(|_| ())
}

pub async fn rebuild_all_index(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    app_handle: &tauri::AppHandle,
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
        allow_keychain_read,
    )
    .await?;

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
    allow_keychain_read: bool,
) -> Result<Vec<GeneratedEmbedding>> {
    let api_key = resolve_embedding_api_key(settings_repo, embeddings, allow_keychain_read).await?;

    let total = photos_to_index.len();
    if emit_progress {
        progress.indexing_progress(IndexingProgressEvent {
            current: 0,
            total,
            status: if total == 0 { "complete" } else { "started" }.into(),
        });
    }

    let mut generated = Vec::new();
    for (i, photo) in photos_to_index.iter().enumerate() {
        match generate_embedding(&api_key, photo, embeddings).await {
            Ok(entry) => {
                tracing::info!(
                    "Generated embedding {}/{}: {}",
                    i + 1,
                    total,
                    photo.file_name
                );
                generated.push(entry);
            }
            Err(e) => {
                tracing::warn!("Embedding failed for {}: {}", photo.file_name, e);
            }
        }

        if emit_progress {
            progress.indexing_progress(IndexingProgressEvent {
                current: i + 1,
                total,
                status: "indexing".into(),
            });
        }
    }

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

async fn index_photos(
    settings_repo: &impl SettingsRepository,
    _photos: &impl PhotoRepository,
    _vectors: &impl VectorRepository,
    embedding_store: &impl EmbeddingRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
    progress: &impl ProgressReporter,
    photos_to_index: Vec<Photo>,
    emit_progress: bool,
    allow_keychain_read: bool,
) -> Result<usize> {
    let generated = generate_embeddings(
        settings_repo,
        embeddings,
        progress,
        &photos_to_index,
        emit_progress,
        allow_keychain_read,
    )
    .await?;

    let mut indexed_count = 0usize;
    for entry in &generated {
        match embedding_store
            .upsert_embedding(&entry.photo.id, &entry.vector_json, EMBEDDING_VERSION)
            .await
        {
            Ok(()) => {
                index
                    .add(entry.photo.id.clone(), entry.embedding.clone())
                    .await;
                indexed_count += 1;
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to persist embedding for {}: {}",
                    entry.photo.file_name,
                    e
                );
            }
        }
    }

    emit_indexing_complete(progress, photos_to_index.len(), emit_progress);
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

fn automatic_embedding_unavailable(embeddings: &impl EmbeddingService) -> bool {
    !embeddings.has_local_model()
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

fn emit_indexing_failed(progress: &impl ProgressReporter, total: usize, emit_progress: bool) {
    if emit_progress {
        progress.indexing_progress(IndexingProgressEvent {
            current: total,
            total,
            status: "failed".into(),
        });
    }
}
