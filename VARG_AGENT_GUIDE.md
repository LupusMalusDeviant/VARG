# Varg: AI Agent Developer Guide (Parser-Verified)

You are an AI assistant writing code in **Varg** — a compiled, statically typed language for autonomous AI agents. Varg transpiles to Rust for native performance with C#-like syntax. This guide is generated directly from the parser source (`varg-parser/src/lib.rs`) and supersedes all previous documentation. Every rule here is verified against the actual parser grammar.

---

## CRITICAL SYNTAX RULES (Read First)

Before writing any Varg code, internalize these rules — they differ from other languages:

| Rule | Correct | Wrong |
|------|---------|-------|
| `fn` parameters | `fn f(int x, string y) -> void` | ~~`fn f(x: int, y: string)`~~ |
| Contract methods | `void run(string s);` (no `fn`) | ~~`fn run(s: string) -> void;`~~ |
| Agent fields | `int counter;` (no `var`) | ~~`var counter: int;`~~ |
| Agent implements | `agent Foo : Bar { }` | ~~`agent Foo extends Bar { }`~~ |
| Semicolons | Optional in blocks (but required after field/struct decls) | — |

---

## 1. Entry Points

A Varg program can start in one of two ways:

```csharp
// Form 1: Agent with Run() method (preferred for agents)
agent App {
    public void Run() {
        print "Hello, World!";
    }
}

// Form 2: Standalone fn main()
fn main() -> void {
    print "Hello, World!";
}
```

---

## 2. Type System

### Primitive Types
```
int       — 64-bit signed integer
float     — 64-bit floating point
bool      — true / false
string    — UTF-8 string
void      — no return value
ulong     — 64-bit unsigned integer
```

### Collection Types
```
Type[]           — array (e.g. int[], string[])
List<Type>       — growable list
map<K, V>        — hash map (also: Map<K, V>)
set<T>           — hash set (also: Set<T>)
(T1, T2, ...)    — tuple type
```

### Special Types
```
Result<T, E>     — success or error
Prompt           — first-class prompt value
Context          — agent context
Tensor           — N-dimensional float array
Embedding        — embedding vector
```

### OCAP Capability Types (see §13)
```
FileAccess       NetworkAccess       DbAccess
LlmAccess        SystemAccess
```

### Nullable & Generic
```csharp
string? maybe = null;           // nullable: Type?
int[]   nums  = [1, 2, 3];     // array
List<string> items = [];        // generic list
map<string, int> scores = {};   // map
(int, string) pair = (1, "a"); // tuple

// Custom generic type
Result<int, string>
MyContainer<T>
```

### Type Aliases
```csharp
type UserId = int;
type NameMap = map<string, string>;
```

---

## 3. Variables & Constants

### `var` / `let` (inferred type)
```csharp
var x = 42;
var name = "Alice";
var flag = true;
var items = [1, 2, 3];
let y = x + 1;          // 'let' is identical to 'var'
```

### Explicit type (no `var`)
```csharp
int count = 0;
string label = "hello";
float ratio = 3.14;
bool active = false;
```

### Constants
```csharp
const int MAX = 100;
const string NAME = "Varg";
const var PI = 3.14159;     // 'const var' for inferred type
```

### Nullable
```csharp
string? result = null;
int? maybe = 42;
```

### Destructuring
```csharp
// Tuple destructuring
var (a, b) = getTuple();
var (x, y, z) = (1, 2, 3);

// Struct/map destructuring
var { name, age } = person;                // binds name, age
var { name, age: myAge } = person;         // 'age' → local 'myAge'
```

---

## 4. Operators & Expressions

### Arithmetic
```csharp
x + y    x - y    x * y    x / y    x % y
```

### Comparison
```csharp
x == y    x != y    x < y    x > y    x <= y    x >= y
```

### Logical
```csharp
x && y    x || y    !x
```

### Assignment & Compound Assignment
```csharp
x = 5
x += 5    x -= 1    x *= 2    x /= 3    x %= 4

// Also on index/property targets:
arr[0] += 1
self.counter -= 1
```

### Ternary
```csharp
var result = cond ? "yes" : "no";
var abs = x < 0 ? -x : x;
```

### Try-Propagate (`?`)
The `?` acts as try-propagate (not ternary) when it appears at end of statement:
```csharp
var data = fs_read("file.txt")?;      // propagates error up
var body = fetch(url, "GET")?;        // in a Result-returning fn
```

### `or` Fallback
```csharp
var text = fs_read("config.txt") or "defaults";
var val  = map["key"]            or "missing";
```

### Range
```csharp
0..10       // exclusive: 0,1,...,9
0..=10      // inclusive: 0,1,...,10
for i in 0..5 { print i; }
```

### Pipe Operator
```csharp
// a |> f  → f(a)
// a |> f(b, c)  → f(a, b, c)
// a |> .method(args)  → a.method(args)

var result = input |> trim |> to_lower |> encode;
var loud   = name  |> to_upper();
var clean  = text  |> replace(" ", "_") |> trim();
```

### Cast
```csharp
var n = x as float;
var s = n as string;
```

### Bitwise Or (in patterns only)
```csharp
match x {
    1 | 2 | 3 => { print "small"; }
    _         => { print "other"; }
}
```

### Cosine Similarity
```csharp
var sim = vec_a ~ vec_b;    // returns float in [-1, 1]
```

### Operator Precedence (lowest to highest)
| Level | Operators | Notes |
|-------|-----------|-------|
| 1 | `~` | cosine similarity |
| 2 | `..` `..=` | range |
| 3 | `?` | ternary / try-propagate |
| 4 | `\|\|` | logical or |
| 5 | `\|` | bitwise or |
| 6 | `&&` | logical and |
| 7 | `==` `!=` | equality |
| 8 | `<` `>` `<=` `>=` | comparison |
| 9 | `+` `-` | additive |
| 10 | `*` `/` `%` | multiplicative |
| 11 | postfix | `.` `[]` `()` `as` `or` |
| 12 | prefix unary | `-x` `!x` `await x` |

### String Interpolation
```csharp
var msg = $"Hello {name}!";
var info = $"Count: {items.len()}, avg: {sum / count}";
// Nested expressions fully supported
var report = $"Status: {x > 0 ? "positive" : "negative"}";
```

Escape sequences in interpolated strings: `\"`, `\\`, `\n`, `\t`, `\{`, `\}`.

### Multiline Strings
```csharp
var text = """
    Line one
    Line two
    Line three
""";
```
Leading/trailing newlines are automatically stripped.

### Struct Literal
```csharp
// TypeName { field: value, ... }
var p = Point { x: 10, y: 20 };
var r = Report { title: "Q1", score: 95.0 };
```
> **Note:** Struct literals are suppressed in `if`/`while`/`match`/`for` conditions. Use parentheses if needed: `if (MyType { }) { }`.

### Map Literal
```csharp
var m = {"key": "value", "n": 42};
```

### Array Literal
```csharp
var arr = [1, 2, 3, 4, 5];
var strs = ["a", "b", "c"];
var empty: int[] = [];
```

### Tuple Literal
```csharp
var pair  = (1, "hello");
var triple = (true, 42, "ok");
```

