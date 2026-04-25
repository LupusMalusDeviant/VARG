// Wave 33: Property-Based Testing
// @[Property] annotation support — randomized input generation + assertion

use std::cell::Cell;
use std::collections::HashMap;

// ── LCG random (no external deps) ────────────────────────────────────────

fn lcg_next() -> u64 {
    thread_local! {
        static STATE: Cell<u64> = Cell::new(
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64
                | 1
        );
    }
    STATE.with(|s| {
        let v = s.get().wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s.set(v);
        v
    })
}

// ── Generators ────────────────────────────────────────────────────────────

/// Random i64 in [min, max).
pub fn __varg_prop_gen_int(min: i64, max: i64) -> i64 {
    if min >= max { return min; }
    let range = (max - min) as u64;
    min + (lcg_next() % range) as i64
}

/// Random f64 in [0.0, 1.0).
pub fn __varg_prop_gen_float() -> f64 {
    lcg_next() as f64 / u64::MAX as f64
}

/// Random bool.
pub fn __varg_prop_gen_bool() -> bool {
    lcg_next() % 2 == 0
}

/// Random ASCII string of length in [1, max_len].
pub fn __varg_prop_gen_string(max_len: i64) -> String {
    let alphabet = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 _-";
    let len = ((lcg_next() % max_len.max(1) as u64) + 1) as usize;
    (0..len).map(|_| alphabet[(lcg_next() as usize) % alphabet.len()] as char).collect()
}

/// Random Vec<i64> with length in [0, max_len].
pub fn __varg_prop_gen_int_list(max_len: i64) -> Vec<i64> {
    let len = (lcg_next() % (max_len.max(0) as u64 + 1)) as usize;
    (0..len).map(|_| lcg_next() as i64).collect()
}

/// Random Vec<String>.
pub fn __varg_prop_gen_string_list(max_len: i64, max_str_len: i64) -> Vec<String> {
    let len = (lcg_next() % (max_len.max(0) as u64 + 1)) as usize;
    (0..len).map(|_| __varg_prop_gen_string(max_str_len)).collect()
}

// ── Runners ───────────────────────────────────────────────────────────────

/// Run `test_fn` `runs` times. Returns {runs, failures, ok}.
pub fn __varg_prop_check(test_fn: impl Fn() -> bool, runs: i64) -> HashMap<String, i64> {
    let runs = runs.max(1);
    let mut failures = 0i64;
    for _ in 0..runs {
        if !test_fn() { failures += 1; }
    }
    let mut r = HashMap::new();
    r.insert("runs".into(), runs);
    r.insert("failures".into(), failures);
    r.insert("ok".into(), if failures == 0 { 1 } else { 0 });
    r
}

