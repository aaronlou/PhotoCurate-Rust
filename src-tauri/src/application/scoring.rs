use crate::application::ports::{
    PhotoEvaluationRepository, PhotoRepository, ProgressReporter, ScoringService,
    SettingsRepository,
};
use crate::domain::models::{AiSettings, IndexingProgressEvent, PhotoEvaluation, ScoringRunResult};
use crate::error::{PhotoCurateError, Result};
use crate::infrastructure;
use sqlx::{Pool, Sqlite};

pub async fn score_photos(
    db: &Pool<Sqlite>,
    photo_ids: Vec<String>,
    app_handle: &tauri::AppHandle,
    allow_keychain_read: bool,
) -> Result<ScoringRunResult> {
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
) -> Result<ScoringRunResult> {
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
    let mut run_result = ScoringRunResult::new(total);

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
                        infrastructure::ai::PHOTO_EVALUATION_PROMPT
                            .version
                            .to_string(),
                    );
                    evaluations.save(&evaluation).await?;
                    run_result.record_success();
                    tracing::info!(
                        "Evaluated {} = {}",
                        photo.file_name,
                        evaluation.overall_score
                    );
                }
                Err(e) => {
                    tracing::warn!("Scoring failed for {}: {}", photo.file_name, e);
                    run_result.record_failure(photo_id.clone(), photo.file_name, e.to_string());
                }
            }
        } else {
            run_result.record_failure(
                photo_id.clone(),
                String::new(),
                format!("photo not found: {photo_id}"),
            );
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

    if run_result.all_failed() {
        return Err(PhotoCurateError::ScoringRunFailed(format!(
            "{} of {} photos failed",
            run_result.failed_count, run_result.total_count
        )));
    }

    Ok(run_result)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::NoopProgressReporter;
    use crate::domain::models::{DimensionScore, Photo, ScoreResult};
    use chrono::Utc;
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct InMemorySettingsRepository {
        settings: RefCell<AiSettings>,
    }

    impl InMemorySettingsRepository {
        fn new(settings: AiSettings) -> Self {
            Self {
                settings: RefCell::new(settings),
            }
        }
    }

    impl SettingsRepository for InMemorySettingsRepository {
        async fn get(&self) -> Result<AiSettings> {
            Ok(self.settings.borrow().clone())
        }

        async fn update(&self, settings: &AiSettings) -> Result<()> {
            self.settings.replace(settings.clone());
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
            Ok(vec![])
        }
    }

    struct InMemoryEvaluationRepository {
        saved: RefCell<Vec<PhotoEvaluation>>,
    }

    impl InMemoryEvaluationRepository {
        fn new() -> Self {
            Self {
                saved: RefCell::new(vec![]),
            }
        }
    }

    impl PhotoEvaluationRepository for InMemoryEvaluationRepository {
        async fn save(&self, evaluation: &PhotoEvaluation) -> Result<()> {
            self.saved.borrow_mut().push(evaluation.clone());
            Ok(())
        }

        async fn find_latest_for_photo(&self, _photo_id: &str) -> Result<Option<PhotoEvaluation>> {
            Ok(None)
        }
    }

    struct FakeScoringService {
        failures: Vec<String>,
    }

    impl ScoringService for FakeScoringService {
        async fn score_image(
            &self,
            _settings: &AiSettings,
            image_path: &str,
        ) -> Result<ScoreResult> {
            if self
                .failures
                .iter()
                .any(|failure| image_path.contains(failure))
            {
                return Err(PhotoCurateError::Ai("model failed".to_string()));
            }

            Ok(ScoreResult {
                score: 88.0,
                review: "review".to_string(),
                summary: "summary".to_string(),
                strengths: vec!["构图稳定".to_string()],
                weaknesses: vec![],
                suggestions: vec![],
                dimension_scores: vec![DimensionScore {
                    name: "构图".to_string(),
                    score: 88.0,
                    note: None,
                }],
                tags: vec!["自然光".to_string()],
                raw_response: "{}".to_string(),
            })
        }

        async fn validate_api_key(&self, _settings: &AiSettings) -> Result<(bool, String)> {
            Ok((true, "ok".to_string()))
        }
    }

    fn settings() -> AiSettings {
        AiSettings {
            id: "default".to_string(),
            provider: "gemini".to_string(),
            api_key: String::new(),
            has_api_key: false,
            key_storage: "memory".to_string(),
            scoring_provider: "gemini".to_string(),
            scoring_model: "gemini-test".to_string(),
            scoring_base_url: String::new(),
            scoring_api_key: "secret".to_string(),
            has_scoring_api_key: true,
            scoring_key_storage: "memory".to_string(),
            ollama_base_url: String::new(),
            ollama_embed_model: String::new(),
            ollama_vision_model: String::new(),
        }
    }

    fn photo(id: &str, file_name: &str) -> Photo {
        Photo {
            id: id.to_string(),
            file_path: format!("/tmp/{file_name}"),
            file_name: file_name.to_string(),
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
            directory_id: None,
            has_been_exported: false,
            export_date: None,
            latest_evaluation: None,
        }
    }

    #[tokio::test]
    async fn scoring_run_reports_partial_failures() {
        let settings_repo = InMemorySettingsRepository::new(settings());
        let photo_repo = InMemoryPhotoRepository::new(vec![
            photo("photo-1", "good.jpg"),
            photo("photo-2", "bad.jpg"),
        ]);
        let evaluation_repo = InMemoryEvaluationRepository::new();
        let scorer = FakeScoringService {
            failures: vec!["bad".to_string()],
        };

        let result = score_photos_with(
            &settings_repo,
            &photo_repo,
            &evaluation_repo,
            &scorer,
            &NoopProgressReporter,
            vec!["photo-1".to_string(), "photo-2".to_string()],
            false,
        )
        .await
        .expect("partial failure should still return run result");

        assert_eq!(result.total_count, 2);
        assert_eq!(result.success_count, 1);
        assert_eq!(result.failed_count, 1);
        assert_eq!(result.failures[0].photo_id, "photo-2");
        assert_eq!(evaluation_repo.saved.borrow().len(), 1);
        assert_eq!(
            evaluation_repo.saved.borrow()[0].prompt_version,
            infrastructure::ai::PHOTO_EVALUATION_PROMPT.version
        );
    }

    #[tokio::test]
    async fn scoring_run_errors_when_every_photo_fails() {
        let settings_repo = InMemorySettingsRepository::new(settings());
        let photo_repo = InMemoryPhotoRepository::new(vec![photo("photo-1", "bad.jpg")]);
        let evaluation_repo = InMemoryEvaluationRepository::new();
        let scorer = FakeScoringService {
            failures: vec!["bad".to_string()],
        };

        let error = score_photos_with(
            &settings_repo,
            &photo_repo,
            &evaluation_repo,
            &scorer,
            &NoopProgressReporter,
            vec!["photo-1".to_string()],
            false,
        )
        .await
        .expect_err("all failures should be surfaced");

        assert!(matches!(error, PhotoCurateError::ScoringRunFailed(_)));
        assert!(evaluation_repo.saved.borrow().is_empty());
    }

    #[tokio::test]
    async fn update_ai_settings_with_normalizes_provider_defaults_and_sanitizes_key() {
        let settings_repo = InMemorySettingsRepository::new(settings());

        let updated = update_ai_settings_with(
            &settings_repo,
            AiSettings {
                scoring_provider: "qwen_vl".to_string(),
                scoring_model: String::new(),
                scoring_base_url: String::new(),
                scoring_api_key: String::new(),
                has_scoring_api_key: false,
                ..settings()
            },
        )
        .await
        .expect("settings update");

        assert_eq!(updated.scoring_provider, "qwen_vl");
        assert_eq!(
            updated.scoring_model,
            infrastructure::ai::default_scoring_model("qwen_vl")
        );
        assert_eq!(
            updated.scoring_base_url,
            infrastructure::ai::default_scoring_base_url("qwen_vl")
        );
        assert!(!updated.has_scoring_api_key);
        assert!(updated.scoring_api_key.is_empty());
    }
}
