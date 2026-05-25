use photo_curate_lib::application::export::export_photos;
use photo_curate_lib::infrastructure::db;
use photo_curate_lib::infrastructure::vector;
use photo_curate_lib::AppState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn create_test_state() -> (AppState, tempfile::TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let app_data_dir = temp_dir.path().to_path_buf();
    let thumbnail_dir = app_data_dir.join("thumbnails");
    std::fs::create_dir_all(&thumbnail_dir).unwrap();

    let db = db::init_db(&app_data_dir).await.unwrap();
    let vector_index = vector::create_index();
    let monitors = Arc::new(Mutex::new(HashMap::new()));
    let chinese_clip = None;

    let state = AppState {
        db,
        thumbnail_dir,
        vector_index,
        monitors,
        chinese_clip,
    };

    (state, temp_dir)
}

#[tokio::test]
async fn test_export_photos_flat() {
    let (state, _temp) = create_test_state().await;

    // Create test source directory with files
    let src_dir = _temp.path().join("source");
    std::fs::create_dir_all(&src_dir).unwrap();
    let file1 = src_dir.join("photo1.jpg");
    let file2 = src_dir.join("photo2.jpg");
    std::fs::write(&file1, b"fake image 1").unwrap();
    std::fs::write(&file2, b"fake image 2").unwrap();

    // Insert directory record
    let dir_id = "dir-001".to_string();
    sqlx::query(
        "INSERT INTO directories (id, path, is_monitoring, date_added) VALUES (?1, ?2, 1, datetime('now'))"
    )
    .bind(&dir_id)
    .bind(src_dir.to_str().unwrap())
    .execute(&state.db)
    .await
    .unwrap();

    // Insert photo records
    let photo1_id = "photo-001".to_string();
    let photo2_id = "photo-002".to_string();
    sqlx::query(
        "INSERT INTO photos (id, file_path, file_name, file_size, date_modified, directory_id) VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)"
    )
    .bind(&photo1_id)
    .bind(file1.to_str().unwrap())
    .bind("photo1.jpg")
    .bind(12i64)
    .bind(&dir_id)
    .execute(&state.db)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO photos (id, file_path, file_name, file_size, date_modified, directory_id) VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)"
    )
    .bind(&photo2_id)
    .bind(file2.to_str().unwrap())
    .bind("photo2.jpg")
    .bind(12i64)
    .bind(&dir_id)
    .execute(&state.db)
    .await
    .unwrap();

    // Export to destination
    let dest_dir = _temp.path().join("export").to_str().unwrap().to_string();
    let result = export_photos(
        &state.db,
        vec![photo1_id.clone(), photo2_id.clone()],
        dest_dir.clone(),
        Some(false), // flat export
    )
    .await
    .unwrap();

    // Assertions
    assert_eq!(result.exported_count, 2);
    assert_eq!(result.failed_count, 0);
    assert!(result.failed_photos.is_empty());

    // Verify files exist in destination
    assert!(PathBuf::from(&dest_dir).join("photo1.jpg").exists());
    assert!(PathBuf::from(&dest_dir).join("photo2.jpg").exists());

    // Verify export status in DB
    let photo1: (bool, Option<String>) = sqlx::query_as(
        "SELECT has_been_exported, export_date FROM photos WHERE id = ?1"
    )
    .bind(&photo1_id)
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert!(photo1.0);
    assert!(photo1.1.is_some());
}

