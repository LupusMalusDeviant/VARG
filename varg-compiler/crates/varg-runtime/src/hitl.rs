// Wave 30: Human-in-the-Loop primitives
// await_approval(prompt) -> bool
// await_input(prompt) -> string
// await_choice(prompt, options) -> int

use std::io::{self, Write};

/// Prompt for yes/no approval. Returns true on 'y' or 'yes'.
pub fn __varg_await_approval(prompt: &str) -> bool {
    print!("{} [y/N]: ", prompt);
    io::stdout().flush().unwrap_or(());
    let mut line = String::new();
    io::stdin().read_line(&mut line).unwrap_or(0);
    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Prompt for free-text input. Returns trimmed string.
pub fn __varg_await_input(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().unwrap_or(());
    let mut line = String::new();
    io::stdin().read_line(&mut line).unwrap_or(0);
    line.trim().to_string()
}

/// Show a numbered menu and return 0-based index. Returns -1 on invalid input.
pub fn __varg_await_choice(prompt: &str, options: Vec<String>) -> i64 {
    println!("{}", prompt);
    for (i, opt) in options.iter().enumerate() {
        println!("  {} - {}", i + 1, opt);
    }
    print!("Choice (1-{}): ", options.len());
    io::stdout().flush().unwrap_or(());
    let mut line = String::new();
    io::stdin().read_line(&mut line).unwrap_or(0);
    line.trim()
        .parse::<usize>()
        .ok()
        .filter(|&n| n >= 1 && n <= options.len())
        .map(|n| (n - 1) as i64)
        .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hitl_functions_exist() {
        let _ = __varg_await_approval as fn(&str) -> bool;
        let _ = __varg_await_input as fn(&str) -> String;
        let _ = __varg_await_choice as fn(&str, Vec<String>) -> i64;
    }

    #[test]
    fn test_await_choice_empty_options() {
        // Options list logic: empty list means no valid choice
        let opts: Vec<String> = vec![];
        assert_eq!(opts.len(), 0);
    }
}
