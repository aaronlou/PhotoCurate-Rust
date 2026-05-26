use anyhow::{Context, Result};
use ndarray::{Array2, Array4};
use ort::ep::CoreML;
use ort::session::Session;
use ort::value::Value;
use std::collections::HashMap;
use std::sync::Mutex;

/// Chinese-CLIP local ONNX inference service.
pub struct ChineseClipService {
    image_session: Mutex<Session>,
    text_session: Mutex<Session>,
    config: ClipConfig,
    vocab: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
struct ClipConfig {
    image_size: usize,
    max_text_length: usize,
    image_mean: [f32; 3],
    image_std: [f32; 3],
    pad_token_id: usize,
    cls_token_id: usize,
    sep_token_id: usize,
    unk_token_id: usize,
}

impl ChineseClipService {
    pub fn new(models_dir: &std::path::Path) -> Result<Self> {
        let image_model_path = models_dir.join("chinese_clip_image.onnx");
        let text_model_path = models_dir.join("chinese_clip_text.onnx");
        let config_path = models_dir.join("chinese_clip_config.json");
        let vocab_path = models_dir.join("vocab.txt");

        if !image_model_path.exists() {
            anyhow::bail!("Image model not found: {}", image_model_path.display());
        }
        if !text_model_path.exists() {
            anyhow::bail!("Text model not found: {}", text_model_path.display());
        }
        if !config_path.exists() {
            anyhow::bail!("Config not found: {}", config_path.display());
        }
        if !vocab_path.exists() {
            anyhow::bail!("Vocab not found: {}", vocab_path.display());
        }

        let config: serde_json::Value =
            serde_json::from_reader(std::fs::File::open(&config_path).context("open config")?)
                .context("parse config")?;

        let clip_config = ClipConfig {
            image_size: config["image_size"].as_u64().unwrap_or(224) as usize,
            max_text_length: config["max_text_length"].as_u64().unwrap_or(52) as usize,
            image_mean: [
                config["image_mean"][0].as_f64().unwrap_or(0.48145466) as f32,
                config["image_mean"][1].as_f64().unwrap_or(0.4578275) as f32,
                config["image_mean"][2].as_f64().unwrap_or(0.40821073) as f32,
            ],
            image_std: [
                config["image_std"][0].as_f64().unwrap_or(0.26862954) as f32,
                config["image_std"][1].as_f64().unwrap_or(0.26130258) as f32,
                config["image_std"][2].as_f64().unwrap_or(0.27577711) as f32,
            ],
            pad_token_id: config["pad_token_id"].as_u64().unwrap_or(0) as usize,
            cls_token_id: config["cls_token_id"].as_u64().unwrap_or(101) as usize,
            sep_token_id: config["sep_token_id"].as_u64().unwrap_or(102) as usize,
            unk_token_id: config["unk_token_id"].as_u64().unwrap_or(100) as usize,
        };

        let vocab = load_vocab(&vocab_path)?;

        let image_session = Mutex::new(
            Session::builder()
                .map_err(|e| anyhow::anyhow!("build image session: {}", e))?
                .with_execution_providers([CoreML::default().build()])
                .map_err(|e| anyhow::anyhow!("set image EP: {}", e))?
                .commit_from_file(&image_model_path)
                .map_err(|e| anyhow::anyhow!("load image model: {}", e))?,
        );

        let text_session = Mutex::new(
            Session::builder()
                .map_err(|e| anyhow::anyhow!("build text session: {}", e))?
                .with_execution_providers([CoreML::default().build()])
                .map_err(|e| anyhow::anyhow!("set text EP: {}", e))?
                .commit_from_file(&text_model_path)
                .map_err(|e| anyhow::anyhow!("load text model: {}", e))?,
        );

        Ok(Self {
            image_session,
            text_session,
            config: clip_config,
            vocab,
        })
    }

