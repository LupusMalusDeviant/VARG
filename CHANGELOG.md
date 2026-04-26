# Varg Changelog

All notable changes to the Varg language and compiler are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Varg uses [Semantic Versioning](https://semver.org/).

---

## [1.0.0] — 2026-04-26

**First stable release.** The language spec, core builtins, OCAP model, and CLI are
now considered stable. No breaking changes will be made to items listed under
_Stable API_ without a major version bump.

### Stable API (v1.0)
- Agent / contract / struct / enum declarations
- All control flow: if/else, while, for, foreach, match, try/catch, retry/fallback
- OCAP capability tokens: FileAccess, NetworkAccess, DbAccess, LlmAccess, SystemAccess
- Standard builtins (103+): file I/O, HTTP, SQLite, WebSocket, JSON, math, string, collections
- Async/await (tokio backend)
- Generics with trait bounds
- Closures and lambdas
- Error propagation (`?` operator, `Result<T, E>`)
- Dependency injection via contract-typed fields
- `vargc build / run / emit-rs / test / fmt / doc / watch` CLI
- `vargc doctor / upgrade / install / search / list` package management

### Added — Wave 44: Runtime Stability
- **Panic hook**: all compiled Varg programs now install a panic hook at startup;
  runtime failures print `Runtime error: <message>` (red, clean) instead of a raw Rust backtrace
- **try/catch catches runtime errors**: the `try/catch` block now uses `std::panic::catch_unwind`
  internally, so it catches both explicit `throw` and runtime panics (bad index, division by zero,
  failed I/O, etc.)
- **Better error messages** throughout the runtime: every `expect()` / `unwrap()` in the standard
  library now includes a plain-English explanation of what went wrong and how to fix it

### Added — Wave 45: Module Imports (already live since v0.9)
- `import foo;` — resolves `foo.varg` in the same directory
- `import foo.bar;` — resolves `foo/bar.varg`
- `import foo.bar.baz;` — resolves `foo/bar/baz.varg`
- `import mod.varg;` — resolves `mod/mod.varg` (directory module)
- Cyclic imports are detected and skipped automatically

### Added — Wave 46: LSP Completeness
- **Goto Definition** (`F12`): jump to where an agent, contract, struct, enum, or function is declared
- **Find References** (`Shift+F12`): list all uses of any identifier in the file
- **Document Symbols** (outline view): sidebar list of all top-level definitions with their kind
- New `symbols.rs` module in varg-lsp with 12 unit tests

### Added — Wave 47: Release Readiness
- **`vargc doc`** now generates a self-contained HTML file (`{stem}.html`) with:
  - Dark-themed sidebar navigation
  - Agent / contract / struct / enum / function sections with signatures
  - Doc-comment display
- Version bumped to **1.0.0**
- This CHANGELOG

---

## [0.13.0] — 2026-04-25

### Added — Wave 40: Local Embeddings + DuckDB
- `embed_local(text)` / `embed_local_batch(texts)` — pure-Rust 384-dim embedding
  via FNV-1a character n-gram hashing; no API key, no network required
- `duckdb_open / duckdb_execute / duckdb_query / duckdb_close` — in-process
  analytical SQL via bundled DuckDB; gated behind `--features duckdb`

### Added — Wave 41: Full-Text Search + Hybrid RAG
- `fts_open / fts_add / fts_commit / fts_search / fts_delete / fts_close` — BM25 full-text
  search via tantivy; in-memory (`:memory:`) or on-disk; gated behind `--features fts`
- `rag_hybrid_search` — Reciprocal Rank Fusion (k=60) over BM25 + cosine similarity

### Added — Wave 42: Installer + Self-Management
- `install.sh` / `install.ps1` — one-line install scripts (curl / Invoke-WebRequest)
- `vargc doctor` — prints system check table (PATH, cargo, rustc, rustup targets, network)
- `vargc upgrade` — downloads and installs the latest vargc binary

### Added — Wave 43: Playground Improvements
- **Share button** — encodes current editor code as base64 `?code=` URL, copies to clipboard
- **URL load-on-init** — opening a share link restores the shared code automatically
- **Monaco error markers** — compile errors shown as red squiggles at the exact source location
- 3 new playground examples: Vector Search (local embeddings), DuckDB Analytics, Structured LLM
- v0.13 badge; `DuckDbHandle`, `FtsHandle` added to Monaco grammar

---

## [0.12.0] — 2026-04-20

### Added — Wave 39: Agent Graph Validation
- Compile-time cycle detection in agent spawn graphs (DFS, `AgentGraphCycle` error)
- Unknown spawn target detection (`AgentSpawnUnknown` error)

### Added — Wave 38: DataFrame Builtins (Polars)
- `df_read_csv / df_filter / df_select / df_groupby / df_agg / df_sort / df_write_csv`
- Gated behind `--features dataframe`

### Added — Wave 37: Generic LLM Output
- `llm_structured<T>(provider, model, prompt)` — typed struct from LLM JSON output
- `GenericCall` AST node + Pratt-parser lookahead for `ident < Type > (` disambiguation

### Added — Wave 36: Tensor Builtins (ndarray)
- `tensor_new / tensor_zeros / tensor_ones / tensor_add / tensor_mul / tensor_dot`
- Gated behind `--features tensor`

### Added — Wave 35: Performance Benchmarks + Optimisations
- `vargc build --release` properly threads through to `cargo build --release`
- Fibonacci benchmark: Varg 15 ms vs Python 695 ms (46× faster)

---

## [0.11.0] — 2026-04-10

### Added — Waves 28–34
- Binary I/O, config cascade, readline/REPL (Wave 29)
- HITL (human-in-the-loop) approval gates, rate limiting (Wave 30)
- LLM cost / budget tracking `@[Budget]` (Wave 31)
- Agent checkpoint/resume, SSE server (Wave 32)
- Typed inter-agent channels, property-based testing (Wave 33)
- Multimodal (image/audio/vision), workflow DAG, package registry (Wave 34)

---

## [0.9.0] — 2026-03-20

### Added — Waves 20–27
- Knowledge graph, vector store, agent memory (Waves 20–21)
- Observability & tracing (Wave 22)
- MCP server mode (Wave 23)
- Reactive pipelines, agent orchestration, self-improving loop (Waves 24–26)
- Base64 + PDF generation (Wave 27)

---

## [0.7.0] — 2026-03-01

### Added — Foundation (Waves 1–19)
- Core language: agents, contracts, generics, closures, async/await
- OCAP security model
- Standard library: 103+ builtins
- Pipe operator, retry/fallback, string interpolation, tuples, ranges, HashSet
- VS Code extension, LSP (hover, completion, diagnostics)
- HTTP server (axum), SQLite (rusqlite), WebSocket (tungstenite), MCP client
- Test framework: `@[Test]`, `@[BeforeEach]`, `@[AfterEach]`, `assert_*` family
- Token efficiency: 1.16× vs Python (Varg is more concise than Python)
