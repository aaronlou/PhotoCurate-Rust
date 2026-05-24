use crate::models::{AiSettings, Directory, Photo, SearchResult};
use crate::{AppState, Result};
use std::path::PathBuf;
use tauri::State;

// ==================== Directory Commands ====================

#[tauri::command]
pub async fn pick_directory() -> Result<Option<String>> {
    // This is handled by the dialog plugin on the frontend
    // But we can expose a native dialog via Rust if needed
    Ok(None)
}

#[tauri::command]
pub async fn add_directory(
    state: State<'_, AppState>,
    path: String,
) -> Result<Directory> {
    // Check if directory already exists
    let existing: Option<Directory> = sqlx::query_as::<_, Directory>("SELECT * FROM directories WHERE path = ?1")
        .bind(&path)
        .fetch_optional(&state.db)
        .await.map_err(|e| e.to_string())?;

    if let Some(dir) = existing {
        // Directory already exists — ensure monitoring is active and rescan
        if let Err(e) = start_monitoring(&state, &path, &dir.id).await {
            tracing::warn!("Failed to restart monitoring {}: {}", path, e);
        }
        // Rescan for new photos
        let _ = rescan_directory(&state, &path, &dir.id).await;
        return Ok(dir);
    }

    let id = uuid::Uuid::new_v4().to_string();

    // On macOS, try to create a security-scoped bookmark
    let bookmark_data = create_bookmark(&path).ok();

    let dir = Directory {
        id: id.clone(),
        path: path.clone(),
        is_monitoring: true,
        date_added: chrono::Utc::now(),
        bookmark_data,
    };

    sqlx::query(
        r#"INSERT INTO directories (id, path, is_monitoring, date_added, bookmark_data)
           VALUES (?1, ?2, ?3, ?4, ?5)"#,
    )
    .bind(&dir.id)
    .bind(&dir.path)
    .bind(dir.is_monitoring)
    .bind(dir.date_added)
    .bind(&dir.bookmark_data)
    .execute(&state.db)
    .await.map_err(|e| e.to_string())?;

    // Start file system monitoring
    if let Err(e) = start_monitoring(&state, &path, &id).await {
        tracing::warn!("Failed to start monitoring {}: {}", path, e);
    }

    // Scan directory for photos
    let _ = rescan_directory(&state, &path, &id).await;

    Ok(dir)
}

#[tauri::command]
pub async fn get_directories(state: State<'_, AppState>) -> Result<Vec<Directory>> {
    let dirs = sqlx::query_as::<_, Directory>("SELECT * FROM directories ORDER BY date_added DESC")
        .fetch_all(&state.db)
        .await.map_err(|e| e.to_string())?;
    Ok(dirs)
}

#[tauri::command]
pub async fn remove_directory(state: State<'_, AppState>, id: String) -> Result<()> {
    sqlx::query("DELETE FROM directories WHERE id = ?1")
        .bind(&id)
        .execute(&state.db)
        .await.map_err(|e| e.to_string())?;

    let mut monitors = state.monitors.lock().await;
    if let Some((_, watcher)) = monitors.remove(&id) {
        drop(watcher);
    }
    Ok(())
}

// ==================== Photo Commands ====================

#[tauri::command]
pub async fn get_photos(state: State<'_, AppState>) -> Result<Vec<Photo>> {
    let photos = sqlx::query_as::<_, Photo>(
        "SELECT * FROM photos ORDER BY date_modified DESC"
    )
    .fetch_all(&state.db)
    .await.map_err(|e| e.to_string())?;
    Ok(photos)
}

