use tower_lsp::lsp_types::*;

/// Provide keyword, type, and OCAP token completions.
pub fn compute_completions(source: &str, pos: Position) -> Option<CompletionResponse> {
    // Get the word being typed at cursor position
    let prefix = get_word_at_position(source, pos);

    let mut items = Vec::new();

    // Keywords
    let keywords = [
        ("agent", "Declare a new agent", CompletionItemKind::KEYWORD),
        ("contract", "Declare a contract (interface)", CompletionItemKind::KEYWORD),
        ("public", "Public visibility modifier", CompletionItemKind::KEYWORD),
        ("system", "System-level agent modifier", CompletionItemKind::KEYWORD),
        ("var", "Declare a mutable variable", CompletionItemKind::KEYWORD),
        ("return", "Return from method", CompletionItemKind::KEYWORD),
        ("if", "Conditional branch", CompletionItemKind::KEYWORD),
        ("else", "Alternative branch", CompletionItemKind::KEYWORD),
        ("while", "While loop", CompletionItemKind::KEYWORD),
        ("for", "C-style for loop", CompletionItemKind::KEYWORD),
        ("foreach", "Iterate over collection", CompletionItemKind::KEYWORD),
        ("match", "Pattern matching", CompletionItemKind::KEYWORD),
        ("enum", "Algebraic data type", CompletionItemKind::KEYWORD),
        ("unsafe", "Unsafe block (requires OCAP)", CompletionItemKind::KEYWORD),
        ("try", "Error handling block", CompletionItemKind::KEYWORD),
        ("catch", "Catch errors from try block", CompletionItemKind::KEYWORD),
        ("throw", "Throw an error", CompletionItemKind::KEYWORD),
        ("print", "Print to stdout", CompletionItemKind::KEYWORD),
        ("stream", "Stream output incrementally", CompletionItemKind::KEYWORD),
        ("where", "Generic constraint clause", CompletionItemKind::KEYWORD),
        ("from", "LINQ from clause", CompletionItemKind::KEYWORD),
        ("select", "LINQ select clause", CompletionItemKind::KEYWORD),
    ];

    // Types
    let types = [
        ("int", "64-bit signed integer", CompletionItemKind::TYPE_PARAMETER),
        ("string", "UTF-8 string", CompletionItemKind::TYPE_PARAMETER),
        ("bool", "Boolean value", CompletionItemKind::TYPE_PARAMETER),
        ("void", "No return value", CompletionItemKind::TYPE_PARAMETER),
        ("ulong", "64-bit unsigned integer", CompletionItemKind::TYPE_PARAMETER),
        ("Prompt", "AI prompt type", CompletionItemKind::TYPE_PARAMETER),
        ("Context", "LLM conversation context", CompletionItemKind::TYPE_PARAMETER),
        ("Tensor", "Multi-dimensional array", CompletionItemKind::TYPE_PARAMETER),
        ("Embedding", "Vector embedding", CompletionItemKind::TYPE_PARAMETER),
    ];

    // OCAP capability tokens
    let capabilities = [
        ("NetworkAccess", "HTTP/network capability token", CompletionItemKind::CONSTANT),
        ("FileAccess", "Filesystem capability token", CompletionItemKind::CONSTANT),
        ("DbAccess", "Database capability token", CompletionItemKind::CONSTANT),
        ("LlmAccess", "LLM inference capability token", CompletionItemKind::CONSTANT),
        ("SystemAccess", "System operations capability token", CompletionItemKind::CONSTANT),
    ];

    // Built-in methods
    let methods = [
        ("fetch", "HTTP request (requires NetworkAccess)", CompletionItemKind::METHOD),
        ("llm_infer", "Single-turn LLM inference (requires LlmAccess)", CompletionItemKind::METHOD),
        ("llm_chat", "Multi-turn LLM chat (requires LlmAccess)", CompletionItemKind::METHOD),
        ("encrypt", "AES-256-GCM encryption", CompletionItemKind::METHOD),
        ("decrypt", "AES-256-GCM decryption", CompletionItemKind::METHOD),
        ("file_read", "Read file to string (requires FileAccess)", CompletionItemKind::METHOD),
        ("file_write", "Write string to file (requires FileAccess)", CompletionItemKind::METHOD),
        ("to_json", "Serialize to JSON string", CompletionItemKind::METHOD),
        ("from_json", "Deserialize from JSON string", CompletionItemKind::METHOD),
        ("context_from", "Create Context from prompt string", CompletionItemKind::METHOD),
        ("time_now", "Current UNIX timestamp", CompletionItemKind::METHOD),
        ("str_replace", "Replace substring", CompletionItemKind::METHOD),
        ("str_trim", "Trim whitespace", CompletionItemKind::METHOD),
        ("str_split", "Split string by delimiter", CompletionItemKind::METHOD),
    ];

    // Snippets
    let snippets = [
        ("agent", "agent ${1:Name} {\n    public void ${2:Run}() {\n        $0\n    }\n}", "Agent template", CompletionItemKind::SNIPPET),
        ("foreach", "foreach (var ${1:item} in ${2:collection}) {\n    $0\n}", "Foreach loop", CompletionItemKind::SNIPPET),
        ("trycatch", "try {\n    $1\n} catch (${2:err}) {\n    $0\n}", "Try/catch block", CompletionItemKind::SNIPPET),
        ("match", "match ${1:expr} {\n    ${2:pattern} => {\n        $0\n    }\n    _ => {}\n}", "Match expression", CompletionItemKind::SNIPPET),
    ];

    let prefix_lower = prefix.to_lowercase();

    for (label, detail, kind) in keywords.iter().chain(types.iter()).chain(capabilities.iter()).chain(methods.iter()) {
        if prefix.is_empty() || label.to_lowercase().starts_with(&prefix_lower) {
            items.push(CompletionItem {
                label: label.to_string(),
                kind: Some(*kind),
                detail: Some(detail.to_string()),
                ..Default::default()
            });
        }
    }

    for (label, insert_text, detail, kind) in &snippets {
        if prefix.is_empty() || label.to_lowercase().starts_with(&prefix_lower) {
            items.push(CompletionItem {
                label: label.to_string(),
                kind: Some(*kind),
                detail: Some(detail.to_string()),
                insert_text: Some(insert_text.to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
        }
    }

    if items.is_empty() {
        None
    } else {
        Some(CompletionResponse::Array(items))
    }
}

/// Extract the word being typed at the cursor position.
fn get_word_at_position(source: &str, pos: Position) -> String {
    let lines: Vec<&str> = source.lines().collect();
    if pos.line as usize >= lines.len() {
        return String::new();
    }

    let line = lines[pos.line as usize];
    let col = (pos.character as usize).min(line.len());

    // Walk backwards from cursor to find word start
    let before_cursor = &line[..col];
    let word_start = before_cursor
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);

    before_cursor[word_start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_with_prefix() {
        let source = "agent Test { public void Run() { pri } }";
        let completions = compute_completions(source, Position::new(0, 36));
        assert!(completions.is_some());
        let items = match completions.unwrap() {
            CompletionResponse::Array(items) => items,
            _ => panic!("Expected array"),
        };
        // Should suggest "print" matching "pri"
        assert!(items.iter().any(|i| i.label == "print"));
    }

    #[test]
    fn test_completion_empty_returns_all() {
        let source = "";
        let completions = compute_completions(source, Position::new(0, 0));
        assert!(completions.is_some());
        let items = match completions.unwrap() {
            CompletionResponse::Array(items) => items,
            _ => panic!("Expected array"),
        };
        // Should return all keywords + types + methods + snippets
        assert!(items.len() > 30);
    }

    #[test]
    fn test_get_word_at_position() {
        let source = "var myVar = 42;";
        assert_eq!(get_word_at_position(source, Position::new(0, 3)), "var");
        assert_eq!(get_word_at_position(source, Position::new(0, 7)), "myV");
        assert_eq!(get_word_at_position(source, Position::new(0, 9)), "myVar");
    }
}
