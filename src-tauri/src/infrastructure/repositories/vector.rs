use crate::application::ports::{EmbeddingRepository, VectorRepository};
use crate::error::Result;
use sqlx::{Pool, Sqlite};

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
            r#"INSERT INTO vector_entries (id, photo_id, vector, created_at)
               VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT(photo_id) DO UPDATE SET
                   vector = excluded.vector,
                   created_at = excluded.created_at"#,
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

pub struct SqliteEmbeddingRepository {
    db: Pool<Sqlite>,
}

impl SqliteEmbeddingRepository {
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self { db }
    }
}

impl EmbeddingRepository for SqliteEmbeddingRepository {
    async fn upsert_embedding(
        &self,
        photo_id: &str,
        vector_json: &str,
        version: i32,
    ) -> Result<()> {
        let mut tx = self.db.begin().await?;

        sqlx::query(
            r#"INSERT INTO vector_entries (id, photo_id, vector, created_at)
               VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT(photo_id) DO UPDATE SET
                   vector = excluded.vector,
                   created_at = excluded.created_at"#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(photo_id)
        .bind(vector_json)
        .bind(chrono::Utc::now())
        .execute(&mut *tx)
        .await?;

        sqlx::query("UPDATE photos SET has_embedding = 1, embedding_version = ?1 WHERE id = ?2")
            .bind(version)
            .bind(photo_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn replace_all_embeddings(
        &self,
        embeddings: &[(String, String)],
        version: i32,
    ) -> Result<()> {
        let mut tx = self.db.begin().await?;

        sqlx::query("DELETE FROM vector_entries")
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE photos SET has_embedding = 0, embedding_version = NULL")
            .execute(&mut *tx)
            .await?;

        for (photo_id, vector_json) in embeddings {
            sqlx::query(
                r#"INSERT INTO vector_entries (id, photo_id, vector, created_at)
                   VALUES (?1, ?2, ?3, ?4)"#,
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(photo_id)
            .bind(vector_json)
            .bind(chrono::Utc::now())
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "UPDATE photos SET has_embedding = 1, embedding_version = ?1 WHERE id = ?2",
            )
            .bind(version)
            .bind(photo_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
