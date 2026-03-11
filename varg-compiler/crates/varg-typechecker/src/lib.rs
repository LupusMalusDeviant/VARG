use varg_ast::ast::*;
use std::collections::HashMap;

/// Semantic errors discovered during Type Checking or OCAP validation
#[derive(Debug, PartialEq)]
pub enum TypeError {
    TypeMismatch { expected: String, found: String },
    UndeclaredVariable(String),
    IllegalOsCall { reason: String }, // OCAP Violation
    // Plan 21: Dedicated capability error for flow analysis
    MissingCapability { capability: String, operation: String },
    CapabilityConstructionOutsideUnsafe { capability: String },
    // Plan 30: Type System Hardening
    UnknownField { type_name: String, field_name: String },
    UnknownMethod { type_name: String, method_name: String },
    NonExhaustiveMatch { type_name: String, missing_variants: Vec<String> },
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
            TypeError::MissingCapability { capability, operation } => {
                format!("operation `{}` requires `{}` capability token — pass it as a parameter or use `unsafe {{}}`", operation, capability)
            }
            TypeError::CapabilityConstructionOutsideUnsafe { capability } => {
                format!("`{}` capability token cannot be constructed outside `unsafe` block", capability)
            }
            TypeError::UnknownField { type_name, field_name } => {
                format!("unknown field `{}` on type `{}`", field_name, type_name)
            }
            TypeError::UnknownMethod { type_name, method_name } => {
                format!("unknown method `{}` on type `{}`", method_name, type_name)
            }
            TypeError::NonExhaustiveMatch { type_name, missing_variants } => {
                format!("non-exhaustive match on `{}`: missing variant(s) {}", type_name, missing_variants.join(", "))
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

    // Wave 5b: Expected return type for current method (for validation)
    current_return_ty: Option<TypeNode>,

    // Plan 19: Agent fields available in method scope
    current_agent_fields: Vec<FieldDecl>,

    // Plan 30: Registered struct fields (struct_name → fields)
    struct_fields: HashMap<String, Vec<FieldDecl>>,
    // Plan 30: Registered agent fields (agent_name → fields)
    agent_fields: HashMap<String, Vec<FieldDecl>>,
    // Plan 30: Registered method signatures (type_name → method_name → signature)
    method_signatures: HashMap<String, HashMap<String, MethodSignature>>,
}

// Plan 30: Method signature for return-type tracking
#[derive(Debug, Clone)]
struct MethodSignature {
    return_ty: Option<TypeNode>,
    #[allow(dead_code)]
    args: Vec<FieldDecl>,
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
            current_return_ty: None,
            current_agent_fields: Vec::new(),
            struct_fields: HashMap::new(),
            agent_fields: HashMap::new(),
            method_signatures: HashMap::new(),
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
            Err(TypeError::MissingCapability {
                capability: cap_name.to_string(),
                operation: operation.to_string(),
            })
        }
    }

    /// Plan 21: Check that capability arguments being passed are actually available
    fn check_capability_propagation(&self, args: &[Expression]) -> Result<(), TypeError> {
        for arg in args {
            if let Expression::Identifier(name) = arg {
                if let Some(TypeNode::Capability(cap)) = self.env.get(name) {
                    // Check if this capability is available in the current scope
                    if !self.in_unsafe_block && !self.has_capability(cap) {
                        let cap_name = match cap {
                            CapabilityType::NetworkAccess => "NetworkAccess",
                            CapabilityType::FileAccess => "FileAccess",
                            CapabilityType::DbAccess => "DbAccess",
                            CapabilityType::LlmAccess => "LlmAccess",
                            CapabilityType::SystemAccess => "SystemAccess",
                        };
                        return Err(TypeError::MissingCapability {
                            capability: cap_name.to_string(),
                            operation: format!("passing {} to method", cap_name),
                        });
                    }
                }
            }
        }
        Ok(())
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
                // Plan 30: Register agent fields for property access resolution
                self.agent_fields.insert(agent.name.clone(), agent.fields.clone());
                // Plan 30: Register method signatures for return-type tracking
                let mut methods = HashMap::new();
                for method in &agent.methods {
                    methods.insert(method.name.clone(), MethodSignature {
                        return_ty: method.return_ty.clone(),
                        args: method.args.clone(),
                    });
                }
                self.method_signatures.insert(agent.name.clone(), methods);

                // Plan 19: Store agent fields so methods can access them
                self.current_agent_fields = agent.fields.clone();
                for method in &agent.methods {
                    self.check_method(method)?;
                }
                self.current_agent_fields.clear();
                Ok(())
            },
            Item::Contract(_contract) => {
                // Contracts are interfaces, no bodies to check right now
                Ok(())
            },
            Item::Struct(s) => {
                // Plan 30: Register struct fields for property access resolution
                self.struct_fields.insert(s.name.clone(), s.fields.clone());
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
            },
            // Plan 25: Standalone functions — type-check like methods
            Item::Function(f) => {
                self.env.clear();
                self.available_capabilities.clear();
                for param in &f.params {
                    self.env.insert(param.name.clone(), param.ty.clone());
                    if let TypeNode::Capability(cap) = &param.ty {
                        self.available_capabilities.push(cap.clone());
                    }
                }
                self.current_return_ty = f.return_ty.clone();
                self.check_block(&f.body)?;
                self.current_return_ty = None;
                Ok(())
            },
            // Plan 23: Prompt templates — validate param types
            Item::PromptTemplate(pt) => {
                for param in &pt.params {
                    match &param.ty {
                        TypeNode::String | TypeNode::Int | TypeNode::Bool => {},
                        other => {
                            return Err(TypeError::TypeMismatch {
                                expected: "String, Int, or Bool".to_string(),
                                found: format!("{:?}", other),
                            });
                        }
                    }
                }
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

        // Validate generic constraints: each constraint must reference a declared type param
        for constraint in &method.constraints {
            if !method.type_params.contains(&constraint.type_param) {
                return Err(TypeError::TypeMismatch {
                    expected: format!("declared type parameter for constraint `where {}: {}`", constraint.type_param, constraint.bound),
                    found: format!("undeclared type parameter `{}`", constraint.type_param),
                });
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

        // Plan 19: Register agent fields in method scope
        for field in &self.current_agent_fields.clone() {
            self.env.insert(field.name.clone(), field.ty.clone());
        }

        // Register arguments in the environment
        for arg in &method.args {
            self.env.insert(arg.name.clone(), arg.ty.clone());
        }

        // Track expected return type for return-statement validation
        self.current_return_ty = method.return_ty.clone();

        if let Some(body) = &method.body {
            self.check_block(body)?;
        }

        self.current_return_ty = None;
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
                Statement::IndexAssign { target, index, value } => {
                    self.infer_expression_type(target)?;
                    self.infer_expression_type(index)?;
                    self.infer_expression_type(value)?;
                },
                Statement::PropertyAssign { target, property: _, value } => {
                    self.infer_expression_type(target)?;
                    self.infer_expression_type(value)?;
                },
                Statement::UnsafeBlock(inner_block) => {
                    self.in_unsafe_block = true;
                    self.check_block(inner_block)?;
                    self.in_unsafe_block = previous_unsafe;
                },

                Statement::Const { name, ty, value } => {
                    let inferred = self.infer_expression_type(value)?;
                    let expected = ty.clone().unwrap_or(inferred.clone());
                    if expected != inferred && inferred != TypeNode::Custom("Dynamic".to_string()) {
                        return Err(TypeError::TypeMismatch {
                            expected: format!("{:?}", expected),
                            found: format!("{:?}", inferred),
                        });
                    }
                    self.env.insert(name.clone(), expected);
                },
                Statement::Break => {},
                Statement::Continue => {},
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
                     let coll_ty = self.infer_expression_type(collection)?;
                     // Extract inner type from Array/List, fall back to Dynamic
                     let item_ty = match &coll_ty {
                         TypeNode::Array(inner) => *inner.clone(),
                         TypeNode::List(inner) => *inner.clone(),
                         _ => TypeNode::Custom("Dynamic".to_string()),
                     };
                     self.env.insert(item_name.clone(), item_ty);
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
                    let val_type = self.infer_expression_type(expr)?;
                    // Validate return type matches method declaration
                    if let Some(expected) = &self.current_return_ty {
                        if *expected != TypeNode::Void && !self.types_match(expected, &val_type) {
                            return Err(TypeError::TypeMismatch {
                                expected: format!("{:?}", expected),
                                found: format!("{:?}", val_type),
                            });
                        }
                    }
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
                    let subject_ty = self.infer_expression_type(subject)?;

                    // Plan 30: Exhaustiveness check for enum types
                    if let TypeNode::Custom(ref enum_name) = subject_ty {
                        if let Some(variants) = self.enum_defs.get(enum_name).cloned() {
                            let has_wildcard = arms.iter().any(|arm|
                                matches!(arm.pattern, Pattern::Wildcard)
                            );
                            if !has_wildcard {
                                let matched_variants: std::collections::HashSet<String> = arms.iter()
                                    .filter_map(|arm| {
                                        if let Pattern::Variant(variant_name, _) = &arm.pattern {
                                            Some(variant_name.clone())
                                        } else { None }
                                    })
                                    .collect();
                                let missing: Vec<String> = variants.iter()
                                    .filter(|v| !matched_variants.contains(&v.name))
                                    .map(|v| v.name.clone())
                                    .collect();
                                if !missing.is_empty() {
                                    return Err(TypeError::NonExhaustiveMatch {
                                        type_name: enum_name.clone(),
                                        missing_variants: missing,
                                    });
                                }
                            }
                        }
                    }

                    // Check each arm's body
                    for arm in arms {
                        let saved_env = self.env.clone();
                        // For Variant patterns with bindings, try to narrow types from enum definition
                        if let Pattern::Variant(variant_name, bindings) = &arm.pattern {
                            // Try to look up field types from enum definition
                            let field_types = self.resolve_variant_field_types(&subject_ty, variant_name);
                            for (i, binding) in bindings.iter().enumerate() {
                                let ty = field_types.get(i).cloned()
                                    .unwrap_or_else(|| TypeNode::Custom("Dynamic".to_string()));
                                self.env.insert(binding.clone(), ty);
                            }
                        }
                        self.check_block(&arm.body)?;
                        self.env = saved_env;
                    }
                },
                // Plan 20: select statement
                Statement::Select { arms } => {
                    for arm in arms {
                        let saved_env = self.env.clone();
                        match &arm.source {
                            SelectSource::Agent(expr) => {
                                self.infer_expression_type(expr)?;
                                // Bind the var_name as String (messages are strings in MVP)
                                self.env.insert(arm.var_name.clone(), TypeNode::String);
                            },
                            SelectSource::Timeout(expr) => {
                                let ty = self.infer_expression_type(expr)?;
                                if ty != TypeNode::Int {
                                    return Err(TypeError::TypeMismatch {
                                        expected: "Int".to_string(),
                                        found: format!("{:?}", ty),
                                    });
                                }
                            },
                        }
                        self.check_block(&arm.body)?;
                        self.env = saved_env;
                    }
                },
            }
        }
        Ok(())
    }

    /// Resolve field types for a variant from an enum definition.
    /// Returns the field types in order, or empty vec if not found.
    fn resolve_variant_field_types(&self, subject_ty: &TypeNode, variant_name: &str) -> Vec<TypeNode> {
        // Try to find the enum name from the subject type
        let enum_name = match subject_ty {
            TypeNode::Custom(name) => name.as_str(),
            _ => return Vec::new(),
        };
        // Look up the enum definition
        if let Some(variants) = self.enum_defs.get(enum_name) {
            for variant in variants {
                if variant.name == variant_name {
                    return variant.fields.iter().map(|(_, ty)| ty.clone()).collect();
                }
            }
        }
        Vec::new()
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
            Expression::BinaryOp { left, operator, right } => {
                let left_ty = self.infer_expression_type(left)?;
                let right_ty = self.infer_expression_type(right)?;
                match operator {
                    BinaryOperator::Eq | BinaryOperator::NotEq |
                    BinaryOperator::Lt | BinaryOperator::Gt |
                    BinaryOperator::LtEq | BinaryOperator::GtEq |
                    BinaryOperator::And | BinaryOperator::Or => Ok(TypeNode::Bool),
                    BinaryOperator::CosineSim => Ok(TypeNode::Custom("f32".to_string())),
                    BinaryOperator::Add => {
                        // String + anything = String (concat)
                        if left_ty == TypeNode::String || right_ty == TypeNode::String {
                            Ok(TypeNode::String)
                        } else {
                            Ok(TypeNode::Int)
                        }
                    },
                    _ => Ok(TypeNode::Int)
                }
            },
            Expression::Await(inner) => {
                // await unwraps the future — for MVP, return the inner expression type
                self.infer_expression_type(inner)
            },
            Expression::UnaryOp { operator, operand } => {
                let inner_ty = self.infer_expression_type(operand)?;
                match operator {
                    UnaryOperator::Negate => Ok(inner_ty), // -x keeps the same type
                    UnaryOperator::Not => Ok(TypeNode::Bool), // !x always Bool
                }
            },
            Expression::MethodCall { caller, method_name, args } => {
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
                // ===== Wave 5: String Methods (caller.method() style) =====
                } else if method_name == "len" {
                    Ok(TypeNode::Int)
                } else if method_name == "contains" || method_name == "starts_with" || method_name == "ends_with" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Bool)
                } else if method_name == "to_upper" || method_name == "to_lower" || method_name == "trim" {
                    Ok(TypeNode::String)
                } else if method_name == "substring" {
                    if args.len() != 2 {
                        return Err(TypeError::TypeMismatch { expected: "2 arguments (start, length)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "char_at" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (index)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "index_of" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (substring)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Int)
                } else if method_name == "split" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (delimiter)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                } else if method_name == "replace" {
                    if args.len() != 2 {
                        return Err(TypeError::TypeMismatch { expected: "2 arguments (search, replace)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                // ===== Wave 5: Collection Methods =====
                } else if method_name == "push" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (item)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Void)
                } else if method_name == "pop" {
                    Ok(TypeNode::Custom("Dynamic".to_string()))
                } else if method_name == "reverse" {
                    Ok(TypeNode::Void)
                } else if method_name == "is_empty" || method_name == "contains_key" {
                    Ok(TypeNode::Bool)
                } else if method_name == "keys" || method_name == "values" {
                    Ok(TypeNode::Array(Box::new(TypeNode::Custom("Dynamic".to_string()))))
                } else if method_name == "remove" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (key)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Void)
                // ===== Plan 16: Agent Messaging Methods =====
                } else if method_name == "send" {
                    if args.is_empty() {
                        return Err(TypeError::TypeMismatch { expected: "at least 1 argument (method name)".to_string(), found: "0 arguments".to_string() });
                    }
                    Ok(TypeNode::Void)
                } else if method_name == "request" {
                    if args.is_empty() {
                        return Err(TypeError::TypeMismatch { expected: "at least 1 argument (method name)".to_string(), found: "0 arguments".to_string() });
                    }
                    Ok(TypeNode::String) // MVP: all responses are String
                } else {
                    // Plan 30: Look up method signatures on known types
                    let caller_ty = self.infer_expression_type(caller)?;
                    if let TypeNode::Custom(ref type_name) = caller_ty {
                        if let Some(methods) = self.method_signatures.get(type_name) {
                            if let Some(sig) = methods.get(method_name.as_str()) {
                                return Ok(sig.return_ty.clone().unwrap_or(TypeNode::Void));
                            }
                            // Known type but unknown method → error
                            return Err(TypeError::UnknownMethod {
                                type_name: type_name.clone(),
                                method_name: method_name.clone(),
                            });
                        }
                    }
                    // Fallback for unknown types: assume void
                    Ok(TypeNode::Void)
                }
            },
            Expression::PropertyAccess { caller, property_name } => {
                let caller_ty = self.infer_expression_type(caller)?;
                // Hardcoded built-in properties
                if *property_name == "text" && caller_ty == TypeNode::Prompt {
                    return Ok(TypeNode::String);
                }
                if caller_ty == TypeNode::Tensor && *property_name == "data" {
                    return Ok(TypeNode::Array(Box::new(TypeNode::Custom("f32".to_string()))));
                }
                // Plan 30: Struct/Agent field lookup
                if let TypeNode::Custom(ref type_name) = caller_ty {
                    // Check struct fields
                    if let Some(fields) = self.struct_fields.get(type_name) {
                        if let Some(field) = fields.iter().find(|f| f.name == *property_name) {
                            return Ok(field.ty.clone());
                        }
                        // Known struct but unknown field → error
                        return Err(TypeError::UnknownField {
                            type_name: type_name.clone(),
                            field_name: property_name.clone(),
                        });
                    }
                    // Check agent fields
                    if let Some(fields) = self.agent_fields.get(type_name) {
                        if let Some(field) = fields.iter().find(|f| f.name == *property_name) {
                            return Ok(field.ty.clone());
                        }
                        // Known agent but unknown field → error
                        return Err(TypeError::UnknownField {
                            type_name: type_name.clone(),
                            field_name: property_name.clone(),
                        });
                    }
                }
                // Fallback for unknown/complex types
                Ok(TypeNode::Custom("Dynamic".to_string()))
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
            },
            // Wave 6: retry returns whatever the body's last expression returns
            Expression::Retry { max_attempts, body, fallback } => {
                let attempts_ty = self.infer_expression_type(max_attempts)?;
                if attempts_ty != TypeNode::Int {
                    return Err(TypeError::TypeMismatch {
                        expected: "Int".to_string(),
                        found: format!("{:?}", attempts_ty),
                    });
                }
                self.check_block(body)?;
                if let Some(fb) = fallback {
                    self.check_block(fb)?;
                }
                Ok(TypeNode::Void)
            },
            // Plan 16: spawn returns an AgentHandle type
            Expression::Spawn { agent_name, args } => {
                for arg in args {
                    self.infer_expression_type(arg)?;
                }
                Ok(TypeNode::AgentHandle(agent_name.clone()))
            },
            // Plan 24: expr? — must be inside a method that returns Result
            Expression::TryPropagate(expr) => {
                let inner_ty = self.infer_expression_type(expr)?;
                // If the expression is Result<T, E>, the ? unwraps to T
                if let TypeNode::Result(ok_ty, _) = inner_ty {
                    Ok(*ok_ty)
                } else {
                    // Allow ? on any expression (runtime will handle)
                    Ok(inner_ty)
                }
            },
            // Plan 24: expr or default — returns inner type
            Expression::OrDefault { expr, default } => {
                let expr_ty = self.infer_expression_type(expr)?;
                let default_ty = self.infer_expression_type(default)?;
                // If expr is Result<T, E>, return T (the unwrapped type)
                if let TypeNode::Result(ok_ty, _) = expr_ty {
                    Ok(*ok_ty)
                } else {
                    Ok(default_ty)
                }
            },
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
        let msg = result.unwrap_err().message();
        assert!(msg.contains("query") || msg.contains("DbAccess"));
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
        let msg = result.unwrap_err().message();
        assert!(msg.contains("fetch") && msg.contains("NetworkAccess"));
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
        let msg = result.unwrap_err().message();
        assert!(msg.contains("llm_infer") && msg.contains("LlmAccess"));
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
        let msg = result.unwrap_err().message();
        assert!(msg.contains("file_read") && msg.contains("FileAccess"));
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
        let msg = result.unwrap_err().message();
        assert!(msg.contains("file_write") && msg.contains("FileAccess"));
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                    fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
        let msg = result.unwrap_err().message();
        assert!(msg.contains("NetworkAccess"));
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
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

    // ===== Stabilization: TypeChecker Improvements =====

    #[test]
    fn test_match_variant_type_narrowing() {
        // When matching an enum, variant bindings should get their declared field types
        let mut checker = TypeChecker::new();
        // Register enum with a variant that has a String field
        checker.enum_defs.insert("Result".to_string(), vec![
            EnumVariant { name: "Ok".to_string(), fields: vec![("value".to_string(), TypeNode::String)] },
            EnumVariant { name: "Err".to_string(), fields: vec![("msg".to_string(), TypeNode::String)] },
        ]);
        // Register the subject variable as the enum type
        checker.env.insert("res".to_string(), TypeNode::Custom("Result".to_string()));

        let block = Block {
            statements: vec![
                Statement::Match {
                    subject: Expression::Identifier("res".to_string()),
                    arms: vec![
                        MatchArm {
                            pattern: Pattern::Variant("Ok".to_string(), vec!["val".to_string()]),
                            body: Block { statements: vec![
                                // val should be usable (bound as String from the enum variant)
                                Statement::Print(Expression::Identifier("val".to_string())),
                            ]},
                        },
                        MatchArm {
                            pattern: Pattern::Wildcard,
                            body: Block { statements: vec![] },
                        },
                    ],
                },
            ],
        };
        // Should succeed — val is narrowed to String from the enum definition
        assert!(checker.check_block(&block).is_ok());
    }

    #[test]
    fn test_foreach_infers_item_type_from_array() {
        let mut checker = TypeChecker::new();
        // Register a variable as Array<Int>
        checker.env.insert("nums".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));

        let block = Block {
            statements: vec![
                Statement::Foreach {
                    item_name: "n".to_string(),
                    collection: Expression::Identifier("nums".to_string()),
                    body: Block { statements: vec![
                        // Use n in a context that requires Int (comparison with Int)
                        Statement::Let {
                            name: "doubled".to_string(),
                            ty: Some(TypeNode::Int),
                            value: Expression::BinaryOp {
                                left: Box::new(Expression::Identifier("n".to_string())),
                                operator: BinaryOperator::Mul,
                                right: Box::new(Expression::Int(2)),
                            },
                        },
                    ]},
                },
            ],
        };
        assert!(checker.check_block(&block).is_ok());
    }

    #[test]
    fn test_generic_constraint_undeclared_type_param_fails() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Sort".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec!["T".to_string()],
                    constraints: vec![
                        GenericConstraint { type_param: "U".to_string(), bound: "Comparable".to_string() },
                    ],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] }),
                }],
            })],
        };
        // Should fail because "U" is not in type_params
        let result = checker.check_program(&program);
        assert!(result.is_err());
    }

    #[test]
    fn test_generic_constraint_valid_type_param_passes() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Sort".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec!["T".to_string()],
                    constraints: vec![
                        GenericConstraint { type_param: "T".to_string(), bound: "Comparable".to_string() },
                    ],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] }),
                }],
            })],
        };
        // Should succeed because "T" is in type_params
        assert!(checker.check_program(&program).is_ok());
    }

    // ===== New Operators: &&, ||, !, %, unary, string concat =====

    #[test]
    fn test_and_or_returns_bool() {
        let mut checker = TypeChecker::new();
        // true && false → Bool
        let ty = checker.infer_expression_type(&Expression::BinaryOp {
            left: Box::new(Expression::Bool(true)),
            operator: BinaryOperator::And,
            right: Box::new(Expression::Bool(false)),
        }).unwrap();
        assert_eq!(ty, TypeNode::Bool);

        // true || false → Bool
        let ty2 = checker.infer_expression_type(&Expression::BinaryOp {
            left: Box::new(Expression::Bool(true)),
            operator: BinaryOperator::Or,
            right: Box::new(Expression::Bool(false)),
        }).unwrap();
        assert_eq!(ty2, TypeNode::Bool);
    }

    #[test]
    fn test_string_concat_returns_string() {
        let mut checker = TypeChecker::new();
        // "hello" + " world" → String
        let ty = checker.infer_expression_type(&Expression::BinaryOp {
            left: Box::new(Expression::String("hello".to_string())),
            operator: BinaryOperator::Add,
            right: Box::new(Expression::String(" world".to_string())),
        }).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_string_concat_mixed_returns_string() {
        let mut checker = TypeChecker::new();
        // "count: " + 5 → String (string on left promotes to string)
        let ty = checker.infer_expression_type(&Expression::BinaryOp {
            left: Box::new(Expression::String("count: ".to_string())),
            operator: BinaryOperator::Add,
            right: Box::new(Expression::Int(5)),
        }).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_modulo_returns_int() {
        let mut checker = TypeChecker::new();
        // 10 % 3 → Int
        let ty = checker.infer_expression_type(&Expression::BinaryOp {
            left: Box::new(Expression::Int(10)),
            operator: BinaryOperator::Mod,
            right: Box::new(Expression::Int(3)),
        }).unwrap();
        assert_eq!(ty, TypeNode::Int);
    }

    #[test]
    fn test_unary_negate_returns_int() {
        let mut checker = TypeChecker::new();
        // -5 → Int
        let ty = checker.infer_expression_type(&Expression::UnaryOp {
            operator: UnaryOperator::Negate,
            operand: Box::new(Expression::Int(5)),
        }).unwrap();
        assert_eq!(ty, TypeNode::Int);
    }

    #[test]
    fn test_unary_not_returns_bool() {
        let mut checker = TypeChecker::new();
        // !true → Bool
        let ty = checker.infer_expression_type(&Expression::UnaryOp {
            operator: UnaryOperator::Not,
            operand: Box::new(Expression::Bool(true)),
        }).unwrap();
        assert_eq!(ty, TypeNode::Bool);
    }

    // ===== Wave 5: String & Collection Method Types =====

    #[test]
    fn test_string_method_types() {
        let mut checker = TypeChecker::new();
        let caller = Box::new(Expression::String("test".to_string()));

        // .len() → Int
        let ty = checker.infer_expression_type(&Expression::MethodCall {
            caller: caller.clone(), method_name: "len".to_string(), args: vec![],
        }).unwrap();
        assert_eq!(ty, TypeNode::Int);

        // .contains("x") → Bool
        let ty = checker.infer_expression_type(&Expression::MethodCall {
            caller: caller.clone(), method_name: "contains".to_string(),
            args: vec![Expression::String("x".to_string())],
        }).unwrap();
        assert_eq!(ty, TypeNode::Bool);

        // .to_upper() → String
        let ty = checker.infer_expression_type(&Expression::MethodCall {
            caller: caller.clone(), method_name: "to_upper".to_string(), args: vec![],
        }).unwrap();
        assert_eq!(ty, TypeNode::String);

        // .index_of("x") → Int
        let ty = checker.infer_expression_type(&Expression::MethodCall {
            caller: caller.clone(), method_name: "index_of".to_string(),
            args: vec![Expression::String("x".to_string())],
        }).unwrap();
        assert_eq!(ty, TypeNode::Int);
    }

    #[test]
    fn test_collection_method_types() {
        let mut checker = TypeChecker::new();
        let caller = Box::new(Expression::Identifier("arr".to_string()));

        // .push(1) → Void
        let ty = checker.infer_expression_type(&Expression::MethodCall {
            caller: caller.clone(), method_name: "push".to_string(),
            args: vec![Expression::Int(1)],
        }).unwrap();
        assert_eq!(ty, TypeNode::Void);

        // .is_empty() → Bool
        let ty = checker.infer_expression_type(&Expression::MethodCall {
            caller: caller.clone(), method_name: "is_empty".to_string(), args: vec![],
        }).unwrap();
        assert_eq!(ty, TypeNode::Bool);

        // .keys() → Array
        let ty = checker.infer_expression_type(&Expression::MethodCall {
            caller: caller.clone(), method_name: "keys".to_string(), args: vec![],
        }).unwrap();
        assert!(matches!(ty, TypeNode::Array(_)));
    }

    // ===== Wave 5: const =====

    #[test]
    fn test_const_type_inference() {
        let mut checker = TypeChecker::new();
        let block = Block { statements: vec![
            Statement::Const {
                name: "MAX".to_string(),
                ty: Some(TypeNode::Int),
                value: Expression::Int(100),
            },
        ]};
        assert!(checker.check_block(&block).is_ok());
    }

    // ===== Wave 5b: Return-Type Validation =====

    #[test]
    fn test_return_type_correct_passes() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "GetName".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                        Statement::Return(Some(Expression::String("hello".to_string())))
                    ]}),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_return_type_mismatch_fails() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "GetCount".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Int),
                    body: Some(Block { statements: vec![
                        // Return String when Int expected → should FAIL
                        Statement::Return(Some(Expression::String("oops".to_string())))
                    ]}),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_err());
    }

    #[test]
    fn test_return_void_allows_anything() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Return(Some(Expression::Int(42)))
                    ]}),
                }],
            })],
        };
        // Void methods don't enforce return type
        assert!(checker.check_program(&program).is_ok());
    }

    // ===== Plan 16: Agent Messaging Tests =====

    #[test]
    fn test_spawn_returns_agent_handle() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Let {
                            name: "worker".to_string(),
                            ty: None,
                            value: Expression::Spawn {
                                agent_name: "Worker".to_string(),
                                args: vec![],
                            },
                        },
                    ]}),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
        // The variable 'worker' should have AgentHandle type (registered in env)
    }

    #[test]
    fn test_send_validates_args() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Expr(Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("worker".to_string())),
                            method_name: "send".to_string(),
                            args: vec![
                                Expression::String("Process".to_string()),
                                Expression::String("data".to_string()),
                            ],
                        }),
                    ]}),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_request_returns_string() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Let {
                            name: "result".to_string(),
                            ty: None,
                            value: Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("worker".to_string())),
                                method_name: "request".to_string(),
                                args: vec![
                                    Expression::String("Process".to_string()),
                                    Expression::String("data".to_string()),
                                ],
                            },
                        },
                    ]}),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    // ===== Plan 20: Select Statement Test =====

    #[test]
    fn test_select_arm_types() {
        let mut checker = TypeChecker::new();
        // Register a variable so agent expression resolves
        checker.env.insert("worker".to_string(), TypeNode::AgentHandle("Worker".to_string()));
        let block = Block { statements: vec![
            Statement::Select { arms: vec![
                SelectArm {
                    var_name: "msg".to_string(),
                    source: SelectSource::Agent(Expression::Identifier("worker".to_string())),
                    body: Block { statements: vec![
                        Statement::Print(Expression::Identifier("msg".to_string())),
                    ]},
                },
                SelectArm {
                    var_name: "_timeout".to_string(),
                    source: SelectSource::Timeout(Expression::Int(5000)),
                    body: Block { statements: vec![
                        Statement::Print(Expression::String("timed out".to_string())),
                    ]},
                },
            ]},
        ]};
        assert!(checker.check_block(&block).is_ok());
    }

    // ===== Plan 21: OCAP Flow Analysis Tests =====

    #[test]
    fn test_capability_available_from_params() {
        // Method with NetworkAccess param can call fetch
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![
                        FieldDecl { name: "net".to_string(), ty: TypeNode::Capability(CapabilityType::NetworkAccess) },
                    ],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Expr(Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: "fetch".to_string(),
                            args: vec![Expression::String("https://example.com".to_string())],
                        }),
                    ]}),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_capability_missing_error() {
        // Method without capability param cannot call fetch
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Expr(Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: "fetch".to_string(),
                            args: vec![Expression::String("https://example.com".to_string())],
                        }),
                    ]}),
                }],
            })],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
        if let Err(TypeError::MissingCapability { capability, operation }) = result {
            assert_eq!(capability, "NetworkAccess");
            assert_eq!(operation, "fetch");
        } else {
            panic!("Expected MissingCapability error, got {:?}", result);
        }
    }

    #[test]
    fn test_multiple_capabilities_tracking() {
        // Method with multiple capability params can call multiple ops
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![
                        FieldDecl { name: "net".to_string(), ty: TypeNode::Capability(CapabilityType::NetworkAccess) },
                        FieldDecl { name: "fs".to_string(), ty: TypeNode::Capability(CapabilityType::FileAccess) },
                        FieldDecl { name: "db".to_string(), ty: TypeNode::Capability(CapabilityType::DbAccess) },
                    ],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Expr(Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: "fetch".to_string(),
                            args: vec![Expression::String("url".to_string())],
                        }),
                        Statement::Expr(Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: "file_read".to_string(),
                            args: vec![Expression::String("path".to_string())],
                        }),
                        Statement::Expr(Expression::Query(SurrealQueryNode {
                            raw_query: "SELECT * FROM users".to_string(),
                        })),
                    ]}),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_unsafe_allows_all_operations() {
        // Unsafe block bypasses all capability checks
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], // No capabilities!
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::UnsafeBlock(Block { statements: vec![
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "fetch".to_string(),
                                args: vec![Expression::String("url".to_string())],
                            }),
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "file_read".to_string(),
                                args: vec![Expression::String("path".to_string())],
                            }),
                        ]}),
                    ]}),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_missing_capability_error_message() {
        let err = TypeError::MissingCapability {
            capability: "FileAccess".to_string(),
            operation: "file_write".to_string(),
        };
        let msg = err.message();
        assert!(msg.contains("file_write"));
        assert!(msg.contains("FileAccess"));
        assert!(msg.contains("capability token"));
    }

    #[test]
    fn test_capability_construction_error_message() {
        let err = TypeError::CapabilityConstructionOutsideUnsafe {
            capability: "NetworkAccess".to_string(),
        };
        let msg = err.message();
        assert!(msg.contains("NetworkAccess"));
        assert!(msg.contains("unsafe"));
    }

    // ---- Plan 23: Prompt Template Type Checking ----
    #[test]
    fn test_prompt_param_types_valid() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::PromptTemplate(PromptTemplateDef {
                name: "greet".to_string(),
                params: vec![
                    FieldDecl { name: "name".to_string(), ty: TypeNode::String },
                    FieldDecl { name: "count".to_string(), ty: TypeNode::Int },
                ],
                body: "Hello {name}, you have {count} items.".to_string(),
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_prompt_param_types_invalid() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::PromptTemplate(PromptTemplateDef {
                name: "bad".to_string(),
                params: vec![
                    FieldDecl { name: "data".to_string(), ty: TypeNode::Array(Box::new(TypeNode::String)) },
                ],
                body: "Data: {data}".to_string(),
            })],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
    }

    // ---- Plan 24: Error Propagation Type Checking ----
    #[test]
    fn test_try_propagate_unwraps_result() {
        // expr? on Result<String, Error> should infer as String
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                        Statement::Let {
                            name: "data".to_string(),
                            ty: Some(TypeNode::String),
                            value: Expression::TryPropagate(
                                Box::new(Expression::String("hello".to_string()))
                            ),
                        },
                        Statement::Return(Some(Expression::Identifier("data".to_string()))),
                    ]}),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_or_default_type_inference() {
        // expr or "default" should type-check
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                        Statement::Let {
                            name: "data".to_string(),
                            ty: Some(TypeNode::String),
                            value: Expression::OrDefault {
                                expr: Box::new(Expression::String("test".to_string())),
                                default: Box::new(Expression::String("fallback".to_string())),
                            },
                        },
                        Statement::Return(Some(Expression::Identifier("data".to_string()))),
                    ]}),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    // ===== Plan 30: Type System Hardening Tests =====

    // Phase A: Struct field resolution
    #[test]
    fn test_struct_field_type_resolved() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![
                Item::Struct(StructDef {
                    name: "User".to_string(),
                    is_public: false,
                    type_params: vec![],
                    fields: vec![
                        FieldDecl { name: "name".to_string(), ty: TypeNode::String },
                        FieldDecl { name: "age".to_string(), ty: TypeNode::Int },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "u".to_string(), ty: TypeNode::Custom("User".to_string()) }],
                        return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::PropertyAccess {
                                caller: Box::new(Expression::Identifier("u".to_string())),
                                property_name: "name".to_string(),
                            })),
                        ]}),
                    }],
                }),
            ],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_unknown_struct_field_error() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![
                Item::Struct(StructDef {
                    name: "User".to_string(),
                    is_public: false,
                    type_params: vec![],
                    fields: vec![
                        FieldDecl { name: "name".to_string(), ty: TypeNode::String },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "u".to_string(), ty: TypeNode::Custom("User".to_string()) }],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Expr(Expression::PropertyAccess {
                                caller: Box::new(Expression::Identifier("u".to_string())),
                                property_name: "invalid".to_string(),
                            }),
                        ]}),
                    }],
                }),
            ],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
        match result.unwrap_err() {
            TypeError::UnknownField { type_name, field_name } => {
                assert_eq!(type_name, "User");
                assert_eq!(field_name, "invalid");
            },
            e => panic!("Expected UnknownField, got {:?}", e),
        }
    }

    #[test]
    fn test_agent_field_type_resolved() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![
                Item::Agent(AgentDef {
                    name: "Worker".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![],
                    fields: vec![
                        FieldDecl { name: "status".to_string(), ty: TypeNode::String },
                    ],
                    methods: vec![],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "w".to_string(), ty: TypeNode::Custom("Worker".to_string()) }],
                        return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::PropertyAccess {
                                caller: Box::new(Expression::Identifier("w".to_string())),
                                property_name: "status".to_string(),
                            })),
                        ]}),
                    }],
                }),
            ],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    // Phase B: Method return type tracking
    #[test]
    fn test_method_return_type_tracked() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![
                Item::Agent(AgentDef {
                    name: "Helper".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Greet".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![], return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::String("hello".to_string()))),
                        ]}),
                    }],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "h".to_string(), ty: TypeNode::Custom("Helper".to_string()) }],
                        return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            // var x = h.Greet(); — should infer as String, not Void
                            Statement::Let {
                                name: "x".to_string(),
                                ty: Some(TypeNode::String),
                                value: Expression::MethodCall {
                                    caller: Box::new(Expression::Identifier("h".to_string())),
                                    method_name: "Greet".to_string(),
                                    args: vec![],
                                },
                            },
                            Statement::Return(Some(Expression::Identifier("x".to_string()))),
                        ]}),
                    }],
                }),
            ],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_unknown_method_error() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![
                Item::Agent(AgentDef {
                    name: "Helper".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Greet".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![], return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::String("hi".to_string()))),
                        ]}),
                    }],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "h".to_string(), ty: TypeNode::Custom("Helper".to_string()) }],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("h".to_string())),
                                method_name: "Invalid".to_string(),
                                args: vec![],
                            }),
                        ]}),
                    }],
                }),
            ],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
        match result.unwrap_err() {
            TypeError::UnknownMethod { type_name, method_name } => {
                assert_eq!(type_name, "Helper");
                assert_eq!(method_name, "Invalid");
            },
            e => panic!("Expected UnknownMethod, got {:?}", e),
        }
    }

    // Phase C: Match exhaustiveness
    #[test]
    fn test_match_exhaustive_all_variants() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![
                Item::Enum(EnumDef {
                    name: "Color".to_string(), is_public: false,
                    variants: vec![
                        EnumVariant { name: "Red".to_string(), fields: vec![] },
                        EnumVariant { name: "Green".to_string(), fields: vec![] },
                        EnumVariant { name: "Blue".to_string(), fields: vec![] },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "c".to_string(), ty: TypeNode::Custom("Color".to_string()) }],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Match {
                                subject: Expression::Identifier("c".to_string()),
                                arms: vec![
                                    MatchArm { pattern: Pattern::Variant("Red".to_string(), vec![]), body: Block { statements: vec![] } },
                                    MatchArm { pattern: Pattern::Variant("Green".to_string(), vec![]), body: Block { statements: vec![] } },
                                    MatchArm { pattern: Pattern::Variant("Blue".to_string(), vec![]), body: Block { statements: vec![] } },
                                ],
                            },
                        ]}),
                    }],
                }),
            ],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_match_non_exhaustive_error() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![
                Item::Enum(EnumDef {
                    name: "Color".to_string(), is_public: false,
                    variants: vec![
                        EnumVariant { name: "Red".to_string(), fields: vec![] },
                        EnumVariant { name: "Green".to_string(), fields: vec![] },
                        EnumVariant { name: "Blue".to_string(), fields: vec![] },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "c".to_string(), ty: TypeNode::Custom("Color".to_string()) }],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Match {
                                subject: Expression::Identifier("c".to_string()),
                                arms: vec![
                                    MatchArm { pattern: Pattern::Variant("Red".to_string(), vec![]), body: Block { statements: vec![] } },
                                    // Missing Green and Blue!
                                ],
                            },
                        ]}),
                    }],
                }),
            ],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
        match result.unwrap_err() {
            TypeError::NonExhaustiveMatch { type_name, missing_variants } => {
                assert_eq!(type_name, "Color");
                assert!(missing_variants.contains(&"Green".to_string()));
                assert!(missing_variants.contains(&"Blue".to_string()));
            },
            e => panic!("Expected NonExhaustiveMatch, got {:?}", e),
        }
    }

    #[test]
    fn test_match_wildcard_covers_all() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![
                Item::Enum(EnumDef {
                    name: "Color".to_string(), is_public: false,
                    variants: vec![
                        EnumVariant { name: "Red".to_string(), fields: vec![] },
                        EnumVariant { name: "Green".to_string(), fields: vec![] },
                        EnumVariant { name: "Blue".to_string(), fields: vec![] },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "c".to_string(), ty: TypeNode::Custom("Color".to_string()) }],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Match {
                                subject: Expression::Identifier("c".to_string()),
                                arms: vec![
                                    MatchArm { pattern: Pattern::Variant("Red".to_string(), vec![]), body: Block { statements: vec![] } },
                                    MatchArm { pattern: Pattern::Wildcard, body: Block { statements: vec![] } },
                                ],
                            },
                        ]}),
                    }],
                }),
            ],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_match_non_enum_no_exhaustiveness() {
        // Match on a non-enum type should not trigger exhaustiveness check
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Match {
                                subject: Expression::Int(42),
                                arms: vec![
                                    MatchArm { pattern: Pattern::Literal(Expression::Int(1)), body: Block { statements: vec![] } },
                                ],
                            },
                        ]}),
                    }],
                }),
            ],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    // ===== Plan 25: Standalone Functions =====
    #[test]
    fn test_fn_param_types_checked() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Function(FunctionDef {
                name: "add".to_string(),
                is_public: false,
                params: vec![
                    FieldDecl { name: "a".to_string(), ty: TypeNode::Int },
                    FieldDecl { name: "b".to_string(), ty: TypeNode::Int },
                ],
                return_ty: Some(TypeNode::Int),
                body: Block { statements: vec![
                    Statement::Return(Some(Expression::BinaryOp {
                        left: Box::new(Expression::Identifier("a".to_string())),
                        operator: BinaryOperator::Add,
                        right: Box::new(Expression::Identifier("b".to_string())),
                    })),
                ]},
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_fn_return_type_validated() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false,
            items: vec![Item::Function(FunctionDef {
                name: "bad".to_string(),
                is_public: false,
                params: vec![],
                return_ty: Some(TypeNode::Int),
                body: Block { statements: vec![
                    // return "hello" but declared -> int = mismatch
                    Statement::Return(Some(Expression::String("hello".to_string()))),
                ]},
            })],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
    }
}
