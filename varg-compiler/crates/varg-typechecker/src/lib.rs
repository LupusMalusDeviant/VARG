use varg_ast::ast::*;
use std::collections::HashMap;

/// Semantic errors discovered during Type Checking or OCAP validation
#[derive(Debug, PartialEq)]
pub enum TypeError {
    TypeMismatch { expected: String, found: String },
    UndeclaredVariable(String),
    IllegalOsCall { reason: String }, // OCAP Violation
}

pub struct TypeChecker {
    // Very simple symbol table for this MVP, tracking variables and their types in current scope
    env: HashMap<String, TypeNode>,
    
    // OCAP state
    in_unsafe_block: bool,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            in_unsafe_block: false,
        }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), TypeError> {
        for item in &program.items {
            self.check_item(item)?;
        }
        Ok(())
    }

    fn check_item(&mut self, item: &Item) -> Result<(), TypeError> {
        match item {
            Item::Import(_) => Ok(()), // MVP: Imports are resolved and merged by CLI earlier
            Item::Agent(agent) => {
                for method in &agent.methods {
                    self.check_method(method)?;
                }
                Ok(())
            },
            Item::Contract(contract) => {
                // Contracts are interfaces, no bodies to check right now
                Ok(())
            },
            Item::Struct(_s) => {
                // Struct property definitions are syntactically valid by parser.
                Ok(())
            }
        }
    }

    fn check_method(&mut self, method: &MethodDecl) -> Result<(), TypeError> {
        // Validate CLI/MCP arguments are primitive types
        for ann in &method.annotations {
            if ann.name == "CliCommand" || ann.name == "McpTool" {
                for arg in &method.args {
                    match arg.ty {
                        TypeNode::String | TypeNode::Int | TypeNode::Bool | TypeNode::Ulong => {},
                        _ => return Err(TypeError::TypeMismatch {
                            expected: "Primitive type (String, Int, Bool, Ulong) for CLI/MCP arguments".to_string(),
                            found: format!("{:?}", arg.ty),
                        }),
                    }
                }
            }
        }

        // Clear environment for new method scope
        self.env.clear();
        self.in_unsafe_block = false;

        // Register arguments in the environment
        for arg in &method.args {
            self.env.insert(arg.name.clone(), arg.ty.clone());
        }

        if let Some(body) = &method.body {
            self.check_block(body)?;
        }
        Ok(())
    }

    fn check_block(&mut self, block: &Block) -> Result<(), TypeError> {
        let previous_unsafe = self.in_unsafe_block;

        for stmt in &block.statements {
            match stmt {
                Statement::Let { name, ty, value } => {
                    let val_type = self.infer_expression_type(value)?;
                    if let Some(expected_ty) = ty {
                        if !self.types_match(expected_ty, &val_type) {
                            return Err(TypeError::TypeMismatch {
                                expected: format!("{:?}", expected_ty),
                                found: format!("{:?}", val_type),
                            });
                        }
                        self.env.insert(name.clone(), expected_ty.clone());
                    } else {
                        // Type inference for `var`!
                        self.env.insert(name.clone(), val_type);
                    }
                },
                Statement::Assign { name, value } => {
                    let expected_ty = self.env.get(name).cloned().ok_or_else(|| TypeError::UndeclaredVariable(name.clone()))?;
                    let val_type = self.infer_expression_type(value)?;
                    if !self.types_match(&expected_ty, &val_type) {
                        return Err(TypeError::TypeMismatch {
                            expected: format!("{:?}", expected_ty),
                            found: format!("{:?}", val_type),
                        });
                    }
                },
                Statement::UnsafeBlock(inner_block) => {
                    self.in_unsafe_block = true;
                    self.check_block(inner_block)?;
                    self.in_unsafe_block = previous_unsafe;
                },

                Statement::If { condition, then_block, else_block } => {
                    let cond_ty = self.infer_expression_type(condition)?;
                    if cond_ty != TypeNode::Bool {
                        return Err(TypeError::TypeMismatch { expected: "Bool".to_string(), found: format!("{:?}", cond_ty) });
                    }
                    self.check_block(then_block)?;
                    if let Some(eb) = else_block {
                        self.check_block(eb)?;
                    }
                },
                Statement::While { condition, body } => {
                    let cond_ty = self.infer_expression_type(condition)?;
                    if cond_ty != TypeNode::Bool {
                        return Err(TypeError::TypeMismatch { expected: "Bool".to_string(), found: format!("{:?}", cond_ty) });
                    }
                    self.check_block(body)?;
                },
                Statement::For { init, condition, update, body } => {
                    self.check_block(&Block { statements: vec![*init.clone()] })?;
                    let cond_ty = self.infer_expression_type(condition)?;
                    if cond_ty != TypeNode::Bool {
                         return Err(TypeError::TypeMismatch { expected: "Bool".to_string(), found: format!("{:?}", cond_ty) });
                    }
                    self.check_block(&Block { statements: vec![*update.clone()] })?;
                    self.check_block(body)?;
                },
                Statement::Foreach { item_name, collection, body } => {
                     let _coll_ty = self.infer_expression_type(collection)?;
                     // register item in scope dynamically for MVP
                     self.env.insert(item_name.clone(), TypeNode::Custom("Dynamic".to_string()));
                     self.check_block(body)?;
                },
                Statement::Stream(expr) => {
                     let ty = self.infer_expression_type(expr)?;
                     if ty != TypeNode::String && ty != TypeNode::Prompt && ty != TypeNode::Void {
                         return Err(TypeError::TypeMismatch { expected: "String or Prompt".to_string(), found: format!("{:?}", ty) });
                     }
                },
                Statement::Print(expr) => {
                     self.infer_expression_type(expr)?;
                },
                Statement::Return(Some(expr)) => {
                    // MVP simplification: just verify the expression's type can be inferred
                    let _val_type = self.infer_expression_type(expr)?;
                },
                Statement::Return(None) => {},
                Statement::Expr(expr) => {
                    self.infer_expression_type(expr)?;
                },
                Statement::TryCatch { try_block, catch_var, catch_block } => {
                    self.check_block(try_block)?;
                    // bind the err var as a string
                    self.env.insert(catch_var.clone(), TypeNode::String);
                    self.check_block(catch_block)?;
                },
                Statement::Throw(expr) => {
                    self.infer_expression_type(expr)?;
                }
            }
        }
        Ok(())
    }

    fn infer_expression_type(&mut self, expr: &Expression) -> Result<TypeNode, TypeError> {
        match expr {
            Expression::Int(_) => Ok(TypeNode::Int),
            Expression::String(_) => Ok(TypeNode::String),
            Expression::PromptLiteral(_) => Ok(TypeNode::Prompt),
            Expression::Bool(_) => Ok(TypeNode::Bool),
            Expression::Identifier(name) => {
                if let Some(ty) = self.env.get(name) {
                    Ok(ty.clone())
                } else {
                    Err(TypeError::UndeclaredVariable(name.clone()))
                }
            },
            Expression::BinaryOp { operator, .. } => {
                // If it's a comparison operator, it returns Bool, otherwise Int for MVP
                match operator {
                    BinaryOperator::Eq | BinaryOperator::NotEq | BinaryOperator::Lt | BinaryOperator::Gt | BinaryOperator::LtEq | BinaryOperator::GtEq => Ok(TypeNode::Bool),
                    BinaryOperator::CosineSim => Ok(TypeNode::Custom("f32".to_string())),
                    _ => Ok(TypeNode::Int)
                }
            },
            Expression::MethodCall { method_name, args, .. } => {
                if method_name == "fetch" {
                    if !self.in_unsafe_block {
                        return Err(TypeError::IllegalOsCall { 
                            reason: "Network API call `fetch` attempted outside of an `unsafe` block. varg requires explicit kernel escalation.".to_string() 
                        });
                    }
                    if args.len() < 1 || args.len() > 4 {
                        return Err(TypeError::TypeMismatch { expected: "1 to 4 arguments (url, [method], [headers], [body])".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "llm_infer" {
                    if !self.in_unsafe_block {
                        return Err(TypeError::IllegalOsCall { 
                            reason: "LLM inference attempted outside of an `unsafe` block. varg requires explicit kernel escalation.".to_string() 
                        });
                    }
                    if args.len() < 1 || args.len() > 2 {
                        return Err(TypeError::TypeMismatch { expected: "1 or 2 arguments (prompt, [model])".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "llm_chat" {
                    if !self.in_unsafe_block {
                        return Err(TypeError::IllegalOsCall { 
                            reason: "LLM chat attempted outside of an `unsafe` block. varg requires explicit kernel escalation.".to_string() 
                        });
                    }
                    if args.len() < 2 || args.len() > 3 {
                        return Err(TypeError::TypeMismatch { expected: "2 or 3 arguments (context, prompt, [model])".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "encrypt" || method_name == "decrypt" {
                    if args.len() != 2 {
                        return Err(TypeError::TypeMismatch { expected: "2 arguments".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "file_read" {
                    if !self.in_unsafe_block {
                        return Err(TypeError::IllegalOsCall { 
                            reason: "File I/O `file_read` attempted outside of an `unsafe` block.".to_string() 
                        });
                    }
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "file_write" {
                    if !self.in_unsafe_block {
                        return Err(TypeError::IllegalOsCall { 
                            reason: "File I/O `file_write` attempted outside of an `unsafe` block.".to_string() 
                        });
                    }
                    if args.len() != 2 {
                        return Err(TypeError::TypeMismatch { expected: "2 arguments (path, data)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Void)
                } else if method_name == "time_now" {
                    if args.len() != 0 {
                        return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Int)
                } else if method_name == "str_replace" {
                    if args.len() != 3 {
                        return Err(TypeError::TypeMismatch { expected: "3 arguments (string, search, replace)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "str_trim" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (string)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "str_split" {
                    if args.len() != 2 {
                        return Err(TypeError::TypeMismatch { expected: "2 arguments (string, delimiter)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                } else if method_name == "__varg_create_context" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (id)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Context)
                } else if method_name == "context_from" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (query result string)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Context)
                } else if method_name == "from_json" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (string)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    // For MVP we assume from_json parses into a flat HashMap of strings
                    Ok(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                } else if method_name == "to_json" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (map or array)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else {
                    // Simplified MVP: assume void for undeclared calls
                    Ok(TypeNode::Void)
                }
            },
            Expression::PropertyAccess { caller, property_name } => {
                let caller_ty = self.infer_expression_type(caller)?;
                // For MVP: assume property exists and is a String
                if *property_name == "text" && caller_ty == TypeNode::Prompt {
                    Ok(TypeNode::String)
                } else if caller_ty == TypeNode::Tensor && *property_name == "data" {
                    Ok(TypeNode::Array(Box::new(TypeNode::Custom("f32".to_string()))))
                } else {
                    Ok(TypeNode::Custom("Dynamic".to_string()))
                }
            },
            Expression::IndexAccess { caller, index } => {
                let caller_ty = self.infer_expression_type(caller)?;
                let _index_ty = self.infer_expression_type(index)?;
                match caller_ty {
                    TypeNode::Array(inner) | TypeNode::List(inner) => Ok(*inner),
                    TypeNode::Map(_, val) => Ok(*val),
                    TypeNode::Custom(_) => Ok(TypeNode::Custom("Dynamic".to_string())),
                    _ => Err(TypeError::TypeMismatch { expected: "List or Map".to_string(), found: format!("{:?}", caller_ty) })
                }
            },
            Expression::Linq(_) => {
                Ok(TypeNode::Custom("Iterator".to_string()))
            },
            Expression::ArrayLiteral(elements) => {
                let inner_ty = if elements.is_empty() {
                    TypeNode::Custom("Dynamic".to_string())
                } else {
                    self.infer_expression_type(&elements[0])?
                };
                Ok(TypeNode::Array(Box::new(inner_ty)))
            },
            Expression::MapLiteral(entries) => {
                let (key_ty, val_ty) = if entries.is_empty() {
                    (TypeNode::Custom("Dynamic".to_string()), TypeNode::Custom("Dynamic".to_string()))
                } else {
                    let k_ty = self.infer_expression_type(&entries[0].0)?;
                    let v_ty = self.infer_expression_type(&entries[0].1)?;
                    (k_ty, v_ty)
                };
                Ok(TypeNode::Map(Box::new(key_ty), Box::new(val_ty)))
            },
            Expression::Query(_) => {
                if !self.in_unsafe_block {
                    return Err(TypeError::IllegalOsCall { 
                        reason: "Native memory query attempted outside of an `unsafe` block. varg requires explicit kernel escalation.".to_string() 
                    });
                }
                // Memory queries return JSON Strings
                Ok(TypeNode::String)
            }
        }
    }

    fn types_match(&self, expected: &TypeNode, actual: &TypeNode) -> bool {
        if expected == actual {
            return true;
        }
        
        match (expected, actual) {
            (TypeNode::Generic(name1, args1), TypeNode::Generic(name2, args2)) => {
                if name1 != name2 || args1.len() != args2.len() { return false; }
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    if !self.types_match(a1, a2) { return false; }
                }
                true
            },
            (TypeNode::List(inner1), TypeNode::Array(inner2)) => {
                self.types_match(inner1, inner2)
            },
            (TypeNode::Array(inner1), TypeNode::List(inner2)) => {
                self.types_match(inner1, inner2)
            },
            (TypeNode::Array(inner1), TypeNode::Array(inner2)) => {
                self.types_match(inner1, inner2)
            },
            (TypeNode::List(inner1), TypeNode::List(inner2)) => {
                self.types_match(inner1, inner2)
            },
            (TypeNode::Map(k1, v1), TypeNode::Map(k2, v2)) => {
                self.types_match(k1, k2) && self.types_match(v1, v2)
            },
            (TypeNode::TypeVar(_), _) => {
                // MVP Generic Substitution: A TypeVar (e.g. T) natively accepts the instanced type
                true
            },
            (TypeNode::Custom(c), _) if c == "Dynamic" => true,
            (_, TypeNode::Custom(c)) if c == "Dynamic" => true,
            _ => false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocap_query_violation() {
        // Attempting to run a query without `unsafe` should fail!
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Hacker".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "StealData".to_string(),
                    is_public: true,
                    annotations: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Expr(Expression::Query(SurrealQueryNode { raw_query: "SELECT secret FROM users".to_string() }))
                        ]
                    })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        
        assert!(result.is_err());
        if let Err(TypeError::IllegalOsCall { reason }) = result {
            assert!(reason.contains("outside of an `unsafe` block"));
        } else {
            panic!("Expected IllegalOsCall error!");
        }
    }

    #[test]
    fn test_valid_unsafe_query() {
        // Same logic but wrapped in `unsafe { }`
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "DbManager".to_string(),
                is_system: true,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Read".to_string(),
                    is_public: true,
                    annotations: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::UnsafeBlock(Block {
                                statements: vec![
                                    Statement::Expr(Expression::Query(SurrealQueryNode { raw_query: "SELECT * FROM public".to_string() }))
                                ]
                            })
                        ]
                    })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_type_mismatch() {
        // int x = "hello";
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Buggy".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Let { 
                                name: "x".to_string(), 
                                ty: Some(TypeNode::Int), 
                                value: Expression::String("hello".to_string()) 
                            }
                        ]
                    })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert_eq!(result, Err(TypeError::TypeMismatch { expected: "Int".to_string(), found: "String".to_string() }));
    }

    #[test]
    fn test_ocap_fetch_violation() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "WebScraper".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Scrape".to_string(),
                    is_public: true,
                    annotations: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Let {
                                name: "res".to_string(),
                                ty: None,
                                value: Expression::MethodCall {
                                    caller: Box::new(Expression::Identifier("self".to_string())),
                                    method_name: "fetch".to_string(),
                                    args: vec![Expression::String("https://api.github.com".to_string())]
                                }
                            }
                        ]
                    })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        
        assert!(result.is_err());
        if let Err(TypeError::IllegalOsCall { reason }) = result {
            assert!(reason.contains("Network API call `fetch` attempted outside of an `unsafe` block"));
        } else {
            panic!("Expected IllegalOsCall error for fetch!");
        }
    }

    #[test]
    fn test_cli_command_invalid_args() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "CliAgent".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "RunCmd".to_string(),
                    is_public: true,
                    annotations: vec![Annotation { 
                        name: "CliCommand".to_string(), 
                        values: vec!["run".to_string(), "Runs it".to_string()] 
                    }],
                    args: vec![FieldDecl {
                        name: "complex_arg".to_string(),
                        ty: TypeNode::Prompt, // Not allowed for CLI input directly
                    }],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        
        assert!(result.is_err());
        if let Err(TypeError::TypeMismatch { expected, found }) = result {
            assert!(expected.contains("Primitive type"));
            assert!(found.contains("Prompt"));
        } else {
            panic!("Expected TypeMismatch error for invalid CLI args!");
        }
    }
}
