use crate::application;
use crate::application::ports::VectorIndexStore;
use crate::infrastructure;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

pub type Monitors = Arc<Mutex<HashMap<String, (String, notify::RecommendedWatcher)>>>;

pub struct AppState {
    pub db: Pool<Sqlite>,
    pub thumbnail_dir: PathBuf,
    pub vector_index: infrastructure::vector::SharedVectorIndex,
    pub monitors: Monitors,
    pub chinese_clip: Option<Arc<infrastructure::ai::ChineseClipService>>,
    pub is_indexing: Arc<AtomicBool>,
}

pub async fn setup_app(app: tauri::AppHandle) -> anyhow::Result<()> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    let thumbnail_dir = app_data_dir.join("thumbnails");
    std::fs::create_dir_all(&thumbnail_dir)?;

    let db = infrastructure::db::init_db(&app_data_dir).await?;

    // Resolve security-scoped bookmarks before accessing saved directories.
    if let Err(e) = application::directory::resolve_bookmarks_on_startup(&db).await {
        tracing::warn!("Failed to resolve some directory bookmarks: {}", e);
    }

    let vector_index = infrastructure::vector::create_index();
    load_vectors(&db, &vector_index).await?;

    let chinese_clip = load_local_model(&app)?;

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
    let vector_index = state.vector_index.clone();
    let chinese_clip = state.chinese_clip.clone();
    let handle = app_handle.clone();
    let is_indexing = state.is_indexing.clone();

    tokio::spawn(async move {
        tracing::info!("Starting background indexing...");
        match application::search::auto_index_unindexed(&db, &vector_index, &chinese_clip, &handle)
            .await
        {
            Ok(()) => tracing::info!("Background indexing completed"),
            Err(e) => tracing::error!("Background indexing failed: {}", e),
        }
        is_indexing.store(false, Ordering::SeqCst);
    });
}

async fn load_vectors(
    db: &Pool<Sqlite>,
    vector_index: &infrastructure::vector::SharedVectorIndex,
) -> anyhow::Result<()> {
    let vectors = infrastructure::repositories::SqliteVectorRepository::new(db.clone())
        .find_all()
        .await?;

    for (photo_id, vector_json) in vectors {
        if let Ok(vector) = serde_json::from_str::<Vec<f64>>(&vector_json) {
            vector_index.add(photo_id, vector).await;
        }
    }

    Ok(())
}

fn load_local_model(
    app: &tauri::AppHandle,
) -> anyhow::Result<Option<Arc<infrastructure::ai::ChineseClipService>>> {
    let models_dir = app.path().app_data_dir()?.join("models");
    if !models_dir.join("chinese_clip_image.onnx").exists() {
        return Ok(None);
    }

    match infrastructure::ai::ChineseClipService::new(&models_dir) {
        Ok(service) => {
            tracing::info!("Chinese-CLIP local model loaded successfully");
            Ok(Some(Arc::new(service)))
        }
        Err(e) => {
            tracing::warn!("Failed to load Chinese-CLIP model: {}", e);
            Ok(None)
        }
    }
}