### Generic Call
```csharp
// funcName<Type>(args)
var report = llm_structured<WeatherReport>("", "", prompt, llm);
```

### Named Arguments
```csharp
// Detected when first argument is: identifier: value
retry(3, backoff: 1000);
http_route(server, method: "GET", path: "/health", handler: h);
```

### LINQ Query Expression
```csharp
var adults = from p in people
             where p.age >= 18
             orderby p.name
             select p.name;

// With descending sort
var top = from s in scores orderby s.value descending select s;
```

---

## 5. Control Flow

### If / Else
```csharp
// Parentheses around condition are optional
if x > 0 {
    print "positive";
} else if x < 0 {
    print "negative";
} else {
    print "zero";
}

// Braceless single-statement form
if x > 0 print "positive";
else print "not positive";

// If-expression (returns a value)
var sign = if x > 0 { 1 } else { -1 };
```

### While
```csharp
while count < 10 {
    count += 1;
}

// Parentheses optional
while (running) {
    process();
}

// Braceless
while !done print "waiting";
```

### For-In (range / collection)
```csharp
// Simple for-in
for item in collection {
    print item;
}

// With range
for i in 0..10 {
    print i;
}

// 'foreach' is identical
foreach item in list {
    print item;
}

// Tuple destructure (maps, pairs)
for (key, value) in myMap {
    print $"{key} = {value}";
}

foreach key, value in myMap {
    print $"{key}: {value}";
}
```

### C-Style For Loop
```csharp
for (var i = 0; i < 10; i += 1) {
    print i;
}

for (int j = 0; j < n; j += 1) {
    process(arr[j]);
}
```

### Match
```csharp
match value {
    0 => { print "zero"; }
    1 | 2 => { print "one or two"; }
    -1 => { print "minus one"; }
    "hello" => { print "greeting"; }
    true => { print "yes"; }
    Status.Active => { print "active"; }
    Ok(val) => { print val; }
    Err(e) => { print $"Error: {e}"; }
    Some(x) if x > 0 => { print "positive some"; }   // guard
    _ => { print "other"; }
}

// Match as expression
var label = match score {
    100 => { "perfect" }
    _ if score >= 90 => { "A" }
    _ if score >= 80 => { "B" }
    _ => { "C" }
};
```

Pattern forms:
- `_` — wildcard (matches anything)
- `42`, `-1` — integer literals
- `"str"` — string literals
- `true`, `false` — booleans
- `Variant` — enum variant (no payload)
- `Variant(x)`, `Variant(x, y)` — enum variant with bindings
- `Status.Active`, `Status::Active` — qualified variant
- `Ok(val)`, `Err(e)`, `Some(x)` — Result/Option variants
- `1 | 2 | 3` — or-pattern (multiple alternatives)
- `pattern if guard` — guarded pattern

### Break & Continue
```csharp
for i in 0..100 {
    if i == 50 { break; }
    if i % 2 == 0 { continue; }
    print i;
}
```

### Return
```csharp
return;              // void return
return value;        // return with value
return Ok(result);   // return Result variant
```

---

## 6. Standalone Functions (`fn`)

> **CRITICAL:** Parameters use **C# order: `Type name`** — NOT `name: Type`.

```csharp
// Basic function
fn add(int a, int b) -> int {
    return a + b;
}

// No return type (void is optional)
fn greet(string name) -> void {
    print $"Hello, {name}!";
}

// Implicit void (no -> clause)
fn log(string msg) {
    print msg;
}

// Default parameters
fn greet(string name = "World") -> void {
    print $"Hello, {name}!";
}

// Multiple params with defaults
fn connect(string host = "localhost", int port = 8080) -> void {
    print $"Connecting to {host}:{port}";
}

// Generics with trait bounds
fn process<T: Display>(T item) -> string {
    return $"Item: {item}";
}

fn merge<T: Clone + Display>(T a, T b) -> T {
    return a;
}

// Where clause (alternative to inline bounds)
fn transform<T, U>(T input) -> U
    where T: Display, U: Clone
{
    // ...
    return input as U;
}

// fn entry point
fn main() -> void {
    var result = add(3, 4);
    greet("Varg");
}
```

---

## 7. Agents

Agents are the primary abstraction — like classes but with lifecycle hooks and actor support.

### Basic Agent
```csharp
agent Counter {
    // Fields: Type name; (no var, no = here)
    int value;
    string label;

    // Constructor: name must match agent name exactly
    public Counter(string label) {
        self.label = label;
        self.value = 0;
    }

    // Public method
    public void Increment() {
        self.value += 1;
    }

    // Private method (no modifier = private)
    void reset() {
        self.value = 0;
    }

    // Async method
    public async string Fetch(string url) {
        return fetch(url, "GET") or "";
    }

    // Entry point
    public void Run() {
        self.Increment();
        print $"{self.label}: {self.value}";
    }
}
```

### Method Modifiers
Modifiers can appear in any order:
```csharp
public void Method()          // public, sync
async public void Method()    // public, async
public async void Method()    // same
private void Method()         // private, sync
```

Shorthand (Varg-min syntax):
```csharp
+m Run() { ... }     // + = public (method shorthand)
+v Run() { ... }     // +v same as +m
```

### Lifecycle Hooks
```csharp
agent Bot {
    public void on_start() {
        print "Bot started";
    }

    public void on_stop() {
        print "Bot stopped";
    }

    public void on_message(string msg, string[] args) {
        print $"Received: {msg}";
    }

    public void Run() {
        // main logic
    }
}
```

### Field Initialization
```csharp
agent App {
    int counter;           // uninitialized field
    string name;
    List<string> log;

    public App(string name) {
        self.name = name;
        self.counter = 0;
        self.log = [];
    }
}
```

### Agent Visibility
```csharp
public agent PublicAgent { }    // visible to other modules
agent PrivateAgent { }          // private by default
+a PublicAgent2 { }             // shorthand for public agent
```

### Spawn & Actor Messaging
```csharp
agent Worker {
    public void on_message(string msg, string[] args) {
        print $"Worker got: {msg}";
    }
    public void Run() { }
}

agent Supervisor {
    public void Run() {
        var w = spawn Worker();
        w.send("start", ["task1"]);
        var reply = w.request("status");
        print reply;
    }
}
```

---

## 8. Contracts & Dependency Injection

Contracts are interfaces. **CRITICAL: NO `fn` keyword inside contracts.**

```csharp
// Contract: ReturnType MethodName(Type param);
contract IDatabase {
    string query(string sql);
    void execute(string sql, string[] params);
    void close();
}

// Multiple method signatures
contract ILogger {
    void info(string msg);
    void error(string msg);
    void warn(string msg);
}

// Async contract methods
contract IFetcher {
    async string fetch(string url);
}
```

