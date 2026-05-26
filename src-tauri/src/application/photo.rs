use crate::application::ports::{DirectoryRepository, PhotoFileGateway, PhotoRepository};
use crate::domain::models::{Photo, PhotoSortOrder};
use crate::error::{PhotoCurateError, Result};
use crate::infrastructure;
use sqlx::{Pool, Sqlite};
use std::path::Path;

pub async fn get_photos(
    db: &Pool<Sqlite>,
    sort_order: Option<PhotoSortOrder>,
) -> Result<Vec<Photo>> {
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    get_photos_with(&photo_repo, sort_order).await
}

pub async fn get_photos_with(
    photos: &impl PhotoRepository,
    sort_order: Option<PhotoSortOrder>,
) -> Result<Vec<Photo>> {
    photos.find_all(sort_order.as_ref()).await
}

pub async fn get_photo_by_id(db: &Pool<Sqlite>, id: String) -> Result<Option<Photo>> {
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    get_photo_by_id_with(&photo_repo, id).await
}

pub async fn get_photo_by_id_with(
    photos: &impl PhotoRepository,
    id: String,
) -> Result<Option<Photo>> {
    photos.find_by_id(&id).await
}

pub async fn get_thumbnail_path(
    db: &Pool<Sqlite>,
    thumbnail_dir: &Path,
    photo_id: String,
) -> Result<Option<String>> {
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let file_gateway = infrastructure::adapters::LocalPhotoFileGateway;

    get_thumbnail_path_with(&photo_repo, &file_gateway, thumbnail_dir, photo_id).await
}

pub async fn get_thumbnail_path_with(
    photos: &impl PhotoRepository,
    files: &impl PhotoFileGateway,
    thumbnail_dir: &Path,
    photo_id: String,
) -> Result<Option<String>> {
    let photo = photos.find_by_id(&photo_id).await?;

    if let Some(photo) = photo {
        let thumb = files
            .generate_thumbnail(&photo.file_path, thumbnail_dir, 400)
            .await?;
        photos.update_thumbnail(&photo_id, &thumb).await?;
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
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let file_gateway = infrastructure::adapters::LocalPhotoFileGateway;

    scan_directory_with(
        &dir_repo,
        &photo_repo,
        &file_gateway,
        thumbnail_dir,
        directory_id,
    )
    .await
}

pub async fn scan_directory_with(
    directories: &impl DirectoryRepository,
    photos: &impl PhotoRepository,
    files: &impl PhotoFileGateway,
    thumbnail_dir: &Path,
    directory_id: &str,
) -> Result<()> {
    let directory = directories
        .find_by_id(directory_id)
        .await?
        .ok_or_else(|| PhotoCurateError::DirectoryNotFound(directory_id.to_string()))?;

    let scanned = files.scan_directory(&directory.path, directory_id).await?;

    for photo in &scanned {
        photos.insert_or_ignore(photo).await?;
    }

    for photo in scanned {
        if let Ok(thumb_path) = files
            .generate_thumbnail(&photo.file_path, thumbnail_dir, 400)
            .await
        {
            let _ = photos.update_thumbnail(&photo.id, &thumb_path).await;
        }
    }

    Ok(())
}
