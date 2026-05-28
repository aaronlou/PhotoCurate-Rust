use crate::application::ports::PhotoRepository;
use crate::domain::models::{
    DimensionInsight, InsightPhoto, LibraryInsights, Photo, PhotoSortOrder, ScoreBucket,
    ScoreTrend, TextInsight,
};
use crate::error::Result;
use crate::infrastructure;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;

const HIGH_SCORE_THRESHOLD: f64 = 80.0;
const TOP_LIMIT: usize = 8;
const RECENT_TREND_WINDOW: usize = 12;

pub async fn get_library_insights(db: &Pool<Sqlite>) -> Result<LibraryInsights> {
    let photo_repo = infrastructure::repositories::SqlitePhotoRepository::new(db.clone());
    get_library_insights_with(&photo_repo).await
}

pub async fn get_library_insights_with(photos: &impl PhotoRepository) -> Result<LibraryInsights> {
    let library = photos.find_all(Some(&PhotoSortOrder::DateDesc)).await?;
    Ok(build_library_insights(&library))
}

fn build_library_insights(photos: &[Photo]) -> LibraryInsights {
    let evaluated: Vec<&Photo> = photos
        .iter()
        .filter(|photo| photo.latest_evaluation.is_some())
        .collect();
    let score_only_photos = photos
        .iter()
        .filter(|photo| photo.has_been_scored && photo.latest_evaluation.is_none())
        .count();
    let mut scores: Vec<f64> = evaluated
        .iter()
        .filter_map(|photo| {
            photo
                .latest_evaluation
                .as_ref()
                .map(|evaluation| evaluation.overall_score)
        })
        .collect();

    let average_score = average(&scores);
    let median_score = median(&mut scores);
    let high_score_count = scores
        .iter()
        .filter(|score| **score >= HIGH_SCORE_THRESHOLD)
        .count();
    let high_score_rate = if scores.is_empty() {
        0.0
    } else {
        high_score_count as f64 / scores.len() as f64
    };

    LibraryInsights {
        total_photos: photos.len(),
        evaluated_photos: evaluated.len(),
        score_only_photos,
        average_score,
        median_score,
        high_score_count,
        high_score_rate,
        score_distribution: score_distribution(&scores),
        dimension_averages: dimension_averages(&evaluated),
        top_strengths: text_frequency(&evaluated, TextKind::Strength),
        recurring_weaknesses: text_frequency(&evaluated, TextKind::Weakness),
        suggested_practices: text_frequency(&evaluated, TextKind::Suggestion),
        top_tags: text_frequency(&evaluated, TextKind::Tag),
        top_photos: top_photos(&evaluated),
        recent_trend: recent_trend(photos),
        coach_notes: coach_notes(average_score, &evaluated),
    }
}

fn average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }

    Some(values.iter().sum::<f64>() / values.len() as f64)
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }

    values.sort_by(|a, b| a.total_cmp(b));
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        Some((values[middle - 1] + values[middle]) / 2.0)
    } else {
        Some(values[middle])
    }
}

fn score_distribution(scores: &[f64]) -> Vec<ScoreBucket> {
    let buckets = [
        ("90+", 90.0, 100.0),
        ("80-89", 80.0, 89.0),
        ("70-79", 70.0, 79.0),
        ("60-69", 60.0, 69.0),
        ("<60", 0.0, 59.0),
    ];

    buckets
        .into_iter()
        .map(|(label, min, max)| ScoreBucket {
            label: label.to_string(),
            min,
            max,
            count: scores
                .iter()
                .filter(|score| score_in_bucket(**score, label))
                .count(),
        })
        .collect()
}

fn score_in_bucket(score: f64, label: &str) -> bool {
    match label {
        "90+" => score >= 90.0,
        "80-89" => (80.0..90.0).contains(&score),
        "70-79" => (70.0..80.0).contains(&score),
        "60-69" => (60.0..70.0).contains(&score),
        "<60" => score < 60.0,
        _ => false,
    }
}

fn dimension_averages(photos: &[&Photo]) -> Vec<DimensionInsight> {
    let mut dimensions: HashMap<String, (f64, usize)> = HashMap::new();

    for photo in photos {
        if let Some(evaluation) = &photo.latest_evaluation {
            for dimension in &evaluation.dimension_scores {
                let name = normalize_label(&dimension.name);
                if name.is_empty() {
                    continue;
                }
                let entry = dimensions.entry(name).or_insert((0.0, 0));
                entry.0 += dimension.score;
                entry.1 += 1;
            }
        }
    }

    let mut insights: Vec<DimensionInsight> = dimensions
        .into_iter()
        .map(|(name, (total, count))| DimensionInsight {
            name,
            average_score: total / count as f64,
            count,
        })
        .collect();

    insights.sort_by(|a, b| b.average_score.total_cmp(&a.average_score));
    insights
}

