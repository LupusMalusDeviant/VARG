// Varg Runtime Library
//
// All runtime helper functions used by compiled Varg programs.
// vargc detects which modules are used and enables only the needed features,
// so unused modules and their dependencies are never compiled into the binary.

/// Installed once by the generated main(). Converts Rust panics into clean
/// "Runtime error: ..." messages instead of raw backtraces.
/// The outcome of an entry method, whichever shape it was generated in.
///
/// `Run()` becomes `()` when its body has no `?` in it and `Result<(), String>` when it does, and
/// the generated `main` cannot know which. Both used to be discarded with `instance.Run();`, so a
/// `?` that propagated out of the entry point ended the program **silently, with status 0** —
/// the output stopped halfway and a shell, a CI job or an agent running the binary read that as
/// success. This lets `main` look at either shape the same way.
pub trait EntryOutcome {
    fn into_outcome(self) -> Result<(), String>;
}

impl EntryOutcome for () {
    fn into_outcome(self) -> Result<(), String> {
        Ok(())
    }
}

impl<T, E: std::fmt::Display> EntryOutcome for Result<T, E> {
    fn into_outcome(self) -> Result<(), String> {
        self.map(|_| ()).map_err(|e| e.to_string())
    }
}

pub fn __varg_entry_outcome<T: EntryOutcome>(value: T) -> Result<(), String> {
    value.into_outcome()
}

thread_local! {
    /// How many `catch_unwind` stretches this thread is currently inside.
    ///
    /// The panic hook exits the process for a failure on the main thread, and it runs before the
    /// unwind reaches any `catch_unwind`. So `try/catch` caught nothing wherever the entry point
    /// runs — it worked only on spawned agent threads, which is where it was tested. While this
    /// is non-zero the hook stays quiet and lets the unwind through to the catch that is waiting.
    static CATCHING: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

pub fn __varg_catching_enter() {
    CATCHING.with(|c| c.set(c.get() + 1));
}

pub fn __varg_catching_exit() {
    CATCHING.with(|c| c.set(c.get().saturating_sub(1)));
}

fn __varg_is_catching() -> bool {
    CATCHING.with(|c| c.get()) > 0
}

pub fn __varg_install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        // Where it happened, in the author's own file.
        //
        // A failure used to report only what went wrong — "index out of bounds: the len is 3 but
        // the index is 99" — with no file, no line and no function, which is the one thing every
        // other language gives you here. The line map is built from the generated Rust after it
        // is formatted, so it costs nothing at runtime: no counter to keep, no statement to
        // instrument.
        let origin = info
            .location()
            .and_then(|l| __varg_source_location(l.file(), l.line()));
        let msg: String = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown runtime error".to_string()
        };
        let clean = msg
            .strip_prefix("Varg runtime error: ")
            .unwrap_or(&msg);
        // Inside a `try`, the catch reports what happened; printing here as well would make
        // one failure look like two.
        if __varg_is_catching() {
            return;
        }
        match origin {
            Some(where_) => eprintln!("\x1b[1;31mRuntime error:\x1b[0m {}\n  in {}", clean, where_),
            None => eprintln!("\x1b[1;31mRuntime error:\x1b[0m {}", clean),
        }
        // Exit only when the *main* thread failed. A spawned agent runs on its own thread,
        // and exiting here took the whole process down over one bad message — the
        // dispatcher's catch_unwind never got a chance, so an agent could not be marked
        // failed and kept running. Returning normally lets the unwind reach that
        // catch_unwind instead. Threads spawned by std are unnamed, so the name is what
        // distinguishes them.
        if std::thread::current().name() == Some("main") {
            std::process::exit(1);
        }
    }));
}

/// Lines of the generated Rust paired with the Varg they came from, newest entry last.
///
/// Registered by the program itself at startup; empty for anything not built by `vargc`.
static __VARG_LINES: std::sync::OnceLock<&'static [(u32, &'static str, u32, &'static str)]> =
    std::sync::OnceLock::new();

