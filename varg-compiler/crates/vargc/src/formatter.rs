/// Wave 13: Varg Source Formatter (AST → Varg Source)
/// Formats .varg source code with consistent style.

use varg_ast::ast::*;

pub struct VargFormatter {
    indent: usize,
    output: String,
}

impl VargFormatter {
    pub fn new() -> Self {
        Self { indent: 0, output: String::new() }
    }

    pub fn format_program(&mut self, program: &Program) -> String {
        self.output.clear();

        if program.no_std {
            self.output.push_str("#![no_std]\n\n");
        }

        for (i, item) in program.items.iter().enumerate() {
            // Add doc comment if available
            if let Some(name) = Self::item_name(item) {
                if let Some(doc) = program.docs.get(&name) {
                    for line in doc.lines() {
                        self.push_indent();
                        self.output.push_str(&format!("/// {}\n", line));
                    }
                }
            }
            self.format_item(item);
            if i + 1 < program.items.len() {
                self.output.push('\n');
            }
        }

        self.output.clone()
    }

    fn item_name(item: &Item) -> Option<String> {
        match item {
            Item::Agent(a) => Some(a.name.clone()),
            Item::Contract(c) => Some(c.name.clone()),
            Item::Struct(s) => Some(s.name.clone()),
            Item::Enum(e) => Some(e.name.clone()),
            Item::Function(f) => Some(f.name.clone()),
            _ => None,
        }
    }

    fn format_item(&mut self, item: &Item) {
        match item {
            Item::Import(name) => {
                self.push_indent();
                self.output.push_str(&format!("import {};\n", name));
            }
            Item::ImportDecl(decl) => {
                self.push_indent();
                match &decl.items {
                    ImportItems::All => self.output.push_str(&format!("import {};\n", decl.module_name)),
                    ImportItems::Single(name) => self.output.push_str(&format!("import {}.{};\n", decl.module_name, name)),
                    ImportItems::Selected(names) => self.output.push_str(&format!("import {}.{{{}}};\n", decl.module_name, names.join(", "))),
                }
            }
            Item::CrateImport { crate_name, version, features } => {
                self.push_indent();
                let mut s = format!("import crate {} = \"{}\"", crate_name, version);
                if !features.is_empty() {
                    s.push_str(&format!(" features [{}]", features.iter().map(|f| format!("\"{}\"", f)).collect::<Vec<_>>().join(", ")));
                }
                s.push_str(";\n");
                self.output.push_str(&s);
            }
            Item::UseExtern { path } => {
                self.push_indent();
                self.output.push_str(&format!("import {};\n", path.join("::")));
            }
            Item::TypeAlias { name, target } => {
                self.push_indent();
                self.output.push_str(&format!("type {} = {};\n", name, self.format_type(target)));
            }
            Item::Struct(s) => self.format_struct(s),
            Item::Enum(e) => self.format_enum(e),
            Item::Agent(a) => self.format_agent(a),
            Item::Contract(c) => self.format_contract(c),
            Item::Function(f) => self.format_function(f),
            Item::PromptTemplate(pt) => self.format_prompt_template(pt),
            Item::Impl { type_name, type_params, methods } => self.format_impl(type_name, type_params, methods),
        }
    }

    fn format_struct(&mut self, s: &StructDef) {
        self.push_indent();
        let vis = if s.is_public { "public " } else { "" };
        let tp = if s.type_params.is_empty() { "".to_string() } else { format!("<{}>", s.type_params.join(", ")) };
        self.output.push_str(&format!("{}struct {}{} {{\n", vis, s.name, tp));
        self.indent += 1;
        for field in &s.fields {
            self.push_indent();
            self.output.push_str(&format!("{} {};\n", self.format_type(&field.ty), field.name));
        }
        self.indent -= 1;
        self.push_indent();
        self.output.push_str("}\n");
    }

