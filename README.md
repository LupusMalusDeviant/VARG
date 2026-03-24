# Varg

<div align="center">
  <img src="docs/varg_logo.png" alt="Varg Logo" width="300"/>
</div>

**A compiled programming language built for autonomous AI agents.**

Varg transpiles to Rust, giving you native performance with a developer-friendly C#-like syntax.
Designed from the ground up for building autonomous agents with built-in capability-based security (OCAP), actor-model concurrency, and native AI/LLM types.

```
Varg Source (.varg) --> vargc --> Rust Source --> cargo build --> Native Binary
```

---

## At a Glance

| Metric | Value |
|--------|-------|
| Test Suite | 767 tests, 0 failures |
| Crates | 10 specialized compiler crates |
| Token Types | 119 lexer tokens |
| AST Variants | 25 statements, 29 expressions |
| Builtins | 140+ typechecker handlers, 180+ codegen handlers |
| Security | 5 OCAP capability types |
| Runtime Modules | 18 (crypto, db, llm, net, vector, http-server, sqlite, websocket, mcp-client, mcp-server, graph, memory, trace, pipeline, orchestration, self-improve, encoding, pdf) |
| Dev Waves | 27 completed development waves |

---

## Quick Example

```csharp
agent WeatherBot {
    public async string GetForecast(string city, NetworkAccess net) {
        var resp = fetch($"https://api.weather.com/{city}", "GET")?;
        var json = json_parse(resp)?;
        var temp = json_get(json, "/main/temp");
        return $"It's {temp} C in {city}";
    }

    public void Run() {
        unsafe {
            var net = NetworkAccess {};
            var forecast = self.GetForecast("Berlin", net);
            print forecast;
        }
    }
}
```

```bash
vargc run weather.varg
```

---

## Why Varg?

| Feature | Varg | Python | TypeScript | Rust |
|---------|:----:|:------:|:----------:|:----:|
| Native binary | Yes | - | - | Yes |
| Agent-first design | Yes | - | - | - |
| Compile-time security (OCAP) | Yes | - | - | - |
| Actor model built-in | Yes | - | - | - |
| LLM/AI types native | Yes | - | - | - |
| Approachable syntax | Yes | Yes | Yes | - |
| Retry/fallback syntax | Yes | - | - | - |
| Prompt as type | Yes | - | - | - |
| Knowledge Graph built-in | Yes | - | - | - |
| Vector Store built-in | Yes | - | - | - |
| Agent Memory (3-layer) | Yes | - | - | - |
| Observability / Tracing | Yes | - | - | - |
| MCP Server + Client | Yes | - | - | - |

---

## Performance

Varg compiles to native Rust binaries -- no interpreter, no garbage collector.

| Benchmark | Varg | Python | C# | TypeScript |
|-----------|-----:|-------:|---:|-----------:|
| Fibonacci(35) | **15ms** | 695ms | 53ms | 53ms |
| Data Pipeline | **1ms** | 5ms | 15ms | 5ms |
| JSON Processing | **1ms** | 1ms | 35ms | 1ms |

**46x faster than Python** on pure compute. Token efficiency is **1.16x vs Python** (near parity for LLM code generation).

---

## Language Features

