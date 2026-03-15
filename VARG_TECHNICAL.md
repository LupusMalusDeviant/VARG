# Varg — Technical Deep Dive

> A compiled programming language purpose-built for autonomous AI agents.
> C#-like ergonomics. Rust-level performance. Native AI primitives.

---

## 1. What Is Varg?

Varg is a statically typed, compiled programming language that transpiles to Rust.
It targets a specific gap in the language landscape: **no existing language treats AI agents as first-class citizens**.

- Python is slow and dynamically typed
- TypeScript needs a runtime (Node/Bun/Deno)
- Rust is fast but verbose and hostile to rapid prototyping
- C# is managed, GC-dependent, and ships a 150MB+ runtime

Varg takes a different approach:

```
.varg source → vargc (Rust compiler) → Rust source → cargo build → native binary
```

The result: **native executables under 160KB**, no runtime, no garbage collector, no VM.

---

## 2. Language Architecture

### 2.1 Compiler Pipeline

```
┌──────────────┐    ┌──────────┐    ┌──────────┐    ┌─────────────┐    ┌──────────┐
│  .varg file  │───►│  Lexer   │───►│  Parser  │───►│ TypeChecker │───►│ CodeGen  │
│              │    │ (Logos)  │    │ (RD)     │    │ (Semantic)  │    │ (Rust)   │
└──────────────┘    └──────────┘    └──────────┘    └─────────────┘    └──────────┘
                                                                            │
                                                                            ▼
                                                                     ┌──────────┐
                                                                     │ cargo    │
                                                                     │ build    │
                                                                     └──────────┘
                                                                            │
                                                                            ▼
                                                                     ┌──────────┐
                                                                     │ native   │
                                                                     │ .exe     │
                                                                     └──────────┘
```

| Phase | Implementation | LOC | Tests |
|-------|---------------|-----|-------|
| Lexer | Logos-based tokenizer | 580 | 28 |
| Parser | Hand-written recursive descent | 4,610 | 124 |
| TypeChecker | Semantic analysis + OCAP | 4,224 | 106 |
| CodeGen | AST → Rust source emission | 3,527 | 111 |
| CLI (vargc) | Build/run/emit-rs commands | 664 | 1 |
| Runtime | Stdlib (crypto, net, db, llm) | 18+ | 28 |
| LSP | VS Code language server | 139 | — |
| **Total** | | **~15,500** | **417** |

### 2.2 Why Transpile to Rust?

Instead of writing a custom backend (LLVM, Cranelift), Varg leverages Rust's mature ecosystem:

- **Zero-cost abstractions** — Rust's optimizer handles the heavy lifting
- **Memory safety** — borrow checker catches bugs Varg's codegen might miss
- **Ecosystem access** — `use extern` maps directly to Cargo crates
- **Cross-platform** — anywhere Rust compiles, Varg compiles
- **Proven optimizations** — LLVM backend via rustc, battle-tested for 10+ years

The trade-off is compile time (~0.5s for a benchmark), but the resulting binary is indistinguishable from hand-written Rust in performance.

---

## 3. Core Language Features

### 3.1 Agents (First-Class Actors)

Every Varg program is an **agent** — a stateful actor with lifecycle hooks.
This isn't a library pattern. It's built into the language grammar.

```varg
agent ApiProcessor : DataProcessor {
    string last_result;
    int request_count;

    public void Init() {
        last_result = "";
        request_count = 0;
    }

    public string Process(string input) {
        request_count += 1;
        last_result = $"[{request_count}] {input.trim()}";
        return last_result;
    }

    public void Run() {
        for i in 0..5000 {
            Process($"  Request {i}  ");
        }
        print $"Processed {request_count} requests";
    }
}
```

**What this compiles to:** A Rust `struct` with an `impl` block, a `main()` that instantiates it, calls `Init()`, then `Run()`. No runtime overhead. No actor framework. Pure compiled code.

### 3.2 Contracts (Interface-First Design)

Contracts are Varg's answer to interfaces — but enforced at compile time with the type checker.

```varg
contract DataProcessor {
    string Process(string input);
    int GetCount();
}

agent MyAgent : DataProcessor {
    // Must implement Process() and GetCount()
    // or compilation fails
}
```

