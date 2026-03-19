# VARG Upgrade-Plan — Egregor-Portierung

> Stand: 15. März 2026 | Basiert auf Wave 17 (Commit `6ebd384`)
> Ziel: VARG so erweitern, dass Egregor vollständig in VARG geschrieben werden kann.

---

## Aktueller Stand nach Wave 17

### ✅ Bereits vorhanden und funktional

| Feature | Details |
|---------|---------|
| LLM Multi-Provider | Ollama, OpenAI, Anthropic — Streaming + Multi-Turn |
| Actor-Model | `spawn`, `send`, `request` — tokio-basiert |
| HTTP-Client | GET/POST/PUT/DELETE/PATCH, Retry-Patterns, Streaming |
| SQLite-Treiber | Connection-Pooling, Parameter-Binding, SQL-Injection-Schutz |
| File I/O + OCAP | fs_read/write/append/read_lines/read_dir mit Capability-Tokens |
| JSON komplett | json_parse, json_get (typisiert: int/bool/array), json_stringify |
| Shell-Execution | exec, exec_status |
| Async/Await | Tokio-Runtime, auto-detection async methods |
| Contracts (Interfaces) | `agent Foo : Contract1, Contract2` — compile-time enforced |
| Module-System | import-Syntax, selektiv/komplett |
| Generics | Type-Substitution, Struct-Field-Access |
| Logging | log_debug/info/warn/error |
| Regex | regex_match, regex_find_all, regex_replace |
| Date/Time | time_millis, time_format, time_parse, time_add, time_diff |
| Embeddings/Vectors | embedding_create, vector_similarity |
| SSE-Client | Line-by-line Streaming-Parsing |
| 682 Tests | 0 Failures |

### ⚠️ Teilweise vorhanden (MVP / API-Surface)

| Feature | Status | Was fehlt |
|---------|--------|-----------|
| HTTP-Server (Axum) | Framework da | Endpoint-Pattern, Middleware, Static Files |
| WebSocket-Client | 4 Funktionen definiert | tokio-tungstenite Integration |
| MCP-Protokoll | API-Surface (list/call) | JSON-RPC Transmission, stdio-Transport |
| Crypto | Stubs vorhanden | Echte AES-GCM, SHA-256, HMAC Implementierung |

### ❌ Fehlt komplett

| Feature | Egregor-Relevanz |
|---------|-----------------|
| Crate-Import / FFI | Kritisch — Zugang zum Rust-Ecosystem |
| Neo4j-Treiber | Kritisch — AKG Knowledge Graph |
| WebSocket-Server | Hoch — Blazor/SignalR-Ersatz |
| SSE-Server | Hoch — Event-Streaming Gateway |
| Telegram Bot SDK | Hoch — Kanal-Integration |
| Matrix-Protokoll | Mittel — Kanal-Integration |
| Docker-API | Mittel — Container-Orchestrierung |
| Dependency Injection | Hoch — Service-Komposition |
| Typisierte Fehler | Hoch — Exception-Hierarchie |
| Test-Mocking | Hoch — Unit-Test-Isolation |

---

## Upgrade-Phasen

### Phase 1: Crate-Import / FFI (Kritischer Pfad)
**Priorität:** 🔴 Blocker für alles andere
**Geschätzter Aufwand:** 2–3 Wochen

**Warum zuerst:** Ohne Crate-Import muss jeder Treiber (Neo4j, WebSocket-Server, etc.)
von Grund auf in VARG geschrieben werden. Mit Crate-Import nutzt man das Rust-Ecosystem.

**Was implementiert werden muss:**

1. **Neue Syntax: `use extern`**
   ```varg
   use extern "neo4rs" version "0.7";
   use extern "axum" version "0.8";
   use extern "tokio-tungstenite" version "0.24";
   ```

2. **Compiler: Cargo.toml-Generierung**
   - CodeGen muss `[dependencies]`-Sektion in generierter Cargo.toml erzeugen
   - Version-Resolution und Feature-Flags unterstützen

3. **TypeChecker: Extern-Typ-Deklarationen**
   ```varg
   extern struct Neo4jClient {
       fn execute(query: string, params: Map<string, string>) -> Result<string>
   }
   ```
   - Extern-Typen werden nicht validiert, sondern als "trusted" durchgereicht
   - CodeGen erzeugt `use neo4rs::*;` im Rust-Output