#[tauri::command]
pub async fn get_photo_by_id(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<Photo>> {
    let photo = sqlx::query_as::<_, Photo>("SELECT * FROM photos WHERE id = ?1")
        .bind(&id)
        .fetch_optional(&state.db)
        .await.map_err(|e| e.to_string())?;
    Ok(photo)
}

#[tauri::command]
pub async fn get_thumbnail_path(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<Option<String>> {
    let photo = sqlx::query_as::<_, Photo>("SELECT * FROM photos WHERE id = ?1")
        .bind(&photo_id)
        .fetch_optional(&state.db)
        .await.map_err(|e| e.to_string())?;

    if let Some(photo) = photo {
        let thumb = crate::fs::generate_thumbnail(&photo.file_path, &state.thumbnail_dir, 400).await.map_err(|e| e.to_string())?;
        // Update thumbnail path in DB
        sqlx::query("UPDATE photos SET thumbnail_path = ?1 WHERE id = ?2")
            .bind(&thumb)
            .bind(&photo_id)
            .execute(&state.db)
            .await.map_err(|e| e.to_string())?;
        Ok(Some(thumb))
    } else {
        Ok(None)
    }
}

// ==================== Scanning ====================

#[tauri::command]
pub async fn start_scanning(
    state: State<'_, AppState>,
    directory_id: String,
) -> Result<()> {
    let dir: Directory = sqlx::query_as("SELECT * FROM directories WHERE id = ?1")
        .bind(&directory_id)
        .fetch_one(&state.db)
        .await.map_err(|e| e.to_string())?;

    let photos = crate::fs::scan_directory(&dir.path, &directory_id).await.map_err(|e| e.to_string())?;
    for photo in &photos {
        insert_photo(&state.db, photo).await.map_err(|e| e.to_string())?;
    }

    for photo in photos {
        if let Ok(thumb_path) = crate::fs::generate_thumbnail(&photo.file_path, &state.thumbnail_dir, 400).await {
            sqlx::query("UPDATE photos SET thumbnail_path = ?1 WHERE id = ?2")
                .bind(&thumb_path)
                .bind(&photo.id)
                .execute(&state.db)
                .await.map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

// ==================== Scoring ====================

#[tauri::command]
pub async fn score_photos(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
) -> Result<()> {
    let settings = get_ai_settings_internal(&state.db).await.map_err(|e| e.to_string())?;
    if settings.api_key.is_empty() {
        return Err("API Key 未配置".into());
    }

    for photo_id in photo_ids {
        let photo = sqlx::query_as::<_, Photo>("SELECT * FROM photos WHERE id = ?1")
            .bind(&photo_id)
            .fetch_optional(&state.db)
            .await.map_err(|e| e.to_string())?;

        if let Some(photo) = photo {
            match crate::ai::score_image(&settings.api_key, &photo.file_path).await {
                Ok(result) => {
                    sqlx::query(
                        r#"UPDATE photos
                           SET aesthetic_score = ?1, has_been_scored = 1, score_date = ?2
                           WHERE id = ?3"#,
                    )
                    .bind(result.score)
                    .bind(chrono::Utc::now())
                    .bind(&photo_id)
                    .execute(&state.db)
                    .await.map_err(|e| e.to_string())?;

                    tracing::info!("Scored {} = {}", photo.file_name, result.score);
                }
                Err(e) => {
                    tracing::warn!("Scoring failed for {}: {}", photo.file_name, e);
                }
            }
        }
    }

    Ok(())
}

// ==================== Search Index ====================

#[tauri::command]
pub async fn build_search_index(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
) -> Result<()> {
    let settings = get_ai_settings_internal(&state.db).await.map_err(|e| e.to_string())?;
    let local_service = state.chinese_clip.as_ref();
    
    if local_service.is_none() && settings.api_key.is_empty() {
        return Err("未配置嵌入服务: 请加载本地 Chinese-CLIP 模型或配置 API Key".into());
    }

    for photo_id in photo_ids {
        let photo = sqlx::query_as::<_, Photo>("SELECT * FROM photos WHERE id = ?1")
            .bind(&photo_id)
            .fetch_optional(&state.db)
            .await.map_err(|e| e.to_string())?;

        if let Some(photo) = photo {
            match crate::ai::embed_image(&settings.api_key, &photo.file_path, local_service.cloned()).await {
                Ok(embedding) => {
                    let vector_json = serde_json::to_string(&embedding).map_err(|e| e.to_string())?;
                    sqlx::query(
                        r#"INSERT OR REPLACE INTO vector_entries (id, photo_id, vector, created_at)
                           VALUES (?1, ?2, ?3, ?4)"#,
                    )
                    .bind(uuid::Uuid::new_v4().to_string())
                    .bind(&photo_id)
                    .bind(&vector_json)
                    .bind(chrono::Utc::now())
                    .execute(&state.db)
                    .await.map_err(|e| e.to_string())?;

                    sqlx::query(
                        "UPDATE photos SET has_embedding = 1, embedding_version = 1 WHERE id = ?1"
                    )
                    .bind(&photo_id)
                    .execute(&state.db)
                    .await.map_err(|e| e.to_string())?;

                    let mut index = state.vector_index.write().await;
                    index.add(photo_id, embedding);

                    tracing::info!("Indexed {}", photo.file_name);
                }
                Err(e) => {
                    tracing::warn!("Embedding failed for {}: {}", photo.file_name, e);
                }
            }
        }
    }

    Ok(())
}

// ==================== Search ====================

#[tauri::command]
pub async fn natural_language_search(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<SearchResult>> {
    let settings = get_ai_settings_internal(&state.db).await.map_err(|e| e.to_string())?;
    let local_service = state.chinese_clip.as_ref();
    
    if local_service.is_none() && settings.api_key.is_empty() {
        return Err("未配置嵌入服务: 请加载本地 Chinese-CLIP 模型或配置 API Key".into());
    }

    let query_embedding = crate::ai::embed_text(&settings.api_key, &query, local_service.cloned()).await.map_err(|e| e.to_string())?;

    let index = state.vector_index.read().await;
    let results = index.search(&query_embedding, 50, 0.2);
    drop(index);

    let mut search_results = Vec::new();
    for (photo_id, similarity) in results {
        let photo = sqlx::query_as::<_, Photo>("SELECT * FROM photos WHERE id = ?1")
            .bind(&photo_id)
            .fetch_optional(&state.db)
            .await.map_err(|e| e.to_string())?;

        if let Some(photo) = photo {
            search_results.push(SearchResult { photo, similarity });
        }
    }

    Ok(search_results)
}

// ==================== Export ====================

#[tauri::command]
pub async fn export_photos(
    _state: State<'_, AppState>,
    photo_ids: Vec<String>,
    destination: String,
) -> Result<()> {
    let dest = PathBuf::from(&destination);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

    for id in photo_ids {
        // In a real implementation, we'd copy the files here
        tracing::info!("Export photo {} to {:?}", id, dest);
    }

    Ok(())
}

// ==================== AI Settings ====================

#[tauri::command]
pub async fn get_ai_settings(state: State<'_, AppState>) -> Result<Option<AiSettings>> {
    let settings = get_ai_settings_internal(&state.db).await.map_err(|e| e.to_string())?;
    Ok(Some(settings))
}

#[tauri::command]
pub async fn update_ai_settings(
    state: State<'_, AppState>,
    settings: serde_json::Value,
) -> Result<AiSettings> {
    let provider = settings["provider"].as_str().unwrap_or("gemini");
    let api_key = settings["api_key"].as_str().unwrap_or("");
    let ollama_base_url = settings["ollama_base_url"].as_str().unwrap_or("http://localhost:11434");
    let ollama_embed_model = settings["ollama_embed_model"].as_str().unwrap_or("nomic-embed-text");
    let ollama_vision_model = settings["ollama_vision_model"].as_str().unwrap_or("llava");

    sqlx::query(
        r#"UPDATE ai_settings SET
           provider = ?1, api_key = ?2, ollama_base_url = ?3,
           ollama_embed_model = ?4, ollama_vision_model = ?5
           WHERE id = 'default'"#,
    )
    .bind(provider)
    .bind(api_key)
    .bind(ollama_base_url)
    .bind(ollama_embed_model)
    .bind(ollama_vision_model)
    .execute(&state.db)
    .await.map_err(|e| e.to_string())?;

    get_ai_settings_internal(&state.db).await.map(Some).map(|s| s.unwrap())
}

#[derive(serde::Serialize)]
pub struct ValidateKeyResult {
    valid: bool,
    message: String,
}

#[tauri::command]
pub async fn check_local_model(state: State<'_, AppState>) -> Result<bool> {
    Ok(state.chinese_clip.is_some())
}

#[tauri::command]
pub async fn validate_api_key(api_key: String) -> Result<ValidateKeyResult> {
    let (valid, msg) = crate::ai::validate_api_key(&api_key).await.map_err(|e| e.to_string())?;
    Ok(ValidateKeyResult { valid, message: msg })
}

// ==================== Helpers ====================

async fn rescan_directory(state: &AppState, path: &str, dir_id: &str) -> Result<()> {
    tracing::info!("Scanning directory: {}", path);
    let photos = crate::fs::scan_directory(path, dir_id).await.map_err(|e| e.to_string())?;
    tracing::info!("Found {} photos", photos.len());
    for photo in &photos {
        match insert_photo(&state.db, photo).await {
            Ok(_) => tracing::info!("Inserted photo {}", photo.file_name),
            Err(e) => tracing::error!("Failed to insert photo {}: {}", photo.file_name, e),
        }
    }

    for photo in photos {
        if let Ok(thumb_path) = crate::fs::generate_thumbnail(&photo.file_path, &state.thumbnail_dir, 400).await {
            let _ = sqlx::query("UPDATE photos SET thumbnail_path = ?1 WHERE id = ?2")
                .bind(&thumb_path)
                .bind(&photo.id)
                .execute(&state.db)
                .await;
        }
    }
    Ok(())
}

async fn insert_photo(db: &sqlx::Pool<sqlx::Sqlite>, photo: &Photo) -> Result<()> {
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
    .execute(db)
    .await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn get_ai_settings_internal(db: &sqlx::Pool<sqlx::Sqlite>) -> Result<AiSettings> {
    let settings = sqlx::query_as::<_, AiSettings>("SELECT * FROM ai_settings WHERE id = 'default'")
        .fetch_one(db)
        .await.map_err(|e| e.to_string())?;
    Ok(settings)
}

async fn start_monitoring(
    state: &AppState,
    path: &str,
    dir_id: &str,
) -> Result<()> {
    use notify::{RecursiveMode, Watcher};

    let path = path.to_string();
    let dir_id = dir_id.to_string();
    let mut watcher = notify::recommended_watcher(move |res| {
        match res {
            Ok(event) => {
                tracing::debug!("FS event: {:?}", event);
                // TODO: handle incremental updates instead of full rescan
            }
            Err(e) => tracing::error!("Watch error: {:?}", e),
        }
    }).map_err(|e| e.to_string())?;

    watcher.watch(std::path::Path::new(&path), RecursiveMode::Recursive).map_err(|e| e.to_string())?;

    let mut monitors = state.monitors.lock().await;
    monitors.insert(dir_id, (path, watcher));

    Ok(())
}

fn create_bookmark(_path: &str) -> Result<Vec<u8>> {
    // TODO: Implement security-scoped bookmark for macOS App Store
    Ok(vec![])
}