### Implementing a Contract
```csharp
// Colon syntax (preferred)
agent SqliteDb : IDatabase {
    string db_path;

    public SqliteDb(string path) {
        self.db_path = path;
    }

    public string query(string sql) {
        var db = db_open(self.db_path);
        var rows = db_query(db, sql, []);
        return json_stringify(rows);
    }

    public void execute(string sql, string[] params) {
        var db = db_open(self.db_path);
        db_execute(db, sql, params);
    }

    public void close() { }
}

// 'implements' keyword (alternative)
agent MockDb implements IDatabase {
    public string query(string sql) { return "[]"; }
    public void execute(string sql, string[] params) { }
    public void close() { }
}

// Multiple contracts
agent MultiImpl : IDatabase, ILogger {
    public string query(string sql) { return ""; }
    public void execute(string sql, string[] params) { }
    public void close() { }
    public void info(string msg) { print msg; }
    public void error(string msg) { print $"ERROR: {msg}"; }
    public void warn(string msg) { print $"WARN: {msg}"; }
}
```

### Dependency Injection
Contract-typed fields compile to `Box<dyn Trait>`:
```csharp
agent MyService {
    IDatabase db;     // DI field — must be contract type
    ILogger log;

    // Constructor injection
    public MyService(IDatabase db, ILogger log) {
        self.db = db;
        self.log = log;
    }

    public string GetData(string key) {
        self.log.info($"Querying {key}");
        return self.db.query($"SELECT * FROM data WHERE key='{key}'");
    }

    public void Run() {
        unsafe {
            // Production: inject real implementation
            var db = SqliteDb("app.db");
            var log = ConsoleLogger();
            var svc = MyService(db, log);
            print svc.GetData("config");
        }
    }
}

// Testing: inject mock
agent MyServiceTest {
    @[Test]
    public void test_get_data() {
        var db = MockDb();
        var log = MockLogger();
        var svc = MyService(db, log);
        var result = svc.GetData("key");
        assert_eq(result, "[]", "mock returns empty");
    }
}
```

---

## 9. Structs

```csharp
struct Point {
    float x;
    float y;
}

struct User {
    int id;
    string name;
    string email;
}

// Generic struct
struct Pair<T, U> {
    T first;
    U second;
}

// With public fields
struct Config {
    public string host;
    public int port;
    public bool debug;
}
```

### Using Structs
```csharp
var p = Point { x: 1.0, y: 2.0 };
var u = User { id: 1, name: "Alice", email: "a@example.com" };

print p.x;
print u.name;
u.email = "new@example.com";
```

### Struct for LLM Structured Output
```csharp
struct WeatherReport {
    string city;
    float temperature;
    string condition;
}

var report = llm_structured<WeatherReport>("", "", prompt, llm);
print $"{report.city}: {report.temperature}°C";
```

---

## 10. Enums

```csharp
// Simple enum
enum Status {
    Active,
    Inactive,
    Pending
}

// Enum with tuple data
enum Shape {
    Circle(float),           // auto-named field0
    Rectangle(float, float), // field0, field1
    Named(float radius),     // named field
    Triangle(float a, float b, float c)
}

// Result-like enum
enum ApiResult {
    Success(string),
    Error(string, int),     // message, code
    Timeout
}
```

### Using Enums
```csharp
var s = Status.Active;
var c = Shape.Circle(5.0);
var r = Shape.Rectangle(10.0, 20.0);

match s {
    Status.Active   => { print "active"; }
    Status.Inactive => { print "inactive"; }
    _               => { print "other"; }
}

match shape {
    Circle(r)     => { print $"circle r={r}"; }
    Rectangle(w, h) => { print $"rect {w}x{h}"; }
    _               => { print "other shape"; }
}
```

### Result Enum (built-in)
```csharp
fn divide(int a, int b) -> Result<int, string> {
    if b == 0 { return Err("division by zero"); }
    return Ok(a / b);
}

var r = divide(10, 2);
match r {
    Ok(val) => { print val; }
    Err(e)  => { print $"Error: {e}"; }
}
```

---

## 11. Impl Blocks

Impl blocks add methods to structs or types. Methods inside `impl` use **`fn` keyword**:

```csharp
struct Circle {
    float radius;
}

impl Circle {
    fn area() -> float {
        return 3.14159 * self.radius * self.radius;
    }

    fn scale(float factor) -> Circle {
        return Circle { radius: self.radius * factor };
    }

    pub fn describe() -> string {
        return $"Circle(r={self.radius})";
    }

    pub async fn fetch_data(string url) -> string {
        return fetch(url, "GET") or "";
    }
}

// Generic impl
impl<T> Pair<T> {
    fn first() -> T { return self.first; }
    fn second() -> T { return self.second; }
}
```

> **Note:** `impl` method parameters use `Type name` order (same as `fn`). The `self` receiver is implicit.

---

## 12. Lambdas & Closures

### Untyped Parameters (most common)
```csharp
// Single param — no type annotation needed
var double = (x) => x * 2;
var greet  = (name) => $"Hello {name}";

// Multiple params
var add = (a, b) => a + b;
var fmt = (k, v) => $"{k}={v}";

// Block body
var process = (item) => {
    var cleaned = trim(item);
    return to_upper(cleaned);
};

// No params
var hello = () => "hello";
var action = () => { print "done"; };
```

### Typed Parameters
```csharp
// (Type name, ...) =>
var add = (int a, int b) => a + b;
var fmt = (string key, int val) => $"{key}: {val}";

// Block body with typed params
var handler = (string req, int id) => {
    print $"Request {id}: {req}";
    return 200;
};
```

### Lambdas in Context
```csharp
// Collection methods
var evens  = nums.filter((x) => x % 2 == 0);
var doubled = nums.map((x) => x * 2);
var found  = nums.find((x) => x > 10);
var hasNeg = nums.any((x) => x < 0);
var allPos = nums.all((x) => x > 0);
var sorted = items.sort((a, b) => a < b);

// HTTP route handler
http_route(server, "GET", "/hello", (req) => {
    return http_response(200, "hello");
});

// Pipeline step
__varg_pipeline_add_step(pipe, "upper", (input) => input.to_upper());

// Event handler
__varg_event_on(bus, "joined", (data) => {
    print $"Welcome {data["name"]}";
    return "ok";
});
```

---

## 13. Error Handling

### `?` — Try-Propagate
Makes the enclosing function return `Result<T, string>`:
```csharp
fn readConfig(string path) -> Result<string, string> {
    var text = fs_read(path)?;       // propagates Err upward
    var data = json_parse(text)?;
    return Ok(data);
}
```

### `or` — Fallback Value
```csharp
var text  = fs_read("config.txt") or "{}";
var value = map["key"]            or "default";
var n     = parse_int(input)      or 0;
```

### `try / catch`
Catches both explicit `throw` and runtime errors (index out of bounds, division by zero, etc.):
```csharp
try {
    var arr = [1, 2, 3];
    var x   = arr[99];    // caught!
} catch e {
    print $"Caught: {e}";
}

// Optional parens around catch variable
try {
    var result = risky_call();
} catch (err) {
    print $"Error: {err}";
}
```

### `throw`
Inside a `try` block: caught by `catch`. Inside a function: becomes `return Err(...)`.
```csharp
fn validate(string s) -> string {
    if s == "" {
        throw "empty input";      // → return Err("empty input")
    }
    return s;
}

try {
    var clean = validate("");
} catch err {
    print $"Validation failed: {err}";
}
```