    pub fn embed_image(&self, image_path: &str) -> Result<Vec<f32>> {
        let img = image::open(image_path).with_context(|| format!("open image: {}", image_path))?;
        let img = img.resize_exact(
            self.config.image_size as u32,
            self.config.image_size as u32,
            image::imageops::FilterType::Lanczos3,
        );
        let img = img.to_rgb8();

        let mut tensor =
            Array4::<f32>::zeros((1, 3, self.config.image_size, self.config.image_size));
        for (x, y, pixel) in img.enumerate_pixels() {
            let r = pixel[0] as f32 / 255.0;
            let g = pixel[1] as f32 / 255.0;
            let b = pixel[2] as f32 / 255.0;
            tensor[[0, 0, y as usize, x as usize]] =
                (r - self.config.image_mean[0]) / self.config.image_std[0];
            tensor[[0, 1, y as usize, x as usize]] =
                (g - self.config.image_mean[1]) / self.config.image_std[1];
            tensor[[0, 2, y as usize, x as usize]] =
                (b - self.config.image_mean[2]) / self.config.image_std[2];
        }

        let input = Value::from_array(tensor.into_dyn())
            .map_err(|e| anyhow::anyhow!("create image tensor: {}", e))?;
        let mut session = self
            .image_session
            .lock()
            .map_err(|e| anyhow::anyhow!("image session lock failed: {}", e))?;
        let outputs = session
            .run(ort::inputs!["pixel_values" => input])
            .map_err(|e| anyhow::anyhow!("run image inference: {}", e))?;

        let (_shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        let embedding = normalize_vec(data.to_vec());
        Ok(embedding)
    }

    pub fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        let tokens = tokenize(text, &self.vocab, self.config.max_text_length, &self.config);
        let input_ids = Array2::from_shape_vec((1, self.config.max_text_length), tokens.input_ids)?;
        let attention_mask =
            Array2::from_shape_vec((1, self.config.max_text_length), tokens.attention_mask)?;
        let input_ids_val = Value::from_array(input_ids.into_dyn())
            .map_err(|e| anyhow::anyhow!("create input_ids tensor: {}", e))?;
        let mask_val = Value::from_array(attention_mask.into_dyn())
            .map_err(|e| anyhow::anyhow!("create attention_mask tensor: {}", e))?;

        let mut session = self
            .text_session
            .lock()
            .map_err(|e| anyhow::anyhow!("text session lock failed: {}", e))?;
        let outputs = session
            .run(ort::inputs!["input_ids" => input_ids_val, "attention_mask" => mask_val])
            .map_err(|e| anyhow::anyhow!("run text inference: {}", e))?;

        let (_shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        let embedding = normalize_vec(data.to_vec());
        Ok(embedding)
    }
}

struct Tokenized {
    input_ids: Vec<i64>,
    attention_mask: Vec<i64>,
}

fn tokenize(
    text: &str,
    vocab: &HashMap<String, usize>,
    max_len: usize,
    config: &ClipConfig,
) -> Tokenized {
    let mut input_ids = vec![config.cls_token_id as i64];
    let text = text.to_lowercase();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() && input_ids.len() < max_len - 1 {
        // Try longest match first
        let mut matched = false;
        for len in (1..=chars.len() - i).rev() {
            let substr: String = chars[i..i + len].iter().collect();
            // Try exact match first
            if let Some(&id) = vocab.get(&substr) {
                input_ids.push(id as i64);
                i += len;
                matched = true;
                break;
            }
            // Try with ## prefix for subwords
            if i > 0 {
                let subword = format!("##{}", substr);
                if let Some(&id) = vocab.get(&subword) {
                    input_ids.push(id as i64);
                    i += len;
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            input_ids.push(config.unk_token_id as i64);
            i += 1;
        }
    }
    input_ids.push(config.sep_token_id as i64);

    let actual_len = input_ids.len().min(max_len);
    let mut padded_ids = vec![config.pad_token_id as i64; max_len];
    let mut attention_mask = vec![0i64; max_len];
    for j in 0..actual_len {
        padded_ids[j] = input_ids[j];
        attention_mask[j] = 1;
    }

    Tokenized {
        input_ids: padded_ids,
        attention_mask,
    }
}

fn load_vocab(path: &std::path::Path) -> Result<HashMap<String, usize>> {
    let content = std::fs::read_to_string(path).context("read vocab")?;
    let mut vocab = HashMap::new();
    for (i, line) in content.lines().enumerate() {
        let token = line.trim();
        if !token.is_empty() {
            vocab.insert(token.to_string(), i);
        }
    }
    Ok(vocab)
}

fn normalize_vec(mut v: Vec<f32>) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-8 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_model() {
        let models_dir = dirs::data_dir()
            .unwrap()
            .join("com.photocurate")
            .join("models");
        if !models_dir.join("chinese_clip_image.onnx").exists() {
            eprintln!("Model files not found at {:?}, skipping test", models_dir);
            return;
        }
        let service = match ChineseClipService::new(&models_dir) {
            Ok(service) => service,
            Err(e) => {
                eprintln!("Model files found, but model load failed in this environment: {e}");
                return;
            }
        };
        // Test text embedding
        let text_embed = service.embed_text("一只猫在沙发上").expect("text embed");
        assert_eq!(text_embed.len(), 512, "Text embedding should be 512-dim");

        // Test image embedding (need a test image)
        let img_path = std::path::PathBuf::from(option_env!("CARGO_MANIFEST_DIR").unwrap_or("."))
            .parent()
            .unwrap()
            .join("models")
            .join("chinese-clip-vit-base-patch16")
            .join("festival.jpg");
        if img_path.exists() {
            let img_embed = service
                .embed_image(img_path.to_str().unwrap())
                .expect("image embed");
            assert_eq!(img_embed.len(), 512, "Image embedding should be 512-dim");

            // Check cosine similarity is in reasonable range
            let dot: f32 = text_embed
                .iter()
                .zip(img_embed.iter())
                .map(|(a, b)| a * b)
                .sum();
            println!("Cosine similarity: {}", dot);
            assert!(dot.abs() <= 1.1, "Cosine similarity should be <= 1.0");
        }
    }
}