4. **OCAP-Integration**
   - Extern-Crates die I/O machen brauchen entsprechende Capability-Tokens
   - `DbAccess` für Datenbank-Crates
   - `NetworkAccess` für HTTP/WebSocket-Crates

**Akzeptanzkriterien:**
- [ ] `use extern` parst ohne Fehler
- [ ] Generierte Cargo.toml enthält korrekte Dependencies
- [ ] Extern-Typen können in Varg-Code verwendet werden
- [ ] OCAP wird für Extern-Calls enforced
- [ ] Mindestens 1 Beispiel: SQLite via `rusqlite` statt Built-in

---

### Phase 2: HTTP-Server Runtime vervollständigen
**Priorität:** 🔴 Kritisch
**Geschätzter Aufwand:** 3–5 Tage
**Abhängigkeit:** Phase 1 (für Axum-Crate) ODER Ausbau des bestehenden Server-Moduls

**Was implementiert werden muss:**

1. **Deklaratives Routing**
   ```varg
   @[Route("GET", "/api/health")]
   method Health(req: Request) -> Response {
       return Response.json(200, '{"status": "ok"}')
   }

   @[Route("POST", "/api/messages")]
   method HandleMessage(req: Request) -> Response {
       let body = req.json()?
       // ...
       return Response.json(201, result)
   }
   ```

2. **Server-Lifecycle**
   ```varg
   agent Gateway {
       on_start() {
           server_listen(8080)
       }
   }
   ```

3. **Middleware-Chain**
   - CORS-Middleware
   - Auth-Middleware (JWT-Validierung)
   - Logging-Middleware (Request/Response)
   - Rate-Limiting

4. **SSE-Server-Endpoints**
   ```varg
   @[Route("GET", "/api/events")]
   method Events(req: Request) -> SseStream {
       return sse_stream(event_channel)
   }
   ```

5. **Static File Serving** (für Web-UI)

**Akzeptanzkriterien:**
- [ ] HTTP-Server startet und akzeptiert Requests
- [ ] Routing via Annotationen funktioniert
- [ ] JSON Request/Response-Handling
- [ ] SSE-Endpoint sendet Events an verbundene Clients
- [ ] Mindestens CORS + Logging Middleware

---

### Phase 3: Datenbank-Treiber (Neo4j + SQLite-Ausbau)
**Priorität:** 🔴 Kritisch
**Geschätzter Aufwand:** 3–5 Tage
**Abhängigkeit:** Phase 1 (für neo4rs-Crate)

**Was implementiert werden muss:**

1. **Neo4j-Treiber**
   ```varg
   let graph = neo4j_connect(db_cap, "bolt://localhost:7687", "neo4j", "password")
   let result = neo4j_query(graph, "MATCH (n:Rule) RETURN n.content", {})
   let nodes = neo4j_query_typed<Rule>(graph, cypher, params)
   ```

2. **SQLite-Ausbau (Built-in erweitern)**
   - Migrations-Support
   - Transaction-Handling (`begin`, `commit`, `rollback`)
   - Prepared Statements mit Typ-Mapping
   - Connection-Pool Konfiguration

3. **Generische DB-Abstraktion**
   ```varg
   contract DatabaseDriver {
       method Connect(cap: DbAccess, connection_string: string) -> DbConnection
       method Query(conn: DbConnection, sql: string, params: Map<string, string>) -> QueryResult
       method Execute(conn: DbConnection, sql: string, params: Map<string, string>) -> int
       method Transaction(conn: DbConnection) -> DbTransaction
   }
   ```

**Akzeptanzkriterien:**
- [ ] Neo4j: Connect, Query (Cypher), parametrisierte Queries
- [ ] SQLite: Transactions, Migrations, Prepared Statements
- [ ] Generisches `DatabaseDriver`-Contract für beide

---

### Phase 4: WebSocket-Server + SSE-Server
**Priorität:** 🟡 Hoch
**Geschätzter Aufwand:** 3–5 Tage
**Abhängigkeit:** Phase 2 (HTTP-Server)

**Was implementiert werden muss:**

1. **WebSocket-Server**
   ```varg
   @[WebSocket("/ws/chat")]
   method ChatSocket(ws: WebSocketConnection) {
       on_message(ws) { msg ->
           let response = process(msg)
           ws_send(ws, response)
       }
   }
   ```

2. **WebSocket-Client vervollständigen**
   - tokio-tungstenite Integration (aktuell nur API-Surface)
   - Reconnect-Logic
   - Ping/Pong Heartbeat

