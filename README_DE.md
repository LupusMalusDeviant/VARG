# Varg

<div align="center">
  <img src="docs/varg_logo.png" alt="Varg Logo" width="300"/>
</div>

**Eine kompilierte Programmiersprache fuer autonome KI-Agenten.**

Varg transpiliert nach Rust und liefert native Performance mit einer entwicklerfreundlichen C#-aehnlichen Syntax.
Von Grund auf fuer autonome Agenten konzipiert -- mit eingebauter Capability-basierter Sicherheit (OCAP), Actor-Model-Concurrency und nativen KI/LLM-Typen.

```
Varg Source (.varg) --> vargc --> Rust Source --> cargo build --> Native Binary
```

---

## Auf einen Blick

| Kennzahl | Wert |
|----------|------|
| Version | **2.2.0** |
| Tests | 1.264 (Standard) / 1.421 (`--features full`) |
| Golden-Programme | 50, jedes in der CI gebaut **und ausgefuehrt**, Ausgabe verglichen |
| Ablehnungsproben | 88 Programme, die abgelehnt werden muessen, je mit erwarteter Meldung |
| Builtins | 407 dokumentiert, 366 von einem Golden-Programm ausgefuehrt — [die 33 anderen, mit Grund](REFERENCE.md#what-ci-proves-and-what-it-does-not) |
| Crates | 10 Compiler-Crates |
| Runtime-Module | 40 |
| Sicherheit | 5 OCAP-Capability-Typen |
| Lexer-Token | 124 |
| AST | 21 Anweisungsarten, 33 Ausdrucksarten |
| Beispiele | 12 Programme, alle in der CI gebaut |

---

## Schnellbeispiel

```csharp
agent WeatherBot {
    public async string GetForecast(string city) {
        unsafe {
            var net = NetworkAccess {};
            var resp = fetch($"https://api.weather.com/{city}", "GET")?;
            var temp = json_get(resp, "/main/temp") or "unknown";
            return $"It's {temp} C in {city}";
        }
    }

    public async void Run() {
        var forecast = await self.GetForecast("Berlin");
        print forecast;
    }
}
```

```bash
vargc run weather.varg
```

---

## Warum Varg?

| Feature | Varg | Python | TypeScript | Rust |
|---------|:----:|:------:|:----------:|:----:|
| Native Binary | Ja | - | - | Ja |
| Agent-First Design | Ja | - | - | - |
| Compile-Time Security (OCAP) | Ja | - | - | - |
| Actor Model eingebaut | Ja | - | - | - |
| LLM/KI-Typen nativ | Ja | - | - | - |
| Zugaengliche Syntax | Ja | Ja | Ja | - |
| Retry/Fallback-Syntax | Ja | - | - | - |
| Prompt als Typ | Ja | - | - | - |
| Knowledge Graph eingebaut | Ja | - | - | - |
| Vector Store eingebaut | Ja | - | - | - |
| Agent Memory (3-Schichten) | Ja | - | - | - |
| Observability / Tracing | Ja | - | - | - |
| MCP Server + Client | Ja | - | - | - |
| Readline/REPL eingebaut | Ja | - | - | - |
| Platform-Config-Kaskade | Ja | - | - | - |
| LLM-Budget / Kosten-Tracking | Ja | - | - | - |
| Agent Checkpoint/Resume | Ja | - | - | - |
| Rate Limiting | Ja | - | - | - |
| Typed Channels | Ja | - | - | - |
| Property-Based Testing | Ja | - | - | - |
| Multimodal (Bild/Audio/Vision) | Ja | - | - | - |
| Workflow-DAG | Ja | - | - | - |
| Paket-Registry | Ja | - | - | - |
| Human-in-the-Loop (HITL) | Ja | - | - | - |
| Lokale Embeddings (kein API-Key) | Ja | - | - | - |
| Analytisches SQL (DuckDB) | Ja | - | - | - |
| Volltextsuche (BM25) | Ja | - | - | - |
| Hybrid-RAG-Suche | Ja | - | - | - |

---

## Leistung

Varg kompiliert zu nativen Rust-Binaries — kein Interpreter, kein Garbage Collector. Alle Zahlen
unten stammen aus [`benchmarks/`](benchmarks/), das eingecheckt ist: Quellen in vier Sprachen,
der Runner und jede einzelne der 30 Messungen pro Wert. Selbst nachfahren mit
`python benchmarks/run_all.py`.

Von den Programmen um die eigene Arbeit gemessene Zeit, Median aus 30 Laeufen auf einer Maschine
(Windows 11, .NET 10, Node 24, Python 3.14):

| Workload | Varg | C# | TypeScript | Python |
|----------|-----:|---:|-----------:|-------:|
| fib(35), rekursiv | **15.5 ms** | 53 ms | 53 ms | 700 ms |
| 100k-Liste: bauen, filtern, mappen, summieren | **1 ms** | 12 ms | 4 ms | 9 ms |
| JSON mit 1000 Records: bauen, parsen, neu serialisieren | 1 ms | 22 ms | **<1 ms** | 2 ms |
| Wortfrequenz, 200k eigene Schluessel | 31 ms | 63 ms | 37 ms | **28 ms** |

Schneller als C# in allen vier. Die Wortfrequenz verliert es gegen Python, und der Grund ist nicht
der naheliegende. In Phasen gemessen ist die Zaehlschleife sogar *schneller* als Pythons — 21 ms
gegen 26 — Hashing und Map-Zugriffe sind also nicht der Rueckstand. Er steckt vollstaendig im
abschliessenden Sortieren: 18 ms gegen 2 ms.

Pythons `dict` bewahrt die Einfuegereihenfolge, `sorted()` bekommt die Schluessel also in der
Reihenfolge, in der sie gezaehlt wurden — voller langer aufsteigender Laeufe, die Timsort fast
umsonst zusammenfuehrt. Rusts `HashMap` gibt sie in Hash-Reihenfolge heraus, praktisch gemischt.
Rust sortiert *dieselben* Schluessel in Zaehlreihenfolge ebenfalls in 2 ms; auf wirklich
gemischter Eingabe braucht Python 30 ms und Rust 18 ms, das Ergebnis dreht sich also um. Der
Workload misst einen Unterschied darin, was eine Map ueber Reihenfolge zusichert, nicht einen
Geschwindigkeitsunterschied.

Varg und C# werden als gebaute Binaries gemessen, Python und Node ueber ihren Interpreter, weil
diese Sprachen so benutzt werden. Die Bauzeit steht getrennt in
[`benchmarks/BENCHMARKS.md`](benchmarks/BENCHMARKS.md) — sie ist ein Preis der Werkzeugkette,
nicht des Programms. Es sind Mikro-Benchmarks auf einer Maschine: lesenswert sind die
Verhaeltnisse.

Ein in Varg geschriebener MCP-Server beantwortet seine erste Anfrage in **5 ms**, gegen 27—29 ms
fuer ein nacktes Node- oder Python-Skript ohne geladenes SDK — und er wird als eine Binary
ausgeliefert, ohne Runtime.

---

## Sprach-Features

### Kern-Sprache
- **C#-meets-Rust-Syntax** -- vertraut fuer die meisten Entwickler
- **Agents & Actors** -- erstklassiges `agent`-Keyword mit Lifecycle (`on_start`, `on_stop`, `on_message`), State-Management und Message-Passing (`spawn`, `send`, `request`)
- **OCAP-Sicherheit** -- 5 Capability-Token-Typen, zur Compile-Zeit erzwungen
- **Contracts** -- Interface-First-Design mit Compile-Time-Enforcement
- **Generics** -- vollstaendige generische Structs, Funktionen und Trait Bounds (`<T: Display>`)
- **Enums + Pattern Matching** -- exhaustives `match` mit Guards und Wildcard
- **Closures & Lambdas** -- `(x) => x * 2` mit Typinferenz
- **Async/Await** -- basierend auf tokio Runtime
- **Error Handling** -- `Result<T, E>`, `?`-Operator, `try/catch`, `or`-Fallback
- **Pipe-Operator** -- `data |> transform |> send`
- **String-Interpolation** -- `$"Hallo {name}, du hast {count} Eintraege"`
- **Multiline Strings** -- `"""..."""` fuer Prompts und Templates
- **Iterator-Chains** -- `.filter().map().find().any().all().sort()`
- **Tuples, Ranges, HashSet** -- `(int, string)`, `0..10`, `set<T>`
- **Modulsystem** -- `import math.{sqrt, abs}`
- **Standalone-Funktionen** -- Top-Level `fn`-Definitionen ausserhalb von Agents
- **Type-Aliase** -- `type Score = int`

### KI/Agent-spezifisch
- **Retry/Fallback** -- `retry(3, backoff: 1000) { api_call() } fallback { cached_result() }`
- **Agent Lifecycle** -- `on_start`, `on_stop`, `on_message` Hooks
- **Agent Messaging** -- `spawn`, `send`, `request` fuer Actor-Model-Kommunikation
- **Prompt-Templates** -- erstklassiges `prompt`-Keyword
- **MCP Client** -- verbinde dich mit MCP-Servern, liste Tools, rufe Tools auf (JSON-RPC ueber Stdio)
- **MCP Server** -- stelle Agent-Methoden als MCP-Tools fuer andere KI-Systeme bereit
- **Knowledge Graph** -- eingebettete Graph-Engine mit Knoten, Kanten, Traversal, Queries
- **Vector Store** -- Text einbetten, Vektoren speichern, Cosine-Similarity-Suche
- **Agent Memory** -- 3-Schichten-Architektur: Working (Key-Value), Episodic (Vector), Semantic (Graph)
- **Observability** -- hierarchisches Span-Tracing mit Events, Attributen, JSON-Export
- **Reactive Pipelines** -- Event Bus (Pub/Sub) + sequentieller Pipeline-Runner
- **Agent Orchestration** -- Fan-Out/Fan-In Parallelisierung, Task-Queues
- **Self-Improving Loop** -- Feedback-Tracking, Success/Failure-Recall via Similarity-Suche
- **LLM-Provider-Abstraktion** -- OpenAI, Anthropic, Ollama mit einheitlicher API

### Standardbibliothek (200+ Builtins)
- **Strings** -- `split`, `contains`, `starts_with`, `ends_with`, `replace`, `trim`, `to_upper`, `to_lower`, `substring`, `index_of`, `pad_left`, `pad_right`, `chars`, `reverse`, `repeat`
- **Collections** -- `push`, `pop`, `len`, `filter`, `map`, `find`, `any`, `all`, `sort`, `contains`, `remove`, `keys`, `values`
- **Datei-I/O** -- `fs_read`, `fs_write`, `fs_append`, `fs_read_lines`, `fs_read_dir`
- **Binaere I/O** -- `fs_read_bytes`, `fs_write_bytes`, `fs_append_bytes`, `fs_size`
- **Config + Platform-Dirs** -- `home_dir`, `config_dir`, `data_dir`, `cache_dir`, `config_load_cascade` (Deep-JSON-Merge ueber mehrere Quellen)
- **REPL / Readline** -- `readline_new`, `readline_read`, `readline_add_history`, `readline_load_history`, `readline_save_history` (rustyline-basierter Line-Editor)
- **HTTP Client** -- `fetch` (GET/POST/PUT/DELETE), `http_request` (mit Status, Headers)
- **HTTP Server** -- `http_serve`, `http_route`, `http_listen` (echter axum-basierter Async-Server)
- **Datenbank** -- `db_open`, `db_execute`, `db_query` (echtes SQLite via rusqlite, gebundelt)
- **WebSocket** -- `ws_connect`, `ws_send`, `ws_receive`, `ws_close` (echter tungstenite)
- **SSE Streaming** -- `sse_connect`, `sse_read`, `sse_close` (Streaming-LLM-Antworten)
- **Prozess-Management** -- `proc_spawn`, `proc_wait`, `proc_kill`, `proc_status`
- **MCP Client** -- `mcp_connect`, `mcp_list_tools`, `mcp_call_tool`, `mcp_disconnect`
- **MCP Server** -- `mcp_server_new`, `mcp_server_register`, `mcp_server_run`
- **Knowledge Graph** -- `graph_open`, `graph_add_node`, `graph_add_edge`, `graph_query`, `graph_traverse`, `graph_neighbors`
- **Vector Store** -- `embed`, `vector_store_open`, `vector_store_upsert`, `vector_store_search`, `vector_store_delete`, `vector_store_count`
- **Agent Memory** -- `memory_open`, `memory_set`, `memory_get`, `memory_store`, `memory_recall`, `memory_add_fact`, `memory_query_facts`
- **Tracing** -- `trace_start`, `trace_span`, `trace_end`, `trace_error`, `trace_event`, `trace_set_attr`, `trace_export`
- **JSON** -- `json_parse`, `json_get`, `json_get_int`, `json_get_bool`, `json_get_array`, `json_stringify`
- **Base64 + PDF** -- `base64_encode`, `base64_decode`, `pdf_create`, `pdf_add_section`, `pdf_add_text`, `pdf_save`
- **Shell** -- `exec`, `exec_status`
- **Datum/Zeit** -- `time_millis`, `time_format`, `time_parse`, `time_add`, `time_diff`
- **Kryptographie** -- `encrypt`, `decrypt`
- **Logging** -- `log_debug`, `log_info`, `log_warn`, `log_error`
- **Mathematik** -- `abs`, `sqrt`, `floor`, `ceil`, `round`, `min`, `max`, `pow`, `parse_int`, `parse_float`
- **Umgebung** -- `env("KEY")` fuer Umgebungsvariablen
- **Lokale Embeddings** -- `embed_local(text)`, `embed_local_batch(texts)` — reines Rust, 384-dimensionale Embeddings, kein API-Key erforderlich
- **DuckDB (Analytisches SQL)** -- `duckdb_open`, `duckdb_execute`, `duckdb_query`, `duckdb_close` — Feature-Flag `duckdb`
- **Volltextsuche (BM25)** -- `fts_open`, `fts_add`, `fts_commit`, `fts_search`, `fts_delete`, `fts_close` — tantivy-basiert, Feature-Flag `fts`
- **RAG-Pipeline** -- `rag_index`, `rag_retrieve`, `rag_build_prompt`, `rag_hybrid_search` (BM25 + Kosinus-RRF, benoetigt `fts`)

### Tooling
- **VS Code Extension** -- Syntax-Highlighting fuer `.varg`-Dateien
- **Language Server (LSP)** -- Echtzeit-Diagnosen, Hover-Info, Autovervollstaendigung, Gehe zu Definition (F12), Referenzen suchen (Shift+F12), Dokumentsymbole (Gliederung/Breadcrumb)
- **Debug-Modus** -- `vargc build --debug` fuer schnelle Iteration (ueberspringt cargo)
- **Source Maps** — ein Compile-Fehler nennt das Varg-Konstrukt, aus dem er stammt, und ein
  Laufzeitfehler nennt Datei, Konstrukt und die wievielte Anweisung darin (Anweisungen, nicht
  Zeilen: der AST fuehrt keine Quellpositionen, und eine Zahl, die wie eine Zeile aussieht und
  keine ist, waere schlechter als keine)
- **Test-Framework** -- `@[Test]`-Annotation + `assert` / `assert_eq`
- **Dokumentation** -- `vargc doc meinedatei.varg` — eigenstaendige, dunkles HTML-Seite mit API-Dokumentation
- **System-Tools** -- `vargc doctor` (Systemgesundheitscheck), `vargc upgrade` (Selbst-Update)
- **Ein-Zeilen-Installation** -- `curl -fsSL https://raw.githubusercontent.com/LupusMalusDeviant/VARG/main/install.sh | bash` (Linux) / `irm https://raw.githubusercontent.com/LupusMalusDeviant/VARG/main/install.ps1 | iex` (Windows)

---

## OCAP-Sicherheitsmodell

Jede privilegierte Operation erfordert ein Capability-Token als Methodenparameter.
Tokens koennen nur innerhalb von `unsafe`-Bloecken erzeugt werden -- der Compiler erzwingt dies zur Compile-Zeit.

```csharp
agent SecureAgent {
    // Deklariert: Diese Methode braucht Dateisystem-Zugriff
    public string ReadConfig(string path, FileAccess cap) {
        return fs_read(path)?;
    }

    public void Run() {
        // Aufrufer muss die Capability explizit gewaehren
        unsafe {
            var cap = FileAccess {};
            var config = self.ReadConfig("config.toml", cap);
            print config;
        }
    }
}
```

**5 Capability-Typen:**

| Capability | Schuetzt |
|------------|----------|
| `FileAccess` | Dateisystem-Lesen/Schreiben/Anhaengen |
| `NetworkAccess` | HTTP-Anfragen, Fetch |
| `DbAccess` | Datenbank-Abfragen (SQLite) |
| `LlmAccess` | LLM-Provider-Aufrufe |
| `SystemAccess` | Shell-Kommando-Ausfuehrung |
**Reichweite der Zusicherung.** OCAP ist ein Compile-Zeit-Gate, keine Laufzeit-Sandbox. Es
entscheidet, *welche* Programmteile nach draussen wirken duerfen, und zwingt das ins Typsystem --
ein Programm laesst sich also durch Lesen seiner Signaturen pruefen. Es prueft nicht, *was* diese
Aufrufe hinausschicken: ein Token autorisiert den Aufruf, nie die Argumente. Speziell `exec`
reicht seinen String an eine Shell weiter; nicht vertrauenswuerdige Eingaben gehoeren stattdessen
in `proc_spawn_args`. Siehe REFERENCE.md, "What OCAP does and does not do".


---

## Erste Schritte

### Ein-Zeilen-Installation

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/LupusMalusDeviant/VARG/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/LupusMalusDeviant/VARG/main/install.ps1 | iex
```

### Einfache Installation (Vorkompiliertes Binary)

Alternativ kann das vorkompilierte Binary manuell heruntergeladen werden:

1. Gehe zur [Releases](../../releases)-Seite.
2. Lade herunter:
   - Linux:   `varg-v2.2.0-linux-x64.tar.gz`
   - Windows: `varg-v2.2.0-windows-x64.zip`
3. Entpacke `vargc` (Linux) bzw. `vargc.exe` (Windows) und lege die Datei irgendwo in deinen System-`PATH` ab.
4. Fertig! Los geht's.
---

### Aus dem Quellcode bauen

#### Voraussetzungen

- [Rust](https://rustup.rs/) (1.75+)

### Compiler bauen

```bash
cd varg-compiler
cargo build --release
```

Die Compiler-Binary liegt dann unter `target/release/vargc`.

### Kompilieren & Ausfuehren

```bash
# .varg-Datei zu Native Binary kompilieren
vargc build hello.varg

# Kompilieren und sofort ausfuehren
vargc run hello.varg

# Generierten Rust-Code ausgeben (zur Inspektion)
vargc emit-rs hello.varg

# Tests mit @[Test]-Annotation ausfuehren
vargc test my_tests.varg

# Watch-Modus (bei Datei-Aenderung neu kompilieren)
vargc watch hello.varg
```

### Hello World

```csharp
// hello.varg
agent Hello {
    public void Run() {
        print "Hallo aus Varg!";
    }
}
```

```bash
vargc run hello.varg
# --> Hallo aus Varg!
```

---

## Beispiele

Siehe das [`examples/`](examples/)-Verzeichnis:

| Datei | Was es zeigt |
|-------|--------------|
| [`hello.varg`](examples/hello.varg) | Minimales Hello World |
| [`file_processor.varg`](examples/file_processor.varg) | Datei-I/O mit OCAP-Sicherheit, try/catch, Verzeichnis-Scan |
| [`api_client.varg`](examples/api_client.varg) | HTTP-Anfragen mit Retry/Fallback und JSON-Parsing |
| [`data_pipeline.varg`](examples/data_pipeline.varg) | Iteratoren, Enums, Maps, Sets, Pattern Matching |
| [`chat_agent.varg`](examples/chat_agent.varg) | Multi-Agent-System mit spawn, send, on_message |
| [`knowledge_graph.varg`](examples/knowledge_graph.varg) | Graph-Knoten, Kanten, Traversal, Queries |
| [`vector_store.varg`](examples/vector_store.varg) | Text-Embedding, Vector-Upsert, Similarity-Suche |
| [`agent_memory.varg`](examples/agent_memory.varg) | 3-Schichten-Memory: Working, Episodic, Semantic |
| [`tracing.varg`](examples/tracing.varg) | Span-basiertes Tracing mit Events und JSON-Export |
| [`claw_lite.varg`](examples/claw_lite.varg) | REPL-artiger CLI-Agent mit doctor/colors/inspect/exec Subkommandos |
| [`wave29_bytes.varg`](examples/wave29_bytes.varg) | Binaere Datei-I/O: lesen, schreiben, anhaengen, Groesse |

---

## Kompilierungs-Pipeline

```
  .varg Quellcode
      |
  [1] Lexer (Logos)         -- Tokenisierung in 119 Token-Typen
      |
  [2] Parser                -- Recursive Descent -> typisierter AST
      |
  [3] TypeChecker           -- Semantische Analyse, Typinferenz, OCAP-Validierung
      |
  [4] CodeGen               -- AST -> Rust-Quellcode
      |
  [5] cargo build           -- Rust -> Native Binary
```

---

## Testsuite

Jede Crate ist getestet, und die Suite laeuft bei jedem Push:

```bash
cd varg-compiler
cargo test
```

| Crate | Tests | Abdeckung |
|-------|------:|-----------|
| varg-ast | 4 | AST-Konstruktion, gemeinsame Builtin-Tabelle |
| varg-lexer | 29 | Alle Token-Typen, Randfaelle |
| varg-parser | 224 | Jede Statement/Expression-Variante; adversarielle Randfaelle |
| varg-typechecker | 367 | Typinferenz, OCAP, DI, alle Builtins |
| varg-codegen | 303 | Rust-Generierung, alle Runtime-Module |
| varg-runtime | 287 | Echtes HTTP/SQLite/WS/MCP + die reinen Rust-Module |
| varg-lsp | 23 | Diagnosen, Hover, Completion, Gehe-zu-Definition |
| vargc | 27 | Vorlagen, Source-Maps, die CLI |
| **Gesamt** | **1.264** | **0 Fehler** |

Mit `--features full` kommen die feature-gegateten Runtime-Module dazu: **1.421**.

---

## Projektstruktur

```
Project X/
  README.md               Englische Version
  README_DE.md            Diese Datei (Deutsch)
  REFERENCE.md            Vollstaendige Sprachreferenz
  VARG_AGENT_GUIDE.md     Anleitung/Prompt fuer KI-Agenten
  docs/                   Bilder und Assets
  examples/               11 Beispielprogramme
  varg-compiler/          Rust Workspace (10 Crates)
  varg-vscode/            VS Code Extension (Syntax Highlighting)
```

---

## Status

Varg wird aktiv entwickelt. Der Compiler ist funktionsfaehig und erzeugt lauffaehige native Binaries.
**Aktuelles Release: v2.2.0** -- 1.264 Tests, 50 Golden-Programme, die gebaut **und ausgefuehrt**
werden, 88 Ablehnungsproben, und jedes dokumentierte Builtin wird von einem davon ausgefuehrt.

Die Sprache eignet sich fuer den Bau von echten Agenten, CLI-Tools, API-Clients, Web-Servern,
Knowledge-Graph-gestuetzten RAG-Systemen, Multi-Agent-Orchestration-Pipelines und REPL-getriebenen
Agent-Frontends (wie ein Claude-Code-artiges Terminal-UI).

---

## Lizenz

MIT
