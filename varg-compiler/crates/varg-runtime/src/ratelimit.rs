// Wave 30: Rate Limiting — token bucket per key
// Used by @[RateLimit(calls: N, per: "second")] annotation

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_per_ms: f64,
    last_check: Instant,
}

impl TokenBucket {
    pub fn new(max_calls: u64, window_ms: u64) -> Self {
        let max = max_calls as f64;
        let refill = max / window_ms.max(1) as f64;
        TokenBucket { tokens: max, max_tokens: max, refill_per_ms: refill, last_check: Instant::now() }
    }

    fn refill(&mut self) {
        let elapsed = self.last_check.elapsed().as_millis() as f64;
        self.tokens = (self.tokens + elapsed * self.refill_per_ms).min(self.max_tokens);
        self.last_check = Instant::now();
    }

    pub fn try_acquire(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub fn acquire(&mut self) {
        loop {
            self.refill();
            if self.tokens >= 1.0 {
                self.tokens -= 1.0;
                return;
            }
            let wait_ms = ((1.0 - self.tokens) / self.refill_per_ms) as u64 + 1;
            std::thread::sleep(Duration::from_millis(wait_ms.min(100)));
        }
    }
}

static LIMITERS: OnceLock<Mutex<HashMap<String, TokenBucket>>> = OnceLock::new();

fn limiters() -> &'static Mutex<HashMap<String, TokenBucket>> {
    LIMITERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Acquire a rate-limit token, blocking until available.
/// key: unique identifier (e.g. "AgentName::MethodName")
pub fn __varg_rate_limit_acquire(key: &str, max_calls: u64, window_ms: u64) {
    limiters().lock().unwrap()
        .entry(key.to_string())
        .or_insert_with(|| TokenBucket::new(max_calls, window_ms))
        .acquire();
}

/// Non-blocking acquire. Returns false if limit exceeded.
pub fn __varg_rate_limit_try(key: &str, max_calls: u64, window_ms: u64) -> bool {
    limiters().lock().unwrap()
        .entry(key.to_string())
        .or_insert_with(|| TokenBucket::new(max_calls, window_ms))
        .try_acquire()
}

/// Reset a limiter (primarily for tests).
pub fn __varg_rate_limit_reset(key: &str) {
    limiters().lock().unwrap().remove(key);
}

// ── Varg builtins for explicit rate limiting ──────────────────────────────

fn nano_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// Create a named rate limiter handle (returns opaque key string).
pub fn __varg_ratelimiter_new(max_calls: i64, window_ms: i64) -> String {
    let key = format!("__rl_{}", nano_id());
    limiters().lock().unwrap()
        .insert(key.clone(), TokenBucket::new(max_calls as u64, window_ms as u64));
    key
}

/// Blocking acquire on a named limiter.
pub fn __varg_ratelimiter_acquire(key: &str) {
    if let Some(lim) = limiters().lock().unwrap().get_mut(key) {
        lim.acquire();
    }
}

/// Non-blocking acquire on a named limiter.
pub fn __varg_ratelimiter_try_acquire(key: &str) -> bool {
    limiters().lock().unwrap()
        .get_mut(key)
        .map(|lim| lim.try_acquire())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_full_at_start() {
        let b = TokenBucket::new(5, 1000);
        assert_eq!(b.max_tokens, 5.0);
        assert_eq!(b.tokens, 5.0);
    }

    #[test]
    fn test_token_bucket_depletes() {
        let mut b = TokenBucket::new(3, 60_000);
        assert!(b.try_acquire());
        assert!(b.try_acquire());
        assert!(b.try_acquire());
        assert!(!b.try_acquire());
    }

    #[test]
    fn test_rate_limit_try_key() {
        let key = format!("test_try_{}", nano_id());
        assert!(__varg_rate_limit_try(&key, 2, 60_000));
        assert!(__varg_rate_limit_try(&key, 2, 60_000));
        assert!(!__varg_rate_limit_try(&key, 2, 60_000));
        __varg_rate_limit_reset(&key);
    }

    #[test]
    fn test_ratelimiter_new_and_try() {
        let key = __varg_ratelimiter_new(3, 60_000);
        assert!(__varg_ratelimiter_try_acquire(&key));
        assert!(__varg_ratelimiter_try_acquire(&key));
        assert!(__varg_ratelimiter_try_acquire(&key));
        assert!(!__varg_ratelimiter_try_acquire(&key));
    }

    #[test]
    fn test_rate_limit_reset_restores() {
        let key = format!("test_reset_{}", nano_id());
        __varg_rate_limit_try(&key, 1, 60_000);
        __varg_rate_limit_try(&key, 1, 60_000); // depleted
        __varg_rate_limit_reset(&key);
        assert!(__varg_rate_limit_try(&key, 1, 60_000)); // fresh again
        __varg_rate_limit_reset(&key);
    }

    // ── Adversarial / edge-case tests ────────────────────────────────────────

    #[test]
    fn test_token_bucket_zero_window_does_not_panic() {
        // window_ms=0 → divides by max(1)=1, must not panic or divide-by-zero
        let mut b = TokenBucket::new(5, 0);
        assert!(b.try_acquire()); // bucket starts full
    }

    #[test]
    fn test_token_bucket_zero_max_calls_always_denies() {
        // max_calls=0 → bucket starts empty and refills at 0/ms
        let mut b = TokenBucket::new(0, 1000);
        assert!(!b.try_acquire(), "0-capacity bucket must always deny");
    }

    #[test]
    fn test_rate_limit_burst_exhaustion_then_denied() {
        let key = format!("burst_{}", nano_id());
        // Exhaust burst of 3
        assert!(__varg_rate_limit_try(&key, 3, 60_000));
        assert!(__varg_rate_limit_try(&key, 3, 60_000));
        assert!(__varg_rate_limit_try(&key, 3, 60_000));
        assert!(!__varg_rate_limit_try(&key, 3, 60_000), "4th call must be denied after burst exhausted");
        __varg_rate_limit_reset(&key);
    }

    #[test]
    fn test_rate_limit_many_keys_are_independent() {
        let k1 = format!("ind_a_{}", nano_id());
        let k2 = format!("ind_b_{}", nano_id());
        // Exhaust k1 but k2 should still work
        for _ in 0..3 { __varg_rate_limit_try(&k1, 3, 60_000); }
        assert!(!__varg_rate_limit_try(&k1, 3, 60_000), "k1 must be exhausted");
        assert!(__varg_rate_limit_try(&k2, 3, 60_000), "k2 must be independent of k1");
        __varg_rate_limit_reset(&k1);
        __varg_rate_limit_reset(&k2);
    }

    #[test]
    fn test_rate_limit_reset_nonexistent_key_is_safe() {
        // Resetting a key that never existed must not panic
        __varg_rate_limit_reset("key_that_does_not_exist_xyz_abc_123");
    }

    #[test]
    fn test_ratelimiter_unknown_key_try_acquire_returns_false() {
        // Acquiring on a key that was never created must return false, not panic
        assert!(!__varg_ratelimiter_try_acquire("nonexistent_limiter_key_xyz"));
    }

    #[test]
    fn test_token_bucket_refills_after_depletion() {
        // With a very short window (1ms), tokens refill after sleeping
        let mut b = TokenBucket::new(10, 1); // 10 calls per 1ms
        for _ in 0..10 { b.try_acquire(); }
        assert!(!b.try_acquire(), "must be depleted");
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(b.try_acquire(), "must have refilled after sleep");
    }

    #[test]
    fn test_rate_limit_negative_window_is_coerced() {
        // Negative window_ms cast to u64 is huge; as i64 it wraps. The new() takes u64.
        // Test via the Varg builtin which passes i64: negative → cast to u64 = huge number
        // What we test: it doesn't panic and eventually allows a call
        let key = format!("negwin_{}", nano_id());
        // With a very large window (huge number from negative cast), tokens refill very slowly
        // but the bucket starts full so first calls should succeed
        let b = TokenBucket::new(5, u64::MAX);
        assert_eq!(b.tokens, 5.0);
        __varg_rate_limit_reset(&key);
    }

    #[test]
    fn test_ratelimiter_new_creates_unique_keys() {
        let k1 = __varg_ratelimiter_new(5, 60_000);
        let k2 = __varg_ratelimiter_new(5, 60_000);
        assert_ne!(k1, k2, "each ratelimiter_new must generate a unique key");
        __varg_rate_limit_reset(&k1);
        __varg_rate_limit_reset(&k2);
    }
}