    fn format_enum(&mut self, e: &EnumDef) {
        self.push_indent();
        let vis = if e.is_public { "public " } else { "" };
        self.output.push_str(&format!("{}enum {} {{\n", vis, e.name));
        self.indent += 1;
        for variant in &e.variants {
            self.push_indent();
            if variant.fields.is_empty() {
                self.output.push_str(&format!("{},\n", variant.name));
            } else {
                let fields: Vec<String> = variant.fields.iter()
                    .map(|(name, ty)| format!("{} {}", self.format_type(ty), name))
                    .collect();
                self.output.push_str(&format!("{}({}),\n", variant.name, fields.join(", ")));
            }
        }
        self.indent -= 1;
        self.push_indent();
        self.output.push_str("}\n");
    }

    fn format_agent(&mut self, a: &AgentDef) {
        self.push_indent();
        let vis = if a.is_public { "public " } else { "" };
        let sys = if a.is_system { "system " } else { "" };
        let implements = if a.implements.is_empty() {
            "".to_string()
        } else {
            format!(" implements {}", a.implements.join(", "))
        };
        self.output.push_str(&format!("{}{}agent {}{} {{\n", vis, sys, a.name, implements));
        self.indent += 1;
        for field in &a.fields {
            self.push_indent();
            self.output.push_str(&format!("{} {};\n", self.format_type(&field.ty), field.name));
        }
        if !a.fields.is_empty() && !a.methods.is_empty() {
            self.output.push('\n');
        }
        for method in &a.methods {
            self.format_method(method);
        }
        self.indent -= 1;
        self.push_indent();
        self.output.push_str("}\n");
    }

    fn format_contract(&mut self, c: &ContractDef) {
        self.push_indent();
        let vis = if c.is_public { "public " } else { "" };
        self.output.push_str(&format!("{}contract {} {{\n", vis, c.name));
        self.indent += 1;
        for method in &c.methods {
            self.format_method(method);
        }
        self.indent -= 1;
        self.push_indent();
        self.output.push_str("}\n");
    }

    fn format_function(&mut self, f: &FunctionDef) {
        self.push_indent();
        let vis = if f.is_public { "pub " } else { "" };
        let params: Vec<String> = f.params.iter()
            .map(|p| format!("{} {}", self.format_type(&p.ty), p.name))
            .collect();
        let ret = f.return_ty.as_ref()
            .map(|t| format!(" -> {}", self.format_type(t)))
            .unwrap_or_default();
        self.output.push_str(&format!("{}fn {}({}){} {{\n", vis, f.name, params.join(", "), ret));
        self.indent += 1;
        self.format_block(&f.body);
        self.indent -= 1;
        self.push_indent();
        self.output.push_str("}\n");
    }

    fn format_prompt_template(&mut self, pt: &PromptTemplateDef) {
        self.push_indent();
        let params: Vec<String> = pt.params.iter()
            .map(|p| format!("{} {}", self.format_type(&p.ty), p.name))
            .collect();
        self.output.push_str(&format!("prompt {}({}) {{\n", pt.name, params.join(", ")));
        self.indent += 1;
        self.push_indent();
        self.output.push_str(&format!("{}\n", pt.body));
        self.indent -= 1;
        self.push_indent();
        self.output.push_str("}\n");
    }

    fn format_impl(&mut self, type_name: &str, type_params: &[String], methods: &[MethodDecl]) {
        self.push_indent();
        let tp = if type_params.is_empty() { "".to_string() } else { format!("<{}>", type_params.join(", ")) };
        self.output.push_str(&format!("impl {}{} {{\n", type_name, tp));
        self.indent += 1;
        for method in methods {
            self.format_method(method);
        }
        self.indent -= 1;
        self.push_indent();
        self.output.push_str("}\n");
    }

