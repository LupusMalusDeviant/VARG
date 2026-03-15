# F41 — VARG Upgrade-Plan für Egregor-Portierung

> Ziel: VARG so erweitern, dass Egregor vollständig in VARG geschrieben werden kann.
> Jede Phase baut auf der vorherigen auf. Phasen innerhalb einer Priorität sind parallelisierbar.

---

## Übersicht

```
Phase 1  Crate-Import / FFI           ← Enabler für ALLES
Phase 2  HTTP-Server Runtime           ← Gateway
Phase 3  Datenbank-Treiber             ← AKG + Memory
Phase 4  WebSocket / SSE               ← Streaming + Web-UI
Phase 5  Erweiterte Fehlerbehandlung   ← Error-Hierarchie
Phase 6  Dependency Injection          ← Interface-First Architektur
Phase 7  Test-Infrastruktur            ← Mocking, Coverage
Phase 8  Telegram Bot Runtime          ← Kanal
Phase 9  Matrix-Protokoll              ← Kanal
Phase 10 Docker-API                    ← Multi-Agent
Phase 11 MCP-Protokoll                 ← Tool-Interop
```

---

## Phase 1 — Crate-Import / FFI 🔴 KRITISCH

**Status:** Fehlt komplett
**Begründung:** Ohne externen Crate-Import ist jede weitere Phase ein Eigenentwicklungs-Mammut. Mit Crate-Import lösen sich Phase 2–4 fast automatisch.

### Was implementiert werden muss

```
1.1  `use extern` Syntax für Rust-Crate-Imports
     → Compiler generiert passende `Cargo.toml` Dependencies
     → Beispiel: `use extern tokio;` oder `use extern "axum" version "0.8";`

1.2  FFI-Bindings für Rust-Typen
     → VARG-Typen ↔ Rust-Typen Mapping erweitern
     → Trait-Implementierungen aus Crates nutzbar machen

1.3  Codegen-Erweiterung
     → Generiertes Cargo.toml enthält deklarierte Dependencies
     → `extern` Funktionen werden als direkte Rust-Calls emittiert

1.4  TypeChecker-Erweiterung
     → Externe Typen als opaque Types behandeln
     → Methoden-Aufrufe auf externen Typen durchlassen (duck-typing oder Manifest)
```

### Akzeptanzkriterien

```varg
use extern "axum" version "0.8";
use extern "serde_json" version "1.0";

agent MyServer {
    public async void Run(NetworkAccess cap) {
        // Extern-Crate direkt nutzbar
        let app = axum::Router::new()
            .route("/health", axum::get(health_handler));
        axum::serve(app, "0.0.0.0:3000").await;
    }
}
```

### Risiken
- TypeChecker kann externe Typen nicht vollständig validieren
- Lösung: Opaque-Type-System + optionale `.d.varg` Deklarationsdateien

---

## Phase 2 — HTTP-Server Runtime 🔴 KRITISCH

**Status:** Fehlt (nur Client-seitig vorhanden)
**Abhängigkeit:** Phase 1 (wenn Crate-Import) ODER eigenständig als Runtime-Modul
**Egregor braucht:** REST-API Gateway, Webhook-Empfang, SSE-Endpoint

### Was implementiert werden muss

```
2.1  Server-Builtin ODER Crate-Bridge
     Option A (mit Phase 1): `use extern "axum"` → direkt nutzen
     Option B (ohne Phase 1): Neues Runtime-Modul `varg-runtime/server.rs`
       → http_serve(port, routes)
       → http_route(method, path, handler)
       → http_response(status, body, headers)

2.2  Request/Response-Typen
     → HttpRequest { method, path, headers, body, query_params }
     → HttpResponse { status, headers, body }

2.3  Middleware-Konzept
     → Vor/Nach Handler-Ausführung
     → Auth-Middleware, Logging-Middleware, CORS

2.4  Route-Parameter
     → Pfad-Parameter: /api/users/{id}
     → Query-Parameter: ?page=1&limit=10
```

