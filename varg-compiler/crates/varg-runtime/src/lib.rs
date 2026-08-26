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
        eprintln!("\x1b[1;31mRuntime error:\x1b[0m {}", clean);
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
pub mod json;          // JSON accessors accepting a parsed value or a raw JSON string
pub mod db;            // legacy stub
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
pub use db::*;
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
