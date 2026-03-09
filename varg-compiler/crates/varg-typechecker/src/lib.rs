use varg_ast::ast::*;
use std::collections::HashMap;

/// Semantic errors discovered during Type Checking or OCAP validation
#[derive(Debug, PartialEq)]
pub enum TypeError {
    TypeMismatch { expected: String, found: String },
    UndeclaredVariable(String),
    IllegalOsCall { reason: String }, // OCAP Violation
}

impl TypeError {
    /// Human-readable error message for formatted output
    pub fn message(&self) -> String {
        match self {
            TypeError::TypeMismatch { expected, found } => {
                format!("type mismatch: expected `{}`, found `{}`", expected, found)
            }
            TypeError::UndeclaredVariable(name) => {
                format!("use of undeclared variable `{}`", name)
            }
            TypeError::IllegalOsCall { reason } => {
                reason.clone()
            }
        }
    }
}

pub struct TypeChecker {
    // Very simple symbol table for this MVP, tracking variables and their types in current scope
    env: HashMap<String, TypeNode>,

    // Registered enum definitions (name → variants)
    pub enum_defs: HashMap<String, Vec<EnumVariant>>,

    // Registered type aliases (name → resolved type)
    pub type_aliases: HashMap<String, TypeNode>,

    // OCAP state
    in_unsafe_block: bool,

