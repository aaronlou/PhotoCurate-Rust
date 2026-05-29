use crate::application::ports::SettingsRepository;
use crate::domain::models::AiSettings;
use crate::error::Result;
use sqlx::{Pool, Sqlite};

#[derive(sqlx::FromRow)]
struct AiSettingsRow {
    id: String,
    provider: String,
    api_key: String,
    has_api_key: bool,
    key_storage: String,
    scoring_provider: String,
    scoring_model: String,
    scoring_base_url: String,
    scoring_api_key: String,
    has_scoring_api_key: bool,
    scoring_key_storage: String,
    ollama_base_url: String,
    ollama_embed_model: String,
    ollama_vision_model: String,
}

impl From<AiSettingsRow> for AiSettings {
    fn from(row: AiSettingsRow) -> Self {
        Self {
            id: row.id,
            provider: row.provider,
            api_key: row.api_key,
            has_api_key: row.has_api_key,
            key_storage: row.key_storage,
            scoring_provider: row.scoring_provider,
            scoring_model: row.scoring_model,
            scoring_base_url: row.scoring_base_url,
            scoring_api_key: row.scoring_api_key,
            has_scoring_api_key: row.has_scoring_api_key,
            scoring_key_storage: row.scoring_key_storage,
            ollama_base_url: row.ollama_base_url,
            ollama_embed_model: row.ollama_embed_model,
            ollama_vision_model: row.ollama_vision_model,
        }
    }
}

pub struct SqliteSettingsRepository {
    db: Pool<Sqlite>,
}

impl SettingsRepository for SqliteSettingsRepository {
    async fn get(&self) -> Result<AiSettings> {
        SqliteSettingsRepository::get(self).await
    }

    async fn update(&self, settings: &AiSettings) -> Result<()> {
        SqliteSettingsRepository::update(self, settings).await
    }
}

impl SqliteSettingsRepository {
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self { db }
    }

    pub async fn get(&self) -> Result<AiSettings> {
        let settings =
            sqlx::query_as::<_, AiSettingsRow>("SELECT * FROM ai_settings WHERE id = 'default'")
                .fetch_one(&self.db)
                .await?;
        Ok(settings.into())
    }

    pub async fn update(&self, settings: &AiSettings) -> Result<()> {
        sqlx::query(
            r#"UPDATE ai_settings SET
               provider = ?1, api_key = ?2, has_api_key = ?3,
               key_storage = ?4, scoring_provider = ?5,
               scoring_model = ?6, scoring_base_url = ?7,
               scoring_api_key = ?8, has_scoring_api_key = ?9,
               scoring_key_storage = ?10, ollama_base_url = ?11,
               ollama_embed_model = ?12, ollama_vision_model = ?13
               WHERE id = 'default'"#,
        )
        .bind(&settings.provider)
        .bind(&settings.api_key)
        .bind(settings.has_api_key)
        .bind(&settings.key_storage)
        .bind(&settings.scoring_provider)
        .bind(&settings.scoring_model)
        .bind(&settings.scoring_base_url)
        .bind(&settings.scoring_api_key)
        .bind(settings.has_scoring_api_key)
        .bind(&settings.scoring_key_storage)
        .bind(&settings.ollama_base_url)
        .bind(&settings.ollama_embed_model)
        .bind(&settings.ollama_vision_model)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}
