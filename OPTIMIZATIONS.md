# Varg — Optimierungen & Roadmap nach dem Bugfixing

> Stand: 2026-07-14 · Version 1.0.0 · 1144 Compiler-Tests grün
>
> Dieses Dokument sammelt alles, was **über reines Bugfixing hinausgeht**: sinnvolle nächste
> Schritte, sobald die kritischen Compiler-Bugs behoben sind (siehe Abschnitt „Erledigte
> Bugfixes"). Priorisiert nach Hebelwirkung.

## Zweite Runde erledigt (Robustheit + Verdrahtung, R1-R5)

- **R1** Graph-`NODE_COUNTER` von global auf pro-Instanz (`next_id` in `GraphDb`) umgestellt.
- **R2** `lock().unwrap()` → `lock().unwrap_or_else(|e| e.into_inner())` über alle
  Runtime-Module (verhindert Poisoned-Lock-Kaskaden nach einem ersten Panic).
- **R3** MCP `send_request`: ID-matchende Leseschleife statt „erste Zeile" — Notifications/
  Log-Zeilen auf stdout desynchronisieren nicht mehr; EOF und chatty-Server abgefangen.
- **R4** Graph-Ladepfad tolerant gegen korrupte DB (skip statt Panic); `graph_open`-Öffnungs-
  fehler → in-memory-Fallback statt Absturz.
- **R5** `pipeline_add_step` und `event_on` an Codegen+Typechecker verdrahtet (waren
  Stub/`Box`-Fehler); Handler-Lambdas bekommen typisierte Parameter (`gen_str_handler`/
  `gen_event_handler`). **VARG_AGENT_GUIDE.md**: interne `__varg_*`-Symbole aus allen 51
  Beispielaufrufen entfernt — die Beispiele kompilieren jetzt. Damit sind **`fan_out`/`fan_in`**
  die einzigen noch nicht verdrahteten Orchestrierungs-Builtins (Runtime vorhanden).

---

## Erledigte Bugfixes (Kontext)

Diese Bugs wurden in diesem Durchgang behoben und mit Regressionstests abgesichert:

| ID | Bug | Fix | Test |
|----|-----|-----|------|
| **B1** | Klammern gingen im Codegen verloren → stille Falschberechnung (`(1+2)*3` → `1+2*3`) | Präzedenzsichere Klammerung aller Binär-/Unär-Operanden (`gen_operand`) | codegen `test_codegen_preserves_parentheses_*` |
| **B2** | OCAP-Bypass: capability-Builtins in verschachtelten Aufrufen umgingen `check_ocap` | Argumente + Caller im `MethodCall`-Arm rekursiv geprüft, nur Cap-Fehler propagiert | typechecker `test_tc_ocap_not_bypassed_by_*` |
| **B3** | Rust-Keywords als Varg-Identifier (`loop`, `move`, `ref`) → nicht kompilierbar | `esc_ident` mit `r#`-Escaping (schont codegen-internes `self`) | codegen `test_codegen_escapes_rust_keyword_ident_b3` |
| **B4** | String-Escapes (`\n`, `\t`, `\"`) nie dekodiert; `print` mit Debug-Quotes | Echte `unescape_string_literal`; `is_string_expr` erkennt String-Vars | parser `test_unescape_string_literal_b4` |
| **B5** | `string + string` kompilierte nicht (`&str` vs `String`) | `is_string_expr` konsultiert `string_vars` (Felder, Params, Konkat) | via B4/Beispiele |
| **B6** | Parser-Stack-Overflow bei tiefer Verschachtelung | Rekursionstiefen-Guard + 256-MB-Worker-Thread für den Compiler | parser `test_deep_nesting_errors_not_overflow_b6` |
| **B7** | Server-Handler blockierten den tokio-Executor; decrypt-Panic | `spawn_blocking` für Handler; `decrypt` gibt Fehler-String statt Panic | runtime-Suite |
| **B8** | Integer-Literale > i32 überliefen | `i64`-Suffix außerhalb i32-Bereich | codegen `test_codegen_large_int_gets_i64_suffix_b8` |
| **B9** | `emit-rs` konnte den Compiler mit Stacktrace crashen | `catch_unwind` + saubere Fehlermeldung | manuell verifiziert |
| **B10** | Graph-Write-Through verschluckte Fehler (`.ok()`) → stiller Datenverlust | Fehler auf stderr sichtbar | runtime-Suite |
| **B11** | SSE-Client-Signaturbruch; `orchestrator_run_all` ohne Codegen | SSE-Signatur konsistent; `orchestrator_run_all` verdrahtet & lauffähig; `@[RateLimit]` akzeptiert Positions- **und** Named-Syntax | manuell verifiziert |

---

## Bei der Validierung neu gefundene, teils vorbestehende Bugs

- ✅ **`crypto`-Feature ohne base64** (behoben): `crypto.rs` nutzt `base64`, aber das Feature
  `crypto = [aes-gcm, pbkdf2, sha2]` zog es nicht ein → **jedes encrypt/decrypt-Programm baute
  nicht**. Fix: `dep:base64` ins `crypto`-Feature. End-to-end verifiziert (Roundtrip + Fehlerpfad).
- ✅ **Feature-Builds vollständig repariert** (waren vorbestehend defekt): alle Features
  (`crypto`, `encoding`, `pdf`, `net`, `llm`, `ws`, `db`, `tensor`, `dataframe`, `fts`,
  `duckdb`) **und `full`** kompilieren und ihre Tests laufen grün (`--features full`: 402/0).
  - Fehlende Feature-Deps ergänzt: `crypto`→base64, `pdf`→base64, `encoding`→reqwest,
    `llm`→net+base64.
  - Echte Code-Brüche behoben: `tensor.rs` (`(**t).clone()` statt Move aus Arc),
    `rag.rs` (Vektor-Ranking über `store.entries` statt nicht existierendem `store.conn`),
    `fts.rs` (tantivy-0.22-Doc-Typ annotiert; ID-Feld `STRING` statt `TEXT`, damit
    `delete_term` exakt matcht), `duckdb_rt.rs` (`column_count()` erst nach `query()`, sonst
    Panic „statement not executed").
  - **Verbleibend (Priorität 0.1):** CI-Job mit `--features full`, damit die feature-gegateten
    Module nicht wieder unbemerkt brechen (die Default-`cargo test`-Läufe kompilieren sie mit
    `default = []` nicht).
- ⬜ **Default-Testsuite verdeckt das**: `cargo test --workspace` nutzt `default = []`, also
  werden die feature-gegateten Module (crypto, rag, fts, tensor, dataframe, duckdb) **gar nicht
  kompiliert** — die „1144 Tests" decken sie nicht ab. **Maßnahme:** CI-Job mit
  `--features full` (nach Reparatur der obigen Module) oder pro-Feature-Matrix, damit solche
  Brüche nicht unbemerkt bleiben. Gehört zu Priorität 0.1 (Test-Abdeckung).

## Compiler-Audit (2026-07-15) — Befunde & Status

Systematisches Abklopfen von Sprache/Codegen/Tooling durch echtes Kompilieren (~35 Probe-Programme).

**Behoben in dieser Runde:**
- ✅ **`vargc check`** — reiner Parse+Typecheck, **39 ms vs. 646 ms Build (~16×)**. Für Editor/CI.
- ✅ **`print` berechneter Werte** — nutzte Debug `{:?}` (Strings mit Anführungszeichen). Jetzt
  einheitlich über `__varg_fmt()` (Strings via Display, Structs/Enums/Collections/Option via
  Debug; User-Typen bekommen eine `__VargFmt`-Impl emittiert).
- ✅ **`add`→`insert`-Korrektheitsbug** — jede Agent-Methode namens `add` (o.ä. Builtin-Name)
  wurde zu `.insert(...)` umgeschrieben. Agent-Methoden schatten jetzt Builtins (wie Impl-Methoden).
- ✅ **`env` Typ-Drift** — Typechecker sagte `String`, Codegen emittiert `Result`. Angeglichen.
- ✅ **`print`/Interpolation eines `Result`** wird jetzt vom Typechecker mit klarer Meldung
  abgelehnt (statt rustc-Leak). Fängt vergessene `?`/`or`.
- ✅ REFERENCE.md Result-Beispiel (Zeile ~457) korrigiert (implizite Erfolgstyp-Idiom).

**Offen — größere Compiler-Projekte (nach Hebelwirkung):**
1. **Typ-annotierter AST (Typechecker→Codegen)** — die eine Wurzel hinter der Codegen-Fragilität.
   **Begonnen (Stufe 1, mit Golden-Output-Netz):**
   - ✅ `golden/` — Golden-Output-Sicherheitsnetz (9 Programme, stdout-Diff) gegen stille
     Miskompilierung.
   - ✅ Codegen-Typumgebung `var_types` + `resolve_type(expr)` (aus Let-/Param-/Feld-Typen).
   - ✅ `is_string_expr` typ-genau über `resolve_type` (statt reiner Heuristik).
   **Allokations-Gewinne (Stufe 1+2):**
   - ✅ `x == "lit"` vergleicht gegen `&str` statt pro Vergleich einen `String` zu allokieren.
   - ✅ Typ-getriebenes `print`: für Display-Primitive (String/Zahl/Bool) direkt `{}` statt der
     Extra-String-Allokation von `__varg_fmt()`.
   - ✅ `filter`: `.iter().filter(..).cloned()` statt `.iter().cloned().filter(..)` — klont nur
     die Überlebenden, nicht die ganze Kollektion vorab (2N → N+K Clones).
   **Stufe 3 (gemeinsame Signatur-Tabelle):**
   - ✅ `varg-ast/src/builtins.rs` — `builtin_return_type(name)` als **Single Source of Truth**
     für Builtin-Rückgabetypen (String/Int/Float/Bool/Result), von `resolve_type` konsultiert.
     `resolve_type` kennt jetzt Builtin-Ergebnisse → `var s = json_get(..); print s;` wird
     typaufgelöst (sauberer print, korrekte Konkat). Fundament, um die 346-vs-393-Duplikation
     schrittweise abzubauen.
   **Stufe 4 (Sprach-Fix auf dem Typ-Fundament):**
   - ✅ **Gemischte int/float-Arithmetik** (`5 + 2.5`, `i * f`): die int-Seite wird zu `f64`
     gecoerct (war E0277). `resolve_type` promotet numerisch (Float wenn ein Operand Float),
     sodass auch verkettete Mixed-Arithmetik über Variablen trägt (`x = 5 + 2.5; x + 1`).
   - ✅ Nebenfund via Golden-Netz: json_get/int/bool/array ignorierten JSON-Pointer-Pfade
     (`/name`) — jetzt korrekt (`.pointer()` für `/`-Pfade, sonst `.get()`).
   **Stufe 5 (Drift-Lock statt Duplikation):**
   - ✅ **Typechecker an die Tabelle gekoppelt** — statt die ~340 Builtin-Arms (die zusätzlich
     Arity-/OCAP-Checks tragen und daher nicht durch reine Tabellen-Lookups ersetzbar sind) blind
     umzuschreiben, treibt ein Cross-Check-Test die *echte* Typechecker-Inferenz für jeden Namen in
     `builtins.rs` und asserted Gleichheit mit `builtin_return_type`. Divergenz bricht CI. Der Lock
     fand sofort **zwei echte Latenz-Bugs**: `fetch`/`http_download_base64` waren als
     `Result<String,Error>` getaggt, ihre Runtime-Fns liefern aber blankes `String` → `resolve_type`
     hätte die Ergebnisse fehlbehandelt. Tabelle auf `String` korrigiert, `known_builtin_names()`
     ergänzt (Test deckt künftige Einträge automatisch ab).
   **Stufe 6 (Receiver-Dispatch + Generics-Bounds):**
   - ✅ **Receiver-getypter Method-Dispatch (T3)** — String/Collection-Builtins (`len`, `to_upper`,
     `split`, `push`, …) auf einem skalaren Empfänger (`n.len()` mit int) werden jetzt im
     Typechecker mit exaktem Source-Span abgelehnt, statt als rustc-Fehler zu leaken. Konservativ:
     feuert nur bei konkretem Nicht-`self`-Empfänger mit definitem Skalar-Typ; `to_string` bleibt
     erlaubt. Keine False-Positives (volle Suite + Golden + 11 Beispiele grün).
   - ✅ **Generics-Bounds-Emission** — der `fn`-Parser verwarf Trait-Bounds (`fn max<T: Comparable>`);
     jetzt werden sie gespeichert (`FunctionDef.constraints`) und vom Codegen emittiert, sodass rustc
     dieselbe Schranke durchsetzt (Parität mit Methoden, die das schon taten). End-to-end verifiziert:
     `fn label<T: IShape>` kompiliert & läuft mit erhaltener Schranke.
   **Stufe 7 (generische Funktions-Pipeline komplett — „durchgezogen"):**
   - ✅ **Agent-Konstruktor-Syntax** `AgentName(args)` — Typechecker erkennt den Aufruf eines
     bekannten Agent-Namens als Konstruktion (→ Agent-Typ, mit Arity-Prüfung gegen den Konstruktor).
     Codegen emittiert den Konstruktor als assoziierte `fn Name(args) -> Self` (Feld-Default-Init +
     privater `&mut self`-Initializer für den Body, ohne `self`-Renaming) und übersetzt die Call-Site
     zu `Name::Name(args)` / `Name::new()` / `Name {}`. Bonus: der Entry-Point-Picker bevorzugt jetzt
     einen Agenten mit `Run`/`Main` statt blind den ersten.
   - ✅ **Float-Arithmetik-Inferenz** — `-`/`*`/`/`/`%` (und `+`) promoten auf `Float`, wenn ein
     Operand Float ist (vorher immer `Int` → `float * float` schlug als Typfehler fehl).
   - ✅ **Generische Body-Methodenauflösung** — ein Methodenaufruf auf einem an einen Contract
     gebundenen Type-Param (`shape.area()` bei `T: IShape`) löst gegen den Contract auf; Codegen
     bindet solche Params `mut` (Contract-Methoden nehmen `&mut self`).
   - **End-to-end verifiziert:** `total_area(Square(3.0))` (generische Funktion über einen per
     Konstruktor gebauten, Contract-implementierenden Agenten) kompiliert & läuft → `9`. Als
     Golden-Programme `generics.varg` + `construction.varg` dauerhaft abgesichert.
   **Stufe 8 (Baubarkeit der Zielprojekte Egregor/Edda/MCP-MCP — Blocker geschlossen):**
   - ✅ **DI-Konstruktoren mit Contract-Feldern** (`Service(ILog l) { self.logger = l; }`): Konstruktor-
     Bodies aus reinen `self.field = expr`-Zuweisungen werden als **Struct-Literal** emittiert (keine
     Default-Init nötig → Contract-`Box<dyn>`-Felder funktionieren); Call-Site **boxt** konkrete Agenten
     in den Trait-Objekt-Parameter. End-to-end: `Service(ConsoleLog())` → läuft. Das ist das
     Kompositions-/Testmuster (CLAUDE.md) für alle drei Zielprojekte.
   - ✅ **User-Methoden vor Builtins** (Typechecker): `agent.get()`/`add()`/`contains()` lösen zur
     User-Methode auf, statt vom gleichnamigen Map/Collection-Builtin-Arm geschluckt zu werden
     (generisch-gebundene Methoden weiterhin über den Bound-Enforcement-Pfad). Codegen priorisierte
     bereits.
   - ✅ **MCP-Server dynamisches Tool-Abschalten**: `mcp_server_remove_tool(srv, name) -> bool` +
     `mcp_server_has_tool` (Runtime + Typechecker + Codegen). `tools/list`/Calls bedienen entfernte
     Tools nicht mehr → Kern-Baustein für einen Router-MCP, der Kind-Capabilities zur Laufzeit
     an/abschaltet. Golden: `mcp_router.varg`.
   - **Baubarkeits-Fazit:** Egregor (Agent-Loop + LLM + MCP-Client + 3-Lagen-Memory/KG/Vector) und
     Edda (KG/Vector/RAG) waren schon durch bestehende Golden-Programme (`agent_memory`,
     `knowledge_graph`, `vector_store`) abgedeckt; es fehlte nur **Komposition (DI)** und **MCP-Tool-
     Hotswap** — beide jetzt zu. Golden-Netz: 17 Programme.
   **Stufe 9 (die fünf Ausbaustufen — alle abgearbeitet):**
   - ✅ **Serverseitiges WebSocket**: `ws_route(server, path, (msg) => reply)` — echter axum-Upgrade,
     bidirektional (Gegenstück zum nur-server→client-SSE). Dabei **zwei latente Defekte gefunden**:
     `VargHttpServerHandle` war aus Varg gar nicht erreichbar (⇒ serverseitiges SSE ließ sich nie
     kompilieren, trotz vorhandener Runtime; `sse_open` emittierte zudem `&` statt `&mut`) — jetzt auf
     **einen** Server-Typ vereinheitlicht (Routes + SSE + WS); und ein **async Entry-Point wurde nie
     awaited** (`instance.Run();` ⇒ Future verworfen, ein `async Run()` mit Server startete stumm nichts).
     Verifiziert: Varg-WS-Client ↔ Varg-WS-Server (`echo: ping`).
   - ✅ **Registry-Download mit Checksum**: `registry_download(reg, name, version, url, sha256)` —
     echter HTTP-Fetch, installiert **nur** bei passendem SHA-256; Mismatch = harter Fehler, nichts
     wird geschrieben/vermerkt (unverifizierter Download = Supply-Chain-Loch). Verifikationspfad von
     HTTP getrennt ⇒ ohne Netz testbar (Known-Vector, Tamper-Reject, Cache-Write). OCAP-gated.
   - ✅ **Produktions-ANN (HNSW)**: LSH war nicht nur schwach — `vector_build_index` **verwarf** den
     Index und `vector_search_fast` baute ihn **pro Query neu** (⇒ approximativ *und* langsamer als
     Brute Force). Jetzt echter HNSW (`instant-distance`, Feature `ann`), am Handle gehalten; Stale-
     Index ⇒ exakter Fallback statt veralteter Treffer. Ohne `ann` exakt (korrekt, linear).
   - ✅ **Workflow-Runner**: `workflow_set_handler` + `workflow_run` führen den DAG wirklich aus
     (Dep-Outputs als JSON an den Handler, Panic/fehlender Handler ⇒ failed + Downstream skipped,
     terminiert sauber). Golden: `workflow_runner.varg`.
   - ✅ **LLM-Token-Streaming**: `llm_stream_to(prompt, model, (token) => …)` liefert Tokens
     **inkrementell** (das alte `llm_stream` sammelte erst alles ⇒ kein Live-Output). Streaming-Kern
     von HTTP getrennt ⇒ mit aufgezeichneten SSE-Zeilen testbar (OpenAI/Anthropic/Ollama). Gegen einen
     lokalen Fake-Provider end-to-end verifiziert.
   **Stufe 10 (die zwei Kleinigkeiten — erledigt):**
   - ✅ **Literale Embeddings**: `vector_store_upsert/search/search_fast` nehmen jetzt sowohl `f32`
     (aus `embed()`) als auch `f64` (Varg-Float-Array-Literale kompilieren zu `Vec<f64>`) — via
     `ToF32Vec`-Konvertierung statt harter `&[f32]`-Signatur. `vector_store_upsert(vs, "x", [1.0, 0.0, 0.0], {})`
     läuft.
   - ✅ **JSON-Accessoren beidseitig**: die Familie widersprach sich — `json_get*` verlangte einen
     **geparsten Wert**, `json_keys`/`json_values`/`json_has` dagegen einen **rohen String**; was man
     auch hatte, die Hälfte lehnte ab. Jetzt nimmt alles `impl AsJson` (Wert **oder** JSON-String),
     zentral in `varg-runtime/src/json.rs` statt als Inline-Codegen. `json_get(s, "/a/b")` ohne
     `json_parse` funktioniert, `json_has(parsed, "k")` ebenfalls (war vorher schlicht kaputt).
2. ✅ **rustc-Fehler → .varg-Konstrukt rückmappen** — Codegen sät `// @varg-ctx <datei> :: <konstrukt>`
   an jeden Funktions-/Methoden-Body; `vargc` fängt fehlgeschlagene Builds ab und übersetzt jede
   `main.rs:NN`-Fehlerstelle in das nächstgelegene Varg-Konstrukt (z. B. „agent Server.handle"),
   statt roher Weitergabe. Nebenbei: ein Nicht-Null-**Programm**exit (aus `vargc run`) wird nicht mehr
   fälschlich als „Compilation failed" gemeldet. Der Happy-Path bleibt unverändert (Live-Ausgabe);
   nur im Fehlerfall läuft ein schneller, cachender Re-Build zum Einsammeln der Diagnostik.
3. **Typechecker-Vollständigkeit** — fängt derzeit NICHT: User-Method-Arity, Methoden-Existenz
   auf Werten, Enum-mit-Daten-Konstruktion (`Circle(5)` → falsch als Methodenaufruf), mixed
   int+float-Coercion, Type-Alias-Transparenz, Funktionstypen `fn(int)->int` (Parser),
   Closure-in-Variable-Typinferenz, explizites `-> Result<T>`-Auto-Wrap, `.sort()`-Rückgabe,
   async Entry-Point (wird nie awaited).
4. **Codegen-Allokations-Quick-Wins** — `"lit".to_string()` in print/Vergleichen,
   String-Vergleich allokiert pro Iteration, Doppel-Clone in `filter`-Closures für Copy-Typen.
   ~27 `to_string`/223 Zeilen im typischen Datenpfad. (Voll erst mit #1 sauber.)
5. **LSP-Härtung** — Typfehler als `WARNING` statt `ERROR`, statische Completion,
   Textscan-Go-to-Definition, kein Rename, Formatter nicht angebunden.

## Priorität 0 — Vertrauen absichern (Voraussetzung für alles Weitere)

### 0.1 Golden-Output-Tests statt nur „kompiliert"-Tests
**Problem:** B1 (stille Falschberechnung) überlebte 1131 Tests, weil kaum ein Test das
**Laufzeit-Ergebnis** eines kompilierten Programms prüft — nur, dass Rust erzeugt wird.
**Maßnahme:** Für jedes Beispiel und jeden Kern-Operator einen `vargc run`-Test mit erwarteter
stdout-Ausgabe (Snapshot/Golden-Files). Dies ist die wichtigste Einzelinvestition — ohne sie
kann sich ein B1-artiger Bug jederzeit wiederholen.
**Aufwand:** mittel · **Hebel:** sehr hoch.

### 0.2 Durchgängiges Typkontext-Modell vom Typechecker in den Codegen
**Problem:** B4, B5, B8 und der `print`-Debug-Bug haben dieselbe Wurzel — der Codegen kennt die
Typen nicht mehr, die der Typechecker längst berechnet hat, und rät (Heuristiken wie
`string_vars`). Das ist fragil und deckt nicht alle Fälle ab (z. B. `print` auf einem
String-Rückgabewert einer Methode).
**Maßnahme:** Eine **typannotierte AST** (Typ an jedem Ausdrucksknoten) oder eine
Symboltabelle, die der Typechecker füllt und der Codegen liest. Löst mehrere Bug-Klassen
strukturell statt per Heuristik und ist die sauberste Basis für künftige Features.
**Aufwand:** hoch · **Hebel:** hoch.

---

## Priorität 1 — Die beworbene „Agent-Layer" real machen

Mehrere Runtime-Module sind implementiert, aber aus der Sprache **nicht (voll) erreichbar**.
Die teuerste Arbeit (die Runtime) existiert bereits — es fehlt nur die Verdrahtung.

### 1.1 Restliche abgeschnittene Builtins anschließen
- ✅ `orchestrator_run_all`, `pipeline_add_step`, `event_on` sind verdrahtet (B11/R5).
- ⬜ `fan_out` / `fan_in` (Runtime in `orchestration.rs`, noch nicht verdrahtet) — dieselbe
  `gen_str_handler`-ABI lässt sich wiederverwenden.
**Aufwand:** niedrig · **Hebel:** mittel.

### 1.2 Stub-Features ehrlich machen oder fertig bauen
| Feature | Aktueller Zustand | Empfehlung |
|---------|-------------------|-----------|
| **SSE-Client** (`sse_stream/send/close`) | lokaler No-op-Writer | Entweder echten SSE-Client (reqwest-stream) bauen oder klar als „server-side writer only" dokumentieren; die neuen `sse_open/sse_push` (server.rs) sind der reale Pfad |
| **Package Registry** (`registry_install/search`) | schreibt nur name→version, lädt nichts; `search` filtert hartcodierte Liste | Echten HTTP-Download + **Checksum-Prüfung** (das `checksum`-Feld existiert, wird nie genutzt) |
| **MCP-Server-Tools** | ✅ `mcp_server_register(srv, name, desc, (args) => result)` verdrahtet den Varg-Handler wirklich (4-Arg-Form; 3-Arg-Echo-Stub bleibt back-compat). Offen: `@[McpTool]` sollte `inputSchema` erzeugen |
| **Workflow-DAG** | reiner Status-Tracker, kein Runner, keine Zyklenerkennung | Runner + Zyklenerkennung ergänzen, sonst als „Tracker" (nicht „Engine") dokumentieren |
| **Embeddings** (`embed`, `llm_embed_batch`) | ✅ provider-agnostisch: OpenAI / Gemini / Ollama (echt, semantisch) via `VARG_EMBED_PROVIDER`/`VARG_EMBED_MODEL`; 384-dim lexikaler Fallback (statt 64-dim Zeichen-Hash). vargc zieht `net` automatisch. Offen: optional lokaler ONNX-Embedder (`fastembed`) für echt-semantisch ohne Ollama/Key |

### 1.3 OCAP zur Laufzeit härten oder Grenzen klar dokumentieren
**Problem:** OCAP ist ein **reines Compile-Zeit-Gate**. `exec` läuft über `cmd /C`/`sh -c`
(Command-Injection bei ungeprüften Eingaben); ein Token verhindert nur den *Aufruf*, nicht
missbräuchliche *Argumente*.
**Maßnahme:** Entweder eine Laufzeit-Sandbox/Argument-Validierung, oder in REFERENCE.md klar
als „Compile-Time-Capabilities, keine Laufzeit-Sandbox" kennzeichnen. Für eine Sprache, deren
USP „capability-based security" ist, zentral.
**Aufwand:** hoch (Sandbox) / niedrig (Doku) · **Hebel:** hoch (Glaubwürdigkeit des USP).

---

## Priorität 2 — Robustheit-Backlog (Runtime-Härtung)

Behoben: extern-getriebene Crash-Vektoren (decrypt, Graph-Datenverlust, Server-Blocking),
Poisoned-Lock-Muster (R2), MCP-Framing mit ID-Matching (R3), Graph-Ladepfad + `graph_open`
(R4), globaler `NODE_COUNTER` (R1). Verbleibend, jeweils ohne Signaturbruch nicht trivial:

- **`llm_structured<T>` sollte `Result<T>`/`Option<T>` zurückgeben** statt bei nicht
  deserialisierbarer LLM-Antwort zu panicken. Erfordert eine API-Änderung (Typechecker +
  Codegen + Aufrufer) — echter Fix, aber invasiv; nur mit Live-LLM auslösbar.
- **`db_open` sollte `Result` liefern** statt bei Öffnungsfehler zu panicken (aktuell
  Fail-Fast mit klarer Meldung — akzeptabel, aber nicht ideal).
- **MCP-Wall-Clock-Timeout:** R3 fängt Notification-Desync, chatty-Server und EOF ab; ein
  Server, der *gar nichts* sendet, blockiert weiterhin auf `read_line` — dafür bräuchte es
  einen dedizierten Reader-Thread mit `recv_timeout`.

---

## Priorität 3 — Echte Neuerungen mit Hebel

1. **Debug-Info / Source-Maps im generierten Rust**, damit `rustc`-Fehler auf die **Varg**-Zeile
   zeigen. Ohne das bleibt jeder Codegen-Fehler für Endnutzer praktisch undebugbar. (Teilweise
   vorhanden via `generate_with_source_map` — konsequent ausbauen.)
2. **`vargc check`** — schneller reiner Typecheck ohne Codegen/cargo, für Editor-Integration
   und CI. Dazu ein `emit-rs`-Modus **mit** Typecheck (der aktuelle überspringt ihn absichtlich).
3. **LSP-Ausbau**: Go-to-Definition, echte Diagnostics aus dem Typechecker, Hover mit Typen.
   Für eine Agenten-Sprache ist Editor-Feedback ein Multiplikator.
4. **Registry mit echtem Download + Checksum** (siehe 1.2) als Voraussetzung für ein
   glaubwürdiges Paket-Ökosystem.

---

## Aufräumen (billig, hohe Wirkung) — teils bereits erledigt

- ✅ Versionschaos in `.claude/CLAUDE.md` vereinheitlicht (v0.9.0/v0.7.0 → v1.0.0, Wave 47,
  1141 Tests, tote `VARG.md`-Referenz entfernt, „5 examples" → 11).
- ✅ Falsche Builtin-Signaturen in REFERENCE.md korrigiert (`workflow_status`, `registry_search`,
  `llm_structured`, `llm_stream`, `llm_embed_batch`).
- ⬜ `MEMORY.md`-Runtime-Tabelle: „net | ureq" → reqwest::blocking; Test-/Wave-Stand aktualisieren.
- ⬜ Alte Release-Zips (`varg-v0.12.0…`, `varg-v0.13.0…`) und `release-staging/` aus dem
  Arbeitsbaum entfernen (Verwechslungsgefahr mit eingefrorenen alten Doku-Kopien).
- ⬜ Leere `docs/Textdokument (neu).txt` löschen.
- ✅ `VARG_AGENT_GUIDE.md`: alle 51 internen `__varg_*`-Symbole aus den Beispielen entfernt;
  `pipeline_add_step`/`event_on` verdrahtet, sodass die Beispiele kompilieren.
