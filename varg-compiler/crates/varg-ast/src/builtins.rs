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
        | "json_stringify" | "json_stringify_pretty" | "json_set" | "json_merge"
        | "to_string" | "to_hex" | "to_binary" | "to_fixed" | "uuid"
        | "base64_encode" | "base64_decode" | "base64_encode_file"
        | "path_join" | "path_parent" | "path_stem" | "path_extension"
        | "time_format" | "timestamp" | "memory_get" | "workflow_status"
        | "ansi_color" | "ansi_bold" | "ansi_reset" | "agents_list" | "exe_path"
        // fetch / http_download_base64 look fallible but their runtime fns return a bare String
        // (errors surface in the body), so their static type is String, not Result. Keeping them
        // here is what the runtime signatures actually guarantee — see net.rs / encoding.rs.
        | "fetch" | "http_download_base64" => TypeNode::String,

        // ── Int ───────────────────────────────────────────────────────────────────
        "len" | "length" | "count" | "count_occurrences" | "sum"
        | "time_millis" | "time_add" | "time_diff" | "channel_len" | "event_count"
        | "vector_store_count" | "estimate_tokens" | "random_int"
        | "proc_pid" | "orchestrator_task_count" | "orchestrator_completed_count"
        | "pipeline_step_count" | "workflow_step_count" | "mcp_server_tool_count"
        | "memory_episode_count" | "trace_span_count"
        | "agents_count" | "agents_count_by_status" => TypeNode::Int,

        // ── Float ─────────────────────────────────────────────────────────────────
        "sqrt" | "floor" | "ceil" | "round" | "pow" | "random_float"
        | "tensor_sum" | "tensor_mean" | "tensor_min" | "tensor_max" | "tensor_dot" => TypeNode::Float,

        // ── Bool ──────────────────────────────────────────────────────────────────
        "contains" | "contains_key" | "starts_with" | "ends_with" | "is_empty"
        | "is_some" | "is_none" | "is_ok" | "is_err" | "path_exists" | "is_file" | "is_dir"
        | "json_has" | "channel_is_closed" | "proc_is_alive"
        | "registry_is_installed" => TypeNode::Bool,

        // ── Result<String, Error> (fallible, string result) ───────────────────────
        // Only builtins whose codegen actually emits a Result (env→std::env::var,
        // fs_read→read_to_string().map_err, exec→…map_err). fetch/http_download_base64 return a
        // bare String (see the String group above) despite looking fallible.
        // LLM calls reach the network and can fail. They used to hand the provider's error
        // payload back as the answer, so a failure was indistinguishable from a reply.
        "fs_read" | "exec" | "env" | "llm_infer" | "llm_chat" =>
            TypeNode::Result(Box::new(TypeNode::String), Box::new(TypeNode::Error)),

        // ── Result<Int/Float, Error> ─────────────────────────────────────────────
        // `parse_int`/`parse_float` are genuinely fallible. They used to lower to
        // `.unwrap_or(0)`, so a malformed input silently became 0 and flowed on as a wrong
        // number. Typing them as Result forces the caller to use `?` or `or`.
        "parse_int" => TypeNode::Result(Box::new(TypeNode::Int), Box::new(TypeNode::Error)),
        "parse_float" => TypeNode::Result(Box::new(TypeNode::Float), Box::new(TypeNode::Error)),

        // ── Nullable ("there may be nothing there") ────────────────────────
        // The JSON accessors answered a plain value with a default baked in, so `""`/`0`/`false`
        // meant five different things at once: absent key, wrong kind, JSON null, empty value,
        // unparseable document. Nullable separates "nothing there" from every value.
        "json_get" => TypeNode::Nullable(Box::new(TypeNode::String)),
        "json_get_int" => TypeNode::Nullable(Box::new(TypeNode::Int)),
        "json_get_bool" => TypeNode::Nullable(Box::new(TypeNode::Bool)),
        "json_get_array" => {
            TypeNode::Nullable(Box::new(TypeNode::Array(Box::new(TypeNode::String))))
        }

        _ => return None,
    };
    Some(t)
}

