use crate::domain::models::{ExportFailure, ExportResult};
use crate::domain::services::resolve_conflict;
use crate::error::Result;
use crate::infrastructure;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::path::PathBuf;

pub async fn export_photos(
    db: &Pool<Sqlite>,
    photo_ids: Vec<String>,
    destination: String,
    preserve_structure: Option<bool>,
) -> Result<ExportResult> {
    let dest = PathBuf::from(&destination);
    std::fs::create_dir_all(&dest)?;
    let preserve = preserve_structure.unwrap_or(false);

    let unique_ids: Vec<String> = photo_ids
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if unique_ids.is_empty() {
        return Ok(ExportResult::empty());
    }

    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    let photos_to_export = photo_repo.find_by_ids(&unique_ids).await?;

    let mut dir_map: HashMap<String, PathBuf> = HashMap::new();
    if preserve {
        let dir_ids: Vec<String> = photos_to_export
            .iter()
            .filter_map(|p| p.directory_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if !dir_ids.is_empty() {
            let dir_repo =
                infrastructure::repositories::SqliteDirectoryRepository::new(db.clone());
            let dirs = dir_repo.find_by_ids(&dir_ids).await?;
            for dir in dirs {
                dir_map.insert(dir.id, PathBuf::from(dir.path));
            }
        }
    }

    let mut exported_count = 0usize;
    let mut failed_photos = Vec::new();

    for photo in photos_to_export {
        let src = PathBuf::from(&photo.file_path);
        if !src.exists() {
            failed_photos.push(ExportFailure {
                id: photo.id.clone(),
                file_name: photo.file_name.clone(),
                error: "源文件不存在".to_string(),
            });
            continue;
        }

        let dest_file = if preserve {
            if let Some(dir_id) = &photo.directory_id {
                if let Some(dir_path) = dir_map.get(dir_id) {
                    if let Ok(relative) = src.strip_prefix(dir_path) {
                        let target_dir = if let Some(parent) = relative.parent() {
                            dest.join(parent)
                        } else {
                            dest.clone()
                        };
                        std::fs::create_dir_all(&target_dir)?;
                        target_dir.join(&photo.file_name)
                    } else {
                        dest.join(&photo.file_name)
                    }
                } else {
                    dest.join(&photo.file_name)
                }
            } else {
                dest.join(&photo.file_name)
            }
        } else {
            dest.join(&photo.file_name)
        };

        let final_dest = resolve_conflict(&dest_file);

        match std::fs::copy(&src, &final_dest) {
            Ok(_) => {
                photo_repo.update_export_status(&photo.id).await?;
                exported_count += 1;
                tracing::info!("Exported {} -> {:?}", photo.file_path, final_dest);
            }
            Err(e) => {
                failed_photos.push(ExportFailure {
                    id: photo.id.clone(),
                    file_name: photo.file_name.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(ExportResult {
        exported_count,
        failed_count: failed_photos.len(),
        failed_photos,
    })
}
