use sqlx::Pool;
use sqlx::Sqlite;
use tauri::Manager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod ai;
pub mod commands;
pub mod db;
pub mod fs;
pub mod models;
pub mod vector;

pub type Result<T> = std::result::Result<T, String>;

pub struct AppState {
    pub db: Pool<Sqlite>,
    pub thumbnail_dir: PathBuf,
    pub vector_index: vector::SharedVectorIndex,
    pub monitors: Arc<Mutex<HashMap<String, (String, notify::RecommendedWatcher)>>>,
    pub chinese_clip: Option<Arc<ai::ChineseClipService>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = setup_app(handle).await {
                    tracing::error!("Failed to setup app: {}", e);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pick_directory,
            commands::add_directory,
            commands::get_directories,
            commands::remove_directory,
            commands::get_photos,
            commands::get_photo_by_id,
            commands::get_thumbnail_path,
            commands::start_scanning,
            commands::score_photos,
            commands::build_search_index,
            commands::natural_language_search,
            commands::export_photos,
            commands::get_ai_settings,
            commands::update_ai_settings,
            commands::check_local_model,
            commands::validate_api_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn setup_app(app: tauri::AppHandle) -> anyhow::Result<()> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    let thumbnail_dir = app_data_dir.join("thumbnails");
    std::fs::create_dir_all(&thumbnail_dir)?;

    let db = db::init_db(&app_data_dir).await?;

    let vector_index = vector::create_index();

    // Load existing vectors into memory index
    let vectors: Vec<(String, String)> =
        sqlx::query_as("SELECT photo_id, vector FROM vector_entries")
            .fetch_all(&db)
            .await?;

    {
        let mut index = vector_index.write().await;
        for (photo_id, vector_json) in vectors {
            if let Ok(vec) = serde_json::from_str::<Vec<f64>>(&vector_json) {
                index.add(photo_id, vec);
            }
        }
    }

    // Try to load Chinese-CLIP local model
    let models_dir = app.path().app_data_dir()?.join("models");
    let chinese_clip = if models_dir.join("chinese_clip_image.onnx").exists() {
        match ai::ChineseClipService::new(&models_dir) {
            Ok(service) => {
                tracing::info!("Chinese-CLIP local model loaded successfully");
                Some(Arc::new(service))
            }
            Err(e) => {
                tracing::warn!("Failed to load Chinese-CLIP model: {}", e);
                None
            }
        }
    } else {
        None
    };

    let state = AppState {
        db,
        thumbnail_dir,
        vector_index,
        monitors: Arc::new(Mutex::new(HashMap::new())),
        chinese_clip,
    };

    app.manage(state);

    Ok(())
}
