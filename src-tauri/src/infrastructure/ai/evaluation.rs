use crate::domain::models::{DimensionScore, ScoreResult};
use anyhow::Context;

pub struct PhotoEvaluationPrompt {
    pub version: &'static str,
    pub text: &'static str,
}

pub const PHOTO_EVALUATION_PROMPT: PhotoEvaluationPrompt = PhotoEvaluationPrompt {
    version: "photo-evaluation-v1",
    text: r#"You are a professional photography critic. Evaluate this photo on a 0-100 scale.

Assess:
- Composition and framing
- Lighting and exposure
- Color harmony
- Subject clarity
- Technical quality
- Mood and storytelling

Respond with compact JSON only, in Chinese for all prose:
{
  "score": 86,
  "summary": "2-3 sentences with the overall judgment.",
  "strengths": ["specific strength 1", "specific strength 2"],
  "weaknesses": ["specific weakness 1"],
  "suggestions": ["actionable suggestion 1", "actionable suggestion 2"],
  "dimension_scores": [
    {"name": "构图", "score": 82, "note": "short note"},
    {"name": "光线", "score": 88, "note": "short note"},
    {"name": "色彩", "score": 84, "note": "short note"},
    {"name": "主体", "score": 80, "note": "short note"},
    {"name": "技术", "score": 86, "note": "short note"},
    {"name": "叙事", "score": 78, "note": "short note"}
  ],
  "tags": ["自然光", "明确主体"]
}"#,
};

pub(super) fn parse_score_response(text: &str) -> anyhow::Result<ScoreResult> {
    if let Ok(result) = parse_json_score_response(text) {
        return Ok(result);
    }

    let score_line = text
        .lines()
        .find(|l| l.to_lowercase().contains("score:"))
        .context("missing score line")?;

    let score: f64 = score_line
        .split(':')
        .nth(1)
        .context("invalid score format")?
        .trim()
        .split_whitespace()
        .next()
        .context("invalid score format")?
        .parse()
        .context("failed to parse score")?;

    let review = text
        .lines()
        .find(|l| l.to_lowercase().contains("review:"))
        .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
        .unwrap_or_default();

    let mut result = ScoreResult::from_score_and_review(score, review);
    result.raw_response = text.to_string();
    Ok(result)
}

fn parse_json_score_response(text: &str) -> anyhow::Result<ScoreResult> {
    let trimmed = text.trim();
    let json_text = if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            &trimmed[start..=end]
        } else {
            trimmed
        }
    } else {
        trimmed
    };

    let json: serde_json::Value = serde_json::from_str(json_text)?;
    let score = json["score"].as_f64().context("missing score")?;
    let summary = json["summary"]
        .as_str()
        .or_else(|| json["review"].as_str())
        .unwrap_or("")
        .to_string();
    let review = json["review"].as_str().unwrap_or(&summary).to_string();

    Ok(ScoreResult {
        score,
        review,
        summary,
        strengths: parse_string_array(&json["strengths"]),
        weaknesses: parse_string_array(&json["weaknesses"]),
        suggestions: parse_string_array(&json["suggestions"]),
        dimension_scores: parse_dimension_scores(&json["dimension_scores"]),
        tags: parse_string_array(&json["tags"]),
        raw_response: text.to_string(),
    })
}

fn parse_string_array(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_dimension_scores(value: &serde_json::Value) -> Vec<DimensionScore> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item["name"].as_str()?.trim().to_string();
                    if name.is_empty() {
                        return None;
                    }
                    let score = item["score"].as_f64()?;
                    let note = item["note"]
                        .as_str()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    Some(DimensionScore { name, score, note })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_json_score_response() {
        let response = r#"
        ```json
        {
          "score": 91,
          "summary": "画面干净，主体明确。",
          "strengths": ["构图稳定"],
          "weaknesses": ["背景略乱"],
          "suggestions": ["压暗背景"],
          "dimension_scores": [
            {"name": "构图", "score": 90, "note": "主体位置稳定"}
          ],
          "tags": ["自然光"]
        }
        ```
        "#;

        let result = parse_score_response(response).expect("parse json score response");

        assert_eq!(result.score, 91.0);
        assert_eq!(result.summary, "画面干净，主体明确。");
        assert_eq!(result.strengths, vec!["构图稳定"]);
        assert_eq!(result.dimension_scores[0].name, "构图");
        assert_eq!(result.tags, vec!["自然光"]);
    }

    #[test]
    fn parses_legacy_score_and_review_response() {
        let result = parse_score_response("Score: 76\nReview: 主体清楚，但光线偏平。")
            .expect("parse legacy score response");

        assert_eq!(result.score, 76.0);
        assert_eq!(result.review, "主体清楚，但光线偏平。");
        assert_eq!(result.summary, "主体清楚，但光线偏平。");
    }

    #[test]
    fn rejects_response_without_score() {
        let error = parse_score_response("Review: no score here").expect_err("missing score");

        assert!(error.to_string().contains("missing score line"));
    }
}