#[derive(Clone, Copy)]
enum TextKind {
    Strength,
    Weakness,
    Suggestion,
    Tag,
}

fn text_frequency(photos: &[&Photo], kind: TextKind) -> Vec<TextInsight> {
    let mut counts: HashMap<String, usize> = HashMap::new();

    for photo in photos {
        if let Some(evaluation) = &photo.latest_evaluation {
            let values = match kind {
                TextKind::Strength => &evaluation.strengths,
                TextKind::Weakness => &evaluation.weaknesses,
                TextKind::Suggestion => &evaluation.suggestions,
                TextKind::Tag => &evaluation.tags,
            };

            for value in values {
                let label = normalize_label(value);
                if label.is_empty() {
                    continue;
                }
                *counts.entry(label).or_insert(0) += 1;
            }
        }
    }

    let mut insights: Vec<TextInsight> = counts
        .into_iter()
        .map(|(label, count)| TextInsight { label, count })
        .collect();
    insights.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
    insights.truncate(TOP_LIMIT);
    insights
}

fn top_photos(photos: &[&Photo]) -> Vec<InsightPhoto> {
    let mut ranked: Vec<InsightPhoto> = photos
        .iter()
        .filter_map(|photo| {
            let evaluation = photo.latest_evaluation.as_ref()?;
            Some(InsightPhoto {
                id: photo.id.clone(),
                file_name: photo.file_name.clone(),
                score: evaluation.overall_score,
                summary: evaluation.summary.clone(),
            })
        })
        .collect();

    ranked.sort_by(|a, b| b.score.total_cmp(&a.score));
    ranked.truncate(6);
    ranked
}

fn recent_trend(photos: &[Photo]) -> Option<ScoreTrend> {
    let mut evaluated: Vec<&Photo> = photos
        .iter()
        .filter(|photo| photo.latest_evaluation.is_some())
        .collect();
    if evaluated.len() < RECENT_TREND_WINDOW * 2 {
        return None;
    }

    evaluated.sort_by(|a, b| b.date_modified.cmp(&a.date_modified));
    let recent_scores: Vec<f64> = evaluated
        .iter()
        .take(RECENT_TREND_WINDOW)
        .filter_map(|photo| {
            photo
                .latest_evaluation
                .as_ref()
                .map(|evaluation| evaluation.overall_score)
        })
        .collect();
    let earlier_scores: Vec<f64> = evaluated
        .iter()
        .skip(RECENT_TREND_WINDOW)
        .take(RECENT_TREND_WINDOW)
        .filter_map(|photo| {
            photo
                .latest_evaluation
                .as_ref()
                .map(|evaluation| evaluation.overall_score)
        })
        .collect();

    let recent_average = average(&recent_scores)?;
    let earlier_average = average(&earlier_scores)?;

    Some(ScoreTrend {
        earlier_average,
        recent_average,
        delta: recent_average - earlier_average,
        recent_count: recent_scores.len(),
    })
}

fn coach_notes(average_score: Option<f64>, photos: &[&Photo]) -> Vec<String> {
    if photos.is_empty() {
        return vec![
            "先为一组照片生成 AI 评价，洞察页会开始识别你的稳定优势和反复短板。".to_string(),
            "建议至少积累 20 张带评价的照片，再观察维度趋势和练习方向。".to_string(),
        ];
    }

    let dimensions = dimension_averages(photos);
    let weaknesses = text_frequency(photos, TextKind::Weakness);
    let strengths = text_frequency(photos, TextKind::Strength);
    let mut notes = Vec::new();

    if let Some(score) = average_score {
        let note = if score >= 82.0 {
            "整体评分已经进入比较稳定的高质量区间，可以开始用系列化和主题一致性来拉开作品辨识度。"
        } else if score >= 72.0 {
            "整体表现有不错基础，下一阶段适合优先减少反复出现的问题，而不是只追求单张高分。"
        } else {
            "当前作品还有较大上升空间，先把主体、光线和画面取舍练稳，会比复杂后期更有效。"
        };
        notes.push(note.to_string());
    }

    if let Some(best) = strengths.first() {
        notes.push(format!(
            "你的高频优势是“{}”，可以把它当成个人风格的起点继续强化。",
            best.label
        ));
    }

    if let Some(weakness) = weaknesses.first() {
        notes.push(format!(
            "最值得优先处理的问题是“{}”，建议下一轮拍摄专门围绕它做取舍练习。",
            weakness.label
        ));
    }

    if let Some(lowest_dimension) = dimensions
        .iter()
        .min_by(|a, b| a.average_score.total_cmp(&b.average_score))
    {
        notes.push(format!(
            "维度均分最低的是“{}”，可以作为接下来一周的练习主题。",
            lowest_dimension.name
        ));
    }

    notes.truncate(4);
    notes
}

