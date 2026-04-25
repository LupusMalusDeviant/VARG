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
}