3. **SSE-Server-Push**
   - Event-Bus Pattern (publish/subscribe)
   - Client-Tracking (connect/disconnect)
   - Ring-Buffer für Event-History

**Akzeptanzkriterien:**
- [ ] WebSocket-Server akzeptiert Verbindungen
- [ ] Bidirektionale Kommunikation funktioniert
- [ ] SSE-Server pusht Events an mehrere Clients
- [ ] WebSocket-Client reconnected automatisch

---

### Phase 5: Typisierte Fehlerbehandlung
**Priorität:** 🟡 Hoch
**Geschätzter Aufwand:** 2–3 Tage
**Abhängigkeit:** Keine

**Problem:** `Result<T, String>` verliert Fehler-Kontext.
Egregor braucht: `AgentException`, `AkgException`, `ToolException`, `ChannelException`.

**Was implementiert werden muss:**

1. **Error-Enums mit Daten**
   ```varg
   enum AgentError {
       ToolFailed { tool_name: string, message: string },
       AkgConflict { rule_ids: List<string> },
       LlmTimeout { provider: string, latency_ms: int },
       ChannelError { channel: string, reason: string }
   }
   ```

2. **Result mit typisierten Errors**
   ```varg
   method Execute(ctx: Context) -> Result<Response, AgentError> {
       let rules = compile_akg(ctx)? // propagiert AgentError
       // ...
   }
   ```

3. **Error-Konvertierung**
   ```varg
   // Automatische Konvertierung String → ErrorType
   method Wrap() -> Result<int, AgentError> {
       let data = fs_read(cap, "file.txt")
           or_error AgentError.ToolFailed { tool_name: "fs_read", message: err }
   }
   ```

4. **Match auf Errors**
   ```varg
   match result {
       Ok(value) => process(value),
       Err(AgentError.ToolFailed { tool_name, message }) => log_error(message),
       Err(AgentError.LlmTimeout { provider, .. }) => retry(provider),
       Err(_) => fallback()
   }
   ```

**Akzeptanzkriterien:**
- [ ] Error-Enums mit Feldern definierbar
- [ ] `Result<T, MyError>` funktioniert end-to-end
- [ ] `?`-Operator propagiert typisierte Errors
- [ ] Pattern-Matching auf Error-Varianten

---

### Phase 6: Dependency Injection
**Priorität:** 🟡 Hoch
**Geschätzter Aufwand:** 4–5 Tage
**Abhängigkeit:** Phase 5 (Contracts müssen solide sein)

**Was implementiert werden muss:**

1. **Service-Container**
   ```varg
   let container = Container.new()
   container.singleton<IModelClient>(OllamaClient { endpoint: "..." })
   container.scoped<IMemoryStore>(SqliteMemoryStore { db: db_conn })
   container.transient<IInputSanitizer>(InputSanitizer {})
   ```

2. **Constructor-Injection via Contracts**
   ```varg
   agent AgentRuntime {
       inject model: IModelClient
       inject memory: IMemoryStore
       inject tools: IToolRegistry

       method Run(message: string) -> string {
           let ctx = memory.load(user_id)?
           let response = model.infer(ctx, message)?
           return response
       }
   }
   ```

3. **Scope-Lifecycle**
   - `singleton` — eine Instanz pro Container
   - `scoped` — eine Instanz pro Request/Operation
   - `transient` — neue Instanz bei jedem Resolve

4. **Auto-Resolve**
   ```varg
   let runtime = container.resolve<AgentRuntime>()
   // Injiziert automatisch alle `inject`-Felder
   ```

**Akzeptanzkriterien:**
- [ ] Container registriert und resolved Services
- [ ] Singleton/Scoped/Transient Lifecycle korrekt
- [ ] `inject`-Keyword in Agents funktioniert
- [ ] Zirkuläre Dependencies werden erkannt (Compile-Error)

---

### Phase 7: Test-Infrastruktur (Mocking + Coverage)
**Priorität:** 🟡 Hoch
**Geschätzter Aufwand:** 5–7 Tage
**Abhängigkeit:** Phase 5, Phase 6

**Was implementiert werden muss:**

1. **Mock-Framework**
   ```varg
   @[Test]
   method TestAgentRuntime() {
       let mock_model = mock<IModelClient>()
       mock_model.when("infer").returns("Hello World")
       mock_model.when("infer").with_args("error").throws(AgentError.LlmTimeout {...})

       let runtime = AgentRuntime { model: mock_model, ... }
       let result = runtime.Run("test")

       assert_eq(result, "Hello World")
       mock_model.verify("infer").called_once()
   }
   ```

