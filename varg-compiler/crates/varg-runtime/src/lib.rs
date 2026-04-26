// Varg Runtime Library
//
// All runtime helper functions used by compiled Varg programs.
// vargc detects which modules are used and enables only the needed features,
// so unused modules and their dependencies are never compiled into the binary.

// ── Always-on modules (pure Rust, no heavy deps) ──────────────────────────────
pub mod db;            // legacy stub
pub mod graph;         // Wave 20: Knowledge Graph
pub mod memory;        // Wave 21: Agent Memory (3 layers)
pub mod trace;         // Wave 22: Observability & Tracing
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

// ── Feature-gated modules ─────────────────────────────────────────────────────
#[cfg(feature = "crypto")]   pub mod crypto;
#[cfg(feature = "net")]      pub mod net;
#[cfg(feature = "net")]      pub mod sse_client;
#[cfg(feature = "server")]   pub mod server;
#[cfg(feature = "db")]       pub mod db_sqlite;
#[cfg(feature = "db")]       pub mod checkpoint;
#[cfg(feature = "llm")]      pub mod llm;
#[cfg(feature = "llm")]      pub mod multimodal;
#[cfg(feature = "ws")]       pub mod websocket;
#[cfg(feature = "pdf")]      pub mod pdf;
#[cfg(feature = "encoding")] pub mod encoding;
#[cfg(feature = "readline")] pub mod readline;

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

#[cfg(feature = "crypto")]   pub use crypto::*;
#[cfg(feature = "net")]      pub use net::*;
#[cfg(feature = "net")]      pub use sse_client::*;
#[cfg(feature = "server")]   pub use server::*;
#[cfg(feature = "db")]       pub use db_sqlite::*;
#[cfg(feature = "db")]       pub use checkpoint::*;
#[cfg(feature = "llm")]      pub use llm::*;
#[cfg(feature = "llm")]      pub use multimodal::*;
#[cfg(feature = "ws")]       pub use websocket::*;
#[cfg(feature = "pdf")]      pub use pdf::*;
#[cfg(feature = "encoding")] pub use encoding::*;
#[cfg(feature = "readline")] pub use readline::*;