/// Called once at startup by a generated program.
pub fn __varg_register_line_map(map: &'static [(u32, &'static str, u32, &'static str)]) {
    let _ = __VARG_LINES.set(map);
}

/// Translate a position in the generated Rust back to the Varg line it came from.
///
/// Only for the generated file itself: a panic raised inside the runtime knows its own source,
/// not the program's, and guessing there would name a line that has nothing to do with it.
pub fn __varg_source_location(file: &str, line: u32) -> Option<String> {
    if !file.replace('\\', "/").ends_with("src/main.rs") {
        return None;
    }
    let map = __VARG_LINES.get()?;
    let idx = match map.binary_search_by_key(&line, |e| e.0) {
        Ok(i) => i,
        Err(0) => return None,
        Err(i) => i - 1,
    };
    let (_, varg_file, statement, context) = map[idx];
    // The count is of statements, not lines: the AST carries no source positions, so a line
    // number here would be a guess. Which statement of which construct is exact, and a number
    // that means what it says beats one that reads like a line and is not.
    Some(if context.is_empty() {
        format!("{}, statement {}", varg_file, statement)
    } else {
        format!("{}, {} (statement {})", varg_file, context, statement)
    })
}

#[cfg(test)]
mod source_location_tests {
    use super::*;

    static MAP: &[(u32, &str, u32, &str)] = &[
        (66, "boom.varg", 1, "fn pick"),
        (75, "boom.varg", 2, "agent Main.Run"),
        (81, "boom.varg", 5, "agent Main.Run"),
    ];

    #[test]
    fn a_position_maps_to_the_statement_that_covers_it() {
        __varg_register_line_map(MAP);
        // Between two entries: the one that started before it.
        assert_eq!(
            __varg_source_location("src/main.rs", 78).as_deref(),
            Some("boom.varg, agent Main.Run (statement 2)")
        );
        // Exactly on an entry.
        assert_eq!(
            __varg_source_location("src/main.rs", 66).as_deref(),
            Some("boom.varg, fn pick (statement 1)")
        );
    }

    #[test]
    fn a_position_before_the_first_entry_maps_to_nothing() {
        __varg_register_line_map(MAP);
        assert_eq!(__varg_source_location("src/main.rs", 5), None);
    }

    #[test]
    fn a_failure_inside_the_runtime_is_not_attributed_to_the_program() {
        // A panic raised in the runtime knows its own source, not the program's. Guessing there
        // would name a construct that has nothing to do with it.
        __varg_register_line_map(MAP);
        assert_eq!(__varg_source_location("crates/varg-runtime/src/vector.rs", 78), None);
    }
}

/// `parse_int(s)` / `parse_float(s)`. These are fallible: the previous lowering was
/// `s.parse().unwrap_or(0)`, which turned every malformed input into a silent 0 — a wrong
/// number that flows on undetected. They now return a Result so `?` and `or` work and a bad
/// parse has to be dealt with.
pub fn __varg_parse_int(s: &str) -> Result<i64, String> {
    s.trim()
        .parse::<i64>()
        .map_err(|_| format!("cannot parse `{}` as int", s))
}

pub fn __varg_parse_float(s: &str) -> Result<f64, String> {
    s.trim()
        .parse::<f64>()
        .map_err(|_| format!("cannot parse `{}` as float", s))
}

// ── Always-on modules (pure Rust, no heavy deps) ──────────────────────────────
// Generated programs name this for `ordered_map<K, V>`; re-exported so they need no
// dependency of their own.
pub use indexmap::IndexMap;

