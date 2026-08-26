# Varg Changelog

All notable changes to the Varg language and compiler are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Varg uses [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Changed — breaking

- **The JSON accessors are Nullable.** `json_get`, `json_get_int`, `json_get_bool` and
  `json_get_array` return `string?` / `int?` / `bool?` / `string[]?` instead of a plain value
  with a default baked in. `""`, `0` and `false` used to mean an absent key, a value of the
  wrong kind, an explicit JSON null, a genuinely empty value and an unparseable document all at
  once — five situations, one answer, none of them distinguishable from the others. `null` now
  means "nothing there" and nothing else; resolve it with `or`, or test it with `== null`.
- **`json_get` renders values it used to discard.** A number, a bool or a nested object comes
  back as its text (`42`, `true`, `{"b":1}`) rather than as `""`. `json_get_array` renders
  non-string elements instead of dropping them, so `[1, 2]` yields `["1", "2"]`.
- **The typed accessors are strict.** `json_get_int` on the string `"42"` answers `null`.
- **`json_parse` returns `Result<JsonValue, string>`.** It used to lower to
  `from_str(..).unwrap_or(Value::Null)`, so a malformed document became an empty one and every
  later read reported its keys as merely absent — the parse failure was unobservable. The error
  carries serde's message, including line and column. Propagate it with `?`, ask with
  `.is_err()`, or drop the parse hop: the accessors read a raw JSON string directly.
- **An optional prints as its value, or as `null`.** It used to render through Rust's `Debug`,
  so a `string?` printed `Some("x")`. This affects `find`, `first` and `last` too.

Migration: add `or <fallback>` at call sites that relied on the old default, or handle the
absent case explicitly. The compiler points at every site — arithmetic on an optional and
comparing one against a real value are now compile errors naming `or`.

### Fixed

- **`or` on an optional did not compile.** It always lowered to `unwrap_or_else(|_| …)`, whose
  closure arity fits `Result` but not `Option`. Nullable types were effectively unusable: the
  one way to resolve them was rejected.
- **`first` and `last` were typed as the element type** while codegen emitted an `Option`, so
  the declared type never matched the generated code.
- **Concatenating or doing arithmetic on an optional leaked a rustc error** about
  `Option<String>` / `Option<i64>` instead of a Varg diagnostic.
- **Error spans could point into comments and strings.** The span for an error is recovered by
  searching the source for the name it mentions, which was a plain substring search — an error
  about `check` underlined the word "checked" inside a doc comment. The search now skips
  comments and string literals and requires identifier boundaries.

### Changed — builtins that invented an answer now report one

Eleven builtins handled failure by returning a plausible value. Each is either Nullable, where
the honest answer is "there is nothing there", or fallible, where something actually went wrong.

Nullable — `char_at`, `split_once`, `path_parent`, `path_extension`, `path_stem`:

- **`split_once` was the clearest case.** Without the separator it returned `("", "")` — which is
  exactly what splitting `"="` on `"="` legitimately gives. Success and failure were the same two
  values.
- `char_at` past the end of the string, and a path with no parent or extension, all answered `""`.
  `path_parent` additionally kept Rust's `Some("")` for a bare name; an empty parent is no parent.

Fallible — `time_format`, `json_set`, `json_merge`, `exe_path`, `readline_read`:

- **`time_format` did not degrade, it crashed.** chrono panics from its `Display` implementation
  on an unknown specifier, so `time_format(0, "%Q")` took the program down with "a Display
  implementation returned an error unexpectedly". The pattern is validated before use.
- **`json_set` silently discarded the document.** An unparseable one was replaced by an empty
  object and the write then reported success: `json_set("not json", "a", "1")` returned `{"a":1}`.
  The caller believes they modified their document; what came back has everything else gone.
- **`json_merge` dropped whichever side would not parse**, which looked exactly like a merge that
  legitimately changed nothing. It now says which side.
- `readline_read` already returned a `Result` naming EOF and Ctrl-C; codegen threw it away, so
  end of input was indistinguishable from the user pressing return.

`json_stringify` and `json_stringify_pretty` keep their default, and now say why in the code: a
`serde_json::Value` cannot fail to serialise — it holds neither a non-string map key nor a
non-finite float.

### Removed — five undocumented duplicates

`file_read`, `file_write`, `to_json`, `from_json` and `time_now` appeared in no documentation and
in no program in this repository, and each duplicated a documented builtin while handling failure
worse: `file_read` returned the error message as the file's contents, `file_write` called
`.unwrap()`. Calling one is an error naming the replacement (`fs_read`, `fs_write`,
`json_stringify`, `json_parse`, `timestamp`) and what it used to do.


### Fixed — the string and collection builtins say how they are called

- **`to_upper("abc")` compiled to `self.to_uppercase()`.** These builtins are methods on the
  value; the free-function spelling most languages would accept type-checked clean and then
  reached rustc as "no method named `to_uppercase` for `&mut A`" — a complaint about generated
  code, about a receiver nobody wrote. Measured across the family: eleven spellings leaked a
  rustc error and six produced an arity message about the method form, which named neither the
  real mistake nor a way out. All of them now say `` `to_upper` is a method on the value, not a
  free function — write `value.to_upper()` ``.

- **Extra arguments were silently dropped.** `"a".to_upper("x")` type-checked *and ran*: those
  branches returned a type without ever looking at their arguments, and codegen discards them.
  It is an error now, for all fifteen no-argument receiver builtins.

- **`len`/`length` are the exception, and are treated as one.** They are the only pair of this
  family that also reads as a free function, so each spelling has its own count: `len(xs)` takes
  the value, `xs.len()` takes nothing. `xs.len("x")` used to pass.

The pipe operator is unaffected — `name |> to_upper()` feeds the left value in as the receiver,
so it is the method form and keeps working.


### Fixed — lambda bodies are type-checked

- **Nothing inside a lambda was ever checked.** Undeclared variables, `5 > "x"`, `true + 1`,
  unhandled `Result`s, string methods on numbers — every error class the typechecker knows
  passed silently inside a lambda body and surfaced, if at all, as a rustc error about
  generated code. This covered route handlers, MCP tool handlers, pipeline steps and
  `map`/`filter`: the construct agent programs are mostly made of was the one construct the
  typechecker never looked inside.

  The cause was not that lambda bodies went unvisited — they were walked all along. Walking
  into a call's caller and arguments had been added to close an OCAP bypass, and it surfaced
  capability violations while discarding every other error, so that programs which type-checked
  before would continue to. That discard also threw away the lambda bodies' errors.

- **`to_upper(nope)` type-checked clean**, for the same reason: an undeclared name inside any
  builtin's argument was discarded too. It compiled to `self.to_uppercase()`, so rustc reported
  that `&mut A` has no such method — a complaint about generated code naming neither the
  undeclared name nor anything in the source.

  Errors from a caller or argument now surface when they are not inference gaps: anything from
  a lambda body, and any unresolved name. Everything else stays discarded, so programs that
  type-checked before still do.

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
