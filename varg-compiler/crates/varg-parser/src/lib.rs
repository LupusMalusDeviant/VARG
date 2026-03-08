use varg_ast::Token;
use varg_ast::ast::*;
use varg_lexer::Lexer;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    UnexpectedToken { expected: String, found: Option<Token> },
    UnexpectedEof,
}

impl Parser {
    pub fn new(source: &str) -> Self {
        let lexer = Lexer::new(source);
        let tokens: Vec<Token> = lexer.filter_map(|(res, _)| res.ok()).collect();
        Self { tokens, pos: 0 }
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }
    
    fn peek_n(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.pos + offset)
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
        match self.advance() {
            Some(t) if t == expected => Ok(()),
            Some(t) => Err(ParseError::UnexpectedToken {
                expected: format!("{:?}", expected),
                found: Some(t),
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        match self.advance() {
            Some(Token::Identifier(name)) => Ok(name),
            Some(t) => Err(ParseError::UnexpectedToken {
                expected: "Identifier".to_string(),
                found: Some(t),
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
                let methods = self.parse_methods_block()?;
                Ok(Item::Agent(AgentDef {
                    name,
                    is_system,
                    is_public,
                    target_annotation,
                    annotations,
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
            Some(t) => Err(ParseError::UnexpectedToken {
                expected: "Agent, Contract, or Struct".to_string(),
                found: Some(t.clone()),
            }),
            None => Err(ParseError::UnexpectedEof),
        }
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
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_methods_block(&mut self) -> Result<Vec<MethodDecl>, ParseError> {
        self.consume(Token::LBrace)?;
        let mut methods = Vec::new();

        while let Some(tok) = self.peek() {
            if *tok == Token::RBrace {
                break;
            }
            
            let annotations = self.parse_annotations()?;

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
                annotations,
                type_params,
                args,
                return_ty: Some(return_ty),
                body,
            });
        }
        self.consume(Token::RBrace)?;
        Ok(methods)
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
                Token::Var | Token::TypeInt | Token::TypeString | Token::TypeBool | Token::Prompt | Token::Tensor | Token::Context | Token::TypeIntShort | Token::TypeStringShort | Token::TypeBoolShort | Token::TypeMapShort => {
                    let ty = if *tok == Token::Var {
                        self.advance(); // consume 'var'
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
                    match self.peek() {
                        Some(Token::Assign) => {
                            if let Expression::Identifier(var_name) = expr {
                                self.advance(); // consume Assign
                                let value = self.parse_expression()?;
                                self.consume(Token::Semicolon)?;
                                statements.push(Statement::Assign { name: var_name, value });
                            } else {
                                return Err(ParseError::UnexpectedToken { expected: "Valid L-Value".to_string(), found: Some(Token::Assign) });
                            }
                        },
                        _ => {
                            self.consume(Token::Semicolon)?;
                            statements.push(Statement::Expr(expr));
                        }
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
                Token::If => {
                    self.advance();
                    self.consume(Token::LParen)?;
                    let condition = self.parse_expression()?;
                    self.consume(Token::RParen)?;
                    let then_block = self.parse_block()?;
                    let mut else_block = None;
                    if let Some(Token::Else) = self.peek() {
                       self.advance();
                       else_block = Some(self.parse_block()?);
                    }
                    statements.push(Statement::If { condition, then_block, else_block });
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
                        _ => return Err(ParseError::UnexpectedToken { expected: "For Loop Init Statement".to_string(), found: self.advance() }),
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
                        return Err(ParseError::UnexpectedToken { expected: "Query String".to_string(), found: self.advance() });
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

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        let mut left = match self.advance() {
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
                    });
                }
            },
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
                        found: self.peek().cloned()
                    });
                }
            },
            Some(t) => return Err(ParseError::UnexpectedToken {
                expected: "Expression Literal/Identifier/From".to_string(),
                found: Some(t),
            }),
            None => return Err(ParseError::UnexpectedEof),
        };

        // Simplified peek for binary ops and property access `.`
        while let Some(tok) = self.peek() {
            match tok {
                Token::GreaterThan | Token::Equals | Token::LessThan | Token::Plus | Token::Minus | Token::Tilde => {
                    let op = match self.advance().unwrap() {
                        Token::GreaterThan => BinaryOperator::Gt,
                        Token::Equals => BinaryOperator::Eq,
                        Token::LessThan => BinaryOperator::Lt,
                        Token::Plus => BinaryOperator::Add,
                        Token::Minus => BinaryOperator::Sub,
                        Token::Tilde => BinaryOperator::CosineSim,
                        _ => unreachable!(),
                    };
                    let right = self.parse_expression()?; // right-associative mock
                    left = Expression::BinaryOp {
                        left: Box::new(left),
                        operator: op,
                        right: Box::new(right),
                    };
                },
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
                        return Err(ParseError::UnexpectedToken { expected: "Identifier before call".to_string(), found: Some(Token::LParen) });
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
}
