// Wave 31: LLM Cost Tracking and Budget Guards
// @[Budget(tokens: N, usd: F)] annotation support

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct BudgetTracker {
    pub max_tokens: u64,
    pub max_usd: f64,
    pub used_tokens: u64,
    pub used_usd: f64,
    provider: String,
}

impl BudgetTracker {
    pub fn new(max_tokens: u64, max_usd: f64) -> Self {
        let provider = std::env::var("VARG_LLM_PROVIDER")
            .unwrap_or_else(|_| "ollama".to_string())
            .to_lowercase();
        BudgetTracker { max_tokens, max_usd, used_tokens: 0, used_usd: 0.0, provider }
    }

    /// Heuristic: 1 token ≈ 4 characters
    pub fn estimate_tokens(text: &str) -> u64 {
        (text.chars().count() / 4).max(1) as u64
    }

    fn usd_per_1k_tokens(&self) -> f64 {
        match self.provider.as_str() {
            "openai"              => 0.005,  // GPT-4o input avg
            "anthropic" | "claude" => 0.003, // Claude Sonnet
            _                     => 0.0,   // Ollama: free
        }
    }

    /// Record a prompt+response exchange. Returns Err if any limit is exceeded.
    pub fn track(&mut self, prompt: &str, response: &str) -> Result<(), String> {
        let tokens = Self::estimate_tokens(prompt) + Self::estimate_tokens(response);
        let cost = (tokens as f64 / 1000.0) * self.usd_per_1k_tokens();

        if self.used_tokens + tokens > self.max_tokens {
            return Err(format!(
                "Token budget exceeded: {}/{} used",
                self.used_tokens, self.max_tokens
            ));
        }
        if self.used_usd + cost > self.max_usd {
            return Err(format!(
                "USD budget exceeded: ${:.4}/${:.4} used",
                self.used_usd, self.max_usd
            ));
        }
        self.used_tokens += tokens;
        self.used_usd += cost;
        Ok(())
    }

    pub fn check(&self) -> Result<(), String> {
        if self.used_tokens >= self.max_tokens {
            return Err(format!("Token budget exhausted ({} tokens)", self.max_tokens));
        }
        if self.used_usd >= self.max_usd {
            return Err(format!("USD budget exhausted (${:.4})", self.max_usd));
        }
        Ok(())
    }

    pub fn remaining_tokens(&self) -> u64 {
        self.max_tokens.saturating_sub(self.used_tokens)
    }

    pub fn remaining_usd(&self) -> f64 {
        (self.max_usd - self.used_usd).max(0.0)
    }

    pub fn report(&self) -> String {
        let tok_pct = if self.max_tokens > 0 {
            100.0 * self.used_tokens as f64 / self.max_tokens as f64
        } else { 0.0 };
        let usd_pct = if self.max_usd > 0.0 {
            100.0 * self.used_usd / self.max_usd
        } else { 0.0 };
        format!(
            "Tokens: {}/{} ({:.1}%) | USD: ${:.4}/${:.4} ({:.1}%)",
            self.used_tokens, self.max_tokens, tok_pct,
            self.used_usd, self.max_usd, usd_pct
        )
    }
}

pub type BudgetHandle = Arc<Mutex<BudgetTracker>>;

pub fn __varg_budget_new(max_tokens: i64, max_usd_cents: i64) -> BudgetHandle {
    Arc::new(Mutex::new(BudgetTracker::new(
        max_tokens as u64,
        max_usd_cents as f64 / 100.0,
    )))
}

pub fn __varg_budget_track(h: &BudgetHandle, prompt: &str, response: &str) -> bool {
    h.lock().unwrap().track(prompt, response).is_ok()
}

pub fn __varg_budget_check(h: &BudgetHandle) -> bool {
    h.lock().unwrap().check().is_ok()
}

pub fn __varg_budget_remaining_tokens(h: &BudgetHandle) -> i64 {
    h.lock().unwrap().remaining_tokens() as i64
}

