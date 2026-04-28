# Varg Language Reference

## Table of Contents
1. [Basics](#basics)
2. [Types](#types)
3. [Variables & Constants](#variables--constants)
4. [Functions](#functions)
5. [Agents](#agents)
6. [Structs](#structs)
7. [Enums & Pattern Matching](#enums--pattern-matching)
8. [Contracts (Interfaces)](#contracts)
9. [Control Flow](#control-flow)
10. [Error Handling](#error-handling)
11. [Collections](#collections)
12. [Closures & Lambdas](#closures--lambdas)
13. [Generics](#generics)
14. [OCAP Security](#ocap-security)
15. [Async & Concurrency](#async--concurrency)
16. [Agent Messaging](#agent-messaging)
17. [Modules & Imports](#modules--imports)
18. [String Interpolation](#string-interpolation)
19. [Pipe Operator](#pipe-operator)
20. [Retry / Fallback](#retry--fallback)
21. [Standard Library](#standard-library)
22. [Annotations](#annotations)
23. [Prompt Templates](#prompt-templates)
24. [Scientific Computing](#scientific-computing)

---

## Basics

### Comments

```csharp
// Single-line comment
/* Multi-line comment */
/// Doc comment (attached to next item)
```

### Print

```csharp
print "Hello, World!";
print 42;
print $"Value: {x}";
```

### Entry Point

Every Varg program needs an agent with a `Run()` method, or a standalone `fn main()`:

```csharp
agent App {
    public void Run() {
        print "Hello!";
    }
}

// Or:
fn main() {
    print "Hello!";
}
```

---

## Types

### Primitives

| Varg Type | Rust Equivalent | Example |
|-----------|----------------|---------|
| `int` | `i64` | `42` |
| `float` | `f64` | `3.14` |
| `string` | `String` | `"hello"` |
| `bool` | `bool` | `true` |
| `void` | `()` | — |
| `ulong` | `u64` | — |

### Complex Types

| Varg Type | Rust Equivalent | Example |
|-----------|----------------|---------|
| `string[]` | `Vec<String>` | `["a", "b"]` |
| `int[]` | `Vec<i64>` | `[1, 2, 3]` |
| `map<K, V>` | `HashMap<K, V>` | `{"key": "val"}` |
| `set<T>` | `HashSet<T>` | `set_of("a", "b")` |
| `(int, string)` | `(i64, String)` | `(42, "hello")` |
| `string?` | `Option<String>` | `null` |
| `Result<T, E>` | `Result<T, E>` | — |
| `List<T>` | `Vec<T>` | — |

### AI Native Types

| Type | Description |
|------|-------------|
| `Prompt` | Structured prompt (not a raw string) |
| `Context` | Conversation context with memory |
| `Tensor` | Multi-dimensional numeric array |
| `Embedding` | Float vector for semantic similarity |

### Type Aliases

```csharp
type UserId = string;
type Matrix = float[];
```

---

## Variables & Constants

```csharp
// Type-inferred
var name = "Varg";
let count = 42;

// Explicitly typed
string greeting = "Hello";
int age = 25;
float pi = 3.14159;

// Constants (compile-time)
const MAX_RETRIES = 5;
const API_URL = "https://api.example.com";

// Mutable (all vars are mutable by default, `mut` is accepted but optional)
let mut x = 10;
x = 20;
```

---

## Functions

### Standalone Functions

```csharp
fn add(int a, int b) -> int {
    return a + b;
}

fn greet(string name) -> string {
    return $"Hello, {name}!";
}

fn log(string msg) {
    print msg;
}

// With default parameters
fn connect(string host, int port = 8080) -> string {
    return $"{host}:{port}";
}

// Public (accessible from other modules)
pub fn helper() -> int {
    return 42;
}
```

### Calling Functions

```csharp
var sum = add(3, 4);        // 7
var msg = greet("World");   // "Hello, World!"
var url = connect("localhost"); // "localhost:8080"
```

---

## Agents

Agents are the primary building block in Varg. They have state, methods, and lifecycle hooks.

```csharp
agent Counter {
    // State (fields)
    int count;
    string name;

    // Lifecycle hooks
    public void on_start() {
        count = 0;
        name = "Counter";
        log_info("Agent started");
    }

    public void on_stop() {
        log_info($"Final count: {count}");
    }

    // Public methods
    public void Increment() {
        count += 1;
    }

    public int GetCount() {
        return count;
    }

    // Private method (only callable within this agent)
    private void Reset() {
        count = 0;
    }

    // Entry point
    public void Run() {
        self.Increment();
        self.Increment();
        print $"Count: {self.GetCount()}";
    }
}
```

### System Agents

System agents run with elevated privileges (Ring 0):

```csharp
system agent MemoryManager {
    public void Run() {
        // Can use unsafe, FFI, hardware access
    }
}
```

---

## Structs

```csharp
struct Point {
    int x;
    int y;
}

struct User {
    string name;
    int age;
    bool active;
}

// Generic struct
struct Pair<T> {
    T first;
    T second;
}
```

### Struct Literals

```csharp
var p = Point { x: 10, y: 20 };
var user = User { name: "Alice", age: 30, active: true };
var pair = Pair { first: 1, second: 2 };
```

### Impl Blocks

```csharp
impl Point {
    public fn distance(Point other) -> float {
        var dx = (self.x - other.x) as float;
        var dy = (self.y - other.y) as float;
        return sqrt(dx * dx + dy * dy);
    }

    public fn sum() -> int {
        return self.x + self.y;
    }
}
```

---

## Enums & Pattern Matching

### Enum Definition

```csharp
enum Color {
    Red,
    Green,
    Blue,
    Custom(int)         // Unnamed tuple field — accessed as field0 in Rust
}

enum Status {
    Active,
    Inactive,
    Error(string msg)   // Named field — both forms work
}
```

### Pattern Matching

Both dot-notation (`Status.Active`) and path-notation (`Status::Active`) are accepted in match arms:

```csharp
match status {
    Status.Active => {          // dot notation
        print "System is running";
    }
    Status::Error(msg) => {     // :: notation also valid
        log_error($"Error: {msg}");
    }
    _ => {
        print "Unknown status";
    }
}

// Match on integers
match code {
    200 => { print "OK"; }
    404 => { print "Not Found"; }
    _ => { print "Other"; }
}

// Match with guards
match value {
    x if x > 100 => { print "Large"; }
    x if x > 0 => { print "Positive"; }
    _ => { print "Zero or negative"; }
}
```

---

## Contracts

Contracts define interfaces that agents must implement:

```csharp
contract Loggable {
    void Log(string message);
    string GetName();
}

contract Serializable {
    string ToJson();
}

agent MyService implements Loggable, Serializable {
    string name;

    public void Log(string message) {
        print $"[{name}] {message}";
    }

    public string GetName() {
        return name;
    }

    public string ToJson() {
        return $"{{\"name\": \"{name}\"}}";
    }

    public void Run() {
        name = "MyService";
        self.Log("Started");
    }
}
```

---

## Control Flow

### If / Else If / Else

```csharp
if x > 10 {
    print "big";
} else if x > 5 {
    print "medium";
} else {
    print "small";
}

// Parentheses are optional
if (x == 0) {
    print "zero";
}
```

### While

```csharp
var i = 0;
while i < 10 {
    print i;
    i += 1;
}
```

### For Loops

```csharp
// For-in (preferred)
for item in items {
    print item;
}

// For-in with range
for i in 0..10 {
    print i;     // 0, 1, 2, ..., 9
}
for i in 0..=10 {
    print i;     // 0, 1, 2, ..., 10
}

// For-in over map (key-value destructuring)
for (key, value) in my_map {
    print $"{key} = {value}";
}

// Foreach (alternative syntax)
foreach item in items {
    print item;
}
foreach (var i in 0..5) {
    print i;
}

// C-style for
for (var i = 0; i < 10; i += 1) {
    print i;
}
```

### Break & Continue

```csharp
for item in items {
    if item == "skip" {
        continue;
    }
    if item == "stop" {
        break;
    }
    print item;
}
```

---

## Error Handling

### Result Type

Fallible operations return `Result<T, String>`:

```csharp
// Using ? operator (auto-propagates errors)
fn read_config(string path, FileAccess cap) -> Result<string, string> {
    var content = fs_read(path)?;  // Propagates error if fs_read fails
    return content;
}

// Using try/catch
try {
    var data = fs_read("config.json")?;
    print data;
} catch err {
    log_error($"Failed: {err}");
}

// Using `or` for fallback values
var name = fs_read("name.txt") or "default";

// Auto-Result wrapping: functions using ? automatically get Result return type
fn load(string path) -> string {
    var data = fs_read(path)?;    // Compiler auto-wraps return type as Result<string, string>
    return data;
}
```

### Throw

`throw` works inside `try` blocks (catches via `catch err`) **and** in any standalone function (becomes `return Err(...)`):

```csharp
fn validate(string input) -> string {
    if input == "" {
        throw "Input cannot be empty";  // → return Err(...) in generated Rust
    }
    return input;
}

if input == "" {
    throw "Input cannot be empty";      // inside try block → caught by catch
}
```

---

## Collections

### Arrays

```csharp
var numbers = [1, 2, 3, 4, 5];
var names = ["Alice", "Bob", "Charlie"];

// Methods
numbers.push(6);
var first = numbers.first();
var last = numbers.last();
var count = numbers.len();
var empty = numbers.is_empty();
numbers.sort();
numbers.reverse();

// Iterator chains
var evens = numbers.filter((n) => n % 2 == 0);
var doubled = numbers.map((n) => n * 2);
var found = numbers.find((n) => n > 3);
var has_big = numbers.any((n) => n > 100);
var all_pos = numbers.all((n) => n > 0);
```

### Maps

```csharp
var config = {"host": "localhost", "port": "8080"};
var scores = {"alice": 95, "bob": 87};

// Access
var host = config["host"];

// Methods
var keys = config.keys();
var vals = config.values();
var has = config.contains_key("host");
config.remove("port");

// Iterate
for (key, value) in config {
    print $"{key}: {value}";
}
```

### Sets

```csharp
var tags = set_of("rust", "varg", "ai");

tags.add("llm");
tags.contains("varg");    // true
tags.remove("ai");
tags.len();               // 3
tags.is_empty();          // false

for tag in tags {
    print tag;
}
```

### Tuples

```csharp
var pair = (42, "hello");
// Access via .0, .1
```

### Ranges

```csharp
0..10     // 0 to 9 (exclusive)
0..=10    // 0 to 10 (inclusive)
```

---

## Closures & Lambdas

```csharp
// Single expression (typed params)
var double = (int x) => x * 2;

// Call closure variable directly
var result = double(21);     // → 42

// Multi-line (block body)
var process = (string s) => {
    var upper = s.to_upper();
    return $"[{upper}]";
};

// Type-inferred params in context
var evens = numbers.filter((n) => n % 2 == 0);
var names = users.map((u) => u.name);
```

---

## Generics

### Generic Structs

```csharp
struct Box<T> {
    T value;
}

struct Pair<A, B> {
    A first;
    B second;
}
```

### Generic Functions

```csharp
fn identity<T>(T value) -> T {
    return value;
}
```

### Trait Bounds

```csharp
fn print_all<T: Display>(T[] items) {
    for item in items {
        print item;
    }
}
```

---

## OCAP Security

Varg enforces capability-based security at compile time. Privileged operations require capability tokens:

| Token | Operations |
|-------|-----------|
| `FileAccess` | `fs_read`, `fs_write`, `fs_append`, `fs_read_lines`, `fs_read_dir`, `create_dir`, `delete_file` |
| `NetworkAccess` | `fetch`, `http_request` |
| `DbAccess` | Database queries |
| `LlmAccess` | `llm_infer`, `llm_chat` |
| `SystemAccess` | `exec`, `exec_status` |

```csharp
agent SecureBot {
    // Declare needed capabilities in signature
    public string FetchPage(string url, NetworkAccess net) {
        return fetch(url, "GET")?;
    }

    public void SaveLog(string msg, FileAccess fs) {
        fs_append("log.txt", msg)?;
    }

    public void Run() {
        // Capabilities can only be constructed in unsafe blocks
        unsafe {
            var net = NetworkAccess {};
            var fs = FileAccess {};
            var html = self.FetchPage("https://example.com", net);
            self.SaveLog(html, fs);
        }
    }
}
```

Attempting to call `fs_read` without a `FileAccess` token in scope causes a compile-time error.

---

## Async & Concurrency

```csharp
agent AsyncBot {
    // Async method
    async public string FetchData(string url, NetworkAccess net) {
        var response = fetch(url, "GET")?;
        return response;
    }

    // Await in caller
    async public void Run() {
        unsafe {
            var net = NetworkAccess {};
            var data = await self.FetchData("https://api.example.com", net);
            print data;
        }
    }
}
```

---

## Agent Messaging

Agents communicate via the actor model:

```csharp
agent Worker {
    public void on_message(string method, string[] args) {
        match method {
            "process" => {
                log_info($"Processing: {args[0]}");
            }
            _ => {}
        }
    }
}

agent Manager {
    public void Run() {
        // Spawn a worker
        var worker = spawn Worker();

        // Fire-and-forget message
        worker.send("process", "task-1");

        // Request-reply (blocks until response)
        var result = worker.request("status");
    }
}
```

---

## Modules & Imports

```csharp
// Import entire module
import math;

// Import specific items
import math.{sqrt, abs};

// Import single item
import utils.helper;

// External crate (from crates.io) — 'import crate' + name = "version"
import crate serde_json;                              // simple, auto-added to Cargo.toml
import crate serde = "1.0" features ["derive"];       // versioned with features
import crate reqwest = "0.11" features ["json"];

// Qualified Rust path imports
import serde_json::Value;
import axum::{Router, Json};
import tokio::*;
```

---

## String Interpolation

```csharp
var name = "World";
var count = 42;

print $"Hello, {name}!";
print $"You have {count} items";
print $"Result: {add(3, 4)}";
print $"Status: {items.len()} items remaining";
```

### Multiline Strings

```csharp
var query = """
SELECT * FROM users
WHERE active = true
ORDER BY name
""";

var prompt = """
You are a helpful assistant.
Respond in JSON format.
""";
```

---

## Pipe Operator

```csharp
var result = data
    |> parse
    |> validate
    |> transform
    |> send;

// Equivalent to: send(transform(validate(parse(data))))
```

---

## Retry / Fallback

```csharp
// Basic retry
var response = retry(3) {
    fetch(url, "GET")?
} fallback {
    "cached response"
};

// With named options (backoff delay in ms, jitter, etc.)
var response = retry(5, backoff: 1000) {
    fetch(url, "GET")?
} fallback {
    "cached response"
};
```

---

## Standard Library

### File I/O (requires FileAccess)

```csharp
var content = fs_read("file.txt")?;           // Result<string, string>
fs_write("out.txt", "data")?;                 // Result<void, string>
fs_append("log.txt", "new line\n")?;          // Result<void, string>
var lines = fs_read_lines("data.csv")?;       // Result<string[], string>
var files = fs_read_dir("./src")?;            // Result<string[], string>
create_dir("./output")?;                      // Result<void, string>
delete_file("temp.txt")?;                     // Result<void, string>
var exists = path_exists("config.toml");       // bool
var joined = path_join("dir", "file.txt");     // string
var parent = path_parent("/a/b/c.txt");        // string
var ext = path_extension("file.tar.gz");       // string
var stem = path_stem("report.pdf");            // string
```

### HTTP (requires NetworkAccess)

```csharp
var body = fetch(url, "GET")?;                           // string
var resp = http_request(url, "POST", headers, body)?;    // JSON with status/body/headers
```

### JSON

```csharp
var obj = json_parse(json_string);         // JsonValue
var name = json_get(obj, "name");          // string
var age = json_get_int(obj, "age");        // int
var active = json_get_bool(obj, "active"); // bool
var items = json_get_array(obj, "items");  // string[]
var out = json_stringify(obj);             // string
```

### Shell (requires SystemAccess)

```csharp
var output = exec("ls -la")?;                // Result<string, string>
var code = exec_status("make build")?;       // Result<int, string>
```

### Date/Time

```csharp
var now = time_millis();                              // int (epoch ms)
var formatted = time_format(now, "%Y-%m-%d %H:%M");  // string
var parsed = time_parse("2024-01-15", "%Y-%m-%d")?;  // Result<int, string>
var later = time_add(now, 60000);                     // int (+ 1 minute)
var delta = time_diff(later, now);                    // int (ms difference)
var ts = timestamp();                                 // string (RFC 3339)
sleep(1000);                                          // sleep 1 second
```

### Regex

```csharp
var matches = regex_match("\\d+", input)?;          // Result<bool, string>
var found = regex_find_all("\\w+", text)?;          // Result<string[], string>
var replaced = regex_replace("\\s+", text, " ")?;   // Result<string, string>
```

### Math

```csharp
var a = abs(-5);          // 5
var s = sqrt(16.0);       // 4.0
var f = floor(3.7);       // 3.0
var c = ceil(3.2);        // 4.0
var r = round(3.5);       // 4.0
var lo = min(3, 7);       // 3
var hi = max(3, 7);       // 7
```

### String Methods

```csharp
var s = "Hello, World!";
s.len();                      // 13
s.contains("World");          // true
s.starts_with("Hello");       // true
s.ends_with("!");             // true
s.to_upper();                 // "HELLO, WORLD!"
s.to_lower();                 // "hello, world!"
s.trim();                     // removes whitespace
s.substring(0, 5);            // "Hello"
s.index_of("World");          // 7
s.split(",");                 // ["Hello", " World!"]
s.replace("World", "Varg");   // "Hello, Varg!"
s.char_at(0);                 // "H"
```

### Logging

```csharp
log_debug("detailed info");   // stdout: [DEBUG] detailed info
log_info("status update");    // stdout: [INFO] status update
log_warn("potential issue");  // stderr: [WARN] potential issue
log_error("something broke"); // stderr: [ERROR] something broke
```

### Environment

```csharp
var key = env("API_KEY");     // reads environment variable
```

### Testing

```csharp
assert(x > 0, "x must be positive");                // message required
assert_eq(result, expected, "values should match");  // message required
assert_ne(a, b, "must differ");                      // message required
assert_true(flag);                                   // message optional
assert_false(flag);                                  // message optional
assert_contains(text, "substring");                  // message optional
assert_throws(() => risky_call());                   // message optional

// With optional message:
assert_true(x > 0, "x must be positive");
assert_contains(output, "success", "output missing success");
```

### Human-in-the-Loop (HITL)

```csharp
var approved = await_approval("Deploy to production?");  // bool — blocks until user responds
var name = await_input("Enter your name: ");              // string
var choice = await_choice("Pick one", ["Yes", "No", "Later"]); // int (index)
```

### Rate Limiting

```csharp
var rl = ratelimiter_new(10, 60000);          // 10 calls per 60s window
var ok = ratelimiter_acquire(rl, "user_123"); // bool — consume 1 token for key
var check = ratelimiter_try_acquire(rl, "user_123"); // bool — non-blocking check
rate_limit_reset(rl, "user_123");             // reset key's bucket
```

### LLM Budget / Cost Tracking

```csharp
var b = budget_new(50000, 500);              // 50k tokens, $5.00 (cents)
var ok = budget_track(b, prompt, response);  // bool — returns false if exceeded
var chk = budget_check(b);                   // bool — false if already exhausted
var tok = budget_remaining_tokens(b);        // int
var cents = budget_remaining_usd_cents(b);   // int
var rpt = budget_report(b);                  // "Tokens: X/Y (Z%) | USD: ..."
var est = estimate_tokens("hello world");    // int — heuristic: chars/4
```

### Agent Checkpoint / Resume

```csharp
var cp = checkpoint_open("state.db", "agent_v1"); // CheckpointHandle
checkpoint_save(cp, json_stringify(state));        // bool
var json = checkpoint_load(cp);                    // string (empty if none)
var exists = checkpoint_exists(cp);               // bool
var age = checkpoint_age(cp);                     // int (seconds since save, -1 if none)
checkpoint_clear(cp);                              // bool
```

### Typed Channels

```csharp
var ch = channel_new(100);               // ChannelHandle (capacity 100)
channel_send(ch, "message");             // bool
var msg = channel_recv(ch);              // string (blocks until message)
var opt = channel_try_recv(ch);          // string (empty if nothing waiting)
var timed = channel_recv_timeout(ch, 5000); // string (empty on timeout)
var n = channel_len(ch);                 // int
channel_close(ch);                       // void
var closed = channel_is_closed(ch);      // bool
```

### Property-Based Testing

```csharp
var i = prop_gen_int(-100, 100);         // int (random in range)
var f = prop_gen_float(0.0, 1.0);        // float
var b = prop_gen_bool();                 // bool
var s = prop_gen_string(5);              // string (random, max 5 chars)
var xs = prop_gen_int_list(0, 100, 10);  // int[] (max 10 elements)
var ss = prop_gen_string_list(3, 5);     // string[] (max 5 strings, max 3 chars each)
var pass = prop_check(100, () => prop_gen_int(0, 10) >= 0); // bool
prop_assert(x >= 0, "must be non-negative");
```

### Multimodal (Image / Audio / Vision)

```csharp
var img = image_load("photo.png", cap);        // ImageHandle (requires FileAccess)
var b64 = image_to_base64(img);                // string
var fmt = image_format(img);                   // "png" | "jpeg" | ...
var sz = image_size_bytes(img);                // int

var aud = audio_load("voice.mp3", cap);        // AudioHandle
var ab64 = audio_to_base64(aud);               // string

// Vision call — sends image to multimodal LLM
var desc = llm_vision("What is in this image?", b64, "png", llm_cap); // string
```

### Workflow DAG

```csharp
var wf = workflow_new("pipeline");
workflow_add_step(wf, "fetch", []);           // no dependencies
workflow_add_step(wf, "parse", ["fetch"]);    // depends on fetch
workflow_add_step(wf, "store", ["parse"]);    // depends on parse

var ready = workflow_ready_steps(wf);         // string[] — steps with all deps done
workflow_set_output(wf, "fetch", data);       // mark step done with output
workflow_set_failed(wf, "parse", "err msg");  // mark step failed

var done = workflow_is_complete(wf);          // bool
var out = workflow_get_output(wf, "store");   // string
var status = workflow_status(wf, "fetch");    // "Pending" | "Done" | "Failed"
var n = workflow_step_count(wf);              // int
```

### Package Registry

```csharp
var reg = registry_open("packages.json");    // RegistryHandle
registry_install(reg, "varg-http", "1.2.0"); // bool
registry_uninstall(reg, "varg-http");        // bool
var installed = registry_is_installed(reg, "varg-http"); // bool
var ver = registry_version(reg, "varg-http");            // string
var all = registry_list(reg);                            // string[]
var found = registry_search(reg, "http");                // string[]
```

### Extended LLM

```csharp
// Structured output (JSON schema enforcement)
var schema = "{\"type\": \"object\", \"properties\": {\"name\": {\"type\": \"string\"}}}";
var json_out = llm_structured("gpt-4o", messages, schema, llm_cap); // string (JSON)

// Streaming (SSE chunks)
var stream = llm_stream("gpt-4o", messages, llm_cap); // SseHandle
var chunk = sse_read(stream);                          // string chunk

// Batch embeddings
var texts = ["hello", "world", "varg"];
var embeddings = llm_embed_batch(texts, llm_cap);      // float[][] (one vec per text)
```

---

## Annotations

### Test Framework

```csharp
@[Test]
public void TestAdd() {
    assert_eq(add(2, 3), 5, "2+3 should be 5");
}

@[BeforeEach]
public void Setup() { /* runs before every @[Test] */ }

@[AfterEach]
public void Teardown() { /* runs after every @[Test] */ }
```

### CLI + MCP Integration

```csharp
@[CliCommand("greet", "Greet a user")]
public void Greet(string name) {
    print $"Hello, {name}!";
}

@[McpTool("search", "Search the database")]
public string Search(string query) {
    return results; // auto-generates MCP JSON schema
}
```

### Rate Limiting

Annotation parameters must be **string literals** (not named args):

```csharp
// @[RateLimit("max_calls", "window_ms")]
@[RateLimit("10", "60000")]
public string CallApi(string prompt, LlmAccess llm) {
    // Enforced: max 10 calls per 60 000 ms (1 minute), per key
    return llm_chat("gpt-4o", [{"role": "user", "content": prompt}], llm);
}
```

### LLM Budget Guards

```csharp
// @[Budget("max_tokens", "max_usd_cents")]
@[Budget("50000", "500")]
public string RunAgent(string task, LlmAccess llm) {
    // Hard budget: 50 000 tokens or $5.00 — whichever hits first stops the agent
    return llm_chat("gpt-4o", [{"role": "user", "content": task}], llm);
}
```

### Agent Checkpoint / Resume

```csharp
@[Checkpointed("agent_state.db")]
public void Process(string input) {
    // State is auto-persisted on each call; resumes from last saved state
    checkpoint_save(self.state_handle, json_stringify(self.state));
}
```

### Property-Based Testing

```csharp
// @[Property("runs")]
@[Property("100")]
public void TestSortIsIdempotent() {
    var xs = prop_gen_int_list(0, 1000, 10);
    var sorted = xs.sort();
    prop_assert(sorted.len() == xs.len(), "sort must not change length");
}
```

---

## Prompt Templates

```csharp
prompt Summarize(string text, int max_words) {
    Summarize the following text in at most {max_words} words:

    {text}

    Be concise and capture the key points.
}
```

---

## Scientific Computing

### Tensor Builtins

```csharp
// Create
var t = tensor_zeros([3, 4]);              // 3×4 all-zeros
var o = tensor_ones([2, 2]);              // 2×2 all-ones
var e = tensor_eye(4);                    // 4×4 identity
var f = tensor_from_list([1.0, 2.0, 3.0, 4.0], [2, 2]); // from flat list

// Inspect
var sh = tensor_shape(t);                 // int[] — e.g. [3, 4]
var ls = tensor_to_list(t);              // float[]

// Transform
var r  = tensor_reshape(t, [12]);        // new shape
var sl = tensor_slice(t, 0, 2);          // rows 0..2

// Arithmetic
var c  = tensor_add(a, b);               // element-wise add
var s  = tensor_sub(a, b);               // element-wise sub
var ms = tensor_mul_scalar(t, 2.5);      // scalar multiply

// Matrix ops (rank-2 only)
var mm = tensor_matmul(a, b);            // matrix multiply
var d  = tensor_dot(a, b);               // dot product

// Reductions
var sum  = tensor_sum(t);                // float
var mean = tensor_mean(t);              // float
var mx   = tensor_max(t);               // float
var mn   = tensor_min(t);               // float
```

### DataFrame Builtins

```csharp
// I/O (requires FileAccess)
var df = df_read_csv("data.csv", file_cap);
var pq = df_read_parquet("data.parquet", file_cap);
df_write_csv(df, "out.csv", file_cap);
df_write_parquet(df, "out.parquet", file_cap);

// Transformation
var slim   = df_select(df, ["col1", "col2"]);   // column projection
var adults = df_filter(df, "age > 18");          // filter DSL: "col op value"
var sorted = df_sort(df, "score", true);         // ascending=true

// Grouping & aggregation
var agg = df_agg(df, ["group_col"], "mean");    // sum|mean|count|min|max

// Utilities
var top    = df_head(df, 5);                    // first N rows
var shape  = df_shape(df);                      // (rows, cols)
var cols   = df_columns(df);                    // string[]
var ext    = df_with_column(df, "rank", [1.0, 2.0]); // add column from float[]
```

Filter DSL operators: `==`, `!=`, `>`, `>=`, `<`, `<=`. Strings as values are auto-quoted: `"name == Alice"`.

### Generic LLM Structured Output

```csharp
struct WeatherReport {
    string city;
    float  temperature;
    string condition;
}

// llm_structured<T>(provider, model, prompt, llm_cap) → T
var report = llm_structured<WeatherReport>("", "", $"Weather for Berlin as JSON", llm_cap);
print $"{report.city}: {report.temperature}°C";
```

Provider and model can be empty strings to use environment defaults (`VARG_LLM_PROVIDER`, `VARG_LLM_MODEL`). The struct fields must match the JSON keys the LLM returns.
