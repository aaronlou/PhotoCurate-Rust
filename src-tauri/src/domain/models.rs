use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Directory {
    pub id: String,
    pub path: String,
    pub is_monitoring: bool,
    pub date_added: DateTime<Utc>,
    pub bookmark_data: Option<Vec<u8>>,
}

impl Directory {
    pub fn new(id: String, path: String, bookmark_data: Option<Vec<u8>>) -> Self {
        Self {
            id,
            path,
            is_monitoring: true,
            date_added: Utc::now(),
            bookmark_data,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: i64,
    pub date_created: Option<DateTime<Utc>>,
    pub date_modified: DateTime<Utc>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub focal_length: Option<f64>,
    pub aperture: Option<f64>,
    pub shutter_speed: Option<f64>,
    pub iso: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub aesthetic_score: Option<f64>,
    pub has_been_scored: bool,
    pub score_date: Option<DateTime<Utc>>,
    pub has_embedding: bool,
    pub embedding_version: Option<i32>,
    pub thumbnail_path: Option<String>,
    pub directory_id: Option<String>,
    pub has_been_exported: bool,
    pub export_date: Option<DateTime<Utc>>,
    pub latest_evaluation: Option<PhotoEvaluation>,
}

impl Photo {
    pub fn extension(&self) -> Option<String> {
        self.file_path.rsplit('.').next().map(|s| s.to_lowercase())
    }

    pub fn is_raw(&self) -> bool {
        matches!(
            self.extension().as_deref(),
            Some("cr2" | "cr3" | "nef" | "arw" | "dng" | "raw")
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    pub id: String,
    pub provider: String,
    pub api_key: String,
    pub has_api_key: bool,
    pub key_storage: String,
    pub scoring_provider: String,
    pub scoring_model: String,
    pub scoring_base_url: String,
    pub scoring_api_key: String,
    pub has_scoring_api_key: bool,
    pub scoring_key_storage: String,
    pub ollama_base_url: String,
    pub ollama_embed_model: String,
    pub ollama_vision_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingUsage {
    pub period_start: Option<DateTime<Utc>>,
    pub period_end: Option<DateTime<Utc>>,
    pub included_credits: i64,
    pub used_credits: i64,
    pub remaining_credits: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingStatus {
    pub provider: String,
    pub plan_id: String,
    pub status: String,
    pub account_id: String,
    pub renews_at: Option<DateTime<Utc>>,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub usage: BillingUsage,
    pub can_use_managed_ai: bool,
    pub checkout_available: bool,
    pub restore_available: bool,
    pub billing_portal_available: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingActionResult {
    pub status: BillingStatus,
    pub message: String,
    pub checkout_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreResult {
    pub score: f64,
    pub review: String,
    pub summary: String,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub suggestions: Vec<String>,
    pub dimension_scores: Vec<DimensionScore>,
    pub tags: Vec<String>,
    pub raw_response: String,
}

impl ScoreResult {
    pub fn from_score_and_review(score: f64, review: String) -> Self {
        Self {
            score,
            summary: review.clone(),
            review,
            strengths: vec![],
            weaknesses: vec![],
            suggestions: vec![],
            dimension_scores: vec![],
            tags: vec![],
            raw_response: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    pub name: String,
    pub score: f64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoEvaluation {
    pub id: String,
    pub photo_id: String,
    pub overall_score: f64,
    pub summary: String,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub suggestions: Vec<String>,
    pub dimension_scores: Vec<DimensionScore>,
    pub tags: Vec<String>,
    pub model_provider: String,
    pub model_name: String,
    pub prompt_version: String,
    pub raw_response: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringFailure {
    pub photo_id: String,
    pub file_name: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringRunResult {
    pub total_count: usize,
    pub success_count: usize,
    pub failed_count: usize,
    pub failures: Vec<ScoringFailure>,
}

impl ScoringRunResult {
    pub fn new(total_count: usize) -> Self {
        Self {
            total_count,
            success_count: 0,
            failed_count: 0,
            failures: vec![],
        }
    }

    pub fn record_success(&mut self) {
        self.success_count += 1;
    }

    pub fn record_failure(&mut self, photo_id: String, file_name: String, error: String) {
        self.failed_count += 1;
        self.failures.push(ScoringFailure {
            photo_id,
            file_name,
            error,
        });
    }

    pub fn all_failed(&self) -> bool {
        self.total_count > 0 && self.success_count == 0 && self.failed_count > 0
    }
}

impl PhotoEvaluation {
    pub fn from_score_result(
        photo_id: String,
        result: ScoreResult,
        model_provider: String,
        model_name: String,
        prompt_version: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            photo_id,
            overall_score: result.score,
            summary: if result.summary.trim().is_empty() {
                result.review
            } else {
                result.summary
            },
            strengths: result.strengths,
            weaknesses: result.weaknesses,
            suggestions: result.suggestions,
            dimension_scores: result.dimension_scores,
            tags: result.tags,
            model_provider,
            model_name,
            prompt_version,
            raw_response: if result.raw_response.trim().is_empty() {
                None
            } else {
                Some(result.raw_response)
            },
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub photo: Photo,
    pub similarity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub exported_count: usize,
    pub failed_count: usize,
    pub failed_photos: Vec<ExportFailure>,
}

impl ExportResult {
    pub fn empty() -> Self {
        Self {
            exported_count: 0,
            failed_count: 0,
            failed_photos: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFailure {
    pub id: String,
    pub file_name: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryInsights {
    pub total_photos: usize,
    pub evaluated_photos: usize,
    pub score_only_photos: usize,
    pub average_score: Option<f64>,
    pub median_score: Option<f64>,
    pub high_score_count: usize,
    pub high_score_rate: f64,
    pub score_distribution: Vec<ScoreBucket>,
    pub dimension_averages: Vec<DimensionInsight>,
    pub top_strengths: Vec<TextInsight>,
    pub recurring_weaknesses: Vec<TextInsight>,
    pub suggested_practices: Vec<TextInsight>,
    pub top_tags: Vec<TextInsight>,
    pub top_photos: Vec<InsightPhoto>,
    pub recent_trend: Option<ScoreTrend>,
    pub coach_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBucket {
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionInsight {
    pub name: String,
    pub average_score: f64,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextInsight {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightPhoto {
    pub id: String,
    pub file_name: String,
    pub score: f64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreTrend {
    pub earlier_average: f64,
    pub recent_average: f64,
    pub delta: f64,
    pub recent_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingProgressEvent {
    pub current: usize,
    pub total: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhotoSortOrder {
    #[serde(rename = "date_desc")]
    DateDesc,
    #[serde(rename = "score_desc")]
    ScoreDesc,
    #[serde(rename = "score_asc")]
    ScoreAsc,
}
