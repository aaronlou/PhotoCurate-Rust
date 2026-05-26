use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

pub mod error;
pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod interface;

pub type Result<T> = error::Result<T>;

pub struct AppState {
    pub db: Pool<Sqlite>,
    pub thumbnail_dir: PathBuf,
    pub vector_index: infrastructure::vector::SharedVectorIndex,
    pub monitors: Arc<Mutex<HashMap<String, (String, notify::RecommendedWatcher)>>>,
    pub chinese_clip: Option<Arc<infrastructure::ai::ChineseClipService>>,
    pub is_indexing: Arc<AtomicBool>,
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
            interface::commands::pick_directory,
            interface::commands::add_directory,
            interface::commands::get_directories,
            interface::commands::remove_directory,
            interface::commands::get_photos,
            interface::commands::get_photo_by_id,
            interface::commands::get_thumbnail_path,
            interface::commands::start_scanning,
            interface::commands::score_photos,
            interface::commands::build_search_index,
            interface::commands::natural_language_search,
            interface::commands::export_photos,
            interface::commands::get_ai_settings,
            interface::commands::update_ai_settings,
            interface::commands::check_local_model,
            interface::commands::validate_api_key,
            interface::commands::get_index_stats,
            interface::commands::rebuild_all_index,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub fn start_background_indexing(app_handle: &tauri::AppHandle) {
    let state = app_handle.state::<AppState>();

    if state
        .is_indexing
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::debug!("Indexing already in progress, skipping");
        return;
    }

    let db = state.db.clone();
    let vi = state.vector_index.clone();
    let cc = state.chinese_clip.clone();
    let handle = app_handle.clone();
    let is_indexing = state.is_indexing.clone();

    tokio::spawn(async move {
        tracing::info!("Starting background indexing...");
        match application::search::auto_index_unindexed(&db, &vi, &cc, &handle).await {
            Ok(()) => tracing::info!("Background indexing completed"),
            Err(e) => tracing::error!("Background indexing failed: {}", e),
        }
        is_indexing.store(false, Ordering::SeqCst);
    });
}

async fn setup_app(app: tauri::AppHandle) -> anyhow::Result<()> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    let thumbnail_dir = app_data_dir.join("thumbnails");
    std::fs::create_dir_all(&thumbnail_dir)?;

    let db = infrastructure::db::init_db(&app_data_dir).await?;

    // Resolve security-scoped bookmarks to regain sandbox access
    // to previously-added photo directories (macOS App Sandbox)
    if let Err(e) = application::directory::resolve_bookmarks_on_startup(&db).await {
        tracing::warn!("Failed to resolve some directory bookmarks: {}", e);
    }

    let vector_index = infrastructure::vector::create_index();

    let vectors = infrastructure::repositories::SqliteVectorRepository::new(db.clone())
        .find_all()
        .await?;

    {
        let mut index = vector_index.write().await;
        for (photo_id, vector_json) in vectors {
            if let Ok(vec) = serde_json::from_str::<Vec<f64>>(&vector_json) {
                index.add(photo_id, vec);
            }
        }
    }

    let models_dir = app.path().app_data_dir()?.join("models");
    let chinese_clip = if models_dir.join("chinese_clip_image.onnx").exists() {
        match infrastructure::ai::ChineseClipService::new(&models_dir) {
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
        is_indexing: Arc::new(AtomicBool::new(false)),
    };

    app.manage(state);

    start_background_indexing(&app);

    Ok(())
}