/// Assert a property holds for `runs` random trials; panics on failure.
pub fn __varg_prop_assert(label: &str, test_fn: impl Fn() -> bool, runs: i64) {
    for i in 0..runs.max(1) {
        if !test_fn() {
            panic!("[Property '{}'] failed on trial {}/{}", label, i + 1, runs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_int_in_range() {
        for _ in 0..200 {
            let v = __varg_prop_gen_int(10, 20);
            assert!((10..20).contains(&v), "{v} not in [10,20)");
        }
    }

    #[test]
    fn test_gen_float_in_range() {
        for _ in 0..200 {
            let v = __varg_prop_gen_float();
            assert!((0.0..1.0).contains(&v), "{v} not in [0,1)");
        }
    }

    #[test]
    fn test_gen_string_max_len() {
        for _ in 0..100 {
            let s = __varg_prop_gen_string(8);
            assert!(!s.is_empty() && s.len() <= 8, "len={}", s.len());
        }
    }

    #[test]
    fn test_gen_int_list_max_len() {
        for _ in 0..50 {
            let v = __varg_prop_gen_int_list(5);
            assert!(v.len() <= 5);
        }
    }

    #[test]
    fn test_prop_check_always_true() {
        let r = __varg_prop_check(|| true, 100);
        assert_eq!(r["failures"], 0);
        assert_eq!(r["runs"], 100);
        assert_eq!(r["ok"], 1);
    }

    #[test]
    fn test_prop_check_always_false() {
        let r = __varg_prop_check(|| false, 10);
        assert_eq!(r["failures"], 10);
        assert_eq!(r["ok"], 0);
    }

    #[test]
    fn test_prop_assert_passes() {
        __varg_prop_assert("x>=0", || __varg_prop_gen_int(0, 100) >= 0, 100);
    }

    #[test]
    #[should_panic(expected = "Property 'always_false'")]
    fn test_prop_assert_fails() {
        __varg_prop_assert("always_false", || false, 1);
    }

    // ── Adversarial / edge-case tests ────────────────────────────────────────

    #[test]
    fn test_gen_int_min_equals_max_returns_min() {
        // When min==max there is no valid range — must return min without panic
        for _ in 0..50 {
            assert_eq!(__varg_prop_gen_int(7, 7), 7);
        }
    }

    #[test]
    fn test_gen_int_min_greater_than_max_returns_min() {
        // Inverted range: min > max — must return min, not panic or go out of range
        for _ in 0..50 {
            assert_eq!(__varg_prop_gen_int(100, 50), 100);
        }
    }

    #[test]
    fn test_gen_int_negative_range() {
        // Range entirely in negatives must still produce values in range
        for _ in 0..200 {
            let v = __varg_prop_gen_int(-100, -10);
            assert!((-100..-10).contains(&v), "{v} not in [-100,-10)");
        }
    }

    #[test]
    fn test_gen_int_extreme_range() {
        // Very large range including i64 boundaries must not panic
        for _ in 0..50 {
            let v = __varg_prop_gen_int(0, i64::MAX);
            assert!(v >= 0);
        }
    }

    #[test]
    fn test_gen_string_zero_max_len_produces_non_empty() {
        // max_len=0 → max(1) guard → should still produce a 1-char string
        for _ in 0..50 {
            let s = __varg_prop_gen_string(0);
            assert!(!s.is_empty(), "max_len=0 must still produce at least 1 char");
            assert!(s.len() <= 1, "max_len=0 coerced to 1 must not exceed length 1");
        }
    }

    #[test]
    fn test_gen_string_negative_max_len_treated_as_zero() {
        // max(-50).max(1) = 1 → 1-char string
        for _ in 0..50 {
            let s = __varg_prop_gen_string(-50);
            assert!(!s.is_empty());
        }
    }

    #[test]
    fn test_gen_int_list_zero_max_len_may_produce_empty() {
        // max_len=0 → length is 0 or 0, so empty list
        for _ in 0..20 {
            let v = __varg_prop_gen_int_list(0);
            assert!(v.is_empty(), "max_len=0 must produce empty list, got len={}", v.len());
        }
    }

    #[test]
    fn test_gen_int_list_negative_max_len_produces_empty() {
        for _ in 0..20 {
            let v = __varg_prop_gen_int_list(-10);
            assert!(v.is_empty());
        }
    }

    #[test]
    fn test_prop_check_zero_runs_clamps_to_one() {
        // runs=0 is clamped to 1 via runs.max(1)
        let r = __varg_prop_check(|| true, 0);
        assert_eq!(r["runs"], 1, "zero runs must be clamped to 1");
    }

    #[test]
    fn test_prop_check_negative_runs_clamps_to_one() {
        let r = __varg_prop_check(|| true, -100);
        assert_eq!(r["runs"], 1);
    }

    #[test]
    fn test_prop_assert_zero_runs_clamps_to_one_and_passes() {
        // Should not panic with zero runs (clamped to 1, always-true fn)
        __varg_prop_assert("zero_runs", || true, 0);
    }

    #[test]
    fn test_prop_check_partial_failures_counted() {
        let call_count = std::sync::Arc::new(std::sync::Mutex::new(0i64));
        let cc = call_count.clone();
        let r = __varg_prop_check(move || {
            let mut n = cc.lock().unwrap();
            *n += 1;
            *n % 2 == 0 // fails on odd calls
        }, 10);
        assert_eq!(r["runs"], 10);
        assert_eq!(r["failures"], 5, "exactly half should fail");
        assert_eq!(r["ok"], 0, "any failure means ok=0");
    }

    #[test]
    fn test_gen_bool_produces_both_values() {
        let mut saw_true = false;
        let mut saw_false = false;
        for _ in 0..200 {
            let b = __varg_prop_gen_bool();
            if b { saw_true = true; } else { saw_false = true; }
            if saw_true && saw_false { break; }
        }
        assert!(saw_true, "gen_bool must produce true at least once in 200 calls");
        assert!(saw_false, "gen_bool must produce false at least once in 200 calls");
    }

    #[test]
    fn test_gen_float_never_exactly_one() {
        // LCG produces values in [0, u64::MAX), so / u64::MAX is always < 1.0
        for _ in 0..500 {
            let f = __varg_prop_gen_float();
            assert!(f < 1.0, "gen_float must be < 1.0, got {f}");
            assert!(f >= 0.0, "gen_float must be >= 0.0, got {f}");
        }
    }

    #[test]
    fn test_gen_string_list_zero_max_produces_only_empty_lists() {
        for _ in 0..30 {
            let v = __varg_prop_gen_string_list(0, 10);
            assert!(v.is_empty(), "max_len=0 must produce empty list");
        }
    }

    #[test]
    fn test_gen_string_only_uses_ascii_alphabet() {
        for _ in 0..200 {
            let s = __varg_prop_gen_string(50);
            for c in s.chars() {
                assert!(c.is_ascii(), "gen_string must only produce ASCII chars, got '{c}'");
            }
        }
    }
}
