use crate::domain::models::Directory;
use crate::error::Result;
use crate::infrastructure;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn add_directory(
    db: &Pool<Sqlite>,
    thumbnail_dir: &Path,
    monitors: &Arc<Mutex<HashMap<String, (String, notify::RecommendedWatcher)>>>,
    path: String,
) -> Result<Directory> {
    let dir_repo = infrastructure::repositories::SqliteDirectoryRepository::new(db.clone());

    if let Some(existing) = dir_repo.find_by_path(&path).await? {
        let _ = start_monitoring(monitors, &existing.path, &existing.id).await;
        let _ = rescan_directory(db, thumbnail_dir, &existing.path, &existing.id).await;
        return Ok(existing);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let bookmark_data = create_bookmark(&path).ok();
    let directory = Directory::new(id.clone(), path.clone(), bookmark_data);

    dir_repo.save(&directory).await?;

    if let Err(e) = start_monitoring(monitors, &path, &id).await {
        tracing::warn!("Failed to start monitoring {}: {}", path, e);
    }
    let _ = rescan_directory(db, thumbnail_dir, &path, &id).await;

    Ok(directory)
}

pub async fn get_directories(db: &Pool<Sqlite>) -> Result<Vec<Directory>> {
    let dir_repo = infrastructure::repositories::SqliteDirectoryRepository::new(db.clone());
    dir_repo.find_all().await
}

pub async fn remove_directory(
    db: &Pool<Sqlite>,
    monitors: &Arc<Mutex<HashMap<String, (String, notify::RecommendedWatcher)>>>,
    id: String,
) -> Result<()> {
    let dir_repo = infrastructure::repositories::SqliteDirectoryRepository::new(db.clone());
    dir_repo.delete(&id).await?;

    let mut mons = monitors.lock().await;
    if let Some((_, watcher)) = mons.remove(&id) {
        drop(watcher);
    }
    Ok(())
}

async fn start_monitoring(
    monitors: &Arc<Mutex<HashMap<String, (String, notify::RecommendedWatcher)>>>,
    path: &str,
    dir_id: &str,
) -> Result<()> {
    use notify::{RecursiveMode, Watcher};
    let path = path.to_string();
    let dir_id = dir_id.to_string();
    let mut watcher = notify::recommended_watcher(move |res| {
        match res {
            Ok(event) => tracing::debug!("FS event: {:?}", event),
            Err(e) => tracing::error!("Watch error: {:?}", e),
        }
    })?;

    watcher.watch(Path::new(&path), RecursiveMode::Recursive)?;

    let mut mons = monitors.lock().await;
    mons.insert(dir_id, (path, watcher));
    Ok(())
}

async fn rescan_directory(
    db: &Pool<Sqlite>,
    thumbnail_dir: &Path,
    path: &str,
    dir_id: &str,
) -> Result<()> {
    tracing::info!("Scanning directory: {}", path);
    let photos = infrastructure::fs::scan_directory(path, dir_id).await?;
    tracing::info!("Found {} photos", photos.len());

    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    for photo in &photos {
        match photo_repo.insert_or_ignore(photo).await {
            Ok(_) => tracing::info!("Inserted photo {}", photo.file_name),
            Err(e) => tracing::error!("Failed to insert photo {}: {}", photo.file_name, e),
        }
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

fn create_bookmark(_path: &str) -> Result<Vec<u8>> {
    // TODO: Implement security-scoped bookmark for macOS App Store
    Ok(vec![])
}