    fn format_method(&mut self, m: &MethodDecl) {
        // Annotations
        for ann in &m.annotations {
            self.push_indent();
            if ann.values.is_empty() {
                self.output.push_str(&format!("@[{}]\n", ann.name));
            } else {
                self.output.push_str(&format!("@[{}({})]\n", ann.name, ann.values.iter().map(|v| format!("\"{}\"", v)).collect::<Vec<_>>().join(", ")));
            }
        }
        self.push_indent();
        let vis = if m.is_public { "public " } else { "" };
        let async_kw = if m.is_async { "async " } else { "" };
        let ret = m.return_ty.as_ref()
            .map(|t| format!("{} ", self.format_type(t)))
            .unwrap_or_default();
        let args: Vec<String> = m.args.iter()
            .map(|a| format!("{} {}", self.format_type(&a.ty), a.name))
            .collect();

        if let Some(ref body) = m.body {
            self.output.push_str(&format!("{}{}{}{}({}) {{\n", vis, async_kw, ret, m.name, args.join(", ")));
            self.indent += 1;
            self.format_block(body);
            self.indent -= 1;
            self.push_indent();
            self.output.push_str("}\n");
        } else {
            self.output.push_str(&format!("{}{}{}{}({});\n", vis, async_kw, ret, m.name, args.join(", ")));
        }
    }

    fn format_block(&mut self, block: &Block) {
        for stmt in &block.statements {
            self.format_statement(stmt);
        }
    }

