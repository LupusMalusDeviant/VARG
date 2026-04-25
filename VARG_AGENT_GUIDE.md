# Varg: AI Agent Developer Guide

You are an AI assistant tasked with writing code in **Varg**, a compiled programming language specifically designed for autonomous AI agents. Varg transpiles to Rust and provides native performance with a C#-like syntax.

**CRITICAL RULES FOR WRITING VARG:**

## 1. Syntax Basics
- **Statically Typed:** Variables are declared with `var` (type inferred) or explicitly (e.g., `string name = "Bot";`).
- **Mutable by Default:** All variables can be reassigned.
- **Statements:** End with semicolons `;`.
- **String Interpolation:** Use `$"Hello {name}"`. Varg's robust string interpolator supports nested expressions and character escaping safely.
- **Functions:** Use `fn name(type arg) -> ret_type { ... }` natively.
- **Method Modifiers:** Flexible modifier ordering is supported (e.g., `public async void` or `async public void`). Methods without visibility modifiers are private by default. Available modifiers include `public`, `private`, and `async`.
- **Entry Point:** Either an `agent` with `public void Run()` or a standalone `fn main()`.

## 2. Agents vs. Classes
Varg uses **Agents**, not classes. Agents have state (fields), lifecycle hooks, and methods.
```csharp
agent MyBot {
    int counter;

    public void on_start() {
        counter = 0;
    }

    public void Increment() {
        counter += 1;
    }

    public void Run() {
        self.Increment();
        print $"Count is {counter}";
    }
}
```

## 3. OCAP Security Model (CRITICAL)
Varg enforces capability-based security. Any system interaction **requires a capability token**, passed explicitly as an argument.

**Tokens:**
1. `FileAccess` - for `fs_read`, `fs_write`, `fs_append`, `fs_read_lines`, `fs_read_dir`, `create_dir`, `delete_file`
2. `NetworkAccess` - for `fetch`, `http_request`
3. `SystemAccess` - for `exec`, `exec_status`
4. `DbAccess` - for database queries
5. `LlmAccess` - for LLM interactions

**How to use them:**
Capabilities can **ONLY** be instantiated inside an `unsafe {}` block.

```csharp
agent WebFetcher {
    // 1. Demand capability in signature
    public string FetchUrl(string url, NetworkAccess net) {
        return fetch(url, "GET")?; // 2. ? propagates errors (Result type)
    }

    public void Run() {
        // 3. Construct token in unsafe block
        unsafe {
            var net = NetworkAccess {}; 
            var code = self.FetchUrl("https://example.com", net);
            print code;
        }
    }
}
```

## 4. Error Handling
- Use the `?` operator for functions returning `Result<T, string>`. 
- Using `?` automatically makes your function's return type `Result<T, string>`.
- Or use `try { ... } catch err { ... }`.
- Or use `or` fallback value: `var data = fs_read("file") or "default";`.

## 5. Built-in Collections & Methods
- **Arrays (`T[]`):** `.push(v)`, `.len()`, `.first()`, `.last()`, `.is_empty()`, `.sort()`, `.reverse()`.
- **Maps (`map<K,V>`):** `{"key": "val"}` or `map["key"]`. Methods: `.keys()`, `.values()`, `.contains_key(k)`, `.remove(k)`.
- **Sets (`set<T>`):** `set_of("a", "b")`. Methods: `.add(x)`, `.contains(x)`, `.remove(x)`.
- **Iterator Chains:** `.filter((x) => x > 0).map((x) => x * 2).find(...).any(...).all(...)`

## 6. Strings, Built-ins and Standard Library
- **Strings:** `.split()`, `.contains()`, `.starts_with()`, `.ends_with()`, `.replace()`, `.trim()`, `.to_upper()`, `.to_lower()`, `.substring()`, `.index_of()`, `.pad_left()`, `.pad_right()`, `.chars()`, `.reverse()`, `.repeat()`.
- **JSON:** `json_parse()`, `json_get()`, `json_get_int()`, `json_get_bool()`, `json_get_array()`, `json_stringify()`.
- **Built-in Prefix Stripping:** For all built-in methods (like Vector Store, Graph, Memory), you can omit the `__varg_` prefix when calling them as a method on their respective objects. For example, `store.vector_store_count()` instead of `__varg_vector_store_count(store)`.

## 7. Advanced Agent Features
- **Actor Messaging:** `spawn Worker {}`, `worker.send("task", args)`, `worker.request("status")`. Worker implements `public void on_message(string msg, string[] args)`.
- **Retry / Fallback:**
```csharp
var html = retry(3) {
    fetch(url, "GET")?
} fallback {
    ""
};
```

