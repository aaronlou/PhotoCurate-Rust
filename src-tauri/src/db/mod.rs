use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;

pub async fn init_db(app_data_dir: &std::path::Path) -> Result<Pool<Sqlite>> {
    let db_path = app_data_dir.join("photo_curate.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    run_migrations(&pool).await?;
    init_default_settings(&pool).await?;

    Ok(pool)
}

async fn run_migrations(pool: &Pool<Sqlite>) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS directories (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            is_monitoring BOOLEAN NOT NULL DEFAULT 1,
            date_added DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            bookmark_data BLOB
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS photos (
            id TEXT PRIMARY KEY,
            file_path TEXT NOT NULL UNIQUE,
            file_name TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            date_created DATETIME,
            date_modified DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            camera_make TEXT,
            camera_model TEXT,
            lens_model TEXT,
            focal_length REAL,
            aperture REAL,
            shutter_speed REAL,
            iso INTEGER,
            width INTEGER,
            height INTEGER,
            aesthetic_score REAL,
            has_been_scored BOOLEAN NOT NULL DEFAULT 0,
            score_date DATETIME,
            has_embedding BOOLEAN NOT NULL DEFAULT 0,
            embedding_version INTEGER,
            thumbnail_path TEXT,
            directory_id TEXT REFERENCES directories(id) ON DELETE CASCADE,
            has_been_exported BOOLEAN NOT NULL DEFAULT 0,
            export_date DATETIME
        );
        CREATE INDEX IF NOT EXISTS idx_photos_directory ON photos(directory_id);
        CREATE INDEX IF NOT EXISTS idx_photos_score ON photos(aesthetic_score);
        CREATE INDEX IF NOT EXISTS idx_photos_date ON photos(date_modified);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS vector_entries (
            id TEXT PRIMARY KEY,
            photo_id TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
            vector TEXT NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_vectors_photo ON vector_entries(photo_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS ai_settings (
            id TEXT PRIMARY KEY DEFAULT 'default',
            provider TEXT NOT NULL DEFAULT 'gemini',
            api_key TEXT NOT NULL DEFAULT '',
            ollama_base_url TEXT NOT NULL DEFAULT 'http://localhost:11434',
            ollama_embed_model TEXT NOT NULL DEFAULT 'nomic-embed-text',
            ollama_vision_model TEXT NOT NULL DEFAULT 'llava'
        );
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn init_default_settings(pool: &Pool<Sqlite>) -> Result<()> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO ai_settings (id, provider, api_key, ollama_base_url, ollama_embed_model, ollama_vision_model)
        VALUES ('default', 'gemini', '', 'http://localhost:11434', 'nomic-embed-text', 'llava')
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}