    fn format_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Let { name, ty, value } => {
                self.push_indent();
                if let Some(t) = ty {
                    self.output.push_str(&format!("let {} {} = {};\n", self.format_type(t), name, self.format_expression(value)));
                } else {
                    self.output.push_str(&format!("var {} = {};\n", name, self.format_expression(value)));
                }
            }
            Statement::Const { name, ty, value } => {
                self.push_indent();
                if let Some(t) = ty {
                    self.output.push_str(&format!("const {} {} = {};\n", self.format_type(t), name, self.format_expression(value)));
                } else {
                    self.output.push_str(&format!("const {} = {};\n", name, self.format_expression(value)));
                }
            }
            Statement::Assign { name, value } => {
                self.push_indent();
                self.output.push_str(&format!("{} = {};\n", name, self.format_expression(value)));
            }
            Statement::IndexAssign { target, index, value } => {
                self.push_indent();
                self.output.push_str(&format!("{}[{}] = {};\n", self.format_expression(target), self.format_expression(index), self.format_expression(value)));
            }
            Statement::PropertyAssign { target, property, value } => {
                self.push_indent();
                self.output.push_str(&format!("{}.{} = {};\n", self.format_expression(target), property, self.format_expression(value)));
            }
            Statement::Return(expr) => {
                self.push_indent();
                if let Some(e) = expr {
                    self.output.push_str(&format!("return {};\n", self.format_expression(e)));
                } else {
                    self.output.push_str("return;\n");
                }
            }
            Statement::Expr(e) => {
                self.push_indent();
                self.output.push_str(&format!("{};\n", self.format_expression(e)));
            }
            Statement::Print(e) => {
                self.push_indent();
                self.output.push_str(&format!("print {};\n", self.format_expression(e)));
            }
            Statement::If { condition, then_block, else_block } => {
                self.push_indent();
                self.output.push_str(&format!("if {} {{\n", self.format_expression(condition)));
                self.indent += 1;
                self.format_block(then_block);
                self.indent -= 1;
                if let Some(eb) = else_block {
                    self.push_indent();
                    self.output.push_str("} else {\n");
                    self.indent += 1;
                    self.format_block(eb);
                    self.indent -= 1;
                }
                self.push_indent();
                self.output.push_str("}\n");
            }
            Statement::While { condition, body } => {
                self.push_indent();
                self.output.push_str(&format!("while {} {{\n", self.format_expression(condition)));
                self.indent += 1;
                self.format_block(body);
                self.indent -= 1;
                self.push_indent();
                self.output.push_str("}\n");
            }
            Statement::For { init, condition, update, body } => {
                self.push_indent();
                let init_str = self.format_statement_inline(init);
                let update_str = self.format_statement_inline(update);
                self.output.push_str(&format!("for {}; {}; {} {{\n", init_str, self.format_expression(condition), update_str));
                self.indent += 1;
                self.format_block(body);
                self.indent -= 1;
                self.push_indent();
                self.output.push_str("}\n");
            }
            Statement::Foreach { item_name, value_name, collection, body } => {
                self.push_indent();
                if let Some(vn) = value_name {
                    self.output.push_str(&format!("foreach ({}, {}) in {} {{\n", item_name, vn, self.format_expression(collection)));
                } else {
                    self.output.push_str(&format!("foreach {} in {} {{\n", item_name, self.format_expression(collection)));
                }
                self.indent += 1;
                self.format_block(body);
                self.indent -= 1;
                self.push_indent();
                self.output.push_str("}\n");
            }
            Statement::Break => {
                self.push_indent();
                self.output.push_str("break;\n");
            }
            Statement::Continue => {
                self.push_indent();
                self.output.push_str("continue;\n");
            }
            Statement::TryCatch { try_block, catch_var, catch_block } => {
                self.push_indent();
                self.output.push_str("try {\n");
                self.indent += 1;
                self.format_block(try_block);
                self.indent -= 1;
                self.push_indent();
                self.output.push_str(&format!("}} catch ({}) {{\n", catch_var));
                self.indent += 1;
                self.format_block(catch_block);
                self.indent -= 1;
                self.push_indent();
                self.output.push_str("}\n");
            }
            Statement::Throw(e) => {
                self.push_indent();
                self.output.push_str(&format!("throw {};\n", self.format_expression(e)));
            }
            Statement::Stream(e) => {
                self.push_indent();
                self.output.push_str(&format!("stream {};\n", self.format_expression(e)));
            }
            Statement::UnsafeBlock(block) => {
                self.push_indent();
                self.output.push_str("unsafe {\n");
                self.indent += 1;
                self.format_block(block);
                self.indent -= 1;
                self.push_indent();
                self.output.push_str("}\n");
            }
            Statement::Match { subject, arms } => {
                self.push_indent();
                self.output.push_str(&format!("match {} {{\n", self.format_expression(subject)));
                self.indent += 1;
                for arm in arms {
                    self.push_indent();
                    self.output.push_str(&format!("{}", self.format_pattern(&arm.pattern)));
                    if let Some(guard) = &arm.guard {
                        self.output.push_str(&format!(" if {}", self.format_expression(guard)));
                    }
                    self.output.push_str(" => {\n");
                    self.indent += 1;
                    self.format_block(&arm.body);
                    self.indent -= 1;
                    self.push_indent();
                    self.output.push_str("}\n");
                }
                self.indent -= 1;
                self.push_indent();
                self.output.push_str("}\n");
            }
            Statement::LetDestructure { pattern, value } => {
                self.push_indent();
                match pattern {
                    DestructurePattern::Tuple(names) => {
                        self.output.push_str(&format!("let ({}) = {};\n", names.join(", "), self.format_expression(value)));
                    }
                    DestructurePattern::Struct(fields) => {
                        let field_strs: Vec<String> = fields.iter().map(|(name, alias)| {
                            if let Some(a) = alias { format!("{}: {}", name, a) } else { name.clone() }
                        }).collect();
                        self.output.push_str(&format!("let {{ {} }} = {};\n", field_strs.join(", "), self.format_expression(value)));
                    }
                }
            }
            Statement::Select { arms } => {
                self.push_indent();
                self.output.push_str("select {\n");
                self.indent += 1;
                for arm in arms {
                    self.push_indent();
                    match &arm.source {
                        SelectSource::Agent(e) => self.output.push_str(&format!("{} from {} => {{\n", arm.var_name, self.format_expression(e))),
                        SelectSource::Timeout(e) => self.output.push_str(&format!("{} timeout({}) => {{\n", arm.var_name, self.format_expression(e))),
                    }
                    self.indent += 1;
                    self.format_block(&arm.body);
                    self.indent -= 1;
                    self.push_indent();
                    self.output.push_str("}\n");
                }
                self.indent -= 1;
                self.push_indent();
                self.output.push_str("}\n");
            }
        }
    }

    /// Format a statement inline (for for-loop init/update)
    fn format_statement_inline(&self, stmt: &Statement) -> String {
        match stmt {
            Statement::Let { name, ty, value } => {
                if let Some(t) = ty {
                    format!("let {} {} = {}", self.format_type(t), name, self.format_expression(value))
                } else {
                    format!("var {} = {}", name, self.format_expression(value))
                }
            }
            Statement::Assign { name, value } => format!("{} = {}", name, self.format_expression(value)),
            _ => "/* unsupported */".to_string(),
        }
    }

    fn format_pattern(&self, pattern: &Pattern) -> String {
        match pattern {
            Pattern::Literal(e) => self.format_expression(e),
            Pattern::Variant(name, bindings) => {
                if bindings.is_empty() {
                    name.clone()
                } else {
                    format!("{}({})", name, bindings.join(", "))
                }
            }
            Pattern::Wildcard => "_".to_string(),
        }
    }

    fn format_expression(&self, expr: &Expression) -> String {
        match expr {
            Expression::Int(n) => n.to_string(),
            Expression::Float(f) => format!("{}", f),
            Expression::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
            Expression::Bool(b) => b.to_string(),
            Expression::Null => "null".to_string(),
            Expression::Identifier(name) => name.clone(),
            Expression::BinaryOp { left, operator, right } => {
                format!("{} {} {}", self.format_expression(left), self.format_binop(operator), self.format_expression(right))
            }
            Expression::UnaryOp { operator, operand } => {
                match operator {
                    UnaryOperator::Negate => format!("-{}", self.format_expression(operand)),
                    UnaryOperator::Not => format!("!{}", self.format_expression(operand)),
                }
            }
            Expression::MethodCall { caller, method_name, args } => {
                let arg_strs: Vec<String> = args.iter().map(|a| self.format_expression(a)).collect();
                format!("{}.{}({})", self.format_expression(caller), method_name, arg_strs.join(", "))
            }
            Expression::PropertyAccess { caller, property_name } => {
                format!("{}.{}", self.format_expression(caller), property_name)
            }
            Expression::IndexAccess { caller, index } => {
                format!("{}[{}]", self.format_expression(caller), self.format_expression(index))
            }
            Expression::ArrayLiteral(elems) => {
                let strs: Vec<String> = elems.iter().map(|e| self.format_expression(e)).collect();
                format!("[{}]", strs.join(", "))
            }
            Expression::MapLiteral(pairs) => {
                let strs: Vec<String> = pairs.iter().map(|(k, v)| format!("{}: {}", self.format_expression(k), self.format_expression(v))).collect();
                format!("{{{}}}", strs.join(", "))
            }
            Expression::TupleLiteral(elems) => {
                let strs: Vec<String> = elems.iter().map(|e| self.format_expression(e)).collect();
                format!("({})", strs.join(", "))
            }
            Expression::StructLiteral { type_name, fields } => {
                let strs: Vec<String> = fields.iter().map(|(n, v)| format!("{}: {}", n, self.format_expression(v))).collect();
                format!("{} {{ {} }}", type_name, strs.join(", "))
            }
            Expression::EnumConstruct { enum_name, variant_name, args } => {
                let prefix = if enum_name.is_empty() { String::new() } else { format!("{}::", enum_name) };
                if args.is_empty() {
                    format!("{}{}", prefix, variant_name)
                } else {
                    let strs: Vec<String> = args.iter().map(|a| self.format_expression(a)).collect();
                    format!("{}{}({})", prefix, variant_name, strs.join(", "))
                }
            }
            Expression::Lambda { params, return_ty, body } => {
                let param_strs: Vec<String> = params.iter().map(|p| format!("{} {}", self.format_type(&p.ty), p.name)).collect();
                let ret = return_ty.as_ref().map(|t| format!(" -> {}", self.format_type(t))).unwrap_or_default();
                let body_str = match body.as_ref() {
                    LambdaBody::Expression(e) => self.format_expression(e),
                    LambdaBody::Block(_) => "{ ... }".to_string(),
                };
                format!("({}){} => {}", param_strs.join(", "), ret, body_str)
            }
            Expression::Await(e) => format!("await {}", self.format_expression(e)),
            Expression::TryPropagate(e) => format!("{}?", self.format_expression(e)),
            Expression::OrDefault { expr, default } => format!("{} or {}", self.format_expression(expr), self.format_expression(default)),
            Expression::Range { start, end, inclusive } => {
                let op = if *inclusive { "..=" } else { ".." };
                format!("{}{}{}", self.format_expression(start), op, self.format_expression(end))
            }
            Expression::Cast { expr, target_type } => format!("{} as {}", self.format_expression(expr), self.format_type(target_type)),
            Expression::InterpolatedString(parts) => {
                let mut s = String::from("$\"");
                for part in parts {
                    match part {
                        InterpolationPart::Literal(text) => s.push_str(text),
                        InterpolationPart::Expression(e) => {
                            s.push('{');
                            s.push_str(&self.format_expression(e));
                            s.push('}');
                        }
                    }
                }
                s.push('"');
                s
            }
            Expression::PromptLiteral(text) => format!("prompt \"\"\"{}\"\"\"", text),
            Expression::Query(q) => format!("query {{ {} }}", q.raw_query),
            Expression::Linq(q) => {
                let mut s = format!("from {} in {}", q.from_var, self.format_expression(&q.in_collection));
                if let Some(w) = &q.where_clause {
                    s.push_str(&format!(" where {}", self.format_expression(w)));
                }
                if let Some(o) = &q.orderby_clause {
                    s.push_str(&format!(" orderby {}", self.format_expression(o)));
                    if q.descending {
                        s.push_str(" descending");
                    }
                }
                s.push_str(&format!(" select {}", self.format_expression(&q.select_clause)));
                s
            }
            Expression::Retry { max_attempts, body: _, fallback: _ } => {
                format!("retry({}) {{ ... }}", self.format_expression(max_attempts))
            }
            Expression::Spawn { agent_name, args } => {
                let strs: Vec<String> = args.iter().map(|a| self.format_expression(a)).collect();
                format!("spawn {}({})", agent_name, strs.join(", "))
            }
            Expression::IfExpr { condition, then_block: _, else_block: _ } => {
                format!("if {} {{ ... }} else {{ ... }}", self.format_expression(condition))
            }
        }
    }

    fn format_binop(&self, op: &BinaryOperator) -> &str {
        match op {
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
            BinaryOperator::CosineSim => "~",
        }
    }

    fn format_type(&self, ty: &TypeNode) -> String {
        match ty {
            TypeNode::Int => "int".to_string(),
            TypeNode::Float => "float".to_string(),
            TypeNode::String => "string".to_string(),
            TypeNode::Bool => "bool".to_string(),
            TypeNode::Void => "void".to_string(),
            TypeNode::Ulong => "ulong".to_string(),
            TypeNode::Prompt => "Prompt".to_string(),
            TypeNode::Context => "Context".to_string(),
            TypeNode::Tensor => "Tensor".to_string(),
            TypeNode::Embedding => "Embedding".to_string(),
            TypeNode::Error => "Error".to_string(),
            TypeNode::Array(inner) => format!("{}[]", self.format_type(inner)),
            TypeNode::List(inner) => format!("List<{}>", self.format_type(inner)),
            TypeNode::Map(k, v) => format!("map<{}, {}>", self.format_type(k), self.format_type(v)),
            TypeNode::Set(inner) => format!("set<{}>", self.format_type(inner)),
            TypeNode::Nullable(inner) => format!("{}?", self.format_type(inner)),
            TypeNode::Result(ok, err) => format!("Result<{}, {}>", self.format_type(ok), self.format_type(err)),
            TypeNode::Tuple(types) => {
                let strs: Vec<String> = types.iter().map(|t| self.format_type(t)).collect();
                format!("({})", strs.join(", "))
            }
            TypeNode::TypeVar(name) => name.clone(),
            TypeNode::Generic(name, params) => {
                let strs: Vec<String> = params.iter().map(|t| self.format_type(t)).collect();
                format!("{}<{}>", name, strs.join(", "))
            }
            TypeNode::Custom(name) => name.clone(),
            TypeNode::Capability(cap) => format!("{:?}", cap),
            TypeNode::Func(params, ret) => {
                let strs: Vec<String> = params.iter().map(|t| self.format_type(t)).collect();
                format!("({}) => {}", strs.join(", "), self.format_type(ret))
            }
            TypeNode::AgentHandle(name) => format!("AgentHandle<{}>", name),
            TypeNode::JsonValue => "JsonValue".to_string(),
        }
    }

    fn push_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use varg_parser::Parser;

    #[test]
    fn test_format_agent_basic() {
        let source = r#"agent App { public void Run() { return; } }"#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        let mut fmt = VargFormatter::new();
        let formatted = fmt.format_program(&program);
        assert!(formatted.contains("agent App {"));
        assert!(formatted.contains("    public void Run()"));
        assert!(formatted.contains("        return;"));
    }

    #[test]
    fn test_format_struct() {
        let source = r#"struct Point { int x; int y; }"#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        let mut fmt = VargFormatter::new();
        let formatted = fmt.format_program(&program);
        assert!(formatted.contains("struct Point {"));
        assert!(formatted.contains("    int x;"));
        assert!(formatted.contains("    int y;"));
    }

    #[test]
    fn test_format_if_else() {
        let source = r#"agent App { public void Run() { if true { return; } else { return; } } }"#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        let mut fmt = VargFormatter::new();
        let formatted = fmt.format_program(&program);
        assert!(formatted.contains("if true {"));
        assert!(formatted.contains("} else {"));
    }

    #[test]
    fn test_format_while() {
        let source = r#"agent App { public void Run() { while true { break; } } }"#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        let mut fmt = VargFormatter::new();
        let formatted = fmt.format_program(&program);
        assert!(formatted.contains("while true {"));
        assert!(formatted.contains("break;"));
    }

    #[test]
    fn test_format_function() {
        let source = r#"fn add(int a, int b) -> int { return a + b; }"#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        let mut fmt = VargFormatter::new();
        let formatted = fmt.format_program(&program);
        assert!(formatted.contains("fn add(int a, int b) -> int {"));
        assert!(formatted.contains("    return a + b;"));
    }

    #[test]
    fn test_format_expressions() {
        let fmt = VargFormatter::new();
        assert_eq!(fmt.format_expression(&Expression::Int(42)), "42");
        assert_eq!(fmt.format_expression(&Expression::String("hello".to_string())), "\"hello\"");
        assert_eq!(fmt.format_expression(&Expression::Bool(true)), "true");
        assert_eq!(fmt.format_expression(&Expression::Null), "null");
        assert_eq!(fmt.format_expression(&Expression::ArrayLiteral(vec![Expression::Int(1), Expression::Int(2)])), "[1, 2]");
    }

    #[test]
    fn test_format_roundtrip() {
        let source = r#"
            struct Point {
                int x;
                int y;
            }

            fn add(int a, int b) -> int {
                return a + b;
            }
        "#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();

        let mut fmt = VargFormatter::new();
        let formatted = fmt.format_program(&program);

        // Re-parse the formatted output
        let mut parser2 = Parser::new(&formatted);
        let program2 = parser2.parse_program().unwrap();

        // ASTs should be equal
        assert_eq!(program.items.len(), program2.items.len());
    }

    #[test]
    fn test_format_match() {
        let source = r#"agent App { public void Run() { match x { 1 => { return; } _ => { return; } } } }"#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        let mut fmt = VargFormatter::new();
        let formatted = fmt.format_program(&program);
        assert!(formatted.contains("match x {"), "formatted: {}", formatted);
    }

    #[test]
    fn test_format_foreach() {
        let source = r#"agent App { public void Run() { foreach item in items { print item; } } }"#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        let mut fmt = VargFormatter::new();
        let formatted = fmt.format_program(&program);
        assert!(formatted.contains("foreach item in items {"), "formatted: {}", formatted);
    }

    #[test]
    fn test_format_try_catch() {
        let source = r#"agent App { public void Run() { try { return; } catch (e) { return; } } }"#;
        let mut parser = Parser::new(source);
        let program = parser.parse_program().unwrap();
        let mut fmt = VargFormatter::new();
        let formatted = fmt.format_program(&program);
        assert!(formatted.contains("try {"), "formatted: {}", formatted);
        assert!(formatted.contains("} catch (e) {"), "formatted: {}", formatted);
    }
}