The TypeChecker validates contract compliance before codegen. Missing methods produce clear errors with source locations.

### 3.3 OCAP Security (Object-Capability Model)

Varg enforces capability-based security at the language level. Privileged operations require explicit capability tokens passed as parameters:

```varg
// This is a compile error — capabilities can't be constructed outside unsafe blocks:
let token = FileSystem;  // ERROR: CapabilityConstructionOutsideUnsafe

// Correct: capabilities must be explicitly granted
public void ReadFile(string path, cap FileSystem fs) {
    var content = fs.read(path);
}
```

The TypeChecker tracks capability flow through the call graph. An agent that doesn't receive a `NetworkAccess` capability literally cannot make HTTP requests — enforced at compile time, not runtime.

### 3.4 Native AI Types

Varg includes AI-specific types in the language itself:

```varg
use os Prompt, Context, Tensor, Embedding;

// LLM inference is a language primitive
var response = llm_infer("gpt-4", prompt, context);

// Environment variables for API keys
var api_key = env("OPENAI_API_KEY");
```

### 3.5 Standalone Functions

Functions exist outside agents for utility logic:

```varg
fn fibonacci(int n) -> int {
    var fa = 0;
    var fb = 1;
    for i in 0..n {
        var temp = fb;
        fb = fa + fb;
        fa = temp;
    }
    return fa;
}
```

These compile to plain Rust functions — no struct, no self, no overhead.

### 3.6 String Interpolation

```varg
var name = "Varg";
var version = "1.0";
print $"Agent: {name} v{version}";
// Compiles to: println!("{}", format!("Agent: {} v{}", name, version));
```

### 3.7 For-In Loops & Ranges

```varg
// Array iteration
var items = [10, 20, 30, 40, 50];
for n in items {
    print $"Item: {n}";
}

// Range (exclusive)
for i in 0..10 { /* 0 to 9 */ }

// Range (inclusive)
for i in 1..=10 { /* 1 to 10 */ }
```

### 3.8 Tuples

```varg
var coords = (10, 20);
print $"X: {coords.0}, Y: {coords.1}";
```

### 3.9 Retry/Fallback (LLM-Native Error Handling)

Built into the language for unreliable operations (API calls, LLM inference):

```varg
var response = retry(3) {
    Fetch(url)
} fallback {
    "cached-response"
};
```

### 3.10 Generics

```varg
struct Pair<K, V> {
    K key;
    V value;
}
```

The TypeChecker validates generic argument counts at compile time:
- `Pair<int>` → Error: expected 2 type arguments, found 1
- `Pair<int, string, bool>` → Error: expected 2, found 3

### 3.11 Return Path Analysis

The TypeChecker verifies that all code paths return a value:

```varg
fn classify(int score) -> string {
    if score > 80 {
        return "good";
    }
    // ERROR: MissingReturn — not all code paths return a value
}
```

### 3.12 Pattern Matching & Enums

```varg
enum Color {
    Red(int intensity),
    Green(int intensity),
    Blue(int intensity)
}

match color {
    Red(i) => print $"Red: {i}",
    Green(i) => print $"Green: {i}",
    _ => print "Other"
}
```

---

## 4. Performance Benchmarks

### 4.1 Test Setup

**Hardware:** Windows x64, MSVC toolchain
**Workload:** 1000 iterations of:
- Fibonacci(40) — integer arithmetic
- 300×300 nested loop — branch prediction / cache
- sum_range(0, 50000) — loop + accumulation

All implementations produce identical output (`1352309155000`), verifying correctness.

### 4.2 Results: Runtime Performance

| Language | Runtime | Median (ms) | vs. Varg |
|----------|---------|-------------|----------|
| **Varg** | Native binary (135 KB) | **13** | **1.0x** |
| C# | .NET 9 (JIT) | 67 | 5.2x slower |
| Node.js | V8 (JIT) | 70 | 5.4x slower |
| Python | CPython 3.14 | 2,595 | **200x slower** |

```
Varg     ██ 13ms
C#       ██████████ 67ms
Node.js  ██████████ 70ms
Python   ██████████████████████████████████████████████████ 2,595ms
```

### 4.3 Why Varg Is Fast

