use crate::domain::models::SearchResult;
use crate::error::{PhotoCurateError, Result};
use crate::infrastructure;
use crate::infrastructure::vector::SharedVectorIndex;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;

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

    let query_embedding =
        infrastructure::ai::embed_text(&settings.api_key, &query, local).await?;

    let index = vector_index.read().await;
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
