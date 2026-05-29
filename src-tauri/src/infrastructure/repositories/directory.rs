use crate::application::ports::DirectoryRepository;
use crate::domain::models::Directory;
use crate::error::Result;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite};

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
