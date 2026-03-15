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
    Custom(int)  // Variant with data
}

enum Status {
    Active,
    Inactive,
    Error(string)
}
```

### Pattern Matching

```csharp
match status {
    Status.Active => {
        print "System is running";
    }
    Status.Error(msg) => {
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

```csharp
if input == "" {
    throw "Input cannot be empty";
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
// Single expression
var double = (int x) => x * 2;

// Multi-line (block body)
var process = (string s) => {
    var upper = s.to_upper();
    return $"[{upper}]";
};

// Type-inferred in context
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
        var worker = spawn Worker {};

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

// External crate (from crates.io)
crate serde = "1.0" ["derive"];
crate reqwest = "0.11" ["json"];
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
var response = retry(3, backoff: 1000) {
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
assert(x > 0, "x must be positive");
assert_eq(result, expected, "values should match");
```

---

## Annotations

```csharp
@[Test]
public void TestAdd() {
    assert_eq(add(2, 3), 5, "2+3 should be 5");
}

@[McpTool("search", "Search the database")]
public string Search(string query) {
    // Auto-generates MCP tool schema
    return results;
}

@[CliCommand("greet", "Greet a user")]
public void Greet(string name) {
    print $"Hello, {name}!";
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
