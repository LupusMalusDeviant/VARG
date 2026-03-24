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
