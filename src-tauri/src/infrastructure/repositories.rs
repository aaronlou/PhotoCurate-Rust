use crate::domain::models::{AiSettings, Directory, Photo};
use crate::error::Result;
use sqlx::{Pool, Sqlite};

pub struct SqlitePhotoRepository {
    db: Pool<Sqlite>,
}

impl SqlitePhotoRepository {
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self { db }
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<Photo>> {
        let photo = sqlx::query_as::<_, Photo>("SELECT * FROM photos WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        Ok(photo)
    }

    pub async fn find_all(&self, sort_order: Option<&crate::domain::models::PhotoSortOrder>) -> Result<Vec<Photo>> {
        let query = match sort_order {
            Some(crate::domain::models::PhotoSortOrder::ScoreDesc) => {
                "SELECT * FROM photos ORDER BY aesthetic_score IS NULL, aesthetic_score DESC"
            }
            Some(crate::domain::models::PhotoSortOrder::ScoreAsc) => {
                "SELECT * FROM photos ORDER BY aesthetic_score IS NULL, aesthetic_score ASC"
            }
            _ => "SELECT * FROM photos ORDER BY date_modified DESC",
        };
        let photos = sqlx::query_as::<_, Photo>(query)
            .fetch_all(&self.db)
            .await?;
        Ok(photos)
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
        sqlx::query(
            "UPDATE photos SET has_embedding = 1, embedding_version = ?1 WHERE id = ?2",
        )
        .bind(version)
        .bind(id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn reset_all_embeddings(&self) -> Result<()> {
        sqlx::query(
            "UPDATE photos SET has_embedding = 0, embedding_version = NULL",
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn update_export_status(&self, id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE photos SET has_been_exported = 1, export_date = ?1 WHERE id = ?2",
        )
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
        let query = format!("SELECT * FROM photos WHERE id IN ({})", placeholders.join(", "));
        let mut q = sqlx::query_as::<_, Photo>(&query);
        for id in ids {
            q = q.bind(id);
        }
        let photos = q.fetch_all(&self.db).await?;
        Ok(photos)
    }

    pub async fn find_unindexed(&self) -> Result<Vec<Photo>> {
        let photos = sqlx::query_as::<_, Photo>(
            "SELECT * FROM photos WHERE has_embedding = 0",
        )
        .fetch_all(&self.db)
        .await?;
        Ok(photos)
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
        let dir = sqlx::query_as::<_, Directory>("SELECT * FROM directories WHERE path = ?1")
            .bind(path)
            .fetch_optional(&self.db)
            .await?;
        Ok(dir)
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<Directory>> {
        let dir = sqlx::query_as::<_, Directory>("SELECT * FROM directories WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        Ok(dir)
    }

    pub async fn find_all(&self) -> Result<Vec<Directory>> {
        let dirs =
            sqlx::query_as::<_, Directory>("SELECT * FROM directories ORDER BY date_added DESC")
                .fetch_all(&self.db)
                .await?;
        Ok(dirs)
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
        let mut q = sqlx::query_as::<_, Directory>(&query);
        for id in ids {
            q = q.bind(id);
        }
        let dirs = q.fetch_all(&self.db).await?;
        Ok(dirs)
    }
}

pub struct SqliteSettingsRepository {
    db: Pool<Sqlite>,
}

impl SqliteSettingsRepository {
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self { db }
    }

    pub async fn get(&self) -> Result<AiSettings> {
        let settings =
            sqlx::query_as::<_, AiSettings>("SELECT * FROM ai_settings WHERE id = 'default'")
                .fetch_one(&self.db)
                .await?;
        Ok(settings)
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
