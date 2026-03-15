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
| Compiler LOC | 24,012 lines of Rust |
| Test Suite | 685 tests, 0 failures |
| Crates | 10 specialized compiler crates |
| Token Types | 119 lexer tokens |
| AST Variants | 25 statements, 29 expressions |
| Builtins | 94 typechecker handlers, 116 codegen handlers |
| Security | 5 OCAP capability types |
| Runtime Modules | 10 (crypto, db, llm, net, vector, core, http-server, sqlite, websocket, mcp) |
| Dev Waves | 18 completed development waves |

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

---

## Language Features

### Core Language
- **C#-meets-Rust syntax** -- familiar to most developers
- **Agents & Actors** -- first-class `agent` keyword with lifecycle (`on_start`, `on_stop`, `on_message`), state management, and message passing (`spawn`, `send`, `request`)
- **OCAP Security** -- 5 capability token types enforced at compile time
- **Contracts** -- interface-first design with compile-time enforcement
- **Generics** -- full generic structs, functions, and trait bounds (`<T: Display>`)
- **Enums + Pattern Matching** -- exhaustive `match` with guards and wildcard
- **Closures & Lambdas** -- `(x) => x * 2` with type inference
- **Async/Await** -- backed by tokio runtime
- **Error Handling** -- `Result<T, E>`, `?` operator, `try/catch`, `or` fallback, `map_err`, `and_then`, `unwrap_or`
- **Dependency Injection** -- contract-typed fields as `Box<dyn Trait>`, constructor injection, factory functions
- **Pipe Operator** -- `data |> transform |> send`
- **String Interpolation** -- `$"Hello {name}, you have {count} items"`
- **Multiline Strings** -- `"""..."""` for prompts and templates
- **Iterator Chains** -- `.filter().map().find().any().all().sort()`
- **Tuples, Ranges, HashSet** -- `(int, string)`, `0..10`, `set<T>`
- **Module System** -- `import math.{sqrt, abs}`
- **Standalone Functions** -- top-level `fn` definitions outside agents
- **Type Aliases** -- `type Score = int`

### AI/Agent-Specific
- **Retry/Fallback** -- `retry(3, backoff: 1000) { api_call() } fallback { cached_result() }`
- **Agent Lifecycle** -- `on_start`, `on_stop`, `on_message` hooks
- **Agent Messaging** -- `spawn`, `send`, `request` for actor-model communication
- **Prompt Templates** -- first-class `prompt` keyword
- **MCP Schema Generation** -- `@[McpTool]` annotation auto-generates tool schemas
- **Implicit Context** -- `@[WithContext]` for automatic context propagation
- **Typed Tool Responses** -- `@[ToolResponse]` for structured LLM outputs
- **LLM Provider Abstraction** -- OpenAI, Anthropic, Ollama with unified API

