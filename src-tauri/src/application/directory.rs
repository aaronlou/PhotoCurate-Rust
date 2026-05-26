use crate::application::ports::{
    BookmarkGateway, DirectoryMonitor, DirectoryRepository, PhotoFileGateway, PhotoRepository,
};
use crate::domain::models::Directory;
use crate::error::Result;
use crate::infrastructure;
use sqlx::{Pool, Sqlite};
use std::path::Path;

pub async fn add_directory(
    db: &Pool<Sqlite>,
    thumbnail_dir: &Path,
    monitors: &crate::Monitors,
    path: String,
) -> Result<Directory> {
    let dir_repo = infrastructure::repositories::SqliteDirectoryRepository::new(db.clone());
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let file_gateway = infrastructure::adapters::LocalPhotoFileGateway;
    let bookmark_gateway = infrastructure::adapters::BookmarkService;
    let monitor = infrastructure::adapters::NotifyDirectoryMonitor::new(monitors.clone());

    add_directory_with(
        &dir_repo,
        &photo_repo,
        &file_gateway,
        &bookmark_gateway,
        &monitor,
        thumbnail_dir,
        path,
    )
    .await
}

pub async fn add_directory_with(
    directories: &impl DirectoryRepository,
    photos: &impl PhotoRepository,
    files: &impl PhotoFileGateway,
    bookmarks: &impl BookmarkGateway,
    monitor: &impl DirectoryMonitor,
    thumbnail_dir: &Path,
    path: String,
) -> Result<Directory> {
    if let Some(existing) = directories.find_by_path(&path).await? {
        let _ = monitor.start_monitoring(&existing.path, &existing.id).await;
        let _ = rescan_directory(photos, files, thumbnail_dir, &existing.path, &existing.id).await;
        return Ok(existing);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let bookmark_data = bookmarks.create_bookmark(&path);
    let directory = Directory::new(id.clone(), path.clone(), bookmark_data);

    directories.save(&directory).await?;

    if let Err(e) = monitor.start_monitoring(&path, &id).await {
        tracing::warn!("Failed to start monitoring {}: {}", path, e);
    }
    let _ = rescan_directory(photos, files, thumbnail_dir, &path, &id).await;

    Ok(directory)
}

pub async fn get_directories(db: &Pool<Sqlite>) -> Result<Vec<Directory>> {
    let dir_repo = infrastructure::repositories::SqliteDirectoryRepository::new(db.clone());
    dir_repo.find_all().await
}

pub async fn remove_directory(
    db: &Pool<Sqlite>,
    monitors: &crate::Monitors,
    id: String,
) -> Result<()> {
    let dir_repo = infrastructure::repositories::SqliteDirectoryRepository::new(db.clone());
    let bookmark_gateway = infrastructure::adapters::BookmarkService;
    let monitor = infrastructure::adapters::NotifyDirectoryMonitor::new(monitors.clone());

    remove_directory_with(&dir_repo, &bookmark_gateway, &monitor, id).await
}

pub async fn remove_directory_with(
    directories: &impl DirectoryRepository,
    bookmarks: &impl BookmarkGateway,
    monitor: &impl DirectoryMonitor,
    id: String,
) -> Result<()> {
    // Release security-scoped access before deleting
    if let Ok(Some(dir)) = directories.find_by_id(&id).await {
        bookmarks.release_access(&dir.path);
    }

    directories.delete(&id).await?;

    monitor.stop_monitoring(&id).await;
    Ok(())
}

/// Resolve all stored security-scoped bookmarks on startup.
/// Must be called before the app accesses any saved directories
/// (scanning, monitoring, etc.) to regain sandbox access.
pub async fn resolve_bookmarks_on_startup(db: &Pool<Sqlite>) -> Result<()> {
    let dir_repo = infrastructure::repositories::SqliteDirectoryRepository::new(db.clone());
    let bookmark_gateway = infrastructure::adapters::BookmarkService;

    resolve_bookmarks_on_startup_with(&dir_repo, &bookmark_gateway).await
}

pub async fn resolve_bookmarks_on_startup_with(
    directories: &impl DirectoryRepository,
    bookmarks: &impl BookmarkGateway,
) -> Result<()> {
    let saved_directories = directories.find_all().await?;

    for dir in saved_directories {
        let bookmark = match &dir.bookmark_data {
            Some(b) if !b.is_empty() => b,
            _ => {
                tracing::warn!(
                    "Directory '{}' has no bookmark data — access may fail under sandbox",
                    dir.path
                );
                continue;
            }
        };

        match bookmarks.resolve_bookmark(bookmark) {
            Ok(resolved_path) => {
                if resolved_path != dir.path {
                    tracing::info!(
                        "Directory moved: '{}' -> '{}', updating stored path",
                        dir.path,
                        resolved_path
                    );
                    // Still store the updated path for future reference
                    // (the bookmark remains valid regardless)
                    let _ = directories.update_path(&dir.id, &resolved_path).await;
                } else {
                    tracing::info!("Resolved sandbox access for directory: '{}'", dir.path);
                }
            }
            Err(e) => {
                tracing::error!(
                    "Failed to resolve bookmark for '{}': {}. \
                     Directory will be inaccessible until re-added.",
                    dir.path,
                    e
                );
            }
        }
    }

    Ok(())
}

async fn rescan_directory(
    photos: &impl PhotoRepository,
    files: &impl PhotoFileGateway,
    thumbnail_dir: &Path,
    path: &str,
    dir_id: &str,
) -> Result<()> {
    tracing::info!("Scanning directory: {}", path);
    let scanned = files.scan_directory(path, dir_id).await?;
    tracing::info!("Found {} photos", scanned.len());

    for photo in &scanned {
        match photos.insert_or_ignore(photo).await {
            Ok(_) => tracing::info!("Inserted photo {}", photo.file_name),
            Err(e) => tracing::error!("Failed to insert photo {}: {}", photo.file_name, e),
        }
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