### Core Language
- **C#-meets-Rust syntax** -- familiar to most developers
- **Agents & Actors** -- first-class `agent` keyword with lifecycle (`on_start`, `on_stop`, `on_message`), state management, and message passing (`spawn`, `send`, `request`)
- **OCAP Security** -- 5 capability token types enforced at compile time
- **Contracts** -- interface-first design with compile-time enforcement
- **Generics** -- full generic structs, functions, and trait bounds (`<T: Display>`)
- **Enums + Pattern Matching** -- exhaustive `match` with guards and wildcard
- **Closures & Lambdas** -- `(x) => x * 2` with type inference (untyped params)
- **Ternary Operator** -- `condition ? true_val : false_val`
- **Async/Await** -- backed by tokio runtime
- **Error Handling** -- `Result<T, E>`, `?` operator, `try/catch`, `or` fallback, `map_err`, `and_then`, `unwrap_or`
- **Dependency Injection** -- contract-typed fields as `Box<dyn Trait>`, constructor injection
- **Pipe Operator** -- `data |> transform |> send`
- **String Interpolation** -- `$"Hello {name}, you have {count} items"`
- **Multiline Strings** -- `"""..."""` for prompts and templates
- **Iterator Chains** -- `.filter().map().find().any().all().sort()`
- **Tuples, Ranges, HashSet** -- `(int, string)`, `0..10`, `set<T>`
- **Module System** -- `import math.{sqrt, abs}`
- **Standalone Functions** -- top-level `fn` definitions outside agents
- **Optional Semicolons** -- semicolons are optional at end of statements in blocks
- **Braceless If/While** -- single-statement bodies without braces

### AI/Agent-Specific
- **Retry/Fallback** -- `retry(3, backoff: 1000) { api_call() } fallback { cached_result() }`
- **Agent Lifecycle** -- `on_start`, `on_stop`, `on_message` hooks
- **Agent Messaging** -- `spawn`, `send`, `request` for actor-model communication
- **Prompt Templates** -- first-class `prompt` keyword
- **MCP Client** -- connect to MCP servers, list tools, call tools (JSON-RPC over stdio)
- **MCP Server** -- expose agent methods as MCP tools for other AI systems
- **Knowledge Graph** -- embedded graph engine with nodes, edges, traversal, queries
- **Vector Store** -- embed text, store vectors, cosine similarity search
- **Agent Memory** -- 3-layer architecture: working (key-value), episodic (vector), semantic (graph)
- **Observability** -- hierarchical span tracing with events, attributes, JSON export
- **Reactive Pipelines** -- event bus (pub/sub) + sequential pipeline runner
- **Agent Orchestration** -- fan-out/fan-in parallel execution, task queues
- **Self-Improving Loop** -- feedback tracking, success/failure recall via similarity search
- **LLM Provider Abstraction** -- OpenAI, Anthropic, Ollama with unified API

### Standard Library (140+ builtins)
- **Strings** -- `split`, `contains`, `starts_with`, `ends_with`, `replace`, `trim`, `to_upper`, `to_lower`, `substring`, `index_of`, `pad_left`, `pad_right`, `chars`, `reverse`, `repeat`
- **Collections** -- `push`, `pop`, `len`, `filter`, `map`, `find`, `any`, `all`, `sort`, `contains`, `remove`, `keys`, `values`
- **File I/O** -- `fs_read`, `fs_write`, `fs_append`, `fs_read_lines`, `fs_read_dir`
- **HTTP Client** -- `fetch` (GET/POST/PUT/DELETE), `http_request` (with status, headers)
- **HTTP Server** -- `http_serve`, `http_route`, `http_listen` (real axum-based async server)
- **Database** -- `db_open`, `db_execute`, `db_query` (real SQLite via rusqlite, bundled)
- **WebSocket** -- `ws_connect`, `ws_send`, `ws_receive`, `ws_close` (real tungstenite)
- **SSE** -- `sse_stream`, `sse_send`, `sse_close`
- **MCP Client** -- `mcp_connect`, `mcp_list_tools`, `mcp_call_tool`, `mcp_disconnect`
- **MCP Server** -- `mcp_server_new`, `mcp_server_register`, `mcp_server_run`
- **Knowledge Graph** -- `graph_open`, `graph_add_node`, `graph_add_edge`, `graph_query`, `graph_traverse`, `graph_neighbors`
- **Vector Store** -- `embed`, `vector_store_open`, `vector_store_upsert`, `vector_store_search`, `vector_store_delete`, `vector_store_count`
- **Agent Memory** -- `memory_open`, `memory_set`, `memory_get`, `memory_store`, `memory_recall`, `memory_add_fact`, `memory_query_facts`
- **Tracing** -- `trace_start`, `trace_span`, `trace_end`, `trace_error`, `trace_event`, `trace_set_attr`, `trace_export`
- **Pipelines** -- `event_bus_new`, `event_emit`, `pipeline_new`, `pipeline_run`
- **Orchestration** -- `orchestrator_new`, `orchestrator_add_task`, `orchestrator_run_all`, `orchestrator_results`
- **Self-Improving** -- `self_improver_new`, `self_improver_record_success`, `self_improver_record_failure`, `self_improver_recall`, `self_improver_stats`
- **JSON** -- `json_parse`, `json_get`, `json_get_int`, `json_get_bool`, `json_get_array`, `json_stringify`
- **Shell** -- `exec`, `exec_status`
- **Date/Time** -- `time_millis`, `time_format`, `time_parse`, `time_add`, `time_diff`
- **Crypto** -- `encrypt`, `decrypt`
- **Logging** -- `log_debug`, `log_info`, `log_warn`, `log_error`
- **Math** -- `abs`, `sqrt`, `floor`, `ceil`, `round`, `min`, `max`, `pow`, `parse_int`, `parse_float`
- **Environment** -- `env("KEY")`

