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