### `Result<T, E>` Returns
```csharp
fn safeDivide(int a, int b) -> Result<int, string> {
    if b == 0 { return Err("division by zero"); }
    return Ok(a / b);
}

match safeDivide(10, 0) {
    Ok(v)  => { print v; }
    Err(e) => { print $"Error: {e}"; }
}
```

### `map_err` / `and_then` (method chaining on Result)
```csharp
var result = fs_read("file.txt")
    .map_err((e) => $"Read failed: {e}")
    .and_then((text) => json_parse(text));
```

---

## 14. OCAP Security Model

All system interactions require a **capability token** passed as a parameter.

### The 5 Capability Types
| Token | Guards |
|-------|--------|
| `FileAccess` | `fs_read`, `fs_write`, `fs_append`, `fs_read_lines`, `fs_read_dir`, `create_dir`, `delete_file` |
| `NetworkAccess` | `fetch`, `http_request`, `fetch_stream` |
| `DbAccess` | `db_open`, `db_execute`, `db_query`, `duckdb_*` |
| `LlmAccess` | `llm_chat`, `llm_complete`, `llm_vision`, `llm_structured`, `llm_embed_batch` |
| `SystemAccess` | `exec`, `exec_status`, `mcp_connect` |

### Rules
1. Tokens can **only** be constructed inside `unsafe {}` blocks
2. Pass tokens as method parameters — never store them as fields (by convention)
3. Methods that require capabilities must declare the token in their signature

```csharp
agent FileProcessor {
    // Demand the capability as a parameter
    public string ReadFile(string path, FileAccess files) {
        return fs_read(path, files) or "not found";
    }

    public void WriteReport(string path, string data, FileAccess files) {
        fs_write(path, data, files);
    }

    public void Run() {
        // Tokens constructed only inside unsafe
        unsafe {
            var files = FileAccess {};
            var content = self.ReadFile("input.txt", files);
            self.WriteReport("output.txt", content, files);
        }
    }
}

agent WebClient {
    public string Fetch(string url, NetworkAccess net) {
        return fetch(url, "GET", net) or "";
    }

    public void Run() {
        unsafe {
            var net = NetworkAccess {};
            var html = self.Fetch("https://example.com", net);
            print html;
        }
    }
}
```

---

## 15. Imports & Modules

### Varg Module Imports
```csharp
import utils;              // imports all from utils.varg
import utils.http;         // imports http from utils module
import agents.worker;      // resolves to agents/worker.varg
import utils.*;            // explicit wildcard
import utils.{a, b, c};   // selected items
```

### External Rust Crate Imports
```csharp
// Simple crate (added to Cargo.toml automatically)
import crate serde_json;

// Versioned crate
import crate reqwest = "0.12";

// With features
import crate reqwest = "0.12" features ["json", "blocking"];

// Qualified path import
import serde_json::Value;
import axum::Router;
import axum::{Router, Json, Extension};    // braced
import tokio::*;                            // wildcard
```

Duplicate imports are deduplicated. Circular imports are detected at compile time.

---

## 16. Annotations

Annotation syntax: `@[Name]` or `@[Name("string1", "string2")]`

> **CRITICAL:** Annotation parameters must be **string literals**. No named args (`key: val`) inside annotations.

### Lifecycle & Test Annotations
```csharp
@[Test]
public void test_something() { ... }

@[BeforeEach]
public void setup() { ... }

@[AfterEach]
public void teardown() { ... }
```

### Agent Feature Annotations
```csharp
@[WithContext]               // auto-injects 'context: Context' field
agent ContextualAgent { ... }

@[McpTool("echo", "Echo input back")]
public string echo(string text) { return text; }

@[ToolResponse]
public string getResult() { ... }
```

### Rate Limiting
```csharp
// @[RateLimit("max_calls", "window_ms")]
@[RateLimit("10", "60000")]
public string CallLlm(string prompt, LlmAccess llm) {
    return llm_chat("gpt-4o", [{"role": "user", "content": prompt}], llm);
}
```

### Budget / Cost Control
```csharp
// @[Budget("max_tokens", "max_usd_cents")]
@[Budget("100000", "1000")]
public string Query(string prompt, LlmAccess llm) {
    return llm_chat("gpt-4o", [{"role": "user", "content": prompt}], llm);
}
```

### Checkpointing
```csharp
// @[Checkpointed("db_path")]
@[Checkpointed("worker.db")]
public void DoWork(string input) {
    // state auto-persisted to worker.db
}
```

### Property-Based Testing
```csharp
// @[Property("runs")]
@[Property("200")]
public void TestRoundTrip() {
    var s = prop_gen_string(50);
    var enc = base64_encode(s);
    var dec = base64_decode(enc);
    prop_assert(dec == s, "roundtrip failed");
}
```

### CLI Command
```csharp
// @[CliCommand("command-name", "description")]
@[CliCommand("greet", "Print a greeting")]
public void Greet(string name) {
    print $"Hello, {name}!";
}
```

### Target Platform
```csharp
#[target("wasm")]
agent WasmOnly { ... }

#[target("native")]
fn nativeOnly() -> void { ... }
```

### Doc Comments
```csharp
/// Fetches weather data for the given city.
/// Returns a JSON string with temperature and conditions.
agent WeatherAgent {
    /// Calls the OpenWeatherMap API.
    public async string GetForecast(string city, NetworkAccess net) {
        // ...
    }
}
```

---

## 17. Generics & Where Clauses

### Generic Functions
```csharp
// Inline bounds: <T: Trait>
fn max<T: Comparable>(T a, T b) -> T {
    return a > b ? a : b;
}

// Multiple bounds: T: Trait1 + Trait2
fn display<T: Display + Clone>(T item) -> string {
    return $"Item: {item}";
}

// Multiple type params
fn zip<T, U>(T a, U b) -> (T, U) {
    return (a, b);
}
```

### Where Clauses
```csharp
fn transform<T, U>(T input) -> U
    where T: Display, U: Clone
{
    return input as U;
}
```

### Generic Methods on Agents
```csharp
agent Container {
    public string Describe<T: Display>(T item) -> string {
        return $"Contains: {item}";
    }

    public List<U> Map<T, U>(List<T> items, (T) => U transform) -> List<U> {
        return items.map(transform);
    }
}
```

### Generic Structs
```csharp
struct Box<T> {
    T value;
}

struct Result2<T, E> {
    bool ok;
    T value;
    E error;
}
```

---

## 18. Pattern Matching (Complete Reference)

```csharp
match subject {
    // Wildcard
    _ => { }

    // Integer literals (including negative)
    0         => { }
    42        => { }
    -1        => { }

    // String literals
    "hello"   => { }
    "world"   => { }

    // Bool literals
    true      => { }
    false     => { }

    // Or-pattern (any of several values)
    1 | 2 | 3 => { }
    "a" | "b" => { }

    // Enum variants (no payload)
    Status.Active    => { }
    Status::Pending  => { }
    Active           => { }     // bare variant name also works

    // Enum variants with bindings
    Ok(val)          => { print val; }
    Err(e)           => { print e; }
    Some(x)          => { print x; }
    Circle(radius)   => { print radius; }
    Pair(a, b)       => { print $"{a}, {b}"; }

    // Guard conditions
    Ok(x) if x > 100 => { print "large ok"; }
    Err(e) if e == "timeout" => { print "timed out"; }
    n if n % 2 == 0  => { print "even"; }
    _                => { }
}
```

