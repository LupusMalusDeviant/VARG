use varg_ast::ast::*;
use std::collections::{HashSet, HashMap};

/// Plan 46: Convert byte offset in source to 1-based line number
pub fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())].matches('\n').count() + 1
}

/// Wave 14: Check if a block contains any TryPropagate (?) expressions.
/// If so, the enclosing function must return Result<T, String>.
fn block_contains_try_propagate(block: &Block) -> bool {
    for stmt in &block.statements {
        if stmt_contains_try_propagate(stmt) {
            return true;
        }
    }
    false
}

fn stmt_contains_try_propagate(stmt: &Statement) -> bool {
    match stmt {
        Statement::Let { value, .. } => expr_contains_try_propagate(value),
        Statement::Assign { value, .. } => expr_contains_try_propagate(value),
        Statement::IndexAssign { value, target, index } => {
            expr_contains_try_propagate(value) || expr_contains_try_propagate(target) || expr_contains_try_propagate(index)
        }
        Statement::PropertyAssign { value, target, .. } => {
            expr_contains_try_propagate(value) || expr_contains_try_propagate(target)
        }
        Statement::Expr(e) | Statement::Print(e) | Statement::Throw(e) | Statement::Stream(e) => {
            expr_contains_try_propagate(e)
        }
        Statement::Return(Some(e)) | Statement::Const { value: e, .. } | Statement::LetDestructure { value: e, .. } => {
            expr_contains_try_propagate(e)
        }
        Statement::If { condition, then_block, else_block } => {
            expr_contains_try_propagate(condition)
                || block_contains_try_propagate(then_block)
                || else_block.as_ref().map_or(false, |b| block_contains_try_propagate(b))
        }
        Statement::While { condition, body } => {
            expr_contains_try_propagate(condition) || block_contains_try_propagate(body)
        }
        Statement::For { condition, body, .. } => {
            expr_contains_try_propagate(condition) || block_contains_try_propagate(body)
        }
        Statement::Foreach { collection, body, .. } => {
            expr_contains_try_propagate(collection) || block_contains_try_propagate(body)
        }
        Statement::TryCatch { try_block, catch_block, .. } => {
            block_contains_try_propagate(try_block) || block_contains_try_propagate(catch_block)
        }
        Statement::Match { subject, arms } => {
            expr_contains_try_propagate(subject)
                || arms.iter().any(|arm| block_contains_try_propagate(&arm.body))
        }
        Statement::Select { arms } => {
            arms.iter().any(|arm| block_contains_try_propagate(&arm.body))
        }
        Statement::UnsafeBlock(b) => block_contains_try_propagate(b),
        _ => false,
    }
}

fn expr_contains_try_propagate(expr: &Expression) -> bool {
    match expr {
        Expression::TryPropagate(_) => true,
        Expression::BinaryOp { left, right, .. } => {
            expr_contains_try_propagate(left) || expr_contains_try_propagate(right)
        }
        Expression::UnaryOp { operand, .. } => expr_contains_try_propagate(operand),
        Expression::MethodCall { caller, args, .. } => {
            expr_contains_try_propagate(caller) || args.iter().any(expr_contains_try_propagate)
        }
        Expression::IndexAccess { caller, index } => {
            expr_contains_try_propagate(caller) || expr_contains_try_propagate(index)
        }
        Expression::PropertyAccess { caller, .. } => expr_contains_try_propagate(caller),
        Expression::OrDefault { expr, default } => {
            expr_contains_try_propagate(expr) || expr_contains_try_propagate(default)
        }
        Expression::Await(inner) | Expression::Cast { expr: inner, .. } => {
            expr_contains_try_propagate(inner)
        }
        Expression::IfExpr { condition, .. } => expr_contains_try_propagate(condition),
        Expression::Retry { body, fallback, .. } => {
            block_contains_try_propagate(body) || fallback.as_ref().map_or(false, |b| block_contains_try_propagate(b))
        }
        _ => false,
    }
}

pub struct RustGenerator {
    /// Plan 19: Agent field names for self-prefix resolution in methods
    agent_field_names: HashSet<String>,
    /// Plan 16: Known agent definitions for spawn method dispatch
    known_agents: HashMap<String, AgentDef>,
    /// Plan 27: Whether program uses async (for tokio spawn/channels)
    use_async: bool,
    /// Plan 33: Known standalone function names for fn ↔ agent interop
    known_functions: HashSet<String>,
    /// Contract method names for trait impl generation
    known_contract_methods: HashMap<String, Vec<String>>,
    /// Track string-typed variables for correct += codegen
    string_vars: HashSet<String>,
    /// Plan 46: Source map - current varg source line counter
    varg_line_counter: usize,
    /// Plan 46: Enable source map comments
    emit_source_maps: bool,
    /// Wave 13: Current source file name for multi-file source maps
    current_file: String,
    /// Wave 13: Last-use tracking — counts remaining uses of variables in current block
    usage_remaining: HashMap<String, usize>,
    /// Wave 12: Known enum definitions for variant construction codegen
    known_enums: HashMap<String, Vec<EnumVariant>>,
    /// Wave 14: Whether current function/method uses ? and returns Result
    in_result_function: bool,
    /// F41-6: Fields with contract types (struct_name.field_name → true) for Box<dyn> wrapping
    contract_typed_fields: HashSet<String>,
}

impl RustGenerator {
    pub fn new() -> Self {
        Self {
            agent_field_names: HashSet::new(),
            known_agents: HashMap::new(),
            use_async: false,
            known_functions: HashSet::new(),
            known_contract_methods: HashMap::new(),
            string_vars: HashSet::new(),
            varg_line_counter: 0,
            emit_source_maps: false,
            current_file: String::new(),
            usage_remaining: HashMap::new(),
            known_enums: HashMap::new(),
            in_result_function: false,
            contract_typed_fields: HashSet::new(),
        }
    }

    /// Wave 13: Set the current source file for source map comments
    pub fn set_current_file(&mut self, file: &str) {
        self.current_file = file.to_string();
    }

    /// Plan 46: Generate with source map comments enabled
    pub fn generate_with_source_map(&mut self, program: &Program, source: &str) -> String {
        self.emit_source_maps = true;
        // Pre-compute line starts for source mapping
        let _ = source; // Source text available for future refinement
        self.generate(program)
    }

    pub fn generate(&mut self, program: &Program) -> String {
        // Pre-pass to collect definitions
        for item in &program.items {
            if let Item::Agent(a) = item {
                self.known_agents.insert(a.name.clone(), a.clone());
                // Plan 27: Detect async methods
                if a.methods.iter().any(|m| m.is_async) {
                    self.use_async = true;
                }
            }
            // Plan 33: Collect standalone function names
            if let Item::Function(f) = item {
                self.known_functions.insert(f.name.clone());
            }
            // Collect contract method names for trait impl filtering
            if let Item::Contract(c) = item {
                self.known_contract_methods.insert(c.name.clone(), c.methods.iter().map(|m| m.name.clone()).collect());
            }
            // Wave 12: Collect enum definitions for variant construction
            if let Item::Enum(e) = item {
                self.known_enums.insert(e.name.clone(), e.variants.clone());
            }
        }

        // F41-6: Second pass — mark agent/struct fields whose type is a contract
        for item in &program.items {
            let (type_name, fields) = match item {
                Item::Agent(a) => (a.name.clone(), &a.fields),
                Item::Struct(s) => (s.name.clone(), &s.fields),
                _ => continue,
            };
            for field in fields {
                if let TypeNode::Custom(ref name) = field.ty {
                    if self.known_contract_methods.contains_key(name) {
                        self.contract_typed_fields.insert(format!("{}.{}", type_name, field.name));
                    }
                }
            }
        }

        let mut output = String::new();
        output.push_str("// --- AUTOGENERATED BY Varg Compiler ---\n");
        output.push_str("use varg_os_types::*;\n");
        output.push_str("use varg_runtime::*;\n\n");

        for item in &program.items {
            output.push_str(&self.gen_item(item));
            output.push('\n');
        }

        output
    }

    fn gen_item(&mut self, item: &Item) -> String {
        match item {
            Item::Import(_) | Item::ImportDecl(_) => String::new(), // Merged by vargc beforehand
            // Plan 41: External crate import — emit `use crate_name;`
            Item::CrateImport { crate_name, .. } => format!("use {};\n", crate_name),
            // F41-1: Qualified extern path import — emit `use axum::Router;`
            Item::UseExtern { path } => format!("use {};\n", path.join("::")),
            // Plan 23: Prompt template → Rust function returning Prompt
            Item::PromptTemplate(pt) => {
                let params: Vec<String> = pt.params.iter()
                    .map(|p| format!("{}: {}", p.name, self.gen_type(&p.ty)))
                    .collect();

                // Parse {var} placeholders → format!() args
                let mut format_str = String::new();
                let mut format_args: Vec<String> = Vec::new();
                let mut chars = pt.body.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '{' {
                        let mut var_name = String::new();
                        for inner in chars.by_ref() {
                            if inner == '}' { break; }
                            var_name.push(inner);
                        }
                        format_str.push_str("{}");
                        format_args.push(var_name.trim().to_string());
                    } else if c == '"' {
                        format_str.push_str("\\\"");
                    } else {
                        format_str.push(c);
                    }
                }

                let body_expr = if format_args.is_empty() {
                    format!("\"{}\".to_string()", format_str)
                } else {
                    format!("format!(\"{}\", {})", format_str, format_args.join(", "))
                };

                format!("fn {}({}) -> Prompt {{\n    Prompt {{ text: {} }}\n}}\n",
                    pt.name, params.join(", "), body_expr)
            },
            // Plan 25: Standalone top-level functions
            Item::Function(f) => {
                let params: Vec<String> = f.params.iter()
                    .map(|p| format!("{}: {}", p.name, self.gen_type(&p.ty)))
                    .collect();
                // Wave 14: Auto-wrap return type in Result if body uses ?
                let uses_try = block_contains_try_propagate(&f.body);
                let ret = if uses_try {
                    let inner = f.return_ty.as_ref()
                        .map(|t| self.gen_type(t))
                        .unwrap_or_else(|| "()".to_string());
                    format!(" -> Result<{}, String>", inner)
                } else {
                    f.return_ty.as_ref()
                        .map(|t| format!(" -> {}", self.gen_type(t)))
                        .unwrap_or_default()
                };
                // Wave 14: Set flag so return statements get Ok()-wrapped
                let prev = self.in_result_function;
                self.in_result_function = uses_try;
                let mut body = self.gen_block(&f.body, 1);
                self.in_result_function = prev;
                // Wave 14: If uses_try, wrap implicit return with Ok(())
                if uses_try {
                    body.push_str("    Ok(())\n");
                }
                format!("fn {}({}){} {{\n{}}}\n", f.name, params.join(", "), ret, body)
            },
            Item::TypeAlias { name, target } => {
                format!("type {} = {};\n", name, self.gen_type(target))
            },
            Item::Enum(e) => {
                let vis = if e.is_public { "pub " } else { "" };
                let mut out = format!("#[derive(Debug, Clone, PartialEq)]\n{}enum {} {{\n", vis, e.name);
                for variant in &e.variants {
                    if variant.fields.is_empty() {
                        out.push_str(&format!("    {},\n", variant.name));
                    } else {
                        let fields: Vec<String> = variant.fields.iter()
                            .map(|(name, ty)| format!("{}: {}", name, self.gen_type(ty)))
                            .collect();
                        out.push_str(&format!("    {} {{ {} }},\n", variant.name, fields.join(", ")));
                    }
                }
                out.push_str("}\n");
                out
            },
            Item::Struct(s) => {
                let vis = if s.is_public { "pub " } else { "" };
                let type_params = if s.type_params.is_empty() { "".to_string() } else { format!("<{}>", s.type_params.join(", ")) };
                let mut out = format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n{}struct {}{} {{\n", vis, s.name, type_params);
                for field in &s.fields {
                    // MVP: all generated fields are pub to the struct
                    out.push_str(&format!("    pub {}: {},\n", field.name, self.gen_type(&field.ty)));
                }
                out.push_str("}\n");
                out
            },
            Item::Contract(c) => {
                let vis = if c.is_public { "pub " } else { "" };
                let mut out = format!("{}trait {} {{\n", vis, c.name);
                for method in &c.methods {
                    if let Some(ref body) = method.body {
                        // Wave 13: Contract default implementation
                        out.push_str(&format!("    {} {{\n", self.gen_method_signature(method, true)));
                        let body_code = self.gen_block(body, 2);
                        out.push_str(&body_code);
                        out.push_str("    }\n");
                    } else {
                        out.push_str(&format!("    {};\n", self.gen_method_signature(method, true)));
                    }
                }
                out.push_str("}\n");
                out
            },
            Item::Agent(a) => {
                // An Agent translates to a struct with state, and an impl block
                // Plan 19: Track agent field names for self-prefix resolution
                self.agent_field_names = a.fields.iter().map(|f| f.name.clone()).collect();
                // Track string-typed fields for correct += codegen
                for field in &a.fields {
                    if matches!(field.ty, TypeNode::String) {
                        self.string_vars.insert(field.name.clone());
                    }
                }
                let vis = if a.is_public { "pub " } else { "" };
                let mut out = String::new();
                if a.fields.is_empty() {
                    out.push_str(&format!("{}struct {} {{}}\n\n", vis, a.name));
                } else {
                    out.push_str(&format!("{}struct {} {{\n", vis, a.name));
                    for field in &a.fields {
                        out.push_str(&format!("    pub {}: {},\n", field.name, self.gen_type(&field.ty)));
                    }
                    out.push_str("}\n\n");
                }
                out.push_str(&format!("impl {} {{\n", a.name));

                // Generate new() constructor if agent has fields
                // F41-6: Skip auto-new() for agents with contract-typed fields (must use DI)
                let has_contract_fields = a.fields.iter().any(|f| {
                    if let TypeNode::Custom(ref name) = f.ty {
                        self.known_contract_methods.contains_key(name)
                    } else {
                        false
                    }
                });
                if !a.fields.is_empty() && !has_contract_fields {
                    out.push_str("    pub fn new() -> Self {\n");
                    out.push_str(&format!("        let mut __self = {} {{\n", a.name));
                    for field in &a.fields {
                        let default = self.gen_type_default(&field.ty);
                        out.push_str(&format!("            {}: {},\n", field.name, default));
                    }
                    out.push_str("        };\n");
                    // Call Init() if it exists
                    if a.methods.iter().any(|m| m.name == "Init") {
                        out.push_str("        __self.Init();\n");
                    }
                    out.push_str("        __self\n");
                    out.push_str("    }\n");
                }

                // Agent-level Annotations
                for ann in &a.annotations {
                    if ann.name == "CliCommand" {
                        out.push_str(&format!("    pub fn run_cli(&mut self) {{\n        println!(\"Starting CLI for agent {}...\");\n        // TODO: Map clap args to methods\n    }}\n", a.name));
                    }
                }

                for method in &a.methods {
                    // Method-level Annotations
                    for ann in &method.annotations {
                        if ann.name == "McpTool" {
                            let desc = ann.values.join(" ");
                            out.push_str(&format!("    pub fn {}_mcp_schema() -> String {{\n        r#\"{{ \"name\": \"{}\", \"description\": \"{}\" }}\"#.to_string()\n    }}\n",
                                method.name, method.name, desc));
                        }
                    }

                    // Wave 14: Auto-detect if method uses ? and needs Result wrapping
                    let uses_try = method.body.as_ref().map_or(false, |b| block_contains_try_propagate(b));
                    if uses_try {
                        out.push_str(&format!("    {} {{\n", self.gen_method_signature_result_wrapped(method, false)));
                    } else {
                        out.push_str(&format!("    {} {{\n", self.gen_method_signature(method, false)));
                    }
                    let prev = self.in_result_function;
                    self.in_result_function = uses_try;
                    if let Some(body) = &method.body {
                        out.push_str(&self.gen_block(body, 2));
                    }
                    self.in_result_function = prev;
                    if uses_try {
                        out.push_str("        Ok(())\n");
                    }
                    out.push_str("    }\n");
                }
                out.push_str("}\n");

                // Generate Drop impl if agent has a Destroy() method
                if a.methods.iter().any(|m| m.name == "Destroy") {
                    out.push_str(&format!("\nimpl Drop for {} {{\n", a.name));
                    out.push_str("    fn drop(&mut self) {\n");
                    out.push_str("        self.Destroy();\n");
                    out.push_str("    }\n");
                    out.push_str("}\n");
                }

                // Plan 29: Generate trait impls for implemented contracts
                for contract_name in &a.implements {
                    out.push_str(&format!("\nimpl {} for {} {{\n", contract_name, a.name));
                    // Only include methods that the contract actually declares
                    let contract_methods = self.known_contract_methods.get(contract_name).cloned().unwrap_or_default();
                    for method in &a.methods {
                        if contract_methods.contains(&method.name) {
                            out.push_str(&format!("    {} {{\n", self.gen_method_signature(method, true)));
                            if let Some(body) = &method.body {
                                out.push_str(&self.gen_block(body, 2));
                            }
                            out.push_str("    }\n");
                        }
                    }
                    out.push_str("}\n");
                }

                self.agent_field_names.clear();
                out
            }
            // Wave 13: impl blocks for structs
            Item::Impl { type_name, type_params, methods } => {
                let tp = if type_params.is_empty() { "".to_string() } else { format!("<{}>", type_params.join(", ")) };
                let mut out = format!("impl{} {} {{\n", tp, type_name);
                for method in methods {
                    out.push_str(&format!("    {} {{\n", self.gen_method_signature(method, false)));
                    if let Some(body) = &method.body {
                        out.push_str(&self.gen_block(body, 2));
                    }
                    out.push_str("    }\n");
                }
                out.push_str("}\n");
                out
            }
        }
    }

    fn gen_method_signature(&self, method: &MethodDecl, force_no_vis: bool) -> String {
        let vis = if method.is_public && !force_no_vis { "pub " } else { "" };
        let args: Vec<String> = method.args.iter()
            .map(|a| format!("{}: {}", a.name, self.gen_type(&a.ty)))
            .collect();
        let arg_str = if args.is_empty() { "&mut self".to_string() } else { format!("&mut self, {}", args.join(", ")) };
        
        let ret_str = match &method.return_ty {
            Some(TypeNode::Void) | None => "".to_string(),
            Some(ty) => format!(" -> {}", self.gen_type(ty)),
        };

        // Plan 39: Emit inline trait bounds <T: Display + Clone> instead of where clause
        let type_params = if method.type_params.is_empty() {
            "".to_string()
        } else {
            let params: Vec<String> = method.type_params.iter().map(|tp| {
                // Find constraints for this type param
                let bounds: Vec<&String> = method.constraints.iter()
                    .filter(|c| &c.type_param == tp)
                    .flat_map(|c| c.bounds.iter())
                    .collect();
                if bounds.is_empty() {
                    tp.clone()
                } else {
                    format!("{}: {}", tp, bounds.iter().map(|b| b.as_str()).collect::<Vec<_>>().join(" + "))
                }
            }).collect();
            format!("<{}>", params.join(", "))
        };
        let async_kw = if method.is_async { "async " } else { "" };
        format!("{}{}fn {}{}({}){}", vis, async_kw, method.name, type_params, arg_str, ret_str)
    }

    /// Wave 14: Generate method signature with Result-wrapped return type
    fn gen_method_signature_result_wrapped(&self, method: &MethodDecl, force_no_vis: bool) -> String {
        let vis = if method.is_public && !force_no_vis { "pub " } else { "" };
        let args: Vec<String> = method.args.iter()
            .map(|a| format!("{}: {}", a.name, self.gen_type(&a.ty)))
            .collect();
        let arg_str = if args.is_empty() { "&mut self".to_string() } else { format!("&mut self, {}", args.join(", ")) };

        let ret_str = match &method.return_ty {
            Some(TypeNode::Void) | None => " -> Result<(), String>".to_string(),
            Some(ty) => format!(" -> Result<{}, String>", self.gen_type(ty)),
        };

        let type_params = if method.type_params.is_empty() {
            "".to_string()
        } else {
            let params: Vec<String> = method.type_params.iter().map(|tp| {
                let bounds: Vec<&String> = method.constraints.iter()
                    .filter(|c| &c.type_param == tp)
                    .flat_map(|c| c.bounds.iter())
                    .collect();
                if bounds.is_empty() {
                    tp.clone()
                } else {
                    format!("{}: {}", tp, bounds.iter().map(|b| b.as_str()).collect::<Vec<_>>().join(" + "))
                }
            }).collect();
            format!("<{}>", params.join(", "))
        };
        let async_kw = if method.is_async { "async " } else { "" };
        format!("{}{}fn {}{}({}){}", vis, async_kw, method.name, type_params, arg_str, ret_str)
    }

    /// Heuristic: does this expression produce a String?
    fn is_string_expr(&self, expr: &Expression) -> bool {
        matches!(expr, Expression::String(_) | Expression::PromptLiteral(_) | Expression::InterpolatedString(_))
            || matches!(expr, Expression::MethodCall { method_name, .. }
                if ["to_upper", "to_lower", "trim", "replace", "substring", "char_at", "join"].contains(&method_name.as_str()))
            || matches!(expr, Expression::Identifier(name) if {
                // Heuristic: if the variable name suggests string type
                // This is imperfect but covers common patterns
                false // Can't determine type at codegen level without type info
            })
    }

    fn gen_type(&self, ty: &TypeNode) -> String {
        match ty {
            TypeNode::Int => "i64".to_string(),
            TypeNode::Float => "f64".to_string(),  // Plan 42
            TypeNode::Ulong => "u64".to_string(),
            TypeNode::String => "String".to_string(),
            TypeNode::Bool => "bool".to_string(),
            TypeNode::Void => "()".to_string(),
            
            // Native AI Types map directly to Rust OS types
            TypeNode::Prompt => "Prompt".to_string(),
            TypeNode::Context => "Context".to_string(),
            TypeNode::Tensor => "Tensor".to_string(),
            TypeNode::Embedding => "Embedding".to_string(),
            
            TypeNode::Nullable(inner) => format!("Option<{}>", self.gen_type(inner)),
            TypeNode::Result(ok, err) => format!("std::result::Result<{}, {}>", self.gen_type(ok), self.gen_type(err)),
            TypeNode::Error => "String".to_string(),
            TypeNode::Array(inner) => format!("Vec<{}>", self.gen_type(inner)),
            TypeNode::List(inner) => format!("Vec<{}>", self.gen_type(inner)),
            TypeNode::Map(k, v) => format!("std::collections::HashMap<{}, {}>", self.gen_type(k), self.gen_type(v)),
            TypeNode::Set(inner) => format!("std::collections::HashSet<{}>", self.gen_type(inner)),
            TypeNode::TypeVar(name) => name.clone(),
            TypeNode::Generic(name, args) => {
                let arg_strs: Vec<String> = args.iter().map(|a| self.gen_type(a)).collect();
                format!("{}<{}>", name, arg_strs.join(", "))
            },
            TypeNode::Capability(cap) => {
                match cap {
                    CapabilityType::NetworkAccess => "NetworkAccess".to_string(),
                    CapabilityType::FileAccess => "FileAccess".to_string(),
                    CapabilityType::DbAccess => "DbAccess".to_string(),
                    CapabilityType::LlmAccess => "LlmAccess".to_string(),
                    CapabilityType::SystemAccess => "SystemAccess".to_string(),
                }
            },
            TypeNode::Func(params, ret) => {
                let param_strs: Vec<String> = params.iter().map(|p| self.gen_type(p)).collect();
                format!("Box<dyn Fn({}) -> {}>", param_strs.join(", "), self.gen_type(ret))
            },
            // Plan 16/27: AgentHandle is a channel sender for messaging
            TypeNode::AgentHandle(_) => {
                if self.use_async {
                    "tokio::sync::mpsc::UnboundedSender<(String, Vec<String>, Option<tokio::sync::oneshot::Sender<String>>)>".to_string()
                } else {
                    "std::sync::mpsc::Sender<(String, Vec<String>, Option<std::sync::mpsc::Sender<String>>)>".to_string()
                }
            },
            // Plan 38: Tuple type
            TypeNode::Tuple(types) => {
                let parts: Vec<String> = types.iter().map(|t| self.gen_type(t)).collect();
                format!("({})", parts.join(", "))
            },
            // Wave 15: JsonValue maps to serde_json::Value
            TypeNode::JsonValue => "serde_json::Value".to_string(),
            TypeNode::Custom(name) => {
                if name == "Dynamic" {
                    "String".to_string() // MVP Fallback for empty []
                } else if name == "Iterator" {
                    "Vec<String>".to_string() // MVP for LINQ eval
                } else if self.known_contract_methods.contains_key(name) {
                    // F41-6: Contract types as fields → Box<dyn Trait> for dynamic dispatch
                    format!("Box<dyn {}>", name)
                } else {
                    name.clone()
                }
            },
        }
    }

    /// Returns a Rust default value for a given Varg type
    fn gen_type_default(&self, ty: &TypeNode) -> String {
        match ty {
            TypeNode::Int => "0".to_string(),
            TypeNode::Float => "0.0_f64".to_string(),  // Plan 42
            TypeNode::Ulong => "0u64".to_string(),
            TypeNode::String => "String::new()".to_string(),
            TypeNode::Bool => "false".to_string(),
            TypeNode::Void => "()".to_string(),
            TypeNode::Array(inner) => format!("Vec::<{}>::new()", self.gen_type(inner)),
            TypeNode::List(inner) => format!("Vec::<{}>::new()", self.gen_type(inner)),
            TypeNode::Map(k, v) => format!("std::collections::HashMap::<{}, {}>::new()", self.gen_type(k), self.gen_type(v)),
            TypeNode::Set(inner) => format!("std::collections::HashSet::<{}>::new()", self.gen_type(inner)),
            TypeNode::Nullable(_) => "None".to_string(),
            TypeNode::Context => "Context::new(\"default\")".to_string(),
            TypeNode::Prompt => "Prompt { text: String::new() }".to_string(),
            _ => format!("{} {{}}", self.gen_type(ty)), // struct-like default
        }
    }

    /// Wave 13: Count variable usages in a block for last-use optimization
    fn count_usages_in_block(&self, block: &Block) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for stmt in &block.statements {
            self.count_usages_in_stmt(stmt, &mut counts);
        }
        counts
    }

