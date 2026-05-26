pub mod application;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod interface;

pub type Result<T> = error::Result<T>;
pub use application::bootstrap::{AppState, Monitors};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = application::bootstrap::setup_app(handle).await {
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