### Akzeptanzkriterien

```varg
agent Gateway {
    public async void Run(NetworkAccess cap) {
        let server = http_serve(cap, 8080);
        server.route("GET", "/api/health", handle_health);
        server.route("POST", "/api/chat", handle_chat);
        server.start().await;
    }

    fn handle_health(req: HttpRequest) -> HttpResponse {
        return HttpResponse(200, json_stringify({ "status": "ok" }));
    }
}
```

---

## Phase 3 — Datenbank-Treiber 🔴 KRITISCH

**Status:** `DbAccess` Capability existiert, keine Implementierung
**Abhängigkeit:** Phase 1 (ideal) oder eigenes Runtime-Modul
**Egregor braucht:** Neo4j (AKG/Knowledge Graph), SQLite (Memory/STM/Conversations)

### Was implementiert werden muss

```
3.1  SQLite-Treiber
     → db_open(cap, "path/to/db.sqlite") → DbConnection
     → db_execute(conn, sql, params) → int (affected rows)
     → db_query(conn, sql, params) → List<Map<string, string>>
     → db_transaction(conn, callback) → Result

3.2  Neo4j-Treiber (Bolt-Protokoll)
     → neo4j_connect(cap, uri, user, password) → Neo4jSession
     → neo4j_run(session, cypher, params) → List<Map<string, any>>
     → neo4j_transaction(session, callback) → Result

3.3  Connection-Pooling
     → Wiederverwendbare Verbindungen
     → Max-Pool-Size konfigurierbar

3.4  Prepared Statements / Parameter-Binding
     → SQL-Injection-Schutz by design
     → Typ-sichere Parameter
```

### Akzeptanzkriterien

```varg
agent MemoryStore {
    let db: DbConnection;

    on_start(DbAccess cap) {
        db = db_open(cap, "data/memory.sqlite");
        db_execute(db, "CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY, user_id TEXT, message TEXT, timestamp INTEGER
        )", []);
    }

    public async List<Map<string, string>> GetHistory(string userId, int limit) {
        return db_query(db,
            "SELECT * FROM conversations WHERE user_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
            [userId, limit.to_string()]
        );
    }
}
```

```varg
agent KnowledgeGraph {
    let session: Neo4jSession;

    on_start(DbAccess cap) {
        session = neo4j_connect(cap, "bolt://localhost:7687", "neo4j", env_var("NEO4J_PASSWORD"));
    }

    public async List<Map<string, any>> QueryRules(string domain) {
        return neo4j_run(session,
            "MATCH (r:KnowledgeRule)-[:BELONGS_TO]->(d:Domain {name: $domain}) RETURN r",
            { "domain": domain }
        );
    }
}
```

---

## Phase 4 — WebSocket / SSE 🔴 KRITISCH

**Status:** Fehlt komplett
**Abhängigkeit:** Phase 2 (HTTP-Server)
**Egregor braucht:** Blazor (SignalR/WebSocket), Streaming (SSE), Echtzeit-Events

### Was implementiert werden muss

```
4.1  WebSocket-Server
     → ws_upgrade(request) → WebSocket
     → ws_send(socket, message)
     → ws_receive(socket) → string
     → ws_on_message(socket, handler)
     → ws_close(socket)

4.2  WebSocket-Client
     → ws_connect(cap, url) → WebSocket
     → Gleiche API wie Server-seitig

4.3  Server-Sent Events (SSE)
     → sse_stream(response) → SseWriter
     → sse_send(writer, event, data)
     → sse_close(writer)

4.4  Event-Bus Pattern
     → Publish/Subscribe für interne Events
     → Bridge zu WebSocket/SSE für externe Clients
```

### Akzeptanzkriterien

```varg
agent ChatHub {
    let clients: Map<string, WebSocket> = {};

    public async void HandleUpgrade(req: HttpRequest) {
        let socket = ws_upgrade(req);
        let userId = req.headers.get("X-User-Id");
        clients.set(userId, socket);

        ws_on_message(socket, fn(msg: string) {
            // Agent-Pipeline triggern, Antwort streamen
            let response = process_message(userId, msg).await;
            ws_send(socket, response);
        });
    }
}
```

