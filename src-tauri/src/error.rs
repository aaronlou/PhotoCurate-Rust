use thiserror::Error;

#[derive(Error, Debug)]
pub enum PhotoCurateError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("AI service error: {0}")]
    Ai(String),

    #[error("photo not found: {0}")]
    PhotoNotFound(String),

    #[error("directory not found: {0}")]
    DirectoryNotFound(String),

    #[error("settings not found")]
    SettingsNotFound,

    #[error("API key not configured")]
    ApiKeyMissing,

    #[error("all scoring attempts failed: {0}")]
    ScoringRunFailed(String),

    #[error("embedding service not configured")]
    EmbeddingServiceMissing,

    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error("image processing error: {0}")]
    Image(String),

    #[error("file watcher error: {0}")]
    Notify(String),

    #[error("other error: {0}")]
    Other(String),
}

impl From<anyhow::Error> for PhotoCurateError {
    fn from(err: anyhow::Error) -> Self {
        PhotoCurateError::Other(err.to_string())
    }
}

impl From<notify::Error> for PhotoCurateError {
    fn from(err: notify::Error) -> Self {
        PhotoCurateError::Notify(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PhotoCurateError>;
