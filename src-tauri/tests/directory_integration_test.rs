use photo_curate_lib::infrastructure::db;

#[tokio::test]
async fn deleting_directory_cascades_photos_evaluations_and_vectors() {
    let temp_dir = tempfile::tempdir().unwrap();
    let pool = db::init_db(temp_dir.path()).await.unwrap();

    sqlx::query(
        "INSERT INTO directories (id, path, is_monitoring, date_added) VALUES (?1, ?2, 1, datetime('now'))",
    )
    .bind("dir-001")
    .bind(temp_dir.path().join("source").to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO photos (id, file_path, file_name, file_size, date_modified, directory_id) VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)",
    )
    .bind("photo-001")
    .bind(temp_dir.path().join("source/photo.jpg").to_string_lossy().to_string())
    .bind("photo.jpg")
    .bind(12i64)
    .bind("dir-001")
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO photo_evaluations (
            id, photo_id, overall_score, summary, strengths_json, weaknesses_json,
            suggestions_json, dimension_scores_json, tags_json, model_provider,
            model_name, prompt_version, raw_response, created_at
        ) VALUES (?1, ?2, 88, ?3, '[]', '[]', '[]', '[]', '[]', ?4, ?5, ?6, ?7, datetime('now'))"#,
    )
    .bind("eval-001")
    .bind("photo-001")
    .bind("summary")
    .bind("gemini")
    .bind("gemini-test")
    .bind("photo-evaluation-v1")
    .bind("{}")
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO vector_entries (id, photo_id, vector, created_at) VALUES (?1, ?2, ?3, datetime('now'))",
    )
    .bind("vector-001")
    .bind("photo-001")
    .bind("[0.1,0.2]")
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("DELETE FROM directories WHERE id = ?1")
        .bind("dir-001")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(count_rows(&pool, "directories").await, 0);
    assert_eq!(count_rows(&pool, "photos").await, 0);
    assert_eq!(count_rows(&pool, "photo_evaluations").await, 0);
    assert_eq!(count_rows(&pool, "vector_entries").await, 0);
}

async fn count_rows(pool: &sqlx::Pool<sqlx::Sqlite>, table: &str) -> i64 {
    let query = format!("SELECT COUNT(*) FROM {table}");
    let (count,): (i64,) = sqlx::query_as(&query).fetch_one(pool).await.unwrap();
    count
}