**It's not magic — it's Rust.**

Varg compiles to optimized Rust, which compiles to native machine code via LLVM.
There is:
- No garbage collector pause
- No JIT warmup time
- No interpreter overhead
- No V8/CLR runtime to load
- No dynamic dispatch (everything is monomorphized)

The 13ms runtime includes process startup. The actual computation is near-instantaneous.

### 4.4 Binary Size

| Language | Deployable Size | Dependencies |
|----------|----------------|--------------|
| **Varg** | **135 KB** (.exe) | None — fully static |
| C# | 5 KB (.dll) + 150 MB runtime | .NET Runtime required |
| Node.js | 2 KB (.js) + 100 MB runtime | Node.js required |
| Python | 1.5 KB (.py) + 80 MB runtime | Python required |

A Varg binary is **self-contained**. Copy the .exe to any Windows x64 machine and it runs.
No `dotnet`, no `node`, no `pip install`, no Docker container, no virtual environment.

### 4.5 Compile Time

| Phase | Time |
|-------|------|
| Varg → Rust transpilation | ~190 ms |
| Rust → native binary (release) | ~380 ms |
| **Total: source → executable** | **~570 ms** |

For comparison:
- `dotnet build -c Release`: ~1,100 ms
- `tsc && node`: ~300 ms (but still needs Node runtime)
- Rust (hand-written equivalent): ~400 ms

### 4.6 Lines of Code Comparison

The same benchmark logic in each language:

| Language | LOC | Boilerplate |
|----------|-----|-------------|
| Python | 41 | Minimal |
| Node.js | 50 | Minimal |
| **Varg** | **55** | Agent/Init structure |
| C# | 65 | Class + using + Main |

Varg's LOC is between JavaScript and C# — closer to scripting languages than systems languages.
For comparison, the equivalent hand-written Rust would be ~90 LOC (with `mod`, `use`, explicit types, lifetime annotations).

---

## 5. Agent-Specific Features (What Makes Varg Unique)

### 5.1 The Problem with Existing Languages for AI Agents

Building an AI agent in Python/TypeScript today requires:

```python
# Python: 47 lines of boilerplate for a simple agent
import asyncio
import aiohttp
from dataclasses import dataclass
from typing import Protocol

class DataProcessor(Protocol):
    def process(self, input: str) -> str: ...
    def get_count(self) -> int: ...

@dataclass
class ApiProcessor:
    last_result: str = ""
    request_count: int = 0

    def process(self, input: str) -> str:
        self.request_count += 1
        self.last_result = f"[{self.request_count}] {input.strip()}"
        return self.last_result

    def get_count(self) -> int:
        return self.request_count

async def main():
    agent = ApiProcessor()
    for i in range(5000):
        agent.process(f"  Request {i}  ")
    print(f"Processed {agent.request_count} requests")

asyncio.run(main())
```

### 5.2 The Same Logic in Varg

```varg
contract DataProcessor {
    string Process(string input);
    int GetCount();
}

agent ApiProcessor : DataProcessor {
    string last_result;
    int request_count;

    public void Init() {
        last_result = "";
        request_count = 0;
    }

    public string Process(string input) {
        request_count += 1;
        last_result = $"[{request_count}] {input.trim()}";
        return last_result;
    }

    public int GetCount() {
        return request_count;
    }

    public void Run() {
        for i in 0..5000 {
            Process($"  Request {i}  ");
        }
        print $"Processed {request_count} requests";
    }
}
```

**Key differences:**
- No `import` statements — agents, contracts, and string interpolation are built-in
- No `self.` prefix for field access inside the agent
- No `async/await` ceremony for the basic case
- Contract compliance is checked at compile time
- The result is a 141 KB native binary, not a script requiring a runtime

### 5.3 Environment Variables & Configuration

```varg
var api_key = env("OPENAI_API_KEY");
var base_url = env("API_BASE_URL");
```

Compiles to `std::env::var("...").unwrap_or_default()`. No dotenv library needed.

### 5.4 Multi-Agent Communication

```varg
agent Worker {
    public void Init() { }
    public string DoWork(string task) {
        return $"completed:{task}";
    }
    public void Run() { }
}
```

