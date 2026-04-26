use varg_ast::ast::*;
use std::collections::HashMap;
use std::ops::Range;

/// Wave 13: Levenshtein distance for "did you mean?" suggestions
fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

/// Find similar names from candidates (max_dist = 2, returns at most 3)
fn suggest_similar(needle: &str, candidates: &[&str]) -> Vec<String> {
    let max_dist = if needle.len() <= 3 { 1 } else { 2 };
    let mut suggestions: Vec<(usize, String)> = candidates.iter()
        .filter(|&&c| c != needle)
        .filter_map(|&c| {
            let d = levenshtein(needle, c);
            if d <= max_dist { Some((d, c.to_string())) } else { None }
        })
        .collect();
    suggestions.sort_by_key(|(d, _)| *d);
    suggestions.into_iter().take(3).map(|(_, s)| s).collect()
}

/// Semantic errors discovered during Type Checking or OCAP validation
#[derive(Debug, PartialEq)]
pub enum TypeError {
    TypeMismatch { expected: String, found: String },
    UndeclaredVariable { name: String, suggestions: Vec<String> },
    IllegalOsCall { reason: String }, // OCAP Violation
    // Plan 21: Dedicated capability error for flow analysis
    MissingCapability { capability: String, operation: String },
    CapabilityConstructionOutsideUnsafe { capability: String },
    // Plan 30: Type System Hardening
    UnknownField { type_name: String, field_name: String, suggestions: Vec<String> },
    UnknownMethod { type_name: String, method_name: String, suggestions: Vec<String> },
    NonExhaustiveMatch { type_name: String, missing_variants: Vec<String> },
    // Plan 29: Contract Enforcement
    MissingContractMethod { agent_name: String, contract_name: String, method_name: String },
    ContractNotDefined { agent_name: String, contract_name: String },
    // Plan 57: Generic type argument count mismatch
    WrongTypeArgumentCount { type_name: String, expected: usize, found: usize },
    // Plan 58: Missing return on non-void function
    MissingReturn { function_name: String },
}

impl TypeError {
    /// Human-readable error message for formatted output
    pub fn message(&self) -> String {
        match self {
            TypeError::TypeMismatch { expected, found } => {
                format!("type mismatch: expected `{}`, found `{}`", expected, found)
            }
            TypeError::UndeclaredVariable { name, suggestions } => {
                let mut msg = format!("use of undeclared variable `{}`", name);
                if let Some(first) = suggestions.first() {
                    msg.push_str(&format!(", did you mean `{}`?", first));
                }
                msg
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
            TypeError::UnknownField { type_name, field_name, suggestions } => {
                let mut msg = format!("unknown field `{}` on type `{}`", field_name, type_name);
                if let Some(first) = suggestions.first() {
                    msg.push_str(&format!(", did you mean `{}`?", first));
                }
                msg
            }
            TypeError::UnknownMethod { type_name, method_name, suggestions } => {
                let mut msg = format!("unknown method `{}` on type `{}`", method_name, type_name);
                if let Some(first) = suggestions.first() {
                    msg.push_str(&format!(", did you mean `{}`?", first));
                }
                msg
            }
            TypeError::NonExhaustiveMatch { type_name, missing_variants } => {
                format!("non-exhaustive match on `{}`: missing variant(s) {}", type_name, missing_variants.join(", "))
            }
            TypeError::MissingContractMethod { agent_name, contract_name, method_name } => {
                format!("agent `{}` implements `{}` but is missing method `{}`", agent_name, contract_name, method_name)
            }
            TypeError::ContractNotDefined { agent_name, contract_name } => {
                format!("agent `{}` implements `{}`, but contract `{}` is not defined (interface-first: define contracts before agents)", agent_name, contract_name, contract_name)
            }
            TypeError::WrongTypeArgumentCount { type_name, expected, found } => {
                format!("type `{}` expects {} type argument(s), but {} were provided", type_name, expected, found)
            }
            TypeError::MissingReturn { function_name } => {
                format!("function `{}` has non-void return type but not all code paths return a value", function_name)
            }
        }
    }

    /// Returns a searchable identifier hint for locating this error in source code
    pub fn search_hint(&self) -> Option<&str> {
        match self {
            TypeError::UndeclaredVariable { name, .. } => Some(name.as_str()),
            TypeError::UnknownField { field_name, .. } => Some(field_name),
            TypeError::UnknownMethod { method_name, .. } => Some(method_name),
            TypeError::MissingContractMethod { method_name, .. } => Some(method_name),
            TypeError::ContractNotDefined { contract_name, .. } => Some(contract_name),
            TypeError::MissingReturn { function_name } => Some(function_name),
            TypeError::MissingCapability { operation, .. } => Some(operation),
            _ => None,
        }
    }
}

/// A TypeError with an optional source span for error reporting
#[derive(Debug)]
pub struct SpannedTypeError {
    pub error: TypeError,
    pub span: Option<Range<usize>>,
}

impl SpannedTypeError {
    /// Convenience: get the error message
    pub fn message(&self) -> String {
        self.error.message()
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

    // Plan 31: Current agent name for `self` type resolution
    current_agent_name: Option<String>,

    // Plan 30: Registered struct fields (struct_name → fields)
    struct_fields: HashMap<String, Vec<FieldDecl>>,
    // Plan 30: Registered agent fields (agent_name → fields)
    agent_fields: HashMap<String, Vec<FieldDecl>>,
    // Plan 30: Registered method signatures (type_name → method_name → signature)
    method_signatures: HashMap<String, HashMap<String, MethodSignature>>,

    // Plan 28: Generic struct definitions (struct_name → StructDef with type_params)
    generic_structs: HashMap<String, StructDef>,

    // Plan 29: Known contract definitions for enforcement
    known_contracts: HashMap<String, ContractDef>,

    // Plan 33: Known standalone functions for fn ↔ agent interop
    known_functions: HashMap<String, MethodSignature>,

    // F41-1: Imported crate names (for opaque type handling)
    imported_crates: std::collections::HashSet<String>,

    // F41-6: Agent → contracts it implements (for DI type compatibility)
    agent_implements: HashMap<String, Vec<String>>,

    // Wave 12 Phase 5: Source text + current item span for error reporting
    source: Option<String>,
    current_item_span: Option<Range<usize>>,
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
            current_agent_name: None,
            struct_fields: HashMap::new(),
            agent_fields: HashMap::new(),
            method_signatures: HashMap::new(),
            generic_structs: HashMap::new(),
            known_contracts: HashMap::new(),
            known_functions: HashMap::new(),
            imported_crates: std::collections::HashSet::new(),
            agent_implements: HashMap::new(),
            source: None,
            current_item_span: None,
        }
    }

    /// Wave 13: Build UndeclaredVariable error with did-you-mean suggestions
    fn undeclared_variable_error(&self, name: &str) -> TypeError {
        let candidates: Vec<&str> = self.env.keys().map(|s| s.as_str()).collect();
        let suggestions = suggest_similar(name, &candidates);
        TypeError::UndeclaredVariable { name: name.to_string(), suggestions }
    }

    /// Wave 13: Build UnknownField error with did-you-mean suggestions
    fn unknown_field_error(&self, type_name: &str, field_name: &str) -> TypeError {
        let mut candidates = Vec::new();
        if let Some(fields) = self.struct_fields.get(type_name) {
            candidates.extend(fields.iter().map(|f| f.name.as_str()));
        }
        if let Some(fields) = self.agent_fields.get(type_name) {
            candidates.extend(fields.iter().map(|f| f.name.as_str()));
        }
        let suggestions = suggest_similar(field_name, &candidates);
        TypeError::UnknownField { type_name: type_name.to_string(), field_name: field_name.to_string(), suggestions }
    }

    /// Wave 13: Build UnknownMethod error with did-you-mean suggestions
    fn unknown_method_error(&self, type_name: &str, method_name: &str) -> TypeError {
        let mut candidates = Vec::new();
        if let Some(methods) = self.method_signatures.get(type_name) {
            candidates.extend(methods.keys().map(|s| s.as_str()));
        }
        // Also include builtin method names
        let builtins = [
            "len", "length", "contains", "starts_with", "ends_with",
            "to_upper", "to_lower", "trim", "substring", "char_at",
            "index_of", "split", "replace", "push", "pop", "first",
            "last", "reverse", "is_empty", "keys", "values", "remove", "get",
            "sort", "join", "count", "filter", "map", "flat_map", "find",
            "any", "all", "abs", "sqrt", "floor", "ceil", "round",
            "min", "max", "parse_int", "parse_float", "to_string",
            "contains_key", "send", "request", "env", "fetch", "http_request",
            "llm_infer", "llm_chat", "encrypt", "decrypt",
            "file_read", "file_write", "time_now",
            "fs_read", "fs_write", "fs_read_dir", "fs_append", "fs_read_lines",
            "create_dir", "delete_file",
            "path_exists", "path_join", "path_parent", "path_extension", "path_stem",
            "regex_match", "regex_find_all", "regex_replace",
            "sleep", "timestamp", "time_millis", "time_format", "time_parse", "time_add", "time_diff",
            "log_debug", "log_info", "log_warn", "log_error",
            "exec", "exec_status",
            "json_parse", "json_get", "json_get_int", "json_get_bool", "json_get_array", "json_stringify",
            "assert", "assert_eq", "assert_ne", "assert_true", "assert_false", "assert_contains", "assert_throws",
            "set_of",
            "graph_open", "graph_add_node", "graph_add_edge", "graph_query", "graph_traverse", "graph_neighbors",
            "embed", "vector_store_open", "vector_store_upsert", "vector_store_search", "vector_store_delete", "vector_store_count",
            "vector_search_text",
            "rag_index", "rag_retrieve", "rag_build_prompt",
            "llm_chat_cached", "llm_structured_schema", "llm_chat_opts",
            "sse_open", "sse_push", "sse_shutdown",
            "memory_open", "memory_set", "memory_get", "memory_store", "memory_recall", "memory_add_fact", "memory_query_facts", "memory_episode_count", "memory_clear_working",
            "trace_start", "trace_span", "trace_end", "trace_error", "trace_event", "trace_set_attr", "trace_span_count", "trace_export",
            "mcp_server_new", "mcp_server_register", "mcp_server_tool_count", "mcp_server_handle_request", "mcp_server_run",
            "event_bus_new", "event_emit", "event_count",
            "pipeline_new", "pipeline_run", "pipeline_step_count",
            "orchestrator_new", "orchestrator_add_task", "orchestrator_run_all", "orchestrator_results", "orchestrator_task_count", "orchestrator_completed_count",
            "self_improver_new", "self_improver_record_success", "self_improver_record_failure", "self_improver_recall", "self_improver_success_rate", "self_improver_iterations", "self_improver_stats",
            // Wave 28: System Primitives
            "args", "stdin_read", "stdin_read_line", "is_dir", "is_file", "path_resolve",
            "fs_copy", "fs_rename", "ansi_color", "ansi_bold", "ansi_reset",
            // Wave 28 Batch 2: Streaming + Process Management
            "sse_client_connect", "sse_client_post", "sse_client_next", "sse_client_close",
            "proc_spawn", "proc_spawn_args", "proc_write_stdin", "proc_close_stdin",
            "proc_read_line", "proc_wait", "proc_kill", "proc_is_alive", "proc_pid",
            // Wave 29: Binary I/O
            "fs_read_bytes", "fs_write_bytes", "fs_append_bytes", "fs_size",
            // Wave 29: Config cascade + platform dirs
            "home_dir", "config_dir", "data_dir", "cache_dir", "config_load_cascade",
            // Wave 29: Readline / REPL
            "readline_new", "readline_read", "readline_add_history",
            "readline_load_history", "readline_save_history",
            "set_env",
            // Wave 30: Human-in-the-Loop
            "await_approval", "await_input", "await_choice",
            // Wave 30: Rate Limiting
            "ratelimiter_new", "ratelimiter_acquire", "ratelimiter_try_acquire",
            "rate_limit_acquire", "rate_limit_try", "rate_limit_reset",
            // Wave 31: Budget / Cost Tracking
            "budget_new", "budget_track", "budget_check",
            "budget_remaining_tokens", "budget_remaining_usd_cents",
            "budget_report", "estimate_tokens",
            // Wave 32: Agent Checkpoint
            "checkpoint_open", "checkpoint_save", "checkpoint_load",
            "checkpoint_clear", "checkpoint_exists", "checkpoint_age",
            // Wave 33: Typed Channels
            "channel_new", "channel_send", "channel_try_recv", "channel_recv",
            "channel_recv_timeout", "channel_len", "channel_close", "channel_is_closed",
            // Wave 33: Property Testing
            "prop_gen_int", "prop_gen_float", "prop_gen_bool", "prop_gen_string",
            "prop_gen_int_list", "prop_gen_string_list", "prop_check", "prop_assert",
            // Wave 34: Multimodal
            "image_load", "image_from_base64", "image_to_base64", "image_format", "image_size_bytes",
            "audio_load", "audio_to_base64", "audio_format", "audio_size_bytes",
            "llm_vision",
            // Wave 34: Workflow / DAG
            "workflow_new", "workflow_add_step", "workflow_set_output", "workflow_set_failed",
            "workflow_ready_steps", "workflow_is_complete", "workflow_get_output",
            "workflow_step_count", "workflow_status",
            // Wave 34: Package Registry
            "registry_open", "registry_install", "registry_uninstall",
            "registry_is_installed", "registry_version", "registry_list", "registry_search",
            // LLM Extended (Wave 30-34)
            "llm_structured", "llm_stream", "llm_embed_batch",
            // Vector Extended (Wave 34)
            "vector_build_index", "vector_search_fast",
            // SSE Server (Wave 32)
            "sse_event", "http_sse_route",
        ];
        candidates.extend(builtins.iter());
        let suggestions = suggest_similar(method_name, &candidates);
        TypeError::UnknownMethod { type_name: type_name.to_string(), method_name: method_name.to_string(), suggestions }
    }

    /// Set source text for span-based error reporting
    pub fn set_source(&mut self, source: &str) {
        self.source = Some(source.to_string());
    }