/// Every builtin name for which [`builtin_return_type`] yields a fixed type. Kept in lockstep
/// with the match above (a unit test asserts every listed name resolves). Consumers that want to
/// enumerate the table — e.g. the typechecker's drift-lock cross-check — iterate this instead of
/// re-hardcoding the set. Names are the bare form (no `__varg_` prefix).
pub fn known_builtin_names() -> &'static [&'static str] {
    &[
        // Nullable
        "json_get", "json_get_int", "json_get_bool", "json_get_array",
        // String
        "to_upper", "to_lower", "trim", "trim_start", "trim_end", "ltrim", "rtrim",
        "replace", "substring", "repeat", "pad_left", "pad_right", "char_at",
        "json_stringify", "json_stringify_pretty", "json_set", "json_merge",
        "to_string", "to_hex", "to_binary", "to_fixed", "uuid",
        "base64_encode", "base64_decode", "base64_encode_file",
        "path_join", "path_parent", "path_stem", "path_extension",
        "time_format", "timestamp", "memory_get", "workflow_status",
        "ansi_color", "ansi_bold", "ansi_reset", "agents_list", "exe_path",
        "fetch", "http_download_base64",
        // Int
        "len", "length", "count", "count_occurrences", "parse_int", "sum",
        "time_millis", "time_add", "time_diff", "channel_len", "event_count",
        "vector_store_count", "estimate_tokens", "random_int",
        "proc_pid", "orchestrator_task_count", "orchestrator_completed_count",
        "pipeline_step_count", "workflow_step_count", "mcp_server_tool_count",
        "memory_episode_count", "trace_span_count",
        "agents_count", "agents_count_by_status",
        // Float
        "sqrt", "floor", "ceil", "round", "pow", "parse_float", "random_float",
        "tensor_sum", "tensor_mean", "tensor_min", "tensor_max", "tensor_dot",
        // Bool
        "contains", "contains_key", "starts_with", "ends_with", "is_empty",
        "is_some", "is_none", "is_ok", "is_err", "path_exists", "is_file", "is_dir",
        "json_has", "channel_is_closed", "proc_is_alive",
        "registry_is_installed",
        // Result<String, Error>
        "fs_read", "exec", "env", "llm_infer", "llm_chat",
    ]
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

    /// The enumerated list and the match must never diverge: every listed name must resolve.
    #[test]
    fn every_listed_name_resolves() {
        for name in known_builtin_names() {
            assert!(
                builtin_return_type(name).is_some(),
                "known_builtin_names() lists `{}` but builtin_return_type() returns None — \
                 the list and the match drifted",
                name
            );
        }
    }

    /// Builtins that cannot be exercised by a golden program without reaching the network.
    /// Everything else must be covered; this list is the only permitted excuse and is kept
    /// deliberately tiny.
    const NETWORK_ONLY: &[&str] = &[
        "fetch",
        "http_download_base64",
        // LLM calls need a live provider; there is nothing deterministic to assert without one.
        "llm_infer",
        "llm_chat",
    ];

    /// Ratchet on end-to-end coverage.
    ///
    /// The unit suite checks that a builtin *type-checks and generates code*; it does not run the
    /// result. Every silent-wrong-value and silent-no-op defect found so far lived in a builtin
    /// that no running program ever called — `abs(-5.0)` returning -5, `parse_int` turning bad
    /// input into 0, agent messages being dropped, the whole tensor API being uncallable. Coverage
    /// was 29% when that was measured. This test keeps it from sliding back: a new builtin has to
    /// arrive with a golden program that runs it and checks what it produced.
    #[test]
    fn golden_programs_exercise_at_least_95_percent_of_builtins() {
        let progs = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../golden/progs");
        let Ok(entries) = std::fs::read_dir(&progs) else {
            // Running outside a checkout (e.g. a packaged crate): nothing to measure.
            return;
        };
        let mut source = String::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("varg") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    source.push_str(&text);
                    source.push('\n');
                }
            }
        }
        assert!(!source.is_empty(), "no golden programs found at {}", progs.display());

        // A call site is the name followed by `(`, not preceded by an identifier character —
        // so `len(` and `x.len(` both count, but `channel_len(` does not count as `len`.
        let calls = |name: &str| -> bool {
            let bytes = source.as_bytes();
            source.match_indices(name).any(|(i, _)| {
                let before_ok = i == 0 || {
                    let c = bytes[i - 1] as char;
                    !(c.is_ascii_alphanumeric() || c == '_')
                };
                let after = source[i + name.len()..].trim_start();
                before_ok && after.starts_with('(')
            })
        };

        let candidates: Vec<&str> = known_builtin_names()
            .iter()
            .copied()
            .filter(|n| !NETWORK_ONLY.contains(n))
            .collect();
        let uncovered: Vec<&str> = candidates.iter().copied().filter(|n| !calls(n)).collect();
        let covered = candidates.len() - uncovered.len();
        let pct = 100.0 * covered as f64 / candidates.len() as f64;

        assert!(
            pct >= 95.0,
            "golden coverage fell to {:.1}% ({}/{}). Uncovered: {:?}\n\
             Add a golden program that calls the missing builtins and checks their results.",
            pct,
            covered,
            candidates.len(),
            uncovered
        );
    }
}
