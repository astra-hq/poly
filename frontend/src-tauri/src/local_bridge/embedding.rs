use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// BGE-M3 embedding dimension (1024) — this is the canonical size.
pub const BGE_M3_DIM: usize = 1024;

/// Lightweight deterministic embedding engine using character n-gram hashing.
///
/// Produces stable, normalized vectors without requiring a model file.
/// Designed as a drop-in that can be replaced with ONNX-based inference
/// once a BGE-M3 ONNX model is available.
pub struct EmbeddingEngine {
    model_name: String,
    dimension: usize,
}

impl EmbeddingEngine {
    pub fn new(model_name: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
            dimension: BGE_M3_DIM,
        }
    }

    pub fn with_dimension(model_name: &str, dim: usize) -> Self {
        Self {
            model_name: model_name.to_string(),
            dimension: dim,
        }
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    pub fn dimensions(&self) -> usize {
        self.dimension
    }

    pub fn is_ready(&self) -> bool {
        true
    }

    /// Embed a batch of texts and return normalized vectors of `self.dimension`.
    pub fn embed(&self, texts: &[String]) -> Vec<Vec<f32>> {
        texts
            .iter()
            .map(|t| embed_text(t, self.dimension))
            .collect()
    }
}

/// Embed a single text into a normalized vector of dimension `dim`.
fn embed_text(text: &str, dim: usize) -> Vec<f32> {
    let lower = text.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let mut vec = vec![0.0f32; dim];

    // Character n-grams from unigram up to 4-gram for richer representation
    for n in 1..=4 {
        if chars.len() < n {
            break;
        }
        for window in chars.windows(n) {
            let seed = hash_window(window);
            accumulate(&mut vec, seed);
        }
    }

    // L2 normalise so dot-product / cosine-similarity is meaningful
    normalize_l2(&mut vec);
    vec
}

/// Hash a character window into a 64-bit seed.
fn hash_window(window: &[char]) -> u64 {
    let mut hasher = DefaultHasher::new();
    window.hash(&mut hasher);
    hasher.finish()
}

/// Add a random-ish unit vector derived from `seed` into `vec`.
///
/// Uses a splitmix64 variant to generate per-dimension values in [-1, 1].
fn accumulate(vec: &mut [f32], seed: u64) {
    let dim = vec.len();
    let mut state = seed;

    for i in 0..dim {
        state = splitmix64(state);
        // Map u64 to [-1.0, 1.0)
        let val = (state as f64 / (u64::MAX as f64)) as f32 * 2.0 - 1.0;
        vec[i] += val;
    }
}

/// L2-normalize a vector in-place. Zero vectors are left as-is.
fn normalize_l2(vec: &mut [f32]) {
    let sum_sq: f32 = vec.iter().map(|x| x * x).sum();
    if sum_sq > 0.0 {
        let inv = 1.0 / sum_sq.sqrt();
        for x in vec.iter_mut() {
            *x *= inv;
        }
    }
}

/// SplitMix64 — fast, high-quality 64-bit PRNG with good distribution.
fn splitmix64(state: u64) -> u64 {
    let mut z = state.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_match_bge_m3() {
        let engine = EmbeddingEngine::new("bge-m3");
        assert_eq!(engine.dimensions(), 1024);
    }

    #[test]
    fn embed_returns_normalized_vectors() {
        let engine = EmbeddingEngine::new("bge-m3");
        let texts = vec!["hello world".to_string()];
        let results = engine.embed(&texts);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].len(), 1024);

        let norm: f32 = results[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.001,
            "vector should be L2-normalized, got norm {}",
            norm
        );
    }

    #[test]
    fn deterministic_same_input_same_output() {
        let engine = EmbeddingEngine::new("bge-m3");
        let r1 = engine.embed(&["test text".to_string()]);
        let r2 = engine.embed(&["test text".to_string()]);
        assert_eq!(r1[0], r2[0], "same input should produce identical vectors");
    }

    #[test]
    fn different_inputs_produce_different_vectors() {
        let engine = EmbeddingEngine::new("bge-m3");
        let r1 = engine.embed(&["hello".to_string()]);
        let r2 = engine.embed(&["world".to_string()]);
        assert_ne!(
            r1[0], r2[0],
            "different inputs should produce different vectors"
        );
    }

    #[test]
    fn batch_embed_preserves_ordering() {
        let engine = EmbeddingEngine::new("bge-m3");
        let results = engine.embed(&["a".to_string(), "b".to_string(), "c".to_string()]);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], engine.embed(&["a".to_string()])[0]);
        assert_eq!(results[1], engine.embed(&["b".to_string()])[0]);
    }

    #[test]
    fn empty_string_produces_valid_vector() {
        let engine = EmbeddingEngine::new("bge-m3");
        let results = engine.embed(&["".to_string()]);
        assert_eq!(results[0].len(), 1024);
        // Zero vector stays zero (no n-grams to hash)
        let norm: f32 = results[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            norm < 0.001,
            "empty text should produce near-zero vector, got norm {}",
            norm
        );
    }

    #[test]
    fn case_insensitive() {
        let engine = EmbeddingEngine::new("bge-m3");
        let r1 = engine.embed(&["Hello World".to_string()]);
        let r2 = engine.embed(&["hello world".to_string()]);
        assert_eq!(r1[0], r2[0], "embedding should be case-insensitive");
    }
}