2. **Test-Assertions erweitern**
   ```varg
   assert_eq(a, b)              // Gleichheit
   assert_ne(a, b)              // Ungleichheit
   assert_true(condition)       // Boolean
   assert_contains(list, item)  // Collection
   assert_throws<ErrorType>(fn) // Exception
   assert_matches(value, pattern) // Pattern-Match
   ```

3. **Test-Fixtures / Setup-Teardown**
   ```varg
   @[TestFixture]
   agent RuntimeTests {
       state db: DbConnection

       @[SetUp]
       method Setup() {
           db = sqlite_open(":memory:")
       }

       @[TearDown]
       method Cleanup() {
           sqlite_close(db)
       }

       @[Test]
       method TestSomething() { ... }
   }
   ```

4. **Code-Coverage-Report**
   - `vargc test --coverage` → Generiert Coverage-Report
   - Line-Coverage und Branch-Coverage

**Akzeptanzkriterien:**
- [ ] `mock<Contract>()` erzeugt Mock-Instanz
- [ ] `when/returns/throws/verify` API funktioniert
- [ ] SetUp/TearDown pro Test-Agent
- [ ] Coverage-Report via CLI

---

### Phase 8: Telegram Bot Framework
**Priorität:** 🟡 Hoch
**Geschätzter Aufwand:** 3–5 Tage
**Abhängigkeit:** Phase 1 (für `teloxide` Crate) ODER Phase 2 (HTTP-Client reicht)

**Was implementiert werden muss:**

1. **Bot-Agent Pattern**
   ```varg
   agent TelegramBot {
       state token: string
       state offset: int

       on_start() {
           token = env("TELEGRAM_BOT_TOKEN")
           start_polling()
       }

       method start_polling() {
           loop {
               let updates = telegram_get_updates(net_cap, token, offset)?
               for update in updates {
                   offset = update.id + 1
                   handle_update(update)
               }
           }
       }

       method handle_update(update: TelegramUpdate) {
           match update.type {
               "message" => on_message(update.message),
               "callback_query" => on_callback(update.callback),
               _ => log_warn($"Unknown update type: {update.type}")
           }
       }
   }
   ```

2. **Telegram API Builtins**
   ```varg
   telegram_get_updates(cap, token, offset) -> List<Update>
   telegram_send_message(cap, token, chat_id, text, opts) -> Message
   telegram_send_photo(cap, token, chat_id, photo, caption) -> Message
   telegram_answer_callback(cap, token, callback_id, text) -> bool
   telegram_edit_message(cap, token, chat_id, msg_id, text) -> Message
   telegram_get_file(cap, token, file_id) -> bytes
   ```

3. **Inline-Keyboard-Builder**
   ```varg
   let keyboard = inline_keyboard([
       [button("Option A", callback: "opt_a"), button("Option B", callback: "opt_b")],
       [button("Abbrechen", callback: "cancel")]
   ])
   telegram_send_message(cap, token, chat_id, "Wähle:", keyboard: keyboard)
   ```

**Akzeptanzkriterien:**
- [ ] Long-Polling empfängt Nachrichten
- [ ] Nachrichten senden (Text, Foto, Dokument)
- [ ] Inline-Keyboards mit Callbacks
- [ ] File-Download (Telegram → lokal)

---

### Phase 9: Matrix-Protokoll
**Priorität:** 🟢 Mittel
**Geschätzter Aufwand:** 3–5 Tage
**Abhängigkeit:** Phase 1 (für `matrix-sdk` Crate), Phase 2 (HTTP-Server für AS)

**Was implementiert werden muss:**

1. **Matrix-Client**
   ```varg
   let client = matrix_connect(net_cap, homeserver, access_token)
   matrix_send(client, room_id, "Hello from VARG!")
   matrix_sync(client) { event ->
       match event.type {
           "m.room.message" => handle_message(event),
           _ => {}
       }
   }
   ```

2. **Application-Service Bridge** (für Egregor-Integration)
   - AS-Registration-File generieren
   - Webhook-Empfänger für Synapse-Events
   - User-Bridging (virtuelle Matrix-User)

**Akzeptanzkriterien:**
- [ ] Login + Sync-Loop
- [ ] Nachrichten senden/empfangen
- [ ] AS-Registration und Event-Handling

---

### Phase 10: Docker-API
**Priorität:** 🟢 Mittel
**Geschätzter Aufwand:** 2–3 Tage
**Abhängigkeit:** Phase 2 (HTTP-Client für Docker Socket)

