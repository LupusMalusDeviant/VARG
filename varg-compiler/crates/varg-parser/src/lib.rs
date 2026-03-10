use varg_ast::Token;
use varg_ast::ast::*;
use varg_lexer::Lexer;
use std::ops::Range;

pub struct Parser {
    tokens: Vec<(Token, Range<usize>)>,
    pos: usize,
    source_len: usize,
    /// Plan 23: Raw source for prompt template body extraction
    raw_source: String,
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    UnexpectedToken { expected: String, found: Option<Token>, span: Range<usize> },
    UnexpectedEof,
}

impl ParseError {
    /// Returns the source span where this error occurred (if available)
    pub fn span(&self) -> Option<Range<usize>> {
        match self {
            ParseError::UnexpectedToken { span, .. } => Some(span.clone()),
            ParseError::UnexpectedEof => None,
        }
    }
}

impl Parser {
    pub fn new(source: &str) -> Self {
        let lexer = Lexer::new(source);
        let tokens: Vec<(Token, Range<usize>)> = lexer
            .filter_map(|(res, span)| res.ok().map(|tok| (tok, span)))
            .collect();
        let source_len = source.len();
        Self { tokens, pos: 0, source_len, raw_source: source.to_string() }
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].0.clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    /// Returns the byte span of the last consumed token
    fn last_span(&self) -> Range<usize> {
        if self.pos > 0 && self.pos <= self.tokens.len() {
            self.tokens[self.pos - 1].1.clone()
        } else {
            self.source_len..self.source_len
        }
    }

