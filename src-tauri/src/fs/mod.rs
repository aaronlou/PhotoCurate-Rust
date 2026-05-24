use crate::models::Photo;
use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use std::path::Path;
use std::process::Command;

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "heic", "heif", "png", "tiff", "tif",
    // RAW formats - thumbnails via sips
    "cr2", "cr3", "nef", "arw", "dng", "raw",
];

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let e = e.to_lowercase();
            SUPPORTED_EXTENSIONS.contains(&e.as_str())
        })
        .unwrap_or(false)
}

pub async fn scan_directory(dir_path: &str, dir_id: &str) -> Result<Vec<Photo>> {
    let mut photos = Vec::new();

    for entry in walkdir::WalkDir::new(dir_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || !is_supported_image(path) {
            continue;
        }

        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let exif = read_exif(path.to_str().unwrap_or(""));
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let photo = Photo {
            id: uuid::Uuid::new_v4().to_string(),
            file_path: path.to_string_lossy().to_string(),
            file_name,
            file_size: metadata.len() as i64,
            date_created: exif.date_taken.map(|d| d.and_utc()),
            date_modified: metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
                .flatten()
                .unwrap_or_else(|| Utc::now()),
            camera_make: exif.camera_make,
            camera_model: exif.camera_model,
            lens_model: exif.lens_model,
            focal_length: exif.focal_length,
            aperture: exif.aperture,
            shutter_speed: exif.shutter_speed,
            iso: exif.iso,
            width: exif.width,
            height: exif.height,
            aesthetic_score: None,
            has_been_scored: false,
            score_date: None,
            has_embedding: false,
            embedding_version: None,
            thumbnail_path: None,
            directory_id: Some(dir_id.to_string()),
            has_been_exported: false,
            export_date: None,
        };

        photos.push(photo);
    }

    Ok(photos)
}

#[derive(Debug, Default)]
pub struct ExifData {
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub focal_length: Option<f64>,
    pub aperture: Option<f64>,
    pub shutter_speed: Option<f64>,
    pub iso: Option<i32>,
    pub date_taken: Option<chrono::NaiveDateTime>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

pub fn read_exif(path: &str) -> ExifData {
    let mut data = ExifData::default();

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return data,
    };
    let mut bufreader = std::io::BufReader::new(&file);
    let exifreader = exif::Reader::new();
    let exif = match exifreader.read_from_container(&mut bufreader) {
        Ok(e) => e,
        Err(_) => return data,
    };

    for field in exif.fields() {
        match field.tag {
            exif::Tag::Make => {
                data.camera_make = Some(field.display_value().with_unit(&exif).to_string());
            }
            exif::Tag::Model => {
                data.camera_model = Some(field.display_value().with_unit(&exif).to_string());
            }
            exif::Tag::LensModel => {
                data.lens_model = Some(field.display_value().with_unit(&exif).to_string());
            }
            exif::Tag::FocalLength => {
                data.focal_length = parse_rational(&field.value);
            }
            exif::Tag::FNumber => {
                data.aperture = parse_rational(&field.value);
            }
            exif::Tag::ExposureTime => {
                data.shutter_speed = parse_rational(&field.value);
            }
            exif::Tag::PhotographicSensitivity => {
                data.iso = parse_int(&field.value);
            }
            exif::Tag::DateTimeOriginal => {
                if let exif::Value::Ascii(ref v) = field.value {
                    if let Some(s) = v.first() {
                        let s = std::str::from_utf8(s).unwrap_or("");
                        data.date_taken = NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S").ok();
                    }
                }
            }
            exif::Tag::ImageWidth => {
                data.width = parse_int(&field.value);
            }
            exif::Tag::ImageLength => {
                data.height = parse_int(&field.value);
            }
            _ => {}
        }
    }

    data
}

fn parse_rational(value: &exif::Value) -> Option<f64> {
    match value {
        exif::Value::Rational(ref v) => v.first().map(|r| r.to_f64()),
        exif::Value::SRational(ref v) => v.first().map(|r| r.to_f64()),
        exif::Value::Ascii(ref v) => v
            .first()
            .and_then(|s| std::str::from_utf8(s).ok())
            .and_then(|s| s.parse().ok()),
        _ => None,
    }
}

fn parse_int(value: &exif::Value) -> Option<i32> {
    match value {
        exif::Value::Short(ref v) => v.first().map(|&n| n as i32),
        exif::Value::Long(ref v) => v.first().map(|&n| n as i32),
        exif::Value::SShort(ref v) => v.first().map(|&n| n as i32),
        exif::Value::SLong(ref v) => v.first().map(|&n| n as i32),
        exif::Value::Ascii(ref v) => v
            .first()
            .and_then(|s| std::str::from_utf8(s).ok())
            .and_then(|s| s.parse().ok()),
        _ => None,
    }
}

pub async fn generate_thumbnail(
    photo_path: &str,
    cache_dir: &Path,
    max_dimension: u32,
) -> Result<String> {
    let hash = blake3::hash(photo_path.as_bytes()).to_hex().to_string();
    let thumb_path = cache_dir.join(format!("{}.jpg", hash));

    if thumb_path.exists() {
        return Ok(thumb_path.to_string_lossy().to_string());
    }

    // Use macOS sips for reliable HEIC/RAW support
    let output = Command::new("sips")
        .args(&[
            "-Z",
            &max_dimension.to_string(),
            photo_path,
            "--out",
            thumb_path.to_str().context("invalid path")?,
        ])
        .output()
        .context("failed to run sips")?;

    if !output.status.success() {
        // Fallback: try image crate for basic formats
        generate_thumbnail_rust(photo_path, &thumb_path, max_dimension).await?;
    }

    Ok(thumb_path.to_string_lossy().to_string())
}

async fn generate_thumbnail_rust(
    photo_path: &str,
    thumb_path: &Path,
    max_dimension: u32,
) -> Result<()> {
    let img = image::open(photo_path)?;
    let thumb = img.resize(
        max_dimension,
        max_dimension,
        image::imageops::FilterType::Lanczos3,
    );
    thumb.save_with_format(thumb_path, image::ImageFormat::Jpeg)?;
    Ok(())
}