### Tooling
- **VS Code Extension** -- syntax highlighting for `.varg` files
- **Language Server (LSP)** -- real-time diagnostics, hover info, completions
- **Debug Mode** -- `vargc build --debug` for fast iteration (skips cargo)
- **Source Maps** -- error messages reference Varg line numbers, not Rust
- **Test Framework** -- `@[Test]`, `@[BeforeEach]`, `@[AfterEach]` + `assert`, `assert_eq`, `assert_ne`, `assert_true`, `assert_false`, `assert_contains`, `assert_throws`
- **Code Coverage** -- `vargc test --coverage` via cargo-llvm-cov integration
- **Qualified Imports** -- `import axum::Router;`, wildcards, braced imports for external crate types

---

## OCAP Security Model

Every privileged operation requires a capability token passed as a method parameter.
Tokens can only be constructed inside `unsafe` blocks -- the compiler enforces this at compile time.

```csharp
agent SecureAgent {
    public string ReadConfig(string path, FileAccess cap) {
        return fs_read(path)?;
    }

    public void Run() {
        unsafe {
            var cap = FileAccess {};
            var config = self.ReadConfig("config.toml", cap);
            print config;
        }
    }
}
```

**5 Capability Types:**

| Capability | Protects |
|------------|----------|
| `FileAccess` | File system read/write/append |
| `NetworkAccess` | HTTP requests, fetch |
| `DbAccess` | Database queries (SQLite) |
| `LlmAccess` | LLM provider calls |
| `SystemAccess` | Shell execution, MCP protocol |

---

## Getting Started

### Easy Install (Pre-compiled Binary)

The easiest way to use Varg is to download the pre-compiled binary:

1. Go to the [Releases](../../releases) page.
2. Download the latest `varg-v0.9.0-windows-x64.zip` (currently v0.9.0).
3. Extract `vargc.exe` and place it somewhere in your system `PATH`.
4. You're ready to go!
---

### Build from Source

#### Prerequisites

