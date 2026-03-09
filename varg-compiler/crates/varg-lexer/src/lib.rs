use logos::Logos;
use varg_ast::Token;

pub struct Lexer<'a> {
    inner: logos::Lexer<'a, Token>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            inner: Token::lexer(source),
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = (Result<Token, ()>, std::ops::Range<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|t| (t, self.inner.span()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_basic_keywords() {
        let source = "system agent MemoryManager { }";
        let mut lexer = Lexer::new(source);

        assert_eq!(lexer.next().unwrap().0, Ok(Token::System));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Agent));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("MemoryManager".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LBrace));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::RBrace));
        assert_eq!(lexer.next(), None);
    }
    
    #[test]
    fn test_lex_unsafe_and_types() {
        let source = "unsafe { Prompt p; }";
        let mut lexer = Lexer::new(source);

        assert_eq!(lexer.next().unwrap().0, Ok(Token::Unsafe));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LBrace));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Prompt));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("p".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Semicolon));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::RBrace));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_literals_and_operators() {
        let source = "int x = 42; string name = \"varg\"; bool b = true;";
        let mut lexer = Lexer::new(source);

        // int x = 42;
        assert_eq!(lexer.next().unwrap().0, Ok(Token::TypeInt));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("x".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Assign));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::IntLiteral(42)));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Semicolon));
        
        // string name = "varg";
        assert_eq!(lexer.next().unwrap().0, Ok(Token::TypeString));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("name".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Assign));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::StringLiteral("\"varg\"".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Semicolon));

        // bool b = true;
        assert_eq!(lexer.next().unwrap().0, Ok(Token::TypeBool));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("b".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Assign));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::BoolLiteral(true)));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Semicolon));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_ignore_comments() {
        let source = "// this is a comment\n agent A { /* multi\nline */ }";
        let mut lexer = Lexer::new(source);
        
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Agent));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("A".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LBrace));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::RBrace));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_all_operators() {
        let source = "== != < > <= >= + - * / ~ = . -> ,";
        let mut lexer = Lexer::new(source);
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Equals));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::NotEquals));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LessThan));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::GreaterThan));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LessOrEqual));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::GreaterOrEqual));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Plus));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Minus));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Multiply));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Divide));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Tilde));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Assign));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Dot));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Arrow));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Comma));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_brackets_and_symbols() {
        let source = "{ } ( ) [ ] ; : ? @";
        let mut lexer = Lexer::new(source);
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LBrace));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::RBrace));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LParen));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::RParen));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LBracket));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::RBracket));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Semicolon));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Colon));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::QuestionMark));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::At));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_control_flow_keywords() {
        let source = "if else while for foreach in return";
        let mut lexer = Lexer::new(source);
        assert_eq!(lexer.next().unwrap().0, Ok(Token::If));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Else));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::While));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::For));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Foreach));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::In));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Return));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_type_system_keywords() {
        let source = "enum type null match";
        let mut lexer = Lexer::new(source);
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Enum));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Type));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Null));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Match));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_error_handling_keywords() {
        let source = "try catch throw";
        let mut lexer = Lexer::new(source);
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Try));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Catch));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Throw));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_native_ai_types() {
        let source = "Prompt Context Tensor Embedding Result";
        let mut lexer = Lexer::new(source);
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Prompt));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Context));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Tensor));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Embedding));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Result));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_linq_keywords() {
        let source = "from where select orderby descending";
        let mut lexer = Lexer::new(source);
        assert_eq!(lexer.next().unwrap().0, Ok(Token::From));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Where));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Select));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Orderby));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Descending));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_empty_input() {
        let mut lexer = Lexer::new("");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_only_whitespace_and_comments() {
        let source = "   \t\n\r\n  // comment\n  /* block */  ";
        let mut lexer = Lexer::new(source);
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_string_with_escapes() {
        let source = r#""hello \"world\"""#;
        let mut lexer = Lexer::new(source);
        let tok = lexer.next().unwrap().0;
        assert!(matches!(tok, Ok(Token::StringLiteral(_))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_identifiers_vs_keywords() {
        // Multi-character identifiers should not conflict with keywords
        let source = "agentName contractX structData enumVal typeAlias";
        let mut lexer = Lexer::new(source);
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("agentName".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("contractX".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("structData".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("enumVal".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("typeAlias".to_string())));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_vargmin_shorthands() {
        let source = "+A -A +M +V";
        let mut lexer = Lexer::new(source);
        assert_eq!(lexer.next().unwrap().0, Ok(Token::PlusA));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::MinusA));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::PlusM));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::PlusV));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_span_positions() {
        let source = "agent X";
        let mut lexer = Lexer::new(source);
        let (tok1, span1) = lexer.next().unwrap();
        assert_eq!(tok1, Ok(Token::Agent));
        assert_eq!(span1, 0..5);
        let (tok2, span2) = lexer.next().unwrap();
        assert_eq!(tok2, Ok(Token::Identifier("X".to_string())));
        assert_eq!(span2, 6..7);
    }

    #[test]
    fn test_contract_and_target_annotation() {
        let source = "@target(\"NPU\") public contract Search { Result<string, Error> find(); }";
        let mut lexer = Lexer::new(source);

        assert_eq!(lexer.next().unwrap().0, Ok(Token::TargetAnnotation));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LParen));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::StringLiteral("\"NPU\"".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::RParen));
        
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Public));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Contract));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("Search".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LBrace));
        
        // Result<string, Error> find();
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Result));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LessThan));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::TypeString));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Comma));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("Error".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::GreaterThan));
        
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Identifier("find".to_string())));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::LParen));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::RParen));
        assert_eq!(lexer.next().unwrap().0, Ok(Token::Semicolon));
        
        assert_eq!(lexer.next().unwrap().0, Ok(Token::RBrace));
        assert_eq!(lexer.next(), None);
    }
}
