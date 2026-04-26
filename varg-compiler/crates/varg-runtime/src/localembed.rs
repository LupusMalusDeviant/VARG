// Wave 40: Pure-Rust local embeddings — character n-gram hashing projection
//
// Produces 384-dimensional unit vectors compatible with the existing VectorStore.
// No API key, no downloads, no LlmAccess token required.
// Algorithm: extract UTF-8 character bigrams + trigrams, hash each with FNV-1a,
// accumulate into a float vector, then L2-normalise.

const DIM: usize = 384;

fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 14695981039346656037;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

pub fn __varg_embed_local(text: &str) -> Vec<f32> {
    let mut vec = vec![0.0f32; DIM];
    let lower = text.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();

    // Unigrams (word tokens)
    for word in lower.split_whitespace() {
        let h = fnv1a(word);
        vec[(h as usize) % DIM] += 1.5; // weighted higher
    }

    // Character bigrams and trigrams
    for n in [2usize, 3] {
        for window in chars.windows(n) {
            let s: String = window.iter().collect();
            let h = fnv1a(&s);
            vec[(h as usize) % DIM] += 1.0;
        }
    }

    // L2 normalise
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-9 {
        for x in &mut vec {
            *x /= norm;
        }
    }
    vec
}

pub fn __varg_embed_local_batch(texts: &[String]) -> Vec<Vec<f32>> {
    texts.iter().map(|t| __varg_embed_local(t)).collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    #[test]
    fn test_embed_local_dimension() {
        let v = __varg_embed_local("hello world");
        assert_eq!(v.len(), DIM);
    }

    #[test]
    fn test_embed_local_unit_vector() {
        let v = __varg_embed_local("the quick brown fox");
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4, "expected unit vector, got norm={}", norm);
    }

    #[test]
    fn test_embed_local_deterministic() {
        let a = __varg_embed_local("some text here");
        let b = __varg_embed_local("some text here");
        assert_eq!(a, b);
    }

    #[test]
    fn test_embed_local_different_texts_differ() {
        let a = __varg_embed_local("cat");
        let b = __varg_embed_local("quantum mechanics");
        let sim = cosine_sim(&a, &b);
        assert!(sim < 0.9, "unrelated texts should not be too similar, got {}", sim);
    }

    #[test]
    fn test_embed_local_similar_texts_more_similar() {
        let a = __varg_embed_local("dog puppy canine");
        let b = __varg_embed_local("cat kitten feline");
        let c = __varg_embed_local("quantum mechanics particle physics");
        let sim_ab = cosine_sim(&a, &b);
        let sim_ac = cosine_sim(&a, &c);
        // dog/cat both short animal words — should be more similar than dog/physics
        assert!(sim_ab > sim_ac, "similar-domain texts should score higher: {} vs {}", sim_ab, sim_ac);
    }

    #[test]
    fn test_embed_local_empty_text() {
        let v = __varg_embed_local("");
        assert_eq!(v.len(), DIM);
        // all zeros (norm guard prevents div by zero)
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(norm < 1e-9);
    }

    #[test]
    fn test_embed_local_batch_count() {
        let texts = vec![
            "hello".to_string(),
            "world".to_string(),
            "varg".to_string(),
            "agents".to_string(),
            "rust".to_string(),
        ];
        let batch = __varg_embed_local_batch(&texts);
        assert_eq!(batch.len(), 5);
    }

    #[test]
    fn test_embed_local_batch_matches_single() {
        let texts = vec!["foo bar".to_string(), "baz qux".to_string()];
        let batch = __varg_embed_local_batch(&texts);
        for (i, text) in texts.iter().enumerate() {
            assert_eq!(batch[i], __varg_embed_local(text));
        }
    }
}
