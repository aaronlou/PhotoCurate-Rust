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
}

pub type SharedVectorIndex = Arc<RwLock<VectorIndex>>;

pub fn create_index() -> SharedVectorIndex {
    Arc::new(RwLock::new(VectorIndex::default()))
}

impl crate::application::ports::VectorIndexStore for SharedVectorIndex {
    async fn add(&self, photo_id: String, vector: Vec<f64>) {
        let mut index = self.write().await;
        index.add(photo_id, vector);
    }

    async fn clear(&self) {
        let mut index = self.write().await;
        index.clear();
    }

    async fn search(&self, query: &[f64], top_k: usize, min_similarity: f64) -> Vec<(String, f64)> {
        let index = self.read().await;
        index.search(query, top_k, min_similarity)
    }

    async fn stats(&self) -> (usize, Option<usize>) {
        let index = self.read().await;
        index.stats()
    }
}

impl VectorIndex {
    pub fn add(&mut self, photo_id: String, vector: Vec<f64>) {
        self.entries.insert(photo_id, vector);
    }

    pub fn remove(&mut self, photo_id: &str) {
        self.entries.remove(photo_id);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn search(&self, query: &[f64], top_k: usize, min_similarity: f64) -> Vec<(String, f64)> {
        if self.entries.is_empty() {
            return vec![];
        }

        let query_norm = normalize(query);
        let mut results: Vec<(String, f64)> = self
            .entries
            .iter()
            .filter_map(|(id, vec)| {
                // Skip entries with mismatched dimensions
                if vec.len() != query.len() {
                    return None;
                }
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

    pub fn stats(&self) -> (usize, Option<usize>) {
        let dim = self.entries.values().next().map(|v| v.len());
        (self.entries.len(), dim)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
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