## 8. HTTP Server (axum-based, real async)
```csharp
agent ApiServer {
    public async void Run() {
        var server = http_serve();
        http_route(server, "GET", "/health", (req) => {
            return http_response(200, "{\"status\": \"ok\"}");
        });
        http_route(server, "POST", "/echo", (req) => {
            return http_response(200, req.body);
        });
        http_listen(server, "0.0.0.0:8080");
    }
}
```
- `http_serve()` creates a server instance
- `http_route(server, method, path, handler)` registers a route
- `http_listen(server, addr)` starts listening (async, blocks)
- Handler receives `req` with `.method`, `.path`, `.headers`, `.body`, `.query_params`
- Returns `http_response(status, body)` with `.status`, `.headers`, `.body`

## 9. Database (SQLite, real rusqlite)
```csharp
agent DbApp {
    public void Run() {
        var db = db_open(":memory:");  // or "app.db" for file
        db_execute(db, "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)", []);
        db_execute(db, "INSERT INTO users (name) VALUES (?1)", ["Alice"]);
        var rows = db_query(db, "SELECT * FROM users", []);
        // rows is List<Map<string, string>>
        for row in rows {
            print row["name"];
        }
    }
}
```
- `db_open(path)` opens SQLite (`:memory:` or file path)
- `db_execute(db, sql, params)` returns affected row count
- `db_query(db, sql, params)` returns `List<Map<string, string>>`
- Use `?1`, `?2` for parameterized queries

## 10. WebSocket Client (real tungstenite)
```csharp
agent WsClient {
    public void Run() {
        var ws = ws_connect("ws://localhost:8080/ws");
        ws_send(ws, "hello");
        var msg = ws_receive(ws);  // blocking
        print msg;
        ws_close(ws);
    }
}
```

## 11. MCP Protocol Client (JSON-RPC over stdio)
```csharp
agent McpClient {
    public void Run() {
        var conn = mcp_connect("npx", ["-y", "@modelcontextprotocol/server-everything"]);
        var tools = mcp_list_tools(conn);
        var result = mcp_call_tool(conn, "echo", {"message": "hello"});
        print result;
        mcp_disconnect(conn);
    }
}
```
- `mcp_connect(cmd, args)` spawns process and does initialize handshake
- `mcp_list_tools(conn)` returns available tools
- `mcp_call_tool(conn, name, params)` calls a tool, returns text result
- `mcp_disconnect(conn)` cleanly shuts down

## 12. SSE (Server-Sent Events)
```csharp
var writer = sse_stream();
sse_send(writer, "update", "data payload");
sse_close(writer);
```

## 13. Contracts & Dependency Injection
```csharp
contract IDatabase {
    fn query(sql: string) -> string;
}

agent SqliteDb implements IDatabase {
    public string query(string sql) { /* real impl */ }
}

agent MockDb implements IDatabase {
    public string query(string sql) { return "mock"; }
}

agent MyService {
    IDatabase db;  // contract-typed field -> Box<dyn Trait>

    public MyService(IDatabase db) {
        self.db = db;
    }

    public string getData() {
        return self.db.query("SELECT ...");
    }
}
```
- Contract-typed fields compile to `Box<dyn Trait>`
- Constructor injection: pass implementation at creation time
- Use for testing: inject MockDb in tests, SqliteDb in production

## 14. Test Framework
```csharp
agent MyTests {
    @[BeforeEach]
    public void setup() {
        // runs before each test
    }

    @[AfterEach]
    public void teardown() {
        // runs after each test
    }

    @[Test]
    public void test_addition() {
        assert_eq(1 + 1, 2);
    }

    @[Test]
    public void test_strings() {
        assert_contains("hello world", "world");
        assert_true("abc".starts_with("a"));
        assert_false("abc".is_empty());
    }
}
```
Run with: `vargc test my_tests.varg`
Coverage: `vargc test --coverage my_tests.varg`

**Assertions:** `assert`, `assert_eq`, `assert_ne`, `assert_true`, `assert_false`, `assert_contains`, `assert_throws`

## 15. External Crate Imports
```csharp
import crate serde_json;           // adds to Cargo.toml automatically
import serde_json::Value;          // qualified type import
import axum::{Router, Json};       // braced imports
import tokio::*;                   // wildcard
```
These compile to Rust `use` statements and the crate is auto-added to the generated Cargo.toml.

## 16. Date/Time, Logging, Environment
```csharp
var now = time_millis();
var formatted = time_format(now, "%Y-%m-%d");
log_info("Starting up");
log_error("Something failed");
var key = env("API_KEY");
```