Agents can call each other's methods directly. The TypeChecker validates that called methods exist and have correct signatures.

### 5.5 Retry/Fallback for Unreliable Operations

AI agents deal with unreliable external services (LLM APIs, network calls). Varg makes this a first-class pattern:

```varg
var response = retry(3) {
    api.Call(prompt)
} fallback {
    "cached-response"
};
```

This compiles to a Rust loop with error handling — no library needed, no `try/catch` boilerplate.

---

## 6. Type System

### 6.1 Primitive Types

| Varg Type | Rust Equivalent | Notes |
|-----------|----------------|-------|
| `int` | `i64` | 64-bit signed integer |
| `float` | `f64` | 64-bit float |
| `string` | `String` | Heap-allocated, UTF-8 |
| `bool` | `bool` | |
| `void` | `()` | |

### 6.2 Collection Types

| Varg Type | Rust Equivalent |
|-----------|----------------|
| `[int]` | `Vec<i64>` |
| `map<string, int>` | `HashMap<String, i64>` |
| `(int, string)` | `(i64, String)` |
| `list<T>` | `Vec<T>` |

### 6.3 Type Safety Features

The TypeChecker performs:

1. **Type inference** — `var x = 42;` infers `int`
2. **Return type validation** — function return types must match
3. **Return path analysis** — all code paths must return a value
4. **Generic argument count validation** — `Pair<int>` on a 2-param generic is an error
5. **Contract compliance** — agents implementing contracts must provide all methods
6. **OCAP flow analysis** — capability tokens can only be constructed in `unsafe` blocks
7. **Collection method type inference** — `arr.first()` returns element type, not `Dynamic`

### 6.4 Error Messages

```
TypeError: Missing return value in function 'classify'
  --> src/main.varg:12:1
   | fn classify(int score) -> string {
   |    ^^^^^^^^ not all code paths return a value

TypeError: Wrong number of type arguments for 'Box'
  --> src/main.varg:5:10
   | var x: Box<int, string> = ...
   |        ^^^ expected 1 type argument, found 2
```

---

## 7. Compilation Model

### 7.1 What vargc Does

```bash
# Transpile + compile to native binary
vargc build myagent.varg
# → Creates myagent.exe (or myagent on Linux/macOS)

# Emit intermediate Rust source
vargc emit-rs myagent.varg
# → Creates myagent.rs (for inspection)

# Build and immediately run
vargc run myagent.varg
```

### 7.2 Generated Rust Quality

Varg generates readable, idiomatic Rust. Example transformation:

**Varg input (55 LOC):**
```varg
agent GenericBench {
    int processed;
    public void Init() { processed = 0; }
    public void Run() {
        var numbers = [10, 20, 30, 40, 50];
        var sum = 0;
        for n in numbers {
            sum += n;
        }
        print $"Sum: {sum}";
    }
}
```

**Generated Rust (~95 LOC including boilerplate):**
```rust
struct GenericBench {
    processed: i64,
}

impl GenericBench {
    pub fn Init(&mut self) {
        self.processed = 0;
    }
    pub fn Run(&mut self) {
        let mut numbers = vec![10i64, 20i64, 30i64, 40i64, 50i64];
        let mut sum = 0i64;
        for n in numbers.iter() {
            sum += n;
        }
        println!("{}", format!("Sum: {}", sum));
    }
}

fn main() {
    println!("[VargOS] Bootstrapping Runtime...");
    let mut agent = GenericBench { processed: 0i64 };
    agent.Init();
    agent.Run();
}
```

### 7.3 Codegen Optimizations

- **`push_str` over `format!`** for simple string concatenation
- **`.clone()` insertion** only where Rust's borrow checker requires it (rvalue contexts)
- **Source maps** — generated Rust contains `// .varg:N` comments mapping back to original lines
- **Release builds** — `cargo build --release` with full LLVM optimization
- **`#[allow]` annotations** — suppresses Rust naming convention warnings for Varg's PascalCase methods

---

## 8. Tooling

### 8.1 VS Code Extension

Varg ships with a VS Code extension providing:
- **Syntax highlighting** for `.varg` files
- **LSP integration** via `varg-lsp` (tower-lsp based)
- Diagnostics, hover information, and completions

### 8.2 MCP Schema Generation