**Was implementiert werden muss:**

1. **Docker-Client via Unix-Socket / HTTP**
   ```varg
   let docker = docker_connect(sys_cap)
   let containers = docker_list(docker, filter: "label=egregor")
   let id = docker_create(docker, image: "egregor-clone:latest", env: env_map)
   docker_start(docker, id)
   docker_stop(docker, id)
   docker_remove(docker, id)
   ```

2. **Container-Lifecycle für Clone-System**
   - Create → Start → Monitor → Stop → Remove
   - Log-Streaming: `docker_logs(docker, id, follow: true)`
   - Health-Check: `docker_inspect(docker, id).health`

**Akzeptanzkriterien:**
- [ ] Container erstellen, starten, stoppen, löschen
- [ ] Container-Logs streamen
- [ ] Image-Management (pull, list)

---

### Phase 11: MCP-Protokoll vervollständigen
**Priorität:** 🟢 Mittel
**Geschätzter Aufwand:** 2–3 Tage
**Abhängigkeit:** Phase 2 (HTTP-Server), Phase 4 (WebSocket-Server optional)

**Was implementiert werden muss:**

1. **JSON-RPC Transport** (aktuell nur API-Surface)
   - stdio-Transport (für lokale MCP-Server)
   - HTTP+SSE Transport (für remote MCP-Server)

2. **MCP-Server-Modus** (Egregor-Tools als MCP-Tools exponieren)
   ```varg
   @[McpTool(name: "manage_memory", description: "Manage agent memory")]
   method ManageMemory(action: string, content: string) -> string {
       // ...
   }
   ```

3. **MCP-Client-Modus** (externe MCP-Server einbinden)
   ```varg
   let mcp = mcp_connect(net_cap, "stdio", command: "npx @modelcontextprotocol/server-files")
   let tools = mcp_list_tools(mcp)
   let result = mcp_call_tool(mcp, "read_file", { path: "/tmp/data.txt" })
   ```

**Akzeptanzkriterien:**
- [ ] JSON-RPC über stdio funktioniert
- [ ] MCP-Server: Tools werden korrekt exponiert
- [ ] MCP-Client: Tools von externen Servern aufrufbar

---

## Zusammenfassung: Reihenfolge und Abhängigkeiten

```
Phase 1: Crate-Import/FFI ──────┐
         (2-3 Wochen)           │
                                ├──→ Phase 3: DB-Treiber (3-5 Tage)
                                ├──→ Phase 8: Telegram (3-5 Tage)
                                ├──→ Phase 9: Matrix (3-5 Tage)
                                │
Phase 2: HTTP-Server ───────────┼──→ Phase 4: WebSocket/SSE-Server (3-5 Tage)
         (3-5 Tage)             ├──→ Phase 10: Docker-API (2-3 Tage)
                                └──→ Phase 11: MCP komplett (2-3 Tage)

Phase 5: Typisierte Fehler ─────┐
         (2-3 Tage)             ├──→ Phase 6: DI-Container (4-5 Tage)
                                └──→ Phase 7: Test-Mocking (5-7 Tage)
```

**Kritischer Pfad:** Phase 1 → Phase 3 → Phase 8 (Neo4j + Telegram = Egregor-Kernfunktion)

**Gesamtaufwand:** ~5–8 Wochen (Vollzeit)

**Empfehlung:** Phase 1 (Crate-Import) und Phase 5 (Typisierte Fehler) können
**parallel** entwickelt werden, da sie unabhängig voneinander sind.

---

## Risiken

| Risiko | Auswirkung | Mitigation |
|--------|-----------|------------|
| Crate-Import-Komplexität | Verzögerung aller abhängigen Phasen | Minimal-Variante: nur `[dependencies]` in Cargo.toml, kein Feature-Flag-Support initial |
| Neo4j-Treiber in Rust unreif | AKG nicht portierbar | `neo4rs` 0.7 ist stabil; alternativ Bolt-Protokoll manuell via TCP |
| OCAP + FFI-Konflikte | Extern-Crates umgehen OCAP | Extern-Aufrufe MÜSSEN Capability-Parameter fordern — Compiler-Enforcement |
| Actor-Model Skalierung | >100 Agents = Performance? | Benchmark nach Phase 1; ggf. Sharding einführen |
| Kein Debugger | Schwierige Fehlersuche | log_debug/log_error + generierte Rust-Source inspizieren |
