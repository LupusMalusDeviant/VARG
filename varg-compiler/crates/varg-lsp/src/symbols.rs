use tower_lsp::lsp_types::*;
use varg_ast::{ast::Item, Token};
use varg_lexer::Lexer;
use varg_parser::Parser;

/// A named symbol definition in the source (agent, function, struct, contract, enum, method).
#[derive(Debug, Clone)]
pub struct SymbolDef {
    pub name: String,
    pub kind: SymbolKind,
    pub range: Range,
}

/// A single identifier reference (every identifier token in the source).
#[derive(Debug, Clone)]
pub struct SymbolRef {
    pub name: String,
    pub range: Range,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Collect all top-level symbol definitions (agents, contracts, structs, enums,
/// standalone functions) plus their methods from Varg source text.
pub fn collect_definitions(source: &str) -> Vec<SymbolDef> {
    let mut parser = Parser::new(source);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(_) => return vec![],
    };

    let mut defs = Vec::new();

    for item in &program.items {
        match item {
            Item::Agent(a) => {
                if let Some(range) = find_decl_name_range(source, &a.name) {
                    defs.push(SymbolDef {
                        name: a.name.clone(),
                        kind: SymbolKind::CLASS,
                        range,
                    });
                }
                for method in &a.methods {
                    if let Some(range) = find_method_name_range(source, &a.name, &method.name) {
                        defs.push(SymbolDef {
                            name: format!("{}.{}", a.name, method.name),
                            kind: SymbolKind::METHOD,
                            range,
                        });
                    }
                }
            }
            Item::Contract(c) => {
                if let Some(range) = find_decl_name_range(source, &c.name) {
                    defs.push(SymbolDef {
                        name: c.name.clone(),
                        kind: SymbolKind::INTERFACE,
                        range,
                    });
                }
                for method in &c.methods {
                    if let Some(range) = find_method_name_range(source, &c.name, &method.name) {
                        defs.push(SymbolDef {
                            name: format!("{}.{}", c.name, method.name),
                            kind: SymbolKind::METHOD,
                            range,
                        });
                    }
                }
            }
            Item::Struct(s) => {
                if let Some(range) = find_decl_name_range(source, &s.name) {
                    defs.push(SymbolDef {
                        name: s.name.clone(),
                        kind: SymbolKind::STRUCT,
                        range,
                    });
                }
            }
            Item::Enum(e) => {
                if let Some(range) = find_decl_name_range(source, &e.name) {
                    defs.push(SymbolDef {
                        name: e.name.clone(),
                        kind: SymbolKind::ENUM,
                        range,
                    });
                }
            }
            Item::Function(f) => {
                if let Some(range) = find_decl_name_range(source, &f.name) {
                    defs.push(SymbolDef {
                        name: f.name.clone(),
                        kind: SymbolKind::FUNCTION,
                        range,
                    });
                }
            }
            _ => {}
        }
    }

    defs
}

/// Collect all identifier token occurrences from the source (for find-references).
pub fn collect_references(source: &str) -> Vec<SymbolRef> {
    let lexer = Lexer::new(source);
    let mut refs = Vec::new();
    for (result, span) in lexer {
        if let Ok(Token::Identifier(name)) = result {
            let range = byte_range_to_lsp(source, span.start, span.end);
            refs.push(SymbolRef { name, range });
        }
    }
    refs
}

/// Extract the identifier word at the given LSP position.
pub fn word_at_position(source: &str, pos: Position) -> Option<String> {
    let offset = position_to_byte_offset(source, pos);
    let bytes = source.as_bytes();
    if offset >= bytes.len() {
        return None;
    }

    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';

    if !is_ident(bytes[offset]) {
        return None;
    }

    // Scan left to find start of identifier
    let start = (0..=offset)
        .rev()
        .find(|&i| !is_ident(bytes[i]))
        .map(|i| i + 1)
        .unwrap_or(0);

    // Scan right to find end of identifier
    let end = (offset..bytes.len())
        .find(|&i| !is_ident(bytes[i]))
        .unwrap_or(bytes.len());

    Some(source[start..end].to_string())
}

// ---------------------------------------------------------------------------
// Position / range helpers (shared with other modules)
// ---------------------------------------------------------------------------

pub fn byte_range_to_lsp(source: &str, start: usize, end: usize) -> Range {
    Range {
        start: byte_offset_to_position(source, start),
        end: byte_offset_to_position(source, end),
    }
}

pub fn byte_offset_to_position(source: &str, offset: usize) -> Position {
    let clamped = offset.min(source.len());
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in source.char_indices() {
        if i >= clamped {
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

pub fn position_to_byte_offset(source: &str, pos: Position) -> usize {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in source.char_indices() {
        if line == pos.line && col == pos.character {
            return i;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    source.len()
}

// ---------------------------------------------------------------------------
// Internal helpers for locating names in the raw source text
// ---------------------------------------------------------------------------

/// Find the LSP Range of `name` as it appears after an `agent`/`contract`/
/// `struct`/`fn`/`enum` keyword in the source.
fn find_decl_name_range(source: &str, name: &str) -> Option<Range> {
    let keywords = ["agent ", "contract ", "struct ", "fn ", "enum "];
    let mut best: Option<usize> = None; // byte offset of the best (earliest) match

    for kw in &keywords {
        let mut search_from = 0usize;
        while let Some(kw_pos) = source[search_from..].find(kw) {
            let abs_kw = search_from + kw_pos;
            let after_kw = &source[abs_kw + kw.len()..];
            let trimmed = after_kw.trim_start();
            let trim_offset = after_kw.len() - trimmed.len();

            if trimmed.starts_with(name) {
                let next = trimmed.as_bytes().get(name.len()).copied();
                let is_boundary = next.map_or(true, |b| !b.is_ascii_alphanumeric() && b != b'_');
                if is_boundary {
                    let name_start = abs_kw + kw.len() + trim_offset;
                    if best.map_or(true, |b| name_start < b) {
                        best = Some(name_start);
                    }
                }
            }
            search_from = abs_kw + 1;
        }
    }

    best.map(|name_start| byte_range_to_lsp(source, name_start, name_start + name.len()))
}

/// Find the LSP Range of `method_name` defined inside the block for `parent_name`.
/// We look for `fn <method_name>` or `void <method_name>` etc. after the parent block opens.
fn find_method_name_range(source: &str, parent_name: &str, method_name: &str) -> Option<Range> {
    // Find the parent declaration first
    let parent_pos = source.find(parent_name)?;
    // Find the opening brace of the parent body
    let block_start = source[parent_pos..].find('{').map(|p| parent_pos + p)?;

    // Within that block's text search for the method name after `fn ` or as an identifier
    // following a return type (e.g. `public void Run()`).
    // We do a simple scan: look for `method_name` followed by `(` in the block region.
    let block_text = &source[block_start..];

    // We use the lexer approach: scan identifier tokens in the block region that match
    // the method name and are followed shortly by `(`.
    let lexer = Lexer::new(block_text);
    let tokens: Vec<(std::result::Result<Token, _>, std::ops::Range<usize>)> = lexer.collect();

    for i in 0..tokens.len() {
        let (ref result, ref span) = tokens[i];
        if let Ok(Token::Identifier(ref id)) = result {
            if id == method_name {
                // Check next non-whitespace token is `(`
                let next_is_lparen = tokens.get(i + 1).map_or(false, |(r, _)| {
                    matches!(r, Ok(Token::LParen))
                });
                if next_is_lparen {
                    let abs_start = block_start + span.start;
                    let abs_end = block_start + span.end;
                    return Some(byte_range_to_lsp(source, abs_start, abs_end));
                }
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
agent Greeter {
    public void Hello() {
        var x = 1;
    }
    public void World() {}
}

contract ILogger {
    void log(string message);
}

fn standalone() -> int {
    return 42;
}

struct Point {
    int x;
    int y;
}

enum Color {
    Red,
    Green,
    Blue,
}
"#;

    #[test]
    fn test_collect_definitions_finds_agent() {
        let defs = collect_definitions(SAMPLE);
        let agent = defs.iter().find(|d| d.name == "Greeter");
        assert!(agent.is_some(), "Expected to find Greeter agent");
        assert_eq!(agent.unwrap().kind, SymbolKind::CLASS);
    }

    #[test]
    fn test_collect_definitions_finds_methods() {
        let defs = collect_definitions(SAMPLE);
        assert!(defs.iter().any(|d| d.name == "Greeter.Hello"),
            "Expected Greeter.Hello method");
        assert!(defs.iter().any(|d| d.name == "Greeter.World"),
            "Expected Greeter.World method");
    }

    #[test]
    fn test_collect_definitions_finds_contract() {
        let defs = collect_definitions(SAMPLE);
        let c = defs.iter().find(|d| d.name == "ILogger");
        assert!(c.is_some(), "Expected ILogger contract");
        assert_eq!(c.unwrap().kind, SymbolKind::INTERFACE);
    }

    #[test]
    fn test_collect_definitions_finds_function() {
        let defs = collect_definitions(SAMPLE);
        let f = defs.iter().find(|d| d.name == "standalone");
        assert!(f.is_some(), "Expected standalone function");
        assert_eq!(f.unwrap().kind, SymbolKind::FUNCTION);
    }

    #[test]
    fn test_collect_definitions_finds_struct() {
        let defs = collect_definitions(SAMPLE);
        let s = defs.iter().find(|d| d.name == "Point");
        assert!(s.is_some(), "Expected Point struct");
        assert_eq!(s.unwrap().kind, SymbolKind::STRUCT);
    }

    #[test]
    fn test_collect_definitions_finds_enum() {
        let defs = collect_definitions(SAMPLE);
        let e = defs.iter().find(|d| d.name == "Color");
        assert!(e.is_some(), "Expected Color enum");
        assert_eq!(e.unwrap().kind, SymbolKind::ENUM);
    }

    #[test]
    fn test_collect_references() {
        let source = "agent Foo { public void Bar() { var x = Foo; } }";
        let refs = collect_references(source);
        let foo_refs: Vec<_> = refs.iter().filter(|r| r.name == "Foo").collect();
        // "Foo" appears at least twice: definition name + usage
        assert!(foo_refs.len() >= 2, "Expected multiple refs to Foo, got {}", foo_refs.len());
    }

    #[test]
    fn test_word_at_position() {
        let source = "agent Greeter {}";
        // Position at 'G' in Greeter (col 6)
        let word = word_at_position(source, Position::new(0, 6));
        assert_eq!(word, Some("Greeter".to_string()));
    }

    #[test]
    fn test_word_at_position_whitespace() {
        let source = "agent Greeter {}";
        // Position on space between 'agent' and 'Greeter' (col 5)
        let word = word_at_position(source, Position::new(0, 5));
        assert_eq!(word, None);
    }

    #[test]
    fn test_byte_offset_to_position_multiline() {
        let source = "line1\nline2\nline3";
        assert_eq!(byte_offset_to_position(source, 0), Position::new(0, 0));
        assert_eq!(byte_offset_to_position(source, 6), Position::new(1, 0));
        assert_eq!(byte_offset_to_position(source, 12), Position::new(2, 0));
    }

    #[test]
    fn test_invalid_source_returns_empty_defs() {
        let defs = collect_definitions("@@@invalid source###");
        // Should not panic and return empty or partial results
        let _ = defs; // any result is fine
    }

    #[test]
    fn test_parser_parses_sample() {
        use varg_parser::Parser;
        let mut parser = Parser::new(SAMPLE);
        let result = parser.parse_program();
        match &result {
            Ok(p) => println!("Parsed OK: {} items", p.items.len()),
            Err(e) => println!("Parse error: {:?}", e),
        }
        assert!(result.is_ok(), "Parser should succeed on SAMPLE");
        let prog = result.unwrap();
        assert!(!prog.items.is_empty(), "Program should have items");
    }
}