---

## Phase 5 — Erweiterte Fehlerbehandlung 🟡 HOCH

**Status:** Nur `Result<T, String>`
**Egregor braucht:** Typisierte Fehler-Hierarchie (AgentException, AkgException, ToolException...)

### Was implementiert werden muss

```
5.1  Error-Enums als Result-Typen
     → Result<T, ErrorEnum> statt Result<T, String>
     → Pattern-Matching auf Fehler-Varianten

5.2  Error-Propagation mit Kontext
     → `?` Operator mit Error-Mapping
     → `.map_err(fn(e) => ...)` Chain

5.3  Error-Traits / Contracts
     → contract Error { fn message() -> string; fn code() -> int; }
     → Eigene Error-Typen implementieren Error

5.4  Stack-Traces (Optional)
     → Fehler-Ursprung nachvollziehbar
     → Debug-Modus mit Zeilen-Info
```

### Akzeptanzkriterien

```varg
enum AgentError {
    ToolFailed { tool_name: string, reason: string },
    AkgCompilationFailed { rule_count: int, conflicts: int },
    LlmTimeout { provider: string, latency_ms: int },
    Unauthorized { user_id: string }
}

agent Runtime {
    public Result<string, AgentError> ProcessMessage(string input) {
        let context = compile_akg(input)
            .map_err(fn(e) => AgentError::AkgCompilationFailed {
                rule_count: e.rules, conflicts: e.conflicts
            })?;

        let result = call_llm(context)
            .map_err(fn(e) => AgentError::LlmTimeout {
                provider: e.provider, latency_ms: e.ms
            })?;

        return Ok(result);
    }
}
```

---

## Phase 6 — Dependency Injection 🟡 HOCH

**Status:** Keine DI-Unterstützung
**Egregor braucht:** Interface-First Architektur, austauschbare Implementierungen, Testbarkeit

### Was implementiert werden muss

```
6.1  Contract-basierte Injection
     → Agents erhalten Dependencies über Constructor oder on_start()
     → Container registriert Contract → Implementierung

6.2  Service-Container
     → register<IModelClient>(OllamaClient)
     → resolve<IModelClient>() → Instanz
     → Lifecycle: Singleton, Scoped, Transient

6.3  Auto-Injection über Annotationen
     → @[Inject] let client: IModelClient;
     → Container löst beim Spawn automatisch auf

6.4  Scoped Services
     → Pro-Request Scope (für User-Scoping)
     → Scope-Factory Pattern
```

### Akzeptanzkriterien

```varg
contract IModelClient {
    async fn infer(prompt: string) -> Result<string, string>;
}

contract IKnowledgeGraph {
    async fn query(cypher: string) -> Result<List<Map<string, any>>, string>;
}

agent AgentRuntime {
    @[Inject] let llm: IModelClient;
    @[Inject] let akg: IKnowledgeGraph;

    public async Result<string, AgentError> Process(string input) {
        let context = akg.query($"MATCH (r:Rule) WHERE ... RETURN r").await?;
        let response = llm.infer($"{context}\n{input}").await?;
        return Ok(response);
    }
}

// Registrierung
let container = ServiceContainer::new();
container.register<IModelClient>(OllamaClient::new());
container.register<IKnowledgeGraph>(Neo4jKnowledgeGraph::new());
let runtime = container.resolve<AgentRuntime>();
```

---

## Phase 7 — Test-Infrastruktur 🟡 HOCH

**Status:** Nur `@[Test]` Annotation, kein Mocking
**Egregor braucht:** 1200+ Unit-Tests mit Mocks, Coverage-Tracking

### Was implementiert werden muss