pub mod json;          // JSON accessors accepting a parsed value or a raw JSON string
// `pub mod db` held `__varg_query`, the store behind the withdrawn `query "..."`
// statement. The real database path is `db_sqlite`.
pub mod regex_utils;   // Regex builtins: regex_match, regex_find_all, regex_replace
pub use regex_utils::*;
pub mod graph;         // Wave 20: Knowledge Graph
pub mod memory;        // Wave 21: Agent Memory (3 layers)
pub mod trace;         // Wave 22: Observability & Tracing
pub mod agents;        // Wave 22b: agent registry behind the dashboard live agent list
pub mod mcp_server;    // Wave 23: MCP Server Mode
pub mod mcp;           // F41-8: MCP Protocol (std::process)
pub mod pipeline;      // Wave 24: Reactive Pipelines
pub mod orchestration; // Wave 25: Agent Orchestration
pub mod self_improve;  // Wave 26: Self-Improving Loop
pub mod hitl;          // Wave 30: Human-in-the-Loop
pub mod ratelimit;     // Wave 30: Rate limiting
pub mod cost;          // Wave 31: LLM cost tracking
pub mod channel;       // Wave 33: Typed inter-agent channels
pub mod proptest;      // Wave 33: Property-based testing
pub mod workflow;      // Wave 34: Workflow / DAG execution
pub mod registry;      // Wave 34: Package registry client
pub mod proc;          // Wave 28: Process Management
pub mod config;        // Wave 29: Platform dirs + config cascade
pub mod vector;        // Wave 20b: Vector Store (Gemini embed gated by llm feature)
pub mod rag;           // RAG pipeline: index, retrieve, build_prompt
pub mod localembed;    // Wave 40: Pure-Rust local embeddings (no API key needed)

// ── Feature-gated modules ─────────────────────────────────────────────────────
#[cfg(feature = "crypto")]    pub mod crypto;
#[cfg(feature = "net")]       pub mod net;
#[cfg(feature = "net")]       pub mod sse_client;
#[cfg(feature = "server")]    pub mod server;
#[cfg(feature = "db")]        pub mod db_sqlite;
#[cfg(feature = "db")]        pub mod checkpoint;
#[cfg(feature = "llm")]       pub mod llm;
#[cfg(feature = "llm")]       pub mod multimodal;
#[cfg(feature = "ws")]        pub mod websocket;
#[cfg(feature = "pdf")]       pub mod pdf;
#[cfg(feature = "encoding")]  pub mod encoding;
#[cfg(feature = "readline")]  pub mod readline;
#[cfg(feature = "tensor")]    pub mod tensor;    // Wave 36: ndarray tensor builtins
#[cfg(feature = "dataframe")] pub mod dataframe; // Wave 38: Polars DataFrame builtins
#[cfg(feature = "duckdb")]    pub mod duckdb_rt; // Wave 40: DuckDB analytical SQL
#[cfg(feature = "fts")]       pub mod fts;       // Wave 41: Full-text search (tantivy)

// ── Re-exports ────────────────────────────────────────────────────────────────
pub use graph::*;
pub use memory::*;
pub use trace::*;
pub use mcp_server::*;
pub use mcp::*;
pub use pipeline::*;
pub use orchestration::*;
pub use self_improve::*;
pub use hitl::*;
pub use ratelimit::*;
pub use cost::*;
pub use channel::*;
pub use proptest::*;
pub use workflow::*;
pub use registry::*;
pub use proc::*;
pub use config::*;
pub use vector::*;
pub use rag::*;
pub use localembed::*;

#[cfg(feature = "crypto")]    pub use crypto::*;
#[cfg(feature = "net")]       pub use net::*;
#[cfg(feature = "net")]       pub use sse_client::*;
#[cfg(feature = "server")]    pub use server::*;
#[cfg(feature = "db")]        pub use db_sqlite::*;
#[cfg(feature = "db")]        pub use checkpoint::*;
#[cfg(feature = "llm")]       pub use llm::*;
#[cfg(feature = "llm")]       pub use multimodal::*;
#[cfg(feature = "ws")]        pub use websocket::*;
#[cfg(feature = "pdf")]       pub use pdf::*;
#[cfg(feature = "encoding")]  pub use encoding::*;
#[cfg(feature = "readline")]  pub use readline::*;
#[cfg(feature = "tensor")]    pub use tensor::*;
#[cfg(feature = "dataframe")] pub use dataframe::*;
#[cfg(feature = "duckdb")]    pub use duckdb_rt::*;
#[cfg(feature = "fts")]       pub use fts::*;
