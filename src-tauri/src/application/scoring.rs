use crate::domain::models::AiSettings;
use crate::error::{PhotoCurateError, Result};
use crate::infrastructure;
use sqlx::{Pool, Sqlite};

pub async fn score_photos(db: &Pool<Sqlite>, photo_ids: Vec<String>) -> Result<()> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let settings = settings_repo.get().await?;

    if settings.api_key.is_empty() {
        return Err(PhotoCurateError::ApiKeyMissing);
    }

    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());

    for photo_id in photo_ids {
        let photo = photo_repo.find_by_id(&photo_id).await?;
        if let Some(photo) = photo {
            match infrastructure::ai::score_image(&settings.api_key, &photo.file_path).await {
                Ok(result) => {
                    photo_repo.update_score(&photo_id, result.score).await?;
                    tracing::info!("Scored {} = {}", photo.file_name, result.score);
                }
                Err(e) => {
                    tracing::warn!("Scoring failed for {}: {}", photo.file_name, e);
                }
            }
        }
    }
    Ok(())
}

pub async fn get_ai_settings(db: &Pool<Sqlite>) -> Result<AiSettings> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    settings_repo.get().await
}

pub async fn update_ai_settings(
    db: &Pool<Sqlite>,
    settings: serde_json::Value,
) -> Result<AiSettings> {
    let provider = settings["provider"].as_str().unwrap_or("gemini");
    let api_key = settings["api_key"].as_str().unwrap_or("");
    let ollama_base_url = settings["ollama_base_url"]
        .as_str()
        .unwrap_or("http://localhost:11434");
    let ollama_embed_model = settings["ollama_embed_model"]
        .as_str()
        .unwrap_or("nomic-embed-text");
    let ollama_vision_model = settings["ollama_vision_model"]
        .as_str()
        .unwrap_or("llava");

    let new_settings = AiSettings {
        id: "default".to_string(),
        provider: provider.to_string(),
        api_key: api_key.to_string(),
        ollama_base_url: ollama_base_url.to_string(),
        ollama_embed_model: ollama_embed_model.to_string(),
        ollama_vision_model: ollama_vision_model.to_string(),
    };

    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    settings_repo.update(&new_settings).await?;
    settings_repo.get().await
}

#[derive(serde::Serialize)]
pub struct ValidateKeyResult {
    pub valid: bool,
    pub message: String,
}

pub async fn validate_api_key(api_key: &str) -> Result<ValidateKeyResult> {
    let (valid, msg) = infrastructure::ai::validate_api_key(api_key).await?;
    Ok(ValidateKeyResult {
        valid,
        message: msg,
    })
}