```
7.1  Mock-Framework
     → mock<IModelClient>() → MockModelClient
     → mock.when("infer").returns("test response")
     → mock.verify("infer").called_times(1)

7.2  Assertion-Library erweitern
     → assert_eq, assert_ne, assert_true, assert_false (existieren teilweise)
     → assert_throws<ErrorType>(fn() => ...)
     → assert_contains, assert_starts_with

7.3  Test-Fixtures / Setup-Teardown
     → @[BeforeEach] fn setup() { ... }
     → @[AfterEach] fn teardown() { ... }
     → @[BeforeAll] / @[AfterAll]

7.4  Code-Coverage
     → vargc test --coverage
     → Zeilen- und Branch-Coverage
     → Threshold-Konfiguration

7.5  Test-Doubles für Builtins
     → Fake-TimeProvider
     → Fake-FileSystem (In-Memory)
     → Fake-HttpClient (Captured Requests)
```

### Akzeptanzkriterien

```varg
@[Test]
fn ProcessMessage_ValidInput_ReturnsResponse() {
    let mockLlm = mock<IModelClient>();
    mockLlm.when("infer").returns(Ok("Hello!"));

    let mockAkg = mock<IKnowledgeGraph>();
    mockAkg.when("query").returns(Ok([]));

    let runtime = AgentRuntime(llm: mockLlm, akg: mockAkg);
    let result = runtime.Process("Hi").await;

    assert_eq(result.unwrap(), "Hello!");
    mockLlm.verify("infer").called_times(1);
}
```

---

## Phase 8 — Telegram Bot Runtime 🟡 MITTEL

**Status:** HTTP-Client vorhanden, kein Bot-Framework
**Abhängigkeit:** Phase 2 (HTTP-Server für Webhooks), Phase 1 (Crate-Import für `teloxide`)

### Was implementiert werden muss

```
8.1  Telegram Bot API Client
     → telegram_bot(token) → TelegramBot
     → bot.get_updates(offset, timeout) → List<Update>
     → bot.send_message(chat_id, text, options)
     → bot.send_photo(chat_id, photo, caption)
     → bot.answer_callback_query(callback_id, text)

8.2  Long-Polling Loop
     → Automatisches Polling mit Offset-Tracking
     → Reconnect bei Fehler
     → Graceful Shutdown

8.3  Webhook-Modus (Alternative)
     → Benötigt Phase 2 (HTTP-Server)
     → bot.set_webhook(url)
     → Eingehende Updates als HTTP-POST

8.4  Inline-Keyboards / Reply-Markup
     → Button-Layouts
     → Callback-Handling

8.5  File-Downloads
     → bot.get_file(file_id) → bytes
     → Foto/Dokument-Verarbeitung
```

---

## Phase 9 — Matrix-Protokoll 🟢 MITTEL

**Status:** Fehlt
**Abhängigkeit:** Phase 2 (HTTP-Server), Phase 4 (WebSocket)

### Was implementiert werden muss

```
9.1  Matrix Client-Server API
     → matrix_login(homeserver, user, password) → MatrixSession
     → matrix_sync(session) → SyncResponse
     → matrix_send(session, room_id, message)

9.2  Application Service Bridge
     → AS-Registration
     → Event-Handling

9.3  End-to-End Encryption (Optional)
     → Olm/Megolm via Crate-Import
```

---

## Phase 10 — Docker-API 🟢 MITTEL

**Status:** `shell_execute("docker", ...)` als Workaround möglich
**Egregor braucht:** Container-Orchestrierung für Multi-Agent (Clones)

### Was implementiert werden muss

```
10.1  Docker Engine API Client (HTTP über Unix-Socket)
      → docker_create_container(image, config) → ContainerId
      → docker_start(id), docker_stop(id), docker_remove(id)
      → docker_logs(id) → Stream<string>
      → docker_exec(id, command) → ExecResult

10.2  Alternativ: Wrapper über CLI
      → Geringerer Aufwand, funktioniert sofort
      → Weniger Kontrolle über Lifecycle
```

---

## Phase 11 — MCP-Protokoll 🟢 MITTEL

**Status:** Fehlt
**Abhängigkeit:** Phase 2 (HTTP-Server für SSE-Transport)

