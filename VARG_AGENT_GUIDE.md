# Varg: AI Agent Developer Guide

You are an AI assistant tasked with writing code in **Varg**, a compiled programming language specifically designed for autonomous AI agents. Varg transpiles to Rust and provides native performance with a C#-like syntax.

**CRITICAL RULES FOR WRITING VARG:**

## 1. Syntax Basics
- **Statically Typed:** Variables are declared with `var` (type inferred) or explicitly (e.g., `string name = "Bot";`).
- **Mutable by Default:** All variables can be reassigned.
- **Statements:** End with semicolons `;`.
- **String Interpolation:** Use `$"Hello {name}"`.
- **Functions:** Use `fn name(type arg) -> ret_type { ... }` natively.
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

## 6. Strings and Standard Library
- **Strings:** `.split()`, `.contains()`, `.starts_with()`, `.ends_with()`, `.replace()`, `.trim()`, `.to_upper()`, `.to_lower()`, `.substring()`, `.index_of()`, `.pad_left()`, `.pad_right()`, `.chars()`, `.reverse()`, `.repeat()`.
- **JSON:** `json_parse()`, `json_get()`, `json_get_int()`, `json_get_bool()`, `json_get_array()`, `json_stringify()`.

## 7. Advanced Agent Features
- **Actor Messaging:** `spawn Worker {}`, `worker.send("task", args)`, `worker.request("status")`. Worker implements `public void on_message(string msg, string[] args)`.
- **Retry / Fallback:** 
```csharp
var html = retry(3, backoff: 1000) {
    fetch(url, "GET")?
} fallback {
    ""
};
```

---
**INSTRUCTIONS FOR YOUR RESPONSE:**
When asked to write Varg code, produce ONLY standard Varg syntax matching the specifications above. Do not use Python, C++, or Rust paradigms directly unless they overlap with the C#-like Varg syntax. ALWAYS honor the OCAP security model.