### Standard Library (94+ builtins)
- **Strings** -- `split`, `contains`, `starts_with`, `ends_with`, `replace`, `trim`, `to_upper`, `to_lower`, `substring`, `index_of`, `pad_left`, `pad_right`, `chars`, `reverse`, `repeat`
- **Collections** -- `push`, `pop`, `len`, `filter`, `map`, `find`, `any`, `all`, `sort`, `contains`, `remove`, `keys`, `values`
- **File I/O** -- `fs_read`, `fs_write`, `fs_append`, `fs_read_lines`, `fs_read_dir`
- **HTTP Client** -- `fetch` (GET/POST/PUT/DELETE), `http_request` (with status, headers)
- **HTTP Server** -- `http_serve`, `http_route`, `http_listen` (axum-based)
- **Database** -- `db_open`, `db_execute`, `db_query` (SQLite driver)
- **WebSocket** -- `ws_connect`, `ws_send`, `ws_receive`, `ws_close`
- **SSE** -- `sse_stream`, `sse_send`, `sse_close` (Server-Sent Events)
- **MCP Protocol** -- `mcp_connect`, `mcp_list_tools`, `mcp_call_tool`, `mcp_disconnect`
- **JSON** -- `json_parse`, `json_get`, `json_get_int`, `json_get_bool`, `json_get_array`, `json_stringify`
- **Shell** -- `exec`, `exec_status`
- **Date/Time** -- `time_millis`, `time_format`, `time_parse`, `time_add`, `time_diff`
- **Regex** -- `regex_match`, `regex_find_all`, `regex_replace`
- **Crypto** -- `encrypt`, `decrypt`
- **Logging** -- `log_debug`, `log_info`, `log_warn`, `log_error`
- **Math** -- `abs`, `sqrt`, `floor`, `ceil`, `round`, `min`, `max`, `pow`, `parse_int`, `parse_float`
- **Environment** -- `env("KEY")` for environment variables

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
    // Declares this method needs file system access
    public string ReadConfig(string path, FileAccess cap) {
        return fs_read(path)?;
    }

    public void Run() {
        // Caller must explicitly grant the capability
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
| `DbAccess` | SurrealDB queries, SQLite driver |
| `LlmAccess` | LLM provider calls |
| `SystemAccess` | Shell execution, MCP protocol |

---

## Getting Started

### Easy Install (Pre-compiled Binary)

The easiest way to use Varg is to download the pre-compiled binary:

1. Go to the [Releases](../../releases) page.
2. Download the latest `varg-vX.Y.Z-windows-x64.zip` (currently v0.7.0).
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

# Watch mode (recompile on file change)
vargc watch hello.varg
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

---

## Compiler Architecture

```
varg-compiler/crates/           24,012 LOC total
  varg-ast/          685 LOC    Token definitions (119 types, Logos) + AST (25 stmt, 29 expr)
  varg-lexer/        403 LOC    Tokenization (29 tests)
  varg-parser/     6,049 LOC    Recursive descent parser (168 tests)
  varg-typechecker/6,715 LOC    Semantic analysis + OCAP + DI (197 tests)
  varg-codegen/    6,250 LOC    AST -> Rust code generation (213 tests)
  vargc/           1,986 LOC    CLI driver (build/run/emit-rs/test/watch/fmt)
  varg-os-types/      91 LOC    Native types: Prompt, Context, Tensor, Embedding
  varg-runtime/    1,177 LOC    Runtime (crypto, net, db, llm, http-server, sqlite, ws, mcp)
  varg-lsp/          641 LOC    Language Server Protocol (diagnostics, hover, completion)
  varg-playground/    15 LOC    Execution sandbox
```

### Compilation Pipeline

```
  .varg source
      |
  [1] Lexer (Logos)        -- tokenize into 119 token types
      |
  [2] Parser               -- recursive descent -> typed AST
      |
  [3] TypeChecker           -- semantic analysis, type inference, OCAP validation
      |
  [4] CodeGen               -- AST -> Rust source code
      |
  [5] cargo build           -- Rust -> native binary
```

---

## Test Suite

685 tests across all crates, all passing:

```bash
cd varg-compiler
cargo test
```

| Crate | Tests | Coverage |
|-------|------:|----------|
| varg-ast | 1 | AST construction |
| varg-lexer | 29 | All token types, edge cases |
| varg-parser | 168 | Every statement/expression variant, qualified imports |
| varg-typechecker | 197 | Type inference, OCAP, DI contracts, Result methods |
| varg-codegen | 213 | Rust generation, HTTP/DB/WS/MCP codegen, DI boxing |
| varg-os-types | 11 | OCAP marker structs |
| varg-runtime | 48 | Real HTTP server, SQLite, WebSocket, SSE, MCP client |
| varg-lsp | 18 | Diagnostics, hover, completion |
| **Total** | **685** | **0 failures** |

---

## Project Structure

```
Project X/
  README.md               This file (English)
  README_DE.md            German version
  REFERENCE.md            Complete language reference
  VARG_AGENT_GUIDE.md     AI Agent programming prompt
  docs/                   Images and assets
  examples/               5 example programs
  varg-compiler/          Rust workspace (10 crates, 22,682 LOC)
  varg-vscode/            VS Code extension (syntax highlighting)
```

---

## Status

Varg is in active development. The compiler is functional and produces working native binaries.
18 development waves completed, 685 tests passing.

Wave 17 (F41) added HTTP server, database drivers, WebSocket/SSE, MCP protocol, dependency injection, extended test infrastructure, and qualified crate imports -- everything needed for full-stack agent applications.

Wave 18 (F42) replaced all runtime stubs with real implementations: axum HTTP server, rusqlite (bundled) database, tungstenite WebSocket client, and MCP JSON-RPC client with process spawning. Added test lifecycle hooks and code coverage support.

The language is suitable for building real agents, CLI tools, API clients, web servers, and automation scripts.

---

## License

MIT
