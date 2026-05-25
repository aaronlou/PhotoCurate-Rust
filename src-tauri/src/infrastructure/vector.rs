use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    pub photo_id: String,
    pub vector: Vec<f64>,
}

#[derive(Debug, Default)]
pub struct VectorIndex {
    entries: HashMap<String, Vec<f64>>,
    dimension: usize,
}

pub type SharedVectorIndex = Arc<RwLock<VectorIndex>>;

pub fn create_index() -> SharedVectorIndex {
    Arc::new(RwLock::new(VectorIndex::default()))
}

impl VectorIndex {
    pub fn configure(&mut self, dimension: usize) {
        self.dimension = dimension;
    }

    pub fn add(&mut self, photo_id: String, vector: Vec<f64>) {
        if self.dimension == 0 {
            self.dimension = vector.len();
        }
        self.entries.insert(photo_id, vector);
    }

    pub fn remove(&mut self, photo_id: &str) {
        self.entries.remove(photo_id);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn search(&self, query: &[f64], top_k: usize, min_similarity: f64) -> Vec<(String, f64)> {
        let query_norm = normalize(query);
        let mut results: Vec<(String, f64)> = self
            .entries
            .iter()
            .filter_map(|(id, vec)| {
                let similarity = cosine_similarity(&query_norm, &normalize(vec));
                if similarity >= min_similarity {
                    Some((id.clone(), similarity))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.into_iter().take(top_k).collect()
    }
}

fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

fn normalize(v: &[f64]) -> Vec<f64> {
    let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm == 0.0 {
        v.to_vec()
    } else {
        v.iter().map(|x| x / norm).collect()
    }
}