- [Rust](https://rustup.rs/) (1.75+)

### Build the Compiler

```bash
cd varg-compiler
cargo build --release
```

The compiler binary will be at `target/release/vargc`.

### Compile & Run

```bash
# Build a .varg file to native binary
vargc build hello.varg

# Build and immediately run
vargc run hello.varg

# Emit generated Rust source (for inspection)
vargc emit-rs hello.varg

# Run tests annotated with @[Test]
vargc test my_tests.varg

# Run tests with coverage
vargc test --coverage my_tests.varg

# Watch mode (recompile on file change)
vargc watch hello.varg

# Format source code
vargc fmt hello.varg
```

### Hello World

```csharp
// hello.varg
agent Hello {
    public void Run() {
        print "Hello from Varg!";
    }
}
```

```bash
vargc run hello.varg
# --> Hello from Varg!
```

---

## Examples

See the [`examples/`](examples/) directory:

| File | What it shows |
|------|---------------|
| [`hello.varg`](examples/hello.varg) | Minimal hello world |
| [`file_processor.varg`](examples/file_processor.varg) | File I/O with OCAP security, try/catch, directory scanning |
| [`api_client.varg`](examples/api_client.varg) | HTTP requests with retry/fallback and JSON parsing |
| [`data_pipeline.varg`](examples/data_pipeline.varg) | Iterators, enums, maps, sets, pattern matching |
| [`chat_agent.varg`](examples/chat_agent.varg) | Multi-agent system with spawn, send, on_message |
| [`knowledge_graph.varg`](examples/knowledge_graph.varg) | Graph nodes, edges, traversal, queries |
| [`vector_store.varg`](examples/vector_store.varg) | Text embedding, vector upsert, similarity search |
| [`agent_memory.varg`](examples/agent_memory.varg) | 3-layer memory: working, episodic, semantic |
| [`tracing.varg`](examples/tracing.varg) | Span-based tracing with events and JSON export |

---

## Runtime Modules

Varg includes 16 runtime modules, all with real implementations (no stubs):

| Module | Backend | Description |
|--------|---------|-------------|
| HTTP Server | axum 0.7 + tokio | Async request/response handling |
| SQLite | rusqlite 0.31 (bundled) | No system deps, parameterized queries |
| WebSocket | tungstenite 0.24 | Client with TLS support |
| MCP Client | std::process + JSON-RPC | Spawns child process, full protocol |
| MCP Server | Pure Rust (stdio) | Register tools, JSON-RPC handler |
| Knowledge Graph | Pure Rust | Adjacency list, node/edge/traversal |
| Vector Store | Pure Rust | Cosine similarity, brute-force search |
| Agent Memory | Pure Rust | 3-layer: working + episodic + semantic |
| Tracing | Pure Rust | Hierarchical spans, OTel-compatible |
| Pipelines | Pure Rust | Event bus + sequential pipeline |
| Orchestration | Pure Rust (threads) | Fan-out/fan-in, parallel tasks |
| Self-Improving | Pure Rust | Feedback loop + similarity recall |
| Crypto | Pure Rust | encrypt, decrypt |
| HTTP Client | reqwest | fetch, http_request |
| LLM | reqwest | OpenAI, Anthropic, Ollama |
| SSE | Placeholder | Server-Sent Events |

---

## Test Suite

748 tests across all crates, all passing:

```bash
cd varg-compiler
cargo test
```

| Crate | Tests | Coverage |
|-------|------:|----------|
| varg-ast | 1 | AST construction |
| varg-lexer | 29 | All token types, edge cases |
| varg-parser | 175 | Every statement/expression variant, ternary, braceless if/while |
| varg-typechecker | 199 | Type inference, OCAP, DI, all 140+ builtins |
| varg-codegen | 217 | Rust generation, all runtime module codegen |
| varg-os-types | 11 | OCAP marker structs |
| varg-runtime | 98 | Real HTTP/SQLite/WS/MCP + graph, vector, memory, trace, pipeline, orchestration, self-improve |
| varg-lsp | 18 | Diagnostics, hover, completion |
| **Total** | **748** | **0 failures** |

---

## Project Structure

```
Project X/
  README.md               This file (English)
  REFERENCE.md            Complete language reference
  VARG_AGENT_GUIDE.md     AI Agent programming guide
  docs/                   Architecture docs, images
  examples/               9 example programs
  varg-compiler/          Rust workspace (10 crates)
  varg-vscode/            VS Code extension (syntax highlighting)
```

---

## Status

Varg is in active development. The compiler is functional and produces working native binaries.
26 development waves completed, 748 tests passing.

The language is suitable for building real agents, CLI tools, API clients, web servers, knowledge-graph-powered RAG systems, and multi-agent orchestration pipelines.

---

## License

MIT