## 17. Vector Store & Embeddings
Varg includes an embedded vector store using cosine similarity natively.
```csharp
var store = __varg_vector_store_open("my_store");
var meta = {"source": "docs"};
var embedding = __varg_embed("this is my text"); // LLM embedding wrapper
__varg_vector_store_upsert(store, "doc1", embedding, meta);

var results = __varg_vector_store_search(store, embedding, 5); // returns List<Map<string,string>>
var count = __varg_vector_store_count(store);
var deleted = __varg_vector_store_delete(store, "doc1");
```

## 18. Knowledge Graph
Native embedded graph database for semantic relationships.
```csharp
var g = __varg_graph_open("my_graph");
var p1 = __varg_graph_add_node(g, "Person", {"name": "Alice"});
var p2 = __varg_graph_add_node(g, "Person", {"name": "Bob"});
__varg_graph_add_edge(g, p1, "knows", p2, {});

var persons = __varg_graph_query(g, "Person");
var network = __varg_graph_traverse(g, p1, 2, "knows");
var neighbors = __varg_graph_neighbors(g, p1);
```

## 19. Agent Memory (3-Layer Architecture)
Manages working (KV), episodic (Vector), and semantic (Graph) memory.
```csharp
var mem = __varg_memory_open("bot");
// Working memory
__varg_memory_set(mem, "task", "coding");
var t = __varg_memory_get(mem, "task", "default");
__varg_memory_clear_working(mem);

// Episodic memory
__varg_memory_store(mem, "User asked for help with Rust", {"mood": "confused"});
var episodes = __varg_memory_recall(mem, "help with Rust", 5);

// Semantic memory
var fact_id = __varg_memory_add_fact(mem, "User", {"name": "Alice"});
var facts = __varg_memory_query_facts(mem, "User");
```

## 20. Event Bus & Pipelines
Reactive message passing.
```csharp
var bus = __varg_event_bus_new("sys");
// Note: Handlers are native Arc<dyn Fn> in compiled code, you can use closures in Varg
__varg_event_on(bus, "user_joined", (data) => {
    print $"Welcome {data["name"]}";
    return "ok";
});
__varg_event_emit(bus, "user_joined", {"name": "Alice"});

var pipe = __varg_pipeline_new("data_pipe");
__varg_pipeline_add_step(pipe, "uppercase", (input) => input.to_upper());
var result = __varg_pipeline_run(pipe, "hello");
```

## 21. Agent Orchestration (Fan-out / Fan-in)
Distributed sub-task execution.
```csharp
var orch = __varg_orchestrator_new("workers");
__varg_orchestrator_add_task(orch, "t1", "input1");
__varg_orchestrator_add_task(orch, "t2", "input2");
__varg_orchestrator_run_all(orch, (input) => { return input.to_upper(); });

var results = __varg_orchestrator_results(orch); // List of maps with id, input, status, result
```

## 22. Self-Improving Agents
Records successes/failures to learn from past mistakes.
```csharp
var si = __varg_self_improver_new("coder_agent", 5);
__varg_self_improver_record_success(si, "Fix bug #12", "Used mutex lock");
__varg_self_improver_record_failure(si, "Parse file", "Forgot to catch exception");

var past_lessons = __varg_self_improver_recall(si, "Fix bug", 3);
var stats = __varg_self_improver_stats(si);
```

## 23. Observability & Tracing
Lightweight OTEL-compatible span tracing.
```csharp
var tracer = __varg_trace_start("my_agent");
var span = __varg_trace_span(tracer, "process_order");
__varg_trace_set_attr(tracer, "order_id", "1234");
__varg_trace_event(tracer, "payment_received", {"amount": "50"});
__varg_trace_end(tracer, span);

var json_export = __varg_trace_export(tracer);
```

## 24. MCP Server Mode
Expose your Varg agent tools via Model Context Protocol.
```csharp
var server = mcp_server_new("my_tools", "1.0.0");
mcp_server_register(server, "greet", "Says hello", (args) => {
    return $"Hello {args}";
});
mcp_server_run(server); // Blocks on stdio JSON-RPC
```

## 25. Human-in-the-Loop (HITL)
Block agent execution until a human provides input or approval.
```csharp
var approved = await_approval("Deploy to production? (this costs $0.50)");
if approved {
    deploy();
}
var name = await_input("What is your name? ");
var action = await_choice("Next step:", ["Retry", "Skip", "Abort"]);
```

