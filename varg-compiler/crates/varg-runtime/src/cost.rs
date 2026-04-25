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
}
