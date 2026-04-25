// Varg Runtime Library
//
// All runtime helper functions used by compiled Varg programs.
// Previously these were injected inline by the codegen as string literals.
// Now they live as proper Rust code with tests and type safety.

pub mod crypto;
pub mod net;
pub mod db;
pub mod llm;
pub mod vector;
pub mod server;      // F41-2: HTTP Server
pub mod db_sqlite;   // F41-3: SQLite Driver
pub mod websocket;   // F41-4: WebSocket / SSE
pub mod mcp;         // F41-8: MCP Protocol
pub mod graph;       // Wave 20: Knowledge Graph
pub mod memory;      // Wave 21: Agent Memory (3 layers)
pub mod trace;       // Wave 22: Observability & Tracing
pub mod mcp_server;  // Wave 23: MCP Server Mode
pub mod pipeline;    // Wave 24: Reactive Pipelines
pub mod orchestration; // Wave 25: Agent Orchestration
pub mod self_improve;  // Wave 26: Self-Improving Loop
pub mod encoding;      // Wave 27: Base64 Encoding/Decoding
pub mod pdf;           // Wave 27: PDF Generation
pub mod sse_client;    // Wave 28: SSE Client (streaming LLM responses)
pub mod proc;          // Wave 28: Process Management (spawn/wait/kill)
pub mod config;        // Wave 29: Platform dirs + JSON config cascade
pub mod readline;      // Wave 29: Readline / REPL primitives (rustyline)
pub mod hitl;          // Wave 30: Human-in-the-Loop primitives
pub mod ratelimit;     // Wave 30: Rate limiting (@[RateLimit])
pub mod cost;          // Wave 31: LLM cost tracking (@[Budget])
pub mod checkpoint;    // Wave 32: Agent state checkpoint/resume
pub mod channel;       // Wave 33: Typed inter-agent channels
pub mod proptest;      // Wave 33: Property-based testing (@[Property])
pub mod multimodal;    // Wave 34: Image / Audio types + vision LLM
pub mod workflow;      // Wave 34: Workflow / DAG execution engine
pub mod registry;      // Wave 34: Package registry client

// Re-export everything so generated code can use `use varg_runtime::*;`
pub use crypto::*;
pub use net::*;
pub use db::*;
pub use llm::*;
pub use vector::*;
pub use server::*;
pub use db_sqlite::*;
pub use websocket::*;
pub use mcp::*;
pub use graph::*;
pub use memory::*;
pub use trace::*;
pub use mcp_server::*;
pub use pipeline::*;
pub use orchestration::*;
pub use self_improve::*;
pub use encoding::*;
pub use pdf::*;
pub use sse_client::*;
pub use proc::*;
pub use config::*;
pub use readline::*;
pub use hitl::*;
pub use ratelimit::*;
pub use cost::*;
pub use checkpoint::*;
pub use channel::*;
pub use proptest::*;
pub use multimodal::*;
pub use workflow::*;
pub use registry::*;