pub fn __varg_budget_remaining_usd_cents(h: &BudgetHandle) -> i64 {
    (h.lock().unwrap().remaining_usd() * 100.0) as i64
}

pub fn __varg_budget_report(h: &BudgetHandle) -> String {
    h.lock().unwrap().report()
}

pub fn __varg_estimate_tokens(text: &str) -> i64 {
    BudgetTracker::estimate_tokens(text) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_new_defaults() {
        let b = BudgetTracker::new(10_000, 5.0);
        assert_eq!(b.max_tokens, 10_000);
        assert_eq!(b.used_tokens, 0);
        assert_eq!(b.remaining_tokens(), 10_000);
    }

    #[test]
    fn test_estimate_tokens_heuristic() {
        let t = BudgetTracker::estimate_tokens("Hello world"); // 11 chars → 2-3 tokens
        assert!(t >= 1 && t <= 5);
    }

    #[test]
    fn test_track_within_limits() {
        let mut b = BudgetTracker::new(100_000, 100.0);
        assert!(b.track("Hello", "World").is_ok());
        assert!(b.used_tokens > 0);
    }

    #[test]
    fn test_track_exceeds_token_limit() {
        let mut b = BudgetTracker::new(1, 100.0);
        assert!(b.track("This is a longer prompt that exceeds 1 token", "response").is_err());
    }

    #[test]
    fn test_remaining_decreases_after_track() {
        let mut b = BudgetTracker::new(1000, 100.0);
        let before = b.remaining_tokens();
        let _ = b.track("Hello", "World");
        assert!(b.remaining_tokens() < before);
    }

    #[test]
    fn test_budget_report_format() {
        let mut b = BudgetTracker::new(1000, 10.0);
        let _ = b.track("Hello", "World");
        let r = b.report();
        assert!(r.contains("Tokens:"));
        assert!(r.contains("USD:"));
    }

    #[test]
    fn test_budget_handle_check() {
        let h = __varg_budget_new(10_000, 500);
        assert!(__varg_budget_check(&h));
        assert_eq!(__varg_budget_remaining_tokens(&h), 10_000);
    }

    #[test]
    fn test_budget_handle_track() {
        let h = __varg_budget_new(10_000, 500);
        assert!(__varg_budget_track(&h, "Hello", "World"));
        assert!(__varg_budget_remaining_tokens(&h) < 10_000);
    }

    #[test]
    fn test_estimate_tokens_builtin() {
        assert!(__varg_estimate_tokens("Hello world") >= 1);
    }

    // ── Adversarial / edge-case tests ────────────────────────────────────────

    #[test]
    fn test_budget_zero_limits_immediate_exhaustion() {
        // max_tokens=0 → any track() with tokens>=1 must fail
        let mut b = BudgetTracker::new(0, 0.0);
        let result = b.track("hi", "hi");
        assert!(result.is_err(), "zero-budget must reject every track call");
    }

    #[test]
    fn test_budget_empty_prompt_and_response_uses_minimum_one_token() {
        // estimate_tokens has .max(1) so even "" costs 1 token
        let tokens = BudgetTracker::estimate_tokens("");
        assert_eq!(tokens, 1, "empty string must estimate to 1 token (max(1) guard)");
    }

    #[test]
    fn test_budget_track_empty_strings_succeeds_with_sufficient_budget() {
        let mut b = BudgetTracker::new(100, 100.0);
        assert!(b.track("", "").is_ok(), "tracking empty strings must succeed with large budget");
        assert!(b.used_tokens >= 1, "tracking empty strings must still consume tokens");
    }

    #[test]
    fn test_budget_remaining_tokens_saturating_no_underflow() {
        // used_tokens must never exceed max_tokens due to the check in track()
        let mut b = BudgetTracker::new(5, 100.0);
        for _ in 0..10 {
            let _ = b.track("x", "x"); // some will fail
        }
        // remaining must not underflow (saturating_sub)
        assert!(b.remaining_tokens() <= 5);
    }

    #[test]
    fn test_budget_check_fails_at_exact_exhaustion() {
        let mut b = BudgetTracker::new(2, 100.0);
        // Force used_tokens == max_tokens manually
        b.used_tokens = 2;
        assert!(b.check().is_err(), "check must fail when used == max");
    }

    #[test]
    fn test_budget_remaining_never_negative() {
        let mut b = BudgetTracker::new(10, 1.0);
        b.used_tokens = 10;
        assert_eq!(b.remaining_tokens(), 0);
        b.used_tokens = 15; // hypothetically over (shouldn't happen via track(), but test the guard)
        let remaining = b.remaining_tokens();
        assert_eq!(remaining, 0, "saturating_sub must clamp to 0, got {remaining}");
    }

    #[test]
    fn test_budget_remaining_usd_never_negative() {
        let mut b = BudgetTracker::new(100_000, 0.0001);
        b.used_usd = 1.0; // way over the limit
        let rem = b.remaining_usd();
        assert!(rem >= 0.0, "remaining_usd must never go negative, got {rem}");
    }

    #[test]
    fn test_budget_usd_limit_enforced() {
        // Set up: tiny USD budget with Ollama (free) → only token limit is active
        // Test with openai provider via env var workaround: just check the math directly
        let mut b = BudgetTracker {
            max_tokens: 1_000_000,
            max_usd: 0.0001, // $0.0001
            used_tokens: 0,
            used_usd: 0.0,
            provider: "openai".to_string(),
        };
        // OpenAI rate: $0.005/1k tokens, so 20 tokens cost $0.0001
        // A small prompt should stay under; a large one should bust the limit
        let large_prompt = "word ".repeat(10000); // ~50000 chars → ~12500 tokens → ~$0.0625
        let result = b.track(&large_prompt, "ok");
        assert!(result.is_err(), "large prompt must exceed tiny USD budget");
    }

    #[test]
    fn test_budget_report_zero_budget_no_nan_or_panic() {
        let b = BudgetTracker::new(0, 0.0);
        let report = b.report();
        // With max_tokens=0, percentage calc would be 0/0 — must not produce NaN
        assert!(!report.contains("NaN"), "report with zero budget must not contain NaN: {report}");
        assert!(!report.contains("inf"), "report with zero budget must not contain inf: {report}");
    }

    #[test]
    fn test_budget_handle_exhausted_track_returns_false() {
        // estimate_tokens("") = 1, so track("","") costs 2 tokens (prompt + response)
        // max_tokens=2: first call succeeds (0+2=2 <= 2), second call fails (2+2=4 > 2)
        let h = __varg_budget_new(2, 10000);
        assert!(__varg_budget_track(&h, "", ""), "first track must succeed (uses exactly 2 tokens)");
        assert!(!__varg_budget_track(&h, "", ""), "second track must fail (budget exhausted)");
    }

    #[test]
    fn test_budget_check_after_exhaustion_returns_false() {
        // Use 2 tokens of a 2-token budget
        let h = __varg_budget_new(2, 10000);
        __varg_budget_track(&h, "", ""); // consume 2 tokens → exhausted (used=2, max=2)
        assert!(!__varg_budget_check(&h), "check must return false when used >= max");
    }

    #[test]
    fn test_estimate_tokens_long_text() {
        let text = "a".repeat(10_000); // 10k chars → ~2500 tokens
        let t = BudgetTracker::estimate_tokens(&text);
        assert!(t >= 2000 && t <= 3000, "10k chars should estimate ~2500 tokens, got {t}");
    }

    #[test]
    fn test_estimate_tokens_unicode_uses_char_count_not_bytes() {
        // "é" is 2 bytes but 1 char — estimate should use chars(), not len()
        let text = "éàü"; // 3 chars → estimate_tokens = max(3/4, 1) = 1
        let t = BudgetTracker::estimate_tokens(text);
        assert_eq!(t, 1);
        // Multi-byte chars should not inflate the count
        let kanji = "漢字テスト"; // 5 chars
        let t2 = BudgetTracker::estimate_tokens(kanji);
        assert_eq!(t2, 1, "5 unicode chars / 4 = 1 (max 1)");
    }
}
