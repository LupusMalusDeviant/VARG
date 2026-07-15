//! Shared builtin metadata — a single source of truth that both the typechecker and the
//! codegen can consult, instead of the two maintaining independent per-builtin `if`-chains
//! (which drift, e.g. `env` was typed String but emitted a Result).
//!
//! This first table covers builtins with a **fixed, argument-independent** return type. It is
//! consumed by the codegen's type resolver today; the typechecker can adopt it incrementally.
//! Argument-dependent or generic builtins return `None` (callers fall back to their own logic).

use crate::ast::TypeNode;

/// Return type of a builtin with a fixed return, or `None` if unknown / argument-dependent.
pub fn builtin_return_type(name: &str) -> Option<TypeNode> {
    // Strip the internal `__varg_` prefixes the way the typechecker/codegen do.
    let name = name
        .trim_start_matches("__varg_min_")
        .trim_start_matches("__varg_");
    let t = match name {
        // ── String ────────────────────────────────────────────────────────────────
        "to_upper" | "to_lower" | "trim" | "trim_start" | "trim_end" | "ltrim" | "rtrim"
        | "replace" | "substring" | "repeat" | "pad_left" | "pad_right" | "char_at"
        | "json_get" | "json_stringify" | "json_stringify_pretty" | "json_set" | "json_merge"
        | "to_string" | "to_hex" | "to_binary" | "to_fixed" | "uuid"
        | "base64_encode" | "base64_decode" | "base64_encode_file"
        | "path_join" | "path_parent" | "path_stem" | "path_extension"
        | "time_format" | "timestamp" | "memory_get" | "workflow_status"
        | "ansi_color" | "ansi_bold" | "ansi_reset" => TypeNode::String,

        // ── Int ───────────────────────────────────────────────────────────────────
        "len" | "length" | "count" | "count_occurrences" | "parse_int" | "sum"
        | "time_millis" | "time_add" | "time_diff" | "channel_len" | "event_count"
        | "vector_store_count" | "json_get_int" | "estimate_tokens" | "random_int"
        | "proc_pid" | "orchestrator_task_count" | "orchestrator_completed_count"
        | "pipeline_step_count" | "workflow_step_count" | "mcp_server_tool_count"
        | "memory_episode_count" | "trace_span_count" => TypeNode::Int,

        // ── Float ─────────────────────────────────────────────────────────────────
        "sqrt" | "floor" | "ceil" | "round" | "pow" | "parse_float" | "random_float"
        | "tensor_sum" | "tensor_mean" | "tensor_min" | "tensor_max" | "tensor_dot" => TypeNode::Float,

        // ── Bool ──────────────────────────────────────────────────────────────────
        "contains" | "contains_key" | "starts_with" | "ends_with" | "is_empty"
        | "is_some" | "is_none" | "is_ok" | "is_err" | "path_exists" | "is_file" | "is_dir"
        | "json_has" | "json_get_bool" | "channel_is_closed" | "proc_is_alive"
        | "registry_is_installed" => TypeNode::Bool,

        // ── Result<String, Error> (fallible, string result) ───────────────────────
        "fs_read" | "exec" | "fetch" | "http_download_base64" | "env" =>
            TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::Error)),

        _ => return None,
    };
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_common_builtins() {
        assert_eq!(builtin_return_type("to_upper"), Some(TypeNode::String));
        assert_eq!(builtin_return_type("len"), Some(TypeNode::Int));
        assert_eq!(builtin_return_type("sqrt"), Some(TypeNode::Float));
        assert_eq!(builtin_return_type("contains"), Some(TypeNode::Bool));
        assert_eq!(builtin_return_type("__varg_to_upper"), Some(TypeNode::String)); // prefix stripped
        assert!(matches!(builtin_return_type("fs_read"), Some(TypeNode::Result(_, _))));
        assert_eq!(builtin_return_type("some_unknown_builtin"), None);
    }
}
