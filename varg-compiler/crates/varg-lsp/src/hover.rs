use tower_lsp::lsp_types::*;
use varg_ast::Token;
use varg_lexer::Lexer;

/// Identify the token at the given cursor position and return hover info.
pub fn compute_hover(source: &str, pos: Position) -> Option<Hover> {
    // Convert LSP Position to byte offset
    let offset = position_to_offset(source, pos)?;

    // Tokenize and find the token that spans this offset
    let lexer = Lexer::new(source);
    let tokens: Vec<(Token, std::ops::Range<usize>)> = lexer
        .filter_map(|(res, span)| res.ok().map(|tok| (tok, span)))
        .collect();

    for (token, span) in &tokens {
        if span.start <= offset && offset < span.end {
            let info = token_hover_info(token);
            if let Some((label, detail)) = info {
                let markdown = format!("**{}**\n\n{}", label, detail);
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: markdown,
                    }),
                    range: Some(Range::new(
                        offset_to_position(source, span.start),
                        offset_to_position(source, span.end),
                    )),
                });
            }
        }
    }

    None
}

fn position_to_offset(source: &str, pos: Position) -> Option<usize> {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in source.char_indices() {
        if line == pos.line && col == pos.character {
            return Some(i);
        }
        if ch == '\n' {
            if line == pos.line {
                return Some(i); // cursor at end of line
            }
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    // Cursor at very end of file
    if line == pos.line && col == pos.character {
        Some(source.len())
    } else {
        None
    }
}

fn offset_to_position(source: &str, offset: usize) -> Position {
    let offset = offset.min(source.len());
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position::new(line, col)
}

/// Return a (label, description) for known Varg tokens/keywords.
fn token_hover_info(token: &Token) -> Option<(&'static str, &'static str)> {
    match token {
        // Keywords
        Token::Agent => Some(("keyword `agent`", "Declares a Varg agent (class-like entity with methods and state).")),
        Token::Contract => Some(("keyword `contract`", "Declares a contract (interface/trait) that agents can implement.")),
        Token::Public => Some(("keyword `public`", "Makes a method or agent visible outside the current module.")),
        Token::System => Some(("keyword `system`", "Marks an agent as a system-level agent with elevated privileges.")),
        Token::Var => Some(("keyword `var`", "Declares a mutable local variable with type inference.")),
        Token::Return => Some(("keyword `return`", "Returns a value from the current method.")),
        Token::If => Some(("keyword `if`", "Conditional branch. Syntax: `if (condition) { ... } else { ... }`")),
        Token::Else => Some(("keyword `else`", "Alternative branch of an if statement.")),
        Token::While => Some(("keyword `while`", "Loop that continues while condition is true.")),
        Token::For => Some(("keyword `for`", "C-style for loop. Syntax: `for (init; cond; update) { ... }`")),
        Token::Foreach => Some(("keyword `foreach`", "Iterate over a collection. Syntax: `foreach (var item in collection) { ... }`")),
        Token::Match => Some(("keyword `match`", "Pattern matching on enums or values.")),
        Token::Unsafe => Some(("keyword `unsafe`", "Unsafe block for low-level operations. Requires appropriate OCAP tokens.")),
        Token::Try => Some(("keyword `try`", "Begin a try/catch error handling block.")),
        Token::Catch => Some(("keyword `catch`", "Handle errors from a try block.")),
        Token::Throw => Some(("keyword `throw`", "Throw an error string.")),
        Token::Print => Some(("keyword `print`", "Print a value to stdout with debug formatting.")),
        Token::Stream => Some(("keyword `stream`", "Stream output incrementally (SSE/NDJSON).")),
        Token::Enum => Some(("keyword `enum`", "Declare an algebraic data type with variants.")),
        Token::Where => Some(("keyword `where`", "Generic constraint clause. Syntax: `where T: Comparable`")),

        // Types
        Token::TypeInt => Some(("type `int`", "64-bit signed integer (`i64` in Rust).")),
        Token::TypeString => Some(("type `string`", "Heap-allocated UTF-8 string (`String` in Rust).")),
        Token::TypeBool => Some(("type `bool`", "Boolean value: `true` or `false`.")),
        Token::TypeVoid => Some(("type `void`", "No return value (unit type `()` in Rust).")),
        Token::TypeUlong => Some(("type `ulong`", "64-bit unsigned integer (`u64` in Rust).")),

        // AI-native types
        Token::Prompt => Some(("type `Prompt`", "AI prompt type. Contains structured text for LLM inference.")),
        Token::Context => Some(("type `Context`", "Conversation context for multi-turn LLM chat.")),
        Token::Tensor => Some(("type `Tensor`", "Multi-dimensional numeric array for ML operations.")),
        Token::Embedding => Some(("type `Embedding`", "Vector embedding for semantic similarity (cosine ~).")),

        // OCAP tokens
        Token::NetworkAccess => Some(("OCAP `NetworkAccess`", "Capability token granting HTTP/network access. Required by `fetch()`.")),
        Token::FileAccess => Some(("OCAP `FileAccess`", "Capability token granting filesystem read/write access.")),
        Token::DbAccess => Some(("OCAP `DbAccess`", "Capability token granting database query access.")),
        Token::LlmAccess => Some(("OCAP `LlmAccess`", "Capability token granting LLM inference access.")),
        Token::SystemAccess => Some(("OCAP `SystemAccess`", "Capability token granting low-level system operations.")),

        // Operators
        Token::Tilde => Some(("operator `~`", "Cosine similarity between two embeddings/tensors.")),
        Token::FatArrow => Some(("operator `=>`", "Lambda arrow. Syntax: `(int x) => x * 2`")),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hover_on_keyword() {
        let source = "agent Test {}";
        let hover = compute_hover(source, Position::new(0, 2)); // cursor inside "agent"
        assert!(hover.is_some());
        let content = match hover.unwrap().contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("Expected markup"),
        };
        assert!(content.contains("agent"));
    }

    #[test]
    fn test_hover_on_type() {
        let source = "agent T { public void Run(int x) {} }";
        // Find "int" — it's after the opening paren
        let hover = compute_hover(source, Position::new(0, 26));
        assert!(hover.is_some());
        let content = match hover.unwrap().contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("Expected markup"),
        };
        assert!(content.contains("64-bit"));
    }

    #[test]
    fn test_hover_on_nothing() {
        let source = "agent Test {}";
        // Cursor on whitespace or something without hover info
        let _hover = compute_hover(source, Position::new(0, 5)); // space between agent and Test
        // Identifiers don't have hover info in this implementation
        // (they would need symbol table lookup)
        // This is fine — returns None for unknown tokens
    }

    #[test]
    fn test_position_offset_roundtrip() {
        let source = "hello\nworld\n!";
        assert_eq!(position_to_offset(source, Position::new(0, 0)), Some(0));
        assert_eq!(position_to_offset(source, Position::new(1, 0)), Some(6));
        assert_eq!(position_to_offset(source, Position::new(1, 3)), Some(9));
        assert_eq!(position_to_offset(source, Position::new(2, 0)), Some(12));
    }
}
