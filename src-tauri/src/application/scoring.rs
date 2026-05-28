use crate::application::ports::{
    PhotoEvaluationRepository, PhotoRepository, ProgressReporter, ScoringService,
    SettingsRepository,
};
use crate::domain::models::{AiSettings, IndexingProgressEvent, PhotoEvaluation};
use crate::error::{PhotoCurateError, Result};
use crate::infrastructure;
use sqlx::{Pool, Sqlite};

const PHOTO_EVALUATION_PROMPT_VERSION: &str = "photo-evaluation-v1";

pub async fn score_photos(
    db: &Pool<Sqlite>,
    photo_ids: Vec<String>,
    app_handle: &tauri::AppHandle,
    allow_keychain_read: bool,
) -> Result<()> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let evaluation_repo =
        infrastructure::repositories::SqlitePhotoEvaluationRepository::new(db.clone());
    let ai = infrastructure::adapters::AiGateway::new(None);
    let progress = infrastructure::adapters::TauriProgressReporter::new(app_handle.clone());

    score_photos_with(
        &settings_repo,
        &photo_repo,
        &evaluation_repo,
        &ai,
        &progress,
        photo_ids,
        allow_keychain_read,
    )
    .await
}

pub async fn score_photos_with(
    settings: &impl SettingsRepository,
    photos: &impl PhotoRepository,
    evaluations: &impl PhotoEvaluationRepository,
    scorer: &impl ScoringService,
    progress: &impl ProgressReporter,
    photo_ids: Vec<String>,
    allow_keychain_read: bool,
) -> Result<()> {
    let mut ai_settings = settings.get().await?;
    normalize_settings(&mut ai_settings);
    let discovered_scoring_key = load_scoring_api_key(&mut ai_settings, allow_keychain_read)?;
    if discovered_scoring_key {
        let mut updated = ai_settings.clone();
        updated.scoring_api_key.clear();
        settings.update(&updated).await?;
    }

    if ai_settings.scoring_api_key.is_empty() {
        return Err(PhotoCurateError::ApiKeyMissing);
    }

    let total = photo_ids.len();

    for (i, photo_id) in photo_ids.iter().enumerate() {
        let photo = photos.find_by_id(photo_id).await?;
        if let Some(photo) = photo {
            match scorer.score_image(&ai_settings, &photo.file_path).await {
                Ok(result) => {
                    let evaluation = PhotoEvaluation::from_score_result(
                        photo_id.clone(),
                        result,
                        ai_settings.scoring_provider.clone(),
                        ai_settings.scoring_model.clone(),
                        PHOTO_EVALUATION_PROMPT_VERSION.to_string(),
                    );
                    evaluations.save(&evaluation).await?;
                    tracing::info!(
                        "Evaluated {} = {}",
                        photo.file_name,
                        evaluation.overall_score
                    );
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
    let mut settings = settings.get().await?;
    normalize_settings(&mut settings);
    sanitize_settings(settings)
}

pub async fn update_ai_settings(
    db: &Pool<Sqlite>,
    incoming: serde_json::Value,
) -> Result<AiSettings> {
    let settings_repo = infrastructure::repositories::SqliteSettingsRepository::new(db.clone());
    let mut new_settings = settings_repo.get().await?;
    normalize_settings(&mut new_settings);
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
    mut new_settings: AiSettings,
) -> Result<AiSettings> {
    normalize_settings(&mut new_settings);
    settings.update(&new_settings).await?;
    let mut settings = settings.get().await?;
    normalize_settings(&mut settings);
    sanitize_settings(settings)
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
        key_storage: "memory".to_string(),
        scoring_provider: provider.to_string(),
        scoring_model: model,
        scoring_base_url: base_url,
        scoring_api_key: api_key.to_string(),
        has_scoring_api_key: !api_key.is_empty(),
        scoring_key_storage: "memory".to_string(),
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
    allow_keychain_read: bool,
) -> Result<String> {
    let mut settings = settings_repo.get().await?;
    normalize_settings(&mut settings);
    let discovered_embedding_key = load_embedding_api_key(&mut settings, allow_keychain_read)?;
    if discovered_embedding_key {
        let mut updated = settings.clone();
        updated.api_key.clear();
        settings_repo.update(&updated).await?;
    }
    Ok(settings.api_key)
}

fn sanitize_settings(mut settings: AiSettings) -> Result<AiSettings> {
    settings.api_key.clear();
    settings.scoring_api_key.clear();
    Ok(settings)
}

fn load_embedding_api_key(settings: &mut AiSettings, allow_keychain_read: bool) -> Result<bool> {
    if !infrastructure::keychain::stores_api_key() {
        settings.has_api_key = !settings.api_key.is_empty();
        return Ok(false);
    }

    if settings.key_storage != "keychain" && !settings.api_key.is_empty() {
        settings.has_api_key = true;
        return Ok(false);
    }

    if !allow_keychain_read {
        return Ok(false);
    }

    if settings.has_api_key {
        let api_key = infrastructure::keychain::get_api_key("gemini")?.or_else(|| {
            infrastructure::keychain::get_legacy_gemini_api_key()
                .ok()
                .flatten()
        });
        if let Some(api_key) = api_key {
            settings.api_key = api_key;
            settings.has_api_key = true;
        }
        return Ok(false);
    }

    if let Some(api_key) = infrastructure::keychain::get_legacy_gemini_api_key()? {
        settings.api_key = api_key;
        settings.has_api_key = true;
        settings.key_storage = "keychain".to_string();
        return Ok(true);
    }

    settings.has_api_key = false;
    Ok(false)
}

fn load_scoring_api_key(settings: &mut AiSettings, allow_keychain_read: bool) -> Result<bool> {
    if !infrastructure::keychain::stores_api_key() {
        settings.has_scoring_api_key = !settings.scoring_api_key.is_empty();
        if settings.scoring_api_key.is_empty() && settings.scoring_provider == "gemini" {
            settings.scoring_api_key = settings.api_key.clone();
            settings.has_scoring_api_key = !settings.scoring_api_key.is_empty();
        }
        return Ok(false);
    }

    if settings.scoring_key_storage != "keychain" && !settings.scoring_api_key.is_empty() {
        settings.has_scoring_api_key = true;
        return Ok(false);
    }

    if !allow_keychain_read {
        return Ok(false);
    }

    if settings.has_scoring_api_key {
        let api_key = infrastructure::keychain::get_api_key(&settings.scoring_provider)?;
        if let Some(api_key) = api_key {
            settings.scoring_api_key = api_key;
            settings.has_scoring_api_key = true;
            return Ok(false);
        }

        if settings.scoring_provider == "gemini" {
            let api_key = infrastructure::keychain::get_api_key("gemini")?.or_else(|| {
                infrastructure::keychain::get_legacy_gemini_api_key()
                    .ok()
                    .flatten()
            });
            if let Some(api_key) = api_key {
                settings.scoring_api_key = api_key;
                settings.has_scoring_api_key = true;
            }
        }
        return Ok(false);
    }

    if settings.scoring_provider == "gemini" {
        if let Some(api_key) = infrastructure::keychain::get_legacy_gemini_api_key()? {
            settings.scoring_api_key = api_key;
            settings.has_scoring_api_key = true;
            settings.scoring_key_storage = "keychain".to_string();
            return Ok(true);
        }

        if let Some(api_key) = infrastructure::keychain::get_api_key("gemini")?.or_else(|| {
            infrastructure::keychain::get_legacy_gemini_api_key()
                .ok()
                .flatten()
        }) {
            settings.scoring_api_key = api_key;
            settings.has_scoring_api_key = true;
            return Ok(false);
        }
    }

    settings.has_scoring_api_key = false;
    Ok(false)
}

fn set_stored_api_key(settings: &mut AiSettings, provider: &str, api_key: &str) -> Result<()> {
    if infrastructure::keychain::stores_api_key() {
        infrastructure::keychain::set_api_key(provider, api_key)?;
        settings.api_key.clear();
        settings.key_storage = "keychain".to_string();
    } else {
        settings.api_key = api_key.to_string();
        settings.key_storage = "database".to_string();
    }
    settings.has_api_key = !api_key.is_empty();
    Ok(())
}

fn set_stored_scoring_api_key(settings: &mut AiSettings, api_key: &str) -> Result<()> {
    if infrastructure::keychain::stores_api_key() {
        infrastructure::keychain::set_api_key(&settings.scoring_provider, api_key)?;
        settings.scoring_api_key.clear();
        settings.scoring_key_storage = "keychain".to_string();
    } else {
        settings.scoring_api_key = api_key.to_string();
        settings.scoring_key_storage = "database".to_string();
    }
    settings.has_scoring_api_key = !api_key.is_empty();

    if settings.scoring_provider == "gemini" {
        settings.has_api_key = !api_key.is_empty();
        if infrastructure::keychain::stores_api_key() {
            infrastructure::keychain::set_api_key("gemini", api_key)?;
            settings.api_key.clear();
            settings.key_storage = "keychain".to_string();
        } else {
            settings.api_key = api_key.to_string();
            settings.key_storage = "database".to_string();
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

fn normalize_settings(settings: &mut AiSettings) {
    settings.provider = normalize_embedding_provider(&settings.provider).to_string();
    settings.scoring_provider =
        infrastructure::ai::normalize_scoring_provider(&settings.scoring_provider).to_string();
    settings.scoring_model = normalize_model(&settings.scoring_model, &settings.scoring_provider);
    settings.scoring_base_url =
        normalize_base_url(&settings.scoring_base_url, &settings.scoring_provider);
    if settings.key_storage.trim().is_empty() {
        settings.key_storage = if infrastructure::keychain::stores_api_key() {
            "keychain".to_string()
        } else {
            "database".to_string()
        };
    }
    if settings.scoring_key_storage.trim().is_empty() {
        settings.scoring_key_storage = if infrastructure::keychain::stores_api_key() {
            "keychain".to_string()
        } else {
            "database".to_string()
        };
    }
    if !settings.api_key.is_empty() {
        settings.has_api_key = true;
    }
    if !settings.scoring_api_key.is_empty() {
        settings.has_scoring_api_key = true;
    }
}