    fn count_usages_in_stmt(&self, stmt: &Statement, counts: &mut HashMap<String, usize>) {
        match stmt {
            Statement::Let { value, .. } => self.count_usages_in_expr(value, counts),
            Statement::Assign { value, .. } => self.count_usages_in_expr(value, counts),
            Statement::Expr(e) | Statement::Print(e) | Statement::Return(Some(e)) | Statement::Throw(e) | Statement::Stream(e) => self.count_usages_in_expr(e, counts),
            Statement::If { condition, then_block, else_block } => {
                self.count_usages_in_expr(condition, counts);
                for s in &then_block.statements { self.count_usages_in_stmt(s, counts); }
                if let Some(eb) = else_block { for s in &eb.statements { self.count_usages_in_stmt(s, counts); } }
            }
            Statement::While { condition, body } => {
                self.count_usages_in_expr(condition, counts);
                for s in &body.statements { self.count_usages_in_stmt(s, counts); }
            }
            Statement::Foreach { collection, body, .. } => {
                self.count_usages_in_expr(collection, counts);
                for s in &body.statements { self.count_usages_in_stmt(s, counts); }
            }
            Statement::IndexAssign { target, index, value } => {
                self.count_usages_in_expr(target, counts);
                self.count_usages_in_expr(index, counts);
                self.count_usages_in_expr(value, counts);
            }
            Statement::PropertyAssign { target, value, .. } => {
                self.count_usages_in_expr(target, counts);
                self.count_usages_in_expr(value, counts);
            }
            _ => {}
        }
    }

    fn count_usages_in_expr(&self, expr: &Expression, counts: &mut HashMap<String, usize>) {
        match expr {
            Expression::Identifier(name) => { *counts.entry(name.clone()).or_insert(0) += 1; }
            Expression::BinaryOp { left, right, .. } => { self.count_usages_in_expr(left, counts); self.count_usages_in_expr(right, counts); }
            Expression::MethodCall { caller, args, .. } => { self.count_usages_in_expr(caller, counts); for a in args { self.count_usages_in_expr(a, counts); } }
            Expression::PropertyAccess { caller, .. } => self.count_usages_in_expr(caller, counts),
            Expression::IndexAccess { caller, index } => { self.count_usages_in_expr(caller, counts); self.count_usages_in_expr(index, counts); }
            Expression::ArrayLiteral(elems) | Expression::TupleLiteral(elems) => { for e in elems { self.count_usages_in_expr(e, counts); } }
            Expression::UnaryOp { operand, .. } => self.count_usages_in_expr(operand, counts),
            Expression::Await(e) | Expression::TryPropagate(e) => self.count_usages_in_expr(e, counts),
            _ => {}
        }
    }

    /// Wave 13: Check if this is the last use of a variable (for move vs clone optimization)
    fn is_last_use(&mut self, name: &str) -> bool {
        if let Some(count) = self.usage_remaining.get_mut(name) {
            *count = count.saturating_sub(1);
            *count == 0
        } else {
            false
        }
    }

    /// Plan 22: Defensive cloning — clone identifiers used as method arguments
    /// to prevent Rust move-errors. Copy types (i64, bool) are no-ops.
    /// Wave 13: Skip clone on last use (move instead).
    fn gen_cloned_arg(&mut self, expr: &Expression) -> String {
        match expr {
            Expression::Identifier(name) => {
                let is_self_field = self.agent_field_names.contains(name);
                let base = if is_self_field {
                    format!("self.{}", name)
                } else {
                    name.clone()
                };
                // Self fields always need clone (can't move out of &mut self)
                // Local variables can be moved on last use
                if !is_self_field && self.is_last_use(name) {
                    base
                } else {
                    format!("{}.clone()", base)
                }
            },
            _ => self.gen_expression(expr),
        }
    }

    /// Plan 53: Clone self.field expressions when used as rvalues (let, return)
    /// to prevent Rust move-out-of-borrow errors. Does NOT clone method calls
    /// on self fields (self.items.push()) since those need &mut self access.
    fn clone_self_field_if_needed(&self, expr_str: &str) -> String {
        if expr_str.starts_with("self.") && !expr_str.contains('(') && !expr_str.ends_with(".clone()") {
            format!("{}.clone()", expr_str)
        } else {
            expr_str.to_string()
        }
    }

    fn gen_block(&mut self, block: &Block, indent_level: usize) -> String {
        // Wave 13: Pre-pass to count variable usages for last-use optimization
        let saved_usage = self.usage_remaining.clone();
        let counts = self.count_usages_in_block(block);
        self.usage_remaining = counts;

        let indent = "    ".repeat(indent_level);
        let mut out = String::new();
        for stmt in &block.statements {
            // Plan 46: Emit source map comment
            if self.emit_source_maps {
                self.varg_line_counter += 1;
                let file_prefix = if self.current_file.is_empty() { ".varg".to_string() } else { self.current_file.clone() };
                out.push_str(&format!("{}// {}:{}\n", indent, file_prefix, self.varg_line_counter));
            }
            match stmt {
                Statement::Let { name, ty, value } => {
                    // Track string variables for correct += codegen
                    if matches!(ty, Some(TypeNode::String)) || self.is_string_expr(value) {
                        self.string_vars.insert(name.clone());
                    }
                    let val_str = self.gen_expression(value);
                    let val_str = self.clone_self_field_if_needed(&val_str);
                    out.push_str(&format!("{}let mut {} = {};\n", indent, name, val_str));
                },
                Statement::Assign { name, value } => {
                    // Plan 19: Resolve field name with self. prefix
                    let resolved_name = if self.agent_field_names.contains(name) {
                        format!("self.{}", name)
                    } else {
                        name.clone()
                    };
                    // Optimization: detect `name = name op expr` → compound assignment
                    if let Expression::BinaryOp { left, operator, right } = value {
                        if let Expression::Identifier(ref lhs_name) = **left {
                            if lhs_name == name {
                                // String optimization: name = name + str → name.push_str(str)
                                let is_string_var = self.string_vars.contains(name);
                                if *operator == BinaryOperator::Add && (is_string_var || self.is_string_expr(left) || self.is_string_expr(right)) {
                                    if let Expression::String(ref s) = **right {
                                        out.push_str(&format!("{}{}.push_str({:?});\n", indent, &resolved_name, s));
                                    } else {
                                        out.push_str(&format!("{}{}.push_str(&({}).to_string());\n", indent, &resolved_name, self.gen_expression(right)));
                                    }
                                    continue;
                                }
                                // Numeric compound assignment: name = name + x → name += x
                                let op_str = match operator {
                                    BinaryOperator::Add => Some("+="),
                                    BinaryOperator::Sub => Some("-="),
                                    BinaryOperator::Mul => Some("*="),
                                    BinaryOperator::Div => Some("/="),
                                    BinaryOperator::Mod => Some("%="),
                                    _ => None,
                                };
                                if let Some(op) = op_str {
                                    out.push_str(&format!("{}{} {} {};\n", indent, &resolved_name, op, self.gen_expression(right)));
                                    continue;
                                }
                            }
                        }
                    }
                    out.push_str(&format!("{}{} = {};\n", indent, &resolved_name, self.gen_expression(value)));
                },
                Statement::IndexAssign { target, index, value } => {
                    let idx_str = self.gen_expression(index);
                    if let Expression::String(_) = index {
                        // String literal key → map insert
                        out.push_str(&format!("{}{}.insert({}, {});\n", indent, self.gen_expression(target), idx_str, self.gen_expression(value)));
                    } else {
                        // Int literal or int variable → array index assign
                        out.push_str(&format!("{}{}[{} as usize] = {};\n", indent, self.gen_expression(target), idx_str, self.gen_expression(value)));
                    }
                },
                Statement::PropertyAssign { target, property, value } => {
                    out.push_str(&format!("{}{}.{} = {};\n", indent, self.gen_expression(target), property, self.gen_expression(value)));
                },
                Statement::Return(Some(expr)) => {
                    let ret_expr = self.gen_expression(expr);
                    // Plan 53: Use unified self-field clone helper
                    let ret_str = self.clone_self_field_if_needed(&ret_expr);
                    // Wave 14: Wrap return value in Ok() if in a Result-returning function
                    if self.in_result_function {
                        out.push_str(&format!("{}return Ok({});\n", indent, ret_str));
                    } else {
                        out.push_str(&format!("{}return {};\n", indent, ret_str));
                    }
                },
                Statement::Return(None) => {
                    if self.in_result_function {
                        out.push_str(&format!("{}return Ok(());\n", indent));
                    } else {
                        out.push_str(&format!("{}return;\n", indent));
                    }
                },
                Statement::UnsafeBlock(inner) => {
                    // Hardware access / DB Queries map perfectly to Rust's unsafe block paradigm
                    out.push_str(&format!("{}unsafe {{\n", indent));
                    out.push_str(&self.gen_block(inner, indent_level + 1));
                    out.push_str(&format!("{}}}\n", indent));
                },

                Statement::Const { name, ty, value } => {
                    let val_str = self.gen_expression(value);
                    if let Some(t) = ty {
                        out.push_str(&format!("{}let {}: {} = {};\n", indent, name, self.gen_type(t), val_str));
                    } else {
                        out.push_str(&format!("{}let {} = {};\n", indent, name, val_str));
                    }
                },
                Statement::Break => {
                    out.push_str(&format!("{}break;\n", indent));
                },
                Statement::Continue => {
                    out.push_str(&format!("{}continue;\n", indent));
                },
                Statement::If { condition, then_block, else_block } => {
                    out.push_str(&format!("{}if {} {{\n", indent, self.gen_expression(condition)));
                    out.push_str(&self.gen_block(then_block, indent_level + 1));
                    if let Some(eb) = else_block {
                        // Detect else-if chain: single If statement in else block
                        if eb.statements.len() == 1 {
                            if let Statement::If { condition: elif_cond, then_block: elif_then, else_block: elif_else } = &eb.statements[0] {
                                out.push_str(&format!("{}}} else if {} {{\n", indent, self.gen_expression(elif_cond)));
                                out.push_str(&self.gen_block(elif_then, indent_level + 1));
                                if let Some(final_else) = elif_else {
                                    // Check for further chaining
                                    if final_else.statements.len() == 1 {
                                        if let Statement::If { .. } = &final_else.statements[0] {
                                            // Recurse: generate the rest of the chain via gen_block
                                            // which will hit this same branch
                                            let chain = self.gen_block(final_else, indent_level);
                                            // Strip the leading indent from the generated if
                                            let trimmed = chain.trim_start();
                                            out.push_str(&format!("{}}} else {}", indent, trimmed));
                                            continue;
                                        }
                                    }
                                    out.push_str(&format!("{}}} else {{\n", indent));
                                    out.push_str(&self.gen_block(final_else, indent_level + 1));
                                }
                                out.push_str(&format!("{}}}\n", indent));
                                continue;
                            }
                        }
                        out.push_str(&format!("{}}} else {{\n", indent));
                        out.push_str(&self.gen_block(eb, indent_level + 1));
                    }
                    out.push_str(&format!("{}}}\n", indent));
                },
                Statement::While { condition, body } => {
                    out.push_str(&format!("{}while {} {{\n", indent, self.gen_expression(condition)));
                    out.push_str(&self.gen_block(body, indent_level + 1));
                    out.push_str(&format!("{}}}\n", indent));
                },
                Statement::For { init, condition, update, body } => {
                    out.push_str(&format!("{}{{\n", indent));
                    out.push_str(&self.gen_block(&Block { statements: vec![*init.clone()] }, indent_level + 1));
                    out.push_str(&format!("{}    while {} {{\n", indent, self.gen_expression(condition)));
                    out.push_str(&self.gen_block(body, indent_level + 2));
                    out.push_str(&self.gen_block(&Block { statements: vec![*update.clone()] }, indent_level + 2));
                    out.push_str(&format!("{}    }}\n", indent));
                    out.push_str(&format!("{}}}\n", indent));
                },
                Statement::Foreach { item_name, value_name, collection, body } => {
                    if let Some(val_name) = value_name {
                        // Wave 16: Map iteration — for (k, v) in map
                        out.push_str(&format!("{}for (mut {}, mut {}) in {} {{\n", indent, item_name, val_name, self.gen_expression(collection)));
                    } else {
                        out.push_str(&format!("{}for mut {} in {} {{\n", indent, item_name, self.gen_expression(collection)));
                    }
                    out.push_str(&self.gen_block(body, indent_level + 1));
                    out.push_str(&format!("{}}}\n", indent));
                },
                Statement::Stream(expr) => {
                    if let Expression::MethodCall { method_name, args, .. } = expr {
                        if method_name == "llm_chat" {
                            let ctx = if args.len() > 0 { format!("&mut {}", self.gen_expression(&args[0])) } else { "\"\"".to_string() };
                            let prompt = if args.len() > 1 { self.gen_expression(&args[1]) } else { "\"\"".to_string() };
                            let model = if args.len() > 2 { self.gen_expression(&args[2]) } else { "\"llama3\"".to_string() };
                            out.push_str(&format!("{}__varg_llm_chat_stream({}, &{}, &{});\n", indent, ctx, prompt, model));
                        } else if method_name == "llm_infer" {
                            let prompt = if args.len() > 0 { self.gen_expression(&args[0]) } else { "\"\"".to_string() };
                            let model = if args.len() > 1 { self.gen_expression(&args[1]) } else { "\"llama3\"".to_string() };
                            out.push_str(&format!("{}__varg_llm_infer_stream(&{}, &{});\n", indent, prompt, model));
                        } else {
                            out.push_str(&format!("{}print!(\"{{}}\", {});\n{}use std::io::Write; std::io::stdout().flush().unwrap();\n", indent, self.gen_expression(expr), indent));
                        }
                    } else {
                        out.push_str(&format!("{}print!(\"{{}}\", {});\n{}use std::io::Write; std::io::stdout().flush().unwrap();\n", indent, self.gen_expression(expr), indent));
                    }
                },
                Statement::Print(expr) => {
                    // Use Display ({}) for strings, Debug ({:?}) for other types
                    if self.is_string_expr(expr) {
                        out.push_str(&format!("{}println!(\"{{}}\", {});\n", indent, self.gen_expression(expr)));
                    } else if let Expression::Identifier(_) = expr {
                        // Identifiers could be any type — use Debug for safety
                        out.push_str(&format!("{}println!(\"{{:?}}\", {});\n", indent, self.gen_expression(expr)));
                    } else {
                        out.push_str(&format!("{}println!(\"{{:?}}\", {});\n", indent, self.gen_expression(expr)));
                    }
                },
                Statement::Expr(expr) => {
                    out.push_str(&format!("{}{};\n", indent, self.gen_expression(expr)));
                },
                Statement::TryCatch { try_block, catch_var, catch_block } => {
                    out.push_str(&format!("{}#[allow(unreachable_code, unused_labels)]\n", indent));
                    out.push_str(&format!("{}let _varg_try_res: std::result::Result<(), String> = 'varg_try: {{\n", indent));
                    out.push_str(&self.gen_block(try_block, indent_level + 1));
                    out.push_str(&format!("{}    Ok(())\n", indent));
                    out.push_str(&format!("{}}};\n", indent));
                    out.push_str(&format!("{}if let Err(mut {}) = _varg_try_res {{\n", indent, catch_var));
                    out.push_str(&self.gen_block(catch_block, indent_level + 1));
                    out.push_str(&format!("{}}}\n", indent));
                },
                Statement::Throw(expr) => {
                    out.push_str(&format!("{}break 'varg_try Err(format!(\"{{}}\", {}));\n", indent, self.gen_expression(expr)));
                },
                Statement::LetDestructure { pattern, value } => {
                    let val_str = self.gen_expression(value);
                    match pattern {
                        DestructurePattern::Tuple(names) => {
                            out.push_str(&format!("{}let ({}) = {};\n", indent, names.join(", "), val_str));
                        }
                        DestructurePattern::Struct(fields) => {
                            let field_strs: Vec<String> = fields.iter().map(|(name, alias)| {
                                match alias {
                                    Some(a) => format!("{}: {}", name, a),
                                    None => name.clone(),
                                }
                            }).collect();
                            out.push_str(&format!("{}let {{ {} }} = {};\n", indent, field_strs.join(", "), val_str));
                        }
                    }
                },
                Statement::Match { subject, arms } => {
                    out.push_str(&format!("{}match {} {{\n", indent, self.gen_expression(subject)));
                    for arm in arms {
                        let pattern_str = self.gen_pattern(&arm.pattern);
                        if let Some(guard_expr) = &arm.guard {
                            let guard_str = self.gen_expression(guard_expr);
                            out.push_str(&format!("{}    {} if {} => {{\n", indent, pattern_str, guard_str));
                        } else {
                            out.push_str(&format!("{}    {} => {{\n", indent, pattern_str));
                        }
                        out.push_str(&self.gen_block(&arm.body, indent_level + 2));
                        out.push_str(&format!("{}    }},\n", indent));
                    }
                    out.push_str(&format!("{}}}\n", indent));
                },
                // Plan 20: select { msg from agent => { ... } timeout(ms) => { ... } }
                Statement::Select { arms } => {
                    let has_timeout = arms.iter().any(|a| matches!(a.source, SelectSource::Timeout(_)));
                    if has_timeout {
                        out.push_str(&format!("{}let __select_start = std::time::Instant::now();\n", indent));
                    }
                    out.push_str(&format!("{}loop {{\n", indent));
                    for arm in arms {
                        match &arm.source {
                            SelectSource::Agent(agent_expr) => {
                                let agent_str = self.gen_expression(agent_expr);
                                out.push_str(&format!("{}    if let Ok({}) = {}.try_recv() {{\n", indent, arm.var_name, agent_str));
                                out.push_str(&self.gen_block(&arm.body, indent_level + 2));
                                out.push_str(&format!("{}        break;\n", indent));
                                out.push_str(&format!("{}    }}\n", indent));
                            },
                            SelectSource::Timeout(ms_expr) => {
                                let ms_str = self.gen_expression(ms_expr);
                                out.push_str(&format!("{}    if __select_start.elapsed() >= std::time::Duration::from_millis({} as u64) {{\n", indent, ms_str));
                                out.push_str(&self.gen_block(&arm.body, indent_level + 2));
                                out.push_str(&format!("{}        break;\n", indent));
                                out.push_str(&format!("{}    }}\n", indent));
                            },
                        }
                    }
                    out.push_str(&format!("{}    std::thread::sleep(std::time::Duration::from_millis(1));\n", indent));
                    out.push_str(&format!("{}}}\n", indent));
                },
            }
        }

        // Wave 13: Restore parent block's usage counts
        self.usage_remaining = saved_usage;
        out
    }

    /// Generate a block where the last expression-statement becomes the block's return value
    /// (no trailing semicolon). Used for retry/fallback bodies that must return a value.
    fn gen_block_as_expr(&mut self, block: &Block, indent_level: usize) -> String {
        if block.statements.is_empty() {
            return "()".to_string();
        }
        let indent = "    ".repeat(indent_level);
        let mut out = String::new();
        let last_idx = block.statements.len() - 1;
        for (i, stmt) in block.statements.iter().enumerate() {
            if i == last_idx {
                // Last statement: if it's an Expr, generate without semicolon (return value)
                if let Statement::Expr(expr) = stmt {
                    out.push_str(&format!("{}{}\n", indent, self.gen_expression(expr)));
                } else if let Statement::Return(Some(expr)) = stmt {
                    out.push_str(&format!("{}{}\n", indent, self.gen_expression(expr)));
                } else {
                    // Not an expression — fall back to normal gen
                    out.push_str(&self.gen_block(&Block { statements: vec![stmt.clone()] }, indent_level));
                }
            } else {
                out.push_str(&self.gen_block(&Block { statements: vec![stmt.clone()] }, indent_level));
            }
        }
        out
    }

    fn gen_pattern(&mut self, pattern: &Pattern) -> String {
        match pattern {
            Pattern::Wildcard => "_".to_string(),
            Pattern::Literal(expr) => self.gen_expression(expr),
            Pattern::Variant(name, bindings) => {
                if bindings.is_empty() {
                    name.clone()
                } else {
                    format!("{}({})", name, bindings.join(", "))
                }
            },
        }
    }

