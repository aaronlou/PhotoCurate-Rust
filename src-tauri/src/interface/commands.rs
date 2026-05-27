use crate::application;
use crate::application::ports::VectorIndexStore;
use crate::domain::models::{AiSettings, Directory, ExportResult, Photo, SearchResult};
use crate::error::PhotoCurateError;
use crate::AppState;
use tauri::State;

fn map_err<T>(result: Result<T, PhotoCurateError>) -> Result<T, String> {
    result.map_err(|e| e.to_string())
}

// ==================== Directory Commands ====================

#[tauri::command]
pub async fn pick_directory() -> Result<Option<String>, String> {
    Ok(None)
}

#[tauri::command]
pub async fn add_directory(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    path: String,
) -> Result<Directory, String> {
    let result = map_err(
        application::directory::add_directory(
            &state.db,
            &state.thumbnail_dir,
            &state.monitors,
            path,
        )
        .await,
    )?;
    application::bootstrap::start_background_indexing(&app_handle);
    Ok(result)
}

#[tauri::command]
pub async fn get_directories(state: State<'_, AppState>) -> Result<Vec<Directory>, String> {
    map_err(application::directory::get_directories(&state.db).await)
}

#[tauri::command]
pub async fn remove_directory(state: State<'_, AppState>, id: String) -> Result<(), String> {
    map_err(application::directory::remove_directory(&state.db, &state.monitors, id).await)
}

// ==================== Photo Commands ====================

#[tauri::command]
pub async fn get_photos(
    state: State<'_, AppState>,
    sort_order: Option<crate::domain::models::PhotoSortOrder>,
) -> Result<Vec<Photo>, String> {
    map_err(application::photo::get_photos(&state.db, sort_order).await)
}

#[tauri::command]
pub async fn get_photo_by_id(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<Photo>, String> {
    map_err(application::photo::get_photo_by_id(&state.db, id).await)
}

#[tauri::command]
pub async fn get_thumbnail_path(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<Option<String>, String> {
    map_err(application::photo::get_thumbnail_path(&state.db, &state.thumbnail_dir, photo_id).await)
}

// ==================== Scanning ====================

#[tauri::command]
pub async fn start_scanning(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    directory_id: String,
) -> Result<(), String> {
    map_err(
        application::photo::scan_directory(&state.db, &state.thumbnail_dir, &directory_id).await,
    )?;
    application::bootstrap::start_background_indexing(&app_handle);
    Ok(())
}

// ==================== Scoring ====================

#[tauri::command]
pub async fn score_photos(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    photo_ids: Vec<String>,
    allow_keychain_read: Option<bool>,
) -> Result<(), String> {
    map_err(
        application::scoring::score_photos(
            &state.db,
            photo_ids,
            &app_handle,
            allow_keychain_read.unwrap_or(false),
        )
        .await,
    )
}

// ==================== Search Index ====================

#[tauri::command]
pub async fn build_search_index(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
    allow_keychain_read: Option<bool>,
) -> Result<(), String> {
    map_err(
        application::search::build_index(
            &state.db,
            &state.vector_index,
            &state.chinese_clip,
            photo_ids,
            allow_keychain_read.unwrap_or(false),
        )
        .await,
    )
}

// ==================== Search ====================

#[tauri::command]
pub async fn natural_language_search(
    state: State<'_, AppState>,
    query: String,
    allow_keychain_read: Option<bool>,
) -> Result<Vec<SearchResult>, String> {
    map_err(
        application::search::natural_language_search(
            &state.db,
            &state.vector_index,
            &state.chinese_clip,
            query,
            allow_keychain_read.unwrap_or(false),
        )
        .await,
    )
}

// ==================== Export ====================

#[tauri::command]
pub async fn export_photos(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
    destination: String,
    preserve_structure: Option<bool>,
) -> Result<ExportResult, String> {
    map_err(
        application::export::export_photos(&state.db, photo_ids, destination, preserve_structure)
            .await,
    )
}

// ==================== AI Settings ====================

#[tauri::command]
pub async fn get_ai_settings(state: State<'_, AppState>) -> Result<Option<AiSettings>, String> {
    map_err(
        application::scoring::get_ai_settings(&state.db)
            .await
            .map(Some),
    )
}

#[tauri::command]
pub async fn update_ai_settings(
    state: State<'_, AppState>,
    settings: serde_json::Value,
) -> Result<AiSettings, String> {
    map_err(application::scoring::update_ai_settings(&state.db, settings).await)
}

#[tauri::command]
pub async fn check_local_model(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.chinese_clip.is_some())
}

#[tauri::command]
pub async fn get_index_stats(state: State<'_, AppState>) -> Result<(usize, Option<usize>), String> {
    Ok(state.vector_index.stats().await)
}

#[tauri::command]
pub async fn rebuild_all_index(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    allow_keychain_read: Option<bool>,
) -> Result<usize, String> {
    map_err(
        application::search::rebuild_all_index(
            &state.db,
            &state.vector_index,
            &state.chinese_clip,
            &app_handle,
            allow_keychain_read.unwrap_or(false),
        )
        .await,
    )
}

#[tauri::command]
pub async fn validate_api_key(
    settings: serde_json::Value,
) -> Result<application::scoring::ValidateKeyResult, String> {
    map_err(application::scoring::validate_api_key(settings).await)
}