Varg can auto-generate MCP (Model Context Protocol) tool schemas from agent method signatures — enabling direct integration with AI assistants.

---

## 9. Benchmark Suite

The repository includes 8 benchmarks validating different language features:

| Benchmark | Features Tested | LOC | Binary |
|-----------|----------------|-----|--------|
| agent_benchmark | Contracts, agents, string interpolation, standalone fn | 45 | 141 KB |
| bench4_generics | For-in loops, tuples, ranges, string methods, array ops | 64 | 156 KB |
| bench5_async_agents | Agent lifecycle, method calls, string building | 71 | 141 KB |
| bench6_patterns | FizzBuzz, data processing, nested conditionals, string accumulation | 97 | 140 KB |
| bench7_fullstack | env(), contracts, for-in, string interpolation | 63 | 157 KB |
| bench_perf | Performance: Fibonacci, matrix, arrays, strings (100 iter) | 55 | 135 KB |
| bench_perf_heavy | Performance: Fibonacci(40), 300x300 matrix, ranges (1000 iter) | 55 | 135 KB |

All benchmarks compile in under 600ms and produce binaries under 160KB.

---

## 10. Comparison Matrix

| Feature | Varg | Python | TypeScript | C# | Rust |
|---------|------|--------|------------|-----|------|
| **Runtime performance** | Native | ~200x slower | ~5x slower | ~5x slower | Native |
| **Binary size** | 135 KB | Needs runtime | Needs runtime | Needs runtime | ~135 KB |
| **Startup time** | <1ms | ~30ms | ~50ms | ~30ms | <1ms |
| **GC pauses** | None | Yes | Yes | Yes | None |
| **Type safety** | Static | Dynamic | Static* | Static | Static |
| **Agent as primitive** | Yes | No | No | No | No |
| **Capability security** | Built-in | No | No | No | No |
| **AI types (Prompt, Tensor)** | Built-in | Library | Library | Library | Library |
| **String interpolation** | `$"..."` | f-strings | Template literals | `$"..."` | `format!()` |
| **Retry/fallback** | Built-in | Library | Library | Library | Library |
| **Learning curve** | C#-like | Easy | Medium | Medium | Hard |
| **Deployment** | Copy .exe | venv + pip | node_modules | .NET runtime | Copy binary |

*TypeScript's type safety is erased at runtime

---

## 11. Project Status

| Metric | Value |
|--------|-------|
| Compiler tests | 417 (0 failures) |
| Compiler LOC | ~15,500 (Rust) |
| Development waves completed | 11 of 11 |
| Benchmarks | 8 (all passing) |
| Supported platforms | Windows x64 (Linux/macOS via Rust toolchain) |

### Completed Development Waves

1. **Housekeeping** — Naming, docs, git init
2. **Compiler Foundation** — Error reporting, type system, test coverage
3. **Core Features** — OCAP security, SurrealQL AST, closures/pattern matching
4. **Runtime & Tooling** — Runtime helpers, VS Code LSP
5. **Core Optimizations** — break/continue, string methods, compound assignment
6. **LLM-Native Features** — Pipe operator, retry/fallback, MCP schemas, agent messaging
7. **Language Identity** — Agent lifecycle, actor concurrency, OCAP flow, prompt templates
8. **Language Maturity** — Standalone functions, module system, async runtime, generics, contracts
8.5. **Stabilization** — Self-type fixes, interop, string interpolation hardening
9. **Production Polish** — Ranges, tuples, trait bounds, stdlib, iterator chains, source maps
10. **Production Readiness** — Parser fixes, LLM provider abstraction, env() builtin
11. **Type Safety Hardening** — Generic validation, return paths, OCAP construction, benchmarks

---

## 12. Vision: VargOS

Varg is designed as the native language for **VargOS** — a user-space operating system where:

- **SurrealDB** serves as the filesystem (vector-indexed, graph-queryable)
- **Agents** are the unit of execution (not processes)
- **VRAM multiplexing** enables multiple LLM agents to share GPU memory
- **Capability tokens** replace Unix permissions

The compiler is Phase 1. The OS is Phase 2.

---

*Built with Rust. Benchmarked against the industry. Designed for what comes next.*
