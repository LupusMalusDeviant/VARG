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
}