    // Plan 03: Capability tokens available in current method scope
    available_capabilities: Vec<CapabilityType>,
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
            enum_defs: HashMap::new(),
            type_aliases: HashMap::new(),
            in_unsafe_block: false,
            available_capabilities: Vec::new(),
        }
    }

    /// Check if the current scope has a specific capability token
    fn has_capability(&self, cap: &CapabilityType) -> bool {
        self.available_capabilities.contains(cap)
    }

    /// Check if an operation is authorized (via unsafe or capability token)
    fn check_ocap(&self, required_cap: &CapabilityType, operation: &str) -> Result<(), TypeError> {
        if self.in_unsafe_block || self.has_capability(required_cap) {
            Ok(())
        } else {
            let cap_name = match required_cap {
                CapabilityType::NetworkAccess => "NetworkAccess",
                CapabilityType::FileAccess => "FileAccess",
                CapabilityType::DbAccess => "DbAccess",
                CapabilityType::LlmAccess => "LlmAccess",
                CapabilityType::SystemAccess => "SystemAccess",
            };
            Err(TypeError::IllegalOsCall {
                reason: format!(
                    "`{}` requires a `{}` capability token or an `unsafe` block.",
                    operation, cap_name
                ),
            })
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
            Item::Contract(_contract) => {
                // Contracts are interfaces, no bodies to check right now
                Ok(())
            },
            Item::Struct(_s) => {
                // Struct property definitions are syntactically valid by parser.
                Ok(())
            },
            Item::Enum(e) => {
                // Register the enum definition for later use
                self.enum_defs.insert(e.name.clone(), e.variants.clone());
                Ok(())
            },
            Item::TypeAlias { name, target } => {
                // Register the type alias for later resolution
                self.type_aliases.insert(name.clone(), target.clone());
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

        // Plan 03: Extract capability tokens from method parameters
        self.available_capabilities.clear();
        for arg in &method.args {
            if let TypeNode::Capability(cap) = &arg.ty {
                self.available_capabilities.push(cap.clone());
            }
        }

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
                },
                Statement::LetDestructure { pattern, value } => {
                    let _val_ty = self.infer_expression_type(value)?;
                    match pattern {
                        DestructurePattern::Tuple(names) => {
                            for name in names {
                                self.env.insert(name.clone(), TypeNode::Custom("Dynamic".to_string()));
                            }
                        }
                        DestructurePattern::Struct(fields) => {
                            for (name, alias) in fields {
                                let var_name = alias.as_ref().unwrap_or(name);
                                self.env.insert(var_name.clone(), TypeNode::Custom("Dynamic".to_string()));
                            }
                        }
                    }
                },
                Statement::Match { subject, arms } => {
                    // Type-check the subject expression
                    let _subject_ty = self.infer_expression_type(subject)?;
                    // Check each arm's body
                    for arm in arms {
                        // For Variant patterns with bindings, register bound variables
                        let saved_env = self.env.clone();
                        if let Pattern::Variant(_, bindings) = &arm.pattern {
                            for binding in bindings {
                                self.env.insert(binding.clone(), TypeNode::Custom("Dynamic".to_string()));
                            }
                        }
                        self.check_block(&arm.body)?;
                        self.env = saved_env;
                    }
                },
            }
        }
        Ok(())
    }

    fn infer_expression_type(&mut self, expr: &Expression) -> Result<TypeNode, TypeError> {
        match expr {
            Expression::Int(_) => Ok(TypeNode::Int),
            Expression::String(_) => Ok(TypeNode::String),
            Expression::Null => Ok(TypeNode::Nullable(Box::new(TypeNode::Custom("Dynamic".to_string())))),
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
                    self.check_ocap(&CapabilityType::NetworkAccess, "fetch")?;
                    if args.len() < 1 || args.len() > 4 {
                        return Err(TypeError::TypeMismatch { expected: "1 to 4 arguments (url, [method], [headers], [body])".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "llm_infer" {
                    self.check_ocap(&CapabilityType::LlmAccess, "llm_infer")?;
                    if args.len() < 1 || args.len() > 2 {
                        return Err(TypeError::TypeMismatch { expected: "1 or 2 arguments (prompt, [model])".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "llm_chat" {
                    self.check_ocap(&CapabilityType::LlmAccess, "llm_chat")?;
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
                    self.check_ocap(&CapabilityType::FileAccess, "file_read")?;
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "file_write" {
                    self.check_ocap(&CapabilityType::FileAccess, "file_write")?;
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
            Expression::Lambda { params, return_ty, body } => {
                // Register lambda params in scope temporarily
                let saved_env = self.env.clone();
                for param in params {
                    self.env.insert(param.name.clone(), param.ty.clone());
                }
                match body.as_ref() {
                    LambdaBody::Expression(expr) => {
                        let _body_ty = self.infer_expression_type(expr)?;
                    },
                    LambdaBody::Block(block) => {
                        self.check_block(block)?;
                    },
                }
                self.env = saved_env;
                // Infer Func type from params and return type
                let param_types: Vec<TypeNode> = params.iter().map(|p| p.ty.clone()).collect();
                let ret = return_ty.as_ref().map(|t| *t.clone()).unwrap_or(TypeNode::Void);
                Ok(TypeNode::Func(param_types, Box::new(ret)))
            },
            Expression::Query(_) => {
                self.check_ocap(&CapabilityType::DbAccess, "query")?;
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
            // Nullable: null can be assigned to any nullable type, and T matches T?
            (TypeNode::Nullable(inner), TypeNode::Nullable(actual_inner)) => {
                self.types_match(inner, actual_inner)
            },
            (TypeNode::Nullable(_), actual) if *actual == TypeNode::Nullable(Box::new(TypeNode::Custom("Dynamic".to_string()))) => {
                // null literal (which is Nullable(Dynamic)) can be assigned to any Nullable type
                true
            },
            (TypeNode::Nullable(inner), actual) => {
                // A non-null value can be assigned to a nullable variable: string? x = "hello"
                self.types_match(inner, actual)
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
                    type_params: vec![],
                    constraints: vec![],
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
            assert!(reason.contains("query") || reason.contains("unsafe"));
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
                    type_params: vec![],
                    constraints: vec![],
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
                    type_params: vec![],
                    constraints: vec![],
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
                    type_params: vec![],
                    constraints: vec![],
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
            assert!(reason.contains("fetch") && reason.contains("NetworkAccess"));
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
                    type_params: vec![],
                    constraints: vec![],
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

    // ---- Plan 08: Extended TypeChecker Coverage ----

    #[test]
    fn test_undeclared_variable() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Expr(Expression::Identifier("nonexistent".to_string()))
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert_eq!(result, Err(TypeError::UndeclaredVariable("nonexistent".to_string())));
    }

    #[test]
    fn test_assign_to_undeclared() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Assign {
                                name: "missing".to_string(),
                                value: Expression::Int(42),
                            }
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert_eq!(result, Err(TypeError::UndeclaredVariable("missing".to_string())));
    }

    #[test]
    fn test_while_non_bool_condition() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::While {
                                condition: Expression::Int(42), // Not a bool!
                                body: Block { statements: vec![] },
                            }
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert_eq!(result, Err(TypeError::TypeMismatch { expected: "Bool".to_string(), found: "Int".to_string() }));
    }

    #[test]
    fn test_if_non_bool_condition() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::If {
                                condition: Expression::String("not bool".to_string()),
                                then_block: Block { statements: vec![] },
                                else_block: None,
                            }
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert_eq!(result, Err(TypeError::TypeMismatch { expected: "Bool".to_string(), found: "String".to_string() }));
    }

    #[test]
    fn test_var_type_inference() {
        // var x = 42; → x should be Int
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Let { name: "x".to_string(), ty: None, value: Expression::Int(42) },
                            // Now assign a string to x → should fail
                            Statement::Assign { name: "x".to_string(), value: Expression::String("oops".to_string()) },
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err()); // Int inferred, can't assign String
    }

    #[test]
    fn test_ocap_llm_infer_violation() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "llm_infer".to_string(),
                                args: vec![Expression::String("hello".to_string())],
                            })
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err());
        if let Err(TypeError::IllegalOsCall { reason }) = result {
            assert!(reason.contains("llm_infer") && reason.contains("LlmAccess"));
        } else { panic!("Expected IllegalOsCall"); }
    }

    #[test]
    fn test_ocap_file_read_violation() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "file_read".to_string(),
                                args: vec![Expression::String("/etc/passwd".to_string())],
                            })
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err());
        if let Err(TypeError::IllegalOsCall { reason }) = result {
            assert!(reason.contains("file_read") && reason.contains("FileAccess"));
        } else { panic!("Expected IllegalOsCall"); }
    }

    #[test]
    fn test_ocap_file_write_violation() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "file_write".to_string(),
                                args: vec![
                                    Expression::String("/tmp/test".to_string()),
                                    Expression::String("data".to_string()),
                                ],
                            })
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err());
        if let Err(TypeError::IllegalOsCall { reason }) = result {
            assert!(reason.contains("file_write") && reason.contains("FileAccess"));
        } else { panic!("Expected IllegalOsCall"); }
    }

    #[test]
    fn test_array_literal_type_inference() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Let {
                                name: "items".to_string(),
                                ty: Some(TypeNode::Array(Box::new(TypeNode::Int))),
                                value: Expression::ArrayLiteral(vec![
                                    Expression::Int(1),
                                    Expression::Int(2),
                                    Expression::Int(3),
                                ]),
                            }
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_map_literal_type_inference() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Let {
                                name: "config".to_string(),
                                ty: Some(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String))),
                                value: Expression::MapLiteral(vec![
                                    (Expression::String("key".to_string()), Expression::String("val".to_string())),
                                ]),
                            }
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_type_alias_registration() {
        let program = Program {
            no_std: false,
            items: vec![
                Item::TypeAlias { name: "UserId".to_string(), target: TypeNode::String },
            ]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
        assert!(checker.type_aliases.contains_key("UserId"));
        assert_eq!(checker.type_aliases["UserId"], TypeNode::String);
    }

    #[test]
    fn test_valid_unsafe_file_read() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: true,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Read".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::UnsafeBlock(Block {
                                statements: vec![
                                    Statement::Expr(Expression::MethodCall {
                                        caller: Box::new(Expression::Identifier("self".to_string())),
                                        method_name: "file_read".to_string(),
                                        args: vec![Expression::String("/tmp/data".to_string())],
                                    })
                                ]
                            })
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_method_args_registered_in_scope() {
        // Method args should be usable in the body
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Echo".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "msg".to_string(), ty: TypeNode::String }],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block {
                        statements: vec![
                            Statement::Return(Some(Expression::Identifier("msg".to_string())))
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_try_catch_registers_error_var() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::TryCatch {
                                try_block: Block {
                                    statements: vec![Statement::Throw(Expression::String("oops".to_string()))]
                                },
                                catch_var: "err".to_string(),
                                catch_block: Block {
                                    statements: vec![
                                        Statement::Print(Expression::Identifier("err".to_string()))
                                    ]
                                },
                            }
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    // ---- Plan 07: Type System Tests ----

    #[test]
    fn test_nullable_null_assignment() {
        // string? name = null; → OK
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Let {
                                name: "name".to_string(),
                                ty: Some(TypeNode::Nullable(Box::new(TypeNode::String))),
                                value: Expression::Null,
                            }
                        ]
                    })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_nullable_value_assignment() {
        // string? name = "hello"; → OK (non-null value can be assigned to nullable)
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Let {
                                name: "name".to_string(),
                                ty: Some(TypeNode::Nullable(Box::new(TypeNode::String))),
                                value: Expression::String("hello".to_string()),
                            }
                        ]
                    })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_non_nullable_null_assignment_fails() {
        // string name = null; → ERROR (can't assign null to non-nullable)
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Let {
                                name: "name".to_string(),
                                ty: Some(TypeNode::String),
                                value: Expression::Null,
                            }
                        ]
                    })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_err());
    }

    #[test]
    fn test_enum_registration() {
        // enum Status { Active, Inactive } → registered in checker
        let program = Program {
            no_std: false,
            items: vec![
                Item::Enum(EnumDef {
                    name: "Status".to_string(),
                    is_public: true,
                    variants: vec![
                        EnumVariant { name: "Active".to_string(), fields: vec![] },
                        EnumVariant { name: "Inactive".to_string(), fields: vec![] },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "Test".to_string(),
                    is_system: false,
                    is_public: false,
                    target_annotation: None,
                    annotations: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        is_public: true,
                        annotations: vec![],
                        type_params: vec![],
                        constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![] })
                    }]
                })
            ]
        };

        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
        assert!(checker.enum_defs.contains_key("Status"));
        assert_eq!(checker.enum_defs["Status"].len(), 2);
    }

    // ---- Plan 03: OCAP Capability Token Tests ----

    #[test]
    fn test_ocap_capability_token_grants_fetch() {
        // Method with NetworkAccess token should be able to call fetch without unsafe
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "ApiClient".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "GetData".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "net".to_string(), ty: TypeNode::Capability(CapabilityType::NetworkAccess) }],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block {
                        statements: vec![
                            Statement::Return(Some(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "fetch".to_string(),
                                args: vec![Expression::String("https://api.example.com".to_string())],
                            }))
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_ocap_capability_token_grants_file_read() {
        // FileAccess token should grant file_read permission
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "FileReader".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "ReadConfig".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "fs".to_string(), ty: TypeNode::Capability(CapabilityType::FileAccess) }],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block {
                        statements: vec![
                            Statement::Return(Some(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "file_read".to_string(),
                                args: vec![Expression::String("config.json".to_string())],
                            }))
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_ocap_wrong_capability_still_denied() {
        // FileAccess token should NOT grant fetch (needs NetworkAccess)
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "fs".to_string(), ty: TypeNode::Capability(CapabilityType::FileAccess) }],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "fetch".to_string(),
                                args: vec![Expression::String("https://evil.com".to_string())],
                            })
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err());
        if let Err(TypeError::IllegalOsCall { reason }) = result {
            assert!(reason.contains("NetworkAccess"));
        } else { panic!("Expected IllegalOsCall"); }
    }

    #[test]
    fn test_ocap_llm_access_token_grants_llm_infer() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "AiAgent".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Think".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "llm".to_string(), ty: TypeNode::Capability(CapabilityType::LlmAccess) }],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block {
                        statements: vec![
                            Statement::Return(Some(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "llm_infer".to_string(),
                                args: vec![Expression::String("What is 2+2?".to_string())],
                            }))
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_ocap_db_access_token_grants_query() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "DbReader".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "ReadAll".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "db".to_string(), ty: TypeNode::Capability(CapabilityType::DbAccess) }],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block {
                        statements: vec![
                            Statement::Return(Some(Expression::Query(SurrealQueryNode { raw_query: "SELECT * FROM users".to_string() })))
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    // ---- Plan 06: Match Statement Tests ----

    #[test]
    fn test_match_statement_valid() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Let { name: "x".to_string(), ty: None, value: Expression::Int(42) },
                            Statement::Match {
                                subject: Expression::Identifier("x".to_string()),
                                arms: vec![
                                    MatchArm {
                                        pattern: Pattern::Literal(Expression::Int(1)),
                                        body: Block { statements: vec![Statement::Print(Expression::String("one".to_string()))] },
                                    },
                                    MatchArm {
                                        pattern: Pattern::Wildcard,
                                        body: Block { statements: vec![Statement::Print(Expression::String("other".to_string()))] },
                                    },
                                ],
                            }
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_match_variant_with_bindings() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Let { name: "val".to_string(), ty: None, value: Expression::Int(10) },
                            Statement::Match {
                                subject: Expression::Identifier("val".to_string()),
                                arms: vec![
                                    MatchArm {
                                        pattern: Pattern::Variant("Some".to_string(), vec!["inner".to_string()]),
                                        body: Block {
                                            statements: vec![
                                                Statement::Print(Expression::Identifier("inner".to_string()))
                                            ]
                                        },
                                    },
                                    MatchArm {
                                        pattern: Pattern::Wildcard,
                                        body: Block { statements: vec![] },
                                    },
                                ],
                            }
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    // ---- Plan 06: Lambda Expression Tests ----

    #[test]
    fn test_lambda_expression_valid() {
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block {
                        statements: vec![
                            Statement::Let {
                                name: "add".to_string(),
                                ty: None,
                                value: Expression::Lambda {
                                    params: vec![
                                        FieldDecl { name: "a".to_string(), ty: TypeNode::Int },
                                        FieldDecl { name: "b".to_string(), ty: TypeNode::Int },
                                    ],
                                    return_ty: Some(Box::new(TypeNode::Int)),
                                    body: Box::new(LambdaBody::Expression(
                                        Expression::BinaryOp {
                                            left: Box::new(Expression::Identifier("a".to_string())),
                                            operator: BinaryOperator::Add,
                                            right: Box::new(Expression::Identifier("b".to_string())),
                                        }
                                    )),
                                },
                            }
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_lambda_inferred_type() {
        // Lambda should infer as Func type
        let mut checker = TypeChecker::new();
        let lambda_expr = Expression::Lambda {
            params: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::String }],
            return_ty: Some(Box::new(TypeNode::Int)),
            body: Box::new(LambdaBody::Expression(Expression::Int(42))),
        };
        let ty = checker.infer_expression_type(&lambda_expr).unwrap();
        assert_eq!(ty, TypeNode::Func(vec![TypeNode::String], Box::new(TypeNode::Int)));
    }

    // ===== Plan 06: Destructuring Type-Checking =====

    #[test]
    fn test_destructure_tuple_binds_variables() {
        let mut checker = TypeChecker::new();
        let block = Block {
            statements: vec![
                Statement::LetDestructure {
                    pattern: DestructurePattern::Tuple(vec!["x".to_string(), "y".to_string()]),
                    value: Expression::Identifier("some_tuple".to_string()),
                },
            ],
        };
        // Register "some_tuple" so it doesn't fail on undeclared
        checker.env.insert("some_tuple".to_string(), TypeNode::Custom("Pair".to_string()));
        checker.check_block(&block).unwrap();
        // After destructuring, x and y should be in scope
        assert!(checker.env.contains_key("x"));
        assert!(checker.env.contains_key("y"));
    }

    #[test]
    fn test_destructure_struct_binds_variables() {
        let mut checker = TypeChecker::new();
        let block = Block {
            statements: vec![
                Statement::LetDestructure {
                    pattern: DestructurePattern::Struct(vec![
                        ("name".to_string(), None),
                        ("age".to_string(), Some("a".to_string())),
                    ]),
                    value: Expression::Identifier("person".to_string()),
                },
            ],
        };
        checker.env.insert("person".to_string(), TypeNode::Custom("Person".to_string()));
        checker.check_block(&block).unwrap();
        // "name" bound directly, "age" bound via alias "a"
        assert!(checker.env.contains_key("name"));
        assert!(checker.env.contains_key("a"));
    }

    #[test]
    fn test_destructure_tuple_value_must_be_valid() {
        let mut checker = TypeChecker::new();
        let block = Block {
            statements: vec![
                Statement::LetDestructure {
                    pattern: DestructurePattern::Tuple(vec!["x".to_string()]),
                    value: Expression::Identifier("nonexistent".to_string()),
                },
            ],
        };
        // Should fail because "nonexistent" is not declared
        assert!(checker.check_block(&block).is_err());
    }
}
