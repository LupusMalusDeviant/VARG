# Varg Changelog

All notable changes to the Varg language and compiler are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Varg uses [Semantic Versioning](https://semver.org/).

---

## [2.0.0] — 2026-08-27

### Changed — breaking

- **Reading a map is nullable.** `m[key]` is `T?` instead of `T`. It used to hand back the value
  type and unwrap at runtime, so a key that is simply not there took the program down with
  `called Option::unwrap() on a None value` — a raw Rust panic for an ordinary outcome. C#
  throws in the same situation; carrying the absence in the type is what makes this safer rather
  than merely equivalent. Resolve it with `or`, or test it with `== null`.
  **Indexing a list is unchanged**: a position out of range is a mistake, a missing key is not.
- **An unresolved optional cannot be passed to a builtin.** `parse_int(m["k"])` used to go
  through the front end and fail in rustc against generated code. Arithmetic on an optional was
  already refused; this closes the same hole on call arguments. Rendering one stays legal —
  interpolation and concatenation both show the value or `null`.

Migration: add `or <fallback>` where a map read fed a plain value. The compiler names every site.

### Added — the WebAssembly target actually works

- `vargc build --target wasm32-wasip1` produces a `.wasm` module. It never could before, for any
  program: the generated manifest declared `[lib] crate-type = ["cdylib"]` while only
  `src/main.rs` was written, so cargo refused it outright. A WASI module is a command, which a bin
  crate already produces.
- Behind that sat a second one. `rusqlite` was an unconditional dependency — despite a comment
  in the manifest claiming `wasm-safe` excluded it — so `libsqlite3-sys` tried to build its
  bundled C for wasm32. It is now behind a `sqlite` feature that the graph, the vector store,
  agent memory and RAG pull in when a program uses them, so nothing silently loses persistence.
- A missing rustup target is now named along with the command that adds it, instead of surfacing
  as a cargo error against generated code.
- CI builds a module and checks its magic bytes.

### Changed — performance

Measured against C# (.NET 10), TypeScript/Node 24 and Python 3.14 over five workloads, timed
in-process. Four changes, each at a point where work was done and thrown away:

- **A map write goes through `get_mut`.** It was always `insert(key.clone(), value)` — one
  string allocation per write even when the entry already existed, which for counting is the
  entire cost.
- **`foreach` over `split`/`chars` iterates lazily** instead of building a complete list first.
- **Concatenation no longer allocates its operands.** `"item-" + n.to_string()` became
  `format!("{}{}", "item-".to_string(), n.to_string())`; both are already `Display`. A literal
  operand goes into the format string, and two literals are joined at compile time.
- **Generated programs use mimalloc** (about 130 KB of binary, not on wasm32). C# puts
  short-lived objects in a GC nursery by bumping a pointer, which the platform allocators here
  cannot match.

fib(32) 4 ms against C#'s 12; a million-integer fill/sum/sort 15 ms against 132; 200k strings
built and joined 7 ms against 11; 500k records filtered and aggregated 1 ms against 4. Word
frequency over 200k distinct keys is still 18 ms against 12: one key copy per new entry plus
SipHash against .NET's faster string hash. Removing that copy needs a move analysis across block
boundaries, where a wrong answer emits wrong code, so it stays.

### Fixed — a program laid out across directories

Writing a 206-line system across `domain/`, `store/`, `agents/` and `api/` found six defects, all
in the constructions a program needs to grow past one file:

- **Imports resolved only against the importing file's own directory**, so every directory was an
  island: `store/` could not see `domain/`. The entry file's directory is now tried as well.
- **A parameter lost to a field of the same name, silently.** A bare name in an agent method
  became `self.<name>` whenever the agent had such a field, whatever else was in scope —
  `greet(string name)` returned the field and dropped the argument. It compiled, ran, and gave
  the wrong answer.
- **A DI constructor whose parameter was named after its field vanished from the output** without
  a word, and the call site then failed against a Rust item nobody wrote. Such a constructor may
  now also do work that does not touch `self`; what still cannot be lowered is refused by name.
- **An agent implementing a contract and using `?` could not compile at all.** The body was
  emitted a second time under the trait's signature, putting `?` in a method returning `()`. The
  trait impl now delegates to the inherent method.
- **An injected dependency was cloned**, but an agent behind a contract has no `Clone`, so no DI
  call site passing a variable compiled. Injecting hands the dependency over, so it moves.
- **`return` counted through neither `unsafe` nor an exhaustive match.** Every method working
  behind a capability was reported as missing a return, as was any function over an enum written
  with one arm per variant. Codegen had the same gap, asked of the generated text — after an
  `unsafe` block the last line is `}`, so an `Ok(())` was appended after a value.

### Fixed — an error in a module pointed at the entry file

The modules of a program are merged into one AST that records nothing about where each item came
from, so an error about something a module declares was reported against the entry file's first
line — naming neither the module nor the line. `vargc` now keeps every file it loaded with its
text and asks each of them. An error mentioning no name at all falls back to the item it was
found in, and only when the entry file does not itself declare that item.

### Fixed — the compiler and the REPL announced different versions

Both banners wrote the version out by hand, and they drifted: the REPL said v0.12.0 while the
compiler said v1.0.0. Both read it from the crate now.

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

### Added — every runnable builtin is now run

Four more golden programs bring the executed surface from 257 to **366 of 399**. The 33 that
remain need a network, an interactive terminal, or block until killed; each is listed by name in
`docs-check` with the reason, so the boundary is explicit and a new builtin cannot join it by
accident. That is the check's third gate: documented, consistent with the compiler, **and run**.

Running them found five more defects, every one of which type-checked:

- **`sse_shutdown` could not be called, and would not have worked.** Codegen passed a reference
  while the runtime took the handle by value, so no program using it compiled — the same shape as
  `http_sse_route` taking the inner server. And what it did was drop one `Arc` clone while the
  caller and the server kept theirs, so it closed nothing. It sets a flag the pushes check now.
- **`channel_try_recv` and `channel_recv_timeout` answered `""` for an empty channel** — exactly
  what a legitimately empty message looks like, in the primitive whose whole job is telling those
  apart. A test named `test_channel_try_recv_empty_returns_empty_string` had pinned it.
- **A value could be put in only one list.** `var a = [s, "1"]; var b = [s, "2"];` failed with
  "use of moved value", about a move nobody wrote.
- **`df_with_column` took `&[f32]`** while Varg's `float` is f64, so it could not be called at all.
- **`time_parse` rejected a date without a time**, which is the form REFERENCE shows.

Corrected along with them: `fan_in`'s reducer takes the whole list rather than a running
accumulator, `rag_index` takes its metadata as a JSON string, and `context_from` takes the query
result as text — all three were documented wrongly in the previous round.

### Changed — CI

The generated programs build into `~/.cache/varg/target`, shared between programs but cached
between runs by nothing, so every run recompiled axum, polars, duckdb and tantivy from scratch:
21 minutes against 85 seconds for the same suite locally, and growing with each golden program.
That directory is cached now. `actions/checkout` moves to v5, which stops the Node 20 warning.


### Added — modules can live in subdirectories

`import a.b;` parses as "the item `b` from module `a`", so a dotted name never reached the module
resolver and its nested-path branch could not fire. A program was a flat pile of files beside its
entry point, which is what kept one from growing. **Nothing else about size was ever the problem:**
1000 standalone functions, 200 agents, 40 modules, 60 levels of nesting and 2000 statements in one
method all compile in well under a second.

`import core.util.strings;` now finds `core/util/strings.varg`. The dotted name is walked from the
longest prefix to the shortest, so `import modules.flat.triple;` finds `modules/flat.varg` and
selects `triple` from it, and a plain `import math.triple;` still finds `math.varg` first — nothing
that worked before changes meaning.

### Fixed — six defects found by running the documentation

Three new golden programs cover 56 builtins the documentation describes and no program ran. Every
one of these type-checked perfectly and failed only when executed:

- **String interpolation silently discarded whatever it could not parse.** The mini-parser inside
  `{...}` stopped at the first token it could not continue and nothing looked at the rest, so
  `$"{a >= 1 and a <= 2}"` printed the value of `a >= 1` — with `a = 5` that is `true`, where the
  whole condition is false. Any typo inside the braces produced a plausible wrong value, in
  silence, in the construct the language is most used through. One of this project's own golden
  programs turned out to contain the mistake.
- **`(n * 2).to_string()` lost its parentheses** and became `n * 2.to_string()`. The same
  precedence mistake that made `abs(-5.0)` return -5, in the receiver half of the expression.
- **`any`, `all`, `take`, `skip`, `zip` and `flat_map` consumed the collection**, so a list could
  be used exactly once. `map`/`fold`/`enumerate`/`flatten` had been fixed; these had not.
- **`prop_check`, `prop_assert` and `assert_throws` wrapped their lambda in a second closure**, so
  `assert_throws` measured a body that merely *created* a closure, always succeeded, and reported
  that nothing was thrown. `assert_throws` also generated an `if` with no `else`, which is a
  statement, so using its documented `bool` result did not compile.
- **`and_then` accepted a closure returning a plain value** and left rustc to report "expected
  `Result<_, String>`, found `i64`".
- **`time_parse("2024-01-15", "%Y-%m-%d")` — the example in REFERENCE — could never have worked.**
  It used `NaiveDateTime`, which requires a time component. A date alone now parses as midnight.

### Fixed — try/catch caught nothing on the main thread

The panic hook exits the process when the failing thread is `main`, and it runs *before* the
unwind reaches any `catch_unwind`. So `try/catch`, a documented core feature, was inert wherever
the entry point runs: the program simply died at the failure and neither the catch nor anything
after it ran. It had only ever been exercised on spawned agent threads. A thread-local depth
counter now marks the stretches being caught, and the hook stays quiet inside them.


### Fixed — a failing program said it had succeeded

**An error propagating out of the entry point ended the program silently, with status 0.** The
generated `main` called `instance.Run();` and discarded the result, so a `?` that reached the top
simply stopped the output halfway — and a shell, a CI job or an agent running the binary read that
as success. The entry point now reports what failed and exits 1.

Found by running the failure paths of the previous change rather than type-checking them, which
also turned up **`self_improver_new(":memory:")` trying to create a file called
`:memory:_learnings_episodic.vector.db`**: the in-memory marker was lost in the derived name. It
had been panicking inside the vector store all along; making that store fallible is what finally
let the message out.

`golden/run.sh` checks exit codes now, for programs that carry an `expected/<name>.exit` file. It
reads `PIPESTATUS[0]` rather than `$?`, which after a pipe belongs to the normaliser — with `$?`
the check would have passed for every program no matter how it ended.


### Changed — the runtime reports failures instead of panicking

An inventory of every place the runtime could take the process down found 56 reachable ones
(the rest were test code, or mutex-poison recovery, which is the opposite of a panic). 48 are
gone; the 8 that remain either cannot fire or are the point, and now say so in the code.

The ones that mattered were all reachable with ordinary input:

- **A typo in SQL took the whole program down.** `duckdb_execute`, `duckdb_query` and
  `duckdb_open` panicked out of the driver; `db_open` did the same for SQLite. The most ordinary
  mistake anyone makes with a database was unrecoverable.
- **Tensor shape mismatches panicked from inside ndarray**, reporting "IncompatibleShape" and
  naming neither tensor. `tensor_add`, `tensor_sub`, `tensor_slice` and `tensor_matmul` did not
  even appear in the inventory, because ndarray panics through its operators rather than through
  an `expect` — the count was an undercount.
- **`df_filter("age >")` and `df_agg(df, cols, "avarage")` panicked on the caller's own string.**
  So did reading a CSV that is not there, and every column name with a typo in it.
- **`decrypt` returned its error as the plaintext.** A wrong password produced the string
  "[VargOS] decrypt error: wrong password or corrupted data", which the caller then stored,
  printed or sent on as though it were the secret — the same shape as the retired `file_read`.
  Three tests asserted exactly that behaviour.
- Full-text search (`fts_*`), the vector store, agent memory, the self-improver and
  `checkpoint_open` panicked on an unreachable path or a locked index. `checkpoint_open` also
  unwrapped its own in-memory fallback, turning a recoverable situation into a crash.

`or` gained a check while this was going on: the fallback was never compared against the value it
stands in for, so `proc_spawn(cmd) or "failed"` reached rustc as "expected `Arc<Mutex<ProcState>>`,
found `String`". It has its own error now, which also surfaces when the mistake sits in a caller.


### Fixed — five defects found by running the documentation

Type-checking the documentation was not enough. Running the newly documented builtins, rather
than only checking them, turned up five things that type-checked perfectly:

- **`"abc".reverse()` emitted `String::reverse`**, which does not exist. The comment in the
  code generator already said strings should reverse through their characters; only the comment
  did.
- **`clamp(15, 0, 10)` became `self.clamp(15, 0)`**, so rustc reported that the agent struct is
  not `Ord`. It is method-only like the string builtins, but a number is exactly what it is for,
  so it needed its own list — the one that rejects a scalar receiver would have rejected the
  correct spelling too. The same applies to `to_hex`, `to_binary` and `to_fixed`.
- **`or` never compared its fallback** against the value it stands in for. `proc_spawn(cmd) or
  "failed"` reached rustc as "expected `Arc<Mutex<ProcState>>`, found `String`".
- **`fold`, `map`, `enumerate` and `flatten` consumed the collection**, so a list could be used
  exactly once and the second use failed with "use of moved value". `filter` had already been
  fixed this way; these had not.
- **`unique` and `distinct` shared `dedup`'s implementation**, which removes only *adjacent*
  duplicates: `[3, 1, 2, 1].unique()` returned all four elements. They de-duplicate properly
  now, in first-seen order; `dedup` keeps the adjacent-only meaning its name states.

Also: `pad_left`, `pad_right` and `repeat` accepted a second argument and dropped it, so
`"x".pad_left(3, ".")` padded with spaces without a word about the fill character.

`golden/progs/documented_builtins.varg` runs 52 checks over the documented surface, because the
doc gate checks types and golden runs programs.


### Added — the other 117 builtins are documented

The reference and the guide between them covered 290 of the 407 builtins the compiler knows. The
117 that appeared in neither included the ones a web-facing agent reaches for first: `ws_route`,
`sse_open`/`sse_push`, `http_response_json`. A builtin nobody can find is a builtin nobody uses.

New sections: **Web Server** (routes, content types, SSE, WebSocket, and the restriction that a
handler cannot reach `self`), **SSE Client**, **Child Processes**, **Terminal Input** and
**Terminal Colours**. Existing sections gained what they were missing — binary file I/O and file
metadata, the platform directories, the rest of the string methods, `fold`/`reduce`/`unique`/
`flatten`/`enumerate`/`join`/`flat_map`, `json_keys`/`json_values`/`json_has`, `is_ok`/`is_some`/
`unwrap`, random numbers and `uuid`, the cached and streaming LLM calls, the vector index, the
workflow runner, fan-out/fan-in, and the MCP server's runtime tool swapping.

`docs-check/check.py` now also fails on a builtin no document mentions. Retired builtins are
exempt, read from the compiler's own retirement table so retiring one exempts it automatically.

### Fixed — eleven more documented calls, hidden behind placeholders

Sharpening the doc check turned up eleven signature errors it had been unable to see: a block is
classified by its first error, so one undeclared placeholder masked everything after it. That is
how `fs_write("trace.json", json, files)` sat unnoticed — `files` was undeclared, the block read
as illustrative, and the extra argument was never reached.

Nearly all were one mistake repeated: **a capability token passed as an argument**. `fs_read(path,
files)`, `exec(cmd, sys)`, `fetch(url, "GET", net)`, `pdf_save(doc, path, files)`,
`df_read_csv(path, cap)`. Holding the token in scope is what authorises the call; the builtins
keep their ordinary signatures. Also corrected: `prop_check` and `prop_assert`, and a pipe-operator
example ending in `send`, which needs a target.

The check now declares placeholders away one at a time and re-runs, escalating only an *arity*
complaint — how many arguments a call takes does not depend on what the placeholder held.


### Fixed — the web-facing surface

- **`http_response` set no `content-type` at all**, leaving every browser to sniff. HTML survives
  that; a stylesheet or a script served the same way does not. Two arguments now mean
  `text/html; charset=utf-8` — not a guess: every `http_response` call in this repository serves
  a page, and JSON has always had `http_response_json`. A third argument names any other type.
- **`http_sse_route` could not be called from Varg.** It took `&mut VargHttpServer` while
  `http_serve()` hands back a `VargHttpServerHandle`, so a program using it type-checked and then
  failed in rustc. Server-sent events were unreachable from the language for as long as the
  function existed.
- **`http_response` had no arity check**, so `http_response()` and `http_response(1,2,3,4)` both
  passed.

`examples/web_server.varg` now exercises every route kind — page, stylesheet, JSON API with query
parameters, POST body, SSE and WebSocket — and is built in CI, which is what would have caught an
unreachable route function.

### Fixed — documented signatures the compiler rejects

Twelve calls in REFERENCE.md and VARG_AGENT_GUIDE.md did not compile. Among them: capability
tokens shown as arguments (`fs_read("f", files)`) when having one in scope is what authorises the
call; `pdf_add_section` and `duckdb_execute` missing an argument; `registry_search` and
`rag_build_prompt` given one too many; `.sort()` ending an iterator chain when it mutates in place
and returns nothing; a lambda stored in a variable presented as needing no parameter type.

`docs-check/check.py` feeds every ```csharp block to `vargc check` and runs in CI. Four rounds of
this session found this same class of defect and nothing caught it, because nothing had ever fed
the documentation to the compiler.

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
