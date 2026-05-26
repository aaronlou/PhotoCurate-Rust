use crate::application::ports::{
    EmbeddingService, PhotoRepository, ProgressReporter, SettingsRepository, VectorIndexStore,
    VectorRepository,
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
) -> Result<()> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let vector_repo = infrastructure::repositories::SqliteVectorRepository::new(db.clone());
    let embeddings = infrastructure::adapters::AiGateway::new(chinese_clip.clone());
    let progress = crate::application::ports::NoopProgressReporter;

    build_index_with(
        &settings_repo,
        &photo_repo,
        &vector_repo,
        vector_index,
        &embeddings,
        &progress,
        photo_ids,
    )
    .await
}

pub async fn build_index_with(
    settings: &impl SettingsRepository,
    photos: &impl PhotoRepository,
    vectors: &impl VectorRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
    progress: &impl ProgressReporter,
    photo_ids: Vec<String>,
) -> Result<()> {
    let selected_photos = find_photos_by_ids(photos, photo_ids).await?;
    index_photos(
        settings,
        photos,
        vectors,
        index,
        embeddings,
        progress,
        selected_photos,
        false,
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
    let embeddings = infrastructure::adapters::AiGateway::new(chinese_clip.clone());
    let progress = infrastructure::adapters::TauriProgressReporter::new(app_handle.clone());

    auto_index_unindexed_with(
        &settings_repo,
        &photo_repo,
        &vector_repo,
        vector_index,
        &embeddings,
        &progress,
    )
    .await
}

pub async fn auto_index_unindexed_with(
    settings: &impl SettingsRepository,
    photos: &impl PhotoRepository,
    vectors: &impl VectorRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
    progress: &impl ProgressReporter,
) -> Result<()> {
    if embedding_unavailable(settings, embeddings).await? {
        progress.indexing_progress(IndexingProgressEvent {
            current: 0,
            total: 0,
            status: "unavailable".into(),
        });
        return Ok(());
    }

    let unindexed = photos.find_unindexed().await?;
    index_photos(
        settings, photos, vectors, index, embeddings, progress, unindexed, true,
    )
    .await
    .map(|_| ())
}

pub async fn rebuild_all_index(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    app_handle: &tauri::AppHandle,
) -> Result<usize> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let vector_repo = infrastructure::repositories::SqliteVectorRepository::new(db.clone());
    let embeddings = infrastructure::adapters::AiGateway::new(chinese_clip.clone());
    let progress = infrastructure::adapters::TauriProgressReporter::new(app_handle.clone());

    rebuild_all_index_with(
        &settings_repo,
        &photo_repo,
        &vector_repo,
        vector_index,
        &embeddings,
        &progress,
    )
    .await
}

pub async fn rebuild_all_index_with(
    settings: &impl SettingsRepository,
    photos: &impl PhotoRepository,
    vectors: &impl VectorRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
    progress: &impl ProgressReporter,
) -> Result<usize> {
    index.clear().await;
    vectors.delete_all().await?;
    photos.reset_all_embeddings().await?;

    let all_photos = photos.find_all(None).await?;
    if all_photos.is_empty() {
        return Ok(0);
    }

    index_photos(
        settings, photos, vectors, index, embeddings, progress, all_photos, true,
    )
    .await
}

pub async fn natural_language_search(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    query: String,
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
    )
    .await
}

pub async fn natural_language_search_with(
    settings: &impl SettingsRepository,
    photos: &impl PhotoRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
    query: String,
) -> Result<Vec<SearchResult>> {
    let settings = settings.get().await?;
    require_embedding_service(&settings.api_key, embeddings)?;

    let query_embedding = embeddings.embed_text(&settings.api_key, &query).await?;
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

async fn index_photos(
    settings_repo: &impl SettingsRepository,
    photos: &impl PhotoRepository,
    vectors: &impl VectorRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
    progress: &impl ProgressReporter,
    photos_to_index: Vec<Photo>,
    emit_progress: bool,
) -> Result<usize> {
    let settings = settings_repo.get().await?;
    require_embedding_service(&settings.api_key, embeddings)?;

    let total = photos_to_index.len();
    if emit_progress {
        progress.indexing_progress(IndexingProgressEvent {
            current: 0,
            total,
            status: if total == 0 { "complete" } else { "started" }.into(),
        });
    }

    if total == 0 {
        return Ok(0);
    }

    let mut indexed_count = 0usize;
    for (i, photo) in photos_to_index.iter().enumerate() {
        match index_photo(&settings.api_key, photo, photos, vectors, index, embeddings).await {
            Ok(()) => {
                indexed_count += 1;
                tracing::info!("Indexed {}/{}: {}", i + 1, total, photo.file_name);
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

    if emit_progress {
        progress.indexing_progress(IndexingProgressEvent {
            current: total,
            total,
            status: "complete".into(),
        });
    }

    Ok(indexed_count)
}

async fn index_photo(
    api_key: &str,
    photo: &Photo,
    photos: &impl PhotoRepository,
    vectors: &impl VectorRepository,
    index: &impl VectorIndexStore,
    embeddings: &impl EmbeddingService,
) -> Result<()> {
    let embedding = embeddings.embed_image(api_key, &photo.file_path).await?;
    let vector_json = serde_json::to_string(&embedding)
        .map_err(|e| PhotoCurateError::InvalidData(e.to_string()))?;

    vectors
        .save(&uuid::Uuid::new_v4().to_string(), &photo.id, &vector_json)
        .await?;
    photos
        .update_embedding(&photo.id, EMBEDDING_VERSION)
        .await?;
    index.add(photo.id.clone(), embedding).await;

    Ok(())
}

async fn embedding_unavailable(
    settings: &impl SettingsRepository,
    embeddings: &impl EmbeddingService,
) -> Result<bool> {
    let settings = settings.get().await?;
    Ok(!embeddings.has_local_model() && settings.api_key.is_empty())
}

fn require_embedding_service(api_key: &str, embeddings: &impl EmbeddingService) -> Result<()> {
    if !embeddings.has_local_model() && api_key.is_empty() {
        return Err(PhotoCurateError::EmbeddingServiceMissing);
    }
    Ok(())
}
