use crate::domain::models::Photo;
use crate::error::{PhotoCurateError, Result};
use crate::infrastructure;
use sqlx::{Pool, Sqlite};
use std::path::Path;

pub async fn get_photos(db: &Pool<Sqlite>) -> Result<Vec<Photo>> {
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    photo_repo.find_all().await
}

pub async fn get_photo_by_id(db: &Pool<Sqlite>, id: String) -> Result<Option<Photo>> {
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    photo_repo.find_by_id(&id).await
}

pub async fn get_thumbnail_path(
    db: &Pool<Sqlite>,
    thumbnail_dir: &Path,
    photo_id: String,
) -> Result<Option<String>> {
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let photo = photo_repo.find_by_id(&photo_id).await?;

    if let Some(photo) = photo {
        let thumb =
            infrastructure::fs::generate_thumbnail(&photo.file_path, thumbnail_dir, 400).await?;
        photo_repo.update_thumbnail(&photo_id, &thumb).await?;
        Ok(Some(thumb))
    } else {
        Ok(None)
    }
}

pub async fn scan_directory(
    db: &Pool<Sqlite>,
    thumbnail_dir: &Path,
    directory_id: &str,
) -> Result<()> {
    let dir_repo = infrastructure::repositories::SqliteDirectoryRepository::new(db.clone());
    let directory = dir_repo
        .find_by_id(directory_id)
        .await?
        .ok_or_else(|| PhotoCurateError::DirectoryNotFound(directory_id.to_string()))?;

    let photos = infrastructure::fs::scan_directory(&directory.path, directory_id).await?;
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());

    for photo in &photos {
        photo_repo.insert_or_ignore(photo).await?;
    }

    for photo in photos {
        if let Ok(thumb_path) =
            infrastructure::fs::generate_thumbnail(&photo.file_path, thumbnail_dir, 400).await
        {
            let _ = photo_repo.update_thumbnail(&photo.id, &thumb_path).await;
        }
    }

    Ok(())
}