fn normalize_label(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| matches!(c, '，' | ',' | '。' | '.' | '；' | ';' | ':' | '：'))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::{DimensionScore, PhotoEvaluation};
    use chrono::{Duration, Utc};

    fn photo(index: usize, score: Option<f64>, date_offset_days: i64) -> Photo {
        let latest_evaluation = score.map(|overall_score| PhotoEvaluation {
            id: format!("evaluation-{index}"),
            photo_id: format!("photo-{index}"),
            overall_score,
            summary: format!("summary {index}"),
            strengths: vec!["构图稳定".to_string(), " 光线自然，".to_string()],
            weaknesses: vec!["背景干扰".to_string()],
            suggestions: vec!["练习主体取舍".to_string()],
            dimension_scores: vec![
                DimensionScore {
                    name: "构图".to_string(),
                    score: overall_score,
                    note: None,
                },
                DimensionScore {
                    name: "光线".to_string(),
                    score: overall_score - 5.0,
                    note: None,
                },
            ],
            tags: vec!["人像".to_string()],
            model_provider: "test".to_string(),
            model_name: "test-model".to_string(),
            prompt_version: "test-prompt".to_string(),
            raw_response: None,
            created_at: Utc::now(),
        });

        Photo {
            id: format!("photo-{index}"),
            file_path: format!("/tmp/photo-{index}.jpg"),
            file_name: format!("photo-{index}.jpg"),
            file_size: 100,
            date_created: None,
            date_modified: Utc::now() - Duration::days(date_offset_days),
            camera_make: None,
            camera_model: None,
            lens_model: None,
            focal_length: None,
            aperture: None,
            shutter_speed: None,
            iso: None,
            width: None,
            height: None,
            aesthetic_score: score,
            has_been_scored: score.is_some(),
            score_date: None,
            has_embedding: false,
            embedding_version: None,
            thumbnail_path: None,
            directory_id: None,
            has_been_exported: false,
            export_date: None,
            latest_evaluation,
        }
    }

    #[test]
    fn empty_library_returns_guidance_without_scores() {
        let insights = build_library_insights(&[]);

        assert_eq!(insights.total_photos, 0);
        assert_eq!(insights.evaluated_photos, 0);
        assert_eq!(insights.average_score, None);
        assert_eq!(
            insights
                .score_distribution
                .iter()
                .map(|bucket| bucket.count)
                .sum::<usize>(),
            0
        );
        assert!(!insights.coach_notes.is_empty());
    }

    #[test]
    fn distinguishes_legacy_scores_from_evaluations() {
        let photos = vec![photo(1, Some(88.0), 1), {
            let mut legacy = photo(2, None, 2);
            legacy.aesthetic_score = Some(76.0);
            legacy.has_been_scored = true;
            legacy
        }];

        let insights = build_library_insights(&photos);

        assert_eq!(insights.total_photos, 2);
        assert_eq!(insights.evaluated_photos, 1);
        assert_eq!(insights.score_only_photos, 1);
        assert_eq!(insights.average_score, Some(88.0));
    }

    #[test]
    fn aggregates_scores_dimensions_and_text_frequency() {
        let photos = vec![
            photo(1, Some(95.0), 1),
            photo(2, Some(89.9), 2),
            photo(3, Some(72.0), 3),
            photo(4, Some(58.0), 4),
        ];

        let insights = build_library_insights(&photos);

        assert_eq!(insights.high_score_count, 2);
        assert_eq!(insights.high_score_rate, 0.5);
        assert_eq!(insights.median_score, Some(80.95));
        assert_eq!(insights.score_distribution[0].count, 1);
        assert_eq!(insights.score_distribution[1].count, 1);
        assert_eq!(insights.score_distribution[2].count, 1);
        assert_eq!(insights.score_distribution[4].count, 1);
        assert_eq!(insights.dimension_averages.len(), 2);
        assert_eq!(insights.top_strengths[0].label, "光线自然");
        assert_eq!(insights.recurring_weaknesses[0].count, 4);
        assert_eq!(insights.top_photos[0].file_name, "photo-1.jpg");
    }

    #[test]
    fn reports_recent_trend_after_enough_evaluations() {
        let photos: Vec<Photo> = (0..24)
            .map(|index| {
                let score = if index < 12 { 90.0 } else { 70.0 };
                photo(index, Some(score), index as i64)
            })
            .collect();

        let insights = build_library_insights(&photos);
        let trend = insights.recent_trend.expect("trend should exist");

        assert_eq!(trend.recent_average, 90.0);
        assert_eq!(trend.earlier_average, 70.0);
        assert_eq!(trend.delta, 20.0);
    }
}