---

## 19. Unsafe Blocks

Required for constructing OCAP capability tokens:
```csharp
unsafe {
    var files  = FileAccess {};
    var net    = NetworkAccess {};
    var db     = DbAccess {};
    var llm    = LlmAccess {};
    var sys    = SystemAccess {};

    // Use them here
    var text   = fs_read("file.txt", files);
    var html   = fetch("https://example.com", "GET", net);
}
```

---

## 20. Prompt Templates

First-class prompt declarations with typed parameters:
```csharp
prompt Greeting(string name, string language) {
    You are a helpful assistant. Greet {name} in {language}.
    Be friendly and concise.
}

// Usage
var p = Greeting("Alice", "French");
```

---

## 21. HTTP Server (axum-based)

```csharp
agent ApiServer {
    public async void Run() {
        var server = http_serve();

        // GET handler
        http_route(server, "GET", "/health", (req) => {
            return http_response(200, "{\"status\": \"ok\"}");
        });

        // POST handler — access request body
        http_route(server, "POST", "/echo", (req) => {
            return http_response(200, req.body);
        });

        // Dynamic path
        http_route(server, "GET", "/users/:id", (req) => {
            var id = req.query_params["id"] or "unknown";
            return http_response(200, $"{{\"id\": \"{id}\"}}");
        });

        // SSE route
        http_sse_route(server, "/events", (req) => {
            sse_event("data", "hello");
        });

        // Start listening (blocks)
        http_listen(server, "0.0.0.0:8080");
    }
}
```

Request object fields: `.method`, `.path`, `.headers`, `.body`, `.query_params`
Response: `http_response(status_code, body_string)`

---

## 22. Database (SQLite, rusqlite)

```csharp
agent DbApp {
    public void Run() {
        var db = db_open(":memory:");          // in-memory
        // var db = db_open("app.db");         // file

        db_execute(db, "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)", []);
        db_execute(db, "INSERT INTO users (name) VALUES (?1)", ["Alice"]);
        db_execute(db, "INSERT INTO users (name) VALUES (?1)", ["Bob"]);

        var rows = db_query(db, "SELECT * FROM users", []);
        // rows: List<map<string, string>>
        for row in rows {
            print $"id={row["id"]} name={row["name"]}";
        }

        // Parameterized query
        var found = db_query(db, "SELECT * FROM users WHERE name = ?1", ["Alice"]);
    }
}
```

- `db_open(path)` → db handle
- `db_execute(db, sql, params)` → row count
- `db_query(db, sql, params)` → `List<map<string, string>>`
- Parameters: `?1`, `?2`, `?3` (1-indexed)

---

## 23. Collections & Standard Library

### Arrays / Lists
```csharp
var arr = [1, 2, 3, 4, 5];
arr.push(6);
var n = arr.len();
var first = arr.first();
var last  = arr.last();
var empty = arr.is_empty();
arr.sort();
arr.reverse();
arr.pop();
var slice = arr.substring(1, 3);  // for string arrays
```

### Maps
```csharp
var m = {"key": "value", "n": 42};
var v   = m["key"];
var safe = m["key"] or "default";     // or fallback
m["new"] = "val";                      // insert/update
var keys = m.keys();
var vals = m.values();
var has  = m.contains_key("key");
m.remove("key");
```

### Sets
```csharp
var s = set_of("a", "b", "c");
s.add("d");
var has  = s.contains("a");
s.remove("b");
var size = s.len();
```

### Iterator Chains
```csharp
var evens = nums
    .filter((x) => x % 2 == 0)
    .map((x) => x * 2)
    .sort();

var found = items.find((x) => x.starts_with("A"));
var any   = items.any((x) => x == "target");
var all   = items.all((x) => x.len() > 0);
```

### Strings
```csharp
s.split(",")
s.contains("sub")
s.starts_with("pre")
s.ends_with("suf")
s.replace("old", "new")
s.trim()
s.to_upper()
s.to_lower()
s.substring(0, 5)
s.index_of("sub")
s.pad_left(10, ' ')
s.pad_right(10, ' ')
s.chars()             // → List<string>
s.reverse()
s.repeat(3)
s.len()
s.is_empty()
parse_int(s)          // → int
parse_float(s)        // → float
```

### Math
```csharp
abs(x)   sqrt(x)   floor(x)   ceil(x)   round(x)
min(a, b)   max(a, b)   pow(base, exp)
```

### Date / Time
```csharp
var now  = time_millis();
var fmt  = time_format(now, "%Y-%m-%d %H:%M:%S");
var ts   = time_parse("2024-01-01", "%Y-%m-%d");
var later = time_add(now, 86400000);   // + 1 day in ms
var diff  = time_diff(ts1, ts2);       // difference in ms
```

### Logging
```csharp
log_debug("debug message");
log_info("info message");
log_warn("warning");
log_error("error occurred");
```

### Environment
```csharp
var key = env("API_KEY");
var host = env("HOST") or "localhost";
```

### File I/O (requires `FileAccess`)
```csharp
var text  = fs_read("file.txt", files);
fs_write("out.txt", content, files);
fs_append("log.txt", line, files);
var lines = fs_read_lines("file.txt", files);    // List<string>
var entries = fs_read_dir("./data", files);       // List<string>
create_dir("./output", files);
delete_file("./tmp.txt", files);
```

### HTTP Client (requires `NetworkAccess`)
```csharp
var html  = fetch("https://example.com", "GET", net);
var resp  = fetch("https://api.example.com/data", "POST", net);
// http_request for headers/status:
var r     = http_request("GET", url, {"Authorization": "Bearer token"}, "", net);
// r.status, r.headers, r.body
```

### Shell (requires `SystemAccess`)
```csharp
var output = exec("ls -la", sys);
var code   = exec_status("git pull", sys);
```

### JSON
```csharp
var parsed  = json_parse(text);
var val     = json_get(parsed, "key");
var num     = json_get_int(parsed, "count");
var flag    = json_get_bool(parsed, "active");
var arr     = json_get_array(parsed, "items");
var out     = json_stringify(data);
```

### Crypto
```csharp
var encrypted = encrypt(plaintext, key);
var decrypted = decrypt(encrypted, key);
```

### Base64
```csharp
var b64 = base64_encode(text);
var dec = base64_decode(b64);
var fileB64 = base64_encode_file("image.png", files);
var raw     = http_download_base64("https://example.com/image.png", net);
```

### PDF Generation
```csharp
var pdf = pdf_create("My Report");
pdf_add_section(pdf, "Section 1");
pdf_add_text(pdf, "Content of section one...");
pdf_save(pdf, "report.pdf", files);
var b64 = pdf_to_base64(pdf);
```

---

## 24. Retry & Fallback

```csharp
// Basic retry
var result = retry(3) {
    fetch(url, "GET", net)?
} fallback {
    "cached_value"
};

// With backoff (named args to retry)
var data = retry(5, backoff: 1000) {
    http_request("GET", url, {}, "", net).body
} fallback {
    ""
};
```

