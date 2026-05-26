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
    let api_key = resolve_api_key_from_settings(settings).await?;

    if api_key.is_empty() {
        return Err(PhotoCurateError::ApiKeyMissing);
    }

    let total = photo_ids.len();

    for (i, photo_id) in photo_ids.iter().enumerate() {
        let photo = photos.find_by_id(photo_id).await?;
        if let Some(photo) = photo {
            match scorer.score_image(&api_key, &photo.file_path).await {
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
    sanitize_settings(settings, settings.get().await?).await
}

pub async fn update_ai_settings(
    db: &Pool<Sqlite>,
    settings: serde_json::Value,
) -> Result<AiSettings> {
    let provider = "gemini";
    let api_key = settings["api_key"].as_str().unwrap_or("");
    let stored_api_key = if infrastructure::keychain::stores_api_key() {
        infrastructure::keychain::set_gemini_api_key(api_key)?;
        String::new()
    } else {
        api_key.to_string()
    };

    let new_settings = AiSettings {
        id: "default".to_string(),
        provider: provider.to_string(),
        api_key: stored_api_key,
        has_api_key: !api_key.is_empty(),
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
    sanitize_settings(settings, settings.get().await?).await
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

pub(crate) async fn resolve_api_key_from_settings(
    settings_repo: &impl SettingsRepository,
) -> Result<String> {
    let mut settings = settings_repo.get().await?;
    load_api_key(settings_repo, &mut settings).await?;
    Ok(settings.api_key)
}

async fn sanitize_settings(
    settings_repo: &impl SettingsRepository,
    mut settings: AiSettings,
) -> Result<AiSettings> {
    load_api_key(settings_repo, &mut settings).await?;
    settings.api_key.clear();
    Ok(settings)
}

async fn load_api_key(
    settings_repo: &impl SettingsRepository,
    settings: &mut AiSettings,
) -> Result<()> {
    if !infrastructure::keychain::stores_api_key() {
        settings.has_api_key = !settings.api_key.is_empty();
        return Ok(());
    }

    if let Some(api_key) = infrastructure::keychain::get_gemini_api_key()? {
        if !settings.api_key.is_empty() {
            let mut updated = settings.clone();
            updated.api_key.clear();
            updated.has_api_key = true;
            settings_repo.update(&updated).await?;
        }
        settings.api_key = api_key;
        settings.has_api_key = true;
        return Ok(());
    }

    if settings.api_key.is_empty() {
        settings.has_api_key = false;
        return Ok(());
    }

    let legacy_api_key = std::mem::take(&mut settings.api_key);
    infrastructure::keychain::set_gemini_api_key(&legacy_api_key)?;

    let mut updated = settings.clone();
    updated.api_key.clear();
    updated.has_api_key = true;
    settings_repo.update(&updated).await?;

    settings.api_key = legacy_api_key;
    settings.has_api_key = true;
    Ok(())
}