#[tokio::test]
async fn test_export_photos_with_conflict() {
    let (state, _temp) = create_test_state().await;

    let src_dir = _temp.path().join("source");
    std::fs::create_dir_all(&src_dir).unwrap();
    let file1 = src_dir.join("photo.jpg");
    std::fs::write(&file1, b"fake image 1").unwrap();

    let dir_id = "dir-002".to_string();
    sqlx::query(
        "INSERT INTO directories (id, path, is_monitoring, date_added) VALUES (?1, ?2, 1, datetime('now'))"
    )
    .bind(&dir_id)
    .bind(src_dir.to_str().unwrap())
    .execute(&state.db)
    .await
    .unwrap();

    let photo1_id = "photo-003".to_string();
    let photo2_id = "photo-004".to_string();
    sqlx::query(
        "INSERT INTO photos (id, file_path, file_name, file_size, date_modified, directory_id) VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)"
    )
    .bind(&photo1_id)
    .bind(file1.to_str().unwrap())
    .bind("photo.jpg")
    .bind(12i64)
    .bind(&dir_id)
    .execute(&state.db)
    .await
    .unwrap();

    // Second photo with same file name (different path, same filename)
    let file2 = src_dir.join("subdir").join("photo.jpg");
    std::fs::create_dir_all(file2.parent().unwrap()).unwrap();
    std::fs::write(&file2, b"fake image 2").unwrap();

    sqlx::query(
        "INSERT INTO photos (id, file_path, file_name, file_size, date_modified, directory_id) VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)"
    )
    .bind(&photo2_id)
    .bind(file2.to_str().unwrap())
    .bind("photo.jpg")
    .bind(12i64)
    .bind(&dir_id)
    .execute(&state.db)
    .await
    .unwrap();

    // Export flat — should auto-rename second file
    let dest_dir = _temp.path().join("export_conflict").to_str().unwrap().to_string();
    let result = export_photos(
        &state.db,
        vec![photo1_id, photo2_id],
        dest_dir.clone(),
        Some(false),
    )
    .await
    .unwrap();

    assert_eq!(result.exported_count, 2);
    assert_eq!(result.failed_count, 0);

    let dest_path = PathBuf::from(&dest_dir);
    assert!(dest_path.join("photo.jpg").exists());
    assert!(dest_path.join("photo (1).jpg").exists());
}

#[tokio::test]
async fn test_export_photos_missing_file() {
    let (state, _temp) = create_test_state().await;

    let dir_id = "dir-003".to_string();
    sqlx::query(
        "INSERT INTO directories (id, path, is_monitoring, date_added) VALUES (?1, ?2, 1, datetime('now'))"
    )
    .bind(&dir_id)
    .bind("/nonexistent/path")
    .execute(&state.db)
    .await
    .unwrap();

    let photo_id = "photo-005".to_string();
    sqlx::query(
        "INSERT INTO photos (id, file_path, file_name, file_size, date_modified, directory_id) VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)"
    )
    .bind(&photo_id)
    .bind("/nonexistent/path/deleted.jpg")
    .bind("deleted.jpg")
    .bind(12i64)
    .bind(&dir_id)
    .execute(&state.db)
    .await
    .unwrap();

    let dest_dir = _temp.path().join("export_missing").to_str().unwrap().to_string();
    let result = export_photos(
        &state.db,
        vec![photo_id],
        dest_dir,
        Some(false),
    )
    .await
    .unwrap();

    assert_eq!(result.exported_count, 0);
    assert_eq!(result.failed_count, 1);
    assert_eq!(result.failed_photos[0].error, "源文件不存在");
}

#[tokio::test]
async fn test_export_photos_preserve_structure() {
    let (state, _temp) = create_test_state().await;

    let src_dir = _temp.path().join("source");
    let sub_dir = src_dir.join("2025").join("May");
    std::fs::create_dir_all(&sub_dir).unwrap();
    let file1 = sub_dir.join("trip.jpg");
    std::fs::write(&file1, b"fake image 1").unwrap();

    let dir_id = "dir-004".to_string();
    sqlx::query(
        "INSERT INTO directories (id, path, is_monitoring, date_added) VALUES (?1, ?2, 1, datetime('now'))"
    )
    .bind(&dir_id)
    .bind(src_dir.to_str().unwrap())
    .execute(&state.db)
    .await
    .unwrap();

    let photo_id = "photo-006".to_string();
    sqlx::query(
        "INSERT INTO photos (id, file_path, file_name, file_size, date_modified, directory_id) VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)"
    )
    .bind(&photo_id)
    .bind(file1.to_str().unwrap())
    .bind("trip.jpg")
    .bind(12i64)
    .bind(&dir_id)
    .execute(&state.db)
    .await
    .unwrap();

    let dest_dir = _temp.path().join("export_struct").to_str().unwrap().to_string();
    let result = export_photos(
        &state.db,
        vec![photo_id],
        dest_dir.clone(),
        Some(true), // preserve structure
    )
    .await
    .unwrap();

    assert_eq!(result.exported_count, 1);
    assert_eq!(result.failed_count, 0);

    let expected_path = PathBuf::from(&dest_dir).join("2025").join("May").join("trip.jpg");
    assert!(expected_path.exists(), "Expected file at {:?}", expected_path);
}
