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
