use super::evaluation::{attach_latest_evaluations, SqlitePhotoEvaluationRepository};
use crate::application::ports::{PhotoEvaluationRepository, PhotoRepository};
use crate::domain::models::{Photo, PhotoSortOrder};
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
            latest_evaluation: None,
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
        let Some(photo) = photo else {
            return Ok(None);
        };

        let mut photo: Photo = photo.into();
        photo.latest_evaluation = SqlitePhotoEvaluationRepository::new(self.db.clone())
            .find_latest_for_photo(&photo.id)
            .await?;
        Ok(Some(photo))
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
        let mut photos: Vec<Photo> = photos.into_iter().map(Into::into).collect();
        attach_latest_evaluations(&self.db, &mut photos).await?;
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
        let mut photos: Vec<Photo> = photos.into_iter().map(Into::into).collect();
        attach_latest_evaluations(&self.db, &mut photos).await?;
        Ok(photos)
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
