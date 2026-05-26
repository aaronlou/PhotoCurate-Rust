use crate::application::ports::{
    DirectoryRepository, PhotoRepository, SettingsRepository, VectorRepository,
};
use crate::domain::models::{AiSettings, Directory, Photo, PhotoSortOrder};
use crate::error::Result;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite};

#[derive(sqlx::FromRow)]
struct PhotoRow {
    id: String,
    file_path: String,
    file_name: String,
    file_size: i64,
    date_created: Option<DateTime<Utc>>,
    date_modified: DateTime<Utc>,
    camera_make: Option<String>,
    camera_model: Option<String>,
    lens_model: Option<String>,
    focal_length: Option<f64>,
    aperture: Option<f64>,
    shutter_speed: Option<f64>,
    iso: Option<i32>,
    width: Option<i32>,
    height: Option<i32>,
    aesthetic_score: Option<f64>,
    has_been_scored: bool,
    score_date: Option<DateTime<Utc>>,
    has_embedding: bool,
    embedding_version: Option<i32>,
    thumbnail_path: Option<String>,
    directory_id: Option<String>,
    has_been_exported: bool,
    export_date: Option<DateTime<Utc>>,
}

impl From<PhotoRow> for Photo {
    fn from(row: PhotoRow) -> Self {
        Self {
            id: row.id,
            file_path: row.file_path,
            file_name: row.file_name,
            file_size: row.file_size,
            date_created: row.date_created,
            date_modified: row.date_modified,
            camera_make: row.camera_make,
            camera_model: row.camera_model,
            lens_model: row.lens_model,
            focal_length: row.focal_length,
            aperture: row.aperture,
            shutter_speed: row.shutter_speed,
            iso: row.iso,
            width: row.width,
            height: row.height,
            aesthetic_score: row.aesthetic_score,
            has_been_scored: row.has_been_scored,
            score_date: row.score_date,
            has_embedding: row.has_embedding,
            embedding_version: row.embedding_version,
            thumbnail_path: row.thumbnail_path,
            directory_id: row.directory_id,
            has_been_exported: row.has_been_exported,
            export_date: row.export_date,
        }
    }
}

#[derive(sqlx::FromRow)]
struct DirectoryRow {
    id: String,
    path: String,
    is_monitoring: bool,
    date_added: DateTime<Utc>,
    bookmark_data: Option<Vec<u8>>,
}

impl From<DirectoryRow> for Directory {
    fn from(row: DirectoryRow) -> Self {
        Self {
            id: row.id,
            path: row.path,
            is_monitoring: row.is_monitoring,
            date_added: row.date_added,
            bookmark_data: row.bookmark_data,
        }
    }
}

#[derive(sqlx::FromRow)]
struct AiSettingsRow {
    id: String,
    provider: String,
    api_key: String,
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
            has_api_key: false,
            ollama_base_url: row.ollama_base_url,
            ollama_embed_model: row.ollama_embed_model,
            ollama_vision_model: row.ollama_vision_model,
        }
    }
}

pub struct SqlitePhotoRepository {
    db: Pool<Sqlite>,
}