    /// Returns the byte span of the current (peeked) token
    fn current_span(&self) -> Range<usize> {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].1.clone()
        } else {
            self.source_len..self.source_len
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(tok, _)| tok)
    }

    fn is_variable_declaration_start(&mut self) -> bool {
        let saved_pos = self.pos;
        let is_decl = match self.parse_type() {
            Ok(_) => matches!(self.peek(), Some(Token::Identifier(_))),
            Err(_) => false,
        };
        self.pos = saved_pos;
        is_decl
    }

    fn consume(&mut self, expected: Token) -> Result<(), ParseError> {
        let span = self.current_span();
        match self.advance() {
            Some(t) if t == expected => Ok(()),
            Some(t) => Err(ParseError::UnexpectedToken {
                expected: format!("{:?}", expected),
                found: Some(t),
                span,
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        let span = self.current_span();
        match self.advance() {
            Some(Token::Identifier(name)) => Ok(name),
            Some(t) => Err(ParseError::UnexpectedToken {
                expected: "Identifier".to_string(),
                found: Some(t),
                span,
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_annotations(&mut self) -> Result<Vec<Annotation>, ParseError> {
        let mut annotations = Vec::new();
        while let Some(Token::At) = self.peek() {
            self.advance();
            self.consume(Token::LBracket)?;
            let name = self.parse_identifier()?;
            let mut values = Vec::new();
            if let Some(Token::LParen) = self.peek() {
                self.advance();
                if self.peek() != Some(&Token::RParen) {
                    loop {
                        if let Some(Token::StringLiteral(val)) = self.advance() {
                            values.push(val.trim_matches('"').to_string());
                        } else {
                            return Err(ParseError::UnexpectedToken {
                                expected: "String Literal".to_string(),
                                found: self.advance(),
                                span: self.current_span(),
                            });
                        }
                        if self.peek() == Some(&Token::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.consume(Token::RParen)?;
            }
            self.consume(Token::RBracket)?;
            annotations.push(Annotation { name, values });
        }
        Ok(annotations)
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut no_std = false;
        if let Some(Token::NoStd) = self.peek() {
            self.advance();
            no_std = true;
        }

        let mut items = Vec::new();
        while self.peek().is_some() {
            items.push(self.parse_item()?);
        }
        Ok(Program { no_std, items })
    }

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        let mut target_annotation = None;
        if let Some(Token::TargetAnnotation) = self.peek() {
            self.advance();
            self.consume(Token::LParen)?;
            if let Some(Token::StringLiteral(val)) = self.advance() {
                target_annotation = Some(val.trim_matches('"').to_string());
            }
            self.consume(Token::RParen)?;
        }

        let annotations = self.parse_annotations()?;

        let mut is_public = false;
        let mut is_system = false;

        loop {
            match self.peek() {
                Some(Token::Public) => {
                    self.advance();
                    is_public = true;
                }
                Some(Token::System) => {
                    self.advance();
                    is_system = true;
                }
                _ => break,
            }
        }

        match self.peek() {
            Some(Token::Import) => {
                self.advance();
                let module = self.parse_identifier()?;
                self.consume(Token::Semicolon)?;
                Ok(Item::Import(module))
            },
            Some(Token::Agent) | Some(Token::PlusA) | Some(Token::MinusA) => {
                let tok = self.advance().unwrap();
                if tok == Token::PlusA { is_public = true; }
                if tok == Token::MinusA { is_public = false; }
                
                let name = self.parse_identifier()?;
                let (mut fields, methods) = self.parse_agent_body()?;
                // Plan 17: @[WithContext] auto-injects context field
                if annotations.iter().any(|a| a.name == "WithContext") {
                    if !fields.iter().any(|f| f.name == "context") {
                        fields.insert(0, FieldDecl { name: "context".to_string(), ty: TypeNode::Context });
                    }
                }
                Ok(Item::Agent(AgentDef {
                    name,
                    is_system,
                    is_public,
                    target_annotation,
                    annotations,
                    fields,
                    methods,
                }))
            },
            Some(Token::Contract) => {
                self.advance();
                let name = self.parse_identifier()?;
                let methods = self.parse_methods_block()?;
                Ok(Item::Contract(ContractDef {
                    name,
                    is_public,
                    target_annotation,
                    methods,
                }))
            },
            Some(Token::Enum) => {
                self.advance();
                let name = self.parse_identifier()?;
                let variants = self.parse_enum_variants()?;
                Ok(Item::Enum(EnumDef {
                    name,
                    is_public,
                    variants,
                }))
            },
            Some(Token::Type) => {
                self.advance();
                let name = self.parse_identifier()?;
                self.consume(Token::Assign)?;
                let target = self.parse_type()?;
                self.consume(Token::Semicolon)?;
                Ok(Item::TypeAlias { name, target })
            },
            Some(Token::Struct) => {
                self.advance();
                let name = self.parse_identifier()?;
                let mut type_params = Vec::new();
                if self.peek() == Some(&Token::LessThan) {
                    self.advance();
                    if self.peek() != Some(&Token::GreaterThan) {
                        loop {
                            type_params.push(self.parse_identifier()?);
                            if let Some(Token::Comma) = self.peek() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.consume(Token::GreaterThan)?;
                }
                let fields = self.parse_struct_fields_block()?;
                Ok(Item::Struct(StructDef {
                    name,
                    is_public,
                    type_params,
                    fields,
                }))
            }
            // Plan 23: prompt template_name(params) { body }
            Some(Token::Prompt) => {
                self.advance();
                let name = self.parse_identifier()?;
                self.consume(Token::LParen)?;
                let mut params = Vec::new();
                if self.peek() != Some(&Token::RParen) {
                    loop {
                        let ty = self.parse_type()?;
                        let param_name = self.parse_identifier()?;
                        params.push(FieldDecl { name: param_name, ty });
                        if self.peek() == Some(&Token::Comma) { self.advance(); } else { break; }
                    }
                }
                self.consume(Token::RParen)?;
                // Extract raw body text between { and }
                self.consume(Token::LBrace)?;
                let body = self.parse_prompt_template_body()?;
                self.consume(Token::RBrace)?;
                Ok(Item::PromptTemplate(PromptTemplateDef { name, params, body }))
            },
            Some(t) => Err(ParseError::UnexpectedToken {
                expected: "Agent, Contract, Struct, Enum, Type, or Prompt".to_string(),
                found: Some(t.clone()),
                span: self.current_span(),
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_enum_variants(&mut self) -> Result<Vec<EnumVariant>, ParseError> {
        self.consume(Token::LBrace)?;
        let mut variants = Vec::new();
        while let Some(tok) = self.peek() {
            if *tok == Token::RBrace {
                break;
            }
            let variant_name = self.parse_identifier()?;
            let mut fields = Vec::new();
            if self.peek() == Some(&Token::LParen) {
                self.advance();
                if self.peek() != Some(&Token::RParen) {
                    loop {
                        let field_ty = self.parse_type()?;
                        let field_name = self.parse_identifier()?;
                        fields.push((field_name, field_ty));
                        if self.peek() == Some(&Token::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.consume(Token::RParen)?;
            }
            // Consume optional comma between variants
            if self.peek() == Some(&Token::Comma) {
                self.advance();
            }
            variants.push(EnumVariant { name: variant_name, fields });
        }
        self.consume(Token::RBrace)?;
        Ok(variants)
    }

    /// Plan 23: Extract raw template body text between { and }
    /// The opening { has already been consumed. Scans forward to the matching }
    /// using brace-depth tracking to handle {var} interpolation markers.
    fn parse_prompt_template_body(&mut self) -> Result<String, ParseError> {
        let start_byte = self.last_span().end;
        let mut depth: usize = 0;

        while self.pos < self.tokens.len() {
            match &self.tokens[self.pos].0 {
                Token::LBrace => {
                    depth += 1;
                    self.pos += 1;
                },
                Token::RBrace => {
                    if depth == 0 {
                        // This is the body-closing brace — don't consume (caller will)
                        let end_byte = self.tokens[self.pos].1.start;
                        let body = self.raw_source[start_byte..end_byte].trim().to_string();
                        return Ok(body);
                    }
                    depth -= 1;
                    self.pos += 1;
                },
                _ => {
                    self.pos += 1;
                }
            }
        }
        Err(ParseError::UnexpectedEof)
    }

    fn parse_struct_fields_block(&mut self) -> Result<Vec<FieldDecl>, ParseError> {
        self.consume(Token::LBrace)?;
        let mut fields = Vec::new();

        while let Some(tok) = self.peek() {
            if *tok == Token::RBrace {
                break;
            }

            // Simple property declaration e.g. `public string name;`
            let mut _is_public = false;
            if *tok == Token::Public {
                self.advance();
                _is_public = true;
            } else if *tok == Token::Private {
                self.advance();
            }

            let ty = self.parse_type()?;
            let name = self.parse_identifier()?;
            self.consume(Token::Semicolon)?;

            fields.push(FieldDecl { name, ty });
        }
        
        self.consume(Token::RBrace)?;
        Ok(fields)
    }

    pub fn parse_type(&mut self) -> Result<TypeNode, ParseError> {
        let base_type = self.parse_base_type()?;
        // Check for nullable suffix: string? → Nullable(String)
        if self.peek() == Some(&Token::QuestionMark) {
            self.advance();
            return Ok(TypeNode::Nullable(Box::new(base_type)));
        }
        Ok(base_type)
    }

    fn parse_base_type(&mut self) -> Result<TypeNode, ParseError> {
        match self.advance() {
            Some(Token::TypeInt) | Some(Token::TypeIntShort) => Ok(TypeNode::Int),
            Some(Token::TypeString) | Some(Token::TypeStringShort) => Ok(TypeNode::String),
            Some(Token::TypeBool) | Some(Token::TypeBoolShort) => Ok(TypeNode::Bool),
            Some(Token::TypeMapShort) => Ok(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::Custom("Dynamic".to_string())))),
            Some(Token::TypeVoid) => Ok(TypeNode::Void),
            Some(Token::TypeUlong) => Ok(TypeNode::Ulong),
            Some(Token::Prompt) => Ok(TypeNode::Prompt),
            Some(Token::Context) => Ok(TypeNode::Context),
            Some(Token::Tensor) => Ok(TypeNode::Tensor),
            Some(Token::Embedding) => Ok(TypeNode::Embedding),
            // OCAP Capability Tokens (Plan 03)
            Some(Token::NetworkAccess) => Ok(TypeNode::Capability(CapabilityType::NetworkAccess)),
            Some(Token::FileAccess) => Ok(TypeNode::Capability(CapabilityType::FileAccess)),
            Some(Token::DbAccess) => Ok(TypeNode::Capability(CapabilityType::DbAccess)),
            Some(Token::LlmAccess) => Ok(TypeNode::Capability(CapabilityType::LlmAccess)),
            Some(Token::SystemAccess) => Ok(TypeNode::Capability(CapabilityType::SystemAccess)),
            Some(Token::Result) => {
                self.consume(Token::LessThan)?;
                let ok_ty = Box::new(self.parse_type()?);
                self.consume(Token::Comma)?;
                let err_ty = Box::new(self.parse_type()?);
                self.consume(Token::GreaterThan)?;
                Ok(TypeNode::Result(ok_ty, err_ty))
            },
            Some(Token::Identifier(name)) => {
                if name == "List" {
                    self.consume(Token::LessThan)?;
                    let inner = Box::new(self.parse_type()?);
                    self.consume(Token::GreaterThan)?;
                    Ok(TypeNode::List(inner))
                } else if name == "Map" {
                    self.consume(Token::LessThan)?;
                    let key = Box::new(self.parse_type()?);
                    self.consume(Token::Comma)?;
                    let val = Box::new(self.parse_type()?);
                    self.consume(Token::GreaterThan)?;
                    Ok(TypeNode::Map(key, val))
                } else if self.peek() == Some(&Token::LessThan) {
                    self.advance();
                    let mut type_args = Vec::new();
                    if self.peek() != Some(&Token::GreaterThan) {
                        loop {
                            type_args.push(self.parse_type()?);
                            if let Some(Token::Comma) = self.peek() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.consume(Token::GreaterThan)?;
                    Ok(TypeNode::Generic(name, type_args))
                } else {
                    Ok(TypeNode::Custom(name))
                }
            },
            Some(t) => Err(ParseError::UnexpectedToken {
                expected: "A Type".to_string(),
                found: Some(t),
                span: self.last_span(),
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    /// Parses an agent body: fields (type name;) and methods (type name(...) { ... })
    fn parse_agent_body(&mut self) -> Result<(Vec<FieldDecl>, Vec<MethodDecl>), ParseError> {
        self.consume(Token::LBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while let Some(tok) = self.peek() {
            if *tok == Token::RBrace { break; }

            let annotations = self.parse_annotations()?;

            // Optional: async modifier
            let is_async = if let Some(Token::Async) = self.peek() {
                self.advance();
                true
            } else {
                false
            };

            let mut is_public = false;
            let mut return_ty = TypeNode::Void;

            match self.peek() {
                Some(Token::PlusM) | Some(Token::PlusV) => {
                    self.advance();
                    is_public = true;
                },
                Some(Token::Public) => {
                    self.advance();
                    is_public = true;
                    return_ty = self.parse_type()?;
                },
                _ => {
                    return_ty = self.parse_type()?;
                }
            }

            let name = self.parse_identifier()?;

            // Distinguish: field (;) vs method (()
            if self.peek() == Some(&Token::Semicolon) && !is_async {
                // It's a field declaration: type name;
                self.advance(); // consume ;
                fields.push(FieldDecl { name, ty: return_ty });
                continue;
            }

            // It's a method — continue parsing like parse_methods_block
            let mut type_params = Vec::new();
            if self.peek() == Some(&Token::LessThan) {
                self.advance();
                if self.peek() != Some(&Token::GreaterThan) {
                    loop {
                        type_params.push(self.parse_identifier()?);
                        if let Some(Token::Comma) = self.peek() { self.advance(); } else { break; }
                    }
                }
                self.consume(Token::GreaterThan)?;
            }

            self.consume(Token::LParen)?;
            let mut args = Vec::new();
            if let Some(inner) = self.peek() {
                if *inner != Token::RParen {
                    loop {
                        let arg_ty = self.parse_type()?;
                        let arg_name = self.parse_identifier()?;
                        args.push(FieldDecl { name: arg_name, ty: arg_ty });
                        if let Some(Token::Comma) = self.peek() { self.advance(); } else { break; }
                    }
                }
            }
            self.consume(Token::RParen)?;

            let mut constraints = Vec::new();
            if self.peek() == Some(&Token::Where) {
                self.advance();
                loop {
                    let type_param = self.parse_identifier()?;
                    self.consume(Token::Colon)?;
                    let bound = self.parse_identifier()?;
                    constraints.push(GenericConstraint { type_param, bound });
                    if self.peek() == Some(&Token::Comma) { self.advance(); } else { break; }
                }
            }

            let body = if let Some(Token::Semicolon) = self.peek() {
                self.advance();
                None
            } else {
                Some(self.parse_block()?)
            };

            methods.push(MethodDecl {
                name, is_public, is_async, annotations, type_params,
                constraints, args, return_ty: Some(return_ty), body,
            });
        }

        self.consume(Token::RBrace)?;
        Ok((fields, methods))
    }

    fn parse_methods_block(&mut self) -> Result<Vec<MethodDecl>, ParseError> {
        self.consume(Token::LBrace)?;
        let mut methods = Vec::new();

        while let Some(tok) = self.peek() {
            if *tok == Token::RBrace {
                break;
            }
            
            let annotations = self.parse_annotations()?;

            // Optional: async modifier
            let is_async = if let Some(Token::Async) = self.peek() {
                self.advance();
                true
            } else {
                false
            };

            let mut is_public = false;
            let mut return_ty = TypeNode::Void;

            match self.peek() {
                Some(Token::PlusM) | Some(Token::PlusV) => {
                    self.advance();
                    is_public = true;
                    // Varg-Min defaults to void return for methods
                },
                Some(Token::Public) => {
                    self.advance();
                    is_public = true;
                    return_ty = self.parse_type()?;
                },
                _ => {
                    return_ty = self.parse_type()?;
                }
            }

            let name = self.parse_identifier()?;
            
            let mut type_params = Vec::new();
            if self.peek() == Some(&Token::LessThan) {
                self.advance();
                if self.peek() != Some(&Token::GreaterThan) {
                    loop {
                        type_params.push(self.parse_identifier()?);
                        if let Some(Token::Comma) = self.peek() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.consume(Token::GreaterThan)?;
            }
            
            self.consume(Token::LParen)?;
            let mut args = Vec::new();
            if let Some(inner) = self.peek() {
                if *inner != Token::RParen {
                    loop {
                        let arg_ty = self.parse_type()?;
                        let arg_name = self.parse_identifier()?;
                        args.push(FieldDecl { name: arg_name, ty: arg_ty });

                        if let Some(Token::Comma) = self.peek() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
            }
            self.consume(Token::RParen)?;

            // Parse optional generic constraints: where T : Comparable, V : Serializable
            let mut constraints = Vec::new();
            if self.peek() == Some(&Token::Where) {
                self.advance();
                loop {
                    let type_param = self.parse_identifier()?;
                    self.consume(Token::Colon)?;
                    let bound = self.parse_identifier()?;
                    constraints.push(GenericConstraint { type_param, bound });
                    if self.peek() == Some(&Token::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }

            // Either a block {} or a semicolon ; (if inside a contract)
            let body = if let Some(Token::Semicolon) = self.peek() {
                self.advance();
                None
            } else {
                Some(self.parse_block()?)
            };

            methods.push(MethodDecl {
                name,
                is_public,
                is_async,
                annotations,
                type_params,
                constraints,
                args,
                return_ty: Some(return_ty),
                body,
            });
        }
        self.consume(Token::RBrace)?;
        Ok(methods)
    }

    /// Parse an optional else branch: `else { ... }` or `else if (...) { ... } else ...`
    fn parse_else_branch(&mut self) -> Result<Option<Block>, ParseError> {
        if let Some(Token::Else) = self.peek() {
            self.advance();
            if let Some(Token::If) = self.peek() {
                // else if → parse the if-statement recursively, wrap in a Block
                let if_stmt = self.parse_if_statement()?;
                Ok(Some(Block { statements: vec![if_stmt] }))
            } else {
                Ok(Some(self.parse_block()?))
            }
        } else {
            Ok(None)
        }
    }

    /// Parse a full if / else-if / else statement (called from parse_block and recursively from parse_else_branch)
    fn parse_if_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'if'
        self.consume(Token::LParen)?;
        let condition = self.parse_expression()?;
        self.consume(Token::RParen)?;
        let then_block = self.parse_block()?;
        let else_block = self.parse_else_branch()?;
        Ok(Statement::If { condition, then_block, else_block })
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        self.consume(Token::LBrace)?;
        let mut statements = Vec::new();

        while let Some(tok) = self.peek() {
            if *tok == Token::RBrace {
                break;
            }
            // TEMPORARY DEBUG LOG
            // dbg!(&tok);

            match tok {
                // Varg-Min Optional Variable Binding Type
                Token::Var | Token::TypeInt | Token::TypeString | Token::TypeBool | Token::Prompt | Token::Tensor | Token::Context | Token::TypeIntShort | Token::TypeStringShort | Token::TypeBoolShort | Token::TypeMapShort | Token::NetworkAccess | Token::FileAccess | Token::DbAccess | Token::LlmAccess | Token::SystemAccess => {
                    let ty = if *tok == Token::Var {
                        self.advance(); // consume 'var'
                        // Plan 06: Check for tuple destructuring: var (a, b) = ...
                        if self.peek() == Some(&Token::LParen) {
                            self.advance(); // consume (
                            let mut names = Vec::new();
                            if self.peek() != Some(&Token::RParen) {
                                loop {
                                    names.push(self.parse_identifier()?);
                                    if self.peek() == Some(&Token::Comma) {
                                        self.advance();
                                    } else { break; }
                                }
                            }
                            self.consume(Token::RParen)?;
                            self.consume(Token::Assign)?;
                            let value = self.parse_expression()?;
                            if let Some(Token::Semicolon) = self.peek() { self.advance(); }
                            statements.push(Statement::LetDestructure {
                                pattern: DestructurePattern::Tuple(names),
                                value,
                            });
                            continue;
                        }
                        // Plan 06: Check for struct destructuring: var { name, age } = ...
                        if self.peek() == Some(&Token::LBrace) {
                            self.advance(); // consume {
                            let mut fields = Vec::new();
                            if self.peek() != Some(&Token::RBrace) {
                                loop {
                                    let field_name = self.parse_identifier()?;
                                    let alias = if self.peek() == Some(&Token::Colon) {
                                        self.advance();
                                        Some(self.parse_identifier()?)
                                    } else {
                                        None
                                    };
                                    fields.push((field_name, alias));
                                    if self.peek() == Some(&Token::Comma) {
                                        self.advance();
                                    } else { break; }
                                }
                            }
                            self.consume(Token::RBrace)?;
                            self.consume(Token::Assign)?;
                            let value = self.parse_expression()?;
                            if let Some(Token::Semicolon) = self.peek() { self.advance(); }
                            statements.push(Statement::LetDestructure {
                                pattern: DestructurePattern::Struct(fields),
                                value,
                            });
                            continue;
                        }
                        None
                    } else {
                        Some(self.parse_type()?)
                    };
                    let name = self.parse_identifier()?;
                    self.consume(Token::Assign)?;

                    // Here we need to NOT eat the { as a block if it's an assignment.
                    // `parse_expression` naturally handles `LBrace` as a MapLiteral now.
                    let value = self.parse_expression()?;

                    if let Some(Token::Semicolon) = self.peek() {
                        self.advance();
                    }
                    statements.push(Statement::Let { name, ty, value });
                },
                Token::Identifier(_) => {
                    if self.is_variable_declaration_start() {
                        let ty = Some(self.parse_type()?);
                        let name = self.parse_identifier()?;
                        self.consume(Token::Assign)?;
                        let value = self.parse_expression()?;
                        if let Some(Token::Semicolon) = self.peek() {
                            self.advance();
                        }
                        statements.push(Statement::Let { name, ty, value });
                        continue;
                    }

                    let expr = self.parse_expression()?;

                    // Check for compound assignment operators (+=, -=, *=, /=, %=)
                    let compound_op = match self.peek() {
                        Some(Token::PlusAssign) => Some(BinaryOperator::Add),
                        Some(Token::MinusAssign) => Some(BinaryOperator::Sub),
                        Some(Token::MulAssign) => Some(BinaryOperator::Mul),
                        Some(Token::DivAssign) => Some(BinaryOperator::Div),
                        Some(Token::ModAssign) => Some(BinaryOperator::Mod),
                        _ => None,
                    };

                    if let Some(op) = compound_op {
                        self.advance(); // consume compound op
                        let rhs = self.parse_expression()?;
                        self.consume(Token::Semicolon)?;
                        // Desugar: target op= value → target = target op value
                        let desugared = Expression::BinaryOp {
                            left: Box::new(expr.clone()),
                            operator: op,
                            right: Box::new(rhs),
                        };
                        match expr {
                            Expression::Identifier(name) => {
                                statements.push(Statement::Assign { name, value: desugared });
                            },
                            Expression::IndexAccess { caller, index } => {
                                statements.push(Statement::IndexAssign { target: *caller, index: *index, value: desugared });
                            },
                            Expression::PropertyAccess { caller, property_name } => {
                                statements.push(Statement::PropertyAssign { target: *caller, property: property_name, value: desugared });
                            },
                            _ => return Err(ParseError::UnexpectedToken { expected: "Valid L-Value for compound assignment".to_string(), found: None, span: self.current_span() }),
                        }
                    } else if let Some(Token::Assign) = self.peek() {
                        self.advance(); // consume =
                        let value = self.parse_expression()?;
                        self.consume(Token::Semicolon)?;
                        match expr {
                            Expression::Identifier(name) => {
                                statements.push(Statement::Assign { name, value });
                            },
                            Expression::IndexAccess { caller, index } => {
                                statements.push(Statement::IndexAssign { target: *caller, index: *index, value });
                            },
                            Expression::PropertyAccess { caller, property_name } => {
                                statements.push(Statement::PropertyAssign { target: *caller, property: property_name, value });
                            },
                            _ => return Err(ParseError::UnexpectedToken { expected: "Valid L-Value".to_string(), found: Some(Token::Assign), span: self.current_span() }),
                        }
                    } else {
                        self.consume(Token::Semicolon)?;
                        statements.push(Statement::Expr(expr));
                    }
                },
                Token::Stream => {
                    self.advance();
                    let expr = self.parse_expression()?;
                    self.consume(Token::Semicolon)?;
                    statements.push(Statement::Stream(expr));
                },
                Token::Print => {
                    self.advance();
                    let expr = self.parse_expression()?;
                    self.consume(Token::Semicolon)?;
                    statements.push(Statement::Print(expr));
                },
                Token::Const => {
                    self.advance();
                    // const TYPE NAME = expr;  or  const var NAME = expr;
                    let ty = if let Some(Token::Var) = self.peek() {
                        self.advance();
                        None
                    } else {
                        Some(self.parse_type()?)
                    };
                    let name = self.parse_identifier()?;
                    self.consume(Token::Assign)?;
                    let value = self.parse_expression()?;
                    self.consume(Token::Semicolon)?;
                    statements.push(Statement::Const { name, ty, value });
                },
                Token::Break => {
                    self.advance();
                    self.consume(Token::Semicolon)?;
                    statements.push(Statement::Break);
                },
                Token::Continue => {
                    self.advance();
                    self.consume(Token::Semicolon)?;
                    statements.push(Statement::Continue);
                },
                Token::If => {
                    statements.push(self.parse_if_statement()?);
                },
                Token::While => {
                    self.advance();
                    self.consume(Token::LParen)?;
                    let condition = self.parse_expression()?;
                    self.consume(Token::RParen)?;
                    let body = self.parse_block()?;
                    statements.push(Statement::While { condition, body });
                },
                Token::For => {
                    self.advance();
                    self.consume(Token::LParen)?;
                    
                    let init_stmt = match self.peek() {
                        Some(Token::Var) | Some(Token::TypeInt) | Some(Token::TypeString) | Some(Token::TypeBool) | Some(Token::Identifier(_)) => {
                            let tok_init = self.peek().unwrap().clone();
                            let ty = if tok_init == Token::Var {
                                self.advance();
                                None
                            } else {
                                Some(self.parse_type()?)
                            };
                            let name = self.parse_identifier()?;
                            self.consume(Token::Assign)?;
                            let value = self.parse_expression()?;
                            Statement::Let { name, ty, value }
                        },
                        _ => {
                            let span = self.current_span();
                            return Err(ParseError::UnexpectedToken { expected: "For Loop Init Statement".to_string(), found: self.advance(), span });
                        },
                    };
                    self.consume(Token::Semicolon)?;

                    let condition = self.parse_expression()?;
                    self.consume(Token::Semicolon)?;

                    let update_name = self.parse_identifier()?;
                    self.consume(Token::Assign)?;
                    let update_value = self.parse_expression()?;
                    let update_stmt = Statement::Assign { name: update_name, value: update_value };
                    
                    self.consume(Token::RParen)?;
                    let body = self.parse_block()?;
                    
                    statements.push(Statement::For { 
                        init: Box::new(init_stmt), 
                        condition, 
                        update: Box::new(update_stmt), 
                        body 
                    });
                },
                Token::Foreach => {
                    self.advance();
                    self.consume(Token::LParen)?;
                    if let Some(Token::Var) = self.peek() {
                        self.advance();
                    }
                    let item_name = self.parse_identifier()?;
                    self.consume(Token::In)?;
                    let collection = self.parse_expression()?;
                    self.consume(Token::RParen)?;
                    let body = self.parse_block()?;
                    statements.push(Statement::Foreach { item_name, collection, body });
                },
                Token::Return => {
                    self.advance();
                    let expr = if let Some(Token::Semicolon) = self.peek() {
                        None
                    } else {
                        Some(self.parse_expression()?)
                    };
                    self.consume(Token::Semicolon)?;
                    statements.push(Statement::Return(expr));
                },
                Token::Unsafe => {
                    self.advance();
                    let unsafe_block = self.parse_block()?;
                    statements.push(Statement::UnsafeBlock(unsafe_block));
                },
                Token::QuestionMark | Token::Query => {
                    self.advance();
                    if let Some(Token::StringLiteral(query)) = self.advance() {
                        let expr = Expression::Query(SurrealQueryNode { raw_query: query.trim_matches('"').to_string() });
                        statements.push(Statement::Expr(expr));
                        if self.peek() == Some(&Token::Semicolon) { self.advance(); }
                    } else {
                        let span = self.current_span();
                        return Err(ParseError::UnexpectedToken { expected: "Query String".to_string(), found: self.advance(), span });
                    }
                },

                Token::Try => {
                    self.advance();
                    let try_block = self.parse_block()?;
                    self.consume(Token::Catch)?;
                    self.consume(Token::LParen)?;
                    let catch_var = self.parse_identifier()?;
                    self.consume(Token::RParen)?;
                    let catch_block = self.parse_block()?;
                    statements.push(Statement::TryCatch { try_block, catch_var, catch_block });
                },
                Token::Throw => {
                    self.advance();
                    let expr = self.parse_expression()?;
                    self.consume(Token::Semicolon)?;
                    statements.push(Statement::Throw(expr));
                },
                Token::Match => {
                    self.advance();
                    let subject = self.parse_expression()?;
                    self.consume(Token::LBrace)?;
                    let mut arms = Vec::new();
                    while self.peek() != Some(&Token::RBrace) {
                        let pattern = self.parse_pattern()?;
                        self.consume(Token::FatArrow)?;
                        let body = self.parse_block()?;
                        // Optional comma between arms
                        if self.peek() == Some(&Token::Comma) {
                            self.advance();
                        }
                        arms.push(MatchArm { pattern, body });
                    }
                    self.consume(Token::RBrace)?;
                    statements.push(Statement::Match { subject, arms });
                },
                // Plan 20: select { msg from agent => { ... } timeout(ms) => { ... } }
                Token::Select => {
                    self.advance();
                    self.consume(Token::LBrace)?;
                    let mut arms = Vec::new();
                    while self.peek() != Some(&Token::RBrace) {
                        // Check for timeout arm
                        if self.peek() == Some(&Token::Timeout) {
                            self.advance(); // consume 'timeout'
                            self.consume(Token::LParen)?;
                            let ms_expr = self.parse_expression()?;
                            self.consume(Token::RParen)?;
                            self.consume(Token::FatArrow)?;
                            let body = self.parse_block()?;
                            arms.push(SelectArm {
                                var_name: "_timeout".to_string(),
                                source: SelectSource::Timeout(ms_expr),
                                body,
                            });
                        } else {
                            // msg from agent_expr => { ... }
                            let var_name = self.parse_identifier()?;
                            self.consume(Token::From)?;
                            let source_expr = self.parse_expression()?;
                            self.consume(Token::FatArrow)?;
                            let body = self.parse_block()?;
                            arms.push(SelectArm {
                                var_name,
                                source: SelectSource::Agent(source_expr),
                                body,
                            });
                        }
                        // Optional comma between arms
                        if self.peek() == Some(&Token::Comma) {
                            self.advance();
                        }
                    }
                    self.consume(Token::RBrace)?;
                    statements.push(Statement::Select { arms });
                },
                // Fallback dummy token skipper to avoid infinite loops if an expression fails
                _ => {
                    let skipped = self.advance();
                    if skipped == Some(Token::Semicolon) {
                        continue;
                    }
                }
            }
        }
        
        self.consume(Token::RBrace)?;
        Ok(Block { statements })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        match self.peek() {
            Some(Token::Underscore) => {
                self.advance();
                Ok(Pattern::Wildcard)
            },
            Some(Token::IntLiteral(_)) => {
                if let Some(Token::IntLiteral(val)) = self.advance() {
                    Ok(Pattern::Literal(Expression::Int(val)))
                } else { unreachable!() }
            },
            Some(Token::StringLiteral(_)) => {
                if let Some(Token::StringLiteral(val)) = self.advance() {
                    Ok(Pattern::Literal(Expression::String(val.trim_matches('"').to_string())))
                } else { unreachable!() }
            },
            Some(Token::BoolLiteral(_)) => {
                if let Some(Token::BoolLiteral(val)) = self.advance() {
                    Ok(Pattern::Literal(Expression::Bool(val)))
                } else { unreachable!() }
            },
            Some(Token::Identifier(_)) => {
                let name = self.parse_identifier()?;
                if self.peek() == Some(&Token::LParen) {
                    self.advance();
                    let mut bindings = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        loop {
                            bindings.push(self.parse_identifier()?);
                            if self.peek() == Some(&Token::Comma) {
                                self.advance();
                            } else { break; }
                        }
                    }
                    self.consume(Token::RParen)?;
                    Ok(Pattern::Variant(name, bindings))
                } else {
                    // Simple identifier used as variant without bindings
                    Ok(Pattern::Variant(name, vec![]))
                }
            },
            _ => {
                let span = self.current_span();
                Err(ParseError::UnexpectedToken {
                    expected: "Pattern (literal, variant, or _)".to_string(),
                    found: self.advance(),
                    span,
                })
            }
        }
    }

    /// Try to parse lambda parameters after LParen has been consumed.
    /// Expects: type1 name1, type2 name2, ... ) =>
    /// Returns Ok(params) if this is a lambda (consumes through `=>`).
    /// Returns Err(()) if not a lambda — caller must restore self.pos.
    fn try_parse_lambda_params(&mut self) -> Result<Vec<FieldDecl>, ()> {
        let mut params = Vec::new();

        // Empty params: () =>
        if self.peek() == Some(&Token::RParen) {
            self.advance(); // consume )
            if self.peek() == Some(&Token::FatArrow) {
                self.advance(); // consume =>
                return Ok(params);
            }
            return Err(());
        }

        // Try parsing typed params: (int a, string b, ...)
        loop {
            let ty = self.parse_type().map_err(|_| ())?;
            let name = self.parse_identifier().map_err(|_| ())?;
            params.push(FieldDecl { name, ty });

            match self.peek() {
                Some(Token::Comma) => { self.advance(); }
                Some(Token::RParen) => {
                    self.advance(); // consume )
                    if self.peek() == Some(&Token::FatArrow) {
                        self.advance(); // consume =>
                        return Ok(params);
                    }
                    return Err(());
                }
                _ => return Err(()),
            }
        }
    }

    /// Returns (left_bp, right_bp) for a binary operator token, or None if not an operator.
    fn token_binding_power(tok: &Token) -> Option<(u8, u8)> {
        match tok {
            Token::Tilde                              => Some((1, 2)),   // cosine_sim: lowest
            Token::Or                                  => Some((3, 4)),  // ||
            Token::And                                 => Some((5, 6)),  // &&
            Token::Equals | Token::NotEquals           => Some((7, 8)),  // == !=
            Token::LessThan | Token::GreaterThan |
            Token::LessOrEqual | Token::GreaterOrEqual => Some((9, 10)), // < > <= >=
            Token::Plus | Token::Minus                 => Some((11, 12)), // + -
            Token::Multiply | Token::Divide |
            Token::Percent                             => Some((13, 14)), // * / %
            _ => None,
        }
    }

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_expression_bp(0)
    }

    fn parse_expression_bp(&mut self, min_bp: u8) -> Result<Expression, ParseError> {
        let mut left = match self.advance() {
            // Unary prefix operators: -expr, !expr
            Some(Token::Minus) => {
                let operand = self.parse_expression_bp(15)?; // high precedence for prefix
                Expression::UnaryOp {
                    operator: UnaryOperator::Negate,
                    operand: Box::new(operand),
                }
            },
            Some(Token::Bang) => {
                let operand = self.parse_expression_bp(15)?;
                Expression::UnaryOp {
                    operator: UnaryOperator::Not,
                    operand: Box::new(operand),
                }
            },
            Some(Token::Await) => {
                let operand = self.parse_expression_bp(15)?;
                Expression::Await(Box::new(operand))
            },
            Some(Token::Null) => Expression::Null,
            Some(Token::IntLiteral(val)) => Expression::Int(val),
            Some(Token::StringLiteral(val)) => Expression::String(val.trim_matches('"').to_string()),
            Some(Token::PromptLiteralToken(val)) => {
                let inner = val.trim_start_matches("prompt").trim().trim_matches('"').to_string();
                Expression::PromptLiteral(inner)
            },
            Some(Token::BoolLiteral(val)) => Expression::Bool(val),
            Some(Token::Identifier(name)) => Expression::Identifier(name),
            Some(Token::LBracket) => {
                let mut elements = Vec::new();
                if self.peek() != Some(&Token::RBracket) {
                    loop {
                        elements.push(self.parse_expression()?);
                        if self.peek() == Some(&Token::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.consume(Token::RBracket)?;
                Expression::ArrayLiteral(elements)
            },
            Some(Token::LBrace) => {
                let mut entries = Vec::new();
                if self.peek() != Some(&Token::RBrace) {
                    loop {
                        let key = self.parse_expression()?;
                        self.consume(Token::Colon)?;
                        let value = self.parse_expression()?;
                        entries.push((key, value));
                        if self.peek() == Some(&Token::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.consume(Token::RBrace)?;
                Expression::MapLiteral(entries)
            },
            Some(Token::From) => {
                let from_var = self.parse_identifier()?;
                self.consume(Token::In)?;
                let in_collection = Box::new(self.parse_expression()?);
                
                let mut where_clause = None;
                if let Some(Token::Where) = self.peek() {
                    self.advance();
                    where_clause = Some(Box::new(self.parse_expression()?));
                }

                let mut orderby_clause = None;
                let mut descending = false;
                if let Some(Token::Orderby) = self.peek() {
                    self.advance();
                    orderby_clause = Some(Box::new(self.parse_expression()?));
                    if let Some(Token::Descending) = self.peek() {
                        self.advance();
                        descending = true;
                    }
                }

                self.consume(Token::Select)?;
                let select_clause = Box::new(self.parse_expression()?);

                return Ok(Expression::Linq(LinqQuery {
                    from_var,
                    in_collection,
                    where_clause,
                    orderby_clause,
                    descending,
                    select_clause
                }));
            },
            Some(Token::Query) => {
                if let Some(Token::StringLiteral(query)) = self.advance() {
                    Expression::Query(SurrealQueryNode { raw_query: query.trim_matches('"').to_string() })
                } else {
                    return Err(ParseError::UnexpectedToken {
                        expected: "Query String".to_string(),
                        found: self.advance(),
                        span: self.last_span(),
                    });
                }
            },
            Some(Token::QuestionMark) => {
                if let Some(Token::StringLiteral(query)) = self.advance() {
                    Expression::Query(SurrealQueryNode { raw_query: query.trim_matches('"').to_string() })
                } else {
                    return Err(ParseError::UnexpectedToken {
                        expected: "Query String".to_string(),
                        found: self.advance(),
                        span: self.last_span(),
                    });
                }
            },
            // Plan 06: Lambda expressions or parenthesized grouping
            Some(Token::LParen) => {
                let saved_pos = self.pos;
                match self.try_parse_lambda_params() {
                    Ok(params) => {
                        // Successfully parsed lambda params and =>
                        if self.peek() == Some(&Token::LBrace) {
                            let body = self.parse_block()?;
                            Expression::Lambda { params, return_ty: None, body: Box::new(LambdaBody::Block(body)) }
                        } else {
                            let expr = self.parse_expression()?;
                            Expression::Lambda { params, return_ty: None, body: Box::new(LambdaBody::Expression(expr)) }
                        }
                    }
                    Err(_) => {
                        // Not a lambda — parse as parenthesized expression
                        self.pos = saved_pos;
                        let expr = self.parse_expression()?;
                        self.consume(Token::RParen)?;
                        expr
                    }
                }
            },
            // OCAP capability tokens as expressions (for token passing)
            Some(Token::NetworkAccess) => Expression::Identifier("NetworkAccess".to_string()),
            Some(Token::FileAccess) => Expression::Identifier("FileAccess".to_string()),
            Some(Token::DbAccess) => Expression::Identifier("DbAccess".to_string()),
            Some(Token::LlmAccess) => Expression::Identifier("LlmAccess".to_string()),
            Some(Token::SystemAccess) => Expression::Identifier("SystemAccess".to_string()),
            Some(Token::FButton) => {
                if let Some(Token::LParen) = self.advance() {
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        loop {
                            args.push(self.parse_expression()?);
                            if self.peek() == Some(&Token::Comma) { self.advance(); } else { break; }
                        }
                    }
                    self.consume(Token::RParen)?;
                    Expression::MethodCall {
                        caller: Box::new(Expression::Identifier("self".to_string())),
                        method_name: "fetch".to_string(),
                        args
                    }
                } else {
                    return Err(ParseError::UnexpectedToken {
                        expected: "( for F fetch caller".to_string(),
                        found: self.peek().cloned(),
                        span: self.last_span(),
                    });
                }
            },
            // Wave 6: retry(N) { body } fallback { fallback_body }
            Some(Token::Retry) => {
                self.consume(Token::LParen)?;
                let max_attempts = self.parse_expression()?;
                self.consume(Token::RParen)?;
                let body = self.parse_block()?;
                let fallback = if self.peek() == Some(&Token::Fallback) {
                    self.advance();
                    Some(Box::new(self.parse_block()?))
                } else {
                    None
                };
                Expression::Retry {
                    max_attempts: Box::new(max_attempts),
                    body: Box::new(body),
                    fallback,
                }
            },
            // Wave 6: spawn Agent(args)
            Some(Token::Spawn) => {
                let agent_name = self.parse_identifier()?;
                self.consume(Token::LParen)?;
                let mut args = Vec::new();
                if self.peek() != Some(&Token::RParen) {
                    loop {
                        args.push(self.parse_expression()?);
                        if self.peek() == Some(&Token::Comma) { self.advance(); } else { break; }
                    }
                }
                self.consume(Token::RParen)?;
                Expression::Spawn { agent_name, args }
            },
            Some(t) => return Err(ParseError::UnexpectedToken {
                expected: "Expression Literal/Identifier/From".to_string(),
                found: Some(t),
                span: self.last_span(),
            }),
            None => return Err(ParseError::UnexpectedEof),
        };

        // Pratt parser loop: binary ops (precedence climbing) + postfix (. [] call)
        while let Some(tok) = self.peek() {
            // Check for binary operator with precedence
            if let Some((l_bp, r_bp)) = Self::token_binding_power(tok) {
                if l_bp < min_bp {
                    break;
                }
                let op = match self.advance().unwrap() {
                    Token::Plus => BinaryOperator::Add,
                    Token::Minus => BinaryOperator::Sub,
                    Token::Multiply => BinaryOperator::Mul,
                    Token::Divide => BinaryOperator::Div,
                    Token::Percent => BinaryOperator::Mod,
                    Token::Equals => BinaryOperator::Eq,
                    Token::NotEquals => BinaryOperator::NotEq,
                    Token::LessThan => BinaryOperator::Lt,
                    Token::GreaterThan => BinaryOperator::Gt,
                    Token::LessOrEqual => BinaryOperator::LtEq,
                    Token::GreaterOrEqual => BinaryOperator::GtEq,
                    Token::And => BinaryOperator::And,
                    Token::Or => BinaryOperator::Or,
                    Token::Tilde => BinaryOperator::CosineSim,
                    _ => unreachable!(),
                };
                let right = self.parse_expression_bp(r_bp)?;
                left = Expression::BinaryOp {
                    left: Box::new(left),
                    operator: op,
                    right: Box::new(right),
                };
                continue;
            }

            match tok {
                Token::Dot => {
                    self.advance(); // consume dot
                    let prop = self.parse_identifier()?;
                    if let Some(Token::LParen) = self.peek() {
                        self.advance();
                        let mut args = Vec::new();
                        if self.peek() != Some(&Token::RParen) {
                            loop {
                                args.push(self.parse_expression()?);
                                if self.peek() == Some(&Token::Comma) {
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
                        }
                        self.consume(Token::RParen)?;
                        left = Expression::MethodCall {
                            caller: Box::new(left),
                            method_name: prop,
                            args,
                        };
                    } else {
                        left = Expression::PropertyAccess {
                            caller: Box::new(left),
                            property_name: prop,
                        };
                    }
                },
                Token::LBracket => {
                    self.advance(); // consume [
                    let index = self.parse_expression()?;
                    self.consume(Token::RBracket)?;
                    left = Expression::IndexAccess {
                        caller: Box::new(left),
                        index: Box::new(index),
                    };
                },
                Token::LParen => {
                    self.advance(); // consume LParen
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        loop {
                            args.push(self.parse_expression()?);
                            if self.peek() == Some(&Token::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.consume(Token::RParen)?;
                    if let Expression::Identifier(method_name) = left {
                        left = Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name,
                            args,
                        };
                    } else {
                        return Err(ParseError::UnexpectedToken { expected: "Identifier before call".to_string(), found: Some(Token::LParen), span: self.last_span() });
                    }
                },
                // Pipe operator: a |> f(b) → f(a, b), a |> .method() → a.method()
                Token::Pipe => {
                    self.advance(); // consume |>
                    if self.peek() == Some(&Token::Dot) {
                        // a |> .method(args) → a.method(args)
                        self.advance(); // consume .
                        let method = self.parse_identifier()?;
                        self.consume(Token::LParen)?;
                        let mut args = Vec::new();
                        if self.peek() != Some(&Token::RParen) {
                            loop {
                                args.push(self.parse_expression()?);
                                if self.peek() == Some(&Token::Comma) { self.advance(); } else { break; }
                            }
                        }
                        self.consume(Token::RParen)?;
                        left = Expression::MethodCall {
                            caller: Box::new(left),
                            method_name: method,
                            args,
                        };
                    } else {
                        // a |> f(b, c) → f(a, b, c)
                        let func_name = self.parse_identifier()?;
                        self.consume(Token::LParen)?;
                        let mut args = vec![left];
                        if self.peek() != Some(&Token::RParen) {
                            loop {
                                args.push(self.parse_expression()?);
                                if self.peek() == Some(&Token::Comma) { self.advance(); } else { break; }
                            }
                        }
                        self.consume(Token::RParen)?;
                        left = Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: func_name,
                            args,
                        };
                    }
                },
                _ => break,
            }
        }

        Ok(left)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_multi_arg_annotation() {
        let source = r#"
            @[CliCommand("fetch_url", "Fetches data from a URL")]
            +A ToolAgent {
                +M Fetch(s url) {}
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();

        match &program.items[0] {
            Item::Agent(a) => {
                assert_eq!(a.annotations.len(), 1);
                assert_eq!(a.annotations[0].name, "CliCommand");
                assert_eq!(a.annotations[0].values, vec!["fetch_url".to_string(), "Fetches data from a URL".to_string()]);
            },
            _ => panic!("Expected Agent"),
        }
    }

    #[test]
    fn test_parse_contract_with_methods() {
        let source = r#"
            @target("NPU") public contract Searcher { 
                public Result<string, Error> Find(Prompt q);
                public void Heartbeat();
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();

        match &program.items[0] {
            Item::Contract(contract) => {
                assert_eq!(contract.name, "Searcher");
                assert_eq!(contract.methods.len(), 2);
                
                let m1 = &contract.methods[0];
                assert_eq!(m1.name, "Find");
                assert_eq!(m1.args.len(), 1);
                assert_eq!(m1.args[0].name, "q");
                assert_eq!(m1.args[0].ty, TypeNode::Prompt);
                
                if let Some(TypeNode::Result(ok, err)) = &m1.return_ty {
                    assert_eq!(**ok, TypeNode::String);
                    assert_eq!(**err, TypeNode::Custom("Error".to_string()));
                } else {
                    panic!("Expected Result type");
                }

                let m2 = &contract.methods[1];
                assert_eq!(m2.name, "Heartbeat");
                assert_eq!(m2.return_ty, Some(TypeNode::Void));
            },
            _ => panic!("Expected Contract"),
        }
    }

    #[test]
    fn test_parse_struct_fields() {
        let source = r#"
            public struct MemoryContext { 
                public Tensor embedding;
                private string content_hash;
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();

        match &program.items[0] {
            Item::Struct(s) => {
                assert_eq!(s.name, "MemoryContext");
                assert!(s.is_public);
                assert_eq!(s.fields.len(), 2);
                
                assert_eq!(s.fields[0].name, "embedding");
                assert_eq!(s.fields[0].ty, TypeNode::Tensor);

                assert_eq!(s.fields[1].name, "content_hash");
                assert_eq!(s.fields[1].ty, TypeNode::String);
            },
            _ => panic!("Expected Struct"),
        }
    }

    #[test]
    fn test_parse_full_agent_with_statements() {
        let source = r#"
            system agent Brain {
                public void Think() {
                    string idea = "varg";
                    unsafe {
                        query "SELECT * FROM memories";
                    }
                    return;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();

        match &program.items[0] {
            Item::Agent(agent) => {
                assert_eq!(agent.name, "Brain");
                assert!(agent.is_system);
                let method = &agent.methods[0];
                assert_eq!(method.name, "Think");
                
                let body = method.body.as_ref().unwrap();
                assert_eq!(body.statements.len(), 3);
                
                // Let statement (string idea = "varg";)
                if let Statement::Let { name, ty, value } = &body.statements[0] {
                    assert_eq!(name, "idea");
                    assert_eq!(*ty, Some(TypeNode::String));
                    assert_eq!(*value, Expression::String("varg".to_string()));
                } else { panic!("Expected Let"); }

                // Unsafe statement
                if let Statement::UnsafeBlock(ub) = &body.statements[1] {
                     if let Statement::Expr(Expression::Query(q)) = &ub.statements[0] {
                         assert_eq!(q.raw_query, "SELECT * FROM memories");
                     } else { panic!("Expected Query in Unsafe") }
                } else { panic!("Expected Unsafe block"); }

                // Return statement
                if let Statement::Return(expr) = &body.statements[2] {
                    assert!(expr.is_none());
                } else { panic!("Expected Return"); }
            },
            _ => panic!("Expected Agent"),
        }
    }

    #[test]
    fn test_parse_try_catch() {
        let source = r#"
            agent TestAgent {
                public void DoWork() {
                    try {
                        throw "Failed";
                    } catch (err) {
                        print err;
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();

        if let Item::Agent(a) = &program.items[0] {
            let m = &a.methods[0];
            let body = m.body.as_ref().unwrap();
            if let Statement::TryCatch { try_block, catch_var, catch_block } = &body.statements[0] {
                assert_eq!(catch_var, "err");
                if let Statement::Throw(expr) = &try_block.statements[0] {
                    assert_eq!(*expr, Expression::String("Failed".to_string()));
                } else { panic!("Expected Throw"); }
                
                if let Statement::Print(expr) = &catch_block.statements[0] {
                    assert_eq!(*expr, Expression::Identifier("err".to_string()));
                } else { panic!("Expected Print"); }
            } else { panic!("Expected TryCatch"); }
        } else { panic!("Expected Agent"); }
    }

    // ---- Plan 08: Extended Parser Coverage ----

    #[test]
    fn test_parse_empty_agent() {
        let source = "agent Empty { }";
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            assert_eq!(a.name, "Empty");
            assert!(a.methods.is_empty());
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_import() {
        let source = r#"import "std/crypto";"#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Import(path) = &program.items[0] {
            assert_eq!(path, "std/crypto");
        } else { panic!("Expected Import"); }
    }

    #[test]
    fn test_parse_while_loop() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    int count = 0;
                    while count < 10 {
                        count = count + 1;
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::While { condition, body: loop_body } = &body.statements[1] {
                assert!(matches!(condition, Expression::BinaryOp { operator: BinaryOperator::Lt, .. }));
                assert_eq!(loop_body.statements.len(), 1);
            } else { panic!("Expected While"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_if_else() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    if true {
                        print "yes";
                    } else {
                        print "no";
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::If { condition, then_block, else_block } = &body.statements[0] {
                assert_eq!(*condition, Expression::Bool(true));
                assert_eq!(then_block.statements.len(), 1);
                assert!(else_block.is_some());
                assert_eq!(else_block.as_ref().unwrap().statements.len(), 1);
            } else { panic!("Expected If"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_foreach() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    foreach item in items {
                        print item;
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Foreach { item_name, collection, .. } = &body.statements[0] {
                assert_eq!(item_name, "item");
                assert_eq!(*collection, Expression::Identifier("items".to_string()));
            } else { panic!("Expected Foreach"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_method_call_with_args() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    var result = self.Process("data", 42);
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::MethodCall { caller, method_name, args } = value {
                    assert_eq!(method_name, "Process");
                    assert_eq!(args.len(), 2);
                    assert!(matches!(&**caller, Expression::Identifier(n) if n == "self"));
                } else { panic!("Expected MethodCall"); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_property_access() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    var name = obj.name;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::PropertyAccess { caller, property_name } = value {
                    assert_eq!(property_name, "name");
                    assert!(matches!(&**caller, Expression::Identifier(n) if n == "obj"));
                } else { panic!("Expected PropertyAccess"); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_array_literal() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    var items = [1, 2, 3];
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::ArrayLiteral(elements) = value {
                    assert_eq!(elements.len(), 3);
                    assert_eq!(elements[0], Expression::Int(1));
                    assert_eq!(elements[1], Expression::Int(2));
                    assert_eq!(elements[2], Expression::Int(3));
                } else { panic!("Expected ArrayLiteral"); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_map_literal() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    var config = {"host": "localhost", "port": 8080};
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::MapLiteral(entries) = value {
                    assert_eq!(entries.len(), 2);
                } else { panic!("Expected MapLiteral"); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_index_access() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    var val = items[0];
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::IndexAccess { caller, index } = value {
                    assert!(matches!(&**caller, Expression::Identifier(n) if n == "items"));
                    assert_eq!(**index, Expression::Int(0));
                } else { panic!("Expected IndexAccess"); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_binary_ops() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    var sum = 1 + 2;
                    var diff = 10 - 5;
                    var eq = 1 == 1;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            // sum
            if let Statement::Let { value, .. } = &body.statements[0] {
                assert!(matches!(value, Expression::BinaryOp { operator: BinaryOperator::Add, .. }));
            } else { panic!("Expected Let"); }
            // diff
            if let Statement::Let { value, .. } = &body.statements[1] {
                assert!(matches!(value, Expression::BinaryOp { operator: BinaryOperator::Sub, .. }));
            } else { panic!("Expected Let"); }
            // eq
            if let Statement::Let { value, .. } = &body.statements[2] {
                assert!(matches!(value, Expression::BinaryOp { operator: BinaryOperator::Eq, .. }));
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_print_statement() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    print "hello";
                    print 42;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            assert_eq!(body.statements.len(), 2);
            if let Statement::Print(expr) = &body.statements[0] {
                assert_eq!(*expr, Expression::String("hello".to_string()));
            } else { panic!("Expected Print"); }
            if let Statement::Print(expr) = &body.statements[1] {
                assert_eq!(*expr, Expression::Int(42));
            } else { panic!("Expected Print"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_stream_statement() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    stream "streaming output";
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Stream(expr) = &body.statements[0] {
                assert_eq!(*expr, Expression::String("streaming output".to_string()));
            } else { panic!("Expected Stream"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_multiple_methods() {
        let source = r#"
            agent MultiAgent {
                public void First() { return; }
                public string Second(int count) { return "done"; }
                private void Third() { }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            assert_eq!(a.methods.len(), 3);
            assert_eq!(a.methods[0].name, "First");
            assert!(a.methods[0].is_public);
            assert_eq!(a.methods[1].name, "Second");
            assert_eq!(a.methods[1].return_ty, Some(TypeNode::String));
            assert_eq!(a.methods[1].args.len(), 1);
            assert_eq!(a.methods[2].name, "Third");
            assert!(!a.methods[2].is_public);
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_var_type_inference() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    var name = "hello";
                    var count = 42;
                    var flag = true;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            assert_eq!(body.statements.len(), 3);
            // var declarations have ty = None
            for stmt in &body.statements {
                if let Statement::Let { ty, .. } = stmt {
                    assert_eq!(*ty, None);
                } else { panic!("Expected Let"); }
            }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_error_unexpected_eof() {
        let source = "agent Broken {";
        let mut parser = Parser::new(source);
        let result = parser.parse_program();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_query_expression() {
        let source = r#"
            agent DbAgent {
                public void Run() {
                    unsafe {
                        var result = query "SELECT * FROM users";
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::UnsafeBlock(ub) = &body.statements[0] {
                if let Statement::Let { value, .. } = &ub.statements[0] {
                    if let Expression::Query(q) = value {
                        assert_eq!(q.raw_query, "SELECT * FROM users");
                    } else { panic!("Expected Query"); }
                } else { panic!("Expected Let"); }
            } else { panic!("Expected Unsafe"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_return_with_value() {
        let source = r#"
            agent TestAgent {
                public int Add(int x, int y) {
                    return x + y;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let method = &a.methods[0];
            assert_eq!(method.name, "Add");
            assert_eq!(method.args.len(), 2);
            assert_eq!(method.return_ty, Some(TypeNode::Int));
            let body = method.body.as_ref().unwrap();
            if let Statement::Return(Some(expr)) = &body.statements[0] {
                assert!(matches!(expr, Expression::BinaryOp { operator: BinaryOperator::Add, .. }));
            } else { panic!("Expected Return with value"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_no_std() {
        let source = r#"
            #![no_std]
            agent Bare { }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        assert!(program.no_std);
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn test_parse_linq_query() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    var result = from item in items where item > 5 select item;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::Linq(q) = value {
                    assert_eq!(q.from_var, "item");
                    assert!(q.where_clause.is_some());
                    assert!(!q.descending);
                } else { panic!("Expected Linq"); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_generic_method() {
        let source = r#"
            agent TestAgent {
                public void Process<T>(List<T> items) {
                    return;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let method = &a.methods[0];
            assert_eq!(method.type_params, vec!["T".to_string()]);
            assert_eq!(method.args.len(), 1);
            assert_eq!(method.args[0].ty, TypeNode::List(Box::new(TypeNode::TypeVar("T".to_string()))));
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_struct_with_generics() {
        let source = r#"
            public struct Container<T> {
                public T value;
                public int count;
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Struct(s) = &program.items[0] {
            assert_eq!(s.name, "Container");
            assert_eq!(s.type_params, vec!["T".to_string()]);
            assert_eq!(s.fields.len(), 2);
        } else { panic!("Expected Struct"); }
    }

    #[test]
    fn test_parse_assign_statement() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    int count = 0;
                    count = 42;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Assign { name, value } = &body.statements[1] {
                assert_eq!(name, "count");
                assert_eq!(*value, Expression::Int(42));
            } else { panic!("Expected Assign"); }
        } else { panic!("Expected Agent"); }
    }

    // ---- Plan 07: Type System Tests ----

    #[test]
    fn test_parse_enum_simple() {
        let source = r#"
            public enum Status {
                Active,
                Inactive,
                Pending,
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();

        if let Item::Enum(e) = &program.items[0] {
            assert_eq!(e.name, "Status");
            assert!(e.is_public);
            assert_eq!(e.variants.len(), 3);
            assert_eq!(e.variants[0].name, "Active");
            assert_eq!(e.variants[1].name, "Inactive");
            assert_eq!(e.variants[2].name, "Pending");
            assert!(e.variants[0].fields.is_empty());
        } else { panic!("Expected Enum"); }
    }

    #[test]
    fn test_parse_enum_with_fields() {
        let source = r#"
            enum ApiResponse {
                Ok(string value),
                Err(string message, int code),
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();

        if let Item::Enum(e) = &program.items[0] {
            assert_eq!(e.name, "ApiResponse");
            assert!(!e.is_public);
            assert_eq!(e.variants.len(), 2);
            assert_eq!(e.variants[0].name, "Ok");
            assert_eq!(e.variants[0].fields.len(), 1);
            assert_eq!(e.variants[0].fields[0].0, "value");
            assert_eq!(e.variants[0].fields[0].1, TypeNode::String);
            assert_eq!(e.variants[1].name, "Err");
            assert_eq!(e.variants[1].fields.len(), 2);
        } else { panic!("Expected Enum"); }
    }

    #[test]
    fn test_parse_type_alias() {
        let source = r#"
            type UserId = string;
            type Score = int;
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();

        if let Item::TypeAlias { name, target } = &program.items[0] {
            assert_eq!(name, "UserId");
            assert_eq!(*target, TypeNode::String);
        } else { panic!("Expected TypeAlias"); }

        if let Item::TypeAlias { name, target } = &program.items[1] {
            assert_eq!(name, "Score");
            assert_eq!(*target, TypeNode::Int);
        } else { panic!("Expected TypeAlias"); }
    }

    #[test]
    fn test_parse_nullable_type() {
        let source = r#"
            agent TestAgent {
                public void Run() {
                    string? name = null;
                    int? count = 42;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();

        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { name, ty, value } = &body.statements[0] {
                assert_eq!(name, "name");
                assert_eq!(*ty, Some(TypeNode::Nullable(Box::new(TypeNode::String))));
                assert_eq!(*value, Expression::Null);
            } else { panic!("Expected Let"); }

            if let Statement::Let { name, ty, value } = &body.statements[1] {
                assert_eq!(name, "count");
                assert_eq!(*ty, Some(TypeNode::Nullable(Box::new(TypeNode::Int))));
                assert_eq!(*value, Expression::Int(42));
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_where_clause() {
        let source = r#"
            agent SortAgent {
                public void Sort<T>(List<T> items) where T : Comparable {
                    return;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();

        if let Item::Agent(a) = &program.items[0] {
            let m = &a.methods[0];
            assert_eq!(m.name, "Sort");
            assert_eq!(m.type_params, vec!["T".to_string()]);
            assert_eq!(m.constraints.len(), 1);
            assert_eq!(m.constraints[0].type_param, "T");
            assert_eq!(m.constraints[0].bound, "Comparable");
        } else { panic!("Expected Agent"); }
    }

    // ---- Plan 03/06: Wave 3 Parser Tests ----

    #[test]
    fn test_parse_capability_type_in_method() {
        let source = r#"
            agent ApiAgent {
                public string Fetch(string url, NetworkAccess net) {
                    return "ok";
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let m = &a.methods[0];
            assert_eq!(m.args.len(), 2);
            assert_eq!(m.args[0].name, "url");
            assert_eq!(m.args[0].ty, TypeNode::String);
            assert_eq!(m.args[1].name, "net");
            assert_eq!(m.args[1].ty, TypeNode::Capability(CapabilityType::NetworkAccess));
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_all_capability_types() {
        let source = r#"
            agent Test {
                public void Run(NetworkAccess a, FileAccess b, DbAccess c, LlmAccess d, SystemAccess e) {
                    return;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let m = &a.methods[0];
            assert_eq!(m.args.len(), 5);
            assert_eq!(m.args[0].ty, TypeNode::Capability(CapabilityType::NetworkAccess));
            assert_eq!(m.args[1].ty, TypeNode::Capability(CapabilityType::FileAccess));
            assert_eq!(m.args[2].ty, TypeNode::Capability(CapabilityType::DbAccess));
            assert_eq!(m.args[3].ty, TypeNode::Capability(CapabilityType::LlmAccess));
            assert_eq!(m.args[4].ty, TypeNode::Capability(CapabilityType::SystemAccess));
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_match_statement() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = 42;
                    match x {
                        1 => { print("one"); }
                        2 => { print("two"); }
                        _ => { print("other"); }
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            // First statement is let, second is match
            if let Statement::Match { subject, arms } = &body.statements[1] {
                assert!(matches!(subject, Expression::Identifier(n) if n == "x"));
                assert_eq!(arms.len(), 3);
                assert!(matches!(&arms[0].pattern, Pattern::Literal(Expression::Int(1))));
                assert!(matches!(&arms[1].pattern, Pattern::Literal(Expression::Int(2))));
                assert!(matches!(&arms[2].pattern, Pattern::Wildcard));
            } else { panic!("Expected Match statement"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_match_with_variant_pattern() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = 1;
                    match x {
                        Some(val) => { print(val); }
                        None => { print("nothing"); }
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Match { arms, .. } = &body.statements[1] {
                assert_eq!(arms.len(), 2);
                if let Pattern::Variant(name, bindings) = &arms[0].pattern {
                    assert_eq!(name, "Some");
                    assert_eq!(bindings, &vec!["val".to_string()]);
                } else { panic!("Expected Variant pattern"); }
                if let Pattern::Variant(name, bindings) = &arms[1].pattern {
                    assert_eq!(name, "None");
                    assert!(bindings.is_empty());
                } else { panic!("Expected Variant pattern for None"); }
            } else { panic!("Expected Match statement"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_match_with_string_pattern() {
        let source = r#"
            agent Test {
                public void Run() {
                    var cmd = "help";
                    match cmd {
                        "help" => { print("showing help"); }
                        "quit" => { print("quitting"); }
                        _ => { print("unknown"); }
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Match { arms, .. } = &body.statements[1] {
                assert_eq!(arms.len(), 3);
                assert!(matches!(&arms[0].pattern, Pattern::Literal(Expression::String(s)) if s == "help"));
                assert!(matches!(&arms[1].pattern, Pattern::Literal(Expression::String(s)) if s == "quit"));
                assert!(matches!(&arms[2].pattern, Pattern::Wildcard));
            } else { panic!("Expected Match statement"); }
        } else { panic!("Expected Agent"); }
    }

    // ===== Plan 06: Lambda Parsing from Source =====

    #[test]
    fn test_parse_lambda_expression_body() {
        let source = r#"
        agent TestAgent {
            public void Run() {
                var add = (int a, int b) => a + b;
            }
        }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value: Expression::Lambda { params, body: lambda_body, .. }, .. } = &body.statements[0] {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].name, "a");
                assert_eq!(params[0].ty, TypeNode::Int);
                assert_eq!(params[1].name, "b");
                assert_eq!(params[1].ty, TypeNode::Int);
                assert!(matches!(lambda_body.as_ref(), LambdaBody::Expression(_)));
            } else { panic!("Expected Let with Lambda expression"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_lambda_block_body() {
        let source = r#"
        agent TestAgent {
            public void Run() {
                var process = (string input) => {
                    return input;
                };
            }
        }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value: Expression::Lambda { params, body: lambda_body, .. }, .. } = &body.statements[0] {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name, "input");
                assert_eq!(params[0].ty, TypeNode::String);
                assert!(matches!(lambda_body.as_ref(), LambdaBody::Block(_)));
            } else { panic!("Expected Let with Lambda block body"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_lambda_empty_params() {
        let source = r#"
        agent TestAgent {
            public void Run() {
                var greet = () => 42;
            }
        }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value: Expression::Lambda { params, .. }, .. } = &body.statements[0] {
                assert_eq!(params.len(), 0);
            } else { panic!("Expected Let with Lambda"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_parenthesized_expression() {
        // Ensure (a + b) is NOT parsed as a lambda
        let source = r#"
        agent TestAgent {
            public void Run() {
                var x = (42);
            }
        }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value: Expression::Int(42), .. } = &body.statements[0] {
                // Parenthesized expression correctly parsed as Int(42)
            } else { panic!("Expected Let with Int(42), got: {:?}", body.statements[0]); }
        } else { panic!("Expected Agent"); }
    }

    // ===== Plan 06: Destructuring Parsing =====

    #[test]
    fn test_parse_tuple_destructuring() {
        let source = r#"
        agent TestAgent {
            public void Run() {
                var (name, age) = GetPerson();
            }
        }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::LetDestructure { pattern: DestructurePattern::Tuple(names), .. } = &body.statements[0] {
                assert_eq!(names, &vec!["name".to_string(), "age".to_string()]);
            } else { panic!("Expected LetDestructure Tuple, got: {:?}", body.statements[0]); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_struct_destructuring() {
        let source = r#"
        agent TestAgent {
            public void Run() {
                var { name, age } = person;
            }
        }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::LetDestructure { pattern: DestructurePattern::Struct(fields), .. } = &body.statements[0] {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0], ("name".to_string(), None));
                assert_eq!(fields[1], ("age".to_string(), None));
            } else { panic!("Expected LetDestructure Struct, got: {:?}", body.statements[0]); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_struct_destructuring_with_alias() {
        let source = r#"
        agent TestAgent {
            public void Run() {
                var { name: n, age: a } = person;
            }
        }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::LetDestructure { pattern: DestructurePattern::Struct(fields), .. } = &body.statements[0] {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0], ("name".to_string(), Some("n".to_string())));
                assert_eq!(fields[1], ("age".to_string(), Some("a".to_string())));
            } else { panic!("Expected LetDestructure Struct with aliases"); }
        } else { panic!("Expected Agent"); }
    }

    // ===== Stabilization: Operator Precedence Tests =====

    #[test]
    fn test_precedence_mul_over_add() {
        // 1 + 2 * 3  should parse as  1 + (2 * 3)
        let source = r#"
            agent Test {
                public void Run() {
                    var x = 1 + 2 * 3;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                // Outer should be Add, right should be Mul
                if let Expression::BinaryOp { operator: BinaryOperator::Add, right, .. } = value {
                    if let Expression::BinaryOp { operator: BinaryOperator::Mul, .. } = right.as_ref() {
                        // correct!
                    } else { panic!("Expected right side to be Mul, got {:?}", right); }
                } else { panic!("Expected Add at top level, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_precedence_comparison_lower_than_arithmetic() {
        // a + 1 == b * 2  should parse as  (a + 1) == (b * 2)
        let source = r#"
            agent Test {
                public void Run() {
                    var x = a + 1 == b * 2;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::BinaryOp { operator: BinaryOperator::Eq, left, right } = value {
                    assert!(matches!(left.as_ref(), Expression::BinaryOp { operator: BinaryOperator::Add, .. }));
                    assert!(matches!(right.as_ref(), Expression::BinaryOp { operator: BinaryOperator::Mul, .. }));
                } else { panic!("Expected Eq at top level, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_precedence_left_associative() {
        // 1 - 2 - 3  should parse as  (1 - 2) - 3
        let source = r#"
            agent Test {
                public void Run() {
                    var x = 1 - 2 - 3;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                // Outer Sub, left should be Sub(1,2), right should be 3
                if let Expression::BinaryOp { operator: BinaryOperator::Sub, left, right } = value {
                    assert!(matches!(left.as_ref(), Expression::BinaryOp { operator: BinaryOperator::Sub, .. }));
                    assert_eq!(**right, Expression::Int(3));
                } else { panic!("Expected Sub at top level, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_mul_div_operators() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = 10 * 5 / 2;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                // (10 * 5) / 2 — left-associative
                if let Expression::BinaryOp { operator: BinaryOperator::Div, left, right } = value {
                    assert!(matches!(left.as_ref(), Expression::BinaryOp { operator: BinaryOperator::Mul, .. }));
                    assert_eq!(**right, Expression::Int(2));
                } else { panic!("Expected Div at top level, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_not_equals_operator() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = a != b;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                assert!(matches!(value, Expression::BinaryOp { operator: BinaryOperator::NotEq, .. }));
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_lteq_gteq_operators() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = a <= b;
                    var y = a >= b;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                assert!(matches!(value, Expression::BinaryOp { operator: BinaryOperator::LtEq, .. }));
            } else { panic!("Expected Let"); }
            if let Statement::Let { value, .. } = &body.statements[1] {
                assert!(matches!(value, Expression::BinaryOp { operator: BinaryOperator::GtEq, .. }));
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    // ===== New Operators: &&, ||, !, %, unary =====

    #[test]
    fn test_parse_and_operator() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = a > 5 && b < 10;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                // (a > 5) && (b < 10)
                if let Expression::BinaryOp { operator: BinaryOperator::And, left, right } = value {
                    assert!(matches!(left.as_ref(), Expression::BinaryOp { operator: BinaryOperator::Gt, .. }));
                    assert!(matches!(right.as_ref(), Expression::BinaryOp { operator: BinaryOperator::Lt, .. }));
                } else { panic!("Expected And at top level, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_or_operator() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = a == 1 || b == 2;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                // (a == 1) || (b == 2)
                if let Expression::BinaryOp { operator: BinaryOperator::Or, left, right } = value {
                    assert!(matches!(left.as_ref(), Expression::BinaryOp { operator: BinaryOperator::Eq, .. }));
                    assert!(matches!(right.as_ref(), Expression::BinaryOp { operator: BinaryOperator::Eq, .. }));
                } else { panic!("Expected Or at top level, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_modulo_operator() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = 10 % 3;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::BinaryOp { operator: BinaryOperator::Mod, left, right } = value {
                    assert_eq!(**left, Expression::Int(10));
                    assert_eq!(**right, Expression::Int(3));
                } else { panic!("Expected Mod, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_unary_negate() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = -5;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::UnaryOp { operator: UnaryOperator::Negate, operand } = value {
                    assert_eq!(**operand, Expression::Int(5));
                } else { panic!("Expected UnaryOp Negate, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_unary_not() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = !flag;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::UnaryOp { operator: UnaryOperator::Not, operand } = value {
                    assert_eq!(**operand, Expression::Identifier("flag".to_string()));
                } else { panic!("Expected UnaryOp Not, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_boolean_precedence_and_binds_tighter_than_or() {
        // a || b && c  should parse as  a || (b && c)
        let source = r#"
            agent Test {
                public void Run() {
                    var x = a || b && c;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                // Top level should be Or, right should be And
                if let Expression::BinaryOp { operator: BinaryOperator::Or, left, right } = value {
                    assert_eq!(**left, Expression::Identifier("a".to_string()));
                    assert!(matches!(right.as_ref(), Expression::BinaryOp { operator: BinaryOperator::And, .. }));
                } else { panic!("Expected Or at top level, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_modulo_same_precedence_as_mul_div() {
        // 10 * 3 % 2  should parse as  (10 * 3) % 2  (left-associative, same level)
        let source = r#"
            agent Test {
                public void Run() {
                    var x = 10 * 3 % 2;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::BinaryOp { operator: BinaryOperator::Mod, left, right } = value {
                    assert!(matches!(left.as_ref(), Expression::BinaryOp { operator: BinaryOperator::Mul, .. }));
                    assert_eq!(**right, Expression::Int(2));
                } else { panic!("Expected Mod at top level, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_unary_negate_in_expression() {
        // var x = 5 + -3;  should parse as  5 + (-(3))
        let source = r#"
            agent Test {
                public void Run() {
                    var x = 5 + -3;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::BinaryOp { operator: BinaryOperator::Add, left, right } = value {
                    assert_eq!(**left, Expression::Int(5));
                    if let Expression::UnaryOp { operator: UnaryOperator::Negate, operand } = right.as_ref() {
                        assert_eq!(**operand, Expression::Int(3));
                    } else { panic!("Expected UnaryOp Negate on right, got {:?}", right); }
                } else { panic!("Expected Add at top level, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    // ===== Wave 5: break / continue =====

    #[test]
    fn test_parse_break() {
        let source = r#"
            agent Test {
                public void Run() {
                    while (true) {
                        break;
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::While { body: loop_body, .. } = &body.statements[0] {
                assert!(matches!(loop_body.statements[0], Statement::Break));
            } else { panic!("Expected While"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_continue() {
        let source = r#"
            agent Test {
                public void Run() {
                    while (true) {
                        continue;
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::While { body: loop_body, .. } = &body.statements[0] {
                assert!(matches!(loop_body.statements[0], Statement::Continue));
            } else { panic!("Expected While"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_break_continue_in_for() {
        let source = r#"
            agent Test {
                public void Run() {
                    foreach (var i in items) {
                        if (i == 0) {
                            continue;
                        }
                        if (i > 10) {
                            break;
                        }
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Foreach { body: loop_body, .. } = &body.statements[0] {
                // First statement is if-continue, second is if-break
                assert_eq!(loop_body.statements.len(), 2);
                if let Statement::If { then_block, .. } = &loop_body.statements[0] {
                    assert!(matches!(then_block.statements[0], Statement::Continue));
                } else { panic!("Expected If"); }
                if let Statement::If { then_block, .. } = &loop_body.statements[1] {
                    assert!(matches!(then_block.statements[0], Statement::Break));
                } else { panic!("Expected If"); }
            } else { panic!("Expected Foreach"); }
        } else { panic!("Expected Agent"); }
    }

    // ===== Wave 5: else if =====

    #[test]
    fn test_parse_else_if() {
        let source = r#"
            agent Test {
                public void Run() {
                    if (x > 10) {
                        print "big";
                    } else if (x > 5) {
                        print "medium";
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::If { else_block: Some(eb), .. } = &body.statements[0] {
                // else block contains a single If statement
                assert_eq!(eb.statements.len(), 1);
                assert!(matches!(eb.statements[0], Statement::If { .. }));
            } else { panic!("Expected If with else block"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_else_if_else() {
        let source = r#"
            agent Test {
                public void Run() {
                    if (x > 10) {
                        print "big";
                    } else if (x > 5) {
                        print "medium";
                    } else {
                        print "small";
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::If { else_block: Some(eb), .. } = &body.statements[0] {
                // else block contains a single If with its own else
                if let Statement::If { else_block: Some(final_else), .. } = &eb.statements[0] {
                    // The final else block has a print statement
                    assert_eq!(final_else.statements.len(), 1);
                    assert!(matches!(final_else.statements[0], Statement::Print(_)));
                } else { panic!("Expected nested If with else"); }
            } else { panic!("Expected If with else block"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_chained_else_if() {
        let source = r#"
            agent Test {
                public void Run() {
                    if (x == 1) {
                        print "one";
                    } else if (x == 2) {
                        print "two";
                    } else if (x == 3) {
                        print "three";
                    } else {
                        print "other";
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            // First level: if (x == 1) ... else { if (x == 2) ... }
            if let Statement::If { else_block: Some(eb1), .. } = &body.statements[0] {
                if let Statement::If { else_block: Some(eb2), .. } = &eb1.statements[0] {
                    // Second level: if (x == 3) ... else { print "other" }
                    if let Statement::If { else_block: Some(eb3), .. } = &eb2.statements[0] {
                        assert_eq!(eb3.statements.len(), 1);
                        assert!(matches!(eb3.statements[0], Statement::Print(_)));
                    } else { panic!("Expected third-level If with else"); }
                } else { panic!("Expected second-level If with else"); }
            } else { panic!("Expected first If with else"); }
        } else { panic!("Expected Agent"); }
    }

    // ===== Wave 5: async / await =====

    #[test]
    fn test_parse_async_method() {
        let source = r#"
            agent Test {
                async public void FetchData() {
                    print "fetching";
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            assert!(a.methods[0].is_async);
            assert_eq!(a.methods[0].name, "FetchData");
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_await_expression() {
        let source = r#"
            agent Test {
                async public void Run() {
                    var result = await fetch("https://api.example.com");
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            assert!(a.methods[0].is_async);
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                assert!(matches!(value, Expression::Await(_)));
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_non_async_method() {
        let source = r#"
            agent Test {
                public void Run() {
                    print "sync";
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            assert!(!a.methods[0].is_async);
        } else { panic!("Expected Agent"); }
    }

    // ===== Wave 5: const =====

    #[test]
    fn test_parse_const_with_type() {
        let source = r#"
            agent Test {
                public void Run() {
                    const int MAX = 100;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Const { name, ty, value } = &body.statements[0] {
                assert_eq!(name, "MAX");
                assert_eq!(*ty, Some(TypeNode::Int));
                assert_eq!(*value, Expression::Int(100));
            } else { panic!("Expected Const, got {:?}", body.statements[0]); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_const_with_var() {
        let source = r#"
            agent Test {
                public void Run() {
                    const var NAME = "varg";
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Const { name, ty, .. } = &body.statements[0] {
                assert_eq!(name, "NAME");
                assert_eq!(*ty, None); // inferred
            } else { panic!("Expected Const"); }
        } else { panic!("Expected Agent"); }
    }

    // ===== Wave 5b: Index/Property Assignment + Compound Assignment =====

    #[test]
    fn test_parse_index_assign() {
        let source = r#"
            agent Test {
                public void Run() {
                    arr[0] = 42;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            assert!(matches!(body.statements[0], Statement::IndexAssign { .. }));
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_property_assign() {
        let source = r#"
            agent Test {
                public void Run() {
                    obj.name = "alice";
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::PropertyAssign { property, .. } = &body.statements[0] {
                assert_eq!(property, "name");
            } else { panic!("Expected PropertyAssign, got {:?}", body.statements[0]); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_compound_plus_assign() {
        let source = r#"
            agent Test {
                public void Run() {
                    x += 5;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            // Desugared to: x = x + 5
            if let Statement::Assign { name, value } = &body.statements[0] {
                assert_eq!(name, "x");
                assert!(matches!(value, Expression::BinaryOp { operator: BinaryOperator::Add, .. }));
            } else { panic!("Expected Assign (desugared), got {:?}", body.statements[0]); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_compound_minus_assign() {
        let source = r#"
            agent Test {
                public void Run() {
                    count -= 1;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Assign { name, value } = &body.statements[0] {
                assert_eq!(name, "count");
                assert!(matches!(value, Expression::BinaryOp { operator: BinaryOperator::Sub, .. }));
            } else { panic!("Expected Assign (desugared)"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_index_compound_assign() {
        let source = r#"
            agent Test {
                public void Run() {
                    arr[0] += 10;
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            // Desugared to IndexAssign with BinaryOp value
            if let Statement::IndexAssign { value, .. } = &body.statements[0] {
                assert!(matches!(value, Expression::BinaryOp { operator: BinaryOperator::Add, .. }));
            } else { panic!("Expected IndexAssign, got {:?}", body.statements[0]); }
        } else { panic!("Expected Agent"); }
    }

    // ===== Plan 13: Pipe Operator =====

    #[test]
    fn test_parse_pipe_simple() {
        // a |> f() → MethodCall { caller: self, method: f, args: [a] }
        let source = r#"
            agent Test {
                public void Run() {
                    var x = data |> process();
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::MethodCall { method_name, args, .. } = value {
                    assert_eq!(method_name, "process");
                    assert_eq!(args.len(), 1); // 'data' is first arg
                    assert!(matches!(&args[0], Expression::Identifier(n) if n == "data"));
                } else { panic!("Expected MethodCall, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_pipe_chain() {
        // a |> f() |> g() → g(f(a))
        let source = r#"
            agent Test {
                public void Run() {
                    var x = data |> step1() |> step2();
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                // Outer: step2(step1(data))
                if let Expression::MethodCall { method_name, args, .. } = value {
                    assert_eq!(method_name, "step2");
                    assert_eq!(args.len(), 1);
                    // Inner: step1(data)
                    if let Expression::MethodCall { method_name: inner_name, args: inner_args, .. } = &args[0] {
                        assert_eq!(inner_name, "step1");
                        assert_eq!(inner_args.len(), 1);
                        assert!(matches!(&inner_args[0], Expression::Identifier(n) if n == "data"));
                    } else { panic!("Expected inner MethodCall"); }
                } else { panic!("Expected MethodCall"); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_pipe_with_args() {
        // a |> f(b, c) → f(a, b, c)
        let source = r#"
            agent Test {
                public void Run() {
                    var x = data |> transform("json", 5);
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::MethodCall { method_name, args, .. } = value {
                    assert_eq!(method_name, "transform");
                    assert_eq!(args.len(), 3); // data, "json", 5
                    assert!(matches!(&args[0], Expression::Identifier(n) if n == "data"));
                    assert!(matches!(&args[1], Expression::String(s) if s == "json"));
                    assert!(matches!(&args[2], Expression::Int(5)));
                } else { panic!("Expected MethodCall"); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_pipe_dot_method() {
        // a |> .to_upper() → a.to_upper()
        let source = r#"
            agent Test {
                public void Run() {
                    var x = name |> .to_upper();
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::MethodCall { caller, method_name, args } = value {
                    assert_eq!(method_name, "to_upper");
                    assert!(matches!(**caller, Expression::Identifier(ref n) if n == "name"));
                    assert_eq!(args.len(), 0);
                } else { panic!("Expected MethodCall, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    // ===== Plan 14: Retry/Fallback =====

    #[test]
    fn test_parse_retry_simple() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = retry(3) { fetch("url") };
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::Retry { max_attempts, fallback, .. } = value {
                    assert!(matches!(**max_attempts, Expression::Int(3)));
                    assert!(fallback.is_none());
                } else { panic!("Expected Retry, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_retry_with_fallback() {
        let source = r#"
            agent Test {
                public void Run() {
                    var x = retry(5) { fetch("url") } fallback { "default" };
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::Retry { max_attempts, fallback, .. } = value {
                    assert!(matches!(**max_attempts, Expression::Int(5)));
                    assert!(fallback.is_some());
                } else { panic!("Expected Retry"); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_spawn() {
        let source = r#"
            agent Test {
                public void Run() {
                    var worker = spawn Worker();
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Let { value, .. } = &body.statements[0] {
                if let Expression::Spawn { agent_name, args } = value {
                    assert_eq!(agent_name, "Worker");
                    assert_eq!(args.len(), 0);
                } else { panic!("Expected Spawn, got {:?}", value); }
            } else { panic!("Expected Let"); }
        } else { panic!("Expected Agent"); }
    }

    // ===== Plan 19: Agent Fields Tests =====

    #[test]
    fn test_parse_agent_fields() {
        let source = r#"
            agent Counter {
                int count;
                string name;
                public void Run() { }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            assert_eq!(a.fields.len(), 2);
            assert_eq!(a.fields[0].name, "count");
            assert_eq!(a.fields[0].ty, TypeNode::Int);
            assert_eq!(a.fields[1].name, "name");
            assert_eq!(a.fields[1].ty, TypeNode::String);
            assert_eq!(a.methods.len(), 1);
            assert_eq!(a.methods[0].name, "Run");
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_agent_fields_mixed_order() {
        let source = r#"
            agent Mixed {
                int value;
                public void Init() { }
                string label;
                public int Get() { return 0; }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            assert_eq!(a.fields.len(), 2);
            assert_eq!(a.fields[0].name, "value");
            assert_eq!(a.fields[1].name, "label");
            assert_eq!(a.methods.len(), 2);
            assert_eq!(a.methods[0].name, "Init");
            assert_eq!(a.methods[1].name, "Get");
        } else { panic!("Expected Agent"); }
    }

    // ===== Plan 17: Implicit Context Passing Tests =====

    #[test]
    fn test_parse_with_context_annotation() {
        let source = r#"
            @[WithContext]
            agent SmartAgent {
                public void Run() { }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            assert_eq!(a.name, "SmartAgent");
            // @[WithContext] auto-injects a context field
            assert_eq!(a.fields.len(), 1);
            assert_eq!(a.fields[0].name, "context");
            assert_eq!(a.fields[0].ty, TypeNode::Context);
            assert_eq!(a.methods.len(), 1);
        } else { panic!("Expected Agent"); }
    }

    // ===== Plan 20: Select Statement Tests =====

    #[test]
    fn test_parse_select_statement() {
        let source = r#"
            agent Test {
                public void Run() {
                    select {
                        msg from worker => {
                            print msg;
                        }
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Select { arms } = &body.statements[0] {
                assert_eq!(arms.len(), 1);
                assert_eq!(arms[0].var_name, "msg");
                assert!(matches!(arms[0].source, SelectSource::Agent(_)));
            } else { panic!("Expected Select, got {:?}", body.statements[0]); }
        } else { panic!("Expected Agent"); }
    }

    #[test]
    fn test_parse_select_with_timeout() {
        let source = r#"
            agent Test {
                public void Run() {
                    select {
                        msg from worker => {
                            print msg;
                        }
                        timeout(5000) => {
                            print "timeout";
                        }
                    }
                }
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::Agent(a) = &program.items[0] {
            let body = a.methods[0].body.as_ref().unwrap();
            if let Statement::Select { arms } = &body.statements[0] {
                assert_eq!(arms.len(), 2);
                assert!(matches!(arms[0].source, SelectSource::Agent(_)));
                assert!(matches!(arms[1].source, SelectSource::Timeout(_)));
            } else { panic!("Expected Select"); }
        } else { panic!("Expected Agent"); }
    }

    // ---- Plan 23: Prompt Template Tests ----
    #[test]
    fn test_parse_prompt_template() {
        // Token is #[token("Prompt")] — capital P
        let source = r#"
            Prompt greet() {
                Hello, World!
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        assert_eq!(program.items.len(), 1);
        if let Item::PromptTemplate(pt) = &program.items[0] {
            assert_eq!(pt.name, "greet");
            assert!(pt.params.is_empty());
            assert!(pt.body.contains("Hello"));
        } else { panic!("Expected PromptTemplate"); }
    }

    #[test]
    fn test_parse_prompt_with_params() {
        let source = r#"
            Prompt analyze(string text, int count) {
                Analyze the following: {text}
                Repeat {count} times
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        if let Item::PromptTemplate(pt) = &program.items[0] {
            assert_eq!(pt.name, "analyze");
            assert_eq!(pt.params.len(), 2);
            assert_eq!(pt.params[0].name, "text");
            assert_eq!(pt.params[1].name, "count");
            assert!(pt.body.contains("{text}"));
            assert!(pt.body.contains("{count}"));
        } else { panic!("Expected PromptTemplate"); }
    }
}
