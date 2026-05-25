use crate::domain::models::{IndexingProgressEvent, SearchResult};
use crate::error::{PhotoCurateError, Result};
use crate::infrastructure;
use crate::infrastructure::vector::SharedVectorIndex;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;
use tauri::Emitter;

pub async fn build_index(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    photo_ids: Vec<String>,
) -> Result<()> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let settings = settings_repo.get().await?;
    let local = chinese_clip.as_ref().map(Arc::clone);

    if local.is_none() && settings.api_key.is_empty() {
        return Err(PhotoCurateError::EmbeddingServiceMissing);
    }

    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let vector_repo = infrastructure::repositories::SqliteVectorRepository::new(db.clone());

    for photo_id in photo_ids {
        let photo = photo_repo.find_by_id(&photo_id).await?;
        if let Some(photo) = photo {
            match infrastructure::ai::embed_image(
                &settings.api_key,
                &photo.file_path,
                local.clone(),
            )
            .await
            {
                Ok(embedding) => {
                    let vector_json = serde_json::to_string(&embedding)
                        .map_err(|e| PhotoCurateError::InvalidData(e.to_string()))?;
                    vector_repo
                        .save(&uuid::Uuid::new_v4().to_string(), &photo_id, &vector_json)
                        .await?;

                    photo_repo.update_embedding(&photo_id, 1).await?;

                    let mut index = vector_index.write().await;
                    index.add(photo_id, embedding);

                    tracing::info!("Indexed {}", photo.file_name);
                }
                Err(e) => {
                    tracing::warn!("Embedding failed for {}: {}", photo.file_name, e);
                }
            }
        }
    }
    Ok(())
}

pub async fn auto_index_unindexed(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    app_handle: &tauri::AppHandle,
) -> Result<()> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let settings = settings_repo.get().await?;
    let local = chinese_clip.as_ref().map(Arc::clone);

    if local.is_none() && settings.api_key.is_empty() {
        let _ = app_handle.emit(
            "indexing-progress",
            IndexingProgressEvent {
                current: 0,
                total: 0,
                status: "unavailable".into(),
            },
        );
        return Ok(());
    }

    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let vector_repo = infrastructure::repositories::SqliteVectorRepository::new(db.clone());

    let unindexed = photo_repo.find_unindexed().await?;
    let total = unindexed.len();

    if total == 0 {
        let _ = app_handle.emit(
            "indexing-progress",
            IndexingProgressEvent {
                current: 0,
                total: 0,
                status: "complete".into(),
            },
        );
        return Ok(());
    }

    tracing::info!("Auto-indexing {} unindexed photos", total);

    let _ = app_handle.emit(
        "indexing-progress",
        IndexingProgressEvent {
            current: 0,
            total,
            status: "started".into(),
        },
    );

    for (i, photo) in unindexed.iter().enumerate() {
        match infrastructure::ai::embed_image(&settings.api_key, &photo.file_path, local.clone())
            .await
        {
            Ok(embedding) => {
                let vector_json = serde_json::to_string(&embedding).unwrap_or_default();
                let _ = vector_repo
                    .save(&uuid::Uuid::new_v4().to_string(), &photo.id, &vector_json)
                    .await;

                let _ = photo_repo.update_embedding(&photo.id, 1).await;

                let mut index = vector_index.write().await;
                index.add(photo.id.clone(), embedding);

                tracing::info!("Indexed {}/{}: {}", i + 1, total, photo.file_name);
            }
            Err(e) => {
                tracing::warn!("Embedding failed for {}: {}", photo.file_name, e);
            }
        }

        let current = i + 1;
        let _ = app_handle.emit(
            "indexing-progress",
            IndexingProgressEvent {
                current,
                total,
                status: "indexing".into(),
            },
        );
    }

    let _ = app_handle.emit(
        "indexing-progress",
        IndexingProgressEvent {
            current: total,
            total,
            status: "complete".into(),
        },
    );

    tracing::info!("Auto-indexing complete: {} photos", total);
    Ok(())
}

pub async fn rebuild_all_index(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    app_handle: &tauri::AppHandle,
) -> Result<usize> {
    // 1. Clear memory index
    {
        let mut index = vector_index.write().await;
        index.clear();
    }

    // 2. Clear database vectors and reset photo embedding flags
    let vector_repo = infrastructure::repositories::SqliteVectorRepository::new(db.clone());
    vector_repo.delete_all().await?;

    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    photo_repo.reset_all_embeddings().await?;

    // 3. Re-index all photos using the currently available model
    let all_photos = photo_repo.find_all(None).await?;
    if all_photos.is_empty() {
        return Ok(0);
    }

    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let settings = settings_repo.get().await?;
    let local = chinese_clip.as_ref().map(Arc::clone);

    if local.is_none() && settings.api_key.is_empty() {
        return Err(PhotoCurateError::EmbeddingServiceMissing);
    }

    let mut indexed_count = 0usize;
    let total = all_photos.len();

    for (i, photo) in all_photos.iter().enumerate() {
        match infrastructure::ai::embed_image(&settings.api_key, &photo.file_path, local.clone()).await {
            Ok(embedding) => {
                let vector_json = serde_json::to_string(&embedding)
                    .map_err(|e| PhotoCurateError::InvalidData(e.to_string()))?;
                vector_repo
                    .save(&uuid::Uuid::new_v4().to_string(), &photo.id, &vector_json)
                    .await?;

                photo_repo.update_embedding(&photo.id, 1).await?;

                let mut index = vector_index.write().await;
                index.add(photo.id.clone(), embedding);

                indexed_count += 1;
                tracing::info!("Re-indexed {}/{}: {}", i + 1, total, photo.file_name);
            }
            Err(e) => {
                tracing::warn!("Re-index failed for {}: {}", photo.file_name, e);
            }
        }

        let _ = app_handle.emit(
            "indexing-progress",
            IndexingProgressEvent {
                current: i + 1,
                total,
                status: "indexing".into(),
            },
        );
    }

    let _ = app_handle.emit(
        "indexing-progress",
        IndexingProgressEvent {
            current: total,
            total,
            status: "complete".into(),
        },
    );

    Ok(indexed_count)
}

pub async fn natural_language_search(
    db: &Pool<Sqlite>,
    vector_index: &SharedVectorIndex,
    chinese_clip: &Option<Arc<infrastructure::ai::ChineseClipService>>,
    query: String,
) -> Result<Vec<SearchResult>> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let settings = settings_repo.get().await?;
    let local = chinese_clip.as_ref().map(Arc::clone);

    if local.is_none() && settings.api_key.is_empty() {
        return Err(PhotoCurateError::EmbeddingServiceMissing);
    }

    let query_embedding = infrastructure::ai::embed_text(&settings.api_key, &query, local).await?;

    let index = vector_index.read().await;
    // Use min_similarity=0.0 to return all candidates; frontend decides grouping
    let results = index.search(&query_embedding, 50, 0.2);
    drop(index);

    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let mut search_results = Vec::new();
    for (photo_id, similarity) in results {
        let photo = photo_repo.find_by_id(&photo_id).await?;
        if let Some(photo) = photo {
            search_results.push(SearchResult { photo, similarity });
        }
    }

    Ok(search_results)
}