## 26. Rate Limiting
Protect APIs and resources from overuse with token-bucket rate limiting.
```csharp
var rl = ratelimiter_new(10, 60000); // 10 calls per 60 seconds
public string CallLlm(string prompt, string user_id, LlmAccess llm) {
    if !ratelimiter_acquire(rl, user_id) {
        return "Rate limit exceeded. Try again later.";
    }
    return llm_chat("gpt-4o", [{"role": "user", "content": prompt}], llm);
}
// Or use the annotation:
@[RateLimit(calls: 10, window_ms: 60000)]
public string CallLlmAnnotated(string prompt, LlmAccess llm) {
    return llm_chat("gpt-4o", [{"role": "user", "content": prompt}], llm);
}
```

## 27. LLM Budget / Cost Tracking
Enforce hard token and USD limits on LLM usage.
```csharp
var b = budget_new(100000, 1000); // 100k tokens, $10.00
public string Query(string prompt, LlmAccess llm) {
    if !budget_check(b) {
        return "Budget exhausted: " + budget_report(b);
    }
    var response = llm_chat("gpt-4o", [{"role":"user","content":prompt}], llm);
    budget_track(b, prompt, response);
    return response;
}
// Or use the annotation:
@[Budget(tokens: 100000, usd: 10)]
public string QueryAnnotated(string prompt, LlmAccess llm) {
    return llm_chat("gpt-4o", [{"role":"user","content":prompt}], llm);
}
```

## 28. Agent Checkpoint & Resume
Persist agent state to SQLite so interrupted agents can resume.
```csharp
var cp = checkpoint_open("agent.db", "worker_v1");
// Try to resume
if checkpoint_exists(cp) {
    var saved = checkpoint_load(cp);
    self.state = json_parse(saved);
    print $"Resumed from checkpoint (age: {checkpoint_age(cp)}s)";
}
// ... do work ...
checkpoint_save(cp, json_stringify(self.state)); // save progress
// Or use annotation — checkpoint() builtin auto-saves state:
@[Checkpointed("worker.db")]
public void DoWork(string input) { /* state auto-persisted */ }
```

## 29. Typed Channels
Pass messages between concurrent parts of an agent safely.
```csharp
var ch = channel_new(50); // buffered channel, capacity 50
// Producer
channel_send(ch, json_stringify(task));
// Consumer
var raw = channel_recv_timeout(ch, 5000); // wait up to 5s
if raw != "" {
    var task = json_parse(raw);
    process(task);
}
channel_close(ch);
```

## 30. Property-Based Testing
Test invariants over randomly generated inputs.
```csharp
@[Property(runs: 200)]
public void TestRoundTrip() {
    var s = prop_gen_string(50);
    var encoded = base64_encode(s);
    var decoded = base64_decode(encoded);
    prop_assert(decoded == s, $"base64 roundtrip failed for: {s}");
}
@[Property(runs: 100)]
public void TestSortLength() {
    var xs = prop_gen_int_list(-1000, 1000, 20);
    prop_assert(xs.sort().len() == xs.len(), "sort must not change length");
}
```

## 31. Multimodal (Image / Audio / Vision)
Load images and audio, pass to LLM for analysis.
```csharp
agent VisionAgent {
    public string Describe(string path, FileAccess files, LlmAccess llm) {
        var img = image_load(path, files);
        var b64 = image_to_base64(img);
        var fmt = image_format(img);
        return llm_vision("Describe this image in detail.", b64, fmt, llm);
    }
    public void Run() {
        unsafe {
            var f = FileAccess {};
            var l = LlmAccess {};
            print self.Describe("photo.png", f, l);
        }
    }
}
```

## 32. Workflow DAG
Declare steps with dependencies — ready steps are executed in order.
```csharp
var wf = workflow_new("data_pipeline");
workflow_add_step(wf, "download", []);
workflow_add_step(wf, "parse",    ["download"]);
workflow_add_step(wf, "validate", ["parse"]);
workflow_add_step(wf, "store",    ["validate"]);

while !workflow_is_complete(wf) {
    var ready = workflow_ready_steps(wf);
    foreach step in ready {
        var result = execute_step(step);
        workflow_set_output(wf, step, result);
    }
}
```

## 33. Package Registry
Manage local Varg packages for modular agent composition.
```csharp
var reg = registry_open("varg-packages.json");
registry_install(reg, "varg-rag", "2.1.0");
if registry_is_installed(reg, "varg-rag") {
    print $"varg-rag {registry_version(reg, "varg-rag")} installed";
}
var http_pkgs = registry_search(reg, "http");
```

---
**INSTRUCTIONS FOR YOUR RESPONSE:**
When asked to write Varg code, produce ONLY standard Varg syntax matching the specifications above. Do not use Python, C++, or Rust paradigms directly unless they overlap with the C#-like Varg syntax. ALWAYS honor the OCAP security model.
