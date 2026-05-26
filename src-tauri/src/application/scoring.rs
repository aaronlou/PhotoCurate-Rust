use crate::application::ports::{
    PhotoRepository, ProgressReporter, ScoringService, SettingsRepository,
};
use crate::domain::models::{AiSettings, IndexingProgressEvent};
use crate::error::{PhotoCurateError, Result};
use crate::infrastructure;
use sqlx::{Pool, Sqlite};

pub async fn score_photos(
    db: &Pool<Sqlite>,
    photo_ids: Vec<String>,
    app_handle: &tauri::AppHandle,
) -> Result<()> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let ai = infrastructure::adapters::AiGateway::new(None);
    let progress = infrastructure::adapters::TauriProgressReporter::new(app_handle.clone());

    score_photos_with(&settings_repo, &photo_repo, &ai, &progress, photo_ids).await
}

pub async fn score_photos_with(
    settings: &impl SettingsRepository,
    photos: &impl PhotoRepository,
    scorer: &impl ScoringService,
    progress: &impl ProgressReporter,
    photo_ids: Vec<String>,
) -> Result<()> {
    let settings = settings.get().await?;

    if settings.api_key.is_empty() {
        return Err(PhotoCurateError::ApiKeyMissing);
    }

    let total = photo_ids.len();

    for (i, photo_id) in photo_ids.iter().enumerate() {
        let photo = photos.find_by_id(photo_id).await?;
        if let Some(photo) = photo {
            match scorer
                .score_image(&settings.api_key, &photo.file_path)
                .await
            {
                Ok(result) => {
                    photos.update_score(photo_id, result.score).await?;
                    tracing::info!("Scored {} = {}", photo.file_name, result.score);
                }
                Err(e) => {
                    tracing::warn!("Scoring failed for {}: {}", photo.file_name, e);
                }
            }
        }

        progress.scoring_progress(IndexingProgressEvent {
            current: i + 1,
            total,
            status: "indexing".into(),
        });
    }

    progress.scoring_progress(IndexingProgressEvent {
        current: total,
        total,
        status: "complete".into(),
    });

    Ok(())
}

pub async fn get_ai_settings(db: &Pool<Sqlite>) -> Result<AiSettings> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    get_ai_settings_with(&settings_repo).await
}

pub async fn get_ai_settings_with(settings: &impl SettingsRepository) -> Result<AiSettings> {
    settings.get().await
}

pub async fn update_ai_settings(
    db: &Pool<Sqlite>,
    settings: serde_json::Value,
) -> Result<AiSettings> {
    let provider = "gemini";
    let api_key = settings["api_key"].as_str().unwrap_or("");

    let new_settings = AiSettings {
        id: "default".to_string(),
        provider: provider.to_string(),
        api_key: api_key.to_string(),
        ollama_base_url: String::new(),
        ollama_embed_model: String::new(),
        ollama_vision_model: String::new(),
    };

    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    update_ai_settings_with(&settings_repo, new_settings).await
}

pub async fn update_ai_settings_with(
    settings: &impl SettingsRepository,
    new_settings: AiSettings,
) -> Result<AiSettings> {
    settings.update(&new_settings).await?;
    settings.get().await
}

#[derive(serde::Serialize)]
pub struct ValidateKeyResult {
    pub valid: bool,
    pub message: String,
}

pub async fn validate_api_key(api_key: &str) -> Result<ValidateKeyResult> {
    let ai = infrastructure::adapters::AiGateway::new(None);
    validate_api_key_with(&ai, api_key).await
}

pub async fn validate_api_key_with(
    scorer: &impl ScoringService,
    api_key: &str,
) -> Result<ValidateKeyResult> {
    let (valid, msg) = scorer.validate_api_key(api_key).await?;
    Ok(ValidateKeyResult {
        valid,
        message: msg,
    })
}