The `retry` block re-executes on exception/error. `fallback` provides a default on final failure.

---

## 25. Actor Messaging & Select

### Send / Request
```csharp
var worker = spawn Worker();
worker.send("process", ["task-1", "priority-high"]);
var status = worker.request("status");
```

### Select (multi-agent receive)
```csharp
select {
    msg from worker1 => {
        print $"Worker1 says: {msg}";
    }
    msg from worker2 => {
        print $"Worker2 says: {msg}";
    }
    timeout(5000) => {
        print "Timed out waiting";
    }
}
```

---

## 26. WebSocket Client

```csharp
agent WsClient {
    public void Run() {
        var ws = ws_connect("ws://localhost:8080/ws");
        ws_send(ws, "hello");
        var msg = ws_receive(ws);    // blocking
        print msg;
        ws_close(ws);
    }
}
```

---

## 27. MCP Protocol Client (JSON-RPC over stdio)

```csharp
agent McpApp {
    public void Run() {
        unsafe {
            var sys = SystemAccess {};
            var conn = mcp_connect("npx", ["-y", "@modelcontextprotocol/server-everything"], sys);
            var tools = mcp_list_tools(conn);
            print tools;
            var result = mcp_call_tool(conn, "echo", {"message": "hello"});
            print result;
            mcp_disconnect(conn);
        }
    }
}
```

### MCP Server Mode
```csharp
agent McpServer {
    public void Run() {
        var server = mcp_server_new("my_tools", "1.0.0");
        mcp_server_register(server, "greet", "Says hello to name", (args) => {
            return $"Hello {args}";
        });
        mcp_server_run(server);    // blocks on stdio JSON-RPC
    }
}
```

---

## 28. LLM Integration (requires `LlmAccess`)

```csharp
agent AiAgent {
    public string Chat(string prompt, LlmAccess llm) {
        var messages = [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user",   "content": prompt}
        ];
        return llm_chat("gpt-4o", messages, llm);
    }

    // Complete (single-turn)
    public string Complete(string prompt, LlmAccess llm) {
        return llm_complete("claude-3-haiku", prompt, llm);
    }

    // Vision
    public string Describe(string imagePath, FileAccess files, LlmAccess llm) {
        var img = image_load(imagePath, files);
        var b64 = image_to_base64(img);
        var fmt = image_format(img);
        return llm_vision("Describe this image.", b64, fmt, llm);
    }

    // Structured output (generic call syntax)
    public WeatherReport GetWeather(string city, LlmAccess llm) {
        var p = $"Return weather for {city} as JSON: city, temperature, condition.";
        return llm_structured<WeatherReport>("", "", p, llm);
    }

    // Streaming (callback per chunk)
    public void Stream(string prompt, LlmAccess llm) {
        llm_stream("gpt-4o", [{"role": "user", "content": prompt}], (chunk) => {
            print chunk;
        }, llm);
    }

    public void Run() {
        unsafe {
            var llm  = LlmAccess {};
            var file = FileAccess {};
            print self.Chat("What is Varg?", llm);
        }
    }
}
```

Provider/model defaults from env: `VARG_LLM_PROVIDER`, `VARG_LLM_MODEL`.

---

## 29. Vector Store & Embeddings

```csharp
agent VectorApp {
    public void Run() {
        var store = __varg_vector_store_open("my_store");

        // Embed text (requires LLM API — use embed_local for no-API-key)
        var emb = __varg_embed("This is my document text");
        // OR: local embedding (384-dim, no network)
        var emb = embed_local("This is my document text");

        // Upsert
        __varg_vector_store_upsert(store, "doc1", emb, {"source": "manual"});

        // Search
        var query_emb = embed_local("search query");
        var results   = __varg_vector_store_search(store, query_emb, 5);
        // results: List<map<string, string>>

        var count = __varg_vector_store_count(store);
        __varg_vector_store_delete(store, "doc1");
    }
}
```

Batch embeddings:
```csharp
var texts = ["doc1", "doc2", "doc3"];
var embeddings = embed_local_batch(texts);    // List of embedding vectors
```

---

## 30. Knowledge Graph

```csharp
var g  = __varg_graph_open("my_graph");
var p1 = __varg_graph_add_node(g, "Person", {"name": "Alice", "age": "30"});
var p2 = __varg_graph_add_node(g, "Person", {"name": "Bob",   "age": "25"});
var c1 = __varg_graph_add_node(g, "Company", {"name": "Acme"});

__varg_graph_add_edge(g, p1, "knows", p2, {});
__varg_graph_add_edge(g, p1, "works_at", c1, {"since": "2020"});

var persons    = __varg_graph_query(g, "Person");
var network    = __varg_graph_traverse(g, p1, 2, "knows");
var neighbors  = __varg_graph_neighbors(g, p1);
```

---

## 31. Agent Memory (3-Layer)

```csharp
var mem = __varg_memory_open("bot_memory");

// Working memory (ephemeral KV)
__varg_memory_set(mem, "current_task", "analysis");
var task = __varg_memory_get(mem, "current_task", "none");
__varg_memory_clear_working(mem);

// Episodic memory (vector-based, persisted)
__varg_memory_store(mem, "User asked about Rust", {"topic": "programming"});
var episodes = __varg_memory_recall(mem, "Rust programming", 5);

// Semantic memory (graph-based facts, persisted)
var fact_id = __varg_memory_add_fact(mem, "User", {"name": "Alice", "lang": "English"});
var facts   = __varg_memory_query_facts(mem, "User");
```

---

## 32. Observability & Tracing

```csharp
var tracer = __varg_trace_start("my_agent");
var span   = __varg_trace_span(tracer, "process_order");
__varg_trace_set_attr(tracer, "order_id", "1234");
__varg_trace_event(tracer, "payment_received", {"amount": "50.00"});
__varg_trace_end(tracer, span);

var json_export = __varg_trace_export(tracer);
fs_write("trace.json", json_export, files);
```

---

## 33. Event Bus & Pipelines

```csharp
// Event Bus
var bus = __varg_event_bus_new("system");
__varg_event_on(bus, "user_joined", (data) => {
    print $"Welcome {data["name"]}";
    return "ok";
});
__varg_event_emit(bus, "user_joined", {"name": "Alice"});
var count = __varg_event_count(bus, "user_joined");

// Pipeline (sequential transforms)
var pipe = __varg_pipeline_new("data_pipe");
__varg_pipeline_add_step(pipe, "clean",   (input) => trim(input));
__varg_pipeline_add_step(pipe, "upper",   (input) => to_upper(input));
__varg_pipeline_add_step(pipe, "bracket", (input) => $"[{input}]");
var result = __varg_pipeline_run(pipe, "  hello world  ");
```

---

## 34. Agent Orchestration

```csharp
var orch = __varg_orchestrator_new("workers");
__varg_orchestrator_add_task(orch, "task1", "input_one");
__varg_orchestrator_add_task(orch, "task2", "input_two");
__varg_orchestrator_add_task(orch, "task3", "input_three");

// Run all tasks with a handler function
__varg_orchestrator_run_all(orch, (input) => {
    return to_upper(input);
});

// Get results — List<map<string, string>>
var results = __varg_orchestrator_results(orch);
for r in results {
    print $"Task {r["id"]}: {r["result"]}";
}
```