### Was implementiert werden muss

```
11.1  MCP-Server (VARG-Tools nach außen exponieren)
      → JSON-RPC über stdio oder SSE
      → Tool-Discovery, Tool-Execution
      → @[McpTool] Annotation → automatische Registrierung

11.2  MCP-Client (externe Tools einbinden)
      → Server-Prozess starten
      → Tool-Liste abrufen
      → Tool-Calls dispatchen
```

---

## Empfohlene Reihenfolge

```
                    ┌─────────────────────┐
                    │  Phase 1: FFI/Crate │ ← ALLES hängt davon ab
                    └────────┬────────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
     ┌────────▼───┐  ┌──────▼─────┐  ┌─────▼──────┐
     │ Phase 2:   │  │ Phase 3:   │  │ Phase 5:   │
     │ HTTP-Server│  │ DB-Treiber │  │ Error-Typen│
     └────────┬───┘  └──────┬─────┘  └─────┬──────┘
              │              │              │
     ┌────────▼───┐          │        ┌─────▼──────┐
     │ Phase 4:   │          │        │ Phase 6:   │
     │ WS / SSE   │          │        │ DI-System  │
     └────────┬───┘          │        └─────┬──────┘
              │              │              │
              └──────────────┼──────────────┘
                             │
                    ┌────────▼────────┐
                    │ Phase 7: Tests  │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
     ┌────────▼───┐  ┌──────▼─────┐  ┌─────▼──────┐
     │ Phase 8:   │  │ Phase 9:   │  │ Phase 10:  │
     │ Telegram   │  │ Matrix     │  │ Docker-API │
     └────────────┘  └────────────┘  └─────┬──────┘
                                           │
                                    ┌──────▼─────┐
                                    │ Phase 11:  │
                                    │ MCP        │
                                    └────────────┘
```

---

## Aufwandsschätzung

| Phase | Aufwand | Kommentar |
|-------|---------|-----------|
| 1. Crate-Import / FFI | 🔴 Groß (2–3 Wochen) | Compiler-Kern: Codegen + TypeChecker + Cargo.toml-Generation |
| 2. HTTP-Server | 🟡 Mittel (3–5 Tage) | Mit Phase 1: nur Builtins um axum wrappen |
| 3. DB-Treiber | 🟡 Mittel (3–5 Tage) | Mit Phase 1: neo4rs + rusqlite wrappen |
| 4. WebSocket / SSE | 🟡 Mittel (3–5 Tage) | Mit Phase 1: tokio-tungstenite wrappen |
| 5. Error-Typen | 🟢 Klein (2–3 Tage) | TypeChecker + CodeGen erweitern |
| 6. DI-System | 🟡 Mittel (4–5 Tage) | Neues Sprachkonzept, Container-Runtime |
| 7. Test-Infrastruktur | 🟡 Mittel (5–7 Tage) | Mock-Framework ist der größte Brocken |
| 8. Telegram | 🟢 Klein (2–3 Tage) | API-Wrapper, mit Phase 1 trivial |
| 9. Matrix | 🟡 Mittel (4–5 Tage) | Komplexes Protokoll |
| 10. Docker | 🟢 Klein (2–3 Tage) | CLI-Wrapper oder HTTP-API |
| 11. MCP | 🟢 Klein (2–3 Tage) | JSON-RPC, klar spezifiziert |
| **Gesamt** | **~5–8 Wochen** | Bei Vollzeit-Entwicklung |

---

## Empfehlung

**Phase 1 (Crate-Import) zuerst.** Sie ist der größte Einzelaufwand, aber reduziert den
Gesamtaufwand aller folgenden Phasen massiv. Ohne sie muss jeder Treiber, jeder Server,
jede Protokoll-Implementierung from scratch in VARG geschrieben werden.

Mit Crate-Import wird VARG nicht nur Egregor-tauglich, sondern ein echtes
General-Purpose-Ökosystem für AI-Agenten — mit dem gesamten Rust-Ecosystem im Rücken.
