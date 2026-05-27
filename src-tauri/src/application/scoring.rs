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
    let mut ai_settings = settings.get().await?;
    hydrate_settings_api_keys(settings, &mut ai_settings).await?;

    if ai_settings.scoring_api_key.is_empty() {
        return Err(PhotoCurateError::ApiKeyMissing);
    }

    let total = photo_ids.len();

    for (i, photo_id) in photo_ids.iter().enumerate() {
        let photo = photos.find_by_id(photo_id).await?;
        if let Some(photo) = photo {
            match scorer.score_image(&ai_settings, &photo.file_path).await {
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
    incoming: serde_json::Value,
) -> Result<AiSettings> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let mut new_settings = settings_repo.get().await?;
    hydrate_settings_api_keys(&settings_repo, &mut new_settings).await?;
    let previous_scoring_provider = new_settings.scoring_provider.clone();

    if let Some(provider) = incoming["provider"].as_str() {
        new_settings.provider = normalize_embedding_provider(provider).to_string();
    }

    let scoring_provider = incoming["scoring_provider"]
        .as_str()
        .or_else(|| incoming["provider"].as_str())
        .map(infrastructure::ai::normalize_scoring_provider)
        .unwrap_or_else(|| {
            infrastructure::ai::normalize_scoring_provider(&new_settings.scoring_provider)
        });
    new_settings.scoring_provider = scoring_provider.to_string();

    if scoring_provider != previous_scoring_provider
        && !incoming["scoring_api_key"].is_string()
        && !incoming["api_key"].is_string()
    {
        new_settings.scoring_api_key.clear();
        new_settings.has_scoring_api_key = false;
    }

    if let Some(model) = incoming["scoring_model"].as_str() {
        new_settings.scoring_model = normalize_model(model, scoring_provider);
    } else if new_settings.scoring_model.trim().is_empty()
        || incoming["scoring_provider"].is_string()
        || incoming["provider"].is_string()
    {
        new_settings.scoring_model =
            infrastructure::ai::default_scoring_model(scoring_provider).to_string();
    }

    if let Some(base_url) = incoming["scoring_base_url"].as_str() {
        new_settings.scoring_base_url = normalize_base_url(base_url, scoring_provider);
    } else if incoming["scoring_provider"].is_string()
        || incoming["provider"].is_string()
        || new_settings.scoring_base_url.trim().is_empty()
    {
        new_settings.scoring_base_url =
            infrastructure::ai::default_scoring_base_url(scoring_provider).to_string();
    }

    if let Some(api_key) = incoming["api_key"].as_str() {
        set_stored_api_key(&mut new_settings, "gemini", api_key)?;
    }

    if let Some(scoring_api_key) = incoming["scoring_api_key"]
        .as_str()
        .or_else(|| incoming["api_key"].as_str())
    {
        set_stored_scoring_api_key(&mut new_settings, scoring_api_key)?;
    }

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

pub async fn validate_api_key(settings: serde_json::Value) -> Result<ValidateKeyResult> {
    let ai = infrastructure::adapters::AiGateway::new(None);
    let provider = settings["scoring_provider"]
        .as_str()
        .or_else(|| settings["provider"].as_str())
        .map(infrastructure::ai::normalize_scoring_provider)
        .unwrap_or("gemini");
    let model = settings["scoring_model"]
        .as_str()
        .map(|model| normalize_model(model, provider))
        .unwrap_or_else(|| infrastructure::ai::default_scoring_model(provider).to_string());
    let base_url = settings["scoring_base_url"]
        .as_str()
        .map(|base_url| normalize_base_url(base_url, provider))
        .unwrap_or_else(|| infrastructure::ai::default_scoring_base_url(provider).to_string());
    let api_key = settings["scoring_api_key"]
        .as_str()
        .or_else(|| settings["api_key"].as_str())
        .unwrap_or("");

    let settings = AiSettings {
        id: "default".to_string(),
        provider: "gemini".to_string(),
        api_key: String::new(),
        has_api_key: false,
        scoring_provider: provider.to_string(),
        scoring_model: model,
        scoring_base_url: base_url,
        scoring_api_key: api_key.to_string(),
        has_scoring_api_key: !api_key.is_empty(),
        ollama_base_url: String::new(),
        ollama_embed_model: String::new(),
        ollama_vision_model: String::new(),
    };

    validate_api_key_with(&ai, &settings).await
}

pub async fn validate_api_key_with(
    scorer: &impl ScoringService,
    settings: &AiSettings,
) -> Result<ValidateKeyResult> {
    let (valid, msg) = scorer.validate_api_key(settings).await?;
    Ok(ValidateKeyResult {
        valid,
        message: msg,
    })
}

pub(crate) async fn resolve_api_key_from_settings(
    settings_repo: &impl SettingsRepository,
) -> Result<String> {
    let mut settings = settings_repo.get().await?;
    load_embedding_api_key(settings_repo, &mut settings).await?;
    Ok(settings.api_key)
}

async fn sanitize_settings(
    settings_repo: &impl SettingsRepository,
    mut settings: AiSettings,
) -> Result<AiSettings> {
    hydrate_settings_api_keys(settings_repo, &mut settings).await?;
    settings.api_key.clear();
    settings.scoring_api_key.clear();
    Ok(settings)
}

async fn hydrate_settings_api_keys(
    settings_repo: &impl SettingsRepository,
    settings: &mut AiSettings,
) -> Result<()> {
    load_embedding_api_key(settings_repo, settings).await?;
    load_scoring_api_key(settings_repo, settings).await?;
    Ok(())
}

async fn load_embedding_api_key(
    settings_repo: &impl SettingsRepository,
    settings: &mut AiSettings,
) -> Result<()> {
    if !infrastructure::keychain::stores_api_key() {
        settings.has_api_key = !settings.api_key.is_empty();
        return Ok(());
    }

    if let Some(api_key) = infrastructure::keychain::get_api_key("gemini")?.or_else(|| {
        infrastructure::keychain::get_legacy_gemini_api_key()
            .ok()
            .flatten()
    }) {
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
    infrastructure::keychain::set_api_key("gemini", &legacy_api_key)?;

    let mut updated = settings.clone();
    updated.api_key.clear();
    updated.has_api_key = true;
    settings_repo.update(&updated).await?;

    settings.api_key = legacy_api_key;
    settings.has_api_key = true;
    Ok(())
}

async fn load_scoring_api_key(
    settings_repo: &impl SettingsRepository,
    settings: &mut AiSettings,
) -> Result<()> {
    settings.scoring_provider =
        infrastructure::ai::normalize_scoring_provider(&settings.scoring_provider).to_string();
    if settings.scoring_model.trim().is_empty() {
        settings.scoring_model =
            infrastructure::ai::default_scoring_model(&settings.scoring_provider).to_string();
    }
    if settings.scoring_base_url.trim().is_empty() {
        settings.scoring_base_url =
            infrastructure::ai::default_scoring_base_url(&settings.scoring_provider).to_string();
    }

    if !infrastructure::keychain::stores_api_key() {
        settings.has_scoring_api_key = !settings.scoring_api_key.is_empty();
        if settings.scoring_api_key.is_empty() && settings.scoring_provider == "gemini" {
            settings.scoring_api_key = settings.api_key.clone();
            settings.has_scoring_api_key = !settings.scoring_api_key.is_empty();
        }
        return Ok(());
    }

    if let Some(api_key) = infrastructure::keychain::get_api_key(&settings.scoring_provider)? {
        if !settings.scoring_api_key.is_empty() {
            let mut updated = settings.clone();
            updated.scoring_api_key.clear();
            updated.has_scoring_api_key = true;
            settings_repo.update(&updated).await?;
        }
        settings.scoring_api_key = api_key;
        settings.has_scoring_api_key = true;
        return Ok(());
    }

    if settings.scoring_provider == "gemini" {
        if let Some(api_key) = infrastructure::keychain::get_api_key("gemini")?.or_else(|| {
            infrastructure::keychain::get_legacy_gemini_api_key()
                .ok()
                .flatten()
        }) {
            settings.scoring_api_key = api_key;
            settings.has_scoring_api_key = true;
            return Ok(());
        }
    }

    if settings.scoring_api_key.is_empty() {
        settings.has_scoring_api_key = false;
        return Ok(());
    }

    let legacy_api_key = std::mem::take(&mut settings.scoring_api_key);
    infrastructure::keychain::set_api_key(&settings.scoring_provider, &legacy_api_key)?;

    let mut updated = settings.clone();
    updated.scoring_api_key.clear();
    updated.has_scoring_api_key = true;
    settings_repo.update(&updated).await?;

    settings.scoring_api_key = legacy_api_key;
    settings.has_scoring_api_key = true;
    Ok(())
}

fn set_stored_api_key(settings: &mut AiSettings, provider: &str, api_key: &str) -> Result<()> {
    if infrastructure::keychain::stores_api_key() {
        infrastructure::keychain::set_api_key(provider, api_key)?;
        settings.api_key.clear();
    } else {
        settings.api_key = api_key.to_string();
    }
    settings.has_api_key = !api_key.is_empty();
    Ok(())
}

fn set_stored_scoring_api_key(settings: &mut AiSettings, api_key: &str) -> Result<()> {
    if infrastructure::keychain::stores_api_key() {
        infrastructure::keychain::set_api_key(&settings.scoring_provider, api_key)?;
        settings.scoring_api_key.clear();
    } else {
        settings.scoring_api_key = api_key.to_string();
    }
    settings.has_scoring_api_key = !api_key.is_empty();

    if settings.scoring_provider == "gemini" {
        settings.has_api_key = !api_key.is_empty();
        if infrastructure::keychain::stores_api_key() {
            infrastructure::keychain::set_api_key("gemini", api_key)?;
            settings.api_key.clear();
        } else {
            settings.api_key = api_key.to_string();
        }
    }

    Ok(())
}

fn normalize_embedding_provider(provider: &str) -> &str {
    match provider {
        "gemini" => "gemini",
        _ => "gemini",
    }
}

fn normalize_model(model: &str, provider: &str) -> String {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        infrastructure::ai::default_scoring_model(provider).to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_base_url(base_url: &str, provider: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        infrastructure::ai::default_scoring_base_url(provider).to_string()
    } else {
        trimmed.trim_end_matches('/').to_string()
    }
}
