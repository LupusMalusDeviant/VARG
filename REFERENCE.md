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

### How the entry agent is constructed

The program's entry agent is the first one with a parameterless `Run` or `Main`. The generated
runtime constructs it, so:

- a parameterless constructor runs before `Run()`,
- fields without a constructor start zero-valued (initialise them in `on_start`),
- a constructor **taking parameters** is rejected — nobody is there to pass them.

Dependency injection therefore happens one level down, which is where it belongs anyway:

```csharp
agent Service {
    ILog logger;
    public Service(ILog l) { self.logger = l; }
}

agent Main {
    public void Run() {
        var svc = Service(ConsoleLog());   // wire it here
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

### A name in scope wins over a field

Inside an agent method a bare name refers to the nearest binding: a parameter, then a local, and
only then a field of the agent. Write `self.<name>` when you mean the field and something nearer
carries the same name.

```csharp
agent Greeter {
    string name;

    public string greet(string name) {
        return "hello " + name;        // the parameter
    }

    public string whoami() {
        return self.name;              // the field
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

### Dependency injection

A field typed by a contract holds any agent implementing it, and a constructor named after the
agent takes it. The same service then runs against the real implementation or a stand-in without
changing a line of it.

```csharp
contract IStore {
    string load(string id);
}

agent MemoryStore implements IStore {
    public string load(string id) { return "in-memory " + id; }
}

agent Service {
    IStore store;

    public Service(IStore store, string label) {
        print "wiring " + label;          // work that does not touch `self` is fine
        self.store = store;               // the parameter may be named after the field
    }

    public string recall(string id) {
        return self.store.load(id);
    }
}

agent Main {
    public void Run() {
        var svc = Service(MemoryStore {}, "svc");
        print svc.recall("t-1");          // in-memory t-1
    }
}
```

Two rules follow from how such a constructor is built. It runs before the object exists, so it
may assign `self.<field> = ...` and do work that does not touch `self`, but it cannot read
`self.<field>` or call one of its own methods — the compiler says so by name. And injecting a
dependency hands it over: the variable passed moves into the new agent and is not usable
afterwards.

A method reached through a contract has the signature the contract declares, which has no error
channel. If the implementation uses `?` and the failure is not handled by an `on_error` hook, it
surfaces as a runtime failure — catchable by `try`, and otherwise reported with a non-zero exit.

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

Fallible operations return `Result<T, String>`.

Declare the **success type** as the return type and use `?` inside — the compiler
auto-wraps the function into `Result<T, String>` when it contains `?`. (Writing an explicit
`-> Result<...>` return type and `return value;` is *not* auto-wrapped; use the success-type
form below.)

```csharp
// Using ? operator (auto-propagates errors). Return type is the SUCCESS type `string`;
// the presence of `?` makes the function fallible automatically.
fn read_config(string path, FileAccess cap) -> string {
    var content = fs_read(path)?;  // Propagates error if fs_read fails
    return content;
}
// Caller handles failure with `or` / `?`:
//   var cfg = read_config(path, cap) or "default";

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

Asking about a `Result` or an optional without unwrapping it:

```csharp
var r = parse_int("42");
if (r.is_ok()) { print "parsed"; }
if (r.is_err()) { print "not a number"; }
var n = r.unwrap();                 // panics on an error — prefer `or` or `?`

var o = json_get("{}", "missing");
if (o.is_none()) { print "absent"; }
if (o.is_some()) { print "present"; }
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

Reducing a collection to one value, and the shape-changing chains:

```csharp
var xs = [3, 1, 2, 1];
var total = xs.fold(0, (acc, x) => acc + x);   // 7
var same = xs.reduce(0, (acc, x) => acc + x);  // 7 — reduce is the same operation
var uniq = xs.unique();                        // [3, 1, 2]
var dd = xs.dedup();                           // removes *adjacent* duplicates only
var dist = xs.distinct();                      // same as unique
var nested = [[1, 2], [3]];
var flat = nested.flatten();                   // [1, 2, 3]
var pairs = xs.enumerate();                    // [(0, 3), (1, 1), ...]
var joined = ["a", "b"].join(", ");            // "a, b"
var words = ["a b", "c"].flat_map((s) => s.split(" "));  // ["a", "b", "c"]
```

`sort` reorders in place and returns nothing, so it is a statement rather than a link in a chain:

```csharp
var names = ["b", "a"];
names.sort();
print names.join(",");        // "a,b"
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

### What OCAP does and does not do

OCAP is a **compile-time gate, not a runtime sandbox.** Knowing exactly where the line runs
matters more than the headline, so:

**What it guarantees.** A privileged operation will not compile unless a matching token is in
scope — passed as a parameter, or minted inside an `unsafe {}` block. Tokens cannot be
constructed anywhere else. So every path that touches the file system, the network, a database,
an LLM or the shell is visible in a signature, and you can audit a program by reading its types.

**What it does not guarantee.** A token authorises the *call*, never the *arguments*. Nothing
inspects what you pass:

```csharp
unsafe {
    var sys = SystemAccess {};
    exec(user_input);        // the whole string goes to `cmd /C` or `sh -c`
}
```

`exec` hands its argument to a shell, so shell metacharacters in it are interpreted —
`exec("echo a && echo b")` runs both commands. With untrusted input that is a command
injection, and the capability system will not say a word about it. The same applies to a path
given to `fs_read` (no confinement to a directory) and to a URL given to `fetch` (no host
allow-list).

**Use `proc_spawn_args` when the input is not yours.** It starts the program directly with
separate arguments and never involves a shell, so metacharacters stay data:

```csharp
unsafe {
    var sys = SystemAccess {};
    var p = proc_spawn_args("git", ["log", "--oneline", user_input])?;   // no shell
}

// rather than
exec("git log --oneline " + user_input);                                 // shell, injectable
```

In short: OCAP tells you **which** program parts may reach the outside world, and forces that
into the type system. Validating **what** they send there is still your job.


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

// External crate (from crates.io). The version is optional; without one the newest
// compatible release is used. Importing a crate the runtime already bundles
// (serde, serde_json, and tokio/chrono/rand when in use) is accepted and ignored.
import crate serde_json;                              // simple, auto-added to Cargo.toml
import crate serde = "1.0" features ["derive"];       // versioned with features
import crate reqwest = "0.11" features ["json"];

// Qualified Rust path imports
import serde_json::Value;
import axum::{Router, Json};
import tokio::*;
```

### A program across several directories

A dotted import names a path: `import store.repo;` reads `store/repo.varg`, and
`import util.text.casing;` reads `util/text/casing.varg`. A directory can also carry a
`mod.varg`, which is what `import store;` reads.

A module resolves its own imports first against its own directory, then against the directory of
the entry file — the one passed to `vargc`. The second is what lets packages reach each other, so
a layout like this works in both directions:

```text
main.varg              import domain.task;  import store.repo;
domain/task.varg
store/repo.varg        import domain.task;   <- found via the entry directory
```

Without that second step every directory would be an island: a module could only import its own
neighbours, which is the opposite of what laying a system out in directories is for.

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

#### Literal braces

Double a brace to emit it literally, as in C#. The backslash form works too:

```csharp
var used = 36;
var who  = "worker";

print $"{{literal}} and {used}";        // {literal} and 36
print $"\{literal\}";                   // {literal}

// Which is how you emit JSON from an interpolated string:
print $"{{\"used\": {used}, \"name\": \"{who}\"}}";
// {"used": 36, "name": "worker"}
```

#### Quotes inside an interpolation

An expression inside `{...}` is already inside the braces, so a string argument there is written
with plain quotes — escaping them puts backslashes into the expression and it stops parsing:

```csharp
var s = "a-b";
print $"{s.replace("-", "+")}";        // a+b
print $"{s.replace(\"-\", ...)}";   // error: quotes here are not escaped
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
    |> trim
    |> to_upper
    |> reverse;

// Equivalent to: reverse(to_upper(trim(data)))
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
// Nullable: a root has no parent and a bare name has no extension, so these answer `null`
// rather than `""` — which used to be their answer for a failure as well.
var parent = path_parent("/a/b/c.txt") or ".";   // string? -> string
var ext = path_extension("file.tar.gz") or "";   // string? -> string
var stem = path_stem("report.pdf") or "";        // string? -> string
var abs = path_resolve("../x")?;               // Result<string, string>
```

Binary files, file metadata, and moving files around:

```csharp
var bytes = fs_read_bytes("logo.png")?;        // Result<int[], string>
fs_write_bytes("copy.png", bytes)?;            // Result<void, string>
fs_append_bytes("log.bin", bytes)?;            // Result<void, string>
var n = fs_size("logo.png")?;                  // Result<int, string> — bytes
fs_copy("a.txt", "b.txt")?;                    // Result<void, string>
fs_rename("b.txt", "c.txt")?;                  // Result<void, string>
var isf = is_file("c.txt");                    // bool
var isd = is_dir("./src");                     // bool
```

The per-user directories the operating system defines. Each returns a path without checking that
it exists:

```csharp
var home = home_dir();                         // string
var cfg = config_dir();                        // string
var cache = cache_dir();                       // string
var data = data_dir();                         // string
```

### HTTP (requires NetworkAccess)

```csharp
var body = fetch(url, "GET")?;                           // string
var resp = http_request(url, "POST", headers, body)?;    // JSON with status/body/headers
```

### Web Server (requires NetworkAccess)

A server is a handle you register routes on, then hand to `http_listen`. The listen call runs the
server, so the method containing it is `async`.

```csharp
var srv = http_serve();

// A page. Two arguments mean `text/html; charset=utf-8`.
http_route(srv, "GET", "/", (req) => {
    return http_response(200, "<h1>hello</h1>");
});

// A third argument names any other type.
http_route(srv, "GET", "/style.css", (req) => {
    return http_response(200, "h1 { color: teal }", "text/css");
});

// JSON, with the content type set for you.
http_route(srv, "GET", "/api/status", (req) => {
    return http_response_json(200, "{\"ok\": true}");
});

http_listen(srv, "127.0.0.1:8080");
```

The request carries the method, path, headers, body and query parameters. The body is a raw JSON
string, which the JSON accessors read directly; the query parameters are a map:

```csharp
var srv = http_serve();
http_route(srv, "POST", "/api/users", (req) => {
    var limit = req.query_params.get("limit", "10");
    var name = json_get(req.body, "name") or "";
    if (name == "") {
        return http_response_json(400, "{\"error\": \"name is required\"}");
    }
    return http_response_json(201, $"{{\"created\": \"{name}\", \"limit\": \"{limit}\"}}");
});
```

**Server-sent events.** `http_sse_route` sends a fixed list of events and closes; `sse_open` and
`sse_push` hold a stream open and push to it as things happen.

```csharp
var srv = http_serve();

// Batch: the handler returns the events to send.
http_sse_route(srv, "/events", (req) => {
    return [sse_event("", "first"), sse_event("progress", "50%")];
});

// Streaming: keep the sender and push whenever there is something to say.
var sender = sse_open(srv, "/live");
sse_push(sender, "started");        // bool — false once every client has gone
sse_shutdown(sender);               // close the stream
```

**WebSocket.** A route handler takes the message that arrived and returns the reply:

```csharp
var srv = http_serve();
ws_route(srv, "/ws", (msg) => {
    return "echo: " + msg;
});
```

> Route handlers become `Fn + Send + Sync` closures, so they cannot reach `self`. Compute what
> the handler needs before registering it and let the closure capture the result. There is no
> static file serving: return file contents from a route, with the type as the third argument.

### SSE Client (requires NetworkAccess)

The other side of the same protocol — reading an event stream someone else serves:

```csharp
var conn = sse_client_connect("https://example.com/events", "{}")?;
var evt = sse_client_next(conn)?;      // Result<string, string>, blocks for the next event
sse_client_close(conn)?;

// POST and then read the stream the response opens (how several LLM APIs stream).
var stream = sse_client_post("https://api.example.com/v1/stream", "{}", "{}")?;
```

### JSON

```csharp
var obj = json_parse(json_string)?;                 // Result<JsonValue, string>

// The accessors are Nullable: `null` means "nothing there" and nothing else.
var name   = json_get(obj, "name") or "";           // string?    -> string
var age    = json_get_int(obj, "age") or 0;         // int?       -> int
var active = json_get_bool(obj, "active") or false; // bool?      -> bool
var items  = json_get_array(obj, "items") or [];    // string[]?  -> string[]
var out    = json_stringify(obj);                   // string
```

A path either has a value or it has none, and those are the only two answers:

| situation | `json_get` answers |
|-----------|--------------------|
| `{"a": "x"}` | `"x"` |
| `{"a": 42}` | `"42"` — numbers render as text |
| `{"a": true}` | `"true"` |
| `{"a": {"b": 1}}` | `{"b":1}` — nested values render as JSON text |
| `{"a": ""}` | `""` — a present empty string is a value |
| key absent | `null` |
| `{"a": null}` | `null` — an explicit JSON null is absence |
| unparseable input | `null` |

The typed accessors are strict: `json_get_int` on `"42"` answers `null`, because a string is not
an integer — read it with `json_get` and `parse_int` when a document carries numbers as text.
`json_get_array` renders non-string elements instead of dropping them, so `[1, 2]` yields
`["1", "2"]`.

Resolve an optional with `or`, or ask about it directly:

```csharp
if (json_get(obj, "name") == null) { print "no name given"; }
```

Reading the shape of a document, and writing to it:

```csharp
var doc = "{\"a\": 1, \"b\": 2}";
var has = json_has(doc, "a");                 // bool
var keys = json_keys(doc);                    // string[] — ["a", "b"]
var vals = json_values(doc);                  // string[] — each value as JSON text
var pretty = json_stringify_pretty(doc);      // string, indented

// Both are fallible: a document that will not parse is not an empty one.
var withC = json_set(doc, "c", "3")?;         // Result<string, string>
var merged = json_merge(doc, "{\"b\": 9}")?;   // Result<string, string>, right side wins
```

Printing an optional without resolving it shows the value, or `null` when there is none.
Arithmetic on one, and comparing one against a real value, are rejected at compile time —
supply a fallback with `or` first.

`json_parse` is fallible for the same reason: a document that will not parse is not an empty
document. It used to lower to `unwrap_or(Value::Null)`, so malformed input silently became a
document whose keys were all merely absent. Handle it by propagating, or by asking:

```csharp
string name_of(string doc) {          // declare the success type; `?` wraps it in a Result
    var j = json_parse(doc)?;
    return json_get(j, "name") or "<none>";
}

if (json_parse(text).is_err()) { print "that was not JSON"; }
```

Most code needs neither: the accessors read a raw JSON string directly, so `json_get(body,
"/name")` works without a parse hop. Parse when you want the document checked once, or read
from it many times.

> **Changed in this release.** These four used to return a plain value with a default baked in,
> so `""`/`0`/`false` meant an absent key, a value of the wrong kind, a JSON null, a genuinely
> empty value and an unparseable document all at once — five situations, one answer. Code that
> relied on the old default needs an explicit `or`. `json_parse` returns a `Result` instead of
> quietly yielding an empty document; propagate it with `?`, or drop the parse hop entirely.

### Shell (requires SystemAccess)

```csharp
var output = exec("ls -la")?;                // Result<string, string>
var code = exec_status("make build")?;       // Result<int, string>
```

### Child Processes (requires SystemAccess)

`exec` runs a command and waits. To keep talking to one while it runs, spawn it:

```csharp
var proc = proc_spawn("python -i")?;      // Result<handle, string>
proc_write_stdin(proc, "print(1+1)\n")?;
var line = proc_read_line(proc)?;         // Result<string, string>
var alive = proc_is_alive(proc);          // bool
var pid = proc_pid(proc);                 // int
proc_close_stdin(proc)?;                  // let it see end of input
var code = proc_wait(proc)?;              // Result<int, string>
proc_kill(proc)?;                         // if it will not stop on its own
```

### Terminal Input

Reading a whole stream, or a single line, from standard input:

```csharp
var all = stdin_read()?;                  // Result<string, string>, to end of input
var one = stdin_read_line()?;             // Result<string, string>
```

An editing line reader with history, for an interactive agent (requires FileAccess to persist
the history):

```csharp
var rl = readline_new()?;
readline_load_history(rl, ".history")?;
var input = readline_read(rl, "> ")?;     // Result<string, string> — an error is EOF or Ctrl-C
readline_add_history(rl, input)?;
readline_save_history(rl, ".history")?;
```

### Terminal Colours

```csharp
print ansi_color("red") + "failed" + ansi_reset();
print ansi_bold() + "important" + ansi_reset();
```

### Date/Time

```csharp
var now = time_millis();                              // int (epoch ms)
// Fallible: an unknown specifier used to panic out of chrono's formatter and take the
// program down, so the pattern is validated first.
var formatted = time_format(now, "%Y-%m-%d %H:%M")?;  // Result<string, string>
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

### Parsing

`parse_int` / `parse_float` are fallible and return a `Result` — a malformed input is an
error, not a silent `0`. Handle it with `or`, or propagate it with `?`:

```csharp
var n = parse_int(input) or 0;      // explicit fallback
var f = parse_float(raw) or 0.0;
var m = parse_int(input)?;          // propagate to the caller
```

### Math

```csharp
var a = abs(-5);          // 5   (also correct over expressions: abs(3 - 10) → 7)
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
s.char_at(0) or "";           // string? — an index past the end has no character
s.trim_start();               // leading whitespace only
s.trim_end();                 // trailing whitespace only
s.ltrim();                    // same as trim_start
s.rtrim();                    // same as trim_end
s.count_occurrences("l");     // 3
s.pad_left(20);               // width 20, padded with spaces on the left
s.pad_right(20);
s.repeat(2);                  // "Hello, World!Hello, World!"
s.chars();                    // ["H", "e", ...]
s.reverse();                  // "!dlroW ,olleH"
```

`split_once` answers `null` when the separator is absent, which is what tells that apart from a
successful split into two empty halves:

```csharp
var parts = "key=value".split_once("=") or ("", "");   // ("key", "value")
var none = "novalue".split_once("=") or ("", "");      // ("", "") — no separator
```

These are methods on the value, so `text.to_upper()` and not `to_upper(text)`. Three have a
free-function form as well, for use in a pipeline:

```csharp
var a = str_trim("  x  ");            // string
var b = str_replace("a-b", "-", "_"); // string
var c = str_split("a,b", ",");        // string[]
```

Numbers render through methods too:

```csharp
var n = 255;
n.to_string();                // "255"
n.to_hex();                   // "ff"
n.to_binary();                // "11111111"
var f = 3.14159;
f.to_fixed(2);                // "3.14"
```

### Random & Identifiers

```csharp
var n = random_int(1, 6);         // int, both ends inclusive
var f = random_float();           // float in 0.0..1.0
var id = uuid();                  // string
var c = 15.clamp(0, 10);          // 10 — a method on the value, like the other numeric ones
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

Setting a variable for child processes, and loading configuration from a chain of files where
each one overrides the last (requires FileAccess):

```csharp
set_env("VARG_MODE", "debug");
var cfg = config_load_cascade(["defaults.json", "local.json"])?;
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

One bucket per limiter, not per key — `ratelimiter_new` returns the limiter, and every call
names it. `acquire` **blocks** until a token frees up (that is throttling, and it returns
nothing); `try_acquire` is the one that reports whether it got through.

There is a second form that needs no limiter object: the bucket is named by a key, and the limit
travels with the call.

```csharp
rate_limit_acquire("openai", 60, 60000);           // blocks until it fits
var got = rate_limit_try("openai", 60, 60000);     // bool — reports instead of waiting
```

```csharp
var rl = ratelimiter_new(10, 60000);      // 10 calls per 60s window
ratelimiter_acquire(rl);                  // blocks until a token is free
var ok = ratelimiter_try_acquire(rl);     // bool — does not block
rate_limit_reset(rl);                     // full allowance again
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

Writes to disk, so it needs `FileAccess` in scope like any other file operation.

```csharp
var cp = checkpoint_open("state.db", "agent_v1")?; // Result<CheckpointHandle, string>
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
var f = prop_gen_float();                // float in 0.0..1.0 — takes no range
var b = prop_gen_bool();                 // bool
var s = prop_gen_string(5);              // string (random, max 5 chars)
var xs = prop_gen_int_list(10);          // int[] (max 10 elements) — takes no range
var ss = prop_gen_string_list(3, 5);     // string[] (max 5 strings, max 3 chars each)
var pass = prop_check(() => prop_gen_int(0, 10) >= 0, 100); // (fn, runs) -> map
// prop_assert runs the property `runs` times and panics on the first counterexample.
prop_assert("non-negative", () => prop_gen_int(0, 10) >= 0, 100);
```

### Multimodal (Image / Audio / Vision)

Loading reads a file, so `FileAccess` must be in scope — as a parameter or under `unsafe`, not
passed as an argument. Loading is fallible: a path that cannot be read is an error, not an empty
image with the format guessed from its extension.

```csharp
var img = image_load("photo.png")?;            // Result<ImageHandle, string>
var decoded = image_from_base64(b64, "png");   // ImageHandle — no file involved
var fmt = audio_format(aud);                   // string — "mp3", "wav", ...
var size = audio_size_bytes(aud);              // int
var b64 = image_to_base64(img);                // string
var fmt = image_format(img);                   // "png" | "jpeg" | ...
var sz  = image_size_bytes(img);               // int

var aud = audio_load("voice.mp3")?;            // Result<AudioHandle, string>
var ab64 = audio_to_base64(aud);               // string

// Vision call — sends the image to a multimodal LLM
var desc = llm_vision("What is in this image?", b64, "png");
```

### Agent Registry

Every `spawn` registers its agent, and the generated dispatcher maintains the status underneath
your program — you do not report it. An agent is `starting` while `on_start` runs, `idle` while
it waits on its mailbox, `running` while it handles a message, `error` if a handler panicked
(the agent keeps serving the next message), and `stopped` once the mailbox closes. The entry
agent is registered too.

```csharp
var json = agents_list();                       // string — JSON array of every agent
var n = agents_count();                         // int
var busy = agents_count_by_status("running");   // int — starting|idle|running|error|stopped
```

Each record carries `id`, `name`, `status`, `started_at`, `updated_at`, `handled`,
`last_message` and `last_error`. This is what the dashboard in `dashboard/` reads for its agent
panel.

### Workflow DAG

```csharp
var wf = workflow_new("pipeline");
workflow_add_step(wf, "fetch", []);           // no dependencies
workflow_add_step(wf, "parse", ["fetch"]);    // depends on fetch
workflow_add_step(wf, "store", ["parse"]);    // depends on parse

// Give each step a body, then run the whole graph in dependency order. A handler receives the
// results of the steps it depends on, as JSON, and returns its own result.
workflow_set_handler(wf, "fetch", (inputs) => {
    return "raw-data";
});
workflow_set_handler(wf, "parse", (inputs) => {
    return "parsed(" + (json_get(inputs, "/fetch") or "") + ")";
});
var out = workflow_run(wf);                   // string — the last step's result
var steps = workflow_step_count(wf);          // int

var ready = workflow_ready_steps(wf);         // string[] — steps with all deps done
workflow_set_output(wf, "fetch", data);       // mark step done with output
workflow_set_failed(wf, "parse", "err msg");  // mark step failed

var done = workflow_is_complete(wf);          // bool
var out = workflow_get_output(wf, "store");   // string
var status = workflow_status(wf);             // summary report string for the whole workflow
var n = workflow_step_count(wf);              // int
```

### Package Registry

```csharp
var reg = registry_open("./varg_packages");   // RegistryHandle — a cache DIRECTORY,
                                             // not a file; state lands in <dir>/installed.json
registry_install(reg, "varg-http", "1.2.0"); // bool
registry_uninstall(reg, "varg-http");        // bool
// Download and verify against a SHA-256 you already know (requires NetworkAccess).
var path = registry_download(reg, "varg-http", "1.2.0", url, sha256)?;
var installed = registry_is_installed(reg, "varg-http"); // bool
var ver = registry_version(reg, "varg-http");            // string
var all = registry_list(reg);                            // string[]
var found = registry_search("http");                     // string[] — query only (1 arg)
```

### LLM calls are fallible

`llm_infer` and `llm_chat` return `Result<string, string>`. A call that cannot reach its provider
used to hand the error payload back *as the answer*, so an agent stored
`{"error": "Network error: ..."}` in its memory and carried on as though the model had replied.
Handle it, or propagate it:

```csharp
unsafe {
    var llm = LlmAccess {};
    var reply = llm_infer("Summarise this", "gpt-4o-mini") or "the model is unavailable";
    print reply;
}

// or, in a method that is allowed to fail:
fn summarise(string text, LlmAccess llm) -> string {
    return llm_infer(text, "gpt-4o-mini")?;
}
```

On failure the error carries the provider response, so the reason survives. `llm_chat` leaves the
context with the user turn and no assistant turn, so a retry does not replay a phantom reply.

### Extended LLM

```csharp
// Structured output (JSON schema enforcement)
var schema = "{\"type\": \"object\", \"properties\": {\"name\": {\"type\": \"string\"}}}";
var json_out = llm_structured(prompt, schema, 3);      // (prompt, schema_json, retries) → string (JSON)

// Streaming (SSE chunks)
var stream = llm_stream(prompt, "gpt-4o");             // (prompt, model) → SseHandle

// Token by token into a handler, as they arrive, instead of waiting for the whole answer.
llm_stream_to(prompt, "gpt-4o", (token) => {
    print token;
})?;

// A cached chat: an identical (context, prompt, model) answers from the cache.
var reply = llm_chat_cached(ctx, prompt, "gpt-4o");    // string

// Temperature and token ceiling spelled out.
var tuned = llm_chat_opts(ctx, prompt, "gpt-4o", 0.2, 512);

// Structured output naming the provider and model rather than taking the defaults.
var shaped = llm_structured_schema("openai", "gpt-4o", schema, prompt);
var chunk = sse_read(stream);                          // string chunk

// Batch embeddings
var texts = ["hello", "world", "varg"];
var embeddings = llm_embed_batch(texts);               // (texts) → float[][] (one vec per text)
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

@[McpTool("Search the database")]
public string Search(string query) {
    return results; // the JSON schema comes from the signature
}
```

An `@[McpTool]` method is reachable three ways, all generated from the same annotation:

```bash
./prog Search "some query"     # as a CLI subcommand
./prog --mcp-discover          # prints the tool schemas as JSON
./prog --mcp-serve             # speaks MCP over stdio
```

`--mcp-serve` makes the program an MCP server: it answers `initialize`, `tools/list` and
`tools/call` as JSON-RPC on stdin/stdout, dispatching to the annotated methods. Arguments arrive
by name and are parsed into the declared parameter type, so an `int` parameter receives a number.
A method returning a struct has its result serialised as JSON; a `void` method replies `ok`. An
unknown tool is answered with a JSON-RPC error, and the connection stays usable.

Input and output schemas are derived from the signature — parameter names and types become
`inputSchema`, the return type becomes `outputSchema`. Nothing is written twice.

Any MCP client can drive it, including Varg's own:

```csharp
unsafe {
    var sys = SystemAccess {};
    var conn = mcp_connect(exe_path()?, ["--mcp-serve"])?;
    var tools = mcp_list_tools(conn)?;
    var sum = mcp_call_tool(conn, "add", {"a": 17, "b": 25}) or "failed";
    mcp_disconnect(conn);
}
```

Tool arguments are a JSON object, so a map literal may mix types — `{"query": "x", "top_k": 3,
"exact": true}` is written exactly as it reads. `{}` means "no arguments". A raw JSON string and
a variable holding a string map are both still accepted.

`exe_path()` returns the running binary, which is what lets a program start itself in server
mode — the pattern `golden/progs/mcp_server_mode.varg` uses to test both halves at once.

### Rate Limiting

Annotation parameters must be **string literals** (not named args):

```csharp
// @[RateLimit("max_calls", "window_ms")]
@[RateLimit("10", "60000")]
public string CallApi(string prompt, LlmAccess llm) {
    // Enforced: at most 10 calls per 60 000 ms, per method and per thread
    return llm_chat("gpt-4o", [{"role": "user", "content": prompt}], llm);
}
```

The bucket is keyed by method and by thread, so each spawned agent gets its own allowance. That
throttles a single worker; it does not cap what a whole program sends to one API.

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
var f = tensor_from_list([1.0, 2.0, 3.0, 4.0], [2, 2])?; // values must fill the shape

// Inspect
var sh = tensor_shape(t);                 // int[] — e.g. [3, 4]
var ls = tensor_to_list(t);              // float[]

// Transform
var r  = tensor_reshape(t, [12]);        // new shape
var sl = tensor_slice(t, 0, 0, 2);          // rows 0..2

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
// I/O (requires FileAccess in scope — the token is not an argument)
var df = df_read_csv("data.csv")?;
var pq = df_read_parquet("data.parquet")?;
df_write_csv(df, "out.csv")?;
df_write_parquet(df, "out.parquet")?;

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

// llm_structured<T>(provider, model, prompt) → Result<T, Error>
// The model reply may not conform, so handle failure with `?` or `or` (never panics).
var report = llm_structured<WeatherReport>("", "", $"Weather for Berlin as JSON")?;
print $"{report.city}: {report.temperature}°C";
```

Provider and model can be empty strings to use environment defaults (`VARG_LLM_PROVIDER`, `VARG_LLM_MODEL`). The struct fields must match the JSON keys the LLM returns.
