pub mod chinese_clip;
mod evaluation;

use crate::domain::models::{AiSettings, ScoreResult};
use crate::error::{PhotoCurateError, Result};
use anyhow::Context;
use serde_json::json;
use std::sync::Arc;

pub use chinese_clip::ChineseClipService;
pub use evaluation::{PhotoEvaluationPrompt, PHOTO_EVALUATION_PROMPT};

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
const GEMINI_SCORE_MODEL: &str = "gemini-3.1-flash-lite";
const QWEN_BASE_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1";
const QWEN_DEFAULT_MODEL: &str = "qwen3-vl-plus";
const EMBED_MODEL: &str = "gemini-embedding-001";

pub fn default_scoring_model(provider: &str) -> &'static str {
    match provider {
        "qwen_vl" => QWEN_DEFAULT_MODEL,
        "openai_compatible_vision" => "gpt-4o-mini",
        _ => GEMINI_SCORE_MODEL,
    }
}

pub fn default_scoring_base_url(provider: &str) -> &'static str {
    match provider {
        "qwen_vl" => QWEN_BASE_URL,
        _ => "",
    }
}

pub fn normalize_scoring_provider(provider: &str) -> &'static str {
    match provider {
        "qwen_vl" => "qwen_vl",
        "openai_compatible_vision" => "openai_compatible_vision",
        _ => "gemini",
    }
}

pub async fn score_image(settings: &AiSettings, image_path: &str) -> Result<ScoreResult> {
    score_image_internal(settings, image_path)
        .await
        .map_err(|e| PhotoCurateError::Ai(e.to_string()))
}

async fn score_image_internal(
    settings: &AiSettings,
    image_path: &str,
) -> anyhow::Result<ScoreResult> {
    match normalize_scoring_provider(&settings.scoring_provider) {
        "gemini" => score_image_gemini(settings, image_path).await,
        "qwen_vl" | "openai_compatible_vision" => {
            score_image_openai_compatible_vision(settings, image_path).await
        }
        _ => score_image_gemini(settings, image_path).await,
    }
}

async fn score_image_gemini(
    settings: &AiSettings,
    image_path: &str,
) -> anyhow::Result<ScoreResult> {
    let image_data = tokio::fs::read(image_path).await?;
    let base64_image =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_data);
    let mime_type = mime_type_for_path(image_path);
    let model = model_or_default(&settings.scoring_model, "gemini");

    let request_body = json!({
        "contents": [{
            "parts": [
                {"text": PHOTO_EVALUATION_PROMPT.text},
                {"inline_data": {"mime_type": mime_type, "data": base64_image}}
            ]
        }],
        "generationConfig": {
            "temperature": 0.3,
            "maxOutputTokens": 1024
        }
    });

    let url = format!(
        "{}/models/{}:generateContent?key={}",
        GEMINI_BASE_URL, model, settings.scoring_api_key
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

    evaluation::parse_score_response(text)
}

async fn score_image_openai_compatible_vision(
    settings: &AiSettings,
    image_path: &str,
) -> anyhow::Result<ScoreResult> {
    let image_data = tokio::fs::read(image_path).await?;
    let base64_image =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_data);
    let mime_type = mime_type_for_path(image_path);
    let image_url = format!("data:{mime_type};base64,{base64_image}");
    let provider = normalize_scoring_provider(&settings.scoring_provider);
    let model = model_or_default(&settings.scoring_model, provider);
    let base_url = base_url_or_default(&settings.scoring_base_url, provider);
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let request_body = json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": PHOTO_EVALUATION_PROMPT.text},
                {"type": "image_url", "image_url": {"url": image_url}}
            ]
        }],
        "temperature": 0.3,
        "max_tokens": 1024
    });

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .bearer_auth(&settings.scoring_api_key)
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("{} API error: {}", provider_label(provider), text);
    }

    let json: serde_json::Value = response.json().await?;
    let text = json["choices"][0]["message"]["content"]
        .as_str()
        .context("missing response text")?;

    evaluation::parse_score_response(text)
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
        GEMINI_BASE_URL, GEMINI_SCORE_MODEL, api_key
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

pub async fn validate_api_key(settings: &AiSettings) -> Result<(bool, String)> {
    validate_api_key_internal(settings)
        .await
        .map_err(|e| PhotoCurateError::Ai(e.to_string()))
}

async fn validate_api_key_internal(settings: &AiSettings) -> anyhow::Result<(bool, String)> {
    if settings.scoring_api_key.is_empty() {
        return Ok((false, "API Key 不能为空".to_string()));
    }

    match normalize_scoring_provider(&settings.scoring_provider) {
        "gemini" => validate_gemini_api_key(settings).await,
        "qwen_vl" | "openai_compatible_vision" => {
            validate_openai_compatible_api_key(settings).await
        }
        provider => Ok((false, format!("不支持的评分服务: {provider}"))),
    }
}

async fn validate_gemini_api_key(settings: &AiSettings) -> anyhow::Result<(bool, String)> {
    let request_body = json!({
        "contents": [{"parts": [{"text": "Say OK"}]}],
        "generationConfig": {"maxOutputTokens": 5}
    });
    let model = model_or_default(&settings.scoring_model, "gemini");

    let url = format!(
        "{}/models/{}:generateContent?key={}",
        GEMINI_BASE_URL, model, settings.scoring_api_key
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

async fn validate_openai_compatible_api_key(
    settings: &AiSettings,
) -> anyhow::Result<(bool, String)> {
    let provider = normalize_scoring_provider(&settings.scoring_provider);
    let model = model_or_default(&settings.scoring_model, provider);
    let base_url = base_url_or_default(&settings.scoring_base_url, provider);
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let request_body = json!({
        "model": model,
        "messages": [{"role": "user", "content": "Say OK"}],
        "max_tokens": 5,
        "temperature": 0.0
    });

    let client = reqwest::Client::new();
    let response = match client
        .post(url)
        .bearer_auth(&settings.scoring_api_key)
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return Ok((false, format!("网络错误: {}", e))),
    };

    let status = response.status();
    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(_) => return Ok((false, "无法解析 API 响应".to_string())),
    };

    if status.is_success() && json["choices"].is_array() {
        return Ok((true, "API Key 验证成功".to_string()));
    }

    if let Some(error) = json["error"]["message"].as_str() {
        return Ok((false, error.to_string()));
    }

    Ok((false, "响应格式异常".to_string()))
}

fn mime_type_for_path(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "heic" | "heif" => "image/heic",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "tiff" | "tif" => "image/tiff",
        _ => "image/jpeg",
    }
}

fn model_or_default<'a>(model: &'a str, provider: &str) -> &'a str {
    if model.trim().is_empty() {
        default_scoring_model(provider)
    } else {
        model.trim()
    }
}

fn base_url_or_default<'a>(base_url: &'a str, provider: &str) -> &'a str {
    if base_url.trim().is_empty() {
        default_scoring_base_url(provider)
    } else {
        base_url.trim()
    }
}

fn provider_label(provider: &str) -> &'static str {
    match provider {
        "qwen_vl" => "Qwen-VL",
        "openai_compatible_vision" => "OpenAI-compatible vision",
        _ => "Gemini",
    }
}
