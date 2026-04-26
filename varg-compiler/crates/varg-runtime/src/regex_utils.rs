use regex::Regex;

pub fn __varg_regex_match(pattern: &str, text: &str) -> Result<bool, String> {
    Regex::new(pattern)
        .map_err(|e| e.to_string())
        .map(|re| re.is_match(text))
}

pub fn __varg_regex_find_all(pattern: &str, text: &str) -> Result<Vec<String>, String> {
    Regex::new(pattern)
        .map_err(|e| e.to_string())
        .map(|re| re.find_iter(text).map(|m| m.as_str().to_string()).collect())
}

pub fn __varg_regex_replace(pattern: &str, text: &str, replacement: &str) -> Result<String, String> {
    Regex::new(pattern)
        .map_err(|e| e.to_string())
        .map(|re| re.replace_all(text, replacement).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_match_digits() {
        assert_eq!(__varg_regex_match("\\d+", "abc123").unwrap(), true);
        assert_eq!(__varg_regex_match("\\d+", "abc").unwrap(), false);
    }

    #[test]
    fn test_regex_match_invalid_pattern() {
        assert!(__varg_regex_match("[invalid", "text").is_err());
    }

    #[test]
    fn test_regex_find_all_words() {
        let words = __varg_regex_find_all("\\w+", "hello world foo").unwrap();
        assert_eq!(words, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn test_regex_find_all_no_matches() {
        let words = __varg_regex_find_all("\\d+", "no numbers here").unwrap();
        assert!(words.is_empty());
    }

    #[test]
    fn test_regex_replace_whitespace() {
        let result = __varg_regex_replace("\\s+", "hello   world", " ").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_regex_replace_no_match() {
        let result = __varg_regex_replace("xyz", "hello world", "!").unwrap();
        assert_eq!(result, "hello world");
    }
}
