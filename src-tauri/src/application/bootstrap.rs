use crate::application;
use crate::application::ports::VectorIndexStore;
use crate::infrastructure;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::path::PathBuf;
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
    pub indexing: Arc<IndexingCoordinator>,
}

#[derive(Debug, Default)]
struct IndexingState {
    running: bool,
    cancel_requested: bool,
}

#[derive(Debug, Default)]
pub struct IndexingCoordinator {
    state: Mutex<IndexingState>,
}

impl IndexingCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn start_manual_run(&self) -> bool {
        let mut state = self.state.lock().await;
        if state.running {
            return false;
        }

        state.running = true;
        state.cancel_requested = false;
        true
    }

    pub async fn request_cancel(&self) {
        let mut state = self.state.lock().await;
        if state.running {
            state.cancel_requested = true;
        }
    }

    pub async fn is_cancelled(&self) -> bool {
        self.state.lock().await.cancel_requested
    }

    pub async fn finish_manual_run(&self) {
        let mut state = self.state.lock().await;
        state.running = false;
        state.cancel_requested = false;
    }
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
        indexing: Arc::new(IndexingCoordinator::new()),
    };

    app.manage(state);

    Ok(())
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