---

## 35. Self-Improving Agents

```csharp
var si = __varg_self_improver_new("coder_agent", 5);

__varg_self_improver_record_success(si, "Fix null pointer", "Added null check before access");
__varg_self_improver_record_failure(si, "Parse JSON", "Forgot to handle empty string");

var lessons = __varg_self_improver_recall(si, "null pointer", 3);
var stats   = __varg_self_improver_stats(si);
print $"Success rate: {stats["success_rate"]}";
```

---

## 36. Human-in-the-Loop (HITL)

```csharp
// Approval gate
var approved = await_approval("Deploy to production? (cost: $0.50)");
if approved {
    deploy();
}

// Text input
var name = await_input("What is your name? ");

// Multiple choice
var action = await_choice("Next step:", ["Retry", "Skip", "Abort"]);
match action {
    "Retry" => { retry_operation(); }
    "Abort" => { return; }
    _       => { }
}
```

---

## 37. Rate Limiting

```csharp
// Manual (token bucket)
var rl = ratelimiter_new(10, 60000);   // 10 calls per 60 seconds
if !ratelimiter_acquire(rl, user_id) {
    return "Rate limit exceeded.";
}

// Via annotation (positional string args: "max_calls", "window_ms")
@[RateLimit("10", "60000")]
public string CallApi(string input, NetworkAccess net) {
    return fetch($"https://api.example.com/{input}", "GET", net) or "";
}
```

---

## 38. LLM Budget / Cost Tracking

```csharp
// Manual
var b = budget_new(100000, 1000);   // 100k tokens, $10.00

if !budget_check(b) {
    return "Budget exhausted: " + budget_report(b);
}
var response = llm_chat("gpt-4o", messages, llm);
budget_track(b, prompt, response);

var remaining = budget_remaining_tokens(b);
var usd_left  = budget_remaining_usd_cents(b);

// Via annotation ("max_tokens", "max_usd_cents")
@[Budget("100000", "1000")]
public string QueryAi(string prompt, LlmAccess llm) {
    return llm_chat("gpt-4o", [{"role": "user", "content": prompt}], llm);
}
```

---

## 39. Agent Checkpoint & Resume

```csharp
var cp = checkpoint_open("agent.db", "worker_v1");

if checkpoint_exists(cp) {
    var saved = checkpoint_load(cp);
    self.state = json_parse(saved);
    print $"Resumed (age: {checkpoint_age(cp)}s)";
}

// Do work...
checkpoint_save(cp, json_stringify(self.state));

checkpoint_clear(cp);    // remove saved state

// Via annotation ("db_path")
@[Checkpointed("worker.db")]
public void DoWork(string input) {
    // state auto-persisted
}
```

---

## 40. Typed Channels

```csharp
var ch = channel_new(50);           // buffered, capacity 50

// Producer
channel_send(ch, json_stringify(task));

// Consumer (blocking)
var raw = channel_recv(ch);
var task = json_parse(raw);

// Consumer with timeout
var raw = channel_recv_timeout(ch, 5000);   // wait up to 5s
if raw != "" {
    process(json_parse(raw));
}

// Non-blocking try
var raw = channel_try_recv(ch);

var pending = channel_len(ch);
channel_close(ch);
var closed = channel_is_closed(ch);
```

---

## 41. Property-Based Testing

```csharp
agent PropertyTests {
    // @[Property("runs")] — runs N random iterations
    @[Property("200")]
    public void TestBase64RoundTrip() {
        var s   = prop_gen_string(50);
        var enc = base64_encode(s);
        var dec = base64_decode(enc);
        prop_assert(dec == s, $"Failed for: {s}");
    }

    @[Property("100")]
    public void TestSortLength() {
        var xs = prop_gen_int_list(-1000, 1000, 20);
        prop_assert(xs.sort().len() == xs.len(), "sort changes length");
    }
}

// Generators
prop_gen_int(min, max)              // random int
prop_gen_float(min, max)            // random float
prop_gen_bool()                     // random bool
prop_gen_string(max_len)            // random string
prop_gen_int_list(min, max, size)   // List<int>
prop_gen_string_list(max_len, size) // List<string>

prop_assert(condition, message)     // assertion in property test
prop_check(runs, fn)                // run property fn N times
```

---

## 42. Workflow DAG

```csharp
var wf = workflow_new("data_pipeline");
workflow_add_step(wf, "download", []);              // no deps
workflow_add_step(wf, "parse",    ["download"]);    // depends on download
workflow_add_step(wf, "validate", ["parse"]);
workflow_add_step(wf, "store",    ["validate"]);

while !workflow_is_complete(wf) {
    var ready = workflow_ready_steps(wf);
    for step in ready {
        var result = execute_step(step);
        workflow_set_output(wf, step, result);
    }
}
var steps  = workflow_step_count(wf);
var status = workflow_status(wf);
```

---

## 43. Test Framework

```csharp
agent MyTests {
    @[BeforeEach]
    public void setup() {
        // runs before each @[Test] method
    }

    @[AfterEach]
    public void teardown() {
        // runs after each @[Test] method
    }

    @[Test]
    public void test_basic_math() {
        assert_eq(1 + 1, 2, "addition");            // message required
        assert_ne(1, 2, "different values");         // message required
        assert(2 > 1, "ordering holds");             // message required
    }

    @[Test]
    public void test_strings() {
        assert_true("abc".starts_with("a"));         // message optional
        assert_false("abc".is_empty());
        assert_contains("hello world", "world");
        assert_throws(() => { throw "boom"; });

        // With optional message
        assert_true(1 > 0, "positive check");
        assert_contains("hello world", "world", "must contain world");
    }

    @[Test]
    public void test_with_di() {
        var db  = MockDb();
        var svc = MyService(db);
        var r   = svc.GetData("key");
        assert_eq(r, "mock", "DI mock result");
    }
}
```

**Assertion signatures:**
| Assertion | Signature | Message |
|-----------|-----------|---------|
| `assert` | `assert(cond, msg)` | **required** |
| `assert_eq` | `assert_eq(actual, expected, msg)` | **required** |
| `assert_ne` | `assert_ne(a, b, msg)` | **required** |
| `assert_true` | `assert_true(cond[, msg])` | optional |
| `assert_false` | `assert_false(cond[, msg])` | optional |
| `assert_contains` | `assert_contains(haystack, needle[, msg])` | optional |
| `assert_throws` | `assert_throws(closure[, msg])` | optional |

CLI: `vargc test my_tests.varg`
Coverage: `vargc test --coverage my_tests.varg`

---

## 44. External Rust Crates

```csharp
// Auto-added to Cargo.toml
import crate serde_json;
import crate reqwest = "0.12";
import crate reqwest = "0.12" features ["json", "blocking"];

// Qualified imports (use statements in generated Rust)
import serde_json::Value;
import axum::Router;
import axum::{Router, Json, Extension};
import tokio::*;
```

---

## 45. Package Registry