    /// Find the byte offset of a name in the source (for error span approximation)
    fn find_name_span(&self, name: &str) -> Option<Range<usize>> {
        if let Some(ref src) = self.source {
            if let Some(pos) = src.find(name) {
                return Some(pos..pos + name.len());
            }
        }
        None
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

    /// Plan 21: Check that capability arguments being passed are actually available.
    /// Currently unused — retained for future OCAP propagation work.
    #[allow(dead_code)]
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

    /// Wave 12: Multi-error — collects all errors instead of stopping at first
    pub fn check_program(&mut self, program: &Program) -> Result<(), Vec<SpannedTypeError>> {
        let mut errors = Vec::new();
        for item in &program.items {
            // Set current item span for error context
            self.current_item_span = match item {
                Item::Agent(a) => self.find_name_span(&a.name),
                Item::Contract(c) => self.find_name_span(&c.name),
                Item::Struct(s) => self.find_name_span(&s.name),
                Item::Enum(e) => self.find_name_span(&e.name),
                Item::Function(f) => self.find_name_span(&f.name),
                Item::PromptTemplate(p) => self.find_name_span(&p.name),
                _ => None,
            };
            if let Err(e) = self.check_item(item) {
                // Try to get a more specific span from the error's search hint
                let span = e.search_hint()
                    .and_then(|hint| self.find_name_span(hint))
                    .or_else(|| self.current_item_span.clone());
                errors.push(SpannedTypeError { error: e, span });
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn check_item(&mut self, item: &Item) -> Result<(), TypeError> {
        match item {
            Item::Import(_) | Item::ImportDecl(_) => Ok(()), // Merged by CLI earlier
            Item::CrateImport { crate_name, .. } => {
                // F41-1: Register crate name for opaque type handling
                self.imported_crates.insert(crate_name.clone());
                Ok(())
            }
            Item::UseExtern { path } => {
                // F41-1: Register the root crate from qualified imports
                if let Some(root) = path.first() {
                    self.imported_crates.insert(root.clone());
                }
                Ok(())
            }
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

                // Plan 29: Contract enforcement — check all implemented contracts
                for contract_name in &agent.implements {
                    if let Some(contract) = self.known_contracts.get(contract_name).cloned() {
                        for required_method in &contract.methods {
                            // Wave 13: Skip check if contract method has a default implementation
                            let has_default = required_method.body.is_some();
                            let agent_has_method = agent.methods.iter().any(|m| m.name == required_method.name);
                            if !agent_has_method && !has_default {
                                return Err(TypeError::MissingContractMethod {
                                    agent_name: agent.name.clone(),
                                    contract_name: contract_name.clone(),
                                    method_name: required_method.name.clone(),
                                });
                            }
                        }
                    } else {
                        // Interface-first: contract must be defined before agent
                        return Err(TypeError::ContractNotDefined {
                            agent_name: agent.name.clone(),
                            contract_name: contract_name.clone(),
                        });
                    }
                }

                // F41-6: Track which agents implement which contracts (for DI compatibility)
                if !agent.implements.is_empty() {
                    self.agent_implements.insert(agent.name.clone(), agent.implements.clone());
                }

                // Plan 19: Store agent fields so methods can access them
                self.current_agent_fields = agent.fields.clone();
                // Plan 31: Store agent name so `self` can be resolved
                self.current_agent_name = Some(agent.name.clone());
                for method in &agent.methods {
                    self.check_method(method)?;
                }
                self.current_agent_fields.clear();
                self.current_agent_name = None;
                Ok(())
            },
            Item::Contract(contract) => {
                // Plan 29: Register contract for enforcement
                self.known_contracts.insert(contract.name.clone(), contract.clone());
                // Wave 13: Type-check default method bodies
                for method in &contract.methods {
                    if method.body.is_some() {
                        self.check_method(method)?;
                    }
                }
                Ok(())
            },
            Item::Struct(s) => {
                // Plan 30: Register struct fields for property access resolution
                self.struct_fields.insert(s.name.clone(), s.fields.clone());
                // Plan 28: Register generic struct definitions
                if !s.type_params.is_empty() {
                    self.generic_structs.insert(s.name.clone(), s.clone());
                }
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
                // Plan 33: Register standalone function for fn ↔ agent interop
                self.known_functions.insert(f.name.clone(), MethodSignature {
                    return_ty: f.return_ty.clone(),
                    args: f.params.clone(),
                });
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
                // Plan 58: Validate all code paths return for non-void functions
                if let Some(ref ret_ty) = f.return_ty {
                    if *ret_ty != TypeNode::Void && !Self::block_always_returns(&f.body) {
                        self.current_return_ty = None;
                        return Err(TypeError::MissingReturn { function_name: f.name.clone() });
                    }
                }
                self.current_return_ty = None;
                Ok(())
            },
            // Wave 13: impl blocks for structs
            Item::Impl { type_name, type_params: _, methods } => {
                // Validate that the struct exists
                if !self.struct_fields.contains_key(type_name) {
                    return Err(TypeError::IllegalOsCall {
                        reason: format!("impl for unknown type `{}`", type_name),
                    });
                }
                // Register methods on the type
                let mut method_sigs = self.method_signatures.remove(type_name).unwrap_or_default();
                for method in methods {
                    method_sigs.insert(method.name.clone(), MethodSignature {
                        return_ty: method.return_ty.clone(),
                        args: method.args.clone(),
                    });
                }
                self.method_signatures.insert(type_name.clone(), method_sigs);

                // Set up context for type-checking method bodies (like agent methods)
                let saved_fields = self.current_agent_fields.clone();
                let saved_name = self.current_agent_name.clone();
                if let Some(fields) = self.struct_fields.get(type_name) {
                    self.current_agent_fields = fields.clone();
                }
                self.current_agent_name = Some(type_name.clone());

                for method in methods {
                    self.check_method(method)?;
                }

                self.current_agent_fields = saved_fields;
                self.current_agent_name = saved_name;
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
                    expected: format!("declared type parameter for constraint `where {}: {}`", constraint.type_param, constraint.bounds.join(" + ")),
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

        // Plan 31: Register `self` in method scope for self-calls
        if let Some(ref agent_name) = self.current_agent_name {
            self.env.insert("self".to_string(), TypeNode::Custom(agent_name.clone()));
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
            // Plan 58: Validate all code paths return for non-void methods
            if let Some(ref ret_ty) = method.return_ty {
                if *ret_ty != TypeNode::Void && !Self::block_always_returns(body) {
                    self.current_return_ty = None;
                    return Err(TypeError::MissingReturn { function_name: method.name.clone() });
                }
            }
        }

        self.current_return_ty = None;
        Ok(())
    }

    fn check_block(&mut self, block: &Block) -> Result<(), TypeError> {
        let previous_unsafe = self.in_unsafe_block;

        for stmt in &block.statements {
            match stmt {
                Statement::Let { name, ty, value } => {
                    // Plan 59: OCAP — capability tokens can only be constructed in unsafe blocks
                    if let Some(TypeNode::Capability(ref cap)) = ty {
                        if !self.in_unsafe_block {
                            let cap_name = format!("{:?}", cap);
                            return Err(TypeError::CapabilityConstructionOutsideUnsafe { capability: cap_name });
                        }
                    }
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
                    let expected_ty = self.env.get(name).cloned().ok_or_else(|| self.undeclared_variable_error(name))?;
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
                Statement::Foreach { item_name, value_name, collection, body } => {
                     let coll_ty = self.infer_expression_type(collection)?;
                     // Wave 16: Map iteration with (key, value) destructuring
                     if let Some(val_name) = value_name {
                         match &coll_ty {
                             TypeNode::Map(key_ty, val_ty) => {
                                 self.env.insert(item_name.clone(), *key_ty.clone());
                                 self.env.insert(val_name.clone(), *val_ty.clone());
                             }
                             // Also handle Generic("map", [K, V]) from parser
                             TypeNode::Generic(name, args) if name == "map" && args.len() == 2 => {
                                 self.env.insert(item_name.clone(), args[0].clone());
                                 self.env.insert(val_name.clone(), args[1].clone());
                             }
                             _ => {
                                 return Err(TypeError::TypeMismatch {
                                     expected: "map<K, V>".to_string(),
                                     found: format!("{:?}", coll_ty),
                                 });
                             }
                         }
                     } else {
                         // Extract inner type from Array/List/Map(keys), fall back to Dynamic
                         let item_ty = match &coll_ty {
                             TypeNode::Array(inner) => *inner.clone(),
                             TypeNode::List(inner) => *inner.clone(),
                             TypeNode::Set(inner) => *inner.clone(),
                             TypeNode::Map(key_ty, _) => *key_ty.clone(),
                             TypeNode::Generic(name, args) if name == "map" && args.len() == 2 => args[0].clone(),
                             _ => TypeNode::Custom("Dynamic".to_string()),
                         };
                         self.env.insert(item_name.clone(), item_ty);
                     }
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

    /// Plan 28: Substitute type variables in a type using a substitution map
    fn substitute_type(ty: &TypeNode, subs: &HashMap<String, TypeNode>) -> TypeNode {
        match ty {
            TypeNode::TypeVar(name) => subs.get(name).cloned().unwrap_or_else(|| ty.clone()),
            TypeNode::Array(inner) => TypeNode::Array(Box::new(Self::substitute_type(inner, subs))),
            TypeNode::List(inner) => TypeNode::List(Box::new(Self::substitute_type(inner, subs))),
            TypeNode::Set(inner) => TypeNode::Set(Box::new(Self::substitute_type(inner, subs))),
            TypeNode::Map(k, v) => TypeNode::Map(
                Box::new(Self::substitute_type(k, subs)),
                Box::new(Self::substitute_type(v, subs)),
            ),
            TypeNode::Nullable(inner) => TypeNode::Nullable(Box::new(Self::substitute_type(inner, subs))),
            TypeNode::Generic(name, args) => TypeNode::Generic(
                name.clone(),
                args.iter().map(|a| Self::substitute_type(a, subs)).collect(),
            ),
            _ => ty.clone(),
        }
    }

    /// Plan 58: Check if a block always returns a value on all code paths
    fn block_always_returns(block: &Block) -> bool {
        if block.statements.is_empty() {
            return false;
        }
        match block.statements.last().unwrap() {
            Statement::Return(Some(_)) => true,
            Statement::If { then_block, else_block: Some(else_b), .. } => {
                Self::block_always_returns(then_block) && Self::block_always_returns(else_b)
            },
            Statement::Match { arms, .. } => {
                if arms.is_empty() { return false; }
                // All arms must return AND there must be a wildcard or exhaustive coverage
                let has_wildcard = arms.iter().any(|arm| matches!(arm.pattern, Pattern::Wildcard));
                has_wildcard && arms.iter().all(|arm| Self::block_always_returns(&arm.body))
            },
            _ => false,
        }
    }

    fn infer_expression_type(&mut self, expr: &Expression) -> Result<TypeNode, TypeError> {
        match expr {
            Expression::Int(_) => Ok(TypeNode::Int),
            Expression::Float(_) => Ok(TypeNode::Float),  // Plan 42
            Expression::String(_) => Ok(TypeNode::String),
            // Plan 35: Interpolated strings are always String type
            Expression::InterpolatedString(parts) => {
                for part in parts {
                    if let InterpolationPart::Expression(expr) = part {
                        self.infer_expression_type(expr)?;
                    }
                }
                Ok(TypeNode::String)
            },
            // Plan 37: Range expressions — both sides must be Int
            Expression::Range { start, end, .. } => {
                let start_ty = self.infer_expression_type(start)?;
                let end_ty = self.infer_expression_type(end)?;
                if start_ty != TypeNode::Int {
                    return Err(TypeError::TypeMismatch { expected: "Int".to_string(), found: format!("{:?}", start_ty) });
                }
                if end_ty != TypeNode::Int {
                    return Err(TypeError::TypeMismatch { expected: "Int".to_string(), found: format!("{:?}", end_ty) });
                }
                Ok(TypeNode::Array(Box::new(TypeNode::Int)))
            },
            // Plan 38: Tuple literal
            Expression::TupleLiteral(elements) => {
                let mut types = Vec::new();
                for elem in elements {
                    types.push(self.infer_expression_type(elem)?);
                }
                Ok(TypeNode::Tuple(types))
            },
            Expression::Null => Ok(TypeNode::Nullable(Box::new(TypeNode::Custom("Dynamic".to_string())))),
            Expression::PromptLiteral(_) => Ok(TypeNode::Prompt),
            Expression::Bool(_) => Ok(TypeNode::Bool),
            Expression::Identifier(name) => {
                if let Some(ty) = self.env.get(name) {
                    Ok(ty.clone())
                } else if self.enum_defs.contains_key(name) {
                    // Enum names are types, not variables — treat as Custom type reference
                    Ok(TypeNode::Custom(name.clone()))
                } else {
                    Err(self.undeclared_variable_error(name))
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
                let method_name = method_name.trim_start_matches("__varg_").trim_start_matches("__varg_min_");
                if method_name == "fetch" {
                    self.check_ocap(&CapabilityType::NetworkAccess, "fetch")?;
                    if args.len() < 1 || args.len() > 4 {
                        return Err(TypeError::TypeMismatch { expected: "1 to 4 arguments (url, [method], [headers], [body])".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                // ===== Wave 15: HTTP Response with Status =====
                } else if method_name == "http_request" {
                    self.check_ocap(&CapabilityType::NetworkAccess, "http_request")?;
                    if args.len() < 1 || args.len() > 4 {
                        return Err(TypeError::TypeMismatch { expected: "1 to 4 arguments (url, [method], [headers], [body])".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
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
                } else if method_name == "len" || method_name == "length" {
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
                } else if method_name == "pop" || method_name == "first" || method_name == "last" {
                    // Plan 54: Infer element type from collection
                    let caller_ty = self.infer_expression_type(caller)?;
                    match &caller_ty {
                        TypeNode::Array(inner) | TypeNode::List(inner) => Ok(*inner.clone()),
                        _ => Ok(TypeNode::Custom("Dynamic".to_string())),
                    }
                } else if method_name == "reverse" {
                    Ok(TypeNode::Void)
                } else if method_name == "is_empty" || method_name == "contains_key" {
                    Ok(TypeNode::Bool)
                } else if method_name == "keys" {
                    // Plan 54: Infer key type from map
                    let caller_ty = self.infer_expression_type(caller)?;
                    match &caller_ty {
                        TypeNode::Map(key, _) => Ok(TypeNode::Array(key.clone())),
                        _ => Ok(TypeNode::Array(Box::new(TypeNode::String))),
                    }
                } else if method_name == "values" {
                    // Plan 54: Infer value type from map
                    let caller_ty = self.infer_expression_type(caller)?;
                    match &caller_ty {
                        TypeNode::Map(_, val) => Ok(TypeNode::Array(val.clone())),
                        _ => Ok(TypeNode::Array(Box::new(TypeNode::Custom("Dynamic".to_string())))),
                    }
                } else if method_name == "remove" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (key)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Void)
                // ===== Wave 19: map.get(key, default) =====
                } else if method_name == "get" {
                    if args.len() != 2 {
                        return Err(TypeError::TypeMismatch { expected: "2 arguments (key, default)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    let caller_ty = self.infer_expression_type(caller)?;
                    match &caller_ty {
                        TypeNode::Map(_, val) => Ok(*val.clone()),
                        _ => Ok(TypeNode::Custom("Dynamic".to_string())),
                    }
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
                // ===== Plan 52: Environment Variables =====
                } else if method_name == "env" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (key)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "set_env" {
                    if args.len() != 2 {
                        return Err(TypeError::TypeMismatch { expected: "2 arguments (key, value)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    self.check_ocap(&CapabilityType::SystemAccess, "set_env")?;
                    Ok(TypeNode::Void)
                // ===== Wave 13: Stdlib Expansion — fs =====
                // ===== Wave 14: Fallible builtins return Result<T, String> =====
                } else if method_name == "fs_read" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "fs_read")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                } else if method_name == "fs_write" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (path, data)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "fs_write")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                } else if method_name == "fs_read_dir" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "fs_read_dir")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Array(Box::new(TypeNode::String))), Box::new(TypeNode::String)))
                } else if method_name == "create_dir" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "create_dir")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                } else if method_name == "delete_file" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "delete_file")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                // ===== Wave 15: fs_append + fs_read_lines =====
                } else if method_name == "fs_append" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (path, data)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "fs_append")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                } else if method_name == "fs_read_lines" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "fs_read_lines")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Array(Box::new(TypeNode::String))), Box::new(TypeNode::String)))
                // ===== Wave 15: Shell Command Execution =====
                } else if method_name == "exec" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (command)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::SystemAccess, "exec")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                } else if method_name == "exec_status" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (command)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::SystemAccess, "exec_status")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)))
                // ===== Wave 15: Typed JSON =====
                } else if method_name == "json_parse" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (json_string)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::JsonValue), Box::new(TypeNode::String)))
                } else if method_name == "json_get" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (json, path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "json_get_int" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (json, path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "json_get_bool" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (json, path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "json_get_array" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (json, path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                } else if method_name == "json_stringify" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (json)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                // ===== Wave 15: Test Framework — assert builtins =====
                } else if method_name == "assert" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (condition, message)".to_string(), found: format!("{} arguments", args.len()) }); }
                    let cond_ty = self.infer_expression_type(&args[0])?;
                    if cond_ty != TypeNode::Bool {
                        return Err(TypeError::TypeMismatch { expected: "bool".to_string(), found: format!("{:?}", cond_ty) });
                    }
                    Ok(TypeNode::Void)
                } else if method_name == "assert_eq" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (actual, expected, message)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                // ===== F41-7: Extended Assertions =====
                } else if method_name == "assert_ne" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (actual, expected, message)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "assert_true" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (condition, message)".to_string(), found: format!("{} arguments", args.len()) }); }
                    let cond_ty = self.infer_expression_type(&args[0])?;
                    if cond_ty != TypeNode::Bool {
                        return Err(TypeError::TypeMismatch { expected: "bool".to_string(), found: format!("{:?}", cond_ty) });
                    }
                    Ok(TypeNode::Void)
                } else if method_name == "assert_false" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (condition, message)".to_string(), found: format!("{} arguments", args.len()) }); }
                    let cond_ty = self.infer_expression_type(&args[0])?;
                    if cond_ty != TypeNode::Bool {
                        return Err(TypeError::TypeMismatch { expected: "bool".to_string(), found: format!("{:?}", cond_ty) });
                    }
                    Ok(TypeNode::Void)
                } else if method_name == "assert_contains" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (haystack, needle, message)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "assert_throws" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (closure, message)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                // ===== Wave 16: set_of() constructor =====
                } else if method_name == "set_of" {
                    if args.is_empty() {
                        Ok(TypeNode::Set(Box::new(TypeNode::Custom("Dynamic".to_string()))))
                    } else {
                        let first_ty = self.infer_expression_type(&args[0])?;
                        Ok(TypeNode::Set(Box::new(first_ty)))
                    }
                // ===== Wave 13: Stdlib Expansion — path =====
                } else if method_name == "path_exists" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "path_join" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (a, b)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "path_parent" || method_name == "path_extension" || method_name == "path_stem" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                // ===== Wave 13: Stdlib Expansion — regex =====
                } else if method_name == "regex_match" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (pattern, string)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Bool), Box::new(TypeNode::String)))
                } else if method_name == "regex_find_all" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (pattern, string)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Array(Box::new(TypeNode::String))), Box::new(TypeNode::String)))
                } else if method_name == "regex_replace" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (pattern, string, replacement)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                // ===== Wave 28: System Primitives — CLI args, stdin, path checks, fs ops, ANSI =====
                } else if method_name == "args" {
                    if !args.is_empty() { return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                } else if method_name == "stdin_read_line" {
                    if !args.is_empty() { return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::SystemAccess, "stdin_read_line")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                } else if method_name == "stdin_read" {
                    if !args.is_empty() { return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::SystemAccess, "stdin_read")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                } else if method_name == "is_dir" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "is_file" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "path_resolve" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                } else if method_name == "fs_copy" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (src, dst)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "fs_copy")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)))
                } else if method_name == "fs_rename" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (src, dst)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "fs_rename")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                } else if method_name == "ansi_color" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (color_name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "ansi_bold" {
                    if !args.is_empty() { return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "ansi_reset" {
                    if !args.is_empty() { return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                // ===== Wave 28 Batch 2: SSE Client =====
                } else if method_name == "sse_client_connect" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (url, headers)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::NetworkAccess, "sse_client_connect")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Custom("SseClientHandle".to_string())), Box::new(TypeNode::String)))
                } else if method_name == "sse_client_post" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (url, headers, body)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::NetworkAccess, "sse_client_post")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Custom("SseClientHandle".to_string())), Box::new(TypeNode::String)))
                } else if method_name == "sse_client_next" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                } else if method_name == "sse_client_close" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                // ===== Wave 28 Batch 2: Process Management =====
                } else if method_name == "proc_spawn" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (command)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::SystemAccess, "proc_spawn")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Custom("ProcHandle".to_string())), Box::new(TypeNode::String)))
                } else if method_name == "proc_spawn_args" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (program, args)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::SystemAccess, "proc_spawn_args")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Custom("ProcHandle".to_string())), Box::new(TypeNode::String)))
                } else if method_name == "proc_write_stdin" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (handle, data)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                } else if method_name == "proc_close_stdin" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                } else if method_name == "proc_read_line" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                } else if method_name == "proc_wait" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)))
                } else if method_name == "proc_kill" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                } else if method_name == "proc_is_alive" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "proc_pid" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                // ===== Wave 29: Binary I/O =====
                } else if method_name == "fs_read_bytes" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "fs_read_bytes")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Array(Box::new(TypeNode::Int))), Box::new(TypeNode::String)))
                } else if method_name == "fs_write_bytes" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (path, bytes)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "fs_write_bytes")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)))
                } else if method_name == "fs_append_bytes" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (path, bytes)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "fs_append_bytes")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)))
                } else if method_name == "fs_size" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "fs_size")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)))
                // ===== Wave 29: Config Cascade + Platform Dirs =====
                } else if method_name == "home_dir" || method_name == "config_dir"
                    || method_name == "data_dir" || method_name == "cache_dir"
                {
                    if !args.is_empty() {
                        return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "config_load_cascade" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (paths: List<string>)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "config_load_cascade")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                // ===== Wave 29: Readline / REPL =====
                } else if method_name == "readline_new" {
                    if !args.is_empty() { return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::SystemAccess, "readline_new")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Custom("ReadlineHandle".to_string())), Box::new(TypeNode::String)))
                } else if method_name == "readline_read" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (handle, prompt)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                } else if method_name == "readline_add_history" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (handle, line)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                } else if method_name == "readline_load_history" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (handle, path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "readline_load_history")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                } else if method_name == "readline_save_history" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (handle, path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "readline_save_history")?;
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                // ===== Wave 30: Human-in-the-Loop =====
                } else if method_name == "await_approval" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (prompt)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "await_input" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (prompt)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "await_choice" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (prompt, options)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                // ===== Wave 30: Rate Limiting =====
                } else if method_name == "ratelimiter_new" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (max_calls, window_ms)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "ratelimiter_acquire" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (key)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "ratelimiter_try_acquire" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (key)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "rate_limit_acquire" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (key, max_calls, window_ms)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "rate_limit_try" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (key, max_calls, window_ms)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "rate_limit_reset" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (key)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                // ===== Wave 31: Budget / Cost Tracking =====
                } else if method_name == "budget_new" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (max_tokens, max_usd_cents)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("BudgetHandle".to_string()))
                } else if method_name == "budget_track" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (budget, prompt, response)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "budget_check" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (budget)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "budget_remaining_tokens" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (budget)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "budget_remaining_usd_cents" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (budget)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "budget_report" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (budget)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "estimate_tokens" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (text)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                // ===== Wave 32: Agent Checkpoint =====
                } else if method_name == "checkpoint_open" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (path, agent_id)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "checkpoint_open")?;
                    Ok(TypeNode::Custom("CheckpointHandle".to_string()))
                } else if method_name == "checkpoint_save" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (handle, state_json)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "checkpoint_load" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "checkpoint_clear" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "checkpoint_exists" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "checkpoint_age" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                // ===== Wave 33: Typed Channels =====
                } else if method_name == "channel_new" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (capacity)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("ChannelHandle".to_string()))
                } else if method_name == "channel_send" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (handle, value)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "channel_try_recv" || method_name == "channel_recv" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "channel_recv_timeout" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (handle, timeout_ms)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "channel_len" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "channel_close" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "channel_is_closed" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (handle)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                // ===== Wave 33: Property-Based Testing =====
                } else if method_name == "prop_gen_int" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (min, max)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "prop_gen_float" {
                    if !args.is_empty() { return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Float)
                } else if method_name == "prop_gen_bool" {
                    if !args.is_empty() { return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "prop_gen_string" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (max_len)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "prop_gen_int_list" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (max_len)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::Int)))
                } else if method_name == "prop_gen_string_list" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (max_len, max_str_len)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                } else if method_name == "prop_check" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (fn, runs)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::Int)))
                } else if method_name == "prop_assert" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (label, fn, runs)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                // ===== Wave 34: Multimodal =====
                } else if method_name == "image_load" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "image_load")?;
                    Ok(TypeNode::Custom("VargImage".to_string()))
                } else if method_name == "image_from_base64" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (b64, format)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("VargImage".to_string()))
                } else if method_name == "image_to_base64" || method_name == "image_format" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (image)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "image_size_bytes" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (image)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "audio_load" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::FileAccess, "audio_load")?;
                    Ok(TypeNode::Custom("VargAudio".to_string()))
                } else if method_name == "audio_to_base64" || method_name == "audio_format" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (audio)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "audio_size_bytes" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (audio)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "llm_vision" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (image, prompt, model)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::LlmAccess, "llm_vision")?;
                    Ok(TypeNode::String)
                // ===== Wave 34: Workflow / DAG =====
                } else if method_name == "workflow_new" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("WorkflowHandle".to_string()))
                } else if method_name == "workflow_add_step" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (workflow, name, deps)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "workflow_set_output" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (workflow, step, output)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "workflow_set_failed" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (workflow, step, error)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "workflow_ready_steps" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (workflow)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                } else if method_name == "workflow_is_complete" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (workflow)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "workflow_get_output" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (workflow, step)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "workflow_step_count" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (workflow)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "workflow_status" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (workflow)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                // ===== Wave 34: Package Registry =====
                } else if method_name == "registry_open" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (cache_path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("RegistryHandle".to_string()))
                } else if method_name == "registry_install" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (registry, name, version)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "registry_uninstall" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (registry, name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "registry_is_installed" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (registry, name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "registry_version" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (registry, name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "registry_list" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (registry)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                } else if method_name == "registry_search" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (query)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                // ===== LLM Extended (Wave 30-34) =====
                } else if method_name == "llm_structured" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (prompt, schema_json, retries)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::LlmAccess, "llm_structured")?;
                    Ok(TypeNode::String)
                } else if method_name == "llm_stream" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (prompt, model)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::LlmAccess, "llm_stream")?;
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                } else if method_name == "llm_embed_batch" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (texts)".to_string(), found: format!("{} arguments", args.len()) }); }
                    self.check_ocap(&CapabilityType::LlmAccess, "llm_embed_batch")?;
                    Ok(TypeNode::Array(Box::new(TypeNode::Array(Box::new(TypeNode::Float)))))
                // ===== Vector Extended =====
                } else if method_name == "vector_build_index" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (store)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "vector_search_fast" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (store, query, top_k)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                // ===== SSE Server (Wave 32) =====
                } else if method_name == "sse_event" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (event_type, data)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "http_sse_route" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (server, path, handler)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                // ===== Wave 13: Stdlib Expansion — time =====
                } else if method_name == "sleep" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (ms)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "timestamp" {
                    if !args.is_empty() { return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                // ===== Wave 16: Date/Time Builtins =====
                } else if method_name == "time_millis" {
                    if !args.is_empty() { return Err(TypeError::TypeMismatch { expected: "0 arguments".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "time_format" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (millis, pattern)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "time_parse" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (string, pattern)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)))
                } else if method_name == "time_add" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (millis, delta_ms)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "time_diff" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (a_millis, b_millis)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                // ===== Wave 16: Logging =====
                } else if method_name == "log_debug" || method_name == "log_info" || method_name == "log_warn" || method_name == "log_error" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (message)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                // ===== F41-2: HTTP Server Builtins =====
                } else if method_name == "http_serve" {
                    self.check_ocap(&CapabilityType::NetworkAccess, "http_serve")?;
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (port)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("HttpServer".to_string()))
                } else if method_name == "http_route" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (method, path, handler)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "http_listen" {
                    self.check_ocap(&CapabilityType::NetworkAccess, "http_listen")?;
                    Ok(TypeNode::Void)
                // ===== F41-3: Database Builtins =====
                } else if method_name == "db_open" {
                    self.check_ocap(&CapabilityType::DbAccess, "db_open")?;
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("DbConnection".to_string()))
                } else if method_name == "db_execute" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (conn, sql, params)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)))
                } else if method_name == "db_query" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (conn, sql, params)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Array(Box::new(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String))))), Box::new(TypeNode::String)))
                // ===== F41-4: WebSocket / SSE Builtins =====
                } else if method_name == "ws_connect" {
                    self.check_ocap(&CapabilityType::NetworkAccess, "ws_connect")?;
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (url)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Custom("WebSocket".to_string())), Box::new(TypeNode::String)))
                } else if method_name == "ws_send" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (socket, message)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                } else if method_name == "ws_receive" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (socket)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                } else if method_name == "ws_close" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (socket)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "sse_stream" {
                    self.check_ocap(&CapabilityType::NetworkAccess, "sse_stream")?;
                    Ok(TypeNode::Custom("SseWriter".to_string()))
                } else if method_name == "sse_send" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (writer, event, data)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)))
                } else if method_name == "sse_close" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (writer)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                // ===== F41-8: MCP Client Builtins =====
                } else if method_name == "mcp_connect" {
                    self.check_ocap(&CapabilityType::SystemAccess, "mcp_connect")?;
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (command, args)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Custom("McpConnection".to_string())), Box::new(TypeNode::String)))
                } else if method_name == "mcp_list_tools" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (connection)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::Array(Box::new(TypeNode::Custom("McpToolInfo".to_string())))), Box::new(TypeNode::String)))
                } else if method_name == "mcp_call_tool" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (connection, tool_name, params)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                } else if method_name == "mcp_disconnect" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (connection)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                // ===== Wave 20: Knowledge Graph Builtins =====
                } else if method_name == "graph_open" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("GraphHandle".to_string()))
                } else if method_name == "graph_add_node" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (graph, label, properties)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "graph_add_edge" {
                    if args.len() != 5 { return Err(TypeError::TypeMismatch { expected: "5 arguments (graph, from_id, relation, to_id, properties)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "graph_query" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (graph, label)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String)))))
                } else if method_name == "graph_traverse" {
                    if args.len() != 4 { return Err(TypeError::TypeMismatch { expected: "4 arguments (graph, start_id, depth, relation_filter)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String)))))
                } else if method_name == "graph_neighbors" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (graph, node_id)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String)))))
                // ===== Wave 20b: Vector Store =====
                } else if method_name == "embed" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (text)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::Float)))
                } else if method_name == "vector_store_open" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("VectorStoreHandle".to_string()))
                } else if method_name == "vector_store_upsert" {
                    if args.len() != 4 { return Err(TypeError::TypeMismatch { expected: "4 arguments (store, id, embedding, metadata)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "vector_store_search" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (store, query_embedding, top_k)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String)))))
                } else if method_name == "vector_store_delete" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (store, id)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "vector_store_count" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (store)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "vector_search_text" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (store, query_text, top_k)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                // ===== RAG Pipeline =====
                } else if method_name == "rag_index" {
                    if args.len() != 4 { return Err(TypeError::TypeMismatch { expected: "4 arguments (store, id, text, metadata)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "rag_retrieve" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (store, query, top_k)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "rag_build_prompt" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (store, query, top_k)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                // ===== LLM Extended =====
                } else if method_name == "llm_chat_cached" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (ctx, prompt, model)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "llm_structured_schema" {
                    if args.len() != 4 { return Err(TypeError::TypeMismatch { expected: "4 arguments (provider, model, schema_json, prompt)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "llm_chat_opts" {
                    if args.len() != 5 { return Err(TypeError::TypeMismatch { expected: "5 arguments (ctx, prompt, model, temperature, max_tokens)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                // ===== SSE Server (channel-based) =====
                } else if method_name == "sse_open" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (server, path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("SseSenderHandle".to_string()))
                } else if method_name == "sse_push" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (sender, data)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Bool)
                } else if method_name == "sse_shutdown" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (sender)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                // ===== Wave 21: Agent Memory =====
                } else if method_name == "memory_open" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("MemoryHandle".to_string()))
                } else if method_name == "memory_set" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (mem, key, value)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "memory_get" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (mem, key, default)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "memory_store" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (mem, content, metadata)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "memory_recall" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (mem, query, top_k)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String)))))
                } else if method_name == "memory_add_fact" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (mem, label, props)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "memory_query_facts" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (mem, label)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String)))))
                } else if method_name == "memory_episode_count" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (mem)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "memory_clear_working" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (mem)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                // ===== Wave 22: Observability & Tracing =====
                } else if method_name == "trace_start" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("TracerHandle".to_string()))
                } else if method_name == "trace_span" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (tracer, name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "trace_end" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (tracer, span_id)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "trace_error" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (tracer, span_id, error_msg)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "trace_event" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (tracer, name, attributes)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "trace_set_attr" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (tracer, key, value)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "trace_span_count" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (tracer)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "trace_export" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (tracer)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                // ===== Wave 23: MCP Server =====
                } else if method_name == "mcp_server_new" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (name, version)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("McpServerHandle".to_string()))
                } else if method_name == "mcp_server_register" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (server, name, description)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "mcp_server_tool_count" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (server)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "mcp_server_handle_request" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (server, request_json)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "mcp_server_run" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (server)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                // ===== Wave 24: Reactive Pipelines =====
                } else if method_name == "event_bus_new" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("EventBusHandle".to_string()))
                } else if method_name == "event_emit" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (bus, event_name, data)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::String)))
                } else if method_name == "event_count" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (bus)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "pipeline_new" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("PipelineHandle".to_string()))
                } else if method_name == "pipeline_run" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (pipeline, input)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "pipeline_step_count" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (pipeline)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                // ===== Wave 25: Agent Orchestration =====
                } else if method_name == "orchestrator_new" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (name)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("OrchestratorHandle".to_string()))
                } else if method_name == "orchestrator_add_task" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (orch, id, input)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "orchestrator_results" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (orch)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String)))))
                } else if method_name == "orchestrator_task_count" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (orch)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "orchestrator_completed_count" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (orch)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                // ===== Wave 26: Self-Improving Loop =====
                } else if method_name == "self_improver_new" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (name, max_retries)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("SelfImproverHandle".to_string()))
                } else if method_name == "self_improver_record_success" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (improver, task, solution)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "self_improver_record_failure" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (improver, task, error)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "self_improver_recall" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (improver, task, top_k)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Array(Box::new(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String)))))
                } else if method_name == "self_improver_success_rate" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (improver)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "self_improver_iterations" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (improver)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Int)
                } else if method_name == "self_improver_stats" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (improver)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String)))
                // ===== Wave 27: Base64 Encoding/Decoding =====
                } else if method_name == "base64_encode" || method_name == "base64_decode" || method_name == "base64_encode_file" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "http_download_base64" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (url, headers)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                // ===== Wave 27: PDF Generation =====
                } else if method_name == "pdf_create" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (title)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Custom("PdfDocHandle".to_string()))
                } else if method_name == "pdf_add_section" {
                    if args.len() != 3 { return Err(TypeError::TypeMismatch { expected: "3 arguments (doc, heading, body)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "pdf_add_text" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (doc, text)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::Void)
                } else if method_name == "pdf_save" {
                    if args.len() != 2 { return Err(TypeError::TypeMismatch { expected: "2 arguments (doc, path)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                } else if method_name == "pdf_to_base64" {
                    if args.len() != 1 { return Err(TypeError::TypeMismatch { expected: "1 argument (doc)".to_string(), found: format!("{} arguments", args.len()) }); }
                    Ok(TypeNode::String)
                // ===== Wave 12: Math Methods =====
                } else if method_name == "abs" {
                    let caller_ty = self.infer_expression_type(caller)?;
                    Ok(caller_ty) // abs preserves int/float
                } else if method_name == "sqrt" || method_name == "floor" || method_name == "ceil" || method_name == "round" {
                    Ok(TypeNode::Float)
                } else if method_name == "min" || method_name == "max" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    let caller_ty = self.infer_expression_type(caller)?;
                    Ok(caller_ty)
                // ===== Wave 12: String Parsing Methods =====
                } else if method_name == "parse_int" {
                    Ok(TypeNode::Int)
                } else if method_name == "parse_float" {
                    Ok(TypeNode::Float)
                } else if method_name == "to_string" {
                    Ok(TypeNode::String)
                // ===== Wave 12: Collection Methods =====
                } else if method_name == "sort" {
                    Ok(TypeNode::Void) // in-place sort
                } else if method_name == "join" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (separator)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::String)
                } else if method_name == "count" {
                    Ok(TypeNode::Int)
                // ===== Wave 12: Iterator Methods =====
                } else if method_name == "filter" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (closure)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    let caller_ty = self.infer_expression_type(caller)?;
                    Ok(caller_ty) // filter preserves collection type
                } else if method_name == "map" || method_name == "flat_map" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (closure)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    // F41-5: Result<T, E>.map(fn(T) -> U) → Result<U, E>
                    let caller_ty = self.infer_expression_type(caller)?;
                    match caller_ty {
                        TypeNode::Result(_, err_ty) => Ok(TypeNode::Result(Box::new(TypeNode::Custom("Dynamic".to_string())), err_ty)),
                        _ => Ok(TypeNode::Array(Box::new(TypeNode::Custom("Dynamic".to_string())))),
                    }
                } else if method_name == "find" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (closure)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    let caller_ty = self.infer_expression_type(caller)?;
                    match &caller_ty {
                        TypeNode::Array(inner) | TypeNode::List(inner) => Ok(TypeNode::Nullable(inner.clone())),
                        _ => Ok(TypeNode::Nullable(Box::new(TypeNode::Custom("Dynamic".to_string())))),
                    }
                } else if method_name == "any" || method_name == "all" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (closure)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    Ok(TypeNode::Bool)
                // ===== F41-5: Result methods (map_err, map, and_then, unwrap, is_ok, is_err) =====
                } else if method_name == "map_err" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (closure)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    let caller_ty = self.infer_expression_type(caller)?;
                    match caller_ty {
                        // Result<T, E>.map_err(fn(E) -> F) → Result<T, F>
                        // For MVP: return Result<T, Dynamic> (we don't infer closure return type)
                        TypeNode::Result(ok_ty, _) => Ok(TypeNode::Result(ok_ty, Box::new(TypeNode::Custom("Dynamic".to_string())))),
                        _ => Ok(TypeNode::Custom("Dynamic".to_string())),
                    }
                } else if method_name == "and_then" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (closure)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    let caller_ty = self.infer_expression_type(caller)?;
                    match caller_ty {
                        // Result<T, E>.and_then(fn(T) -> Result<U, E>) → Result<U, E>
                        TypeNode::Result(_, err_ty) => Ok(TypeNode::Result(Box::new(TypeNode::Custom("Dynamic".to_string())), err_ty)),
                        _ => Ok(TypeNode::Custom("Dynamic".to_string())),
                    }
                } else if method_name == "unwrap" {
                    let caller_ty = self.infer_expression_type(caller)?;
                    match caller_ty {
                        TypeNode::Result(ok_ty, _) => Ok(*ok_ty),
                        TypeNode::Nullable(inner) => Ok(*inner),
                        _ => Ok(TypeNode::Custom("Dynamic".to_string())),
                    }
                } else if method_name == "unwrap_or" {
                    if args.len() != 1 {
                        return Err(TypeError::TypeMismatch { expected: "1 argument (default value)".to_string(), found: format!("{} arguments", args.len()) });
                    }
                    let caller_ty = self.infer_expression_type(caller)?;
                    match caller_ty {
                        TypeNode::Result(ok_ty, _) => Ok(*ok_ty),
                        TypeNode::Nullable(inner) => Ok(*inner),
                        _ => self.infer_expression_type(&args[0]),
                    }
                } else if method_name == "is_ok" || method_name == "is_err" || method_name == "is_some" || method_name == "is_none" {
                    Ok(TypeNode::Bool)
                } else {
                    // Plan 33: Check known standalone functions first
                    if let Some(sig) = self.known_functions.get(method_name) {
                        return Ok(sig.return_ty.clone().unwrap_or(TypeNode::Void));
                    }
                    // Issue #4: When caller is synthetic 'self' (bare function call in standalone fn),
                    // don't try to resolve 'self' — treat as forward-declared function call
                    if matches!(&**caller, Expression::Identifier(n) if n == "self") && self.current_agent_name.is_none() {
                        return Ok(TypeNode::Custom("Dynamic".to_string()));
                    }
                    // Plan 30: Look up method signatures on known types
                    let caller_ty = self.infer_expression_type(caller)?;
                    if let TypeNode::Custom(ref type_name) = caller_ty {
                        if let Some(methods) = self.method_signatures.get(type_name) {
                            if let Some(sig) = methods.get(method_name) {
                                return Ok(sig.return_ty.clone().unwrap_or(TypeNode::Void));
                            }
                            // Known type but unknown method → check standalone fns before erroring
                            // (already checked above, so this is truly unknown)
                            return Err(self.unknown_method_error(type_name, method_name));
                        }
                        // F41-6: Contract-typed callers — look up method in contract definition
                        if let Some(contract) = self.known_contracts.get(type_name) {
                            for m in &contract.methods {
                                if m.name == *method_name {
                                    return Ok(m.return_ty.clone().unwrap_or(TypeNode::Void));
                                }
                            }
                            return Err(self.unknown_method_error(type_name, method_name));
                        }
                    }
                    // F41-1: Fallback for unknown/opaque types (e.g. from external crates)
                    if !self.imported_crates.is_empty() {
                        Ok(TypeNode::Custom("Dynamic".to_string()))
                    } else {
                        Ok(TypeNode::Void)
                    }
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
                        return Err(self.unknown_field_error(type_name, property_name));
                    }
                    // Check agent fields
                    if let Some(fields) = self.agent_fields.get(type_name) {
                        if let Some(field) = fields.iter().find(|f| f.name == *property_name) {
                            return Ok(field.ty.clone());
                        }
                        // Known agent but unknown field → error
                        return Err(self.unknown_field_error(type_name, property_name));
                    }
                }
                // Plan 28: Generic struct field access with type substitution
                if let TypeNode::Generic(ref struct_name, ref type_args) = caller_ty {
                    if let Some(struct_def) = self.generic_structs.get(struct_name).cloned() {
                        // Plan 57: Validate type argument count before substitution
                        if type_args.len() != struct_def.type_params.len() {
                            return Err(TypeError::WrongTypeArgumentCount {
                                type_name: struct_name.clone(),
                                expected: struct_def.type_params.len(),
                                found: type_args.len(),
                            });
                        }
                        // Build substitution map: zip(type_params, type_args)
                        let subs: HashMap<String, TypeNode> = struct_def.type_params.iter()
                            .zip(type_args.iter())
                            .map(|(param, arg)| (param.clone(), arg.clone()))
                            .collect();
                        if let Some(field) = struct_def.fields.iter().find(|f| f.name == *property_name) {
                            return Ok(Self::substitute_type(&field.ty, &subs));
                        }
                        let generic_type_name = format!("{}<{}>", struct_name, type_args.iter().map(|a| format!("{:?}", a)).collect::<Vec<_>>().join(", "));
                        let field_suggestions: Vec<&str> = struct_def.fields.iter().map(|f| f.name.as_str()).collect();
                        let suggestions = suggest_similar(property_name, &field_suggestions);
                        return Err(TypeError::UnknownField {
                            type_name: generic_type_name,
                            field_name: property_name.clone(),
                            suggestions,
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
                // Infer type from last statement of the body block
                let body_ty = body.statements.last().map(|stmt| match stmt {
                    Statement::Return(Some(e)) | Statement::Expr(e) => {
                        self.infer_expression_type(e).unwrap_or(TypeNode::Void)
                    }
                    _ => TypeNode::Void,
                }).unwrap_or(TypeNode::Void);
                Ok(body_ty)
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
            // Wave 11: If-expression — returns type of then-block's last expr
            Expression::IfExpr { condition, then_block, else_block: _ } => {
                let cond_ty = self.infer_expression_type(condition)?;
                if cond_ty != TypeNode::Bool {
                    return Err(TypeError::TypeMismatch {
                        expected: "bool".to_string(),
                        found: format!("{:?}", cond_ty),
                    });
                }
                // Infer type from last expression in then-block
                if let Some(Statement::Expr(last_expr)) = then_block.statements.last() {
                    self.infer_expression_type(last_expr)
                } else if let Some(Statement::Return(Some(ret_expr))) = then_block.statements.last() {
                    self.infer_expression_type(ret_expr)
                } else {
                    Ok(TypeNode::Void)
                }
            },
            // Wave 11: Type casting — expr as Type
            Expression::Cast { expr, target_type } => {
                // Validate the source expression type-checks
                let _source_ty = self.infer_expression_type(expr)?;
                // The result type is always the target type
                Ok(target_type.clone())
            },
            // Wave 12: Struct literal — Point { x: 5, y: 10 }
            Expression::StructLiteral { type_name, fields } => {
                // Clone known names to avoid borrow conflict
                let known_names: Option<Vec<String>> = self.struct_fields.get(type_name)
                    .map(|sf| sf.iter().map(|f| f.name.clone()).collect());
                if let Some(ref names) = known_names {
                    for (field_name, value) in fields {
                        if !names.iter().any(|n| n == field_name) {
                            return Err(self.unknown_field_error(type_name, field_name));
                        }
                        let _val_ty = self.infer_expression_type(value)?;
                    }
                } else {
                    // Unknown struct — still type-check values
                    for (_field_name, value) in fields {
                        let _val_ty = self.infer_expression_type(value)?;
                    }
                }
                Ok(TypeNode::Custom(type_name.clone()))
            },
            // Wave 12: Enum variant construction — Shape::Circle(5) or Ok(value)
            Expression::NamedCall { method_name, named_args, .. } => {
                // Type-check all argument expressions
                for (_, expr) in named_args {
                    self.infer_expression_type(expr)?;
                }
                // Return type from known function signatures if available
                if let Some(sig) = self.known_functions.get(method_name.as_str()) {
                    Ok(sig.return_ty.clone().unwrap_or(TypeNode::Void))
                } else {
                    Ok(TypeNode::Custom("Dynamic".to_string()))
                }
            },

            Expression::EnumConstruct { enum_name, variant_name, args } => {
                // Type-check all arguments
                for arg in args {
                    let _arg_ty = self.infer_expression_type(arg)?;
                }
                // Bare variants: Ok/Err → Result, Some → Option
                if enum_name.is_empty() {
                    match variant_name.as_str() {
                        "Ok" | "Err" => {
                            let inner = if args.is_empty() {
                                TypeNode::Void
                            } else {
                                self.infer_expression_type(&args[0])?
                            };
                            if variant_name == "Ok" {
                                Ok(TypeNode::Result(Box::new(inner), Box::new(TypeNode::Error)))
                            } else {
                                Ok(TypeNode::Result(Box::new(TypeNode::Void), Box::new(inner)))
                            }
                        },
                        "Some" => {
                            let inner = if args.is_empty() {
                                TypeNode::Void
                            } else {
                                self.infer_expression_type(&args[0])?
                            };
                            Ok(TypeNode::Nullable(Box::new(inner)))
                        },
                        _ => Ok(TypeNode::Custom(variant_name.clone())),
                    }
                } else {
                    // Qualified: validate enum exists
                    if let Some(variants) = self.enum_defs.get(enum_name) {
                        if !variants.iter().any(|v| v.name == *variant_name) {
                            return Err(TypeError::TypeMismatch {
                                expected: format!("valid variant of enum `{}`", enum_name),
                                found: variant_name.clone(),
                            });
                        }
                    }
                    Ok(TypeNode::Custom(enum_name.clone()))
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
            (TypeNode::Set(inner1), TypeNode::Set(inner2)) => {
                self.types_match(inner1, inner2)
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
            // F41-6: Contract compatibility — agent that implements a contract matches the contract type
            (TypeNode::Custom(contract_name), TypeNode::Custom(agent_name)) => {
                // Check if contract_name is a known contract and agent_name implements it
                if self.known_contracts.contains_key(contract_name) {
                    if let Some(implements) = self.agent_implements.get(agent_name) {
                        implements.contains(contract_name)
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Hacker".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "StealData".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Expr(Expression::Query(SurrealQueryNode { raw_query: "SELECT secret FROM users".to_string() }))
                        ]
                    })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        
        assert!(result.is_err());
        let msg = result.unwrap_err().into_iter().next().unwrap().message();
        assert!(msg.contains("query") || msg.contains("DbAccess"));
    }

    #[test]
    fn test_valid_unsafe_query() {
        // Same logic but wrapped in `unsafe { }`
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "DbManager".to_string(),
                is_system: true,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Read".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Buggy".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let { 
                                name: "x".to_string(), 
                                ty: Some(TypeNode::Int), 
                                value: Expression::String("hello".to_string()) }
                        ]
                    })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(matches!(&result.unwrap_err()[0].error, TypeError::TypeMismatch { expected, found } if expected == "Int" && found == "String"));
    }

    #[test]
    fn test_ocap_fetch_violation() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "WebScraper".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Scrape".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "res".to_string(),
                                ty: None,
                                value: Expression::MethodCall {
                                    caller: Box::new(Expression::Identifier("self".to_string())),
                                    method_name: "fetch".to_string(),
                                    args: vec![Expression::String("https://api.github.com".to_string())] }
                            }
                        ]
                    })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        
        assert!(result.is_err());
        let msg = result.unwrap_err().into_iter().next().unwrap().message();
        assert!(msg.contains("fetch") && msg.contains("NetworkAccess"));
    }

    #[test]
    fn test_cli_command_invalid_args() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "CliAgent".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
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
                    args: vec![FieldDecl { name: "complex_arg".to_string(), ty: TypeNode::Prompt, default_value: None }], // Not allowed for CLI input directly
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] })
                }]
            })]
        };

        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        
        assert!(result.is_err());
        if let Err(ref errs) = result {
            if let TypeError::TypeMismatch { expected, found } = &errs[0].error {
                assert!(expected.contains("Primitive type"));
                assert!(found.contains("Prompt"));
            } else {
                panic!("Expected TypeMismatch error for invalid CLI args!");
            }
        } else {
            panic!("Expected error!");
        }
    }

    // ---- Plan 08: Extended TypeChecker Coverage ----

    #[test]
    fn test_undeclared_variable() {
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
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Expr(Expression::Identifier("nonexistent".to_string()))
                        ] })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(matches!(&result.unwrap_err()[0].error, TypeError::UndeclaredVariable { name, .. } if name == "nonexistent"));
    }

    #[test]
    fn test_assign_to_undeclared() {
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
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Assign {
                                name: "missing".to_string(),
                                value: Expression::Int(42) }
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(matches!(&result.unwrap_err()[0].error, TypeError::UndeclaredVariable { name, .. } if name == "missing"));
    }

    #[test]
    fn test_while_non_bool_condition() {
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
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
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
        assert!(matches!(&result.unwrap_err()[0].error, TypeError::TypeMismatch { expected, found } if expected == "Bool" && found == "Int"));
    }

    #[test]
    fn test_if_non_bool_condition() {
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
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
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
        assert!(matches!(&result.unwrap_err()[0].error, TypeError::TypeMismatch { expected, found } if expected == "Bool" && found == "String"));
    }

    #[test]
    fn test_var_type_inference() {
        // var x = 42; → x should be Int
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
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "llm_infer".to_string(),
                                args: vec![Expression::String("hello".to_string())] })
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err());
        let msg = result.unwrap_err().into_iter().next().unwrap().message();
        assert!(msg.contains("llm_infer") && msg.contains("LlmAccess"));
    }

    #[test]
    fn test_ocap_file_read_violation() {
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
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "file_read".to_string(),
                                args: vec![Expression::String("/etc/passwd".to_string())] })
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err());
        let msg = result.unwrap_err().into_iter().next().unwrap().message();
        assert!(msg.contains("file_read") && msg.contains("FileAccess"));
    }

    #[test]
    fn test_ocap_file_write_violation() {
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
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "file_write".to_string(),
                                args: vec![
                                    Expression::String("/tmp/test".to_string()),
                                    Expression::String("data".to_string()),
                                ] })
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err());
        let msg = result.unwrap_err().into_iter().next().unwrap().message();
        assert!(msg.contains("file_write") && msg.contains("FileAccess"));
    }

    #[test]
    fn test_array_literal_type_inference() {
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
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "items".to_string(),
                                ty: Some(TypeNode::Array(Box::new(TypeNode::Int))),
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
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_map_literal_type_inference() {
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
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "config".to_string(),
                                ty: Some(TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::String))),
                                value: Expression::MapLiteral(vec![
                                    (Expression::String("key".to_string()), Expression::String("val".to_string())),
                                ]) }
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
            no_std: false, docs: std::collections::HashMap::new(),
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: true,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Read".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::UnsafeBlock(Block {
                                statements: vec![
                                    Statement::Expr(Expression::MethodCall {
                                        caller: Box::new(Expression::Identifier("self".to_string())),
                                        method_name: "file_read".to_string(),
                                        args: vec![Expression::String("/tmp/data".to_string())] })
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Echo".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "msg".to_string(), ty: TypeNode::String, default_value: None }],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::Identifier("msg".to_string())))
                        ] })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_try_catch_registers_error_var() {
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
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::TryCatch {
                                try_block: Block {
                                    statements: vec![Statement::Throw(Expression::String("oops".to_string()))] },
                                catch_var: "err".to_string(),
                                catch_block: Block { statements: vec![
                                        Statement::Print(Expression::Identifier("err".to_string()))
                                    ] },
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "name".to_string(),
                                ty: Some(TypeNode::Nullable(Box::new(TypeNode::String))),
                                value: Expression::Null }
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "name".to_string(),
                                ty: Some(TypeNode::Nullable(Box::new(TypeNode::String))),
                                value: Expression::String("hello".to_string()) }
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "name".to_string(),
                                ty: Some(TypeNode::String),
                                value: Expression::Null }
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
            no_std: false, docs: std::collections::HashMap::new(),
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
                    implements: vec![],
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "ApiClient".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "GetData".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "net".to_string(), ty: TypeNode::Capability(CapabilityType::NetworkAccess), default_value: None }],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "fetch".to_string(),
                                args: vec![Expression::String("https://api.example.com".to_string())] }))
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "FileReader".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "ReadConfig".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "fs".to_string(), ty: TypeNode::Capability(CapabilityType::FileAccess), default_value: None }],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "file_read".to_string(),
                                args: vec![Expression::String("config.json".to_string())] }))
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "fs".to_string(), ty: TypeNode::Capability(CapabilityType::FileAccess), default_value: None }],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "fetch".to_string(),
                                args: vec![Expression::String("https://evil.com".to_string())] })
                        ]
                    })
                }]
            })]
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err());
        let msg = result.unwrap_err().into_iter().next().unwrap().message();
        assert!(msg.contains("NetworkAccess"));
    }

    #[test]
    fn test_ocap_llm_access_token_grants_llm_infer() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "AiAgent".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Think".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "llm".to_string(), ty: TypeNode::Capability(CapabilityType::LlmAccess), default_value: None }],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                method_name: "llm_infer".to_string(),
                                args: vec![Expression::String("What is 2+2?".to_string())] }))
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "DbReader".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "ReadAll".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![FieldDecl { name: "db".to_string(), ty: TypeNode::Capability(CapabilityType::DbAccess), default_value: None }],
                    return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let { name: "x".to_string(), ty: None, value: Expression::Int(42) },
                            Statement::Match {
                                subject: Expression::Identifier("x".to_string()),
                                arms: vec![
                                    MatchArm {
                                        pattern: Pattern::Literal(Expression::Int(1)),
                                        guard: None,
                                        body: Block { statements: vec![Statement::Print(Expression::String("one".to_string()))] },
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
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_match_variant_with_bindings() {
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
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let { name: "val".to_string(), ty: None, value: Expression::Int(10) },
                            Statement::Match {
                                subject: Expression::Identifier("val".to_string()),
                                arms: vec![
                                    MatchArm {
                                        pattern: Pattern::Variant("Some".to_string(), vec!["inner".to_string()]),
                                        guard: None,
                                        body: Block { statements: vec![
                                                Statement::Print(Expression::Identifier("inner".to_string()))
                                            ] },
                                    },
                                    MatchArm {
                                        pattern: Pattern::Wildcard,
                                        guard: None,
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec![],
                    constraints: vec![],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "add".to_string(),
                                ty: None,
                                value: Expression::Lambda {
                                    params: vec![
                                        FieldDecl { name: "a".to_string(), ty: TypeNode::Int, default_value: None },
                                        FieldDecl { name: "b".to_string(), ty: TypeNode::Int, default_value: None },
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
            params: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::String, default_value: None }],
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
        let block = Block { statements: vec![
                Statement::LetDestructure {
                    pattern: DestructurePattern::Tuple(vec!["x".to_string(), "y".to_string()]),
                    value: Expression::Identifier("some_tuple".to_string()) },
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
        let block = Block { statements: vec![
                Statement::LetDestructure {
                    pattern: DestructurePattern::Struct(vec![
                        ("name".to_string(), None),
                        ("age".to_string(), Some("a".to_string())),
                    ]),
                    value: Expression::Identifier("person".to_string()) },
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
        let block = Block { statements: vec![
                Statement::LetDestructure {
                    pattern: DestructurePattern::Tuple(vec!["x".to_string()]),
                    value: Expression::Identifier("nonexistent".to_string()) },
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

        let block = Block { statements: vec![
                Statement::Match {
                    subject: Expression::Identifier("res".to_string()),
                    arms: vec![
                        MatchArm {
                            pattern: Pattern::Variant("Ok".to_string(), vec!["val".to_string()]),
                            guard: None,
                            body: Block { statements: vec![
                                // val should be usable (bound as String from the enum variant)
                                Statement::Print(Expression::Identifier("val".to_string())),
                            ] },
                        },
                        MatchArm {
                            pattern: Pattern::Wildcard,
                            guard: None,
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

        let block = Block { statements: vec![
                Statement::Foreach {
                    item_name: "n".to_string(),
                    value_name: None,
                    collection: Expression::Identifier("nums".to_string()),
                    body: Block { statements: vec![
                        // Use n in a context that requires Int (comparison with Int)
                        Statement::Let {
                            name: "doubled".to_string(),
                            ty: Some(TypeNode::Int),
                            value: Expression::BinaryOp {
                                left: Box::new(Expression::Identifier("n".to_string())),
                                operator: BinaryOperator::Mul,
                                right: Box::new(Expression::Int(2)) },
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Sort".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec!["T".to_string()],
                    constraints: vec![
                        GenericConstraint { type_param: "U".to_string(), bounds: vec!["Comparable".to_string()] },
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false,
                is_public: false,
                target_annotation: None,
                annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Sort".to_string(),
                    is_public: true,
                    annotations: vec![],
                    type_params: vec!["T".to_string()],
                    constraints: vec![
                        GenericConstraint { type_param: "T".to_string(), bounds: vec!["Comparable".to_string()] },
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
        // Plan 54: Declare arr as Map<string,int> so keys()/values() can infer types
        checker.env.insert("arr".to_string(), TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::Int)));
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
                value: Expression::Int(100) },
        ]};
        assert!(checker.check_block(&block).is_ok());
    }

    // ===== Wave 5b: Return-Type Validation =====

    #[test]
    fn test_return_type_correct_passes() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "GetName".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::String),
                    body: Some(Block { statements: vec![
                        Statement::Return(Some(Expression::String("hello".to_string())))
                    ] }),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_return_type_mismatch_fails() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "GetCount".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Int),
                    body: Some(Block { statements: vec![
                        // Return String when Int expected → should FAIL
                        Statement::Return(Some(Expression::String("oops".to_string())))
                    ] }),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_err());
    }

    #[test]
    fn test_return_void_allows_anything() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Return(Some(Expression::Int(42)))
                    ] }),
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
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
                                args: vec![] },
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
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
                            ] }),
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
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
                                ] },
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
                    ] },
                },
                SelectArm {
                    var_name: "_timeout".to_string(),
                    source: SelectSource::Timeout(Expression::Int(5000)),
                    body: Block { statements: vec![
                        Statement::Print(Expression::String("timed out".to_string())),
                    ] },
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![
                        FieldDecl { name: "net".to_string(), ty: TypeNode::Capability(CapabilityType::NetworkAccess), default_value: None },
                    ],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Expr(Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: "fetch".to_string(),
                            args: vec![Expression::String("https://example.com".to_string())] }),
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
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
                            args: vec![Expression::String("https://example.com".to_string())] }),
                    ]}),
                }],
            })],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
        if let Err(ref errs) = result {
            if let TypeError::MissingCapability { capability, operation } = &errs[0].error {
                assert_eq!(capability, "NetworkAccess");
                assert_eq!(operation, "fetch");
            } else {
                panic!("Expected MissingCapability error, got {:?}", result);
            }
        } else {
            panic!("Expected error, got {:?}", result);
        }
    }

    #[test]
    fn test_multiple_capabilities_tracking() {
        // Method with multiple capability params can call multiple ops
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl { is_async: false,
                    name: "Run".to_string(), is_public: true,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![
                        FieldDecl { name: "net".to_string(), ty: TypeNode::Capability(CapabilityType::NetworkAccess), default_value: None },
                        FieldDecl { name: "fs".to_string(), ty: TypeNode::Capability(CapabilityType::FileAccess), default_value: None },
                        FieldDecl { name: "db".to_string(), ty: TypeNode::Capability(CapabilityType::DbAccess), default_value: None },
                    ],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Expr(Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: "fetch".to_string(),
                            args: vec![Expression::String("url".to_string())] }),
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
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
                                args: vec![Expression::String("url".to_string())] }),
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::PromptTemplate(PromptTemplateDef {
                name: "greet".to_string(),
                params: vec![
                    FieldDecl { name: "name".to_string(), ty: TypeNode::String, default_value: None },
                    FieldDecl { name: "count".to_string(), ty: TypeNode::Int, default_value: None },
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::PromptTemplate(PromptTemplateDef {
                name: "bad".to_string(),
                params: vec![
                    FieldDecl { name: "data".to_string(), ty: TypeNode::Array(Box::new(TypeNode::String)), default_value: None },
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
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
                            ) },
                        Statement::Return(Some(Expression::Identifier("data".to_string()))),
                    ]}),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    // ===== F41-5: Result Method Type Inference =====

    #[test]
    fn test_result_map_err_type() {
        let mut checker = TypeChecker::new();
        // result.map_err(fn(e) => ...) should return Result<T, Dynamic>
        checker.env.insert("result".to_string(), TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("result".to_string())),
            method_name: "map_err".to_string(),
            args: vec![Expression::Lambda {
                params: vec![FieldDecl { name: "e".to_string(), ty: TypeNode::String, default_value: None }],
                return_ty: None,
                body: Box::new(LambdaBody::Expression(Expression::String("mapped".to_string()))),
            }],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        // Should be Result<Int, Dynamic>
        if let TypeNode::Result(ok_ty, _) = ty {
            assert_eq!(*ok_ty, TypeNode::Int);
        } else {
            panic!("Expected Result type, got {:?}", ty);
        }
    }

    #[test]
    fn test_result_unwrap_type() {
        let mut checker = TypeChecker::new();
        checker.env.insert("result".to_string(), TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::Error)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("result".to_string())),
            method_name: "unwrap".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String, "unwrap on Result<String, _> should return String");
    }

    #[test]
    fn test_result_is_ok_type() {
        let mut checker = TypeChecker::new();
        checker.env.insert("result".to_string(), TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("result".to_string())),
            method_name: "is_ok".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Bool, "is_ok should return Bool");
    }

    #[test]
    fn test_result_unwrap_or_type() {
        let mut checker = TypeChecker::new();
        checker.env.insert("result".to_string(), TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::Error)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("result".to_string())),
            method_name: "unwrap_or".to_string(),
            args: vec![Expression::String("default".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String, "unwrap_or on Result<String, _> should return String");
    }

    #[test]
    fn test_or_default_type_inference() {
        // expr or "default" should type-check
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Test".to_string(),
                is_system: false, is_public: false,
                target_annotation: None, annotations: vec![],
                    implements: vec![],
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
                                default: Box::new(Expression::String("fallback".to_string())) },
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Struct(StructDef {
                    name: "User".to_string(),
                    is_public: false,
                    type_params: vec![],
                    fields: vec![
                        FieldDecl { name: "name".to_string(), ty: TypeNode::String, default_value: None },
                        FieldDecl { name: "age".to_string(), ty: TypeNode::Int, default_value: None },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "u".to_string(), ty: TypeNode::Custom("User".to_string()), default_value: None }],
                        return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::PropertyAccess {
                                caller: Box::new(Expression::Identifier("u".to_string())),
                                property_name: "name".to_string() })),
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Struct(StructDef {
                    name: "User".to_string(),
                    is_public: false,
                    type_params: vec![],
                    fields: vec![
                        FieldDecl { name: "name".to_string(), ty: TypeNode::String, default_value: None },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "u".to_string(), ty: TypeNode::Custom("User".to_string()), default_value: None }],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Expr(Expression::PropertyAccess {
                                caller: Box::new(Expression::Identifier("u".to_string())),
                                property_name: "invalid".to_string() }),
                        ]}),
                    }],
                }),
            ],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
        match &result.unwrap_err()[0].error {
            TypeError::UnknownField { type_name, field_name, .. } => {
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "Worker".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![],
                    implements: vec![],
                    fields: vec![
                        FieldDecl { name: "status".to_string(), ty: TypeNode::String, default_value: None },
                    ],
                    methods: vec![],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "w".to_string(), ty: TypeNode::Custom("Worker".to_string()), default_value: None }],
                        return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::PropertyAccess {
                                caller: Box::new(Expression::Identifier("w".to_string())),
                                property_name: "status".to_string() })),
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "Helper".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Greet".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![], return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::String("hello".to_string()))),
                        ] }),
                    }],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "h".to_string(), ty: TypeNode::Custom("Helper".to_string()), default_value: None }],
                        return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            // var x = h.Greet(); — should infer as String, not Void
                            Statement::Let {
                                name: "x".to_string(),
                                ty: Some(TypeNode::String),
                                value: Expression::MethodCall {
                                    caller: Box::new(Expression::Identifier("h".to_string())),
                                    method_name: "Greet".to_string(),
                                    args: vec![] },
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "Helper".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Greet".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![], return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::String("hi".to_string()))),
                        ] }),
                    }],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "h".to_string(), ty: TypeNode::Custom("Helper".to_string()), default_value: None }],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Expr(Expression::MethodCall {
                                caller: Box::new(Expression::Identifier("h".to_string())),
                                method_name: "Invalid".to_string(),
                                args: vec![] }),
                        ]}),
                    }],
                }),
            ],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
        match &result.unwrap_err()[0].error {
            TypeError::UnknownMethod { type_name, method_name, .. } => {
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
            no_std: false, docs: std::collections::HashMap::new(),
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
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "c".to_string(), ty: TypeNode::Custom("Color".to_string()), default_value: None }],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Match {
                                subject: Expression::Identifier("c".to_string()),
                                arms: vec![
                                    MatchArm { pattern: Pattern::Variant("Red".to_string(), vec![]), guard: None, body: Block { statements: vec![] } },
                                    MatchArm { pattern: Pattern::Variant("Green".to_string(), vec![]), guard: None, body: Block { statements: vec![] } },
                                    MatchArm { pattern: Pattern::Variant("Blue".to_string(), vec![]), guard: None, body: Block { statements: vec![] } },
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
            no_std: false, docs: std::collections::HashMap::new(),
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
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "c".to_string(), ty: TypeNode::Custom("Color".to_string()), default_value: None }],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Match {
                                subject: Expression::Identifier("c".to_string()),
                                arms: vec![
                                    MatchArm { pattern: Pattern::Variant("Red".to_string(), vec![]), guard: None, body: Block { statements: vec![] } },
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
        match &result.unwrap_err()[0].error {
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
            no_std: false, docs: std::collections::HashMap::new(),
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
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "c".to_string(), ty: TypeNode::Custom("Color".to_string()), default_value: None }],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Match {
                                subject: Expression::Identifier("c".to_string()),
                                arms: vec![
                                    MatchArm { pattern: Pattern::Variant("Red".to_string(), vec![]), guard: None, body: Block { statements: vec![] } },
                                    MatchArm { pattern: Pattern::Wildcard, guard: None, body: Block { statements: vec![] } },
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
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Match {
                                subject: Expression::Int(42),
                                arms: vec![
                                    MatchArm { pattern: Pattern::Literal(Expression::Int(1)), guard: None, body: Block { statements: vec![] } },
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
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_fn_return_type_validated() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Function(FunctionDef {
                name: "bad".to_string(),
                is_public: false,
                params: vec![],
                return_ty: Some(TypeNode::Int),
                body: Block { statements: vec![
                    // return "hello" but declared -> int = mismatch
                    Statement::Return(Some(Expression::String("hello".to_string()))),
                ] },
            })],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
    }

    // ===== Plan 28: Working Generics =====
    #[test]
    fn test_generic_struct_field_type_resolved() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Struct(StructDef {
                    name: "Box".to_string(),
                    is_public: false,
                    type_params: vec!["T".to_string()],
                    fields: vec![
                        FieldDecl { name: "value".to_string(), ty: TypeNode::TypeVar("T".to_string()), default_value: None },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "box_val".to_string(), ty: TypeNode::Generic("Box".to_string(), vec![TypeNode::Int]), default_value: None }],
                        return_ty: Some(TypeNode::Int),
                        body: Some(Block { statements: vec![
                            // return box_val.value → should resolve T to Int
                            Statement::Return(Some(Expression::PropertyAccess {
                                caller: Box::new(Expression::Identifier("box_val".to_string())),
                                property_name: "value".to_string() })),
                        ]}),
                    }],
                }),
            ],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_generic_unknown_field_error() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Struct(StructDef {
                    name: "Pair".to_string(),
                    is_public: false,
                    type_params: vec!["T".to_string()],
                    fields: vec![
                        FieldDecl { name: "first".to_string(), ty: TypeNode::TypeVar("T".to_string()), default_value: None },
                        FieldDecl { name: "second".to_string(), ty: TypeNode::TypeVar("T".to_string()), default_value: None },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "App".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![], implements: vec![], fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "p".to_string(), ty: TypeNode::Generic("Pair".to_string(), vec![TypeNode::String]), default_value: None }],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Expr(Expression::PropertyAccess {
                                caller: Box::new(Expression::Identifier("p".to_string())),
                                property_name: "invalid".to_string() }),
                        ]}),
                    }],
                }),
            ],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
        assert!(matches!(&result.unwrap_err()[0].error, TypeError::UnknownField { .. }));
    }

    #[test]
    fn test_type_substitution_map() {
        // Test the substitute_type helper directly
        let mut subs = HashMap::new();
        subs.insert("T".to_string(), TypeNode::Int);
        subs.insert("U".to_string(), TypeNode::String);

        assert_eq!(TypeChecker::substitute_type(&TypeNode::TypeVar("T".to_string()), &subs), TypeNode::Int);
        assert_eq!(TypeChecker::substitute_type(&TypeNode::TypeVar("U".to_string()), &subs), TypeNode::String);
        assert_eq!(
            TypeChecker::substitute_type(&TypeNode::Array(Box::new(TypeNode::TypeVar("T".to_string()))), &subs),
            TypeNode::Array(Box::new(TypeNode::Int))
        );
        // Unknown type var stays unchanged
        assert_eq!(
            TypeChecker::substitute_type(&TypeNode::TypeVar("V".to_string()), &subs),
            TypeNode::TypeVar("V".to_string())
        );
    }

    // ===== Plan 29: Contract Enforcement =====
    #[test]
    fn test_contract_enforcement_success() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Contract(ContractDef {
                    name: "Greeter".to_string(), is_public: false,
                    target_annotation: None,
                    methods: vec![MethodDecl {
                        name: "Greet".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![], return_ty: Some(TypeNode::String), body: None,
                    }],
                }),
                Item::Agent(AgentDef {
                    name: "MyAgent".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![],
                    implements: vec!["Greeter".to_string()],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Greet".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![], return_ty: Some(TypeNode::String),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::String("hello".to_string()))),
                        ] }),
                    }],
                }),
            ],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_contract_missing_method_error() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Contract(ContractDef {
                    name: "Worker".to_string(), is_public: false,
                    target_annotation: None,
                    methods: vec![
                        MethodDecl {
                            name: "Process".to_string(), is_public: true, is_async: false,
                            annotations: vec![], type_params: vec![], constraints: vec![],
                            args: vec![], return_ty: Some(TypeNode::Void), body: None,
                        },
                        MethodDecl {
                            name: "Cleanup".to_string(), is_public: true, is_async: false,
                            annotations: vec![], type_params: vec![], constraints: vec![],
                            args: vec![], return_ty: Some(TypeNode::Void), body: None,
                        },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "MyWorker".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![],
                    implements: vec!["Worker".to_string()],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Process".to_string(), is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![], return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![] }),
                    }],
                    // Missing Cleanup!
                }),
            ],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
        match &result.unwrap_err()[0].error {
            TypeError::MissingContractMethod { agent_name, contract_name, method_name } => {
                assert_eq!(agent_name, "MyWorker");
                assert_eq!(contract_name, "Worker");
                assert_eq!(method_name, "Cleanup");
            },
            e => panic!("Expected MissingContractMethod, got {:?}", e),
        }
    }

    #[test]
    fn test_interface_first_enforcement() {
        // Agent references contract that hasn't been defined yet
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                // Agent comes BEFORE contract — should fail
                Item::Agent(AgentDef {
                    name: "EarlyAgent".to_string(), is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![],
                    implements: vec!["LateContract".to_string()],
                    fields: vec![],
                    methods: vec![],
                }),
                Item::Contract(ContractDef {
                    name: "LateContract".to_string(), is_public: false,
                    target_annotation: None, methods: vec![],
                }),
            ],
        };
        let result = checker.check_program(&program);
        assert!(result.is_err());
        match &result.unwrap_err()[0].error {
            TypeError::ContractNotDefined { agent_name, contract_name } => {
                assert_eq!(agent_name, "EarlyAgent");
                assert_eq!(contract_name, "LateContract");
            },
            e => panic!("Expected ContractNotDefined, got {:?}", e),
        }
    }

    // ---- Plan 37: Range Expression Tests ----

    #[test]
    fn test_range_expression_type() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "T".to_string(), is_system: false, is_public: false,
                target_annotation: None, annotations: vec![], implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(), is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Foreach {
                            item_name: "i".to_string(),
                            value_name: None,
                            collection: Expression::Range {
                                start: Box::new(Expression::Int(0)),
                                end: Box::new(Expression::Int(10)),
                                inclusive: false },
                            body: Block { statements: vec![Statement::Print(Expression::Identifier("i".to_string()))] },
                        }
                    ]}),
                }],
            })],
        };
        let result = checker.check_program(&program);
        assert!(result.is_ok(), "Range 0..10 should typecheck: {:?}", result);
    }

    #[test]
    fn test_range_inclusive_type() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "T".to_string(), is_system: false, is_public: false,
                target_annotation: None, annotations: vec![], implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(), is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Foreach {
                            item_name: "i".to_string(),
                            value_name: None,
                            collection: Expression::Range {
                                start: Box::new(Expression::Int(0)),
                                end: Box::new(Expression::Int(10)),
                                inclusive: true },
                            body: Block { statements: vec![Statement::Print(Expression::Identifier("i".to_string()))] },
                        }
                    ]}),
                }],
            })],
        };
        let result = checker.check_program(&program);
        assert!(result.is_ok(), "Range 0..=10 should typecheck: {:?}", result);
    }

    // ---- Plan 38: Tuple Tests ----

    #[test]
    fn test_tuple_literal_type() {
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "T".to_string(), is_system: false, is_public: false,
                target_annotation: None, annotations: vec![], implements: vec![],
                fields: vec![],
                methods: vec![MethodDecl {
                    name: "Run".to_string(), is_public: true, is_async: false,
                    annotations: vec![], type_params: vec![], constraints: vec![],
                    args: vec![], return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![
                        Statement::Let {
                            name: "t".to_string(),
                            ty: None,
                            value: Expression::TupleLiteral(vec![
                                Expression::Int(1),
                                Expression::String("hi".to_string()),
                            ]) },
                    ]}),
                }],
            })],
        };
        let result = checker.check_program(&program);
        assert!(result.is_ok(), "Tuple literal should typecheck: {:?}", result);
    }

    // ---- Plan 39: Trait Bounds Tests ----

    #[test]
    fn test_multiple_trait_bounds_valid() {
        let mut checker = TypeChecker::new();
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
                    name: "Process".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec!["T".to_string()],
                    constraints: vec![
                        GenericConstraint { type_param: "T".to_string(), bounds: vec!["Display".to_string(), "Clone".to_string()] },
                    ],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] }),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_multiple_trait_bounds_invalid_type_param() {
        let mut checker = TypeChecker::new();
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
                    name: "Process".to_string(),
                    is_public: true, is_async: false,
                    annotations: vec![],
                    type_params: vec!["T".to_string()],
                    constraints: vec![
                        GenericConstraint { type_param: "X".to_string(), bounds: vec!["Display".to_string(), "Clone".to_string()] },
                    ],
                    args: vec![],
                    return_ty: Some(TypeNode::Void),
                    body: Some(Block { statements: vec![] }),
                }],
            })],
        };
        assert!(checker.check_program(&program).is_err());
    }

    // ===== Plan 52: env() builtin =====

    #[test]
    fn test_env_returns_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "env".to_string(),
            args: vec![Expression::String("HOME".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_env_wrong_arg_count() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "env".to_string(),
            args: vec![],
        };
        assert!(checker.infer_expression_type(&expr).is_err());
    }

    // ===== Plan 54: Collection Method Type Inference =====

    #[test]
    fn test_pop_returns_int_from_int_array() {
        let mut checker = TypeChecker::new();
        // Declare a variable of type Array(Int)
        checker.env.insert("nums".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("nums".to_string())),
            method_name: "pop".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Int, "pop() on Array<int> should return int");
    }

    #[test]
    fn test_pop_returns_string_from_string_array() {
        let mut checker = TypeChecker::new();
        checker.env.insert("names".to_string(), TypeNode::Array(Box::new(TypeNode::String)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("names".to_string())),
            method_name: "pop".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String, "pop() on Array<string> should return string");
    }

    #[test]
    fn test_first_returns_element_type() {
        let mut checker = TypeChecker::new();
        checker.env.insert("items".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "first".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Int, "first() on Array<int> should return int");
    }

    #[test]
    fn test_last_returns_element_type() {
        let mut checker = TypeChecker::new();
        checker.env.insert("items".to_string(), TypeNode::Array(Box::new(TypeNode::String)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "last".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String, "last() on Array<string> should return string");
    }

    #[test]
    fn test_keys_returns_typed_array() {
        let mut checker = TypeChecker::new();
        checker.env.insert("data".to_string(), TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("data".to_string())),
            method_name: "keys".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Array(Box::new(TypeNode::String)), "keys() on Map<string,int> should return Array<string>");
    }

    #[test]
    fn test_values_returns_typed_array() {
        let mut checker = TypeChecker::new();
        checker.env.insert("data".to_string(), TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("data".to_string())),
            method_name: "values".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Array(Box::new(TypeNode::Int)), "values() on Map<string,int> should return Array<int>");
    }

    // ===== Wave 19: map.get(key, default) =====

    #[test]
    fn test_map_get_returns_value_type() {
        let mut checker = TypeChecker::new();
        checker.env.insert("data".to_string(), TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("data".to_string())),
            method_name: "get".to_string(),
            args: vec![
                Expression::String("key".to_string()),
                Expression::Int(0),
            ],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Int, "get() on Map<string,int> should return int");
    }

    #[test]
    fn test_map_get_wrong_arg_count() {
        let mut checker = TypeChecker::new();
        checker.env.insert("data".to_string(), TypeNode::Map(Box::new(TypeNode::String), Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("data".to_string())),
            method_name: "get".to_string(),
            args: vec![Expression::String("key".to_string())],
        };
        assert!(checker.infer_expression_type(&expr).is_err(), "get() with 1 arg should fail");
    }

    // ===== Plan 57: Generic Type Argument Count Validation =====

    #[test]
    fn test_generic_correct_arg_count() {
        let mut checker = TypeChecker::new();
        checker.generic_structs.insert("Pair".to_string(), StructDef {
            name: "Pair".to_string(), is_public: true,
            type_params: vec!["K".to_string(), "V".to_string()],
            fields: vec![
                FieldDecl { name: "key".to_string(), ty: TypeNode::TypeVar("K".to_string()), default_value: None },
                FieldDecl { name: "val".to_string(), ty: TypeNode::TypeVar("V".to_string()), default_value: None },
            ],
        });
        checker.env.insert("p".to_string(), TypeNode::Generic("Pair".to_string(), vec![TypeNode::String, TypeNode::Int]));
        let expr = Expression::PropertyAccess {
            caller: Box::new(Expression::Identifier("p".to_string())),
            property_name: "key".to_string(),
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_generic_too_many_args() {
        let mut checker = TypeChecker::new();
        checker.generic_structs.insert("Box".to_string(), StructDef {
            name: "Box".to_string(), is_public: true,
            type_params: vec!["T".to_string()],
            fields: vec![FieldDecl { name: "value".to_string(), ty: TypeNode::TypeVar("T".to_string()), default_value: None }],
        });
        // Box has 1 type param but we give 2
        checker.env.insert("b".to_string(), TypeNode::Generic("Box".to_string(), vec![TypeNode::Int, TypeNode::String]));
        let expr = Expression::PropertyAccess {
            caller: Box::new(Expression::Identifier("b".to_string())),
            property_name: "value".to_string(),
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_err(), "Too many type args should error");
        if let Err(TypeError::WrongTypeArgumentCount { expected, found, .. }) = result {
            assert_eq!(expected, 1);
            assert_eq!(found, 2);
        } else { panic!("Expected WrongTypeArgumentCount, got: {:?}", result); }
    }

    #[test]
    fn test_generic_too_few_args() {
        let mut checker = TypeChecker::new();
        checker.generic_structs.insert("Pair".to_string(), StructDef {
            name: "Pair".to_string(), is_public: true,
            type_params: vec!["K".to_string(), "V".to_string()],
            fields: vec![
                FieldDecl { name: "key".to_string(), ty: TypeNode::TypeVar("K".to_string()), default_value: None },
            ],
        });
        // Pair has 2 type params but we give 1
        checker.env.insert("p".to_string(), TypeNode::Generic("Pair".to_string(), vec![TypeNode::Int]));
        let expr = Expression::PropertyAccess {
            caller: Box::new(Expression::Identifier("p".to_string())),
            property_name: "key".to_string(),
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_err(), "Too few type args should error");
        if let Err(TypeError::WrongTypeArgumentCount { expected, found, .. }) = result {
            assert_eq!(expected, 2);
            assert_eq!(found, 1);
        } else { panic!("Expected WrongTypeArgumentCount, got: {:?}", result); }
    }

    // ===== Plan 58: Return Path Analysis =====

    #[test]
    fn test_return_path_simple_return() {
        let mut checker = TypeChecker::new();
        let func = Item::Function(FunctionDef {
            name: "get_value".to_string(),
            is_public: true,
            params: vec![],
            return_ty: Some(TypeNode::Int),
            body: Block { statements: vec![
                Statement::Return(Some(Expression::Int(42))),
            ]},
        });
        assert!(checker.check_item(&func).is_ok(), "Function with return should pass");
    }

    #[test]
    fn test_return_path_missing_return() {
        let mut checker = TypeChecker::new();
        let func = Item::Function(FunctionDef {
            name: "get_value".to_string(),
            is_public: true,
            params: vec![],
            return_ty: Some(TypeNode::Int),
            body: Block { statements: vec![
                Statement::Let { name: "x".to_string(), ty: None, value: Expression::Int(42) },
            ]},
        });
        let result = checker.check_item(&func);
        assert!(result.is_err(), "Function without return should fail");
        assert!(matches!(result, Err(TypeError::MissingReturn { .. })));
    }

    #[test]
    fn test_return_path_if_else_both() {
        let mut checker = TypeChecker::new();
        checker.env.insert("x".to_string(), TypeNode::Bool);
        let func = Item::Function(FunctionDef {
            name: "pick".to_string(),
            is_public: true,
            params: vec![],
            return_ty: Some(TypeNode::Int),
            body: Block { statements: vec![
                Statement::If {
                    condition: Expression::Bool(true),
                    then_block: Block { statements: vec![
                        Statement::Return(Some(Expression::Int(1))),
                    ]},
                    else_block: Some(Block { statements: vec![
                        Statement::Return(Some(Expression::Int(2))),
                    ]}),
                },
            ]},
        });
        assert!(checker.check_item(&func).is_ok(), "If/else both returning should pass");
    }

    #[test]
    fn test_return_path_if_only_no_else() {
        let mut checker = TypeChecker::new();
        let func = Item::Function(FunctionDef {
            name: "maybe".to_string(),
            is_public: true,
            params: vec![],
            return_ty: Some(TypeNode::Int),
            body: Block { statements: vec![
                Statement::If {
                    condition: Expression::Bool(true),
                    then_block: Block { statements: vec![
                        Statement::Return(Some(Expression::Int(1))),
                    ]},
                    else_block: None,
                },
            ]},
        });
        let result = checker.check_item(&func);
        assert!(result.is_err(), "If without else should fail for non-void return");
    }

    #[test]
    fn test_return_path_void_no_return_ok() {
        let mut checker = TypeChecker::new();
        let func = Item::Function(FunctionDef {
            name: "do_stuff".to_string(),
            is_public: true,
            params: vec![],
            return_ty: Some(TypeNode::Void),
            body: Block { statements: vec![
                Statement::Let { name: "x".to_string(), ty: None, value: Expression::Int(42) },
            ]},
        });
        assert!(checker.check_item(&func).is_ok(), "Void function without return should pass");
    }

    // ===== Plan 59: OCAP Construction Validation =====

    #[test]
    fn test_capability_let_in_unsafe_ok() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        checker.env.insert("token".to_string(), TypeNode::Capability(CapabilityType::NetworkAccess));
        let block = Block { statements: vec![
            Statement::Let {
                name: "net".to_string(),
                ty: Some(TypeNode::Capability(CapabilityType::NetworkAccess)),
                value: Expression::Identifier("token".to_string()),
            },
        ]};
        // Inside unsafe block should be OK
        assert!(checker.check_block(&block).is_ok());
    }

    #[test]
    fn test_capability_let_outside_unsafe_error() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = false;
        let block = Block { statements: vec![
            Statement::Let {
                name: "net".to_string(),
                ty: Some(TypeNode::Capability(CapabilityType::NetworkAccess)),
                value: Expression::Identifier("token".to_string()),
            },
        ]};
        let result = checker.check_block(&block);
        assert!(result.is_err(), "Capability construction outside unsafe should error");
        assert!(matches!(result, Err(TypeError::CapabilityConstructionOutsideUnsafe { .. })));
    }

    #[test]
    fn test_capability_as_param_not_construction() {
        // Capability as method parameter type is NOT construction — should not error
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = false;
        // Simulating having a capability in env (passed as param, not constructed)
        checker.env.insert("net_cap".to_string(), TypeNode::Capability(CapabilityType::NetworkAccess));
        checker.available_capabilities.push(CapabilityType::NetworkAccess);
        // Using the capability should be fine
        let block = Block { statements: vec![
            Statement::Let {
                name: "x".to_string(),
                ty: None, // No explicit Capability type — just using the value
                value: Expression::Identifier("net_cap".to_string()),
            },
        ]};
        assert!(checker.check_block(&block).is_ok(), "Using capability var should not trigger construction check");
    }

    // ===== Wave 11: Type Casting =====

    #[test]
    fn test_cast_returns_target_type() {
        let mut checker = TypeChecker::new();
        checker.env.insert("x".to_string(), TypeNode::Int);
        let expr = Expression::Cast {
            expr: Box::new(Expression::Identifier("x".to_string())),
            target_type: TypeNode::Float,
        };
        let result = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(result, TypeNode::Float);
    }

    #[test]
    fn test_cast_int_to_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::Cast {
            expr: Box::new(Expression::Int(42)),
            target_type: TypeNode::String,
        };
        let result = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(result, TypeNode::String);
    }

    // ===== Wave 11: If-Expression Type =====

    #[test]
    fn test_if_expr_returns_then_type() {
        let mut checker = TypeChecker::new();
        let expr = Expression::IfExpr {
            condition: Box::new(Expression::Bool(true)),
            then_block: Block { statements: vec![Statement::Expr(Expression::Int(42))] },
            else_block: Block { statements: vec![Statement::Expr(Expression::Int(0))] },
        };
        let result = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(result, TypeNode::Int);
    }

    #[test]
    fn test_if_expr_requires_bool_condition() {
        let mut checker = TypeChecker::new();
        let expr = Expression::IfExpr {
            condition: Box::new(Expression::Int(1)),
            then_block: Block { statements: vec![Statement::Expr(Expression::Int(42))] },
            else_block: Block { statements: vec![Statement::Expr(Expression::Int(0))] },
        };
        assert!(checker.infer_expression_type(&expr).is_err(), "Non-bool condition should fail");
    }

    // ===== Realistic TypeChecker Use Case Tests =====

    #[test]
    fn test_realistic_agent_method_return_types() {
        // Agent with typed methods — type system validates returns
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Agent(AgentDef {
                name: "Calculator".to_string(),
                is_system: false, is_public: true,
                target_annotation: None, annotations: vec![],
                implements: vec![],
                fields: vec![
                    FieldDecl { name: "total".to_string(), ty: TypeNode::Int, default_value: None },
                ],
                methods: vec![
                    MethodDecl {
                        name: "Add".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None }],
                        return_ty: Some(TypeNode::Int),
                        body: Some(Block { statements: vec![
                            Statement::Assign {
                                name: "total".to_string(),
                                value: Expression::BinaryOp {
                                    left: Box::new(Expression::Identifier("total".to_string())),
                                    operator: BinaryOperator::Add,
                                    right: Box::new(Expression::Identifier("x".to_string())),
                                },
                            },
                            Statement::Return(Some(Expression::Identifier("total".to_string()))),
                        ]}),
                    },
                ],
            })],
        };
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_realistic_contract_enforcement_wrong_return() {
        // Contract says string, agent returns int — must fail
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Contract(ContractDef {
                    name: "Formatter".to_string(),
                    is_public: true, target_annotation: None,
                    methods: vec![MethodDecl {
                        name: "Format".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![FieldDecl { name: "val".to_string(), ty: TypeNode::Int, default_value: None }],
                        return_ty: Some(TypeNode::String),
                        body: None,
                    }],
                }),
                Item::Agent(AgentDef {
                    name: "BadFormatter".to_string(),
                    is_system: false, is_public: true,
                    target_annotation: None, annotations: vec![],
                    implements: vec!["Formatter".to_string()],
                    fields: vec![],
                    methods: vec![
                        // Missing the Format method entirely
                    ],
                }),
            ],
        };
        // Should fail because agent doesn't implement required Format method
        assert!(checker.check_program(&program).is_err());
    }

    #[test]
    fn test_realistic_foreach_type_inference() {
        // foreach item in items — item should be typed from array element
        let mut checker = TypeChecker::new();
        checker.env.insert("scores".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        let block = Block { statements: vec![
            Statement::Foreach {
                item_name: "score".to_string(),
                value_name: None,
                collection: Expression::Identifier("scores".to_string()),
                body: Block { statements: vec![
                    // score should be int, using it in arithmetic should work
                    Statement::Let {
                        name: "doubled".to_string(),
                        ty: None,
                        value: Expression::BinaryOp {
                            left: Box::new(Expression::Identifier("score".to_string())),
                            operator: BinaryOperator::Mul,
                            right: Box::new(Expression::Int(2)),
                        },
                    },
                ]},
            },
        ]};
        assert!(checker.check_block(&block).is_ok());
    }

    #[test]
    fn test_realistic_match_exhaustiveness() {
        // Enum with 3 variants, match only covers 2 — must error
        let mut checker = TypeChecker::new();
        checker.enum_defs.insert("Color".to_string(), vec![
            EnumVariant { name: "Red".to_string(), fields: vec![] },
            EnumVariant { name: "Green".to_string(), fields: vec![] },
            EnumVariant { name: "Blue".to_string(), fields: vec![] },
        ]);
        checker.env.insert("c".to_string(), TypeNode::Custom("Color".to_string()));
        let block = Block { statements: vec![
            Statement::Match {
                subject: Expression::Identifier("c".to_string()),
                arms: vec![
                    MatchArm { pattern: Pattern::Variant("Red".to_string(), vec![]), guard: None, body: Block { statements: vec![] } },
                    MatchArm { pattern: Pattern::Variant("Green".to_string(), vec![]), guard: None, body: Block { statements: vec![] } },
                    // Missing Blue!
                ],
            },
        ]};
        assert!(checker.check_block(&block).is_err(), "Non-exhaustive match should fail");
    }

    #[test]
    fn test_realistic_match_with_wildcard_exhaustive() {
        // Wildcard _ covers remaining variants — should pass
        let mut checker = TypeChecker::new();
        checker.enum_defs.insert("Status".to_string(), vec![
            EnumVariant { name: "Ok".to_string(), fields: vec![] },
            EnumVariant { name: "Err".to_string(), fields: vec![("msg".to_string(), TypeNode::String)] },
            EnumVariant { name: "Pending".to_string(), fields: vec![] },
        ]);
        checker.env.insert("s".to_string(), TypeNode::Custom("Status".to_string()));
        let block = Block { statements: vec![
            Statement::Match {
                subject: Expression::Identifier("s".to_string()),
                arms: vec![
                    MatchArm { pattern: Pattern::Variant("Ok".to_string(), vec![]), guard: None, body: Block { statements: vec![] } },
                    MatchArm { pattern: Pattern::Wildcard, guard: None, body: Block { statements: vec![] } },
                ],
            },
        ]};
        assert!(checker.check_block(&block).is_ok());
    }

    #[test]
    fn test_realistic_ocap_fetch_without_capability() {
        // Calling fetch() without NetworkAccess — must fail
        let mut checker = TypeChecker::new();
        let block = Block { statements: vec![
            Statement::Expr(Expression::MethodCall {
                caller: Box::new(Expression::Identifier("self".to_string())),
                method_name: "fetch".to_string(),
                args: vec![Expression::String("https://api.com".to_string())],
            }),
        ]};
        assert!(checker.check_block(&block).is_err(), "fetch without NetworkAccess should fail");
    }

    #[test]
    fn test_realistic_ocap_fetch_with_capability() {
        // Calling fetch() with NetworkAccess in scope — should pass
        let mut checker = TypeChecker::new();
        checker.available_capabilities.push(CapabilityType::NetworkAccess);
        let block = Block { statements: vec![
            Statement::Expr(Expression::MethodCall {
                caller: Box::new(Expression::Identifier("self".to_string())),
                method_name: "fetch".to_string(),
                args: vec![Expression::String("https://api.com".to_string())],
            }),
        ]};
        assert!(checker.check_block(&block).is_ok());
    }

    #[test]
    fn test_realistic_binary_op_type_checking() {
        // int + int = int, string + string = string, int + string = string
        let mut checker = TypeChecker::new();
        // int + int → int
        let expr1 = Expression::BinaryOp {
            left: Box::new(Expression::Int(1)),
            operator: BinaryOperator::Add,
            right: Box::new(Expression::Int(2)),
        };
        assert_eq!(checker.infer_expression_type(&expr1).unwrap(), TypeNode::Int);

        // string + string → string
        let expr2 = Expression::BinaryOp {
            left: Box::new(Expression::String("hello".to_string())),
            operator: BinaryOperator::Add,
            right: Box::new(Expression::String(" world".to_string())),
        };
        assert_eq!(checker.infer_expression_type(&expr2).unwrap(), TypeNode::String);

        // comparison → bool
        let expr3 = Expression::BinaryOp {
            left: Box::new(Expression::Int(1)),
            operator: BinaryOperator::Gt,
            right: Box::new(Expression::Int(2)),
        };
        assert_eq!(checker.infer_expression_type(&expr3).unwrap(), TypeNode::Bool);
    }

    #[test]
    fn test_realistic_try_propagate_with_result() {
        // expr? on Result<string, Error> should unwrap to string
        let mut checker = TypeChecker::new();
        checker.env.insert("response".to_string(), TypeNode::Result(
            Box::new(TypeNode::String),
            Box::new(TypeNode::Error),
        ));
        let expr = Expression::TryPropagate(Box::new(Expression::Identifier("response".to_string())));
        let result = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(result, TypeNode::String);
    }

    #[test]
    fn test_realistic_or_default_with_result() {
        // result or "fallback" on Result<string, Error>
        let mut checker = TypeChecker::new();
        checker.env.insert("maybe".to_string(), TypeNode::Result(
            Box::new(TypeNode::String),
            Box::new(TypeNode::Error),
        ));
        let expr = Expression::OrDefault {
            expr: Box::new(Expression::Identifier("maybe".to_string())),
            default: Box::new(Expression::String("fallback".to_string())),
        };
        let result = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(result, TypeNode::String);
    }

    #[test]
    fn test_realistic_lambda_in_filter() {
        // (int x) => x > 5 — lambda type inference
        let mut checker = TypeChecker::new();
        let expr = Expression::Lambda {
            params: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None }],
            return_ty: None,
            body: Box::new(LambdaBody::Expression(Expression::BinaryOp {
                left: Box::new(Expression::Identifier("x".to_string())),
                operator: BinaryOperator::Gt,
                right: Box::new(Expression::Int(5)),
            })),
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_ok(), "Lambda should type-check: {:?}", result);
    }

    #[test]
    fn test_realistic_undeclared_variable_in_method() {
        // Using undeclared variable should error
        let mut checker = TypeChecker::new();
        let block = Block { statements: vec![
            Statement::Let {
                name: "x".to_string(),
                ty: None,
                value: Expression::BinaryOp {
                    left: Box::new(Expression::Identifier("undefined_var".to_string())),
                    operator: BinaryOperator::Add,
                    right: Box::new(Expression::Int(1)),
                },
            },
        ]};
        assert!(checker.check_block(&block).is_err(), "Undeclared variable should fail");
    }

    #[test]
    fn test_realistic_if_condition_must_be_bool() {
        // if x + 1 { ... } — non-bool condition must fail
        let mut checker = TypeChecker::new();
        checker.env.insert("x".to_string(), TypeNode::Int);
        let block = Block { statements: vec![
            Statement::If {
                condition: Expression::BinaryOp {
                    left: Box::new(Expression::Identifier("x".to_string())),
                    operator: BinaryOperator::Add,
                    right: Box::new(Expression::Int(1)),
                },
                then_block: Block { statements: vec![] },
                else_block: None,
            },
        ]};
        assert!(checker.check_block(&block).is_err(), "Non-bool if condition should fail");
    }

    #[test]
    fn test_realistic_while_condition_must_be_bool() {
        // while x { ... } where x is int — must fail
        let mut checker = TypeChecker::new();
        checker.env.insert("counter".to_string(), TypeNode::Int);
        let block = Block { statements: vec![
            Statement::While {
                condition: Expression::Identifier("counter".to_string()),
                body: Block { statements: vec![] },
            },
        ]};
        assert!(checker.check_block(&block).is_err(), "Non-bool while condition should fail");
    }

    #[test]
    fn test_realistic_cast_chain() {
        // (x as float) as int — chained cast should return int
        let mut checker = TypeChecker::new();
        checker.env.insert("x".to_string(), TypeNode::Int);
        let expr = Expression::Cast {
            expr: Box::new(Expression::Cast {
                expr: Box::new(Expression::Identifier("x".to_string())),
                target_type: TypeNode::Float,
            }),
            target_type: TypeNode::Int,
        };
        let result = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(result, TypeNode::Int);
    }

    #[test]
    fn test_realistic_generic_struct_with_type_params() {
        // Struct with generic type params — verify struct registration
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Struct(StructDef {
                    name: "Container".to_string(),
                    is_public: true,
                    type_params: vec!["T".to_string()],
                    fields: vec![FieldDecl { name: "value".to_string(), ty: TypeNode::TypeVar("T".to_string()), default_value: None }],
                }),
                Item::Agent(AgentDef {
                    name: "Test".to_string(),
                    is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![],
                    implements: vec![],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "x".to_string(),
                                ty: Some(TypeNode::Int),
                                value: Expression::Int(42),
                            },
                        ]}),
                    }],
                }),
            ],
        };
        assert!(checker.check_program(&program).is_ok());
        // Verify the generic struct was registered
        assert!(checker.generic_structs.contains_key("Container"));
    }

    // ===== Wave 12: TypeChecker Method Validation Tests =====

    #[test]
    fn test_abs_preserves_type() {
        let mut checker = TypeChecker::new();
        checker.env.insert("x".to_string(), TypeNode::Int);
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("x".to_string())),
            method_name: "abs".to_string(),
            args: vec![],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::Int);

        checker.env.insert("f".to_string(), TypeNode::Float);
        let expr2 = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("f".to_string())),
            method_name: "abs".to_string(),
            args: vec![],
        };
        assert_eq!(checker.infer_expression_type(&expr2).unwrap(), TypeNode::Float);
    }

    #[test]
    fn test_sqrt_returns_float() {
        let mut checker = TypeChecker::new();
        checker.env.insert("x".to_string(), TypeNode::Float);
        for method in &["sqrt", "floor", "ceil", "round"] {
            let expr = Expression::MethodCall {
                caller: Box::new(Expression::Identifier("x".to_string())),
                method_name: method.to_string(),
                args: vec![],
            };
            assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::Float,
                "{} should return Float", method);
        }
    }

    #[test]
    fn test_min_max_validate_args() {
        let mut checker = TypeChecker::new();
        checker.env.insert("x".to_string(), TypeNode::Int);
        // Valid: 1 arg
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("x".to_string())),
            method_name: "min".to_string(),
            args: vec![Expression::Int(5)],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::Int);
        // Invalid: 0 args
        let bad = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("x".to_string())),
            method_name: "max".to_string(),
            args: vec![],
        };
        assert!(checker.infer_expression_type(&bad).is_err());
    }

    #[test]
    fn test_parse_int_float_return_types() {
        let mut checker = TypeChecker::new();
        checker.env.insert("s".to_string(), TypeNode::String);
        let expr_int = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("s".to_string())),
            method_name: "parse_int".to_string(),
            args: vec![],
        };
        assert_eq!(checker.infer_expression_type(&expr_int).unwrap(), TypeNode::Int);

        let expr_float = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("s".to_string())),
            method_name: "parse_float".to_string(),
            args: vec![],
        };
        assert_eq!(checker.infer_expression_type(&expr_float).unwrap(), TypeNode::Float);
    }

    #[test]
    fn test_to_string_returns_string() {
        let mut checker = TypeChecker::new();
        checker.env.insert("x".to_string(), TypeNode::Int);
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("x".to_string())),
            method_name: "to_string".to_string(),
            args: vec![],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::String);
    }

    #[test]
    fn test_sort_returns_void() {
        let mut checker = TypeChecker::new();
        checker.env.insert("items".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "sort".to_string(),
            args: vec![],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::Void);
    }

    #[test]
    fn test_join_returns_string() {
        let mut checker = TypeChecker::new();
        checker.env.insert("items".to_string(), TypeNode::Array(Box::new(TypeNode::String)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "join".to_string(),
            args: vec![Expression::String(", ".to_string())],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::String);
        // Invalid: 0 args
        let bad = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "join".to_string(),
            args: vec![],
        };
        assert!(checker.infer_expression_type(&bad).is_err());
    }

    #[test]
    fn test_count_returns_int() {
        let mut checker = TypeChecker::new();
        checker.env.insert("items".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "count".to_string(),
            args: vec![],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::Int);
    }

    #[test]
    fn test_filter_preserves_type() {
        let mut checker = TypeChecker::new();
        checker.env.insert("items".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "filter".to_string(),
            args: vec![Expression::Lambda {
                params: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None }],
                return_ty: None,
                body: Box::new(LambdaBody::Expression(Expression::Bool(true))),
            }],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::Array(Box::new(TypeNode::Int)));
    }

    #[test]
    fn test_map_returns_dynamic_array() {
        let mut checker = TypeChecker::new();
        checker.env.insert("items".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "map".to_string(),
            args: vec![Expression::Lambda {
                params: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None }],
                return_ty: None,
                body: Box::new(LambdaBody::Expression(Expression::String("a".to_string()))),
            }],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::Array(Box::new(TypeNode::Custom("Dynamic".to_string()))));
    }

    #[test]
    fn test_find_returns_nullable_element() {
        let mut checker = TypeChecker::new();
        checker.env.insert("items".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("items".to_string())),
            method_name: "find".to_string(),
            args: vec![Expression::Lambda {
                params: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None }],
                return_ty: None,
                body: Box::new(LambdaBody::Expression(Expression::Bool(true))),
            }],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::Nullable(Box::new(TypeNode::Int)));
    }

    #[test]
    fn test_any_all_return_bool() {
        let mut checker = TypeChecker::new();
        checker.env.insert("items".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        let closure = Expression::Lambda {
            params: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None }],
            return_ty: None,
            body: Box::new(LambdaBody::Expression(Expression::Bool(true))),
        };
        for method in &["any", "all"] {
            let expr = Expression::MethodCall {
                caller: Box::new(Expression::Identifier("items".to_string())),
                method_name: method.to_string(),
                args: vec![closure.clone()],
            };
            assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::Bool,
                "{} should return Bool", method);
        }
    }

    #[test]
    fn test_iterator_methods_require_one_arg() {
        let mut checker = TypeChecker::new();
        checker.env.insert("items".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        for method in &["filter", "map", "flat_map", "find", "any", "all"] {
            let bad = Expression::MethodCall {
                caller: Box::new(Expression::Identifier("items".to_string())),
                method_name: method.to_string(),
                args: vec![],
            };
            assert!(checker.infer_expression_type(&bad).is_err(),
                "{} should fail with 0 args", method);
        }
    }

    // ===== Wave 12: Multi-Error TypeChecker Tests =====

    #[test]
    fn test_multi_error_collects_multiple_agent_errors() {
        // Two agents with separate errors — both should be reported
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "Agent1".to_string(),
                    is_system: false, is_public: true,
                    target_annotation: None, annotations: vec![],
                    implements: vec!["NonexistentContract".to_string()],
                    fields: vec![],
                    methods: vec![],
                }),
                Item::Agent(AgentDef {
                    name: "Agent2".to_string(),
                    is_system: false, is_public: true,
                    target_annotation: None, annotations: vec![],
                    implements: vec!["AlsoNonexistent".to_string()],
                    fields: vec![],
                    methods: vec![],
                }),
            ],
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.len() >= 2, "Expected at least 2 errors, got {}: {:?}", errors.len(), errors);
    }

    #[test]
    fn test_multi_error_no_errors_returns_ok() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "GoodAgent".to_string(),
                    is_system: false, is_public: true,
                    target_annotation: None, annotations: vec![],
                    implements: vec![],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![],
                        constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Print(Expression::String("hello".to_string())),
                        ]}),
                    }],
                }),
            ],
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    // ===== Wave 12: Struct Literal TypeChecker Tests =====

    #[test]
    fn test_struct_literal_type_inference() {
        let mut checker = TypeChecker::new();
        checker.struct_fields.insert("Point".to_string(), vec![
            FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None },
            FieldDecl { name: "y".to_string(), ty: TypeNode::Int, default_value: None },
        ]);
        let expr = Expression::StructLiteral {
            type_name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Expression::Int(5)),
                ("y".to_string(), Expression::Int(10)),
            ],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Custom("Point".to_string()));
    }

    #[test]
    fn test_struct_literal_unknown_field_error() {
        let mut checker = TypeChecker::new();
        checker.struct_fields.insert("Point".to_string(), vec![
            FieldDecl { name: "x".to_string(), ty: TypeNode::Int, default_value: None },
        ]);
        let expr = Expression::StructLiteral {
            type_name: "Point".to_string(),
            fields: vec![
                ("z".to_string(), Expression::Int(5)),
            ],
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TypeError::UnknownField { .. }));
    }

    // ===== Wave 12: Enum Construct TypeChecker Tests =====

    #[test]
    fn test_bare_ok_returns_result_type() {
        let mut checker = TypeChecker::new();
        let expr = Expression::EnumConstruct {
            enum_name: String::new(),
            variant_name: "Ok".to_string(),
            args: vec![Expression::String("success".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert!(matches!(ty, TypeNode::Result(_, _)));
    }

    #[test]
    fn test_bare_some_returns_nullable_type() {
        let mut checker = TypeChecker::new();
        let expr = Expression::EnumConstruct {
            enum_name: String::new(),
            variant_name: "Some".to_string(),
            args: vec![Expression::Int(42)],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert!(matches!(ty, TypeNode::Nullable(_)));
    }

    #[test]
    fn test_qualified_enum_construct_validates() {
        let mut checker = TypeChecker::new();
        checker.enum_defs.insert("Color".to_string(), vec![
            EnumVariant { name: "Red".to_string(), fields: vec![] },
            EnumVariant { name: "Blue".to_string(), fields: vec![] },
        ]);
        // Valid variant
        let expr = Expression::EnumConstruct {
            enum_name: "Color".to_string(),
            variant_name: "Red".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Custom("Color".to_string()));

        // Invalid variant
        let bad = Expression::EnumConstruct {
            enum_name: "Color".to_string(),
            variant_name: "Green".to_string(),
            args: vec![],
        };
        assert!(checker.infer_expression_type(&bad).is_err());
    }

    // ===== Wave 12 Phase 5: Source Span Tests =====

    #[test]
    fn test_spanned_error_has_span_for_undeclared_variable() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "SpanAgent".to_string(),
                    is_system: false, is_public: true,
                    target_annotation: None, annotations: vec![],
                    implements: vec![],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![],
                        constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Expr(Expression::Identifier("unknown_var".to_string())),
                        ]}),
                    }],
                }),
            ],
        };
        let mut checker = TypeChecker::new();
        checker.set_source("agent SpanAgent { public void Run() { unknown_var; } }");
        let result = checker.check_program(&program);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        // Span should point to "unknown_var" in the source
        assert!(errors[0].span.is_some(), "Expected span to be set");
        let span = errors[0].span.as_ref().unwrap();
        assert_eq!(&"agent SpanAgent { public void Run() { unknown_var; } }"[span.start..span.end], "unknown_var");
    }

    #[test]
    fn test_spanned_error_falls_back_to_item_span() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "FallbackAgent".to_string(),
                    is_system: false, is_public: true,
                    target_annotation: None, annotations: vec![],
                    implements: vec![],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![],
                        constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Int),
                        body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "x".to_string(),
                                ty: Some(TypeNode::Int),
                                value: Expression::String("oops".to_string()),
                            },
                        ]}),
                    }],
                }),
            ],
        };
        let mut checker = TypeChecker::new();
        checker.set_source("agent FallbackAgent { public int Run() { let int x = \"oops\"; } }");
        let result = checker.check_program(&program);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        // Should have a span (falls back to item name since TypeMismatch has no search_hint)
        assert!(errors[0].span.is_some(), "Expected fallback span");
    }

    #[test]
    fn test_spanned_error_none_without_source() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "NoSource".to_string(),
                    is_system: false, is_public: true,
                    target_annotation: None, annotations: vec![],
                    implements: vec![],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![],
                        constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Expr(Expression::Identifier("missing".to_string())),
                        ]}),
                    }],
                }),
            ],
        };
        let mut checker = TypeChecker::new();
        // No set_source call
        let result = checker.check_program(&program);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].span.is_none(), "Expected no span when source is not set");
    }

    // ===== Wave 13: Did-You-Mean Suggestion Tests =====

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("hello", "hello"), 0);
        assert_eq!(levenshtein("abc", "ab"), 1);
        assert_eq!(levenshtein("lenght", "length"), 2); // transposition = 2 edits
        assert_eq!(levenshtein("naem", "name"), 2);
    }

    #[test]
    fn test_suggest_similar_finds_close_match() {
        let candidates = vec!["name", "age", "email", "address"];
        let suggestions = suggest_similar("naem", &candidates);
        assert!(suggestions.contains(&"name".to_string()), "Should suggest 'name' for 'naem': {:?}", suggestions);
    }

    #[test]
    fn test_suggest_similar_no_match_for_distant_name() {
        let candidates = vec!["name", "age", "email"];
        let suggestions = suggest_similar("zzzzzzzzz", &candidates);
        assert!(suggestions.is_empty(), "Should have no suggestions for completely different name");
    }

    #[test]
    fn test_undeclared_variable_with_suggestion() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Agent(AgentDef {
                    name: "SuggestAgent".to_string(),
                    is_system: false, is_public: true,
                    target_annotation: None, annotations: vec![],
                    implements: vec![],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![],
                        constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Let { name: "counter".to_string(), ty: Some(TypeNode::Int), value: Expression::Int(0) },
                            // Typo: "conter" instead of "counter"
                            Statement::Expr(Expression::Identifier("conter".to_string())),
                        ]}),
                    }],
                }),
            ],
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err());
        let err = &result.unwrap_err()[0].error;
        let msg = err.message();
        assert!(msg.contains("did you mean"), "Error should contain suggestion: {}", msg);
        assert!(msg.contains("counter"), "Error should suggest 'counter': {}", msg);
    }

    #[test]
    fn test_unknown_field_with_suggestion() {
        let mut checker = TypeChecker::new();
        checker.struct_fields.insert("User".to_string(), vec![
            FieldDecl { name: "name".to_string(), ty: TypeNode::String, default_value: None },
            FieldDecl { name: "email".to_string(), ty: TypeNode::String, default_value: None },
        ]);
        let expr = Expression::PropertyAccess {
            caller: Box::new(Expression::Identifier("u".to_string())),
            property_name: "naem".to_string(),
        };
        checker.env.insert("u".to_string(), TypeNode::Custom("User".to_string()));
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_err());
        let msg = result.unwrap_err().message();
        assert!(msg.contains("did you mean"), "Error should contain suggestion: {}", msg);
        assert!(msg.contains("name"), "Error should suggest 'name': {}", msg);
    }

    #[test]
    fn test_unknown_method_with_suggestion() {
        let mut checker = TypeChecker::new();
        let mut methods = HashMap::new();
        methods.insert("Calculate".to_string(), MethodSignature {
            return_ty: Some(TypeNode::Int),
            args: vec![],
        });
        checker.method_signatures.insert("MathHelper".to_string(), methods);
        checker.env.insert("m".to_string(), TypeNode::Custom("MathHelper".to_string()));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("m".to_string())),
            method_name: "Calculte".to_string(),
            args: vec![],
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_err());
        let msg = result.unwrap_err().message();
        assert!(msg.contains("did you mean"), "Error should contain suggestion: {}", msg);
        assert!(msg.contains("Calculate"), "Error should suggest 'Calculate': {}", msg);
    }

    #[test]
    fn test_builtin_method_suggestion() {
        // Typo on a builtin method
        let mut checker = TypeChecker::new();
        checker.env.insert("arr".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("arr".to_string())),
            method_name: "lenght".to_string(),
            args: vec![],
        };
        let _ = checker.infer_expression_type(&expr);
        // For Custom types with registered methods, unknown methods get suggestions
        checker.env.insert("obj".to_string(), TypeNode::Custom("MyType".to_string()));
        // Register type with some methods so UnknownMethod triggers
        checker.method_signatures.insert("MyType".to_string(), HashMap::new());
        let expr2 = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("obj".to_string())),
            method_name: "lenght".to_string(),
            args: vec![],
        };
        let result2 = checker.infer_expression_type(&expr2);
        assert!(result2.is_err());
        let msg = result2.unwrap_err().message();
        assert!(msg.contains("did you mean"), "Error should contain builtin suggestion: {}", msg);
        assert!(msg.contains("length"), "Error should suggest 'length': {}", msg);
    }

    // ===== Wave 13: Contract Default Implementation Tests =====

    #[test]
    fn test_contract_default_impl_agent_can_skip() {
        // Contract with default method — agent doesn't need to implement it
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Contract(ContractDef {
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
                            body: None, // abstract — must be implemented
                        },
                        MethodDecl {
                            name: "format".to_string(),
                            is_public: true, is_async: false,
                            annotations: vec![], type_params: vec![], constraints: vec![],
                            args: vec![FieldDecl { name: "msg".to_string(), ty: TypeNode::String, default_value: None }],
                            return_ty: Some(TypeNode::String),
                            body: Some(Block { statements: vec![
                                Statement::Return(Some(Expression::Identifier("msg".to_string()))),
                            ]}), // default implementation
                        },
                    ],
                }),
                Item::Agent(AgentDef {
                    name: "MyLogger".to_string(),
                    is_system: false, is_public: true,
                    target_annotation: None, annotations: vec![],
                    implements: vec!["Logger".to_string()],
                    fields: vec![],
                    methods: vec![
                        // Only implements `log`, skips `format` (has default)
                        MethodDecl {
                            name: "log".to_string(),
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
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok(), "Agent should be able to skip methods with defaults");
    }

    #[test]
    fn test_contract_no_default_still_required() {
        // Contract without default — agent MUST implement
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Contract(ContractDef {
                    name: "Runnable".to_string(),
                    is_public: false,
                    target_annotation: None,
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: None, // no default
                    }],
                }),
                Item::Agent(AgentDef {
                    name: "LazyAgent".to_string(),
                    is_system: false, is_public: true,
                    target_annotation: None, annotations: vec![],
                    implements: vec!["Runnable".to_string()],
                    fields: vec![],
                    methods: vec![], // Missing Run!
                }),
            ],
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err(), "Should error when required method is missing");
    }

    // ===== Wave 13: impl Blocks for Structs =====

    #[test]
    fn test_impl_block_registers_methods() {
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
                            Statement::Return(Some(Expression::BinaryOp {
                                left: Box::new(Expression::PropertyAccess {
                                    caller: Box::new(Expression::Identifier("self".to_string())),
                                    property_name: "x".to_string(),
                                }),
                                operator: BinaryOperator::Add,
                                right: Box::new(Expression::PropertyAccess {
                                    caller: Box::new(Expression::Identifier("self".to_string())),
                                    property_name: "y".to_string(),
                                }),
                            }))
                        ]}),
                    }],
                },
            ],
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_impl_block_method_call_on_struct() {
        // Test that after impl, we can call the method on a struct variable
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Struct(StructDef {
                    name: "Counter".to_string(),
                    is_public: false,
                    type_params: vec![],
                    fields: vec![
                        FieldDecl { name: "count".to_string(), ty: TypeNode::Int, default_value: None },
                    ],
                }),
                Item::Impl {
                    type_name: "Counter".to_string(),
                    type_params: vec![],
                    methods: vec![MethodDecl {
                        name: "get_count".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Int),
                        body: Some(Block { statements: vec![
                            Statement::Return(Some(Expression::PropertyAccess {
                                caller: Box::new(Expression::Identifier("self".to_string())),
                                property_name: "count".to_string(),
                            }))
                        ]}),
                    }],
                },
                Item::Agent(AgentDef {
                    name: "App".to_string(),
                    is_system: false, is_public: false,
                    target_annotation: None, annotations: vec![],
                    implements: vec![],
                    fields: vec![],
                    methods: vec![MethodDecl {
                        name: "Run".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![
                            Statement::Let {
                                name: "c".to_string(),
                                ty: Some(TypeNode::Custom("Counter".to_string())),
                                value: Expression::StructLiteral {
                                    type_name: "Counter".to_string(),
                                    fields: vec![("count".to_string(), Expression::Int(5))],
                                },
                            },
                            Statement::Let {
                                name: "val".to_string(),
                                ty: None,
                                value: Expression::MethodCall {
                                    caller: Box::new(Expression::Identifier("c".to_string())),
                                    method_name: "get_count".to_string(),
                                    args: vec![],
                                },
                            },
                        ]}),
                    }],
                }),
            ],
        };
        let mut checker = TypeChecker::new();
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_impl_block_nonexistent_struct_error() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Impl {
                    type_name: "Ghost".to_string(),
                    type_params: vec![],
                    methods: vec![MethodDecl {
                        name: "boo".to_string(),
                        is_public: true, is_async: false,
                        annotations: vec![], type_params: vec![], constraints: vec![],
                        args: vec![],
                        return_ty: Some(TypeNode::Void),
                        body: Some(Block { statements: vec![] }),
                    }],
                },
            ],
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_err(), "impl for nonexistent struct should fail");
    }

    // ===== Wave 13: Stdlib Expansion Tests =====

    #[test]
    fn test_stdlib_fs_read_requires_capability() {
        let mut checker = TypeChecker::new();
        // Without FileAccess capability, fs_read should fail
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_read".to_string(),
            args: vec![Expression::String("test.txt".to_string())],
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_stdlib_fs_read_with_capability() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true; // unsafe bypasses OCAP
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_read".to_string(),
            args: vec![Expression::String("test.txt".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        // Wave 14: fs_read now returns Result<String, String>
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)));
    }

    // ===== Wave 15: fs_append + fs_read_lines =====

    #[test]
    fn test_stdlib_fs_append_type() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_append".to_string(),
            args: vec![Expression::String("log.txt".to_string()), Expression::String("line\n".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_stdlib_fs_read_lines_type() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_read_lines".to_string(),
            args: vec![Expression::String("data.txt".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::Array(Box::new(TypeNode::String))), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_stdlib_fs_append_requires_capability() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_append".to_string(),
            args: vec![Expression::String("log.txt".to_string()), Expression::String("data".to_string())],
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_err(), "fs_append without FileAccess should fail");
    }

    // ===== Wave 15: Shell Command Execution =====

    #[test]
    fn test_stdlib_exec_type() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "exec".to_string(),
            args: vec![Expression::String("echo hello".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_stdlib_exec_status_type() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "exec_status".to_string(),
            args: vec![Expression::String("ls".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_stdlib_exec_requires_system_access() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "exec".to_string(),
            args: vec![Expression::String("echo hello".to_string())],
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_err(), "exec without SystemAccess should fail");
    }

    // ===== Wave 15: Test Framework — assert builtins =====

    #[test]
    fn test_stdlib_assert_type() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "assert".to_string(),
            args: vec![Expression::Bool(true), Expression::String("should pass".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Void);
    }

    #[test]
    fn test_stdlib_assert_requires_bool_condition() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "assert".to_string(),
            args: vec![Expression::String("not bool".to_string()), Expression::String("msg".to_string())],
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_err(), "assert with non-bool condition should fail");
    }

    #[test]
    fn test_stdlib_assert_eq_type() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "assert_eq".to_string(),
            args: vec![Expression::Int(5), Expression::Int(5), Expression::String("should match".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Void);
    }

    // ===== Wave 15: Typed JSON =====

    #[test]
    fn test_stdlib_json_parse_type() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "json_parse".to_string(),
            args: vec![Expression::String("{\"key\": \"value\"}".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::JsonValue), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_stdlib_json_get_type() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "json_get".to_string(),
            args: vec![Expression::Identifier("json".to_string()), Expression::String("/name".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_stdlib_json_get_int_type() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "json_get_int".to_string(),
            args: vec![Expression::Identifier("json".to_string()), Expression::String("/age".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Int);
    }

    #[test]
    fn test_stdlib_json_get_array_type() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "json_get_array".to_string(),
            args: vec![Expression::Identifier("json".to_string()), Expression::String("/tags".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Array(Box::new(TypeNode::String)));
    }

    // ===== Wave 15: HTTP Response with Status =====

    #[test]
    fn test_stdlib_http_request_type() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "http_request".to_string(),
            args: vec![Expression::String("https://api.example.com".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_stdlib_http_request_requires_network_access() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "http_request".to_string(),
            args: vec![Expression::String("https://api.example.com".to_string())],
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_err(), "http_request without NetworkAccess should fail");
    }

    #[test]
    fn test_stdlib_path_exists_returns_bool() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "path_exists".to_string(),
            args: vec![Expression::String("/tmp".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Bool);
    }

    #[test]
    fn test_stdlib_path_join_returns_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "path_join".to_string(),
            args: vec![Expression::String("/home".to_string()), Expression::String("user".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_stdlib_regex_match_returns_result_bool() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "regex_match".to_string(),
            args: vec![Expression::String("\\d+".to_string()), Expression::String("abc123".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        // Wave 14: regex_match returns Result<Bool, String>
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::Bool), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_stdlib_regex_find_all_returns_result_array() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "regex_find_all".to_string(),
            args: vec![Expression::String("\\d+".to_string()), Expression::String("a1b2c3".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        // Wave 14: regex_find_all returns Result<string[], String>
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::Array(Box::new(TypeNode::String))), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_stdlib_regex_replace_returns_result_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "regex_replace".to_string(),
            args: vec![
                Expression::String("\\d+".to_string()),
                Expression::String("a1b2".to_string()),
                Expression::String("X".to_string()),
            ],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        // Wave 14: regex_replace returns Result<String, String>
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_stdlib_sleep_returns_void() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "sleep".to_string(),
            args: vec![Expression::Int(1000)],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Void);
    }

    #[test]
    fn test_stdlib_timestamp_returns_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "timestamp".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_stdlib_fs_read_dir_returns_result_string_array() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_read_dir".to_string(),
            args: vec![Expression::String("/tmp".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        // Wave 14: fs_read_dir returns Result<string[], String>
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::Array(Box::new(TypeNode::String))), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_stdlib_path_parent_returns_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "path_parent".to_string(),
            args: vec![Expression::String("/home/user/file.txt".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_stdlib_wrong_arg_count() {
        let mut checker = TypeChecker::new();
        // fs_read with 0 args should fail
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "path_join".to_string(),
            args: vec![Expression::String("only_one".to_string())],
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_err(), "path_join needs 2 args");
    }

    // ===== Wave 16: for (k, v) in map =====

    #[test]
    fn test_foreach_map_kv_destructure() {
        let mut checker = TypeChecker::new();
        checker.env.insert("config".to_string(), TypeNode::Map(
            Box::new(TypeNode::String), Box::new(TypeNode::Int),
        ));
        let block = Block { statements: vec![
            Statement::Foreach {
                item_name: "key".to_string(),
                value_name: Some("val".to_string()),
                collection: Expression::Identifier("config".to_string()),
                body: Block { statements: vec![
                    // key should be string, val should be int
                    Statement::Let {
                        name: "msg".to_string(),
                        ty: Some(TypeNode::String),
                        value: Expression::Identifier("key".to_string()),
                    },
                    Statement::Let {
                        name: "num".to_string(),
                        ty: Some(TypeNode::Int),
                        value: Expression::Identifier("val".to_string()),
                    },
                ]},
            },
        ]};
        assert!(checker.check_block(&block).is_ok(), "Map KV destructure should typecheck");
    }

    #[test]
    fn test_foreach_kv_on_non_map_fails() {
        let mut checker = TypeChecker::new();
        checker.env.insert("items".to_string(), TypeNode::Array(Box::new(TypeNode::Int)));
        let block = Block { statements: vec![
            Statement::Foreach {
                item_name: "k".to_string(),
                value_name: Some("v".to_string()),
                collection: Expression::Identifier("items".to_string()),
                body: Block { statements: vec![] },
            },
        ]};
        assert!(checker.check_block(&block).is_err(), "KV destructure on Array should fail");
    }

    // ---- F41-6: Dependency Injection Tests ----

    #[test]
    fn test_di_contract_type_compatibility() {
        let mut checker = TypeChecker::new();
        checker.known_contracts.insert("ILogger".to_string(), ContractDef {
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
        });
        checker.agent_implements.insert("ConsoleLogger".to_string(), vec!["ILogger".to_string()]);
        assert!(checker.types_match(
            &TypeNode::Custom("ILogger".to_string()),
            &TypeNode::Custom("ConsoleLogger".to_string())
        ), "Agent implementing contract should be type-compatible");
    }

    #[test]
    fn test_di_non_implementing_agent_incompatible() {
        let mut checker = TypeChecker::new();
        checker.known_contracts.insert("ILogger".to_string(), ContractDef {
            name: "ILogger".to_string(),
            is_public: false,
            target_annotation: None,
            methods: vec![],
        });
        assert!(!checker.types_match(
            &TypeNode::Custom("ILogger".to_string()),
            &TypeNode::Custom("OtherAgent".to_string())
        ), "Agent not implementing contract should NOT be type-compatible");
    }

    #[test]
    fn test_di_contract_method_resolution() {
        let mut checker = TypeChecker::new();
        checker.known_contracts.insert("IModelClient".to_string(), ContractDef {
            name: "IModelClient".to_string(),
            is_public: false,
            target_annotation: None,
            methods: vec![MethodDecl {
                name: "infer".to_string(),
                args: vec![FieldDecl { name: "prompt".to_string(), ty: TypeNode::String, default_value: None }],
                return_ty: Some(TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String))),
                body: None,
                is_public: true,
                is_async: false,
                annotations: vec![],
                type_params: vec![],
                constraints: vec![],
            }],
        });
        checker.env.insert("client".to_string(), TypeNode::Custom("IModelClient".to_string()));
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("client".to_string())),
            method_name: "infer".to_string(),
            args: vec![Expression::String("hello".to_string())],
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_ok(), "Method call on contract-typed var should resolve: {:?}", result);
        assert_eq!(result.unwrap(), TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)));
    }

    // ===== Issue #4: Regression tests =====

    #[test]
    fn test_issue4_self_in_standalone_fn_no_error() {
        // Bare function calls in standalone fn bodies use synthetic 'self' as caller.
        // The typechecker must not fail with "undeclared variable self".
        let mut checker = TypeChecker::new();
        // Register a helper function
        checker.known_functions.insert("helper".to_string(), MethodSignature {
            return_ty: Some(TypeNode::String),
            args: vec![FieldDecl { name: "x".to_string(), ty: TypeNode::String, default_value: None }],
        });
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Function(FunctionDef {
                name: "main_fn".to_string(),
                is_public: false,
                params: vec![],
                return_ty: Some(TypeNode::String),
                body: Block { statements: vec![
                    // var result = helper("test") — parser makes this MethodCall with caller=self
                    Statement::Let {
                        name: "result".to_string(),
                        ty: None,
                        value: Expression::MethodCall {
                            caller: Box::new(Expression::Identifier("self".to_string())),
                            method_name: "helper".to_string(),
                            args: vec![Expression::String("test".to_string())],
                        },
                    },
                    Statement::Return(Some(Expression::Identifier("result".to_string()))),
                ] },
            })],
        };
        assert!(checker.check_program(&program).is_ok(), "Standalone fn with bare function calls must not fail");
    }

    #[test]
    fn test_issue4_unknown_fn_in_standalone_fn_no_crash() {
        // Even forward-declared / unknown functions should not crash with 'self' error
        let mut checker = TypeChecker::new();
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![Item::Function(FunctionDef {
                name: "caller_fn".to_string(),
                is_public: false,
                params: vec![],
                return_ty: Some(TypeNode::Void),
                body: Block { statements: vec![
                    Statement::Expr(Expression::MethodCall {
                        caller: Box::new(Expression::Identifier("self".to_string())),
                        method_name: "unknown_future_fn".to_string(),
                        args: vec![],
                    }),
                ] },
            })],
        };
        // Should not crash — returns Dynamic for unknown calls
        assert!(checker.check_program(&program).is_ok());
    }

    #[test]
    fn test_issue4_base64_builtins_typecheck() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "base64_encode".to_string(),
            args: vec![Expression::String("hello".to_string())],
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TypeNode::String);
    }

    #[test]
    fn test_issue4_pdf_builtins_typecheck() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "pdf_create".to_string(),
            args: vec![Expression::String("My Doc".to_string())],
        };
        let result = checker.infer_expression_type(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TypeNode::Custom("PdfDocHandle".to_string()));
    }

    // ===== Wave 28: System Primitives Tests =====
    #[test]
    fn test_wave28_args_returns_string_array() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "args".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Array(Box::new(TypeNode::String)));
    }

    #[test]
    fn test_wave28_args_rejects_arguments() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "args".to_string(),
            args: vec![Expression::String("oops".to_string())],
        };
        assert!(checker.infer_expression_type(&expr).is_err());
    }

    #[test]
    fn test_wave28_stdin_read_line_requires_system_access() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "stdin_read_line".to_string(),
            args: vec![],
        };
        assert!(checker.infer_expression_type(&expr).is_err());
    }

    #[test]
    fn test_wave28_stdin_read_line_with_capability() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "stdin_read_line".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_wave28_stdin_read_with_capability() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "stdin_read".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_wave28_is_dir_returns_bool() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "is_dir".to_string(),
            args: vec![Expression::String("/tmp".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Bool);
    }

    #[test]
    fn test_wave28_is_file_returns_bool() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "is_file".to_string(),
            args: vec![Expression::String("/tmp/x".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Bool);
    }

    #[test]
    fn test_wave28_path_resolve_returns_result() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "path_resolve".to_string(),
            args: vec![Expression::String("./foo".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_wave28_fs_copy_requires_capability() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_copy".to_string(),
            args: vec![Expression::String("a".to_string()), Expression::String("b".to_string())],
        };
        assert!(checker.infer_expression_type(&expr).is_err());
    }

    #[test]
    fn test_wave28_fs_copy_with_capability() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_copy".to_string(),
            args: vec![Expression::String("a".to_string()), Expression::String("b".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_wave28_fs_rename_requires_capability() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_rename".to_string(),
            args: vec![Expression::String("a".to_string()), Expression::String("b".to_string())],
        };
        assert!(checker.infer_expression_type(&expr).is_err());
    }

    #[test]
    fn test_wave28_fs_rename_with_capability() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_rename".to_string(),
            args: vec![Expression::String("a".to_string()), Expression::String("b".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String)));
    }

    #[test]
    fn test_wave28_ansi_color_returns_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "ansi_color".to_string(),
            args: vec![Expression::String("red".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_wave28_ansi_bold_zero_args() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "ansi_bold".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_wave28_ansi_reset_zero_args() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "ansi_reset".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    // ===== Wave 28 Batch 2: SSE Client Tests =====

    #[test]
    fn test_wave28b_sse_client_connect_requires_network() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "sse_client_connect".to_string(),
            args: vec![
                Expression::String("https://api.example.com/stream".to_string()),
                Expression::Identifier("headers".to_string()),
            ],
        };
        assert!(checker.infer_expression_type(&expr).is_err(), "should require NetworkAccess");
    }

    #[test]
    fn test_wave28b_sse_client_connect_returns_handle() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "sse_client_connect".to_string(),
            args: vec![
                Expression::String("https://api.example.com/stream".to_string()),
                Expression::Identifier("headers".to_string()),
            ],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(
                Box::new(TypeNode::Custom("SseClientHandle".to_string())),
                Box::new(TypeNode::String)
            )
        );
    }

    #[test]
    fn test_wave28b_sse_client_post_returns_handle() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "sse_client_post".to_string(),
            args: vec![
                Expression::String("https://api.anthropic.com/v1/messages".to_string()),
                Expression::Identifier("headers".to_string()),
                Expression::String("{}".to_string()),
            ],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert!(matches!(ty, TypeNode::Result(_, _)));
    }

    #[test]
    fn test_wave28b_sse_client_next_returns_result_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "sse_client_next".to_string(),
            args: vec![Expression::Identifier("handle".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String))
        );
    }

    #[test]
    fn test_wave28b_sse_client_close_returns_result_void() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "sse_client_close".to_string(),
            args: vec![Expression::Identifier("handle".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String))
        );
    }

    // ===== Wave 28 Batch 2: Process Management Tests =====

    #[test]
    fn test_wave28b_proc_spawn_requires_system_access() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "proc_spawn".to_string(),
            args: vec![Expression::String("echo hi".to_string())],
        };
        assert!(checker.infer_expression_type(&expr).is_err());
    }

    #[test]
    fn test_wave28b_proc_spawn_returns_handle() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "proc_spawn".to_string(),
            args: vec![Expression::String("echo hi".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(
                Box::new(TypeNode::Custom("ProcHandle".to_string())),
                Box::new(TypeNode::String)
            )
        );
    }

    #[test]
    fn test_wave28b_proc_spawn_args_returns_handle() {
        let mut checker = TypeChecker::new();
        checker.in_unsafe_block = true;
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "proc_spawn_args".to_string(),
            args: vec![
                Expression::String("python".to_string()),
                Expression::Identifier("argv".to_string()),
            ],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert!(matches!(ty, TypeNode::Result(_, _)));
    }

    #[test]
    fn test_wave28b_proc_read_line_returns_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "proc_read_line".to_string(),
            args: vec![Expression::Identifier("handle".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String))
        );
    }

    #[test]
    fn test_wave28b_proc_write_stdin_returns_void() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "proc_write_stdin".to_string(),
            args: vec![
                Expression::Identifier("handle".to_string()),
                Expression::String("data".to_string()),
            ],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String))
        );
    }

    #[test]
    fn test_wave28b_proc_wait_returns_int() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "proc_wait".to_string(),
            args: vec![Expression::Identifier("handle".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String))
        );
    }

    #[test]
    fn test_wave28b_proc_kill_returns_void() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "proc_kill".to_string(),
            args: vec![Expression::Identifier("handle".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String))
        );
    }

    #[test]
    fn test_wave28b_proc_is_alive_returns_bool() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "proc_is_alive".to_string(),
            args: vec![Expression::Identifier("handle".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Bool);
    }

    #[test]
    fn test_wave28b_proc_pid_returns_int() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "proc_pid".to_string(),
            args: vec![Expression::Identifier("handle".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::Int);
    }

    // ===== Wave 29: Binary I/O tests =====

    #[test]
    fn test_wave29_fs_read_bytes_requires_file_access() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_read_bytes".to_string(),
            args: vec![Expression::String("/tmp/x.bin".to_string())],
        };
        let err = checker.infer_expression_type(&expr).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    #[test]
    fn test_wave29_fs_read_bytes_returns_result_array_int() {
        let mut checker = TypeChecker::new();
        checker.available_capabilities.push(CapabilityType::FileAccess);
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_read_bytes".to_string(),
            args: vec![Expression::String("/tmp/x.bin".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(
                Box::new(TypeNode::Array(Box::new(TypeNode::Int))),
                Box::new(TypeNode::String)
            )
        );
    }

    #[test]
    fn test_wave29_fs_write_bytes_requires_file_access() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_write_bytes".to_string(),
            args: vec![
                Expression::String("/tmp/x.bin".to_string()),
                Expression::ArrayLiteral(vec![Expression::Int(1), Expression::Int(2)]),
            ],
        };
        let err = checker.infer_expression_type(&expr).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    #[test]
    fn test_wave29_fs_write_bytes_returns_result_int() {
        let mut checker = TypeChecker::new();
        checker.available_capabilities.push(CapabilityType::FileAccess);
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_write_bytes".to_string(),
            args: vec![
                Expression::String("/tmp/x.bin".to_string()),
                Expression::ArrayLiteral(vec![Expression::Int(65), Expression::Int(66)]),
            ],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String))
        );
    }

    #[test]
    fn test_wave29_fs_append_bytes_returns_result_int() {
        let mut checker = TypeChecker::new();
        checker.available_capabilities.push(CapabilityType::FileAccess);
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_append_bytes".to_string(),
            args: vec![
                Expression::String("/tmp/log.bin".to_string()),
                Expression::ArrayLiteral(vec![Expression::Int(0)]),
            ],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String))
        );
    }

    #[test]
    fn test_wave29_fs_size_returns_result_int() {
        let mut checker = TypeChecker::new();
        checker.available_capabilities.push(CapabilityType::FileAccess);
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "fs_size".to_string(),
            args: vec![Expression::String("/tmp/x.bin".to_string())],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::String))
        );
    }

    #[test]
    fn test_wave29_home_dir_returns_string_no_ocap() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "home_dir".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_wave29_config_dir_returns_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "config_dir".to_string(),
            args: vec![],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::String);
    }

    #[test]
    fn test_wave29_data_dir_returns_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "data_dir".to_string(),
            args: vec![],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::String);
    }

    #[test]
    fn test_wave29_cache_dir_returns_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "cache_dir".to_string(),
            args: vec![],
        };
        assert_eq!(checker.infer_expression_type(&expr).unwrap(), TypeNode::String);
    }

    #[test]
    fn test_wave29_config_load_cascade_requires_file_access() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "config_load_cascade".to_string(),
            args: vec![Expression::ArrayLiteral(vec![
                Expression::String("/a.json".to_string()),
            ])],
        };
        let err = checker.infer_expression_type(&expr).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    #[test]
    fn test_wave29_config_load_cascade_returns_result_string() {
        let mut checker = TypeChecker::new();
        checker.available_capabilities.push(CapabilityType::FileAccess);
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "config_load_cascade".to_string(),
            args: vec![Expression::ArrayLiteral(vec![
                Expression::String("/a.json".to_string()),
            ])],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String))
        );
    }

    #[test]
    fn test_wave29_readline_new_requires_system_access() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "readline_new".to_string(),
            args: vec![],
        };
        let err = checker.infer_expression_type(&expr).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    #[test]
    fn test_wave29_readline_new_returns_handle() {
        let mut checker = TypeChecker::new();
        checker.available_capabilities.push(CapabilityType::SystemAccess);
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "readline_new".to_string(),
            args: vec![],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(
                Box::new(TypeNode::Custom("ReadlineHandle".to_string())),
                Box::new(TypeNode::String)
            )
        );
    }

    #[test]
    fn test_wave29_readline_read_returns_result_string() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "readline_read".to_string(),
            args: vec![
                Expression::Identifier("handle".to_string()),
                Expression::String("> ".to_string()),
            ],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::String))
        );
    }

    #[test]
    fn test_wave29_readline_add_history_returns_result_void() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "readline_add_history".to_string(),
            args: vec![
                Expression::Identifier("handle".to_string()),
                Expression::String("ls".to_string()),
            ],
        };
        let ty = checker.infer_expression_type(&expr).unwrap();
        assert_eq!(
            ty,
            TypeNode::Result(Box::new(TypeNode::Void), Box::new(TypeNode::String))
        );
    }

    #[test]
    fn test_wave29_readline_load_history_requires_file_access() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "readline_load_history".to_string(),
            args: vec![
                Expression::Identifier("handle".to_string()),
                Expression::String("/tmp/hist".to_string()),
            ],
        };
        let err = checker.infer_expression_type(&expr).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    #[test]
    fn test_wave29_readline_save_history_requires_file_access() {
        let mut checker = TypeChecker::new();
        let expr = Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: "readline_save_history".to_string(),
            args: vec![
                Expression::Identifier("handle".to_string()),
                Expression::String("/tmp/hist".to_string()),
            ],
        };
        let err = checker.infer_expression_type(&expr).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    // ===== Adversarial / Error Path Tests — Waves 30-34 =====

    fn call(method: &str, args: Vec<Expression>) -> Expression {
        Expression::MethodCall {
            caller: Box::new(Expression::Identifier("self".to_string())),
            method_name: method.to_string(),
            args,
        }
    }

    fn ident(name: &str) -> Expression { Expression::Identifier(name.to_string()) }
    fn str_lit(s: &str) -> Expression { Expression::String(s.to_string()) }
    fn int_lit(n: i64) -> Expression { Expression::Int(n) }

    // ── Wrong argument counts ────────────────────────────────────────────────

    #[test]
    fn test_tc_await_approval_wrong_arg_count() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("await_approval", vec![])).unwrap_err();
        assert!(matches!(err, TypeError::TypeMismatch { .. }), "0-arg await_approval must error");
    }

    #[test]
    fn test_tc_await_choice_wrong_arg_count() {
        let mut c = TypeChecker::new();
        // needs 2 args, give 1
        let err = c.infer_expression_type(&call("await_choice", vec![str_lit("pick one")])).unwrap_err();
        assert!(matches!(err, TypeError::TypeMismatch { .. }));
    }

    #[test]
    fn test_tc_budget_new_wrong_arg_count() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("budget_new", vec![int_lit(100)])).unwrap_err();
        assert!(matches!(err, TypeError::TypeMismatch { .. }), "budget_new needs 2 args");
    }

    #[test]
    fn test_tc_checkpoint_open_wrong_arg_count() {
        let mut c = TypeChecker::new();
        // needs FileAccess capability — but also wrong arg count
        let err = c.infer_expression_type(&call("checkpoint_open", vec![])).unwrap_err();
        // Could be MissingCapability or TypeMismatch depending on check order
        assert!(matches!(err, TypeError::MissingCapability { .. } | TypeError::TypeMismatch { .. }));
    }

    #[test]
    fn test_tc_channel_new_wrong_arg_count() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("channel_new", vec![])).unwrap_err();
        assert!(matches!(err, TypeError::TypeMismatch { .. }), "channel_new needs 1 arg");
    }

    #[test]
    fn test_tc_channel_send_wrong_arg_count() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("channel_send", vec![ident("ch")])).unwrap_err();
        assert!(matches!(err, TypeError::TypeMismatch { .. }), "channel_send needs 2 args");
    }

    #[test]
    fn test_tc_prop_gen_int_wrong_arg_count() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("prop_gen_int", vec![int_lit(0)])).unwrap_err();
        assert!(matches!(err, TypeError::TypeMismatch { .. }), "prop_gen_int needs 2 args");
    }

    #[test]
    fn test_tc_workflow_add_step_wrong_arg_count() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("workflow_add_step", vec![ident("w"), str_lit("step")])).unwrap_err();
        assert!(matches!(err, TypeError::TypeMismatch { .. }), "workflow_add_step needs 3 args");
    }

    #[test]
    fn test_tc_registry_install_wrong_arg_count() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("registry_install", vec![ident("r"), str_lit("pkg")])).unwrap_err();
        assert!(matches!(err, TypeError::TypeMismatch { .. }), "registry_install needs 3 args");
    }

    #[test]
    fn test_tc_llm_structured_wrong_arg_count() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("llm_structured", vec![str_lit("prompt")])).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. } | TypeError::TypeMismatch { .. }));
    }

    // ── OCAP gating ─────────────────────────────────────────────────────────

    #[test]
    fn test_tc_llm_structured_requires_llm_access() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("llm_structured", vec![
            str_lit("prompt"), str_lit("{\"type\":\"string\"}"), int_lit(3)
        ])).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }), "llm_structured must require LlmAccess");
    }

    #[test]
    fn test_tc_llm_stream_requires_llm_access() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("llm_stream", vec![
            str_lit("prompt"), str_lit("gpt-4")
        ])).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    #[test]
    fn test_tc_llm_embed_batch_requires_llm_access() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("llm_embed_batch", vec![ident("texts")])).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    #[test]
    fn test_tc_llm_vision_requires_llm_access() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("llm_vision", vec![
            ident("img"), str_lit("what is this?"), str_lit("gpt-4-vision")
        ])).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    #[test]
    fn test_tc_image_load_requires_file_access() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("image_load", vec![str_lit("photo.jpg")])).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    #[test]
    fn test_tc_audio_load_requires_file_access() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("audio_load", vec![str_lit("clip.mp3")])).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    #[test]
    fn test_tc_checkpoint_open_requires_file_access() {
        let mut c = TypeChecker::new();
        let err = c.infer_expression_type(&call("checkpoint_open", vec![
            str_lit("./ckpt.db"), str_lit("agent1")
        ])).unwrap_err();
        assert!(matches!(err, TypeError::MissingCapability { .. }));
    }

    // ── Return type verification ─────────────────────────────────────────────

    #[test]
    fn test_tc_await_approval_returns_bool() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("await_approval", vec![str_lit("Continue?")])).unwrap();
        assert_eq!(ty, TypeNode::Bool);
    }

    #[test]
    fn test_tc_await_input_returns_string() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("await_input", vec![str_lit("Enter value: ")])).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    #[test]
    fn test_tc_await_choice_returns_int() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("await_choice", vec![
            str_lit("Pick:"), ident("options")
        ])).unwrap();
        assert_eq!(ty, TypeNode::Int);
    }

    #[test]
    fn test_tc_channel_new_returns_channel_handle() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("channel_new", vec![int_lit(10)])).unwrap();
        assert_eq!(ty, TypeNode::Custom("ChannelHandle".to_string()));
    }

    #[test]
    fn test_tc_channel_len_returns_int() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("channel_len", vec![ident("ch")])).unwrap();
        assert_eq!(ty, TypeNode::Int);
    }

    #[test]
    fn test_tc_channel_is_closed_returns_bool() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("channel_is_closed", vec![ident("ch")])).unwrap();
        assert_eq!(ty, TypeNode::Bool);
    }

    #[test]
    fn test_tc_budget_new_returns_budget_handle() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("budget_new", vec![int_lit(10000), int_lit(500)])).unwrap();
        assert_eq!(ty, TypeNode::Custom("BudgetHandle".to_string()));
    }

    #[test]
    fn test_tc_budget_track_returns_bool() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("budget_track", vec![
            ident("b"), str_lit("prompt"), str_lit("response")
        ])).unwrap();
        assert_eq!(ty, TypeNode::Bool);
    }

    #[test]
    fn test_tc_budget_remaining_tokens_returns_int() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("budget_remaining_tokens", vec![ident("b")])).unwrap();
        assert_eq!(ty, TypeNode::Int);
    }

    #[test]
    fn test_tc_workflow_new_returns_workflow_handle() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("workflow_new", vec![str_lit("pipe")])).unwrap();
        assert_eq!(ty, TypeNode::Custom("WorkflowHandle".to_string()));
    }

    #[test]
    fn test_tc_workflow_is_complete_returns_bool() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("workflow_is_complete", vec![ident("w")])).unwrap();
        assert_eq!(ty, TypeNode::Bool);
    }

    #[test]
    fn test_tc_workflow_ready_steps_returns_array_of_string() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("workflow_ready_steps", vec![ident("w")])).unwrap();
        assert_eq!(ty, TypeNode::Array(Box::new(TypeNode::String)));
    }

    #[test]
    fn test_tc_registry_search_returns_array_of_string() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("registry_search", vec![str_lit("http")])).unwrap();
        assert_eq!(ty, TypeNode::Array(Box::new(TypeNode::String)));
    }

    #[test]
    fn test_tc_prop_gen_float_returns_float() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("prop_gen_float", vec![])).unwrap();
        assert_eq!(ty, TypeNode::Float);
    }

    #[test]
    fn test_tc_prop_gen_bool_returns_bool() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("prop_gen_bool", vec![])).unwrap();
        assert_eq!(ty, TypeNode::Bool);
    }

    #[test]
    fn test_tc_prop_gen_int_list_returns_array_of_int() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("prop_gen_int_list", vec![int_lit(10)])).unwrap();
        assert_eq!(ty, TypeNode::Array(Box::new(TypeNode::Int)));
    }

    #[test]
    fn test_tc_estimate_tokens_returns_int() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("estimate_tokens", vec![str_lit("hello")])).unwrap();
        assert_eq!(ty, TypeNode::Int);
    }

    #[test]
    fn test_tc_sse_event_returns_string() {
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("sse_event", vec![
            str_lit("message"), str_lit("hello")
        ])).unwrap();
        assert_eq!(ty, TypeNode::String);
    }

    // ── Unknown builtin → dynamic fallback (typechecker does not reject unknown calls) ────

    #[test]
    fn test_tc_unknown_builtin_returns_dynamic_not_panic() {
        // The typechecker resolves unknown free function calls to Dynamic rather than erroring.
        // This preserves interop flexibility; actual resolution happens at codegen/link time.
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("nonexistent_builtin_xyz", vec![])).unwrap();
        assert!(matches!(ty, TypeNode::Custom(_)), "unknown builtin must resolve to Dynamic/Custom, not panic");
    }

    #[test]
    fn test_tc_known_builtin_channel_new_resolves() {
        // "channel_new" is a registered builtin and must resolve to a non-error type
        let mut c = TypeChecker::new();
        let ty = c.infer_expression_type(&call("channel_new", vec![
            int_lit(100),
        ])).unwrap();
        // channel_new returns a ChannelHandle (Custom type)
        assert!(matches!(ty, TypeNode::Custom(_)), "channel_new must resolve to a custom handle type");
    }

    // ===== Regression: Issue #7 — enum name must not produce UndeclaredVariable =====
    #[test]
    fn test_tc_enum_identifier_resolves_to_custom_type() {
        // Color.Red: infer_expression_type(Identifier("Color")) must not error
        let mut c = TypeChecker::new();
        c.enum_defs.insert("Color".to_string(), vec![
            EnumVariant { name: "Red".to_string(), fields: vec![] },
        ]);
        let ty = c.infer_expression_type(&Expression::Identifier("Color".to_string())).unwrap();
        assert!(matches!(ty, TypeNode::Custom(ref n) if n == "Color"),
            "enum name must resolve to Custom(\"Color\"), got {:?}", ty);
    }

    #[test]
    fn test_tc_enum_declaration_does_not_error() {
        let program = Program {
            no_std: false, docs: std::collections::HashMap::new(),
            items: vec![
                Item::Enum(EnumDef {
                    name: "Color".to_string(),
                    is_public: false,
                    variants: vec![
                        EnumVariant { name: "Red".to_string(), fields: vec![] },
                        EnumVariant { name: "Green".to_string(), fields: vec![] },
                        EnumVariant { name: "Blue".to_string(), fields: vec![] },
                    ],
                }),
                Item::Function(FunctionDef {
                    name: "get_red".to_string(),
                    is_public: false,
                    params: vec![],
                    return_ty: Some(TypeNode::Custom("Color".to_string())),
                    body: Block { statements: vec![
                        Statement::Return(Some(Expression::PropertyAccess {
                            caller: Box::new(Expression::Identifier("Color".to_string())),
                            property_name: "Red".to_string(),
                        })),
                    ]},
                }),
            ],
        };
        let mut checker = TypeChecker::new();
        let result = checker.check_program(&program);
        assert!(result.is_ok(), "enum declaration + variant access must typecheck: {:?}", result);
    }
}