impl SqlitePhotoRepository {
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self { db }
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<Photo>> {
        let photo = sqlx::query_as::<_, PhotoRow>("SELECT * FROM photos WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        Ok(photo.map(Into::into))
    }

    pub async fn find_all(&self, sort_order: Option<&PhotoSortOrder>) -> Result<Vec<Photo>> {
        let query = match sort_order {
            Some(PhotoSortOrder::ScoreDesc) => {
                "SELECT * FROM photos ORDER BY aesthetic_score IS NULL, aesthetic_score DESC"
            }
            Some(PhotoSortOrder::ScoreAsc) => {
                "SELECT * FROM photos ORDER BY aesthetic_score IS NULL, aesthetic_score ASC"
            }
            _ => "SELECT * FROM photos ORDER BY date_modified DESC",
        };
        let photos = sqlx::query_as::<_, PhotoRow>(query)
            .fetch_all(&self.db)
            .await?;
        Ok(photos.into_iter().map(Into::into).collect())
    }

    pub async fn insert_or_ignore(&self, photo: &Photo) -> Result<()> {
        sqlx::query(
            r#"INSERT OR IGNORE INTO photos (
                id, file_path, file_name, file_size, date_created, date_modified,
                camera_make, camera_model, lens_model, focal_length, aperture,
                shutter_speed, iso, width, height, aesthetic_score, has_been_scored,
                score_date, has_embedding, embedding_version, thumbnail_path,
                directory_id, has_been_exported, export_date
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)"#,
        )
        .bind(&photo.id)
        .bind(&photo.file_path)
        .bind(&photo.file_name)
        .bind(photo.file_size)
        .bind(photo.date_created)
        .bind(photo.date_modified)
        .bind(&photo.camera_make)
        .bind(&photo.camera_model)
        .bind(&photo.lens_model)
        .bind(photo.focal_length)
        .bind(photo.aperture)
        .bind(photo.shutter_speed)
        .bind(photo.iso)
        .bind(photo.width)
        .bind(photo.height)
        .bind(photo.aesthetic_score)
        .bind(photo.has_been_scored)
        .bind(photo.score_date)
        .bind(photo.has_embedding)
        .bind(photo.embedding_version)
        .bind(&photo.thumbnail_path)
        .bind(&photo.directory_id)
        .bind(photo.has_been_exported)
        .bind(photo.export_date)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn update_thumbnail(&self, id: &str, path: &str) -> Result<()> {
        sqlx::query("UPDATE photos SET thumbnail_path = ?1 WHERE id = ?2")
            .bind(path)
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn update_score(&self, id: &str, score: f64) -> Result<()> {
        sqlx::query(
            r#"UPDATE photos
               SET aesthetic_score = ?1, has_been_scored = 1, score_date = ?2
               WHERE id = ?3"#,
        )
        .bind(score)
        .bind(chrono::Utc::now())
        .bind(id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn update_embedding(&self, id: &str, version: i32) -> Result<()> {
        sqlx::query("UPDATE photos SET has_embedding = 1, embedding_version = ?1 WHERE id = ?2")
            .bind(version)
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn reset_all_embeddings(&self) -> Result<()> {
        sqlx::query("UPDATE photos SET has_embedding = 0, embedding_version = NULL")
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn update_export_status(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE photos SET has_been_exported = 1, export_date = ?1 WHERE id = ?2")
            .bind(chrono::Utc::now())
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn find_by_ids(&self, ids: &[String]) -> Result<Vec<Photo>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
        let query = format!(
            "SELECT * FROM photos WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut q = sqlx::query_as::<_, PhotoRow>(&query);
        for id in ids {
            q = q.bind(id);
        }
        let photos = q.fetch_all(&self.db).await?;
        Ok(photos.into_iter().map(Into::into).collect())
    }

    pub async fn find_unindexed(&self) -> Result<Vec<Photo>> {
        let photos = sqlx::query_as::<_, PhotoRow>("SELECT * FROM photos WHERE has_embedding = 0")
            .fetch_all(&self.db)
            .await?;
        Ok(photos.into_iter().map(Into::into).collect())
    }
}

impl PhotoRepository for SqlitePhotoRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<Photo>> {
        SqlitePhotoRepository::find_by_id(self, id).await
    }

    async fn find_all(&self, sort_order: Option<&PhotoSortOrder>) -> Result<Vec<Photo>> {
        SqlitePhotoRepository::find_all(self, sort_order).await
    }

    async fn insert_or_ignore(&self, photo: &Photo) -> Result<()> {
        SqlitePhotoRepository::insert_or_ignore(self, photo).await
    }

    async fn update_thumbnail(&self, id: &str, path: &str) -> Result<()> {
        SqlitePhotoRepository::update_thumbnail(self, id, path).await
    }

    async fn update_score(&self, id: &str, score: f64) -> Result<()> {
        SqlitePhotoRepository::update_score(self, id, score).await
    }

    async fn update_embedding(&self, id: &str, version: i32) -> Result<()> {
        SqlitePhotoRepository::update_embedding(self, id, version).await
    }

    async fn reset_all_embeddings(&self) -> Result<()> {
        SqlitePhotoRepository::reset_all_embeddings(self).await
    }

    async fn update_export_status(&self, id: &str) -> Result<()> {
        SqlitePhotoRepository::update_export_status(self, id).await
    }

    async fn find_by_ids(&self, ids: &[String]) -> Result<Vec<Photo>> {
        SqlitePhotoRepository::find_by_ids(self, ids).await
    }

    async fn find_unindexed(&self) -> Result<Vec<Photo>> {
        SqlitePhotoRepository::find_unindexed(self).await
    }
}

pub struct SqliteDirectoryRepository {
    db: Pool<Sqlite>,
}

impl SqliteDirectoryRepository {
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self { db }
    }

    pub async fn find_by_path(&self, path: &str) -> Result<Option<Directory>> {
        let dir = sqlx::query_as::<_, DirectoryRow>("SELECT * FROM directories WHERE path = ?1")
            .bind(path)
            .fetch_optional(&self.db)
            .await?;
        Ok(dir.map(Into::into))
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<Directory>> {
        let dir = sqlx::query_as::<_, DirectoryRow>("SELECT * FROM directories WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        Ok(dir.map(Into::into))
    }

    pub async fn find_all(&self) -> Result<Vec<Directory>> {
        let dirs =
            sqlx::query_as::<_, DirectoryRow>("SELECT * FROM directories ORDER BY date_added DESC")
                .fetch_all(&self.db)
                .await?;
        Ok(dirs.into_iter().map(Into::into).collect())
    }

    pub async fn save(&self, directory: &Directory) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO directories (id, path, is_monitoring, date_added, bookmark_data)
               VALUES (?1, ?2, ?3, ?4, ?5)"#,
        )
        .bind(&directory.id)
        .bind(&directory.path)
        .bind(directory.is_monitoring)
        .bind(directory.date_added)
        .bind(&directory.bookmark_data)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM directories WHERE id = ?1")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn find_by_ids(&self, ids: &[String]) -> Result<Vec<Directory>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
        let query = format!(
            "SELECT * FROM directories WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut q = sqlx::query_as::<_, DirectoryRow>(&query);
        for id in ids {
            q = q.bind(id);
        }
        let dirs = q.fetch_all(&self.db).await?;
        Ok(dirs.into_iter().map(Into::into).collect())
    }

    pub async fn update_path(&self, id: &str, path: &str) -> Result<()> {
        sqlx::query("UPDATE directories SET path = ?1 WHERE id = ?2")
            .bind(path)
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}

impl DirectoryRepository for SqliteDirectoryRepository {
    async fn find_by_path(&self, path: &str) -> Result<Option<Directory>> {
        SqliteDirectoryRepository::find_by_path(self, path).await
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Directory>> {
        SqliteDirectoryRepository::find_by_id(self, id).await
    }

    async fn find_all(&self) -> Result<Vec<Directory>> {
        SqliteDirectoryRepository::find_all(self).await
    }

    async fn save(&self, directory: &Directory) -> Result<()> {
        SqliteDirectoryRepository::save(self, directory).await
    }

    async fn delete(&self, id: &str) -> Result<()> {
        SqliteDirectoryRepository::delete(self, id).await
    }

    async fn find_by_ids(&self, ids: &[String]) -> Result<Vec<Directory>> {
        SqliteDirectoryRepository::find_by_ids(self, ids).await
    }

    async fn update_path(&self, id: &str, path: &str) -> Result<()> {
        SqliteDirectoryRepository::update_path(self, id, path).await
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
               provider = ?1, api_key = ?2, ollama_base_url = ?3,
               ollama_embed_model = ?4, ollama_vision_model = ?5
               WHERE id = 'default'"#,
        )
        .bind(&settings.provider)
        .bind(&settings.api_key)
        .bind(&settings.ollama_base_url)
        .bind(&settings.ollama_embed_model)
        .bind(&settings.ollama_vision_model)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

impl VectorRepository for SqliteVectorRepository {
    async fn find_all(&self) -> Result<Vec<(String, String)>> {
        SqliteVectorRepository::find_all(self).await
    }

    async fn save(&self, id: &str, photo_id: &str, vector_json: &str) -> Result<()> {
        SqliteVectorRepository::save(self, id, photo_id, vector_json).await
    }

    async fn delete_all(&self) -> Result<()> {
        SqliteVectorRepository::delete_all(self).await
    }
}

pub struct SqliteVectorRepository {
    db: Pool<Sqlite>,
}

impl SqliteVectorRepository {
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self { db }
    }

    pub async fn find_all(&self) -> Result<Vec<(String, String)>> {
        let vectors: Vec<(String, String)> =
            sqlx::query_as("SELECT photo_id, vector FROM vector_entries")
                .fetch_all(&self.db)
                .await?;
        Ok(vectors)
    }

    pub async fn save(&self, id: &str, photo_id: &str, vector_json: &str) -> Result<()> {
        sqlx::query(
            r#"INSERT OR REPLACE INTO vector_entries (id, photo_id, vector, created_at)
               VALUES (?1, ?2, ?3, ?4)"#,
        )
        .bind(id)
        .bind(photo_id)
        .bind(vector_json)
        .bind(chrono::Utc::now())
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn delete_all(&self) -> Result<()> {
        sqlx::query("DELETE FROM vector_entries")
            .execute(&self.db)
            .await?;
        Ok(())
    }
}
