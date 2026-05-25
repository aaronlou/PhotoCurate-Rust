pub mod chinese_clip;

use crate::domain::models::ScoreResult;
use crate::error::{PhotoCurateError, Result};
use anyhow::Context;
use serde_json::json;
use std::sync::Arc;

pub use chinese_clip::ChineseClipService;

/// Embed an image using the best available method.
/// If local Chinese-CLIP model is available, use it. Otherwise fall back to Gemini API.
pub async fn embed_image(
    api_key: &str,
    image_path: &str,
    local: Option<Arc<ChineseClipService>>,
) -> Result<Vec<f64>> {
    if let Some(service) = local {
        let image_path = image_path.to_string();
        let embedding = tokio::task::spawn_blocking(move || service.embed_image(&image_path))
            .await
            .map_err(|e| PhotoCurateError::Other(format!("join error: {}", e)))?
            .map_err(|e| PhotoCurateError::Ai(e.to_string()))?;
        Ok(embedding.into_iter().map(|v| v as f64).collect())
    } else {
        embed_image_gemini(api_key, image_path)
            .await
            .map_err(|e| PhotoCurateError::Ai(e.to_string()))
    }
}

/// Embed text using the best available method.
/// If local Chinese-CLIP model is available, use it. Otherwise fall back to Gemini API.
pub async fn embed_text(
    api_key: &str,
    text: &str,
    local: Option<Arc<ChineseClipService>>,
) -> Result<Vec<f64>> {
    if let Some(service) = local {
        let text = text.to_string();
        let embedding = tokio::task::spawn_blocking(move || service.embed_text(&text))
            .await
            .map_err(|e| PhotoCurateError::Other(format!("join error: {}", e)))?
            .map_err(|e| PhotoCurateError::Ai(e.to_string()))?;
        Ok(embedding.into_iter().map(|v| v as f64).collect())
    } else {
        embed_text_gemini(api_key, text)
            .await
            .map_err(|e| PhotoCurateError::Ai(e.to_string()))
    }
}

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const GEMINI_EMBED_URL: &str = "https://generativelanguage.googleapis.com/v1";
const SCORE_MODEL: &str = "gemini-3.1-flash-lite";
const EMBED_MODEL: &str = "gemini-embedding-001";

pub async fn score_image(api_key: &str, image_path: &str) -> Result<ScoreResult> {
    score_image_internal(api_key, image_path)
        .await
        .map_err(|e| PhotoCurateError::Ai(e.to_string()))
}

async fn score_image_internal(api_key: &str, image_path: &str) -> anyhow::Result<ScoreResult> {
    let image_data = tokio::fs::read(image_path).await?;
    let base64_image =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_data);
    let mime_type = mime_type_for_path(image_path);

    let request_body = json!({
        "contents": [{
            "parts": [
                {"text": score_prompt()},
                {"inline_data": {"mime_type": mime_type, "data": base64_image}}
            ]
        }],
        "generationConfig": {
            "temperature": 0.3,
            "maxOutputTokens": 512
        }
    });

    let url = format!(
        "{}/models/{}:generateContent?key={}",
        GEMINI_BASE_URL, SCORE_MODEL, api_key
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("Gemini API error: {}", text);
    }

    let json: serde_json::Value = response.json().await?;
    let text = json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .context("missing response text")?;

    parse_score_response(text)
}

async fn embed_text_gemini(api_key: &str, text: &str) -> anyhow::Result<Vec<f64>> {
    let request_body = json!({
        "model": format!("models/{}", EMBED_MODEL),
        "content": {
            "parts": [{"text": text}]
        },
        "outputDimensionality": 768
    });

    let url = format!(
        "{}/models/{}:embedContent?key={}",
        GEMINI_EMBED_URL, EMBED_MODEL, api_key
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("Gemini embedding error: {}", text);
    }

    let json: serde_json::Value = response.json().await?;
    let values = json["embedding"]["values"]
        .as_array()
        .context("missing embedding values")?;

    Ok(values.iter().filter_map(|v| v.as_f64()).collect())
}

async fn embed_image_gemini(api_key: &str, image_path: &str) -> anyhow::Result<Vec<f64>> {
    let caption = caption_image(api_key, image_path).await?;
    embed_text_gemini(api_key, &caption).await
}

async fn caption_image(api_key: &str, image_path: &str) -> anyhow::Result<String> {
    let image_data = tokio::fs::read(image_path).await?;
    let base64_image =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_data);
    let mime_type = mime_type_for_path(image_path);

    let request_body = json!({
        "contents": [{
            "parts": [
                {"text": "Describe this image in one concise sentence for search indexing."},
                {"inline_data": {"mime_type": mime_type, "data": base64_image}}
            ]
        }],
        "generationConfig": {
            "temperature": 0.2,
            "maxOutputTokens": 128
        }
    });

    let url = format!(
        "{}/models/{}:generateContent?key={}",
        GEMINI_BASE_URL, SCORE_MODEL, api_key
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    let json: serde_json::Value = response.json().await?;
    let text = json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    Ok(text)
}

pub async fn validate_api_key(api_key: &str) -> Result<(bool, String)> {
    validate_api_key_internal(api_key)
        .await
        .map_err(|e| PhotoCurateError::Ai(e.to_string()))
}

async fn validate_api_key_internal(api_key: &str) -> anyhow::Result<(bool, String)> {
    if api_key.is_empty() {
        return Ok((false, "API Key 不能为空".to_string()));
    }

    let request_body = json!({
        "contents": [{"parts": [{"text": "Say OK"}]}],
        "generationConfig": {"maxOutputTokens": 5}
    });

    let url = format!(
        "{}/models/{}:generateContent?key={}",
        GEMINI_BASE_URL, SCORE_MODEL, api_key
    );

    let client = reqwest::Client::new();
    let response = match client
        .post(&url)
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return Ok((false, format!("网络错误: {}", e))),
    };

    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(_) => return Ok((false, "无法解析 API 响应".to_string())),
    };

    if json["candidates"].is_array() {
        return Ok((true, "API Key 验证成功".to_string()));
    }

    if let Some(error) = json["error"]["message"].as_str() {
        return Ok((false, error.to_string()));
    }

    Ok((false, "响应格式异常".to_string()))
}

fn mime_type_for_path(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "heic" | "heif" => "image/heic",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "tiff" | "tif" => "image/tiff",
        _ => "image/jpeg",
    }
}

fn score_prompt() -> &'static str {
    r#"You are a professional photography critic. Rate this image on a scale of 0-100 based on:
- Composition and framing
- Lighting and exposure
- Color harmony
- Subject matter and interest
- Technical quality

Respond in this exact format:
Score: [number]
Review: [2-3 sentences describing strengths and weaknesses]"#
}

fn parse_score_response(text: &str) -> anyhow::Result<ScoreResult> {
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

    Ok(ScoreResult { score, review })
}
