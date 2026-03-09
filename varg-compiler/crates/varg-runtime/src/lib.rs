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

// Re-export everything so generated code can use `use varg_runtime::*;`
pub use crypto::*;
pub use net::*;
pub use db::*;
pub use llm::*;
pub use vector::*;