```csharp
var reg = registry_open("varg-packages.json");
registry_install(reg, "varg-rag", "2.1.0");

if registry_is_installed(reg, "varg-rag") {
    print $"varg-rag {registry_version(reg, "varg-rag")} installed";
}

registry_uninstall(reg, "old-pkg");
var http_pkgs = registry_search(reg, "http");
var all       = registry_list(reg);
```

---

## 46. Tensor Operations (ndarray)

```csharp
var zeros = tensor_zeros([3, 4]);
var ones  = tensor_ones([2, 2]);
var eye   = tensor_eye(4);
var t     = tensor_from_list([1.0, 2.0, 3.0, 4.0], [2, 2]);

var shape = tensor_shape(t);          // [2, 2]
var flat  = tensor_reshape(t, [4]);

var c   = tensor_add(a, b);
var s   = tensor_mul_scalar(t, 2.0);
var mm  = tensor_matmul(a, b);
var dot = tensor_dot(a, b);

var sum  = tensor_sum(t);
var mean = tensor_mean(t);
var max  = tensor_max(t);

var data = tensor_to_list(t);         // float[]
```

---

## 47. DataFrame (Polars)

Requires `--features dataframe` when building varg-runtime.

```csharp
var df = df_read_csv("data.csv", file_cap);
var pq = df_read_parquet("data.parquet", file_cap);
df_write_csv(df, "out.csv", file_cap);
df_write_parquet(df, "out.parquet", file_cap);

var slim   = df_select(df, ["name", "age"]);
var adults = df_filter(df, "age > 18");         // DSL: "col op value"
var sorted = df_sort(df, "score", true);         // ascending=true

var grouped = df_groupby(df, ["city"]);
var agg     = df_agg(df, ["city"], "mean");      // sum|mean|count|min|max

var top     = df_head(df, 10);
var shape   = df_shape(df);                      // (rows, cols)
var cols    = df_columns(df);

var extended = df_with_column(df, "rank", [1.0, 2.0, 3.0]);
```

---

## 48. DuckDB Analytical SQL

Requires `--features duckdb`.

```csharp
unsafe {
    var da = DbAccess {};
    var db = duckdb_open(":memory:");
    duckdb_execute(db, "CREATE TABLE sales (product TEXT, amount DOUBLE)", da);
    duckdb_execute(db, "INSERT INTO sales VALUES ('Widget', 120.5)", da);
    var rows = duckdb_query(db, "SELECT product, SUM(amount) FROM sales GROUP BY product", da);
    for row in rows {
        print $"{row["product"]}: {row["SUM(amount)"]}";
    }
    duckdb_close(db);
}
```

---

## 49. Full-Text Search (BM25 / tantivy)

Requires `--features fts`.

```csharp
var idx = fts_open(":memory:");
fts_add(idx, "doc1", "the quick brown fox");
fts_add(idx, "doc2", "rust systems programming");
fts_commit(idx);

var results = fts_search(idx, "fox", 10);   // ranked doc IDs
for id in results {
    print id;
}

fts_delete(idx, "doc1");
fts_close(idx);

// Hybrid BM25 + vector search (RRF fusion)
var hits = rag_hybrid_search(fts_idx, vector_store, embed_local(query), query, 5);
```

---

## 50. RAG Pipeline

```csharp
var store = vector_store_open("docs");

// Index
rag_index(store, "doc1", "Varg is a compiled language for AI agents", {});
rag_index(store, "doc2", "Rust provides memory safety without GC", {});

// Retrieve
var chunks = rag_retrieve(store, embed_local("AI agent language"), 3);

// Build prompt with injected context
var prompt = rag_build_prompt("What is Varg?", chunks);
print prompt;
```

---

## 51. Compile-Time Safety Features

### Agent Spawn Graph Validation
```csharp
// Cycle detection — this fails to compile:
agent A { public void Run() { var b = spawn B(); } }
agent B { public void Run() { var a = spawn A(); } }
// Error: agent spawn cycle detected: A → B → A
```

### Import Cycle Detection
Circular module imports are detected at compile time.

---

## 52. CLI Reference

```bash
vargc build hello.varg              # compile to native binary
vargc run hello.varg                # compile and run
vargc emit-rs hello.varg            # emit generated Rust source
vargc test my_tests.varg            # run @[Test] functions
vargc test --coverage my_tests.varg # run with LLVM coverage
vargc watch hello.varg              # recompile on file change
vargc fmt hello.varg                # format source code
vargc doc myfile.varg               # generate HTML API docs → myfile.html
```

---

## 53. API Documentation (`vargc doc`)

```csharp
/// Main application agent.
/// Handles user requests and orchestrates sub-agents.
agent MyApp {
    /// Processes the given input and returns a result.
    /// Returns empty string on failure.
    public string Process(string input) {
        return input.to_upper();
    }
}
```
`vargc doc myfile.varg` → `myfile.html` (dark-themed, self-contained)

---

## Complete Example: HTTP API with SQLite & DI

```csharp
// contracts.varg
contract IUserRepo {
    void create(string name, string email);
    string find(int id);
    string list();
}

// repo.varg
import contracts;

agent SqliteUserRepo : IUserRepo {
    string db_path;

    public SqliteUserRepo(string path) {
        self.db_path = path;
        var db = db_open(path);
        db_execute(db, "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)", []);
    }

    public void create(string name, string email) {
        var db = db_open(self.db_path);
        db_execute(db, "INSERT INTO users (name, email) VALUES (?1, ?2)", [name, email]);
    }

    public string find(int id) {
        var db = db_open(self.db_path);
        var rows = db_query(db, "SELECT * FROM users WHERE id = ?1", [$"{id}"]);
        return json_stringify(rows);
    }

    public string list() {
        var db = db_open(self.db_path);
        var rows = db_query(db, "SELECT * FROM users", []);
        return json_stringify(rows);
    }
}

// main.varg
import contracts;
import repo;

agent ApiServer {
    IUserRepo users;

    public ApiServer(IUserRepo repo) {
        self.users = repo;
    }

    public async void Run() {
        var server = http_serve();

        http_route(server, "GET", "/users", (req) => {
            return http_response(200, self.users.list());
        });

        http_route(server, "POST", "/users", (req) => {
            var data = json_parse(req.body);
            var name  = json_get(data, "name");
            var email = json_get(data, "email");
            self.users.create(name, email);
            return http_response(201, "{\"ok\":true}");
        });

        http_route(server, "GET", "/health", (req) => {
            return http_response(200, "{\"status\":\"ok\"}");
        });

        print "Server running on :8080";
        http_listen(server, "0.0.0.0:8080");
    }
}

fn main() -> void {
    var repo   = SqliteUserRepo("users.db");
    var server = ApiServer(repo);
    server.Run();
}
```

---

**INSTRUCTIONS FOR AI AGENTS:**
When writing Varg code, strictly follow this guide. The most common mistakes are:
1. Using `name: Type` in `fn` parameters — use `Type name` instead
2. Using `fn` keyword inside `contract` — contracts use `ReturnType Name(Type param);`
3. Using `var` for agent fields — agent body fields use `Type name;`
4. Using annotation parameters that aren't string literals — `@[Name("val")]` only
5. Forgetting OCAP tokens — any file/network/db/llm/shell operation needs a capability token
