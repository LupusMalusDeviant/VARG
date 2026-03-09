use tower_lsp::lsp_types::*;
use varg_parser::Parser;
use varg_typechecker::TypeChecker;

/// Convert a byte offset into an LSP Position (line, character).
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

/// Run the Varg parser and typechecker on the source text,
/// returning any errors as LSP Diagnostics.
pub fn compute_diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Phase 1: Parse
    let mut parser = Parser::new(source);
    let program = match parser.parse_program() {
        Ok(prog) => prog,
        Err(parse_err) => {
            let (message, range) = match &parse_err {
                varg_parser::ParseError::UnexpectedToken { expected, found, span } => {
                    let msg = format!(
                        "Unexpected token: expected {}, found {:?}",
                        expected,
                        found.as_ref().map(|t| format!("{:?}", t)).unwrap_or_else(|| "EOF".to_string())
                    );
                    let start = offset_to_position(source, span.start);
                    let end = offset_to_position(source, span.end);
                    (msg, Range::new(start, end))
                }
                varg_parser::ParseError::UnexpectedEof => {
                    let pos = offset_to_position(source, source.len());
                    ("Unexpected end of file".to_string(), Range::new(pos, pos))
                }
            };

            diagnostics.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("varg-parser".to_string()),
                message,
                ..Default::default()
            });

            return diagnostics;
        }
    };

    // Phase 2: Type check
    let mut checker = TypeChecker::new();
    if let Err(type_err) = checker.check_program(&program) {
        let message = type_err.message();

        // TypeChecker errors don't carry spans yet — mark the whole file
        let range = Range::new(Position::new(0, 0), offset_to_position(source, source.len()));

        diagnostics.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::WARNING),
            source: Some("varg-typechecker".to_string()),
            message,
            ..Default::default()
        });
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_program_no_diagnostics() {
        let source = r#"
            agent Hello {
                public void Run() {
                    var x = 42;
                    print(x);
                }
            }
        "#;
        let diags = compute_diagnostics(source);
        assert!(diags.is_empty(), "Expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn test_parse_error_produces_diagnostic() {
        let source = r#"
            agent Hello {
                public void Run() {
                    var x = ;
                }
            }
        "#;
        let diags = compute_diagnostics(source);
        assert!(!diags.is_empty());
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diags[0].source.as_deref() == Some("varg-parser"));
    }

    #[test]
    fn test_type_error_produces_warning() {
        let source = r#"
            agent Hello {
                public void Run() {
                    var x = undeclared_var;
                }
            }
        "#;
        let diags = compute_diagnostics(source);
        assert!(!diags.is_empty());
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::WARNING));
        assert!(diags[0].source.as_deref() == Some("varg-typechecker"));
    }

    #[test]
    fn test_offset_to_position_basic() {
        let source = "line1\nline2\nline3";
        assert_eq!(offset_to_position(source, 0), Position::new(0, 0));
        assert_eq!(offset_to_position(source, 5), Position::new(0, 5)); // newline char
        assert_eq!(offset_to_position(source, 6), Position::new(1, 0));
        assert_eq!(offset_to_position(source, 12), Position::new(2, 0));
    }
}
