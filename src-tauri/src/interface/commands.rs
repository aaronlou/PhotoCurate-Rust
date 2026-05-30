use crate::application;
use crate::application::ports::VectorIndexStore;
use crate::domain::models::{
    AiSettings, BillingActionResult, BillingStatus, Directory, ExportResult, LibraryInsights,
    Photo, ScoringRunResult, SearchResult,
};
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
pub async fn add_directory(state: State<'_, AppState>, path: String) -> Result<Directory, String> {
    map_err(
        application::directory::add_directory(
            &state.db,
            &state.thumbnail_dir,
            &state.monitors,
            path,
        )
        .await,
    )
}

#[tauri::command]
pub async fn get_directories(state: State<'_, AppState>) -> Result<Vec<Directory>, String> {
    map_err(application::directory::get_directories(&state.db).await)
}

#[tauri::command]
pub async fn remove_directory(state: State<'_, AppState>, id: String) -> Result<(), String> {
    map_err(application::directory::remove_directory(&state.db, &state.monitors, id).await)?;

    if let Err(e) = application::search::reload_vector_index(&state.db, &state.vector_index).await {
        tracing::warn!(
            "Failed to reload vector index after directory removal: {}",
            e
        );
    }

    Ok(())
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
    directory_id: String,
) -> Result<(), String> {
    map_err(
        application::photo::scan_directory(&state.db, &state.thumbnail_dir, &directory_id).await,
    )
}

// ==================== Scoring ====================

#[tauri::command]
pub async fn score_photos(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    photo_ids: Vec<String>,
    allow_keychain_read: Option<bool>,
) -> Result<ScoringRunResult, String> {
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
    app_handle: tauri::AppHandle,
    photo_ids: Vec<String>,
    allow_keychain_read: Option<bool>,
) -> Result<usize, String> {
    if !state.indexing.start_manual_run().await {
        return Err("已有索引任务正在运行".into());
    }

    let result = application::search::build_index(
        &state.db,
        &state.vector_index,
        &state.chinese_clip,
        &app_handle,
        &state.indexing,
        photo_ids,
        allow_keychain_read.unwrap_or(false),
    )
    .await;
    state.indexing.finish_manual_run().await;

    map_err(result)
}

#[tauri::command]
pub async fn cancel_search_indexing(state: State<'_, AppState>) -> Result<(), String> {
    state.indexing.request_cancel().await;
    Ok(())
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

// ==================== Insights ====================

#[tauri::command]
pub async fn get_library_insights(state: State<'_, AppState>) -> Result<LibraryInsights, String> {
    map_err(application::insights::get_library_insights(&state.db).await)
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
    if !state.indexing.start_manual_run().await {
        return Err("已有索引任务正在运行".into());
    }

    let result = application::search::rebuild_all_index(
        &state.db,
        &state.vector_index,
        &state.chinese_clip,
        &app_handle,
        &state.indexing,
        allow_keychain_read.unwrap_or(false),
    )
    .await;
    state.indexing.finish_manual_run().await;

    map_err(result)
}

#[tauri::command]
pub async fn validate_api_key(
    settings: serde_json::Value,
) -> Result<application::scoring::ValidateKeyResult, String> {
    map_err(application::scoring::validate_api_key(settings).await)
}

// ==================== Billing ====================

#[tauri::command]
pub async fn get_billing_status(state: State<'_, AppState>) -> Result<BillingStatus, String> {
    map_err(application::billing::get_billing_status(&state.db).await)
}

#[tauri::command]
pub async fn start_managed_ai_checkout(
    state: State<'_, AppState>,
    plan_id: String,
    interval: String,
) -> Result<BillingActionResult, String> {
    map_err(application::billing::start_managed_ai_checkout(&state.db, plan_id, interval).await)
}

#[tauri::command]
pub async fn restore_managed_ai_purchases(
    state: State<'_, AppState>,
) -> Result<BillingActionResult, String> {
    map_err(application::billing::restore_managed_ai_purchases(&state.db).await)
}

#[tauri::command]
pub async fn open_billing_portal(
    state: State<'_, AppState>,
) -> Result<BillingActionResult, String> {
    map_err(application::billing::open_billing_portal(&state.db).await)
}