    fn gen_expression(&mut self, expr: &Expression) -> String {
        match expr {
            Expression::Null => "None".to_string(),
            Expression::Int(i) => i.to_string(),
            Expression::Float(f) => format!("{}_f64", f),  // Plan 42
            Expression::String(s) => format!("{:?}.to_string()", s),
            // Plan 35: String interpolation → format!()
            Expression::InterpolatedString(parts) => {
                let mut fmt_str = String::new();
                let mut args = Vec::new();
                for part in parts {
                    match part {
                        InterpolationPart::Literal(text) => {
                            // Escape braces for format!
                            fmt_str.push_str(&text.replace('{', "{{").replace('}', "}}"));
                        },
                        InterpolationPart::Expression(expr) => {
                            fmt_str.push_str("{}");
                            args.push(self.gen_expression(expr));
                        },
                    }
                }
                if args.is_empty() {
                    format!("{:?}.to_string()", fmt_str)
                } else {
                    format!("format!({:?}, {})", fmt_str, args.join(", "))
                }
            },
            // Plan 38: Tuple literal
            Expression::TupleLiteral(elements) => {
                let parts: Vec<String> = elements.iter().map(|e| self.gen_expression(e)).collect();
                format!("({})", parts.join(", "))
            },
            // Plan 37: Range expressions
            Expression::Range { start, end, inclusive } => {
                let s = self.gen_expression(start);
                let e = self.gen_expression(end);
                if *inclusive {
                    format!("({}..={})", s, e)
                } else {
                    format!("({}..{})", s, e)
                }
            },
            Expression::PromptLiteral(s) => {
                let mut stripped = String::new();
                let mut args = Vec::new();
                let mut chars = s.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '$' && chars.peek() == Some(&'{') {
                        chars.next(); // consume '{'
                        let mut expr_str = String::new();
                        while let Some(inner_c) = chars.next() {
                            if inner_c == '}' { break; }
                            expr_str.push(inner_c);
                        }
                        stripped.push_str("{}");
                        args.push(expr_str); 
                    } else if c == '{' {
                        stripped.push_str("{{");
                    } else if c == '}' {
                        stripped.push_str("}}");
                    } else {
                        stripped.push(c);
                    }
                }
                
                let args_joined = if args.is_empty() {
                    "".to_string()
                } else {
                    format!(", {}", args.join(", "))
                };
                
                format!("Prompt {{ text: format!({:?}{}) }}", stripped, args_joined)
            },
            Expression::Bool(b) => b.to_string(),
            Expression::Identifier(name) => {
                // Plan 19: Agent fields are accessed via self.
                if self.agent_field_names.contains(name) {
                    format!("self.{}", name)
                } else {
                    name.clone()
                }
            },
            Expression::BinaryOp { left, operator, right } => {
                if let BinaryOperator::CosineSim = operator {
                    return format!("__varg_cosine_sim(&{}, &{})", self.gen_expression(left), self.gen_expression(right));
                }

                // String concatenation: format! for expression-level concat
                if let BinaryOperator::Add = operator {
                    if self.is_string_expr(left) || self.is_string_expr(right) {
                        return format!("format!(\"{{}}{{}}\", {}, {})", self.gen_expression(left), self.gen_expression(right));
                    }
                }

                let op = match operator {
                    BinaryOperator::Add => "+",
                    BinaryOperator::Sub => "-",
                    BinaryOperator::Mul => "*",
                    BinaryOperator::Div => "/",
                    BinaryOperator::Mod => "%",
                    BinaryOperator::Eq => "==",
                    BinaryOperator::NotEq => "!=",
                    BinaryOperator::Lt => "<",
                    BinaryOperator::Gt => ">",
                    BinaryOperator::LtEq => "<=",
                    BinaryOperator::GtEq => ">=",
                    BinaryOperator::And => "&&",
                    BinaryOperator::Or => "||",
                    BinaryOperator::CosineSim => unreachable!(),
                };
                format!("{} {} {}", self.gen_expression(left), op, self.gen_expression(right))
            },
            Expression::Await(inner) => {
                format!("{}.await", self.gen_expression(inner))
            },
            Expression::UnaryOp { operator, operand } => {
                let expr = self.gen_expression(operand);
                match operator {
                    UnaryOperator::Negate => format!("-{}", expr),
                    UnaryOperator::Not => format!("!{}", expr),
                }
            },
            Expression::MethodCall { caller, method_name, args } => {
                let arg_strs: Vec<String> = args.iter().map(|a| self.gen_expression(a)).collect();
                if method_name == "encrypt" {
                    format!("__varg_encrypt(&{}, &{})", arg_strs[0], arg_strs[1])
                } else if method_name == "decrypt" {
                    format!("__varg_decrypt(&{}, &{})", arg_strs[0], arg_strs[1])
                } else if method_name == "fetch" {
                    let url = if arg_strs.len() > 0 { &arg_strs[0] } else { "\"\"" };
                    let met = if arg_strs.len() > 1 { &arg_strs[1] } else { "\"GET\"" };
                    let hdr = if arg_strs.len() > 2 { &arg_strs[2] } else { "std::collections::HashMap::new()" };
                    let bod = if arg_strs.len() > 3 { &arg_strs[3] } else { "\"\"" };
                    format!("__varg_fetch(&{}, &{}, {}, &{})", url, met, hdr, bod)
                // ===== Wave 15: HTTP Response with Status =====
                } else if method_name == "http_request" {
                    let url = if arg_strs.len() > 0 { &arg_strs[0] } else { "\"\"" };
                    let met = if arg_strs.len() > 1 { &arg_strs[1] } else { "\"GET\"" };
                    let hdr = if arg_strs.len() > 2 { &arg_strs[2] } else { "std::collections::HashMap::new()" };
                    let bod = if arg_strs.len() > 3 { &arg_strs[3] } else { "\"\"" };
                    format!("__varg_http_request(&{}, &{}, {}, &{})", url, met, hdr, bod)
                } else if method_name == "llm_infer" {
                    let prompt = if arg_strs.len() > 0 { &arg_strs[0] } else { "\"\"" };
                    let model = if arg_strs.len() > 1 { &arg_strs[1] } else { "\"llama3\"" };
                    format!("__varg_llm_infer(&{}, &{})", prompt, model)
                } else if method_name == "llm_chat" {
                    let ctx = if arg_strs.len() > 0 { format!("&mut {}", arg_strs[0]) } else { "\"\"".to_string() };
                    let prompt = if arg_strs.len() > 1 { &arg_strs[1] } else { "\"\"" };
                    let model = if arg_strs.len() > 2 { &arg_strs[2] } else { "\"llama3\"" };
                    format!("__varg_llm_chat({}, &{}, &{})", ctx, prompt, model)
                } else if method_name == "to_json" {
                    format!("serde_json::to_string(&{}).unwrap_or_else(|e| format!(\"{{}}\", e))", arg_strs[0])
                } else if method_name == "from_json" {
                    // For MVP: parse into a flat String HashMap
                    format!("serde_json::from_str::<std::collections::HashMap<String, String>>(&{}).unwrap_or_default()", arg_strs[0])
                } else if method_name == "__varg_create_tensor" {
                    format!("__varg_create_tensor({})", arg_strs[0])
                } else if method_name == "__varg_create_context" {
                    format!("__varg_create_context(&{})", arg_strs[0])
                } else if method_name == "context_from" {
                    format!("__varg_context_from(&{})", arg_strs[0])
                } else if method_name == "file_read" {
                    format!("std::fs::read_to_string(&{}).unwrap_or_else(|e| format!(\"{{}}\", e))", arg_strs[0])
                } else if method_name == "file_write" {
                    format!("std::fs::write(&{}, &{}).unwrap()", arg_strs[0], arg_strs[1])
                } else if method_name == "time_now" {
                    "(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64)".to_string()
                } else if method_name == "str_replace" {
                    format!("{}.replace(&{}, &{})", arg_strs[0], arg_strs[1], arg_strs[2])
                } else if method_name == "str_trim" {
                    format!("{}.trim().to_string()", arg_strs[0])
                } else if method_name == "str_split" {
                    format!("{}.split(&{}).map(|s| s.to_string()).collect::<Vec<String>>()", arg_strs[0], arg_strs[1])
                // ===== Wave 5: String Methods (caller-as-receiver) =====
                } else if method_name == "len" || method_name == "length" {
                    format!("{}.len() as i64", self.gen_expression(caller))
                } else if method_name == "contains" {
                    format!("{}.contains(&{})", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "starts_with" {
                    format!("{}.starts_with(&{})", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "ends_with" {
                    format!("{}.ends_with(&{})", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "to_upper" {
                    format!("{}.to_uppercase()", self.gen_expression(caller))
                } else if method_name == "to_lower" {
                    format!("{}.to_lowercase()", self.gen_expression(caller))
                } else if method_name == "substring" {
                    format!("{}.chars().skip({} as usize).take({} as usize).collect::<String>()", self.gen_expression(caller), arg_strs[0], arg_strs[1])
                } else if method_name == "char_at" {
                    format!("{}.chars().nth({} as usize).map(|c| c.to_string()).unwrap_or_default()", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "index_of" {
                    format!("{}.find(&{}).map(|i| i as i64).unwrap_or(-1)", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "trim" {
                    format!("{}.trim().to_string()", self.gen_expression(caller))
                } else if method_name == "split" {
                    format!("{}.split(&{}).map(|s| s.to_string()).collect::<Vec<String>>()", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "replace" {
                    format!("{}.replace(&{}, &{})", self.gen_expression(caller), arg_strs[0], arg_strs[1])
                // ===== Wave 5: Collection Methods =====
                } else if method_name == "push" {
                    format!("{}.push({})", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "pop" {
                    format!("{}.pop().unwrap()", self.gen_expression(caller))
                } else if method_name == "reverse" {
                    format!("{}.reverse()", self.gen_expression(caller))
                } else if method_name == "is_empty" {
                    format!("{}.is_empty()", self.gen_expression(caller))
                } else if method_name == "keys" {
                    format!("{}.keys().cloned().collect::<Vec<_>>()", self.gen_expression(caller))
                } else if method_name == "values" {
                    format!("{}.values().cloned().collect::<Vec<_>>()", self.gen_expression(caller))
                } else if method_name == "contains_key" {
                    format!("{}.contains_key(&{})", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "remove" {
                    format!("{}.remove(&{})", self.gen_expression(caller), arg_strs[0])
                // ===== Plan 42: Stdlib Expansion =====
                } else if method_name == "to_string" {
                    format!("{}.to_string()", self.gen_expression(caller))
                } else if method_name == "parse_int" {
                    format!("{}.parse::<i64>().unwrap_or(0)", self.gen_expression(caller))
                } else if method_name == "parse_float" {
                    format!("{}.parse::<f64>().unwrap_or(0.0)", self.gen_expression(caller))
                } else if method_name == "abs" {
                    format!("{}.abs()", self.gen_expression(caller))
                } else if method_name == "sort" {
                    format!("{}.sort()", self.gen_expression(caller))
                } else if method_name == "join" {
                    format!("{}.join(&{})", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "min" {
                    format!("std::cmp::min({}, {})", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "max" {
                    format!("std::cmp::max({}, {})", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "sqrt" {
                    format!("({} as f64).sqrt()", self.gen_expression(caller))
                } else if method_name == "floor" {
                    format!("({}).floor()", self.gen_expression(caller))
                } else if method_name == "ceil" {
                    format!("({}).ceil()", self.gen_expression(caller))
                } else if method_name == "round" {
                    format!("({}).round()", self.gen_expression(caller))
                // ===== Plan 43: Iterator Chains =====
                } else if method_name == "filter" {
                    let lambda = self.gen_expression(&args[0]);
                    let caller_code = self.gen_expression(caller);
                    format!("{}.into_iter().filter({}).collect::<Vec<_>>()", caller_code, lambda)
                } else if method_name == "map" {
                    let lambda = self.gen_expression(&args[0]);
                    let caller_code = self.gen_expression(caller);
                    format!("{}.into_iter().map({}).collect::<Vec<_>>()", caller_code, lambda)
                } else if method_name == "any" {
                    let lambda = self.gen_expression(&args[0]);
                    let caller_code = self.gen_expression(caller);
                    format!("{}.into_iter().any({})", caller_code, lambda)
                } else if method_name == "all" {
                    let lambda = self.gen_expression(&args[0]);
                    let caller_code = self.gen_expression(caller);
                    format!("{}.into_iter().all({})", caller_code, lambda)
                } else if method_name == "count" {
                    format!("{}.len()", self.gen_expression(caller))
                } else if method_name == "first" {
                    format!("{}.first().cloned()", self.gen_expression(caller))
                } else if method_name == "last" {
                    format!("{}.last().cloned()", self.gen_expression(caller))
                } else if method_name == "flat_map" {
                    let lambda = self.gen_expression(&args[0]);
                    let caller_code = self.gen_expression(caller);
                    format!("{}.into_iter().flat_map({}).collect::<Vec<_>>()", caller_code, lambda)
                } else if method_name == "find" {
                    let lambda = self.gen_expression(&args[0]);
                    let caller_code = self.gen_expression(caller);
                    format!("{}.into_iter().find({})", caller_code, lambda)
                // ===== Plan 52: Environment Variables =====
                } else if method_name == "env" {
                    format!("std::env::var({}).unwrap_or_default()", arg_strs[0])
                // ===== Wave 13/14: Stdlib Expansion — fs (Result-based) =====
                } else if method_name == "fs_read" {
                    format!("std::fs::read_to_string({}).map_err(|e| e.to_string())", arg_strs[0])
                } else if method_name == "fs_write" {
                    format!("std::fs::write({}, {}).map_err(|e| e.to_string())", arg_strs[0], arg_strs[1])
                } else if method_name == "fs_read_dir" {
                    format!("std::fs::read_dir({}).map_err(|e| e.to_string()).map(|entries| entries.filter_map(|e| e.ok()).map(|e| e.path().to_string_lossy().to_string()).collect::<Vec<String>>())", arg_strs[0])
                } else if method_name == "create_dir" {
                    format!("std::fs::create_dir_all({}).map_err(|e| e.to_string())", arg_strs[0])
                } else if method_name == "delete_file" {
                    format!("std::fs::remove_file({}).map_err(|e| e.to_string())", arg_strs[0])
                // ===== Wave 15: fs_append + fs_read_lines =====
                } else if method_name == "fs_append" {
                    format!("std::fs::OpenOptions::new().append(true).create(true).open({}).and_then(|mut f| std::io::Write::write_all(&mut f, {}.as_bytes())).map_err(|e| e.to_string())", arg_strs[0], arg_strs[1])
                } else if method_name == "fs_read_lines" {
                    format!("std::fs::read_to_string({}).map(|s| s.lines().map(|l| l.to_string()).collect::<Vec<String>>()).map_err(|e| e.to_string())", arg_strs[0])
                // ===== Wave 15: Shell Command Execution =====
                } else if method_name == "exec" {
                    format!("std::process::Command::new(if cfg!(target_os = \"windows\") {{ \"cmd\" }} else {{ \"sh\" }}).args(if cfg!(target_os = \"windows\") {{ vec![\"/C\", &{}] }} else {{ vec![\"-c\", &{}] }}).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).map_err(|e| e.to_string())", arg_strs[0], arg_strs[0])
                // ===== Wave 15: Typed JSON =====
                } else if method_name == "json_parse" {
                    format!("serde_json::from_str::<serde_json::Value>(&{}).map_err(|e| e.to_string())", arg_strs[0])
                } else if method_name == "json_get" {
                    format!("{}.pointer(&{}).and_then(|v| v.as_str()).unwrap_or_default().to_string()", arg_strs[0], arg_strs[1])
                } else if method_name == "json_get_int" {
                    format!("{}.pointer(&{}).and_then(|v| v.as_i64()).unwrap_or(0)", arg_strs[0], arg_strs[1])
                } else if method_name == "json_get_bool" {
                    format!("{}.pointer(&{}).and_then(|v| v.as_bool()).unwrap_or(false)", arg_strs[0], arg_strs[1])
                } else if method_name == "json_get_array" {
                    format!("{}.pointer(&{}).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<String>>()).unwrap_or_default()", arg_strs[0], arg_strs[1])
                } else if method_name == "json_stringify" {
                    format!("serde_json::to_string(&{}).unwrap_or_default()", arg_strs[0])
                // ===== Wave 15: Test Framework — assert builtins =====
                } else if method_name == "assert" {
                    format!("if !({}) {{ panic!(\"Assertion failed: {{}}\", {}); }}", arg_strs[0], arg_strs[1])
                } else if method_name == "assert_eq" {
                    format!("if ({}) != ({}) {{ panic!(\"assert_eq failed: expected {{:?}}, got {{:?}} — {{}}\", {}, {}, {}); }}", arg_strs[0], arg_strs[1], arg_strs[1], arg_strs[0], arg_strs[2])
                // ===== F41-7: Extended Assertions =====
                } else if method_name == "assert_ne" {
                    format!("if ({}) == ({}) {{ panic!(\"assert_ne failed: both were {{:?}} — {{}}\", {}, {}); }}", arg_strs[0], arg_strs[1], arg_strs[0], arg_strs[2])
                } else if method_name == "assert_true" {
                    format!("if !({}) {{ panic!(\"assert_true failed — {{}}\", {}); }}", arg_strs[0], arg_strs[1])
                } else if method_name == "assert_false" {
                    format!("if ({}) {{ panic!(\"assert_false failed — {{}}\", {}); }}", arg_strs[0], arg_strs[1])
                } else if method_name == "assert_contains" {
                    format!("if !format!(\"{{:?}}\", {}).contains(&format!(\"{{}}\", {})) {{ panic!(\"assert_contains failed: {{:?}} does not contain {{:?}} — {{}}\", {}, {}, {}); }}", arg_strs[0], arg_strs[1], arg_strs[0], arg_strs[1], arg_strs[2])
                } else if method_name == "assert_throws" {
                    format!("{{ let __result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {{ {} }})); if __result.is_ok() {{ panic!(\"assert_throws: expected panic but none occurred — {{}}\", {}); }} }}", arg_strs[0], arg_strs[1])
                // ===== Wave 16: set_of() constructor =====
                } else if method_name == "set_of" {
                    format!("vec![{}].into_iter().collect::<std::collections::HashSet<_>>()", arg_strs.join(", "))
                } else if method_name == "add" {
                    // HashSet.add(x) → .insert(x) in Rust
                    format!("{}.insert({})", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "exec_status" {
                    format!("std::process::Command::new(if cfg!(target_os = \"windows\") {{ \"cmd\" }} else {{ \"sh\" }}).args(if cfg!(target_os = \"windows\") {{ vec![\"/C\", &{}] }} else {{ vec![\"-c\", &{}] }}).status().map(|s| s.code().unwrap_or(-1) as i64).map_err(|e| e.to_string())", arg_strs[0], arg_strs[0])
                // ===== Wave 13: Stdlib Expansion — path =====
                } else if method_name == "path_exists" {
                    format!("std::path::Path::new(&{}).exists()", arg_strs[0])
                } else if method_name == "path_join" {
                    format!("std::path::Path::new(&{}).join(&{}).to_string_lossy().to_string()", arg_strs[0], arg_strs[1])
                } else if method_name == "path_parent" {
                    format!("std::path::Path::new(&{}).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()", arg_strs[0])
                } else if method_name == "path_extension" {
                    format!("std::path::Path::new(&{}).extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default()", arg_strs[0])
                } else if method_name == "path_stem" {
                    format!("std::path::Path::new(&{}).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()", arg_strs[0])
                // ===== Wave 13/14: Stdlib Expansion — regex (Result-based) =====
                } else if method_name == "regex_match" {
                    format!("regex::Regex::new(&{}).map(|r| r.is_match(&{})).map_err(|e| e.to_string())", arg_strs[0], arg_strs[1])
                } else if method_name == "regex_find_all" {
                    format!("regex::Regex::new(&{}).map(|r| r.find_iter(&{}).map(|m| m.as_str().to_string()).collect::<Vec<String>>()).map_err(|e| e.to_string())", arg_strs[0], arg_strs[1])
                } else if method_name == "regex_replace" {
                    format!("regex::Regex::new(&{}).map(|r| r.replace_all(&{}, {}).to_string()).map_err(|e| e.to_string())", arg_strs[0], arg_strs[1], arg_strs[2])
                // ===== Wave 13: Stdlib Expansion — time =====
                } else if method_name == "sleep" {
                    format!("std::thread::sleep(std::time::Duration::from_millis({} as u64))", arg_strs[0])
                } else if method_name == "timestamp" {
                    "chrono::Local::now().to_rfc3339()".to_string()
                // ===== Wave 16: Date/Time Builtins =====
                } else if method_name == "time_millis" {
                    "(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64)".to_string()
                } else if method_name == "time_format" {
                    format!("chrono::DateTime::from_timestamp_millis({}).map(|dt| dt.format(&{}).to_string()).unwrap_or_default()", arg_strs[0], arg_strs[1])
                } else if method_name == "time_parse" {
                    format!("chrono::NaiveDateTime::parse_from_str(&{}, &{}).map(|dt| dt.and_utc().timestamp_millis()).map_err(|e| e.to_string())", arg_strs[0], arg_strs[1])
                } else if method_name == "time_add" {
                    format!("({} + {})", arg_strs[0], arg_strs[1])
                } else if method_name == "time_diff" {
                    format!("({} - {})", arg_strs[0], arg_strs[1])
                // ===== Wave 16: Logging =====
                } else if method_name == "log_debug" {
                    format!("println!(\"[DEBUG] {{}}\", {})", arg_strs[0])
                } else if method_name == "log_info" {
                    format!("println!(\"[INFO] {{}}\", {})", arg_strs[0])
                } else if method_name == "log_warn" {
                    format!("eprintln!(\"[WARN] {{}}\", {})", arg_strs[0])
                } else if method_name == "log_error" {
                    format!("eprintln!(\"[ERROR] {{}}\", {})", arg_strs[0])
                // ===== Plan 16: Agent Messaging =====
                } else if method_name == "send" {
                    // Fire-and-forget: handle.send("Method", args...)
                    let method_arg = &arg_strs[0];
                    let msg_args: Vec<String> = arg_strs[1..].iter()
                        .map(|a| format!("format!(\"{{}}\", {})", a))
                        .collect();
                    let args_vec = if msg_args.is_empty() { "vec![]".to_string() } else { format!("vec![{}]", msg_args.join(", ")) };
                    format!("{}.send(({}, {}, None)).unwrap()", self.gen_expression(caller), method_arg, args_vec)
                } else if method_name == "request" {
                    // Request/reply: handle.request("Method", args...)
                    let method_arg = &arg_strs[0];
                    let msg_args: Vec<String> = arg_strs[1..].iter()
                        .map(|a| format!("format!(\"{{}}\", {})", a))
                        .collect();
                    let args_vec = if msg_args.is_empty() { "vec![]".to_string() } else { format!("vec![{}]", msg_args.join(", ")) };
                    if self.use_async {
                        format!("{{\n    let (__reply_tx, __reply_rx) = tokio::sync::oneshot::channel();\n    {}.send(({}, {}, Some(__reply_tx))).unwrap();\n    __reply_rx.await.unwrap()\n}}", self.gen_expression(caller), method_arg, args_vec)
                    } else {
                        format!("{{\n    let (__reply_tx, __reply_rx) = std::sync::mpsc::channel();\n    {}.send(({}, {}, Some(__reply_tx))).unwrap();\n    __reply_rx.recv().unwrap()\n}}", self.gen_expression(caller), method_arg, args_vec)
                    }
                // ===== F41-5: Result methods (direct Rust passthrough) =====
                } else if method_name == "map_err" {
                    let lambda = self.gen_expression(&args[0]);
                    format!("{}.map_err({})", self.gen_expression(caller), lambda)
                } else if method_name == "and_then" {
                    let lambda = self.gen_expression(&args[0]);
                    format!("{}.and_then({})", self.gen_expression(caller), lambda)
                } else if method_name == "unwrap" {
                    format!("{}.unwrap()", self.gen_expression(caller))
                } else if method_name == "unwrap_or" {
                    format!("{}.unwrap_or({})", self.gen_expression(caller), arg_strs[0])
                } else if method_name == "is_ok" {
                    format!("{}.is_ok()", self.gen_expression(caller))
                } else if method_name == "is_err" {
                    format!("{}.is_err()", self.gen_expression(caller))
                } else if method_name == "is_some" {
                    format!("{}.is_some()", self.gen_expression(caller))
                } else if method_name == "is_none" {
                    format!("{}.is_none()", self.gen_expression(caller))
                // ===== F41-2: HTTP Server Builtins =====
                } else if method_name == "http_serve" {
                    // http_serve(cap) → VargHttpServer::new()
                    "varg_runtime::server::__varg_http_server()".to_string()
                } else if method_name == "http_route" {
                    // http_route(server, method, path, handler)
                    format!("varg_runtime::server::__varg_http_route(&mut {}, &{}, &{}, {})", arg_strs[0], arg_strs[1], arg_strs[2], arg_strs[3])
                } else if method_name == "http_listen" {
                    // http_listen(cap, server, addr) → async
                    format!("varg_runtime::server::__varg_http_listen({}, &{}).await", arg_strs[0], arg_strs[1])
                // ===== F41-3: Database Driver Builtins =====
                } else if method_name == "db_open" {
                    // db_open(cap, path) → Result<DbConnection, string>
                    format!("varg_runtime::db_sqlite::__varg_db_open(&{})", arg_strs[0])
                } else if method_name == "db_execute" {
                    // db_execute(conn, sql, params) → Result<int, string>
                    format!("varg_runtime::db_sqlite::__varg_db_execute(&{}, &{}, &{})", arg_strs[0], arg_strs[1], arg_strs[2])
                } else if method_name == "db_query" {
                    // db_query(conn, sql, params) → Result<string, string>
                    format!("varg_runtime::db_sqlite::__varg_db_query(&{}, &{}, &{})", arg_strs[0], arg_strs[1], arg_strs[2])
                // ===== F41-4: WebSocket Builtins =====
                } else if method_name == "ws_connect" {
                    format!("varg_runtime::websocket::__varg_ws_connect(&{})", arg_strs[0])
                } else if method_name == "ws_send" {
                    format!("varg_runtime::websocket::__varg_ws_send(&mut {}, &{})", arg_strs[0], arg_strs[1])
                } else if method_name == "ws_receive" {
                    format!("varg_runtime::websocket::__varg_ws_receive(&mut {})", arg_strs[0])
                } else if method_name == "ws_close" {
                    format!("varg_runtime::websocket::__varg_ws_close(&mut {})", arg_strs[0])
                // ===== F41-4: SSE Builtins =====
                } else if method_name == "sse_stream" {
                    format!("varg_runtime::websocket::__varg_sse_stream(&{})", arg_strs[0])
                } else if method_name == "sse_send" {
                    format!("varg_runtime::websocket::__varg_sse_send(&{}, &{}, &{})", arg_strs[0], arg_strs[1], arg_strs[2])
                } else if method_name == "sse_close" {
                    format!("varg_runtime::websocket::__varg_sse_close(&mut {})", arg_strs[0])
                // ===== F41-8: MCP Protocol Builtins =====
                } else if method_name == "mcp_connect" {
                    format!("varg_runtime::mcp::__varg_mcp_connect(&{}, &{})", arg_strs[0], arg_strs[1])
                } else if method_name == "mcp_list_tools" {
                    format!("varg_runtime::mcp::__varg_mcp_list_tools(&mut {})", arg_strs[0])
                } else if method_name == "mcp_call_tool" {
                    format!("varg_runtime::mcp::__varg_mcp_call_tool(&mut {}, &{}, &{})", arg_strs[0], arg_strs[1], arg_strs[2])
                } else if method_name == "mcp_disconnect" {
                    format!("varg_runtime::mcp::__varg_mcp_disconnect(&mut {})", arg_strs[0])
                } else {
                    // Plan 33: If caller is `self` and method is a known standalone function, call directly
                    if matches!(**caller, Expression::Identifier(ref name) if name == "self") && self.known_functions.contains(method_name.as_str()) {
                        let cloned_args: Vec<String> = args.iter().map(|a| self.gen_cloned_arg(a)).collect();
                        format!("{}({})", method_name, cloned_args.join(", "))
                    } else {
                        // Plan 22: Defensive cloning for user-defined method calls
                        let cloned_args: Vec<String> = args.iter().map(|a| self.gen_cloned_arg(a)).collect();
                        format!("{}.{}({})", self.gen_expression(caller), method_name, cloned_args.join(", "))
                    }
                }
            },
            Expression::PropertyAccess { caller, property_name } => {
                format!("{}.{}", self.gen_expression(caller), property_name)
            },
            Expression::IndexAccess { caller, index } => {
                let idx_str = self.gen_expression(index);
                if let Expression::String(_) = **index {
                    // String literal key → map access
                    format!("{}.get(&{}).unwrap().clone()", self.gen_expression(caller), idx_str)
                } else {
                    // Int literal or int variable → array access (no .clone() needed for Copy types)
                    format!("{}[{} as usize]", self.gen_expression(caller), idx_str)
                }
            },
            Expression::ArrayLiteral(elements) => {
                let elems: Vec<String> = elements.iter().map(|e| self.gen_expression(e)).collect();
                format!("vec![{}]", elems.join(", "))
            },
            Expression::MapLiteral(entries) => {
                let pairs: Vec<String> = entries.iter().map(|(k, v)| format!("({}, {})", self.gen_expression(k), self.gen_expression(v))).collect();
                format!("std::collections::HashMap::from([{}])", pairs.join(", "))
            },
            Expression::Linq(q) => {
                // LINQ transpiles into a highly efficient Rust Iterator chain
                let mut rust_query = format!("{}.clone().into_iter()", self.gen_expression(&q.in_collection));
                
                if let Some(where_c) = &q.where_clause {
                    rust_query.push_str(&format!(".filter(|{}| {})", q.from_var, self.gen_expression(where_c)));
                }

                if let Some(orderby_c) = &q.orderby_clause {
                    rust_query = format!("{{ let mut _ltmp = {}.collect::<Vec<_>>(); _ltmp.sort_by_key(|{}| {}); if {} {{ _ltmp.reverse(); }} _ltmp.into_iter() }}", 
                        rust_query, q.from_var, self.gen_expression(orderby_c), q.descending);
                }

                rust_query.push_str(&format!(".map(|{}| {}).collect::<Vec<_>>()", q.from_var, self.gen_expression(&q.select_clause)));
                
                rust_query
            },
            Expression::Lambda { params, return_ty: _, body } => {
                let param_strs: Vec<String> = params.iter()
                    .map(|p| format!("{}: {}", p.name, self.gen_type(&p.ty)))
                    .collect();
                let body_str = match body.as_ref() {
                    LambdaBody::Expression(expr) => self.gen_expression(expr),
                    LambdaBody::Block(block) => {
                        format!("{{\n{}}}", self.gen_block(block, 1))
                    },
                };
                format!("|{}| {}", param_strs.join(", "), body_str)
            },
            Expression::Query(q) => {
                // Native embedded DB call
                format!("__varg_query({:?})", q.raw_query)
            },
            // Wave 6: retry(N) { body } fallback { fallback_body }
            Expression::Retry { max_attempts, body, fallback } => {
                let attempts_str = self.gen_expression(max_attempts);
                let body_expr_str = self.gen_block_as_expr(body, 3);
                let fallback_str = if let Some(fb) = fallback {
                    let fb_expr = self.gen_block_as_expr(fb, 2);
                    format!("{{ {} }}", fb_expr.trim())
                } else {
                    "{ panic!(\"retry: all attempts failed\") }".to_string()
                };
                format!("{{\n    let mut __retry_result = None;\n    for __retry_i in 0..{} {{\n        match (|| -> std::result::Result<_, String> {{\n            Ok({})\n        }})() {{\n            Ok(val) => {{ __retry_result = Some(val); break; }}\n            Err(_) => {{}}\n        }}\n    }}\n    __retry_result.unwrap_or_else(|| {})\n}}", attempts_str, body_expr_str.trim(), fallback_str)
            },
            // Plan 16: spawn Agent(args) — creates worker thread with message dispatch
            Expression::Spawn { agent_name, args: _ } => {
                // Determine agent construction
                let agent_init = if let Some(agent_def) = self.known_agents.get(agent_name) {
                    if agent_def.fields.is_empty() {
                        format!("{} {{}}", agent_name)
                    } else {
                        format!("{}::new()", agent_name)
                    }
                } else {
                    format!("{} {{}}", agent_name)
                };

                // Generate method dispatch match arms from agent's public methods
                let dispatch = if let Some(agent_def) = self.known_agents.get(agent_name).cloned() {
                    let arms: Vec<String> = agent_def.methods.iter()
                        .filter(|m| m.is_public && m.name != "Init" && m.name != "Destroy")
                        .map(|m| {
                            let arg_bindings: Vec<String> = m.args.iter().enumerate()
                                .map(|(i, _)| format!("args[{}].clone()", i))
                                .collect();
                            let call = if arg_bindings.is_empty() {
                                format!("__agent.{}()", m.name)
                            } else {
                                format!("__agent.{}({})", m.name, arg_bindings.join(", "))
                            };
                            let call_with_result = if m.return_ty == Some(TypeNode::Void) || m.return_ty.is_none() {
                                format!("{{ {}; \"ok\".to_string() }}", call)
                            } else {
                                format!("format!(\"{{}}\", {})", call)
                            };
                            format!("                \"{}\" => {}", m.name, call_with_result)
                        })
                        .collect();
                    if arms.is_empty() {
                        "                _ => \"unknown\".to_string()".to_string()
                    } else {
                        format!("{},\n                _ => \"unknown\".to_string()", arms.join(",\n"))
                    }
                } else {
                    "                _ => \"ok\".to_string()".to_string()
                };

                if self.use_async {
                    // Plan 27: tokio async spawn
                    format!("{{\n    let (__tx, mut __rx) = tokio::sync::mpsc::unbounded_channel::<(String, Vec<String>, Option<tokio::sync::oneshot::Sender<String>>)>();\n    tokio::spawn(async move {{\n        let mut __agent = {};\n        while let Some((method, args, reply_tx)) = __rx.recv().await {{\n            let result = match method.as_str() {{\n{}\n            }};\n            if let Some(reply) = reply_tx {{ let _ = reply.send(result); }}\n        }}\n    }});\n    __tx\n}}", agent_init, dispatch)
                } else {
                    format!("{{\n    let (__tx, __rx) = std::sync::mpsc::channel::<(String, Vec<String>, Option<std::sync::mpsc::Sender<String>>)>();\n    std::thread::spawn(move || {{\n        let mut __agent = {};\n        for (method, args, reply_tx) in __rx {{\n            let result = match method.as_str() {{\n{}\n            }};\n            if let Some(reply) = reply_tx {{ let _ = reply.send(result); }}\n        }}\n    }});\n    __tx\n}}", agent_init, dispatch)
                }
            },
            // Plan 24: expr? → try-propagate
            Expression::TryPropagate(expr) => {
                format!("({})?", self.gen_expression(expr))
            },
            // Plan 24: expr or default → unwrap_or_else
            Expression::OrDefault { expr, default } => {
                format!("({}).unwrap_or_else(|_| {})", self.gen_expression(expr), self.gen_expression(default))
            },
            // Wave 11: If-expression — if cond { a } else { b }
            Expression::IfExpr { condition, then_block, else_block } => {
                let cond_str = self.gen_expression(condition);
                let then_str = self.gen_block_as_expr(then_block, 2);
                let else_str = self.gen_block_as_expr(else_block, 2);
                format!("if {} {{\n{}\n    }} else {{\n{}\n    }}", cond_str, then_str.trim_end(), else_str.trim_end())
            },
            // Wave 11: Type casting — expr as Type
            Expression::Cast { expr, target_type } => {
                let expr_str = self.gen_expression(expr);
                match target_type {
                    TypeNode::Int => format!("({} as i64)", expr_str),
                    TypeNode::Float => format!("({} as f64)", expr_str),
                    TypeNode::Ulong => format!("({} as u64)", expr_str),
                    TypeNode::String => format!("format!(\"{{}}\", {})", expr_str),
                    TypeNode::Bool => format!("({} != 0)", expr_str),
                    _ => format!("({} as {})", expr_str, self.gen_type(target_type)),
                }
            },
            // Wave 12: Struct literal — Point { x: 5, y: 10 }
            Expression::StructLiteral { type_name, fields } => {
                let field_strs: Vec<String> = fields.iter()
                    .map(|(name, val)| {
                        let val_code = self.gen_expression(val);
                        // F41-6: Wrap contract-typed fields in Box::new() for dyn dispatch
                        let key = format!("{}.{}", type_name, name);
                        if self.contract_typed_fields.contains(&key) {
                            format!("{}: Box::new({})", name, val_code)
                        } else {
                            format!("{}: {}", name, val_code)
                        }
                    })
                    .collect();
                format!("{} {{ {} }}", type_name, field_strs.join(", "))
            },
            // Wave 12: Enum variant construction — Shape::Circle(5) or Ok(value)
            Expression::EnumConstruct { enum_name, variant_name, args } => {
                // Bare variants: Ok, Err, Some, None
                if enum_name.is_empty() {
                    if args.is_empty() {
                        variant_name.clone()
                    } else if args.len() == 1 {
                        format!("{}({})", variant_name, self.gen_expression(&args[0]))
                    } else {
                        let arg_strs: Vec<String> = args.iter().map(|a| self.gen_expression(a)).collect();
                        format!("{}({})", variant_name, arg_strs.join(", "))
                    }
                } else {
                    // Qualified: Shape::Circle { radius: 5 }
                    if args.is_empty() {
                        format!("{}::{}", enum_name, variant_name)
                    } else {
                        // Clone enum fields to avoid borrow conflict with self
                        let variant_fields: Option<Vec<(String, TypeNode)>> = self.known_enums.get(enum_name)
                            .and_then(|variants| variants.iter().find(|v| v.name == *variant_name))
                            .map(|v| v.fields.clone());

                        if let Some(ref fields) = variant_fields {
                            if !fields.is_empty() && fields.len() == args.len() {
                                // Named fields
                                let field_strs: Vec<String> = fields.iter().zip(args.iter())
                                    .map(|((name, _), val)| format!("{}: {}", name, self.gen_expression(val)))
                                    .collect();
                                format!("{}::{} {{ {} }}", enum_name, variant_name, field_strs.join(", "))
                            } else {
                                let arg_strs: Vec<String> = args.iter().map(|a| self.gen_expression(a)).collect();
                                format!("{}::{}({})", enum_name, variant_name, arg_strs.join(", "))
                            }
                        } else {
                            // Unknown enum: tuple-style fallback
                            let arg_strs: Vec<String> = args.iter().map(|a| self.gen_expression(a)).collect();
                            format!("{}::{}({})", enum_name, variant_name, arg_strs.join(", "))
                        }
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codegen_agent() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "MemoryAgent".to_string(),
                is_system: false,
                is_public: true,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Recall".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "mem".to_string(),
                                ty: Some(TypeNode::String),
                                value: Expression::String("Data".to_string()) },
                            Statement::Return(Some(Expression::Identifier("mem".to_string()))),
                        ]
                    })
                }]
            })]
        };

        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        
        assert!(code.contains("pub struct MemoryAgent"));
        assert!(code.contains("pub fn Recall(&mut self) -> String"));
        assert!(code.contains("let mut mem = \"Data\".to_string();"));
        assert!(code.contains("return mem;"));
    }

    // ---- Plan 08: Extended CodeGen Coverage ----

    #[test]
    fn test_codegen_contract_trait() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Contract(ContractDef {
                name: "Searchable".to_string(),
                is_public: true,
                target_annotation: None,
                methods: vec![
                    MethodDecl {
                        name: "Find".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![],
                        type_params: vec![],
                        constraints: vec![],
                        args: vec![FieldDecl { name: "query".to_string(), ty: TypeNode::String, default_value: None }],
                        return_ty: Some(TypeNode::String),
                        body: None,
                    },
                ],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("pub trait Searchable"));
        assert!(code.contains("fn Find(&mut self, query: String) -> String;"));
    }

    #[test]
    fn test_codegen_contract_with_default_impl() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Contract(ContractDef {
                name: "Logger".to_string(),
                is_public: false,
                target_annotation: None,
                methods: vec![
                    MethodDecl {
                        name: "log".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "msg".to_string(), ty: TypeNode::String, default_value: None }],
                        return_ty: Some(TypeNode::Void),
                        body: None, // abstract
                    },
                    MethodDecl {
                        name: "format".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "msg".to_string(), ty: TypeNode::String, default_value: None }],
                        return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::Identifier("msg".to_string()))),
                        ]}),
                    },
                ],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        // Abstract method should be a signature only
        assert!(code.contains("fn log(&mut self, msg: String);"), "Abstract method should have semicolon: {}", code);
        // Default method should have a body
        assert!(code.contains("fn format(&mut self, msg: String) -> String {"), "Default method should have body: {}", code);
        assert!(code.contains("return msg"), "Default body should contain return: {}", code);
    }

    #[test]
    fn test_codegen_struct() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Struct(StructDef {
                name: "UserProfile".to_string(),
                is_public: true,
                type_params: vec![],
                fields: vec![
                    FieldDecl { name: "name".to_string(), ty: TypeNode::String, default_value: None },
                    FieldDecl { name: "age".to_string(), ty: TypeNode::Int, default_value: None },
                    FieldDecl { name: "active".to_string(), ty: TypeNode::Bool, default_value: None },
                ],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("pub struct UserProfile"));
        assert!(code.contains("pub name: String,"));
        assert!(code.contains("pub age: i64,"));
        assert!(code.contains("pub active: bool,"));
        // Plan 18: Structs get serde derives for JSON serialization
        assert!(code.contains("serde::Serialize"));
        assert!(code.contains("serde::Deserialize"));
    }

    #[test]
    fn test_codegen_if_else() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::If {
                                condition: Expression::Bool(true),
                                then_block: Block {
                                    statements: vec![Statement::Print(Expression::String("yes".to_string()))] },
                                else_block: Some(Block { statements: vec![Statement::Print(Expression::String("no".to_string()))] }),
                            }
                        ]
                    })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("if true {"));
        assert!(code.contains("} else {"));
    }

    #[test]
    fn test_codegen_while_loop() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::While {
                                condition: Expression::Bool(true),
                                body: Block {
                                    statements: vec![Statement::Return(None)] },
                            }
                        ]
                    })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("while true {"));
        assert!(code.contains("return;"));
    }

    #[test]
    fn test_codegen_foreach() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Foreach {
                                item_name: "item".to_string(),
                                value_name: None,
                                collection: Expression::Identifier("items".to_string()),
                                body: Block {
                                    statements: vec![Statement::Print(Expression::Identifier("item".to_string()))] },
                            }
                        ]
                    })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("for mut item in items {"));
    }

    #[test]
    fn test_codegen_try_catch() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::TryCatch {
                                try_block: Block {
                                    statements: vec![Statement::Throw(Expression::String("error".to_string()))] },
                                catch_var: "err".to_string(),
                                catch_block: Block { statements: vec![Statement::Print(Expression::Identifier("err".to_string()))] },
                            }
                        ]
                    })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("'varg_try:"));
        assert!(code.contains("if let Err(mut err)"));
        assert!(code.contains("break 'varg_try Err("));
    }

    #[test]
    fn test_codegen_print() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Print(Expression::String("hello world".to_string()))
                        ] })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        // String literals use Display format
        assert!(code.contains("println!(\"{}\","));
    }

    #[test]
    fn test_codegen_binary_ops() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "sum".to_string(),
                                ty: None,
                                value: Expression::BinaryOp {
                                    left: Box::new(Expression::Int(1)),
                                    operator: BinaryOperator::Add,
                                    right: Box::new(Expression::Int(2)) },
                            },
                            Statement::Let {
                                name: "eq".to_string(),
                                ty: None,
                                value: Expression::BinaryOp {
                                    left: Box::new(Expression::Int(1)),
                                    operator: BinaryOperator::Eq,
                                    right: Box::new(Expression::Int(1)),
                                },
                            },
                        ]
                    })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("1 + 2"));
        assert!(code.contains("1 == 1"));
    }

    #[test]
    fn test_codegen_array_literal() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "arr".to_string(),
                                ty: None,
                                value: Expression::ArrayLiteral(vec![
                                    Expression::Int(1),
                                    Expression::Int(2),
                                    Expression::Int(3),
                                ]) }
                        ]
                    })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("vec![1, 2, 3]"));
    }

    #[test]
    fn test_codegen_unsafe_block() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: true,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::UnsafeBlock(Block {
                                statements: vec![Statement::Return(None)] })
                        ]
                    })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("unsafe {"));
        assert!(code.contains("return;"));
    }

    #[test]
    fn test_codegen_method_with_where_clause() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Sort".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec!["T".to_string()],
                    constraints: vec![GenericConstraint {
                        type_param: "T".to_string(),
                        bounds: vec!["Comparable".to_string()],
                    }],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        // Plan 39: Inline bounds instead of where clause
        assert!(code.contains("fn Sort<T: Comparable>"));
    }

    #[test]
    fn test_codegen_all_type_mappings() {
        // Test that all Varg types map to correct Rust types
        let mut gen = RustGenerator::new();
        assert_eq!(gen.gen_type(&TypeNode::Int), "i64");
        assert_eq!(gen.gen_type(&TypeNode::Ulong), "u64");
        assert_eq!(gen.gen_type(&TypeNode::String), "String");
        assert_eq!(gen.gen_type(&TypeNode::Bool), "bool");
        assert_eq!(gen.gen_type(&TypeNode::Void), "()");
        assert_eq!(gen.gen_type(&TypeNode::Prompt), "Prompt");
        assert_eq!(gen.gen_type(&TypeNode::Context), "Context");
        assert_eq!(gen.gen_type(&TypeNode::Tensor), "Tensor");
        assert_eq!(gen.gen_type(&TypeNode::Embedding), "Embedding");
        assert_eq!(gen.gen_type(&TypeNode::Error), "String");
        assert_eq!(gen.gen_type(&TypeNode::Array(Box::new(TypeNode::Int))), "Vec<i64>");
        assert_eq!(gen.gen_type(&TypeNode::List(Box::new(TypeNode::String))), "Vec<String>");
        assert_eq!(gen.gen_type(&TypeNode::Nullable(Box::new(TypeNode::Bool))), "Option<bool>");
        assert_eq!(gen.gen_type(&TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::Int))), "std::collections::HashMap<String, i64>");
        assert_eq!(gen.gen_type(&TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::Error))), "std::result::Result<String, String>");
        assert_eq!(gen.gen_type(&TypeNode::TypeVar("T".to_string())), "T");
    }

    #[test]
    fn test_codegen_struct_with_generics() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Struct(StructDef {
                name: "Container".to_string(),
                is_public: true,
                type_params: vec!["T".to_string()],
                fields: vec![
                    FieldDecl { name: "value".to_string(), ty: TypeNode::TypeVar("T".to_string()), default_value: None },
                ],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("pub struct Container<T>"));
        assert!(code.contains("pub value: T,"));
    }

    // ---- Plan 07 Tests ----

    #[test]
    fn test_codegen_enum() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Enum(EnumDef {
                name: "Status".to_string(),
                is_public: true,
                variants: vec![
                    EnumVariant { name: "Active".to_string(), fields: vec![] },
                    EnumVariant { name: "Suspended".to_string(), fields: vec![
                        ("reason".to_string(), TypeNode::String),
                    ]},
                ],
            })]
        };

        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);

        assert!(code.contains("pub enum Status"));
        assert!(code.contains("Active,"));
        assert!(code.contains("Suspended { reason: String }"));
    }

    #[test]
    fn test_codegen_type_alias() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::TypeAlias {
                name: "UserId".to_string(),
                target: TypeNode::String,
            }]
        };

        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);

        assert!(code.contains("type UserId = String;"));
    }

    #[test]
    fn test_codegen_nullable_type() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "name".to_string(), ty: TypeNode::Nullable(Box::new(TypeNode::String)), default_value: None }],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Let {
                            name: "x".to_string(),
                            ty: None,
                            value: Expression::Null }
                    ]})
                }]
            })]
        };

        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);

        assert!(code.contains("name: Option<String>"));
        assert!(code.contains("let mut x = None;"));
    }

    // ---- Plan 03: OCAP Capability Codegen Tests ----

    #[test]
    fn test_codegen_capability_type_mapping() {
        let mut gen = RustGenerator::new();
        assert_eq!(gen.gen_type(&TypeNode::Capability(CapabilityType::NetworkAccess)), "NetworkAccess");
        assert_eq!(gen.gen_type(&TypeNode::Capability(CapabilityType::FileAccess)), "FileAccess");
        assert_eq!(gen.gen_type(&TypeNode::Capability(CapabilityType::DbAccess)), "DbAccess");
        assert_eq!(gen.gen_type(&TypeNode::Capability(CapabilityType::LlmAccess)), "LlmAccess");
        assert_eq!(gen.gen_type(&TypeNode::Capability(CapabilityType::SystemAccess)), "SystemAccess");
    }

    #[test]
    fn test_codegen_method_with_capability_param() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "ApiAgent".to_string(),
                is_system: false,
                is_public: true,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Fetch".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![
                        FieldDecl { name: "url".to_string(), ty: TypeNode::String, default_value: None },
                        FieldDecl { name: "net".to_string(), ty: TypeNode::Capability(CapabilityType::NetworkAccess), default_value: None },
                    ],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                        Statement::Return(Some(Expression::String("ok".to_string())))
                    ] })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("url: String, net: NetworkAccess"));
    }

    // ---- Plan 06: Match Codegen Tests ----

    #[test]
    fn test_codegen_match_statement() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let { name: "x".to_string(), ty: None, value: Expression::Int(1) },
                            Statement::Match {
                                subject: Expression::Identifier("x".to_string()),
                                arms: vec![
                                    MatchArm {
                                        pattern: Pattern::Literal(Expression::Int(1)),
                                        guard: None,
                                        body: Block { statements: vec![Statement::Print(Expression::String("one".to_string()))] },
                                    },
                                    MatchArm {
                                        pattern: Pattern::Literal(Expression::Int(2)),
                                        guard: None,
                                        body: Block { statements: vec![Statement::Print(Expression::String("two".to_string()))] },
                                    },
                                    MatchArm {
                                        pattern: Pattern::Wildcard,
                                        guard: None,
                                        body: Block { statements: vec![Statement::Print(Expression::String("other".to_string()))] },
                                    },
                                ],
                            }
                        ]
                    })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("match x {"));
        assert!(code.contains("1 => {"));
        assert!(code.contains("2 => {"));
        assert!(code.contains("_ => {"));
    }

    #[test]
    fn test_codegen_match_variant_pattern() {
        let mut gen = RustGenerator::new();
        assert_eq!(gen.gen_pattern(&Pattern::Wildcard), "_");
        assert_eq!(gen.gen_pattern(&Pattern::Literal(Expression::Int(42))), "42");
        assert_eq!(gen.gen_pattern(&Pattern::Variant("None".to_string(), vec![])), "None");
        assert_eq!(gen.gen_pattern(&Pattern::Variant("Some".to_string(), vec!["val".to_string()])), "Some(val)");
    }

    // ---- Plan 06: Lambda Codegen Tests ----

    #[test]
    fn test_codegen_lambda_expression() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "double".to_string(),
                                ty: None,
                                value: Expression::Lambda {
                                    params: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None }],
                                    return_ty: Some(Box::new(TypeNode::Int)),
                                    body: Box::new(LambdaBody::Expression(
                                        Expression::BinaryOp {
                                            left: Box::new(Expression::Identifier("x".to_string())),
                                            operator: BinaryOperator::Mul,
                                            right: Box::new(Expression::Int(2)),
                                        }
                                    )),
                                },
                            }
                        ]
                    })
                }]
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("|x: i64| x * 2"));
    }

    #[test]
    fn test_codegen_func_type_mapping() {
        let mut gen = RustGenerator::new();
        let func_ty = TypeNode::Func(vec![TypeNode::Int, TypeNode::String], Box::new(TypeNode::Bool));
        assert_eq!(gen.gen_type(&func_ty), "Box<dyn Fn(i64, String) -> bool>");
    }

    // ===== Plan 06: Destructuring CodeGen =====

    #[test]
    fn test_codegen_tuple_destructuring() {
        let program = Program {
            no_std: true, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(), is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::LetDestructure {
                            pattern: DestructurePattern::Tuple(vec!["x".to_string(), "y".to_string()]),
                            value: Expression::Identifier("pair".to_string()) },
                    ]}),
                }],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("let (x, y) = pair;"));
    }

    #[test]
    fn test_codegen_struct_destructuring() {
        let program = Program {
            no_std: true, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(), is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::LetDestructure {
                            pattern: DestructurePattern::Struct(vec![
                                ("name".to_string(), None),
                                ("age".to_string(), Some("a".to_string())),
                            ]),
                            value: Expression::Identifier("person".to_string()) },
                    ]}),
                }],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("let { name, age: a } = person;"));
    }

    // ===== Stabilization: Missing CodeGen Tests =====

    #[test]
    fn test_codegen_for_loop() {
        let program = Program {
            no_std: true, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(), is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::For {
                            init: Box::new(Statement::Let { name: "i".to_string(), ty: None, value: Expression::Int(0) }),
                            condition: Expression::BinaryOp {
                                left: Box::new(Expression::Identifier("i".to_string())),
                                operator: BinaryOperator::Lt,
                                right: Box::new(Expression::Int(10)),
                            },
                            update: Box::new(Statement::Assign {
                                name: "i".to_string(),
                                value: Expression::BinaryOp {
                                    left: Box::new(Expression::Identifier("i".to_string())),
                                    operator: BinaryOperator::Add,
                                    right: Box::new(Expression::Int(1)),
                                },
                            }),
                            body: Block { statements: vec![
                                Statement::Print(Expression::Identifier("i".to_string())),
                            ] },
                        }
                    ]}),
                }],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("let mut i = 0;"));
        assert!(code.contains("while i < 10 {"));
        assert!(code.contains("i += 1;"));
    }

    #[test]
    fn test_codegen_property_access() {
        let mut gen = RustGenerator::new();
        let expr = Expression::PropertyAccess {
            caller: Box::new(Expression::Identifier("obj".to_string())),
            property_name: "name".to_string(),
        };
        assert_eq!(gen.gen_expression(&expr), "obj.name");
    }

    #[test]
    fn test_codegen_index_access_int() {
        let mut gen = RustGenerator::new();
        let expr = Expression::IndexAccess {
            caller: Box::new(Expression::Identifier("arr".to_string())),
            index: Box::new(Expression::Int(0)),
        };
        assert_eq!(gen.gen_expression(&expr), "arr[0 as usize]");
    }

    #[test]
    fn test_codegen_index_access_string() {
        let mut gen = RustGenerator::new();
        let expr = Expression::IndexAccess {
            caller: Box::new(Expression::Identifier("map".to_string())),
            index: Box::new(Expression::Identifier("key".to_string())),
        };
        // Identifier index → int-style array access
        assert_eq!(gen.gen_expression(&expr), "map[key as usize]");
    }

    #[test]
    fn test_codegen_map_literal() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MapLiteral(vec![
            (Expression::String("a".to_string()), Expression::Int(1)),
            (Expression::String("b".to_string()), Expression::Int(2)),
        ]);
        let code = gen.gen_expression(&expr);
        assert!(code.contains("HashMap::from("));
        assert!(code.contains("(\"a\".to_string(), 1)"));
        assert!(code.contains("(\"b\".to_string(), 2)"));
    }

    #[test]
    fn test_codegen_linq_query() {
        let mut gen = RustGenerator::new();
        let expr = Expression::Linq(LinqQuery {
            from_var: "x".to_string(),
            in_collection: Box::new(Expression::Identifier("items".to_string())),
            where_clause: Some(Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Identifier("x".to_string())),
                operator: BinaryOperator::Gt,
                right: Box::new(Expression::Int(5)),
            })),
            orderby_clause: None,
            descending: false,
            select_clause: Box::new(Expression::Identifier("x".to_string())),
        });
        let code = gen.gen_expression(&expr);
        assert!(code.contains("items.clone().into_iter()"));
        assert!(code.contains(".filter(|x| x > 5)"));
        assert!(code.contains(".map(|x| x).collect::<Vec<_>>()"));
    }

    #[test]
    fn test_codegen_linq_with_orderby_desc() {
        let mut gen = RustGenerator::new();
        let expr = Expression::Linq(LinqQuery {
            from_var: "n".to_string(),
            in_collection: Box::new(Expression::Identifier("nums".to_string())),
            where_clause: None,
            orderby_clause: Some(Box::new(Expression::Identifier("n".to_string()))),
            descending: true,
            select_clause: Box::new(Expression::Identifier("n".to_string())),
        });
        let code = gen.gen_expression(&expr);
        assert!(code.contains("sort_by_key(|n| n)"));
        assert!(code.contains("if true { _ltmp.reverse(); }"));
    }

    #[test]
    fn test_codegen_query_expression() {
        let mut gen = RustGenerator::new();
        let expr = Expression::Query(SurrealQueryNode {
            raw_query: "SELECT * FROM users WHERE age > 18".to_string(),
        });
        let code = gen.gen_expression(&expr);
        assert!(code.contains("__varg_query("));
        assert!(code.contains("SELECT * FROM users WHERE age > 18"));
    }

    #[test]
    fn test_codegen_prompt_literal_interpolation() {
        let mut gen = RustGenerator::new();
        let expr = Expression::PromptLiteral("Hello ${name}, you have ${count} items".to_string());
        let code = gen.gen_expression(&expr);
        assert!(code.contains("Prompt { text: format!"));
        assert!(code.contains("name"));
        assert!(code.contains("count"));
    }

    #[test]
    fn test_codegen_prompt_literal_plain() {
        let mut gen = RustGenerator::new();
        let expr = Expression::PromptLiteral("Hello world".to_string());
        let code = gen.gen_expression(&expr);
        assert!(code.contains("Prompt { text: format!"));
        assert!(code.contains("Hello world"));
    }

    #[test]
    fn test_codegen_stream_statement() {
        let program = Program {
            no_std: true, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(), is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Stream(Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: "llm_chat".to_string(),
                            args: vec![
                                Expression::Identifier("ctx".to_string()),
                                Expression::String("hi".to_string()),
                            ] }),
                    ]}),
                }],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("__varg_llm_chat_stream("));
    }

    #[test]
    fn test_codegen_throw_statement() {
        let program = Program {
            no_std: true, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(), is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::TryCatch {
                            try_block: Block { statements: vec![
                                Statement::Throw(Expression::String("something went wrong".to_string())),
                            ] },
                            catch_var: "err".to_string(),
                            catch_block: Block { statements: vec![
                                Statement::Print(Expression::Identifier("err".to_string())),
                            ] },
                        }
                    ]}),
                }],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("break 'varg_try Err("));
        assert!(code.contains("something went wrong"));
        assert!(code.contains("if let Err(mut err) = _varg_try_res"));
    }

    #[test]
    fn test_codegen_array_literal_direct() {
        let mut gen = RustGenerator::new();
        let expr = Expression::ArrayLiteral(vec![
            Expression::Int(1), Expression::Int(2), Expression::Int(3),
        ]);
        assert_eq!(gen.gen_expression(&expr), "vec![1, 2, 3]");
    }

    #[test]
    fn test_codegen_cosine_similarity_operator() {
        let mut gen = RustGenerator::new();
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Identifier("a".to_string())),
            operator: BinaryOperator::CosineSim,
            right: Box::new(Expression::Identifier("b".to_string())),
        };
        assert_eq!(gen.gen_expression(&expr), "__varg_cosine_sim(&a, &b)");
    }

    // ===== Plan 06: Destructuring CodeGen =====

    #[test]
    fn test_codegen_lambda_with_block_body() {
        let program = Program {
            no_std: true, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(), is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Let {
                            name: "process".to_string(),
                            ty: None,
                            value: Expression::Lambda {
                                params: vec![FieldDecl { name: "s".to_string(), ty: TypeNode::String, default_value: None }],
                                return_ty: None,
                                body: Box::new(LambdaBody::Block(Block { statements: vec![
                                        Statement::Return(Some(Expression::Identifier("s".to_string()))),
                                    ] })),
                            },
                        },
                    ]}),
                }],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("|s: String|"));
        assert!(code.contains("return s;"));
    }

    // ===== New Operators: &&, ||, !, %, unary, string concat =====

    #[test]
    fn test_codegen_and_operator() {
        let mut gen = RustGenerator::new();
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Identifier("a".to_string())),
            operator: BinaryOperator::And,
            right: Box::new(Expression::Identifier("b".to_string())),
        };
        assert_eq!(gen.gen_expression(&expr), "a && b");
    }

    #[test]
    fn test_codegen_or_operator() {
        let mut gen = RustGenerator::new();
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Identifier("a".to_string())),
            operator: BinaryOperator::Or,
            right: Box::new(Expression::Identifier("b".to_string())),
        };
        assert_eq!(gen.gen_expression(&expr), "a || b");
    }

    #[test]
    fn test_codegen_modulo_operator() {
        let mut gen = RustGenerator::new();
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Int(10)),
            operator: BinaryOperator::Mod,
            right: Box::new(Expression::Int(3)),
        };
        assert_eq!(gen.gen_expression(&expr), "10 % 3");
    }

    #[test]
    fn test_codegen_unary_negate() {
        let mut gen = RustGenerator::new();
        let expr = Expression::UnaryOp {
            operator: UnaryOperator::Negate,
            operand: Box::new(Expression::Int(5)),
        };
        assert_eq!(gen.gen_expression(&expr), "-5");
    }

    #[test]
    fn test_codegen_unary_not() {
        let mut gen = RustGenerator::new();
        let expr = Expression::UnaryOp {
            operator: UnaryOperator::Not,
            operand: Box::new(Expression::Identifier("flag".to_string())),
        };
        assert_eq!(gen.gen_expression(&expr), "!flag");
    }

    #[test]
    fn test_codegen_string_concat() {
        let mut gen = RustGenerator::new();
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::String("hello ".to_string())),
            operator: BinaryOperator::Add,
            right: Box::new(Expression::String("world".to_string())),
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("format!"));
        assert!(code.contains("hello "));
        assert!(code.contains("world"));
    }

    #[test]
    fn test_codegen_string_concat_with_identifier() {
        let mut gen = RustGenerator::new();
        // "Hello " + name  — left is string, right is identifier
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::String("Hello ".to_string())),
            operator: BinaryOperator::Add,
            right: Box::new(Expression::Identifier("name".to_string())),
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("format!"));
    }

    #[test]
    fn test_codegen_unary_negate_nested() {
        let mut gen = RustGenerator::new();
        // -(a + b)
        let expr = Expression::UnaryOp {
            operator: UnaryOperator::Negate,
            operand: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Identifier("a".to_string())),
                operator: BinaryOperator::Add,
                right: Box::new(Expression::Identifier("b".to_string())),
            }),
        };
        let code = gen.gen_expression(&expr);
        assert!(code.starts_with('-'));
        assert!(code.contains("a + b"));
    }

    // ===== Wave 5: break / continue =====

    #[test]
    fn test_codegen_break() {
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![Statement::Break] };
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("break;"));
    }

    #[test]
    fn test_codegen_continue() {
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![Statement::Continue] };
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("continue;"));
    }

    // ===== Wave 5: String Methods =====

    #[test]
    fn test_codegen_string_len() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("s".to_string())),
            method_name: "len".to_string(),
            args: vec![],
        };
        assert_eq!(gen.gen_expression(&expr), "s.len() as i64");
    }

    #[test]
    fn test_codegen_string_contains() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("s".to_string())),
            method_name: "contains".to_string(),
            args: vec![Expression::String("hello".to_string())],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("s.contains("));
        assert!(code.contains("hello"));
    }

    #[test]
    fn test_codegen_string_to_upper() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("name".to_string())),
            method_name: "to_upper".to_string(),
            args: vec![],
        };
        assert_eq!(gen.gen_expression(&expr), "name.to_uppercase()");
    }

    #[test]
    fn test_codegen_string_substring() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("text".to_string())),
            method_name: "substring".to_string(),
            args: vec![Expression::Int(2), Expression::Int(5)],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("text.chars().skip(2 as usize).take(5 as usize)"));
    }

    // ===== Wave 5: Collection Methods =====

    #[test]
    fn test_codegen_array_len() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("arr".to_string())),
            method_name: "len".to_string(),
            args: vec![],
        };
        assert_eq!(gen.gen_expression(&expr), "arr.len() as i64");
    }

    #[test]
    fn test_codegen_array_push() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("arr".to_string())),
            method_name: "push".to_string(),
            args: vec![Expression::Int(42)],
        };
        assert_eq!(gen.gen_expression(&expr), "arr.push(42)");
    }

    #[test]
    fn test_codegen_map_keys() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("map".to_string())),
            method_name: "keys".to_string(),
            args: vec![],
        };
        assert_eq!(gen.gen_expression(&expr), "map.keys().cloned().collect::<Vec<_>>()");
    }

    #[test]
    fn test_codegen_map_contains_key() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("map".to_string())),
            method_name: "contains_key".to_string(),
            args: vec![Expression::String("key".to_string())],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("map.contains_key("));
    }

    // ===== Wave 5: async / await =====

    #[test]
    fn test_codegen_async_method() {
        let program = Program {
            no_std: true, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "FetchData".to_string(), is_public: true, is_async: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Print(Expression::String("hello".to_string())),
                    ] }),
                }],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("async fn FetchData"));
    }

    #[test]
    fn test_codegen_await_expression() {
        let mut gen = RustGenerator::new();
        let expr = Expression::Await(Box::new(
            Expression::MethodCall {
                caller: Box::new(Expression::Identifier("client".to_string())),
                method_name: "fetch".to_string(),
                args: vec![Expression::String("url".to_string())],
            }
        ));
        let code = gen.gen_expression(&expr);
        assert!(code.contains(".await"));
    }

    // ===== Wave 5: const =====

    #[test]
    fn test_codegen_const_with_type() {
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::Const {
                name: "MAX".to_string(),
                ty: Some(TypeNode::Int),
                value: Expression::Int(100) },
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("let MAX: i64 = 100;"));
    }

    #[test]
    fn test_codegen_const_without_type() {
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::Const {
                name: "NAME".to_string(),
                ty: None,
                value: Expression::String("varg".to_string()) },
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("let NAME = \"varg\".to_string();"));
    }

    // ===== Wave 5b: Index/Property/Compound Assignment =====

    #[test]
    fn test_codegen_index_assign() {
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::IndexAssign {
                target: Expression::Identifier("arr".to_string()),
                index: Expression::Int(0),
                value: Expression::Int(42) },
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("arr[0 as usize] = 42;"));
    }

    #[test]
    fn test_codegen_property_assign() {
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::PropertyAssign {
                target: Expression::Identifier("obj".to_string()),
                property: "name".to_string(),
                value: Expression::String("alice".to_string()) },
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("obj.name = \"alice\".to_string();"));
    }

    #[test]
    fn test_codegen_map_insert_via_index_assign() {
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::IndexAssign {
                target: Expression::Identifier("map".to_string()),
                index: Expression::String("key".to_string()),
                value: Expression::String("value".to_string()) },
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("map.insert("));
    }

    // ===== Plan 19: Agent Lifecycle & State Tests =====

    #[test]
    fn test_codegen_agent_with_fields() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Counter".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![
                    FieldDecl { name: "count".to_string(), ty: TypeNode::Int, default_value: None },
                    FieldDecl { name: "name".to_string(), ty: TypeNode::String, default_value: None },
                ],
                methods: vec![],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("struct Counter"));
        assert!(code.contains("pub count: i64"));
        assert!(code.contains("pub name: String"));
    }

    #[test]
    fn test_codegen_agent_new_constructor() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "MyAgent".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![
                    FieldDecl { name: "value".to_string(), ty: TypeNode::Int, default_value: None },
                ],
                methods: vec![MethodDecl {
                    name: "Init".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![], constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] }),
                }],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("pub fn new() -> Self"));
        assert!(code.contains("__self.Init()"));
        assert!(code.contains("value: 0"));
    }

    #[test]
    fn test_codegen_agent_drop_destroy() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Cleanable".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Destroy".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![], constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] }),
                }],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("impl Drop for Cleanable"));
        assert!(code.contains("self.Destroy()"));
    }

    #[test]
    fn test_codegen_agent_field_self_access() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Stateful".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![
                    FieldDecl { name: "counter".to_string(), ty: TypeNode::Int, default_value: None },
                ],
                methods: vec![MethodDecl {
                    name: "Increment".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![], constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Assign {
                            name: "counter".to_string(),
                            value: Expression::Int(42) },
                    ]}),
                }],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        // Agent field 'counter' should be accessed via self.counter
        assert!(code.contains("self.counter = 42"));
    }

    // ===== Plan 17: Implicit Context Passing Tests =====

    #[test]
    fn test_codegen_context_agent_struct() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "SmartAgent".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![Annotation { name: "WithContext".to_string(), values: vec![] }],
                implements: vec![],
                fields: vec![
                    FieldDecl { name: "context".to_string(), ty: TypeNode::Context, default_value: None },
                ],
                methods: vec![],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("pub context: Context"));
        assert!(code.contains("Context::new(\"default\")"));
    }

    #[test]
    fn test_codegen_context_implicit_access() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "CtxAgent".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![
                    FieldDecl { name: "context".to_string(), ty: TypeNode::Context, default_value: None },
                ],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![], constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        // Accessing 'context' should generate 'self.context'
                        Statement::Print(Expression::Identifier("context".to_string())),
                    ] }),
                }],
            })]
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("self.context"));
    }

    // ===== Plan 16: Agent-to-Agent Messaging Tests =====

    #[test]
    fn test_codegen_spawn_with_dispatch() {
        // Two agents: Coordinator spawns Worker
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "Worker".to_string(),
                    is_system: false,
                    is_public: false,
                    target_annotation: None,
                    annotations: vec![],
                    implements: vec![],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Process".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![],
                        type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "input".to_string(), ty: TypeNode::String, default_value: None }],
                        return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::String("done".to_string()))),
                        ] }),
                    }],
                }),
                Item::Agent(AgentDef {
                    name: "Coordinator".to_string(),
                    is_system: false,
                    is_public: false,
                    target_annotation: None,
                    annotations: vec![],
                    implements: vec![],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![],
                        type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "worker".to_string(),
                                ty: None,
                                value: Expression::Spawn {
                                    agent_name: "Worker".to_string(),
                                    args: vec![] },
                            },
                        ]}),
                    }],
                }),
            ],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        // spawn should create channel + thread with method dispatch
        assert!(code.contains("std::sync::mpsc::channel"));
        assert!(code.contains("std::thread::spawn"));
        assert!(code.contains("\"Process\" =>"));
        assert!(code.contains("__agent.Process("));
    }

    #[test]
    fn test_codegen_send() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("worker".to_string())),
            method_name: "send".to_string(),
            args: vec![
                Expression::String("Process".to_string()),
                Expression::String("hello".to_string()),
            ],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains(".send("));
        assert!(code.contains("None"));
        assert!(code.contains("vec!["));
    }

    #[test]
    fn test_codegen_request() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("worker".to_string())),
            method_name: "request".to_string(),
            args: vec![
                Expression::String("Process".to_string()),
                Expression::String("data".to_string()),
            ],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("__reply_tx"));
        assert!(code.contains("__reply_rx"));
        assert!(code.contains("recv().unwrap()"));
    }

    #[test]
    fn test_codegen_spawn_agent_with_fields() {
        // Spawn an agent that has fields → should use ::new()
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "StatefulWorker".to_string(),
                    is_system: false,
                    is_public: false,
                    target_annotation: None,
                    annotations: vec![],
                    implements: vec![],
                    fields: vec![
                        FieldDecl { name: "count".to_string(), ty: TypeNode::Int, default_value: None },
                    ],
                    methods: vec![MethodDecl {
                        name: "Tick".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![],
                        type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![] }),
                    }],
                }),
                Item::Agent(AgentDef {
                    name: "Main".to_string(),
                    is_system: false,
                    is_public: false,
                    target_annotation: None,
                    annotations: vec![],
                    implements: vec![],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![],
                        type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "w".to_string(),
                                ty: None,
                                value: Expression::Spawn {
                                    agent_name: "StatefulWorker".to_string(),
                                    args: vec![] },
                            },
                        ]}),
                    }],
                }),
            ],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("StatefulWorker::new()"));
        assert!(code.contains("\"Tick\" =>"));
    }

    // ===== Plan 20: Actor-Model Concurrency Tests =====

    #[test]
    fn test_codegen_select_multi_agent() {
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::Select { arms: vec![
                SelectArm {
                    var_name: "msg".to_string(),
                    source: SelectSource::Agent(Expression::Identifier("worker".to_string())),
                    body: Block { statements: vec![
                        Statement::Print(Expression::Identifier("msg".to_string())),
                    ] },
                },
                SelectArm {
                    var_name: "_timeout".to_string(),
                    source: SelectSource::Timeout(Expression::Int(5000)),
                    body: Block { statements: vec![
                        Statement::Print(Expression::String("timeout".to_string())),
                    ] },
                },
            ]},
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("loop {"));
        assert!(code.contains("try_recv()"));
        assert!(code.contains("Instant::now()"));
        assert!(code.contains("5000 as u64"));
        assert!(code.contains("break;"));
    }

    #[test]
    fn test_codegen_select_agent_only() {
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::Select { arms: vec![
                SelectArm {
                    var_name: "msg".to_string(),
                    source: SelectSource::Agent(Expression::Identifier("agent1".to_string())),
                    body: Block { statements: vec![] },
                },
            ]},
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("agent1.try_recv()"));
        // No timeout — should not have Instant::now()
        assert!(!code.contains("Instant::now()"));
    }

    // ===== Plan 22: Simplified Memory Model Tests =====

    #[test]
    fn test_clone_on_method_call_args() {
        // Identifier args in user-defined method calls get .clone()
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("obj".to_string())),
            method_name: "custom_method".to_string(),
            args: vec![Expression::Identifier("name".to_string())],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("name.clone()"), "Expected name.clone(), got: {}", code);
    }

    #[test]
    fn test_no_clone_on_literals() {
        // Literal args don't get .clone()
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("obj".to_string())),
            method_name: "custom_method".to_string(),
            args: vec![Expression::String("hello".to_string()), Expression::Int(42)],
        };
        let code = gen.gen_expression(&expr);
        // String literals become "hello".to_string() — no extra .clone()
        assert!(!code.contains(".clone()"), "Literals shouldn't be cloned, got: {}", code);
    }

    #[test]
    fn test_clone_agent_field_in_method_call() {
        // Agent fields get self.field.clone()
        let mut gen = RustGenerator::new();
        gen.agent_field_names.insert("data".to_string());
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("obj".to_string())),
            method_name: "process".to_string(),
            args: vec![Expression::Identifier("data".to_string())],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("self.data.clone()"), "Expected self.data.clone(), got: {}", code);
    }

    #[test]
    fn test_builtin_methods_no_double_clone() {
        // Built-in methods like push/contains handle args themselves, no extra clone
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("vec".to_string())),
            method_name: "push".to_string(),
            args: vec![Expression::Identifier("item".to_string())],
        };
        let code = gen.gen_expression(&expr);
        // push uses arg_strs directly (not gen_cloned_arg)
        assert_eq!(code, "vec.push(item)");
    }

    // ---- Plan 23: Prompt Template Codegen Tests ----
    #[test]
    fn test_codegen_prompt_template_function() {
        let mut gen = RustGenerator::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::PromptTemplate(PromptTemplateDef {
                name: "greet".to_string(),
                params: vec![],
                body: "Hello, World!".to_string(),
            })],
        };
        let code = gen.generate(&program);
        assert!(code.contains("fn greet() -> Prompt"));
        assert!(code.contains("Hello, World!"));
        assert!(code.contains("Prompt { text:"));
    }

    #[test]
    fn test_codegen_prompt_interpolation() {
        let mut gen = RustGenerator::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::PromptTemplate(PromptTemplateDef {
                name: "analyze".to_string(),
                params: vec![
                    FieldDecl { name: "text".to_string(), ty: TypeNode::String, default_value: None },
                    FieldDecl { name: "fmt".to_string(), ty: TypeNode::String, default_value: None },
                ],
                body: "Analyze: {text}\nFormat: {fmt}".to_string(),
            })],
        };
        let code = gen.generate(&program);
        assert!(code.contains("fn analyze(text: String, fmt: String) -> Prompt"));
        assert!(code.contains("format!"));
        assert!(code.contains("text"));
        assert!(code.contains("fmt"));
    }

    // ---- Plan 24: Error Propagation Codegen Tests ----
    #[test]
    fn test_codegen_question_mark_operator() {
        let mut gen = RustGenerator::new();
        let expr = Expression::TryPropagate(
            Box::new(Expression::MethodCall {
                caller: Box::new(Expression::Identifier("self".to_string())),
                method_name: "fetch".to_string(),
                args: vec![Expression::String("url".to_string())],
            })
        );
        let code = gen.gen_expression(&expr);
        assert!(code.contains("?"));
        assert!(code.contains("fetch"));
    }

    #[test]
    fn test_codegen_or_default() {
        let mut gen = RustGenerator::new();
        let expr = Expression::OrDefault {
            expr: Box::new(Expression::MethodCall {
                caller: Box::new(Expression::Identifier("self".to_string())),
                method_name: "fetch".to_string(),
                args: vec![Expression::String("url".to_string())],
            }),
            default: Box::new(Expression::String("fallback".to_string())),
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("unwrap_or_else"));
        assert!(code.contains("fallback"));
    }

    // ===== Plan 25: Standalone Functions =====
    #[test]
    fn test_codegen_standalone_function() {
        let mut gen = RustGenerator::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Function(FunctionDef {
                name: "add".to_string(),
                is_public: false,
                params: vec![
                    FieldDecl { name: "a".to_string(), ty: TypeNode::Int, default_value: None },
                    FieldDecl { name: "b".to_string(), ty: TypeNode::Int, default_value: None },
                ],
                return_ty: Some(TypeNode::Int),
                body: Block { statements: vec![
                    Statement::Return(Some(Expression::BinaryOp {
                        left: Box::new(Expression::Identifier("a".to_string())),
                        operator: BinaryOperator::Add,
                        right: Box::new(Expression::Identifier("b".to_string())) })),
                ]},
            })],
        };
        let code = gen.generate(&program);
        assert!(code.contains("fn add(a: i64, b: i64) -> i64"));
        assert!(code.contains("return"));
    }

    // ===== Plan 27: Async Runtime =====
    #[test]
    fn test_codegen_async_method_generates_async_fn() {
        let mut gen = RustGenerator::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Worker".to_string(), is_system: false, is_public: false,
                target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                methods: vec![MethodDecl {
                    name: "Fetch".to_string(), is_public: true, is_async: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                        Statement::Return(Some(Expression::String("data".to_string()))),
                    ] }),
                }],
            })],
        };
        let code = gen.generate(&program);
        assert!(code.contains("async fn Fetch"));
        assert!(gen.use_async); // should detect async
    }

    #[test]
    fn test_codegen_sync_program_no_tokio_flag() {
        let mut gen = RustGenerator::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "App".to_string(), is_system: false, is_public: false,
                target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(), is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] }),
                }],
            })],
        };
        let _code = gen.generate(&program);
        assert!(!gen.use_async);
    }

    #[test]
    fn test_codegen_spawn_uses_tokio_when_async() {
        let mut gen = RustGenerator::new();
        // First generate a program with async to set the flag
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Worker".to_string(), is_system: false, is_public: false,
                target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                methods: vec![MethodDecl {
                    name: "Process".to_string(), is_public: true, is_async: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                        Statement::Return(Some(Expression::String("ok".to_string()))),
                    ] }),
                }],
            })],
        };
        let _ = gen.generate(&program);
        // Now test spawn expression
        let spawn = Expression::Spawn { agent_name: "Worker".to_string(), args: vec![] };
        let code = gen.gen_expression(&spawn);
        assert!(code.contains("tokio::spawn"));
        assert!(code.contains("tokio::sync::mpsc"));
    }

    // ---- Plan 37: Range Expression Codegen ----

    #[test]
    fn test_codegen_range_exclusive() {
        let mut gen = RustGenerator::new();
        let range = Expression::Range {
            start: Box::new(Expression::Int(0)),
            end: Box::new(Expression::Int(10)),
            inclusive: false,
        };
        let code = gen.gen_expression(&range);
        assert_eq!(code, "(0..10)");
    }

    #[test]
    fn test_codegen_range_inclusive() {
        let mut gen = RustGenerator::new();
        let range = Expression::Range {
            start: Box::new(Expression::Int(0)),
            end: Box::new(Expression::Int(10)),
            inclusive: true,
        };
        let code = gen.gen_expression(&range);
        assert_eq!(code, "(0..=10)");
    }

    // ---- Plan 38: Tuple Codegen ----

    #[test]
    fn test_codegen_tuple_literal() {
        let mut gen = RustGenerator::new();
        let tuple = Expression::TupleLiteral(vec![
            Expression::Int(1),
            Expression::String("hello".to_string()),
        ]);
        let code = gen.gen_expression(&tuple);
        assert_eq!(code, "(1, \"hello\".to_string())");
    }

    #[test]
    fn test_codegen_tuple_type() {
        let gen = RustGenerator::new();
        let ty = TypeNode::Tuple(vec![TypeNode::Int, TypeNode::String, TypeNode::Bool]);
        let code = gen.gen_type(&ty);
        assert_eq!(code, "(i64, String, bool)");
    }

    // ---- Plan 39: Trait Bounds Codegen Tests ----

    #[test]
    fn test_codegen_inline_multiple_bounds() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Sorter".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Sort".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec!["T".to_string()],
                    constraints: vec![GenericConstraint {
                        type_param: "T".to_string(),
                        bounds: vec!["Display".to_string(), "Clone".to_string()],
                    }],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] }),
                }],
            })],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("fn Sort<T: Display + Clone>"), "Expected inline bounds, got: {}", code);
    }

    #[test]
    fn test_codegen_multiple_type_params_with_bounds() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Processor".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec!["T".to_string(), "U".to_string()],
                    constraints: vec![
                        GenericConstraint { type_param: "T".to_string(), bounds: vec!["Display".to_string()] },
                        GenericConstraint { type_param: "U".to_string(), bounds: vec!["Send".to_string(), "Sync".to_string()] },
                    ],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] }),
                }],
            })],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("fn Run<T: Display, U: Send + Sync>"), "Expected inline bounds, got: {}", code);
    }

    // ---- Plan 42: Float & Stdlib Codegen Tests ----

    #[test]
    fn test_codegen_float_literal() {
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&Expression::Float(3.14));
        assert_eq!(code, "3.14_f64");
    }

    #[test]
    fn test_codegen_float_type() {
        let gen = RustGenerator::new();
        assert_eq!(gen.gen_type(&TypeNode::Float), "f64");
    }

    #[test]
    fn test_codegen_parse_int_method() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("s".to_string())),
            method_name: "parse_int".to_string(),
            args: vec![],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("parse::<i64>()"), "Expected parse_int codegen, got: {}", code);
    }

    #[test]
    fn test_codegen_abs_method() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("x".to_string())),
            method_name: "abs".to_string(),
            args: vec![],
        };
        let code = gen.gen_expression(&expr);
        assert_eq!(code, "x.abs()");
    }

    // ---- Plan 43: Iterator Chain Codegen Tests ----

    #[test]
    fn test_codegen_filter() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "filter".to_string(),
            args: vec![Expression::Lambda {
                params: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None }],
                return_ty: None,
                body: Box::new(LambdaBody::Expression(
                    Expression::BinaryOp {
                        left: Box::new(Expression::Identifier("x".to_string())),
                        operator: BinaryOperator::Gt,
                        right: Box::new(Expression::Int(0)),
                    }
                )),
            }],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("into_iter().filter("), "Expected filter chain, got: {}", code);
        assert!(code.contains("collect::<Vec<_>>()"), "Expected collect, got: {}", code);
    }

    #[test]
    fn test_codegen_map() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "map".to_string(),
            args: vec![Expression::Lambda {
                params: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None }],
                return_ty: None,
                body: Box::new(LambdaBody::Expression(
                    Expression::BinaryOp {
                        left: Box::new(Expression::Identifier("x".to_string())),
                        operator: BinaryOperator::Mul,
                        right: Box::new(Expression::Int(2)),
                    }
                )),
            }],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("into_iter().map("), "Expected map chain, got: {}", code);
    }

    #[test]
    fn test_codegen_any() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "any".to_string(),
            args: vec![Expression::Lambda {
                params: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None }],
                return_ty: None,
                body: Box::new(LambdaBody::Expression(
                    Expression::BinaryOp {
                        left: Box::new(Expression::Identifier("x".to_string())),
                        operator: BinaryOperator::Gt,
                        right: Box::new(Expression::Int(5)),
                    }
                )),
            }],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("into_iter().any("), "Expected any, got: {}", code);
    }

    #[test]
    fn test_codegen_first_last() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "first".to_string(),
            args: vec![],
        };
        let code = gen.gen_expression(&expr);
        assert_eq!(code, "items.first().cloned()");
    }

    // ---- Plan 41: Crate Import Codegen Test ----

    #[test]
    fn test_codegen_crate_import() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::CrateImport {
                crate_name: "reqwest".to_string(),
                version: "0.12".to_string(),
                features: vec![],
            }],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("use reqwest;"), "Expected use statement, got: {}", code);
    }

    // ---- F41-1: UseExtern Tests ----

    #[test]
    fn test_codegen_use_extern_simple() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::UseExtern {
                path: vec!["axum".to_string(), "Router".to_string()],
            }],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("use axum::Router;"), "Expected use axum::Router, got: {}", code);
    }

    #[test]
    fn test_codegen_use_extern_deep_path() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::UseExtern {
                path: vec!["axum".to_string(), "routing".to_string(), "get".to_string()],
            }],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("use axum::routing::get;"), "Expected deep path, got: {}", code);
    }

    #[test]
    fn test_codegen_use_extern_braced() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::UseExtern {
                path: vec!["axum".to_string(), "{Router, Json}".to_string()],
            }],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("use axum::{Router, Json};"), "Expected braced import, got: {}", code);
    }

    // ---- F41-5: Result Method Codegen Tests ----

    #[test]
    fn test_codegen_map_err() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("result".to_string())),
            method_name: "map_err".to_string(),
            args: vec![Expression::Lambda {
                params: vec![FieldDecl { name: "e".to_string(), ty: TypeNode::String, default_value: None }],
                return_ty: None,
                body: Box::new(LambdaBody::Expression(Expression::String("mapped".to_string()))),
            }],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains(".map_err("), "Expected .map_err(), got: {}", code);
    }

    #[test]
    fn test_codegen_unwrap() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("result".to_string())),
            method_name: "unwrap".to_string(),
            args: vec![],
        };
        let code = gen.gen_expression(&expr);
        assert_eq!(code, "result.unwrap()");
    }

    #[test]
    fn test_codegen_is_ok() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("result".to_string())),
            method_name: "is_ok".to_string(),
            args: vec![],
        };
        let code = gen.gen_expression(&expr);
        assert_eq!(code, "result.is_ok()");
    }

    #[test]
    fn test_codegen_unwrap_or() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("result".to_string())),
            method_name: "unwrap_or".to_string(),
            args: vec![Expression::String("default".to_string())],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains(".unwrap_or("), "Expected .unwrap_or(), got: {}", code);
    }

    #[test]
    fn test_codegen_and_then() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("result".to_string())),
            method_name: "and_then".to_string(),
            args: vec![Expression::Lambda {
                params: vec![FieldDecl { name: "v".to_string(), ty: TypeNode::String, default_value: None }],
                return_ty: None,
                body: Box::new(LambdaBody::Expression(Expression::Identifier("v".to_string()))),
            }],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains(".and_then("), "Expected .and_then(), got: {}", code);
    }

    // ---- F41-2/3/4/8: Runtime Builtin Codegen Tests ----

    #[test]
    fn test_codegen_http_serve() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "http_serve".to_string(),
            args: vec![Expression::Identifier("cap".to_string())],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("__varg_http_server()"), "Expected http_server, got: {}", code);
    }

    #[test]
    fn test_codegen_db_open() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "db_open".to_string(),
            args: vec![Expression::Identifier("cap".to_string()), Expression::String("test.db".to_string())],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("__varg_db_open"), "Expected __varg_db_open, got: {}", code);
    }

    #[test]
    fn test_codegen_ws_connect() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "ws_connect".to_string(),
            args: vec![Expression::Identifier("cap".to_string()), Expression::String("ws://localhost".to_string())],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("__varg_ws_connect"), "Expected __varg_ws_connect, got: {}", code);
    }

    #[test]
    fn test_codegen_mcp_connect() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "mcp_connect".to_string(),
            args: vec![
                Expression::Identifier("cap".to_string()),
                Expression::String("npx".to_string()),
                Expression::ArrayLiteral(vec![Expression::String("-y".to_string())]),
            ],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("__varg_mcp_connect"), "Expected __varg_mcp_connect, got: {}", code);
    }

    #[test]
    fn test_codegen_mcp_call_tool() {
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "mcp_call_tool".to_string(),
            args: vec![
                Expression::Identifier("conn".to_string()),
                Expression::String("read_file".to_string()),
                Expression::MapLiteral(vec![]),
            ],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("__varg_mcp_call_tool"), "Expected __varg_mcp_call_tool, got: {}", code);
    }

    // ---- F41-6: DI Codegen Tests ----

    #[test]
    fn test_codegen_contract_field_box_dyn() {
        let program = Program {
            no_std: false,
            docs: std::collections::HashMap::new(),
            items: vec![
                Item::Contract(ContractDef {
                    name: "ILogger".to_string(),
                    is_public: false,
                    target_annotation: None,
                    methods: vec![MethodDecl {
                        name: "log".to_string(),
                        args: vec![FieldDecl { name: "msg".to_string(), ty: TypeNode::String, default_value: None }],
                        return_ty: Some(TypeNode::Void),
                        body: None,
                        is_public: true,
                        is_async: false,
                        annotations: vec![],
                        type_params: vec![],
                        constraints: vec![],
                    }],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(),
                    is_system: false,
                    is_public: false,
                    target_annotation: None,
                    fields: vec![FieldDecl {
                        name: "logger".to_string(),
                        ty: TypeNode::Custom("ILogger".to_string()),
                        default_value: None,
                    }],
                    methods: vec![],
                    implements: vec![],
                    annotations: vec![],
                }),
            ],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("Box<dyn ILogger>"), "Contract field should be Box<dyn ILogger>: {}", code);
        assert!(!code.contains("pub fn new() -> Self"), "Agent with contract field should not have auto-new(): {}", code);
    }

    #[test]
    fn test_codegen_struct_literal_box_wrapping() {
        let program = Program {
            no_std: false,
            docs: std::collections::HashMap::new(),
            items: vec![
                Item::Contract(ContractDef {
                    name: "ILogger".to_string(),
                    is_public: false,
                    target_annotation: None,
                    methods: vec![],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(),
                    is_system: false,
                    is_public: false,
                    target_annotation: None,
                    fields: vec![FieldDecl {
                        name: "logger".to_string(),
                        ty: TypeNode::Custom("ILogger".to_string()),
                        default_value: None,
                    }],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Expr(Expression::Identifier("noop".to_string())),
                        ]}),
                        is_public: true,
                        is_async: false,
                        annotations: vec![],
                        type_params: vec![],
                        constraints: vec![],
                    }],
                    implements: vec![],
                    annotations: vec![],
                }),
                Item::Function(FunctionDef {
                    name: "main".to_string(),
                    is_public: true,
                    params: vec![],
                    return_ty: None,
                    body: Block { statements: vec![
                        Statement::Let {
                            name: "app".to_string(),
                            ty: None,
                            value: Expression::StructLiteral {
                                type_name: "App".to_string(),
                                fields: vec![("logger".to_string(), Expression::Identifier("my_logger".to_string()))],
                            },
                        },
                    ]},
                }),
            ],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("Box::new(my_logger)"), "Contract field in struct literal should be wrapped in Box::new(): {}", code);
    }

    // ---- Plan 46: Source Map Tests ----

    #[test]
    fn test_byte_offset_to_line() {
        let source = "line1\nline2\nline3\n";
        assert_eq!(byte_offset_to_line(source, 0), 1);
        assert_eq!(byte_offset_to_line(source, 5), 1); // newline char
        assert_eq!(byte_offset_to_line(source, 6), 2);
        assert_eq!(byte_offset_to_line(source, 12), 3);
    }

    #[test]
    fn test_source_map_comments_emitted() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Let { name: "x".to_string(), ty: None, value: Expression::Int(42) },
                        Statement::Return(None),
                    ] }),
                }],
            })],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate_with_source_map(&program, "");
        assert!(code.contains("// .varg:1"), "Expected source map comment .varg:1, got: {}", code);
        assert!(code.contains("// .varg:2"), "Expected source map comment .varg:2, got: {}", code);
    }

    // ===== Plan 52: env() builtin =====

    #[test]
    fn test_codegen_env() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "env".to_string(),
            args: vec![Expression::String("API_KEY".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("std::env::var"), "Expected std::env::var, got: {}", code);
        assert!(code.contains("unwrap_or_default()"), "Expected unwrap_or_default, got: {}", code);
        assert!(code.contains("API_KEY"), "Expected API_KEY, got: {}", code);
    }

    // ===== Plan 53: Self-Field Clone Generalization =====

    #[test]
    fn test_let_from_self_field_cloned() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Bot".to_string(),
                is_system: false,
                is_public: true,
                target_annotation: None,
                annotations: vec![],
                implements: vec![],
                fields: vec![FieldDecl { name: "name".to_string(), ty: TypeNode::String, default_value: None }],
                methods: vec![MethodDecl {
                    name: "get_name".to_string(),
                    is_public: true,
                    is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                        Statement::Let { name: "x".to_string(), ty: None, value: Expression::Identifier("name".to_string()) },
                    ]}),
                }],
            })],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("self.name.clone()"), "Let from self field should clone: {}", code);
    }

    #[test]
    fn test_return_self_field_still_cloned() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Bot".to_string(),
                is_system: false,
                is_public: true,
                target_annotation: None,
                annotations: vec![],
                implements: vec![],
                fields: vec![FieldDecl { name: "data".to_string(), ty: TypeNode::String, default_value: None }],
                methods: vec![MethodDecl {
                    name: "get_data".to_string(),
                    is_public: true,
                    is_async: false,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                        Statement::Return(Some(Expression::Identifier("data".to_string()))),
                    ]}),
                }],
            })],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("self.data.clone()"), "Return of self field should clone: {}", code);
    }

    #[test]
    fn test_self_field_method_call_no_clone() {
        let mut gen = RustGenerator::new();
        gen.agent_field_names.insert("items".to_string());
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "push".to_string(),
            args: vec![Expression::Int(42)],
        };
        let code = gen.gen_expression(&expr);
        assert!(!code.contains(".clone().push"), "Method call on self field should not clone caller: {}", code);
    }

    #[test]
    fn test_clone_self_field_helper() {
        let gen = RustGenerator::new();
        assert_eq!(gen.clone_self_field_if_needed("self.name"), "self.name.clone()");
        assert_eq!(gen.clone_self_field_if_needed("self.count"), "self.count.clone()");
        assert_eq!(gen.clone_self_field_if_needed("self.items.len()"), "self.items.len()");
        assert_eq!(gen.clone_self_field_if_needed("self.name.clone()"), "self.name.clone()");
        assert_eq!(gen.clone_self_field_if_needed("x"), "x");
    }

    // ===== Wave 11: Type Casting Codegen =====

    #[test]
    fn test_codegen_cast_int_to_float() {
        let mut gen = RustGenerator::new();
        let expr = Expression::Cast {
            expr: Box::new(Expression::Int(42)),
            target_type: TypeNode::Float,
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("as f64"), "Expected 'as f64', got: {}", code);
    }

    #[test]
    fn test_codegen_cast_float_to_int() {
        let mut gen = RustGenerator::new();
        let expr = Expression::Cast {
            expr: Box::new(Expression::Float(3.14)),
            target_type: TypeNode::Int,
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("as i64"), "Expected 'as i64', got: {}", code);
    }

    #[test]
    fn test_codegen_cast_to_string() {
        let mut gen = RustGenerator::new();
        let expr = Expression::Cast {
            expr: Box::new(Expression::Identifier("count".to_string())),
            target_type: TypeNode::String,
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("format!"), "Expected format! for string cast, got: {}", code);
    }

    #[test]
    fn test_codegen_cast_to_ulong() {
        let mut gen = RustGenerator::new();
        let expr = Expression::Cast {
            expr: Box::new(Expression::Int(10)),
            target_type: TypeNode::Ulong,
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("as u64"), "Expected 'as u64', got: {}", code);
    }

    // ===== Wave 11: If-Expression Codegen =====

    #[test]
    fn test_codegen_if_expression() {
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::Let {
                name: "x".to_string(),
                ty: None,
                value: Expression::IfExpr {
                    condition: Box::new(Expression::Bool(true)),
                    then_block: Block { statements: vec![Statement::Expr(Expression::Int(1))] },
                    else_block: Block { statements: vec![Statement::Expr(Expression::Int(0))] },
                },
            },
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("if true"), "Expected 'if true', got: {}", code);
        assert!(code.contains("else"), "Expected 'else', got: {}", code);
    }

    // ===== Wave 11: Match Guard Codegen =====

    #[test]
    fn test_codegen_match_with_guard() {
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::Match {
                subject: Expression::Identifier("x".to_string()),
                arms: vec![
                    MatchArm {
                        pattern: Pattern::Variant("Ok".to_string(), vec!["val".to_string()]),
                        guard: Some(Expression::BinaryOp {
                            left: Box::new(Expression::Identifier("val".to_string())),
                            operator: BinaryOperator::Gt,
                            right: Box::new(Expression::Int(0)),
                        }),
                        body: Block { statements: vec![Statement::Print(Expression::String("positive".to_string()))] },
                    },
                    MatchArm {
                        pattern: Pattern::Wildcard,
                        guard: None,
                        body: Block { statements: vec![Statement::Print(Expression::String("other".to_string()))] },
                    },
                ],
            },
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("if val > 0"), "Expected guard 'if val > 0', got: {}", code);
    }

    // ===== Wave 11: Retry Block Returns Value =====

    #[test]
    fn test_codegen_retry_returns_value() {
        let mut gen = RustGenerator::new();
        let expr = Expression::Retry {
            max_attempts: Box::new(Expression::Int(3)),
            body: Box::new(Block { statements: vec![
                Statement::Expr(Expression::MethodCall {
                    caller: Box::new(Expression::Identifier("self".to_string())),
                    method_name: "process".to_string(),
                    args: vec![Expression::String("data".to_string())],
                }),
            ]}),
            fallback: Some(Box::new(Block { statements: vec![
                Statement::Expr(Expression::String("fallback-result".to_string())),
            ]})),
        };
        let code = gen.gen_expression(&expr);
        // The body should NOT have a semicolon after process() — it's the return value
        assert!(code.contains("Ok("), "Expected Ok(...) wrapping, got: {}", code);
        // The fallback should contain the fallback string
        assert!(code.contains("fallback-result"), "Expected fallback-result in code, got: {}", code);
    }

    // ===== Wave 11: Pipe Operator Fixed =====

    #[test]
    fn test_codegen_pipe_calls_on_expr() {
        // a |> f() should generate a.f(), NOT self.f(a)
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::Let {
                name: "x".to_string(),
                ty: None,
                value: Expression::MethodCall {
                    caller: Box::new(Expression::Identifier("data".to_string())),
                    method_name: "process".to_string(),
                    args: vec![],
                },
            },
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("data.process()"), "Expected data.process(), got: {}", code);
        assert!(!code.contains("self.process(data)"), "Should NOT generate self.process(data)");
    }

    // ===== Realistic Codegen Use Case Tests =====

    #[test]
    fn test_realistic_codegen_agent_with_fields_and_methods() {
        // Full agent: fields, Init, Run with self-field access
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Counter".to_string(),
                is_system: false, is_public: true,
                target_annotation: None, annotations: vec![],
                implements: vec![],
                fields: vec![
                    FieldDecl { name: "count".to_string(), ty: TypeNode::Int, default_value: None },
                    FieldDecl { name: "name".to_string(), ty: TypeNode::String, default_value: None },
                ],
                methods: vec![
                    MethodDecl {
                        name: "Init".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Assign { name: "count".to_string(), value: Expression::Int(0) },
                            Statement::Assign { name: "name".to_string(), value: Expression::String("default".to_string()) },
                        ]}),
                    },
                    MethodDecl {
                        name: "Increment".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Int),
                        body: Some(Block { statements: vec![
                            Statement::Assign {
                                name: "count".to_string(),
                                value: Expression::BinaryOp {
                                    left: Box::new(Expression::Identifier("count".to_string())),
                                    operator: BinaryOperator::Add,
                                    right: Box::new(Expression::Int(1)),
                                },
                            },
                            Statement::Return(Some(Expression::Identifier("count".to_string()))),
                        ]}),
                    },
                ],
            })],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        // Should have struct with fields
        assert!(code.contains("struct Counter"), "Missing struct: {}", code);
        assert!(code.contains("count: i64"), "Missing count field: {}", code);
        assert!(code.contains("name: String"), "Missing name field: {}", code);
        // Should have new() constructor
        assert!(code.contains("fn new()"), "Missing new(): {}", code);
        // Methods should use self.field
        assert!(code.contains("self.count"), "Missing self.count: {}", code);
    }

    #[test]
    fn test_realistic_codegen_nested_loops() {
        // While > foreach > if — common data processing pattern
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::Let { name: "round".to_string(), ty: Some(TypeNode::Int), value: Expression::Int(0) },
            Statement::While {
                condition: Expression::BinaryOp {
                    left: Box::new(Expression::Identifier("round".to_string())),
                    operator: BinaryOperator::Lt,
                    right: Box::new(Expression::Int(3)),
                },
                body: Block { statements: vec![
                    Statement::Foreach {
                        item_name: "item".to_string(),
                        value_name: None,
                        collection: Expression::Identifier("items".to_string()),
                        body: Block { statements: vec![
                            Statement::If {
                                condition: Expression::BinaryOp {
                                    left: Box::new(Expression::Identifier("item".to_string())),
                                    operator: BinaryOperator::Gt,
                                    right: Box::new(Expression::Int(5)),
                                },
                                then_block: Block { statements: vec![
                                    Statement::Print(Expression::Identifier("item".to_string())),
                                ]},
                                else_block: None,
                            },
                        ]},
                    },
                    Statement::Assign {
                        name: "round".to_string(),
                        value: Expression::BinaryOp {
                            left: Box::new(Expression::Identifier("round".to_string())),
                            operator: BinaryOperator::Add,
                            right: Box::new(Expression::Int(1)),
                        },
                    },
                ]},
            },
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("while round < 3"), "Missing while: {}", code);
        assert!(code.contains("for mut item in"), "Missing for-in: {}", code);
        assert!(code.contains("if item > 5"), "Missing if: {}", code);
    }

    #[test]
    fn test_realistic_codegen_try_catch_with_throw() {
        // Try/catch with throw — error recovery pattern
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::TryCatch {
                try_block: Block { statements: vec![
                    Statement::Let {
                        name: "data".to_string(),
                        ty: None,
                        value: Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: "fetch".to_string(),
                            args: vec![Expression::String("url".to_string())],
                        },
                    },
                    Statement::Print(Expression::Identifier("data".to_string())),
                ]},
                catch_var: "err".to_string(),
                catch_block: Block { statements: vec![
                    Statement::Print(Expression::InterpolatedString(vec![
                        InterpolationPart::Literal("Error: ".to_string()),
                        InterpolationPart::Expression(Expression::Identifier("err".to_string())),
                    ])),
                ]},
            },
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("Result<(), String>"), "Missing Result type: {}", code);
        assert!(code.contains("Ok(())"), "Missing Ok: {}", code);
        assert!(code.contains("Err(mut err)"), "Missing Err binding: {}", code);
    }

    #[test]
    fn test_realistic_codegen_iterator_chain_filter_map() {
        // scores.filter(|s| s >= 80).map(|s| s * 2)
        let mut gen = RustGenerator::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::MethodCall {
                caller: Box::new(Expression::Identifier("scores".to_string())),
                method_name: "filter".to_string(),
                args: vec![Expression::Lambda {
                    params: vec![FieldDecl { name: "s".to_string(), ty: TypeNode::Int, default_value: None }],
                    return_ty: None,
                    body: Box::new(LambdaBody::Expression(Expression::BinaryOp {
                        left: Box::new(Expression::Identifier("s".to_string())),
                        operator: BinaryOperator::GtEq,
                        right: Box::new(Expression::Int(80)),
                    })),
                }],
            }),
            method_name: "map".to_string(),
            args: vec![Expression::Lambda {
                params: vec![FieldDecl { name: "s".to_string(), ty: TypeNode::Int, default_value: None }],
                return_ty: None,
                body: Box::new(LambdaBody::Expression(Expression::BinaryOp {
                    left: Box::new(Expression::Identifier("s".to_string())),
                    operator: BinaryOperator::Mul,
                    right: Box::new(Expression::Int(2)),
                })),
            }],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains(".filter("), "Missing filter: {}", code);
        assert!(code.contains(".map("), "Missing map: {}", code);
        assert!(code.contains(">= 80"), "Missing filter condition: {}", code);
        assert!(code.contains("* 2"), "Missing map transform: {}", code);
    }

    #[test]
    fn test_realistic_codegen_cast_in_arithmetic() {
        // (total as float) / (count as float) — division with cast
        let mut gen = RustGenerator::new();
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Cast {
                expr: Box::new(Expression::Identifier("total".to_string())),
                target_type: TypeNode::Float,
            }),
            operator: BinaryOperator::Div,
            right: Box::new(Expression::Cast {
                expr: Box::new(Expression::Identifier("count".to_string())),
                target_type: TypeNode::Float,
            }),
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("total as f64"), "Missing left cast: {}", code);
        assert!(code.contains("count as f64"), "Missing right cast: {}", code);
        assert!(code.contains("/"), "Missing division: {}", code);
    }

    #[test]
    fn test_realistic_codegen_contract_with_impl() {
        // Contract + Agent implementing it — interface-first pattern
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Contract(ContractDef {
                    name: "Logger".to_string(),
                    is_public: true,
                    target_annotation: None,
                    methods: vec![
                        MethodDecl {
                            name: "Log".to_string(),
                            is_public: true, is_async: false,
                            annotations: vec![], type_params: vec![], constraints: vec![],
                            args: vec![FieldDecl { name: "msg".to_string(), ty: TypeNode::String, default_value: None }],
                            return_ty: Some(TypeNode::Void),
                            body: None,
                        },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "FileLogger".to_string(),
                    is_system: false, is_public: true,
                    target_annotation: None, annotations: vec![],
                    implements: vec!["Logger".to_string()],
                    fields: vec![],
                    methods: vec![
                        MethodDecl {
                            name: "Log".to_string(),
                            is_public: true, is_async: false,
                            annotations: vec![], type_params: vec![], constraints: vec![],
                            args: vec![FieldDecl { name: "msg".to_string(), ty: TypeNode::String, default_value: None }],
                            return_ty: Some(TypeNode::Void),
                            body: Some(Block { statements: vec![
                                Statement::Print(Expression::Identifier("msg".to_string())),
                            ]}),
                        },
                    ],
                }),
            ],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("trait Logger"), "Missing trait: {}", code);
        assert!(code.contains("impl Logger for FileLogger"), "Missing impl: {}", code);
    }

    #[test]
    fn test_realistic_codegen_interpolated_string_with_method() {
        // $"Processed {items.len()} items in {elapsed}ms"
        let mut gen = RustGenerator::new();
        let expr = Expression::InterpolatedString(vec![
            InterpolationPart::Literal("Processed ".to_string()),
            InterpolationPart::Expression(Expression::MethodCall {
                caller: Box::new(Expression::Identifier("items".to_string())),
                method_name: "len".to_string(),
                args: vec![],
            }),
            InterpolationPart::Literal(" items in ".to_string()),
            InterpolationPart::Expression(Expression::Identifier("elapsed".to_string())),
            InterpolationPart::Literal("ms".to_string()),
        ]);
        let code = gen.gen_expression(&expr);
        assert!(code.contains("format!"), "Missing format!: {}", code);
        assert!(code.contains("items.len()"), "Missing items.len(): {}", code);
        assert!(code.contains("Processed"), "Missing literal part: {}", code);
    }

    #[test]
    fn test_realistic_codegen_enum_and_match() {
        // Enum definition + match with variant destructuring
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Enum(EnumDef {
                    name: "Status".to_string(),
                    is_public: true,
                    variants: vec![
                        EnumVariant { name: "Active".to_string(), fields: vec![] },
                        EnumVariant { name: "Error".to_string(), fields: vec![("msg".to_string(), TypeNode::String)] },
                    ],
                }),
            ],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("enum Status"), "Missing enum: {}", code);
        assert!(code.contains("Active"), "Missing Active variant: {}", code);
        assert!(code.contains("Error { msg: String }"), "Missing Error variant: {}", code);
    }

    #[test]
    fn test_realistic_codegen_standalone_fn_with_loop() {
        // Standalone function: fibonacci with while loop
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Function(FunctionDef {
                name: "fibonacci".to_string(),
                is_public: false,
                params: vec![FieldDecl { name: "n".to_string(), ty: TypeNode::Int, default_value: None }],
                return_ty: Some(TypeNode::Int),
                body: Block { statements: vec![
                    Statement::Let { name: "a".to_string(), ty: Some(TypeNode::Int), value: Expression::Int(0) },
                    Statement::Let { name: "b".to_string(), ty: Some(TypeNode::Int), value: Expression::Int(1) },
                    Statement::Let { name: "i".to_string(), ty: Some(TypeNode::Int), value: Expression::Int(0) },
                    Statement::While {
                        condition: Expression::BinaryOp {
                            left: Box::new(Expression::Identifier("i".to_string())),
                            operator: BinaryOperator::Lt,
                            right: Box::new(Expression::Identifier("n".to_string())),
                        },
                        body: Block { statements: vec![
                            Statement::Let { name: "temp".to_string(), ty: None, value: Expression::Identifier("b".to_string()) },
                            Statement::Assign {
                                name: "b".to_string(),
                                value: Expression::BinaryOp {
                                    left: Box::new(Expression::Identifier("a".to_string())),
                                    operator: BinaryOperator::Add,
                                    right: Box::new(Expression::Identifier("b".to_string())),
                                },
                            },
                            Statement::Assign { name: "a".to_string(), value: Expression::Identifier("temp".to_string()) },
                            Statement::Assign {
                                name: "i".to_string(),
                                value: Expression::BinaryOp {
                                    left: Box::new(Expression::Identifier("i".to_string())),
                                    operator: BinaryOperator::Add,
                                    right: Box::new(Expression::Int(1)),
                                },
                            },
                        ]},
                    },
                    Statement::Return(Some(Expression::Identifier("a".to_string()))),
                ]},
            })],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("fn fibonacci(n: i64) -> i64"), "Missing fn signature: {}", code);
        assert!(code.contains("while i < n"), "Missing while: {}", code);
        assert!(code.contains("return a"), "Missing return: {}", code);
    }

    #[test]
    fn test_realistic_codegen_for_in_range() {
        // foreach i in 0..100 { sum += i }
        let mut gen = RustGenerator::new();
        let block = Block { statements: vec![
            Statement::Let { name: "sum".to_string(), ty: Some(TypeNode::Int), value: Expression::Int(0) },
            Statement::Foreach {
                item_name: "i".to_string(),
                value_name: None,
                collection: Expression::Range {
                    start: Box::new(Expression::Int(0)),
                    end: Box::new(Expression::Int(100)),
                    inclusive: false,
                },
                body: Block { statements: vec![
                    Statement::Assign {
                        name: "sum".to_string(),
                        value: Expression::BinaryOp {
                            left: Box::new(Expression::Identifier("sum".to_string())),
                            operator: BinaryOperator::Add,
                            right: Box::new(Expression::Identifier("i".to_string())),
                        },
                    },
                ]},
            },
        ]};
        let code = gen.gen_block(&block, 1);
        assert!(code.contains("for mut i in (0..100)"), "Missing for-in range: {}", code);
        assert!(code.contains("sum += i") || code.contains("sum = sum + i"), "Missing sum accumulation: {}", code);
    }

    // ===== End-to-End Integration Tests: Parse → TypeCheck → Codegen =====

    fn e2e(source: &str) -> String {
        use varg_parser::Parser;
        use varg_typechecker::TypeChecker;
        let mut parser = Parser::new(source);
        let ast = parser.parse_program().expect("Parse failed");
        let mut checker = TypeChecker::new();
        checker.check_program(&ast).map_err(|errs| format!("TypeCheck failed: {:?}", errs)).expect("TypeCheck failed");
        let mut gen = RustGenerator::new();
        gen.generate(&ast)
    }

    #[test]
    fn test_e2e_fibonacci_agent() {
        let code = e2e(r#"
            fn fibonacci(int n) -> int {
                int a = 0;
                int b = 1;
                int i = 0;
                while i < n {
                    int temp = b;
                    b = a + b;
                    a = temp;
                    i += 1;
                }
                return a;
            }

            agent FibDemo {
                public void Run() {
                    var result = fibonacci(10);
                    print $"fib(10) = {result}";
                }
            }
        "#);
        assert!(code.contains("fn fibonacci(n: i64) -> i64"), "Bad fn signature: {}", code);
        assert!(code.contains("struct FibDemo"), "Missing agent struct: {}", code);
        assert!(code.contains("fibonacci("), "Missing fn call: {}", code);
    }

    #[test]
    fn test_e2e_contract_agent_implementation() {
        let code = e2e(r#"
            contract Greeter {
                string Greet(string name);
            }

            agent FriendlyBot : Greeter {
                int greet_count;

                public void Init() {
                    greet_count = 0;
                }

                public string Greet(string name) {
                    greet_count += 1;
                    return $"Hello {name}! (#{greet_count})";
                }

                public void Run() {
                    Init();
                    var msg = Greet("Alice");
                    print msg;
                }
            }
        "#);
        assert!(code.contains("trait Greeter"), "Missing trait");
        assert!(code.contains("impl Greeter for FriendlyBot"), "Missing impl");
        assert!(code.contains("greet_count: i64"), "Missing field");
    }

    #[test]
    fn test_e2e_enum_and_match() {
        let code = e2e(r#"
            enum Shape {
                Circle(int radius),
                Rectangle(int width, int height),
            }

            agent Geometry {
                public void Run() {
                    var shape = "circle";
                    match shape {
                        Circle(r) => {
                            print $"Circle: r={r}";
                        }
                        Rectangle(w, h) => {
                            print $"Rect: {w}x{h}";
                        }
                    }
                }
            }
        "#);
        assert!(code.contains("enum Shape"), "Missing enum");
        assert!(code.contains("Circle { radius: i64 }"), "Missing Circle variant: {}", code);
        assert!(code.contains("match"), "Missing match");
    }

    #[test]
    fn test_e2e_generic_struct() {
        let code = e2e(r#"
            struct Pair<K, V> {
                K key;
                V value;
            }

            agent Config {
                public void Run() {
                    print "config ready";
                }
            }
        "#);
        assert!(code.contains("struct Pair<K, V>"), "Missing generic struct: {}", code);
        assert!(code.contains("key: K"), "Missing key field: {}", code);
        assert!(code.contains("value: V"), "Missing value field: {}", code);
    }

    #[test]
    fn test_e2e_iterator_chain() {
        let code = e2e(r#"
            agent Analytics {
                public void Run() {
                    var scores = [85, 42, 97, 31, 78];
                    var high = scores.filter((int s) => s >= 80);
                    print $"High: {high.len()}";
                }
            }
        "#);
        assert!(code.contains(".filter("), "Missing filter: {}", code);
        assert!(code.contains(">= 80"), "Missing predicate: {}", code);
    }

    #[test]
    fn test_e2e_try_catch_error_recovery() {
        let code = e2e(r#"
            agent SafeRunner {
                public void Run() {
                    try {
                        print "risky operation";
                    } catch(err) {
                        print $"Error: {err}";
                    }
                }
            }
        "#);
        assert!(code.contains("Result<(), String>"), "Missing try result type");
        assert!(code.contains("Err(mut err)"), "Missing catch binding");
    }

    #[test]
    fn test_e2e_async_agent() {
        let code = e2e(r#"
            agent Fetcher {
                async public string Process(string input) {
                    return $"processed: {input}";
                }

                async public void Run() {
                    var result = await Process("data");
                    print result;
                }
            }
        "#);
        assert!(code.contains("async fn"), "Missing async fn: {}", code);
        assert!(code.contains(".await"), "Missing .await: {}", code);
    }

    #[test]
    fn test_e2e_type_casting() {
        let code = e2e(r#"
            agent Converter {
                public void Run() {
                    int total = 100;
                    int count = 3;
                    var avg = (total as float) / (count as float);
                    var label = total as string;
                    print $"Avg: {avg}, Label: {label}";
                }
            }
        "#);
        assert!(code.contains("as f64"), "Missing float cast: {}", code);
        assert!(code.contains("format!"), "Missing string cast: {}", code);
    }

    #[test]
    fn test_e2e_if_expression() {
        let code = e2e(r#"
            agent Classifier {
                public void Run() {
                    int score = 85;
                    var grade = if score >= 90 { "A" } else { "B" };
                    print $"Grade: {grade}";
                }
            }
        "#);
        assert!(code.contains("if score >= 90"), "Missing if-expr condition: {}", code);
        assert!(code.contains("else"), "Missing else: {}", code);
    }

    #[test]
    fn test_e2e_match_with_guard() {
        let code = e2e(r#"
            enum HttpResult {
                Success(string value),
                Failure(int code),
            }

            agent Handler {
                public void Run() {
                    var response = "ok";
                    match response {
                        Success(val) => {
                            print val;
                        }
                        Failure(code) if code >= 500 => {
                            print "server error";
                        }
                        _ => {
                            print "other";
                        }
                    }
                }
            }
        "#);
        assert!(code.contains("if code >= 500"), "Missing match guard: {}", code);
    }

    #[test]
    fn test_e2e_for_in_range_accumulator() {
        let code = e2e(r#"
            agent Summer {
                public void Run() {
                    int sum = 0;
                    foreach i in 0..1000 {
                        sum += i;
                    }
                    print $"Sum: {sum}";
                }
            }
        "#);
        assert!(code.contains("for mut i in (0..1000)"), "Missing range loop: {}", code);
        assert!(code.contains("sum += i") || code.contains("sum = sum + i"), "Missing accumulator: {}", code);
    }

    #[test]
    fn test_e2e_complex_interpolation() {
        let code = e2e(r#"
            agent Logger {
                int count;

                public void Run() {
                    count = 0;
                    var items = [1, 2, 3];
                    count += 1;
                    print $"[{count}] Processing {items.len()} items";
                }
            }
        "#);
        assert!(code.contains("format!"), "Missing format!: {}", code);
        assert!(code.contains("self.count"), "Missing self.count: {}", code);
        assert!(code.contains("items.len()"), "Missing items.len(): {}", code);
    }

    #[test]
    fn test_e2e_multiple_standalone_functions() {
        let code = e2e(r#"
            fn add(int a, int b) -> int {
                return a + b;
            }

            fn multiply(int a, int b) -> int {
                return a * b;
            }

            agent Math {
                public void Run() {
                    var sum = add(3, 4);
                    var product = multiply(sum, 2);
                    print $"Result: {product}";
                }
            }
        "#);
        assert!(code.contains("fn add(a: i64, b: i64) -> i64"), "Missing add fn: {}", code);
        assert!(code.contains("fn multiply(a: i64, b: i64) -> i64"), "Missing multiply fn: {}", code);
    }

    #[test]
    fn test_e2e_default_params() {
        let code = e2e(r#"
            agent Server {
                public string Respond(string body, int status = 200) {
                    return $"HTTP {status}: {body}";
                }

                public void Run() {
                    var ok = Respond("success");
                    var err = Respond("not found", 404);
                }
            }
        "#);
        // Default params generate code that works
        assert!(code.contains("struct Server"), "Missing agent: {}", code);
    }

    // ===== Wave 12: Struct Literal Tests =====

    #[test]
    fn test_e2e_struct_literal() {
        let code = e2e(r#"
            struct Point {
                int x;
                int y;
            }

            agent Geometry {
                public void Run() {
                    var p = Point { x: 10, y: 20 };
                    print $"Point: ({p.x}, {p.y})";
                }
            }
        "#);
        assert!(code.contains("Point { x: 10, y: 20 }"), "Missing struct literal: {}", code);
    }

    #[test]
    fn test_e2e_struct_literal_with_expressions() {
        let code = e2e(r#"
            struct Config {
                string name;
                int max_retries;
            }

            agent App {
                public void Run() {
                    int r = 3;
                    var cfg = Config { name: "prod", max_retries: r };
                    print cfg.name;
                }
            }
        "#);
        assert!(code.contains("Config { name:"), "Missing struct literal: {}", code);
    }

    // ===== Wave 12: Enum Construction Tests =====

    #[test]
    fn test_e2e_enum_construction_qualified() {
        let code = e2e(r#"
            enum Color {
                Red,
                Rgb(int r, int g, int b),
            }

            agent Painter {
                public void Run() {
                    var c = Color::Red;
                    var c2 = Color::Rgb(255, 128, 0);
                    print "painted";
                }
            }
        "#);
        assert!(code.contains("Color::Red"), "Missing Color::Red: {}", code);
        assert!(code.contains("Color::Rgb"), "Missing Color::Rgb: {}", code);
    }

    #[test]
    fn test_e2e_ok_err_construction() {
        let code = e2e(r#"
            agent Worker {
                public void Run() {
                    var success = Ok("done");
                    var failure = Err("oops");
                    print "results ready";
                }
            }
        "#);
        assert!(code.contains("Ok("), "Missing Ok(): {}", code);
        assert!(code.contains("Err("), "Missing Err(): {}", code);
    }

    #[test]
    fn test_codegen_struct_literal_direct() {
        let mut gen = RustGenerator::new();
        let expr = Expression::StructLiteral {
            type_name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Expression::Int(5)),
                ("y".to_string(), Expression::Int(10)),
            ],
        };
        let code = gen.gen_expression(&expr);
        assert_eq!(code, "Point { x: 5, y: 10 }");
    }

    #[test]
    fn test_codegen_enum_construct_bare() {
        let mut gen = RustGenerator::new();
        let expr = Expression::EnumConstruct {
            enum_name: String::new(),
            variant_name: "Ok".to_string(),
            args: vec![Expression::String("done".to_string())],
        };
        let code = gen.gen_expression(&expr);
        assert!(code.contains("Ok("), "Bare Ok: {}", code);
    }

    #[test]
    fn test_codegen_enum_construct_qualified_named_fields() {
        let mut gen = RustGenerator::new();
        // Register the enum
        gen.known_enums.insert("Shape".to_string(), vec![
            EnumVariant { name: "Circle".to_string(), fields: vec![("radius".to_string(), TypeNode::Int)] },
        ]);
        let expr = Expression::EnumConstruct {
            enum_name: "Shape".to_string(),
            variant_name: "Circle".to_string(),
            args: vec![Expression::Int(42)],
        };
        let code = gen.gen_expression(&expr);
        assert_eq!(code, "Shape::Circle { radius: 42 }");
    }

    // ===== Wave 13: impl Blocks for Structs =====

    #[test]
    fn test_codegen_impl_block_basic() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Struct(StructDef {
                    name: "Point".to_string(),
                    is_public: false,
                    type_params: vec![],
                    fields: vec![
                        FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None },
                        FieldDecl { name: "y".to_string(), ty: TypeNode::Int, default_value: None },
                    ],
                }),
                Item::Impl {
                    type_name: "Point".to_string(),
                    type_params: vec![],
                    methods: vec![MethodDecl {
                        name: "sum".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Int),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::Int(0)))
                        ]}),
                    }],
                },
            ],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("impl Point {"), "Should contain impl block");
        assert!(code.contains("pub fn sum(&mut self) -> i64"), "Should contain method signature");
    }

    #[test]
    fn test_codegen_impl_block_with_type_params() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Struct(StructDef {
                    name: "Box".to_string(),
                    is_public: false,
                    type_params: vec!["T".to_string()],
                    fields: vec![],
                }),
                Item::Impl {
                    type_name: "Box".to_string(),
                    type_params: vec!["T".to_string()],
                    methods: vec![MethodDecl {
                        name: "unwrap".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![] }),
                    }],
                },
            ],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("impl<T> Box {"), "Should contain generic impl block: {}", code);
    }

    // ===== Wave 13: Stdlib Expansion Codegen Tests =====

    #[test]
    fn test_source_map_with_filename() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "App".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                implements: vec![], fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![Statement::Return(None)] }),
                }],
            })],
        };
        let mut gen = RustGenerator::new();
        gen.set_current_file("main.varg");
        let code = gen.generate_with_source_map(&program, "");
        assert!(code.contains("// main.varg:"), "Should contain filename in source map: {}", code);
    }

    #[test]
    fn test_codegen_fs_read_returns_result() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_read".to_string(),
            args: vec![Expression::String("test.txt".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("std::fs::read_to_string"), "fs_read should use std::fs::read_to_string: {}", code);
        // Wave 14: Must use map_err instead of unwrap
        assert!(code.contains("map_err"), "fs_read should return Result via map_err: {}", code);
        assert!(!code.contains("unwrap"), "fs_read must NOT use unwrap: {}", code);
    }

    #[test]
    fn test_codegen_path_exists() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "path_exists".to_string(),
            args: vec![Expression::String("/tmp".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("Path::new"), "path_exists should use Path::new: {}", code);
        assert!(code.contains(".exists()"), "path_exists should call .exists(): {}", code);
    }

    #[test]
    fn test_codegen_regex_match_returns_result() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "regex_match".to_string(),
            args: vec![Expression::String("\\d+".to_string()), Expression::String("abc".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("regex::Regex::new"), "regex_match should use regex crate: {}", code);
        assert!(code.contains("is_match"), "regex_match should call is_match: {}", code);
        // Wave 14: Must use map/map_err instead of unwrap
        assert!(code.contains("map_err"), "regex_match should return Result via map_err: {}", code);
        assert!(!code.contains("unwrap"), "regex_match must NOT use unwrap: {}", code);
    }

    #[test]
    fn test_codegen_sleep() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "sleep".to_string(),
            args: vec![Expression::Int(500)],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("std::thread::sleep"), "sleep should use std::thread::sleep: {}", code);
        assert!(code.contains("Duration::from_millis"), "sleep should use Duration::from_millis: {}", code);
    }

    #[test]
    fn test_codegen_timestamp() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "timestamp".to_string(),
            args: vec![],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("chrono::Local::now()"), "timestamp should use chrono: {}", code);
    }

    // ===== Wave 13: Ownership/Borrowing Improvement Tests =====

    #[test]
    fn test_last_use_no_clone() {
        // When a variable is used only once as a method arg, it should not be cloned
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "App".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                implements: vec![], fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Let {
                            name: "msg".to_string(),
                            ty: Some(TypeNode::String),
                            value: Expression::String("hello".to_string()),
                        },
                        // Only one use of msg — should be moved, not cloned
                        Statement::Expr(Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: "println".to_string(),
                            args: vec![Expression::Identifier("msg".to_string())],
                        }),
                    ]}),
                }],
            })],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        // msg is used only once — should NOT have .clone()
        // Note: the identifier "msg" appears as a method arg via gen_cloned_arg
        assert!(!code.contains("msg.clone()"), "Single-use variable should not be cloned: {}", code);
    }

    #[test]
    fn test_self_field_always_cloned() {
        // Self fields must always be cloned (can't move out of &mut self)
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "App".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                implements: vec![],
                fields: vec![FieldDecl { name: "data".to_string(), ty: TypeNode::String, default_value: None }],
                methods: vec![MethodDecl {
                    name: "Run".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Let {
                            name: "x".to_string(),
                            ty: None,
                            value: Expression::Identifier("data".to_string()),
                        },
                    ]}),
                }],
            })],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        // data is a self field — should always clone
        assert!(code.contains("self.data.clone()"), "Self fields should always be cloned: {}", code);
    }

    #[test]
    fn test_usage_counting() {
        let gen = RustGenerator::new();
        let block = Block {
            statements: vec![
                Statement::Expr(Expression::Identifier("x".to_string())),
                Statement::Expr(Expression::Identifier("x".to_string())),
                Statement::Expr(Expression::Identifier("y".to_string())),
            ],
        };
        let counts = gen.count_usages_in_block(&block);
        assert_eq!(counts.get("x"), Some(&2));
        assert_eq!(counts.get("y"), Some(&1));
    }

    // ===== Wave 14: Auto-Result-Wrapping Tests =====

    #[test]
    fn test_function_with_try_propagate_gets_result_return() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Function(FunctionDef {
                name: "load_config".to_string(),
                is_public: false,
                params: vec![FieldDecl { name: "path".to_string(), ty: TypeNode::String, default_value: None }],
                return_ty: Some(TypeNode::String),
                body: Block {
                    statements: vec![
                        Statement::Return(Some(Expression::TryPropagate(
                            Box::new(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "fs_read".to_string(),
                                args: vec![Expression::Identifier("path".to_string())],
                            })
                        ))),
                    ],
                },
            })],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("-> Result<String, String>"), "Function with ? should return Result: {}", code);
        assert!(code.contains("return Ok("), "Return should be wrapped in Ok(): {}", code);
    }

    #[test]
    fn test_function_without_try_propagate_normal_return() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Function(FunctionDef {
                name: "add".to_string(),
                is_public: false,
                params: vec![
                    FieldDecl { name: "a".to_string(), ty: TypeNode::Int, default_value: None },
                    FieldDecl { name: "b".to_string(), ty: TypeNode::Int, default_value: None },
                ],
                return_ty: Some(TypeNode::Int),
                body: Block {
                    statements: vec![
                        Statement::Return(Some(Expression::BinaryOp {
                            left: Box::new(Expression::Identifier("a".to_string())),
                            operator: BinaryOperator::Add,
                            right: Box::new(Expression::Identifier("b".to_string())),
                        })),
                    ],
                },
            })],
        };
        let mut gen = RustGenerator::new();
        let code = gen.generate(&program);
        assert!(code.contains("-> i64"), "Function without ? should have normal return: {}", code);
        assert!(!code.contains("Result<"), "Function without ? should NOT return Result: {}", code);
    }

    #[test]
    fn test_block_contains_try_propagate_detection() {
        // Block with ? operator
        let block_with_try = Block {
            statements: vec![
                Statement::Let {
                    name: "data".to_string(),
                    ty: None,
                    value: Expression::TryPropagate(
                        Box::new(Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: "fs_read".to_string(),
                            args: vec![Expression::String("test.txt".to_string())],
                        })
                    ),
                },
            ],
        };
        assert!(block_contains_try_propagate(&block_with_try));

        // Block without ? operator
        let block_without = Block {
            statements: vec![
                Statement::Let {
                    name: "x".to_string(),
                    ty: None,
                    value: Expression::Int(42),
                },
            ],
        };
        assert!(!block_contains_try_propagate(&block_without));
    }

    #[test]
    fn test_fs_write_result_based() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_write".to_string(),
            args: vec![Expression::String("out.txt".to_string()), Expression::String("data".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("map_err"), "fs_write should return Result via map_err: {}", code);
        assert!(!code.contains("unwrap"), "fs_write must NOT use unwrap: {}", code);
    }

    // ===== Wave 15: fs_append + fs_read_lines =====

    #[test]
    fn test_codegen_fs_append() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_append".to_string(),
            args: vec![Expression::String("log.txt".to_string()), Expression::String("data\n".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("OpenOptions"), "fs_append should use OpenOptions: {}", code);
        assert!(code.contains("append(true)"), "fs_append should set append mode: {}", code);
        assert!(code.contains("create(true)"), "fs_append should create if missing: {}", code);
        assert!(code.contains("map_err"), "fs_append should return Result: {}", code);
    }

    #[test]
    fn test_codegen_fs_read_lines() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_read_lines".to_string(),
            args: vec![Expression::String("data.txt".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("read_to_string"), "fs_read_lines should use read_to_string: {}", code);
        assert!(code.contains("lines()"), "fs_read_lines should split by lines: {}", code);
        assert!(code.contains("Vec<String>"), "fs_read_lines should collect to Vec<String>: {}", code);
        assert!(code.contains("map_err"), "fs_read_lines should return Result: {}", code);
    }

    #[test]
    fn test_codegen_exec() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "exec".to_string(),
            args: vec![Expression::String("echo hello".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("Command::new"), "exec should use Command::new: {}", code);
        assert!(code.contains("target_os"), "exec should have platform switch: {}", code);
        assert!(code.contains("map_err"), "exec should return Result: {}", code);
        assert!(code.contains("stdout"), "exec should capture stdout: {}", code);
    }

    #[test]
    fn test_codegen_exec_status() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "exec_status".to_string(),
            args: vec![Expression::String("ls".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("Command::new"), "exec_status should use Command::new: {}", code);
        assert!(code.contains("status()"), "exec_status should use .status(): {}", code);
        assert!(code.contains("code()"), "exec_status should extract exit code: {}", code);
    }

    // ===== Wave 15: Test Framework — assert builtins =====

    #[test]
    fn test_codegen_assert() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "assert".to_string(),
            args: vec![Expression::Bool(true), Expression::String("should pass".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("panic!"), "assert should generate panic!: {}", code);
        assert!(code.contains("Assertion failed"), "assert should have failure message: {}", code);
    }

    #[test]
    fn test_codegen_assert_eq() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "assert_eq".to_string(),
            args: vec![Expression::Int(5), Expression::Int(5), Expression::String("should match".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("panic!"), "assert_eq should generate panic!: {}", code);
        assert!(code.contains("assert_eq failed"), "assert_eq should have failure message: {}", code);
    }

    // ===== F41-7: Extended Assertion Tests =====

    #[test]
    fn test_codegen_assert_ne() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "assert_ne".to_string(),
            args: vec![Expression::Int(1), Expression::Int(2), Expression::String("should differ".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("assert_ne failed"), "assert_ne should have failure message: {}", code);
        assert!(code.contains("=="), "assert_ne should check equality: {}", code);
    }

    #[test]
    fn test_codegen_assert_true() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "assert_true".to_string(),
            args: vec![Expression::Bool(true), Expression::String("should be true".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("assert_true failed"), "assert_true should have failure message: {}", code);
    }

    #[test]
    fn test_codegen_assert_false() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "assert_false".to_string(),
            args: vec![Expression::Bool(false), Expression::String("should be false".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("assert_false failed"), "assert_false should have failure message: {}", code);
    }

    #[test]
    fn test_codegen_assert_contains() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "assert_contains".to_string(),
            args: vec![Expression::String("hello world".to_string()), Expression::String("world".to_string()), Expression::String("should contain".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("assert_contains failed"), "assert_contains should have failure message: {}", code);
    }

    #[test]
    fn test_codegen_assert_throws() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "assert_throws".to_string(),
            args: vec![
                Expression::Lambda {
                    params: vec![],
                    return_ty: None,
                    body: Box::new(LambdaBody::Expression(Expression::Int(1))),
                },
                Expression::String("should throw".to_string()),
            ],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("catch_unwind"), "assert_throws should use catch_unwind: {}", code);
        assert!(code.contains("assert_throws"), "assert_throws should have failure message: {}", code);
    }

    // ===== Wave 15: Typed JSON =====

    #[test]
    fn test_codegen_json_parse() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "json_parse".to_string(),
            args: vec![Expression::String("{\"key\": \"value\"}".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("serde_json::from_str"), "json_parse should use serde_json::from_str: {}", code);
        assert!(code.contains("serde_json::Value"), "json_parse should parse to serde_json::Value: {}", code);
        assert!(code.contains("map_err"), "json_parse should return Result: {}", code);
    }

    #[test]
    fn test_codegen_json_get() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "json_get".to_string(),
            args: vec![Expression::Identifier("json".to_string()), Expression::String("/user/name".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("pointer"), "json_get should use JSON Pointer: {}", code);
        assert!(code.contains("as_str"), "json_get should extract as string: {}", code);
    }

    #[test]
    fn test_codegen_json_get_int() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "json_get_int".to_string(),
            args: vec![Expression::Identifier("json".to_string()), Expression::String("/age".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("pointer"), "json_get_int should use JSON Pointer: {}", code);
        assert!(code.contains("as_i64"), "json_get_int should extract as i64: {}", code);
    }

    #[test]
    fn test_codegen_json_get_array() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "json_get_array".to_string(),
            args: vec![Expression::Identifier("json".to_string()), Expression::String("/tags".to_string())],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("pointer"), "json_get_array should use JSON Pointer: {}", code);
        assert!(code.contains("as_array"), "json_get_array should extract as array: {}", code);
        assert!(code.contains("Vec<String>"), "json_get_array should produce Vec<String>: {}", code);
    }

    #[test]
    fn test_codegen_json_value_type() {
        let mut gen = RustGenerator::new();
        let ty = gen.gen_type(&TypeNode::JsonValue);
        assert_eq!(ty, "serde_json::Value");
    }

    // ===== Wave 15: HTTP Response =====

    #[test]
    fn test_codegen_http_request() {
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "http_request".to_string(),
            args: vec![
                Expression::String("https://api.example.com".to_string()),
                Expression::String("GET".to_string()),
            ],
        };
        let mut gen = RustGenerator::new();
        let code = gen.gen_expression(&expr);
        assert!(code.contains("__varg_http_request"), "http_request should call __varg_http_request: {}", code);
    }

    // ===== Wave 14: E2E Integration Tests (Parse → TypeCheck → CodeGen) =====

    /// Helper: Full pipeline from Varg source to Rust code
    fn e2e_compile(source: &str) -> String {
        use varg_parser::Parser;
        use varg_typechecker::TypeChecker;

        let mut parser = Parser::new(source);
        let ast = parser.parse_program().expect("Parse should succeed");
        let mut checker = TypeChecker::new();
        checker.check_program(&ast).expect("TypeCheck should succeed");
        let mut gen = RustGenerator::new();
        gen.generate(&ast)
    }

    #[test]
    fn test_e2e_agent_lifecycle() {
        let code = e2e_compile(r#"
            agent Calculator {
                int result;
                public void Init() {
                    result = 0;
                }
                public int Add(int a, int b) {
                    return a + b;
                }
            }
        "#);
        assert!(code.contains("struct Calculator"), "Should generate struct: {}", code);
        assert!(code.contains("fn new()"), "Should generate constructor: {}", code);
        assert!(code.contains("fn Init("), "Should generate Init: {}", code);
        assert!(code.contains("fn Add("), "Should generate Add: {}", code);
        assert!(code.contains("a + b"), "Should generate addition: {}", code);
    }

    #[test]
    fn test_e2e_standalone_fn_with_agent() {
        let code = e2e_compile(r#"
            fn helper(int x) -> int {
                return x * 2;
            }
            agent App {
                public void Run() {
                    var val = helper(21);
                    print val;
                }
            }
        "#);
        assert!(code.contains("fn helper(x: i64) -> i64"), "Should generate standalone fn: {}", code);
        assert!(code.contains("helper(21"), "Should call standalone fn from agent: {}", code);
    }

    #[test]
    fn test_e2e_struct_with_impl() {
        let code = e2e_compile(r#"
            struct Point {
                int x;
                int y;
            }
            impl Point {
                public fn sum() -> int {
                    return x + y;
                }
            }
        "#);
        assert!(code.contains("struct Point"), "Should generate struct: {}", code);
        assert!(code.contains("impl Point"), "Should generate impl block: {}", code);
        assert!(code.contains("fn sum("), "Should generate method: {}", code);
    }

    #[test]
    fn test_e2e_try_catch() {
        let code = e2e_compile(r#"
            agent App {
                public void Run() {
                    try {
                        print "risky";
                    } catch (err) {
                        print err;
                    }
                }
            }
        "#);
        assert!(code.contains("'varg_try"), "Should generate labeled block: {}", code);
        assert!(code.contains("Err(mut err)"), "Should generate catch binding: {}", code);
    }

    #[test]
    fn test_e2e_or_operator_with_default() {
        let code = e2e_compile(r#"
            agent App {
                public void Run() {
                    var name = "test" or "default";
                }
            }
        "#);
        assert!(code.contains("unwrap_or_else"), "Should generate or fallback: {}", code);
    }

    #[test]
    fn test_e2e_match_int() {
        let code = e2e_compile(r#"
            agent App {
                public void Run() {
                    var x = 42;
                    match x {
                        1 => { print "one"; }
                        2 => { print "two"; }
                        _ => { print "other"; }
                    }
                }
            }
        "#);
        assert!(code.contains("match"), "Should generate match: {}", code);
    }

    #[test]
    fn test_e2e_string_interpolation() {
        let code = e2e_compile(r#"
            agent App {
                public void Run() {
                    var name = "World";
                    var msg = $"Hello {name}!";
                    print msg;
                }
            }
        "#);
        assert!(code.contains("format!"), "Should generate format! for interpolation: {}", code);
    }

    #[test]
    fn test_e2e_for_range_loop() {
        let code = e2e_compile(r#"
            agent App {
                public void Run() {
                    for i in 0..10 {
                        print i;
                    }
                }
            }
        "#);
        assert!(code.contains("0..10") || code.contains("0 ..10") || code.contains("for"), "Should generate range loop: {}", code);
    }

    #[test]
    fn test_e2e_contract_with_agent() {
        let code = e2e_compile(r#"
            contract Greeter {
                public string Greet(string name);
            }
            agent Bot : Greeter {
                public string Greet(string name) {
                    return $"Hello {name}";
                }
            }
        "#);
        assert!(code.contains("trait Greeter"), "Should generate trait: {}", code);
        assert!(code.contains("impl Greeter for Bot"), "Should generate trait impl: {}", code);
    }

    #[test]
    fn test_e2e_async_agent_method_pipeline() {
        let code = e2e_compile(r#"
            agent Fetcher {
                async public string FetchData(string url, NetworkAccess net) {
                    return "data";
                }
            }
        "#);
        assert!(code.contains("async fn FetchData"), "Should generate async method: {}", code);
    }

    #[test]
    fn test_e2e_result_function_with_try_propagate() {
        let code = e2e_compile(r#"
            fn load_data(string path, FileAccess fa) -> string {
                var content = fs_read(path)?;
                return content;
            }
        "#);
        assert!(code.contains("Result<String, String>"), "Function with ? should return Result: {}", code);
        assert!(code.contains("return Ok("), "Return should be Ok-wrapped: {}", code);
    }

    // ===== Wave 16: for (k, v) in map =====

    #[test]
    fn test_e2e_for_kv_in_map() {
        let code = e2e_compile(r#"
            agent Config {
                map<string, int> settings;

                public void PrintAll() {
                    for (key, value) in self.settings {
                        print $"{key}: {value}";
                    }
                }
            }
        "#);
        assert!(code.contains("for (mut key, mut value) in"), "Should generate tuple destructure: {}", code);
    }

    // ===== Wave 16: HashSet =====

    #[test]
    fn test_e2e_set_of_and_add() {
        let code = e2e_compile(r#"
            fn main() {
                var tags = set_of("rust", "varg");
                tags.add("ai");
                print tags.len();
                print tags.contains("varg");
            }
        "#);
        assert!(code.contains("HashSet"), "Should use HashSet: {}", code);
        assert!(code.contains(".insert("), "add should become insert: {}", code);
    }

    // ===== Wave 16: Date/Time =====

    #[test]
    fn test_e2e_time_builtins() {
        let code = e2e_compile(r#"
            fn main() {
                var now = time_millis();
                var formatted = time_format(now, "%Y-%m-%d %H:%M:%S");
                var later = time_add(now, 60000);
                var delta = time_diff(later, now);
                print formatted;
            }
        "#);
        assert!(code.contains("SystemTime::now()"), "time_millis: {}", code);
        assert!(code.contains("from_timestamp_millis"), "time_format: {}", code);
        assert!(code.contains("60000"), "time_add: {}", code);
    }

    // ===== Wave 16: Logging =====

    #[test]
    fn test_e2e_logging_levels() {
        let code = e2e_compile(r#"
            fn main() {
                log_debug("starting up");
                log_info("processing request");
                log_warn("rate limit approaching");
                log_error("connection failed");
            }
        "#);
        assert!(code.contains("println!(\"[DEBUG]"), "log_debug: {}", code);
        assert!(code.contains("println!(\"[INFO]"), "log_info: {}", code);
        assert!(code.contains("eprintln!(\"[WARN]"), "log_warn: {}", code);
        assert!(code.contains("eprintln!(\"[ERROR]"), "log_error: {}", code);
    }
}
