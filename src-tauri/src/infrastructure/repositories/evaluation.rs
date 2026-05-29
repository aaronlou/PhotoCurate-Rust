use crate::application::ports::PhotoEvaluationRepository;
use crate::domain::models::{DimensionScore, Photo, PhotoEvaluation};
use crate::error::{PhotoCurateError, Result};
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;

#[derive(sqlx::FromRow)]
struct PhotoEvaluationRow {
    id: String,
    photo_id: String,
    overall_score: f64,
    summary: String,
    strengths_json: String,
    weaknesses_json: String,
    suggestions_json: String,
    dimension_scores_json: String,
    tags_json: String,
    model_provider: String,
    model_name: String,
    prompt_version: String,
    raw_response: Option<String>,
    created_at: DateTime<Utc>,
}

impl TryFrom<PhotoEvaluationRow> for PhotoEvaluation {
    type Error = PhotoCurateError;

    fn try_from(row: PhotoEvaluationRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            photo_id: row.photo_id,
            overall_score: row.overall_score,
            summary: row.summary,
            strengths: parse_json_list(&row.strengths_json)?,
            weaknesses: parse_json_list(&row.weaknesses_json)?,
            suggestions: parse_json_list(&row.suggestions_json)?,
            dimension_scores: parse_dimension_scores(&row.dimension_scores_json)?,
            tags: parse_json_list(&row.tags_json)?,
            model_provider: row.model_provider,
            model_name: row.model_name,
            prompt_version: row.prompt_version,
            raw_response: row.raw_response,
            created_at: row.created_at,
        })
    }
}

pub(super) async fn attach_latest_evaluations(
    db: &Pool<Sqlite>,
    photos: &mut [Photo],
) -> Result<()> {
    if photos.is_empty() {
        return Ok(());
    }

    let placeholders: Vec<String> = (1..=photos.len()).map(|i| format!("?{}", i)).collect();
    let query = format!(
        r#"
        SELECT e.*
        FROM photo_evaluations e
        JOIN (
            SELECT photo_id, MAX(created_at) AS max_created_at
            FROM photo_evaluations
            WHERE photo_id IN ({})
            GROUP BY photo_id
        ) latest
          ON e.photo_id = latest.photo_id
         AND e.created_at = latest.max_created_at
        "#,
        placeholders.join(", ")
    );

    let mut q = sqlx::query_as::<_, PhotoEvaluationRow>(&query);
    for photo in photos.iter() {
        q = q.bind(&photo.id);
    }

    let rows = q.fetch_all(db).await?;
    let mut evaluations = HashMap::new();
    for row in rows {
        let evaluation = PhotoEvaluation::try_from(row)?;
        evaluations.insert(evaluation.photo_id.clone(), evaluation);
    }

    for photo in photos {
        photo.latest_evaluation = evaluations.remove(&photo.id);
    }

    Ok(())
}

fn parse_json_list(json: &str) -> Result<Vec<String>> {
    serde_json::from_str(json).map_err(|e| PhotoCurateError::InvalidData(e.to_string()))
}

fn parse_dimension_scores(json: &str) -> Result<Vec<DimensionScore>> {
    serde_json::from_str(json).map_err(|e| PhotoCurateError::InvalidData(e.to_string()))
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|e| PhotoCurateError::InvalidData(e.to_string()))
}

pub struct SqlitePhotoEvaluationRepository {
    db: Pool<Sqlite>,
}

impl SqlitePhotoEvaluationRepository {
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self { db }
    }
}

impl PhotoEvaluationRepository for SqlitePhotoEvaluationRepository {
    async fn save(&self, evaluation: &PhotoEvaluation) -> Result<()> {
        let mut tx = self.db.begin().await?;

        sqlx::query(
            r#"INSERT INTO photo_evaluations (
                id, photo_id, overall_score, summary, strengths_json, weaknesses_json,
                suggestions_json, dimension_scores_json, tags_json, model_provider,
                model_name, prompt_version, raw_response, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
        )
        .bind(&evaluation.id)
        .bind(&evaluation.photo_id)
        .bind(evaluation.overall_score)
        .bind(&evaluation.summary)
        .bind(to_json(&evaluation.strengths)?)
        .bind(to_json(&evaluation.weaknesses)?)
        .bind(to_json(&evaluation.suggestions)?)
        .bind(to_json(&evaluation.dimension_scores)?)
        .bind(to_json(&evaluation.tags)?)
        .bind(&evaluation.model_provider)
        .bind(&evaluation.model_name)
        .bind(&evaluation.prompt_version)
        .bind(&evaluation.raw_response)
        .bind(evaluation.created_at)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"UPDATE photos
               SET aesthetic_score = ?1, has_been_scored = 1, score_date = ?2
               WHERE id = ?3"#,
        )
        .bind(evaluation.overall_score)
        .bind(evaluation.created_at)
        .bind(&evaluation.photo_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn find_latest_for_photo(&self, photo_id: &str) -> Result<Option<PhotoEvaluation>> {
        let row = sqlx::query_as::<_, PhotoEvaluationRow>(
            r#"SELECT *
               FROM photo_evaluations
               WHERE photo_id = ?1
               ORDER BY created_at DESC
               LIMIT 1"#,
        )
        .bind(photo_id)
        .fetch_optional(&self.db)
        .await?;

        row.map(PhotoEvaluation::try_from).transpose()
    }
}
