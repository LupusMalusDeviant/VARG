# Varg — Optimierungen & Roadmap nach dem Bugfixing

> Stand: 2026-08-26 · Version 1.0.0 · 1241 Compiler-Tests (default) / 1394 (`--features full`) · Golden 38/38 · Probes 55/55
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

   **Stufe 11 (MCP-MCP-Spike — `spikes/mcp-mcp/`):** ein MCP-Router (Kind-MCPs attachen, Tools
   aggregieren, Calls weiterleiten, zur Laufzeit hot-unpluggen, + HTTP-Control-UI) ist in Varg
   **baubar** — end-to-end verifiziert, Kind-MCPs selbst in Varg (kein npm/Netz). Der Spike war
   primär ein Ausdrucksfähigkeits-Test und legte sechs Lücken offen, alle im Compiler/Runtime
   geschlossen (nicht im Spike umschifft):
   - ✅ **`McpConnection` ist jetzt ein Handle** (`Arc<Mutex<_>>`) wie jeder andere zustandsbehaftete
     Runtime-Handle. Als nacktes `&mut`-Struct war es aus einem Tool-Handler (`Fn`+Send+Sync)
     unbenutzbar — also aus genau dem, was ein Router braucht.
   - ✅ **`mcp_call_tool` reicht rohe Argumente verbatim weiter** (`ToToolArgs`: Map **oder** JSON-
     String). Vorher nur `HashMap<String,String>` **mit Stringifizierung jedes Werts** (`{"n":42}` →
     `{"n":"42"}`) — für einen Proxy tödlich.
   - ✅ **`return` in Lambdas wird nicht mehr `Ok(...)`-gewrappt**, wenn die umgebende Methode `?`
     nutzt (Flag leckte in den Lambda-Scope).
   - ✅ **Handler-Closures klonen ihre Captures** — zwei Tools desselben Kindes scheiterten an
     „use of moved value".
   - ✅ **`http_route`/`ws_route`-Handler können capturen** — sie wurden als borrowende Closure
     emittiert („closure may outlive the current function"), Web-Handler waren damit faktisch auf
     zustandslos beschränkt (die UI wäre unmöglich gewesen). Jetzt `move` + geklonte Captures.
   - ✅ **`foreach` verbraucht die Kollektion nicht mehr**, wenn sie später nochmal benutzt wird
     (fragt die vorhandene Last-Use-Analyse). Nebenfund: der Usage-Walker besuchte `or`, Lambda,
     Match, Interpolation u. a. **gar nicht** → Verwendungen unterzählt (speist auch Move-vs-Clone).
   - ✅ **Verschachtelte Lambdas verloren ihre Bindungen** — der Parameter eines inneren Lambdas
     wurde vom äußeren Handler als Capture gewertet und geklont (`let args = args.clone();` →
     „not found in this scope"). Das blockierte genau den Fall *Tool aus einem HTTP-Handler heraus
     registrieren* = **Attach zur Laufzeit**. Gebundene Namen innerer Lambdas sind jetzt keine freien
     Variablen mehr. (Der Bug stammte aus dem Capture-Cloning oben — beim Nachfassen gefunden.)
   **Damit auch UI-getriebenes Attach:** der Router startet nur mit `echo`; `POST /attach` spawnt das
   math-Kind **zur Laufzeit** und exponiert seine Tools, `POST /detach` entfernt sie, Re-Attach
   funktioniert. Live über HTTP verifiziert.
   **Stufe 12 (Aufräum-Sweep — „was lauert noch?"):**
   - ✅ **`sse_stream`/`sse_send`/`sse_close` entfernt.** Sie zeigten auf einen Platzhalter, der das
     Event **wegwarf und `Ok` zurückgab** — Erfolg gemeldet, nie etwas gesendet. Ein Unittest prüfte
     genau dieses Ok/Err und gab damit falsche Sicherheit. Der Typechecker weist sie jetzt mit einem
     Zeiger auf das echte `sse_open`/`sse_push` ab; Platzhalter + Lügen-Test gelöscht.
   - ✅ **`fan_out`/`fan_in` verdrahtet.** Lagen fertig in der Runtime (echte Thread-Parallelität),
     waren aus Varg aber **gar nicht erreichbar** (kein Typechecker-Arm, kein Codegen). Verifiziert:
     4×300 ms laufen in <900 ms, Reihenfolge erhalten.
   - ✅ **`--features server` allein kompilierte nicht** (Test nutzte reqwest ohne `net`-Gate) und
     `normalize` war ohne `ann` toter Code (Warnung bei jedem Default-Build). Beides gefixt, dazu ein
     **CI-Job, der jedes Feature einzeln kompiliert** — die Lücke, die default/full strukturell nicht
     sehen können.

   - ✅ **`try/catch` + `return` war generell kaputt** — nicht handler-spezifisch. Der try-Body wird
     für `catch_unwind` in eine Closure gewickelt, also verließ ein `return` nur die Closure
     (Typfehler); und der Typechecker zählte `try/catch` gar nicht als returnend („not all code
     paths return"). Die Closure trägt den Rückgabewert jetzt heraus (`Ok(Some(v))`, RET von Rust
     inferiert), der Typechecker kennt `try/catch`/`throw` als returnend. **Damit geht auch `?` im
     Handler**: es propagiert in die try-Closure → wird zum `catch`. Golden: `try_return.varg`.
   - ✅ **`self` im Handler: Grenze ehrlich gemacht statt gefaked.** Sie bleibt real — ein geklontes
     `self` hätte Snapshot-Semantik (stille Mutation einer Kopie), ein `Arc<Mutex<Agent>>` würde
     deadlocken (die umgebende Methode hält `self` schon, während `http_listen` darin blockiert).
     Der Typechecker lehnt `self`-Zugriffe in Handler-Lambdas jetzt mit Nennung des Members und dem
     Ausweg ab („compute `page()` before the handler and let it capture the result"), statt rustcs
     „cannot borrow `*self` as mutable" durchzureichen. Unterscheidet echte `self`-Zugriffe von
     bloßen Builtin-Aufrufen (die ebenfalls als `caller: self` geparst werden) — keine False
     Positives über Spike, Golden und Beispiele.
   **Verbleibende benannte Grenze:** Route-Handler erreichen `self` nicht — jetzt aber mit klarer
   Compiler-Meldung statt rustc-Leak.
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

## Audit 2026-08-25 — Blindprobe mit frischen Programmen (Stufe 13)

**Methode:** 43 kleine Programme, die der Compiler noch nie gesehen hatte, quer über die
dokumentierten Sprachfeatures — nicht aus der Testsuite abgeleitet, sondern aus REFERENCE.md und
VARG_AGENT_GUIDE.md. Ausgangslage vorher: 1194 Unit-Tests grün, Golden 19/19 grün, 11 Beispiele
bauen. **Trefferquote der Blindprobe: 14/20 im ersten Durchgang** — die grüne Suite hat den
Zustand deutlich zu gut dargestellt.

### Gefunden und behoben

| # | Defekt | Klasse | Warum die Suite es nicht sah |
|---|--------|--------|------------------------------|
| **A1** | `abs(-5.0)` ergab **-5**. `abs(x)` lowert zu `x.abs()`; ohne Klammern liest Rust `-5.0.abs()` als `-(5.0.abs())`. Auch `abs(3 - 10)` war betroffen. | **stiller Rechenfehler** | Der einzige Unittest asserted `x.abs()` — also genau die kaputte Form, nur mit einem Operanden, bei dem sie zufällig stimmt. Kein Golden-Programm rief `abs` auf. |
| **A2** | **Agent-Messaging war ein No-op.** Der Dispatcher routete nur Nachrichten, die exakt so heißen wie eine Methode; das dokumentierte `send("process", x)` → `on_message("process", x)` fiel in `_ => "unknown"`. Nachricht weg, kein Fehler, Exit 0. | **stiller No-op** | Kein Test und kein Golden-Programm sendet je eine Nachricht. `examples/chat_agent.varg` — das Vorzeigebeispiel für den Actor-Teil — baute sauber und gab nur „Starting…"/„shutdown complete" aus. |
| **A3** | **`on_start`/`on_stop` wurden nie aufgerufen** — weder für den Entry-Agenten noch für gespawnte. In `on_start` initialisierte Felder blieben null. | **stiller No-op** | dito |
| **A4** | Nachrichten-Payload: ein Array-Argument wurde `format!("{}", vec![…])` — kompiliert gar nicht; die dokumentierte Signatur `on_message(string, string[])` war damit unerreichbar. | Compile-Fehler | dito |
| **A5** | `parse_int` / `parse_float` lowerten zu `.unwrap_or(0)` — jede kaputte Eingabe wurde **stumm zu 0** und floss als echte Zahl weiter. Gleichzeitig war das dokumentierte `parse_int(x) or 0` nicht kompilierbar (kein Result). | **stiller Falschwert** | Beide Richtungen ungetestet. |
| **A6** | Datentragende Enums waren **matchbar, aber nicht konstruierbar**: `Shape.Circle(5)` landete wörtlich im Rust-Output. | Compile-Fehler | Golden nutzt nur feldlose Enums. |
| **A7** | `.find((n) => n > 10)` kompilierte nie (Prädikat bekommt eine Referenz) und hätte die Kollektion verbraucht. | Compile-Fehler | `find` in keinem Golden-Programm. |
| **A8** | Type-Aliase wurden registriert, aber nie aufgelöst — `type Id = int; Id u = 21;` war ein Typfehler. Ein Alias war faktisch ein unbewohnbarer Nominaltyp. | Compile-Fehler | Der Test prüfte nur die Registrierung, nie die Benutzung. |

Zusätzlich gehärtet: `send` auf eine geschlossene Mailbox meldet jetzt auf stderr statt den
Sender per `unwrap()` mitzureißen; ein Agent ohne `on_message` warnt bei unroutbarer Nachricht,
statt sie spurlos zu schlucken; kurze Nachrichten panicken den Agent-Thread nicht mehr
(`args.get(i)` statt `args[i]`).

**Bewusste Breaking Change:** `parse_int`/`parse_float` liefern jetzt `Result`. `parse_int(s) + 1`
braucht ein `or`/`?`. Das ist der Preis dafür, dass eine kaputte Eingabe nicht mehr als 0 durchgeht;
REFERENCE.md und VARG_AGENT_GUIDE.md sind angeglichen.

### Golden-Netz erweitert: 19 → 22 Programme
`actor_messaging` (spawn/send/request/on_message/Lifecycle), `numeric_precision` (Präzedenz +
Fallibilität der numerischen Builtins), `enum_construct` (datentragende Enums, `find`, Type-Alias).
Das ist der Punkt: A1–A8 haben 1194 Unit-Tests überlebt, weil die Suite prüft, *dass* Rust erzeugt
wird, und selten, *was das Programm ausrechnet*. Jeder Fix hier ist deshalb mit einem
Laufzeit-Ergebnis abgesichert, nicht mit einem Codegen-String.

### Stand danach
1196 Unit-Tests (default) / 1348 (`--features full`), 0 Failures · Golden 22/22 · 11 Beispiele bauen
· Blindprobe 40/43, die 3 Ablehnungen sind gewollt (OCAP-Gate + unbehandelte Results).

### Weiterhin offen (nicht in diesem Durchgang angefasst)
~~Der Typechecker fängt User-Method-Arity, Methoden-Existenz und Argumenttypen nicht.~~
**Erledigt in Stufe 15** — alle 13 Fehlerfälle werden jetzt vom Typechecker mit Quellspanne
abgelehnt.

---

## Golden-Abdeckung 29 % → 97,9 % (Stufe 14)

**Ausgangspunkt:** Stufe 13 hatte gemessen, dass nur **27 von 94 Builtins** je von einem laufenden
Programm aufgerufen werden. Genau dort saßen alle acht Defekte jener Runde. Diese Stufe schließt
die Lücke.

**Ergebnis:** **92 von 94 (97,9 %)** in der ausgeführten und gediffte Golden-Suite. Die beiden
Ausnahmen — `fetch` und `http_download_base64` — brauchen Netz und sind bewusst ausgenommen; von
allem offline Prüfbaren sind es **100 %**. Golden-Programme: 22 → **31**.

### Design: selbstprüfende Programme statt Wertedumps
Jede Zeile druckt `OK <name>` oder `FAIL <name>: got … want …`. Ein falsches Ergebnis steht damit
als `FAIL` in der Ausgabe, statt als scheinbar plausibler Wert in die Erwartungsdatei eingebacken
zu werden. Das ist der Unterschied, der zählt: `--update` auf einem kaputten Compiler hätte den
Fehler sonst zur Norm erklärt. Nicht-deterministische Builtins (`random_*`, `uuid`, `timestamp`,
`time_millis`) werden über ihren **Vertrag** geprüft (Wertebereich, Länge, Eindeutigkeit,
Monotonie), nicht über den Wert — die Golden-Ausgabe bleibt dadurch stabil.

### Dabei gefunden und behoben

| # | Defekt | Klasse |
|---|--------|--------|
| **C1** | **`uuid()`, `random_int()`, `random_float()` waren nie kompilierbar.** Ihr Codegen emittiert `use rand::Rng;`, aber `vargc` hat die Crate nie in die generierte `Cargo.toml` geschrieben → `unresolved import` bei jedem Programm, das sie benutzt. Jetzt auto-injiziert wie chrono. | Compile-Fehler, 100 % der Aufrufer |
| **C2** | **Die gesamte Tensor-API war aus Varg unerreichbar.** `tensor_from_list` verlangte `&[f32]`, Varg-Float-Literale sind `f64` — der dokumentierte Konstruktor ließ sich nicht aufrufen. Derselbe Fehler wie bei den Embeddings in Stufe 10, dort gefixt, hier nicht. Jetzt über das gemeinsame `ToF32Vec`. | Compile-Fehler |
| **C3** | **Typ-Drift an der Tensor-Grenze:** Runtime lieferte `f32`, die Signatur-Tabelle behauptet `Float` (= f64). `tensor_mul_scalar(t, 2.0)` kompilierte nicht, und kein Reduktionsergebnis ließ sich mit einem anderen Varg-Float verrechnen. Grenze auf f64 vereinheitlicht (ndarray bleibt intern f32). | Compile-Fehler + latente Drift |
| **C4** | **Leere Array-Literale waren unbenutzbar** — auch mit deklariertem Typ. `int[] xs = [];` verwarf den Elementtyp und emittierte ein nacktes `vec![]`, das rustc nicht auflösen kann. Der `Array`-Arm fehlte neben dem `List`-Arm in der Typannotation. | Compile-Fehler |

### Doku-Korrekturen (Signatur stimmte nicht mit der Implementierung überein)
- `event_count(bus, "name")` → `event_count(bus)`: zählt **alle** Events des Bus, nicht pro Name.
- `registry_open("packages.json")` → nimmt ein **Cache-Verzeichnis**, keine Datei; der Zustand
  landet in `<dir>/installed.json`.
- `abs(-5)`-Beispiel um den Ausdrucksfall ergänzt (nach dem Präzedenz-Fix aus Stufe 13).

### Bekannte Sprachlücke (nicht behoben, umgangen)
Escapte Anführungszeichen in Interpolation sind nicht parsbar: `$"{f(\"x\")}"` bricht ab. Ein
String-Literal als Argument innerhalb einer Interpolation ist damit unmöglich — man muss den Wert
vorher an eine Variable binden. Betrifft nur die Schreibweise, nicht die Semantik.

### Die Abdeckung ist jetzt eine Ratsche, kein Schnappschuss
`varg-ast/src/builtins.rs::golden_programs_exercise_at_least_95_percent_of_builtins` liest
`golden/progs/*.varg`, zählt die tatsächlichen Aufrufstellen und bricht unter 95 % mit Nennung der
unabgedeckten Namen. Ein neues Builtin muss also mit einem Golden-Programm ankommen, das es
**ausführt und das Ergebnis prüft**. Negativ verifiziert: entfernt man ein Golden-Programm, fällt
der Test mit „coverage fell to 94.6% … Uncovered: [tensor_sum, …]".

### Stand danach
1197 Unit-Tests (default) / 1348 (`--features full`), 0 Failures · Golden **31/31**, drei Läufe
stabil · alle 12 Feature-Isolationen kompilieren · 11 Beispiele bauen.

---

## Typechecker-Vollständigkeit: Arity, Methoden-Existenz, Argumenttypen (Stufe 15)

**Ausgangspunkt:** Stufe 13/14 hatten das als die größte verbleibende Hürde benannt — `add(1)` bei
`fn add(int,int)`, `p.nonexistent()` und `add("a","b")` gingen alle erst bei rustc hoch, gemeldet in
Rust-Begriffen über generierten Code, den niemand geschrieben hat. Von 13 Fehlerfällen wurden **8
durchgereicht**.

**Ergebnis: 13 von 13 werden jetzt vom Typechecker gefangen**, mit Quellspanne auf das eigene
`.varg`-Konstrukt.

### Was das Material war
`MethodSignature.args` wurde seit jeher gesammelt und trug ein `#[allow(dead_code)]` — die
deklarierten Parameter lagen also vor, wurden aber an keiner Aufrufstelle konsultiert. Ergänzt um
`type_params` (damit ein generischer Parameter nicht gegen einen konkreten Typ geprüft wird) ist das
die ganze Grundlage.

### Drei Aufrufstellen
| Stelle | Vorher |
|--------|--------|
| Standalone-Funktionen | Rückgabetyp wurde geliefert, Argumente nie angesehen |
| Agent-/Impl-Methoden (Shadowing-Zweig) | kehrte vorzeitig zurück mit dem Kommentar „Arity-Prüfung bleibt dem allgemeinen Pfad überlassen" — der dadurch nie lief |
| Methoden über die Signaturtabelle | dito |
| Unbekannte Methode auf einem Typ **ohne** Methoden | ungeprüft: nur Typen mit ≥1 registrierter Methode wurden angesehen, `struct P { int x; }` akzeptierte jeden Aufruf |

### Konservativ ausgelegt
Eine solche Prüfung ist nur so viel wert, wie sie **kein** funktionierendes Programm ablehnt.
Übersprungen wird deshalb: Parameter, die einen eigenen Typparameter des Aufgerufenen nennen;
`TypeVar` und `Dynamic` auf beiden Seiten; Lambda-Argumente (deren inferierter Typ die Signatur
nicht modelliert); und jedes Argument, dessen Typ sich nicht bestimmen lässt. Parameter mit
Default-Wert dürfen von rechts weggelassen werden.

### Dabei gefunden und behoben
- **Capability-Token hatten zwei Schreibweisen**, die nicht verglichen wurden: der deklarierte
  Parametertyp ist `Capability(FileAccess)`, der Wert aus `FileAccess {}` inferiert als
  `Custom("FileAccess")`. Das Weiterreichen eines Tokens sah damit wie ein Typfehler aus — drei
  Beispiele fielen sofort um. In `types_match` angeglichen; damit funktioniert auch
  `FileAccess cap = FileAccess {};`.
- **Default-Parameter von Methoden wurden vom Codegen nicht gefüllt** (nur die von Standalone-
  Funktionen). Da die neue Arity-Prüfung Defaults ausdrücklich erlaubt, hätte der Typechecker sonst
  einen Aufruf durchgewinkt, den der Codegen nicht bauen kann. Jetzt symmetrisch — mehrdeutige
  Namen (zwei Typen, gleicher Methodenname, verschiedene Defaults) werden bewusst nicht gefüllt.
- **Contract-typisierte Parameter von Standalone-Funktionen waren unbenutzbar**: `fn total(IShape s)`
  ließ sich nicht mit `total(Sq())` aufrufen, weil die Call-Site den konkreten Agenten nicht in
  `Box<dyn IShape>` boxte (Konstruktoren taten das für DI längst) und der Parameter nicht `mut`
  gebunden war, obwohl Contract-Methoden `&mut self` nehmen.
- **Vorschläge waren irreführend:** ein vertippter Struct-Methodenname wurde mit „did you mean
  `pop`?" beantwortet — einem Collection-Builtin, das der Typ gar nicht hat. Ursache: die ~250
  Builtin-Namen wurden gleichrangig mit den Membern des Typs gematcht. Jetzt gewinnen die eigenen
  Methoden; auf Builtins wird nur zurückgefallen, wenn der Anfangsbuchstabe übereinstimmt — das
  trennt `lenght`→`length` (plausibler Tippfehler) von `nope`→`pop` (beide Distanz 2, nur der erste
  gemeint).

### Abgesichert
Neun Typechecker-Unittests für die Ablehnungen (ein Compile-Fehler lässt sich nicht golden testen)
**und** ein Golden-Programm `call_signatures` für die Akzeptanz-Seite: Defaults, Capability-Token,
Contract-Parameter, generische Parameter, Lambdas. Letzteres ist das wichtigere der beiden — es
hält fest, dass die Prüfung nichts kaputtmacht.

### Stand danach
1206 Unit-Tests (default) / 1358 (`--features full`), 0 Failures · Golden **32/32**, drei Läufe
stabil · 11 Beispiele + 4 Spike-Programme bauen.

### Weiterhin offen
~~`int? x` als Parameter~~ — **erledigt in Stufe 17**.

---

## Semantische Vollständigkeit: die restliche Sprachoberfläche (Stufe 16)

**Methode wie Stufe 13:** 35 frische Sonden über die Bereiche, die noch nie vermessen waren —
Struct-Literale, Rückgabetypen, Zuweisungen, Operatoren, Index, Match, Contracts, Scope, Casts,
Schleifen, async. Ausgangslage: **13 gefangen, 20 an rustc durchgereicht, 2 stumm akzeptiert**.

**Ergebnis: 34 von 35 vom Typechecker gefangen, null rustc-Leaks.** Der eine akzeptierte Fall
(`5 + "x"` → `"5x"`) ist beabsichtigte String-Konkatenation, keine Lücke.

### Der schwerwiegendste Fund: ein Match-Arm mit falschem Variantennamen
`Gren => …` bei `enum Color { Red, Green, Blue }` kompiliert zu einer **irrefutablen Bindung** —
sie matcht alles, und jeder Arm darunter ist tot. Ohne `_`-Arm fing die Exhaustivitätsprüfung das
entstehende Loch; **mit** `_`-Arm wurde sie komplett übersprungen, und der vertippte Arm
beantwortete stumm jeden anderen Fall:

```
Red   -> red
Green -> TYPO-ARM-SWALLOWED-IT     (erwartet: other)
Blue  -> TYPO-ARM-SWALLOWED-IT     (erwartet: other)
```

Kompiliert, läuft, Exit 0, falsches Ergebnis — dieselbe Klasse wie `abs(-5.0) = -5`.

Beim Absichern kam heraus, dass der erste Fix nur bei **typisiertem Parameter** griff. Bei der
weit häufigeren Form `var c = Colour.Red; match c { … }` war weiterhin nichts geprüft: `Colour.Red`
wurde als Feld-Zugriff inferiert und lieferte keinen Enum-Typ, also lief **jede** enum-abhängige
Prüfung (Exhaustivität eingeschlossen) ins Leere. Erst die korrekte Typisierung des
Varianten-Zugriffs schließt das.

### Die zwölf neuen Prüfungen
| Bereich | Vorher |
|---------|--------|
| Struct-Literal: fehlende Felder, falsche Feldtypen | nur *überzählige* Felder wurden geprüft |
| `return <wert>` aus `void` | explizit übersprungen (`expected != Void`) |
| `const` neu zuweisen | Const-ness wurde gar nicht verfolgt |
| Zuweisung an unbekanntes Agent-Feld | ungeprüft |
| Vergleich/Arithmetik über inkompatible Typen | Ergebnistyp berechnet, Operanden nie geprüft |
| Index mit falschem Schlüsseltyp | Index-Ausdruck wurde verworfen (`let _index_ty`) |
| `break`/`continue` außerhalb einer Schleife | keine Schleifen-Tiefe verfolgt |
| Iteration über einen Skalar | fiel auf `Dynamic` zurück |
| Ungültiger Cast (`"abc" as int`) | Quelltyp wurde verworfen |
| `await` auf Nicht-async / vergessenes `await` | Asyncness war in der Signatur nicht gespeichert |
| **Block-Scoping** | `env` war eine flache Map — Variablen aus `if`/`while`/`unsafe` blieben danach sichtbar |
| Match-Arm mit Nicht-Variante | s.o. |

### Konservativ und lowering-treu
Jede Prüfung feuert nur bei **definiten** Typen; alles Unaufgelöste passiert. Zwei Regeln mussten
an der tatsächlichen Lowering-Semantik nachgeschärft werden, statt an der Intuition:
- `as string` lowert zu `format!()` und akzeptiert daher **alles** — die erste, breitere
  Cast-Regel lehnte `42 as string` fälschlich ab und wurde vom Testlauf sofort gestellt.
- Ein **benannter Catch-All** (`other => …`) ist eine irrefutable Bindung und deckt die Restfälle
  wie `_`. Er wurde als Variante gezählt und meldete die abgedeckten Varianten als fehlend.

### Ein Test, der den Bug festgeschrieben hatte
`test_return_void_allows_anything` behauptete „Void methods don't enforce return type". Das war
keine Freiheit, sondern das Loch: `return 5;` aus `void Run()` erzeugt nicht kompilierbares Rust.
Test korrigiert und umbenannt.

### Abgesichert
14 Typechecker-Tests, geschrieben als **Varg-Quelltext** statt AST-Literale (`varg-parser` als
dev-dependency — kein Zyklus). Jeder Test prüft beide Richtungen: die Ablehnung *und* ein gültiges
Gegenstück. Dazu das Golden-Programm `semantics` mit 17 Prüfungen für die Akzeptanz-Seite.

### Stand danach
1220 Unit-Tests (default) / 1372 (`--features full`), 0 Failures · Golden **33/33**, drei Läufe
stabil · 11 Beispiele + 4 Spike-Programme bauen · Sonde 34/35, keine False Positives.

### Weiterhin offen
~~`int? x` als Parameter~~ — **erledigt in Stufe 17**.

---

## Parser/Lexer-Sonde und die letzten Leaks (Stufe 17)

Die bisherigen Runden hatten **Semantik** vermessen, nie **Syntax**. 32 Sonden über Literale,
Trenner, Kommentare, Lambdas, Präzedenz, Annotationen, Ranges, Pipes und Fehlerqualität. Dazu ein
Sweep über alle ~95 angesammelten Sondenprogramme, um eigene Regressionen zu finden.

**Ergebnis: von ~95 Sonden leakt noch genau eine** — rustcs `unconditional_panic`-Lint bei
Division durch eine Variable, die nachweislich 0 ist. Das ist ein *korrekter* Compile-Zeit-Fang,
nur in Rust-Worten; ihn selbst zu bauen bräuchte Konstantenpropagation. Division durch ein
**Literal** 0 lehnt der Typechecker jetzt ab.

### Lexer: drei Literalformen fehlten
`0xFF` lexte als `0` gefolgt vom Bezeichner `xFF` — der Nutzer sah „use of undeclared variable
`xFF`" für ein vollkommen gewöhnliches Literal. Ebenso `0b1010`, `1_000_000` und `1.5e3`
(→ „undeclared variable `e3`"). Alle vier Formen sind jetzt implementiert, inklusive `2E-4` und
`1_000.5`.

### Parser: Trailing Commas
`[1, 2, 3,]`, `{"a": 1,}`, `add(1, 2,)`, `fn f(int a, int b,)`, `Colour.Rgb(1, 2, 3,)` — alle
abgelehnt. Jetzt überall erlaubt.

**Dabei fast selbst einen Bug gebaut:** der erste, mechanische Patch setzte das `break` in drei
Schleifen **vor** das zugehörige `push` — die letzte Enum-Variante bzw. der letzte Match-Arm wären
stillschweigend verschwunden. Die bestehende Testsuite hat das sofort gestellt
(`test_e2e_enum_construction_qualified`, dessen Enum zufällig ein Trailing Comma hat). Danach jede
der 14 Einfügestellen einzeln auf die Push-Reihenfolge geprüft.

### Zwei Regressionen aus den eigenen vorherigen Runden
Der Sweep über die alten Sonden war der eigentliche Gewinn:

1. **`return` in einem Lambda** wurde der umgebenden Methode zugerechnet, also lehnte die neue
   „void gibt keinen Wert zurück"-Prüfung `(int x) => { return x * 3; }` in einer void-Methode ab.
2. **Der Pipe-Operator** reicht den Wert implizit als erstes Argument weiter; die neue
   Arity-Prüfung zählte ihn nicht mit und lehnte **jede** Pipeline ab.

### Und ein stiller Falschwert, den ich selbst eingebaut hatte
`abs(-3.7)` lieferte **3**. Der `abs`-Fix aus Stufe 13 wählt den Cast (`i64` vs `f64`) über
`resolve_type` — und das kannte **Unär-Operatoren nicht**, sah also bei `-3.7` keinen Float und
schnitt ab. Betroffen war auch jede Variable, die aus einem unären Ausdruck kam (`var f = -3.7`).

Warum das Golden-Netz es nicht fing: die Prüfung dort nutzte `abs(-5.0)`, und `5.0` formatiert als
`5` — die Erwartung war gegen Abschneiden **blind**. Das Programm nutzt jetzt `-3.7` und `-2.25`,
Werte, bei denen Abschneiden sichtbar wird. `resolve_type` versteht jetzt Unär und Cast.

### Weitere geschlossene Leaks
- Arithmetik auf einem unbehandelten `Result` (`parse_int("17") + 1`) — jetzt dieselbe Meldung wie
  bei `print`/Interpolation.
- Das Ergebnis einer In-place-Mutation binden (`var s = n.sort();`) — mit dem Hinweis, `sort` als
  Anweisung aufzurufen.
- Lambda mit untypisiertem Parameter in einer Variablen — abgelehnt mit dem Hinweis, den Typ zu
  schreiben oder das Lambda direkt an den konsumierenden Aufruf zu geben.
- `int? x` als Parameter (offener Punkt aus Stufe 15): Call-Sites adaptieren Argumente jetzt
  generell an den deklarierten Parameter — `Some(...)` für Nullable, `Box::new(...)` für Contracts.

### Kein Bug, entgegen früherer Notiz
Escapte Quotes in Interpolation: `$"{s.replace("-", "+")}"` funktioniert. Nur `\"` innerhalb der
Interpolation bricht — und das braucht man dort gar nicht. Die Notiz aus Stufe 14 war zu breit.

### Abgesichert
7 weitere Typechecker-Tests (Pipe in beide Richtungen, Lambda-Return-Scope, untypisiertes Lambda,
In-place-Mutation, Result-Arithmetik, Literal-Division durch 0, Nullable/Contract-Parameter) und
das Golden-Programm `syntax` mit 30 Prüfungen über Literale, Trenner, Escapes, Interpolation,
Präzedenz, Pipes, Compound-Assignment und Ranges.

### Stand danach
1227 Unit-Tests (default) / 1379 (`--features full`), 0 Failures · Golden **34/34**, drei Läufe
stabil · 11 Beispiele + 4 Spikes bauen · ~95 Sonden, 1 verbleibender rustc-Leak (s.o.).

---

## Der Rückwärts-Sweep wird ein CI-Schritt (Stufe 18)

Die letzte Runde hat gezeigt, dass der Sweep über **alle** angesammelten Sondenprogramme drei
Fehler fand, die keine Testsuite gemeldet hatte — zwei selbst eingeschleppte Regressionen und ein
stiller Falschwert (`abs(-3.7)` → 3). Ein manueller Schritt, der so viel findet, gehört automatisiert.

**Dabei kam heraus, dass die CI die Golden-Suite bisher gar nicht lief.** Das wichtigste
Sicherheitsnetz des Projekts war nicht in der Pipeline. Beide laufen jetzt.

### `probes/` — die Gegenrichtung zu `golden/`
| | prüft |
|---|---|
| `golden/` | gültige Programme kompilieren **und rechnen das Richtige** (Build + Run + stdout-Diff) |
| `probes/` | ungültige Programme werden **abgelehnt — und zwar vom Varg-Frontend, nicht von rustc** |

Der Prüfbefehl ist `vargc check` (Parse + Typecheck, kein Codegen, ~40 ms je Programm). Das macht
den Vertrag exakt: **akzeptiert `check` ein ungültiges Programm, erreicht der Fehler rustc** — und
genau diese Form hatte jeder gefundene Leak. 51 Sonden laufen dadurch in Sekunden statt Minuten.

### Jede Sonde nennt die erwartete Meldung
```
// @probe reject: takes 2 argument(s)
```
Ohne diese Angabe würde eine Sonde bestehen, solange *irgendetwas* fehlschlägt — und damit
stillschweigend eine Ablehnung aus völlig falschem Grund akzeptieren. Fehlt die Direktive, meckert
der Runner.

### Ausnahmen können nicht verrotten
`probes/known-rustc-leak/` enthält die eine dokumentierte Ausnahme (Division durch eine Variable,
die nachweislich 0 ist — rustcs `unconditional_panic`-Lint fängt das korrekt; selbst zu fangen
bräuchte Konstantenpropagation). Der Runner prüft **beide** Hälften: `check` muss durchlassen und
der Build muss scheitern. Lernt das Frontend es später, schlägt die Sonde mit
„NOW-CAUGHT — move it to reject/" fehl. Die Ausnahmeliste kann also nicht unbemerkt veralten.

### Negativ verifiziert
Jeder Fehlerpfad des Runners wurde durch absichtlich kaputte Sonden ausgelöst: `NOT-REJECTED`
(gültiges Programm), `WRONG-REASON` (falsche Meldung), `NO-DIRECTIVE` (fehlende Angabe) — alle
liefern Exit 1. Ein Runner, der nie fehlschlägt, wäre wertlos.

### Nebenbefund: Capability-Konstruktion
Die Sonde `ocap_construct_outside_unsafe` schlug sofort an. `var cap = FileAccess {};` außerhalb
von `unsafe` wurde akzeptiert — die Regel war nur für die **typisierte** Form
(`FileAccess cap = …`) durchgesetzt. **Kein ausnutzbares Loch:** der privilegierte Aufruf selbst
verlangt weiterhin ein echtes Token im Scope, `fs_write` wurde auch ohne `unsafe` abgelehnt
(verifiziert). Aber dokumentierte Regel und Durchsetzung waren auseinandergedriftet; jetzt
deckungsgleich.

### Stand danach
1228 Unit-Tests (default) / 1380 (`--features full`), 0 Failures · Golden **34/34** · Probes
**51/51** · CI-Jobs: 3 → **4**.

---

## Wave 22b — Agent Control Plane, in Varg geschrieben (Stufe 19)

Der Entwurf (`next_06_web_dashboard.md`) bot SvelteKit **oder** „Dashboard als Varg-Agent, der HTML
generiert". Entschieden: das Zweite. Ein natives Binary, kein Node, kein Build-Schritt — und vom
selben golden/probes-Netz gedeckt wie alles andere. Der Preis ist ehrlich: der Graph zeichnet
handgeschriebenes SVG statt D3/Cytoscape.

`dashboard/dashboard.varg` · vier Panels · 8,2 KB Seite · Auto-Refresh 2 s.

### Das einzige wirklich fehlende Stück: eine Agent-Registry
„Live-Status" wäre gelogen gewesen, wenn ihn das Programm selbst melden müsste. Also führt ihn der
**generierte Dispatcher**: jedes `spawn` registriert seinen Agenten, und der Zustand wandert
`starting → idle → running → idle → … → stopped` mit. Prozessglobal statt handle-basiert, weil ein
gespawnter Agent sich aus seinem eigenen Thread eintragen muss.

Neue Builtins: `agents_list()`, `agents_count()`, `agents_count_by_status(s)`.

### Nebenbefund mit Gewicht: ein Panic riss den ganzen Prozess mit
Der globale Panic-Hook rief `process::exit(1)` — aus **jedem** Thread. Eine kaputte Nachricht an
einen Agenten beendete damit das gesamte Programm, und das `catch_unwind` im Dispatcher kam nie zum
Zug. Der Hook beendet jetzt nur noch, wenn der **Haupt**-Thread gefallen ist; ein Agent wird
stattdessen als `error` mit der Panic-Meldung vermerkt und bedient die nächste Nachricht weiter.
Verifiziert: Agent überlebt, Fehler protokolliert, Exit 0.

### Drei Ergonomie-Lücken, beim Bauen aufgelaufen
| Lücke | Status |
|---|---|
| ~~Interpolation kennt kein Escaping für literale Klammern~~ — **Einschätzung war falsch**: `\{`/`\}` gab es längst, nur undokumentiert. Siehe Stufe 20 | korrigiert in Stufe 20 |
| ~~Interpolation verträgt kein String-Literal als Argument~~ — **auch das war zu breit**: plain quotes funktionieren, nur die *escapte* Form bricht | korrigiert in Stufe 20 |
| Eine `fn` **nach** einem Agenten war für diesen unsichtbar | **behoben** — der Typechecker registriert Standalone-Funktionen jetzt in einem Vorlauf, wie der Codegen längst |

### Was die Architektur formt
Route-Handler sind `Fn`-Closures und erreichen `self` nicht. Alles, was sie ausliefern, ist deshalb
entweder ein gecapturetes Laufzeit-Handle (die sind `Arc`-basiert, bleiben also live — daher zeigen
die Panels aktuelle Daten) oder ein vor dem Serverstart berechneter Wert. Die HTML-Seite ist
Letzteres: einmal gerendert, gecaptured, aus dem Speicher bedient.

### Abgesichert
Das Dashboard selbst kann kein Golden-Programm sein — es endet in `http_listen` und blockiert.
Gepinnt wird stattdessen der Vertrag, von dem die Panels abhängen:
`golden/progs/dashboard_payloads.varg` prüft die Form aller vier Payloads (23 Prüfungen). Ein
formverändertes Payload bräche sonst ein Panel stumm im Browser, wo kein Test hinsieht. Dazu 6
Unittests für die Registry.

Im Browser verifiziert: alle vier Panels mit Live-Daten, Auto-Refresh, SVG-Graph mit Kanten-Labels,
Span-Timeline mit Verschachtelung, Dark und Light Mode, mobil ohne horizontales Scrollen, keine
Konsolenfehler.

### Stand danach
1234 Unit-Tests (default) / 1386 (`--features full`), 0 Failures · Golden **35/35** · Probes 51/51.

---

## Interpolation: eine Korrektur und zwei echte Funde (Stufe 20)

**Zuerst die Korrektur an mir selbst.** In Stufe 19 hatte ich notiert, Interpolation könne keine
literalen Klammern und damit kein JSON. Beim Nachmessen: `$"\{\"used\": {n}\}"` funktionierte die
ganze Zeit — die Backslash-Form war implementiert, nur nirgends dokumentiert. Ebenso war
„verträgt kein String-Literal als Argument" zu breit: `$"{s.replace("-", "+")}"` läuft; nur die
*escapte* Schreibweise `\"` bricht, und die braucht man dort gar nicht. Beide Einträge sind in
Stufe 19 durchgestrichen.

Übrig blieben damit ein Idiomatik-Problem und zwei echte Defekte.

### `{{` und `}}` ergänzt
Varg wirbt mit C#-ähnlicher Syntax, und in C# verdoppelt man die Klammer. Genau das greift man,
wenn man JSON aus einem interpolierten String schreiben will — und es schlug fehl. Jetzt
unterstützt, die Backslash-Form bleibt gültig.

### Der eigentliche Fund: Interpolation **ohne** Ausdruck gab die Maskierung roh aus
`$"{{a}}"` druckte `{{a}}`. Der Codegen verdoppelt Klammern für `format!` — emittiert einen String
ohne Ausdrücke aber als **einfaches Literal**, wo diese Verdopplung nie aufgelöst wird. Ein
vorbestehender Bug, der die ganze Zeit dalag: mit einem Ausdruck drin (`$"\{a\} {n}"`) lief alles
über `format!` und war korrekt, ohne Ausdruck war es falsch. Genau die Art Fall, die man nur
findet, wenn man beide Varianten prüft — mein erster Golden-Test hatte nur die mit Ausdruck.

### Zwei Fehlermeldungen brauchbar gemacht
- Escapte Quotes in einer Interpolation warfen den zerstückelten Text zurück. Jetzt: „quotes inside
  an interpolation are not escaped — write {…}" mit der korrigierten Fassung. Als Sonde abgesichert.
- Eine nicht geschlossene Interpolation (`$"a { b"`) meldet weiterhin „unexpected end of file".
  **Bewusst so gelassen:** das scheitert schon im *Lexer*, der Klammer-Tiefe zählt, um Quotes in
  Ausdrücken zu erlauben — die Parser-Ebene sieht den Fall nie. Eine bessere Meldung bräuchte einen
  Fehlerpfad durch den Logos-Callback; der Aufwand steht nicht zum Nutzen.

### Dogfooding
Die beiden Payload-Builder im Dashboard sind von Konkatenation auf Interpolation umgestellt — aus
fünf Zeilen wird eine, und der Fix ist damit in einem echten Programm belegt, nicht nur im Test.
Nebenbei aufgefallen und behoben: der Demo-Graph wuchs bei jedem Neustart (jetzt nur noch geseedet,
wenn leer), und `graph_open("x.graph.db")` legt `x.graph.db.graph.db` an — die Runtime hängt das
Suffix selbst an, der Name im Programm ist jetzt entsprechend bloß `dashboard`.

### Abgesichert
4 Prüfungen im Golden-Programm `syntax` (beide Klammer-Formen, JSON-Fall, String-Argument) und eine
Sonde für die neue Fehlermeldung. Der No-Expression-Bug hätte sich sonst wieder verstecken können —
er trat nur ohne Ausdruck auf.

### Stand danach
1234 Unit-Tests (default), 0 Failures · Golden 35/35 · Probes **52/52**.

---

## Wave 23 — Agent als MCP-Tool-Provider (Stufe 21)

Vorher fehlte weniger, als der Entwurf vermuten ließ: `@[McpTool]` erzeugte **schon** ein
vollständiges input- **und** outputSchema aus der Methodensignatur und einen CLI-Dispatch-Arm.
Was fehlte, war das **Protokoll** — es gab keinen Weg, wie ein echter MCP-Client die Methoden
aufruft.

**Jetzt:** `./prog --mcp-serve` spricht JSON-RPC über stdio (`initialize`, `tools/list`,
`tools/call`) und dispatcht auf die annotierten Methoden. Argumente kommen **namentlich** und
werden in den deklarierten Parametertyp geparst; ein Struct-Rückgabewert wird als JSON
serialisiert, `void` antwortet `ok`, ein unbekanntes Tool bekommt einen JSON-RPC-Fehler und die
Verbindung bleibt nutzbar.

### Aufteilung: Protokoll in die Runtime, Dispatch in den Codegen
Die Envelopes und das Parsen liegen in `mcp_server.rs` (für jedes Programm gleich, dort direkt
testbar — 7 Unittests). Der **Dispatch** muss generiert werden: er ruft eine Methode auf dem
Entry-Agenten auf, und ein `Fn + Send + Sync`-Handler könnte kein `&mut` darauf halten. Die neuen
Helfer nutzen `serde_json` statt der älteren Substring-Extraktion daneben — ein echter Client
schickt verschachtelte Objekte und escapte Strings, an denen Substring-Suche scheitert.

### `exe_path()` ergänzt
`args()` überspringt bewusst argv[0], ein Programm konnte seine eigene Binary also nicht finden.
Damit ließ sich das Muster „starte dich selbst im Servermodus" nicht schreiben — und genau das
macht den Test möglich.

### Der Test ist beide Hälften in einem Programm
`golden/progs/mcp_server_mode.varg` ist Client **und** Server: normal gestartet spawnt es sich
selbst mit `--mcp-serve` und ruft über Vargs eigenen MCP-Client seine eigenen Tools auf. Ein
Golden-Programm deckt damit den vollen Rundlauf ab, ohne zweite Binary und ohne Netz — inklusive
Int-Argumenten, Quotes im String-Argument, Struct-Rückgabe, unbekanntem Tool und der Prüfung, dass
die Verbindung den Fehler übersteht.

### Tool-Argumente: erst falsch gelöst, dann richtig
Erster Versuch: `ToToolArgs` zusätzliche Impls für Maps mit Zahl-/Bool-Werten geben. Das machte
`{}` — „dieses Tool nimmt keine Argumente" — **mehrdeutig**, weil Rust den Wertetyp nicht mehr
inferieren konnte, und löste den gemischten Fall (`{"query": "x", "top_k": 3}`) ohnehin nicht.
Zurückgenommen.

Der Denkfehler war, das Problem in der **Runtime** lösen zu wollen. Ein Tool-Argument ist ein
JSON-Objekt, keine Varg-Map — also gehört die Umsetzung in den **Codegen**: ein Map-Literal in
Argumentposition wird zu einem `serde_json::Map` aufgebaut, Eintrag für Eintrag, jeder Wert über
`json!` einzeln. Damit behält jeder Wert seinen eigenen Typ, `{}` ist ein `Value` statt einer
HashMap unbestimmten Typs, und die Runtime braucht genau **eine** Impl (`for serde_json::Value`).

Gemischte Literale gehen jetzt so, wie man sie schreibt. Beide Altformen (roher JSON-String,
Variable mit String-Map) bleiben gültig — im Golden-Programm mitgeprüft, weil genau sie stumm
brechen würden.

### Stand danach
1241 Unit-Tests (default), 0 Failures · Golden **36/36** · Probes 52/52.

---

## Die drei Funde aus der Usecase-Sonde (Stufe 22)

Ein realistisches Agenten-Programm pro beworbenem Usecase gebaut, statt zu schätzen. Was trägt:
Knowledge Graph + Vector Store + RAG, MCP Client **und** Server, Actor-Model mit Registry,
HTTP-Server, Observability, Cost, Memory. Was hakte, waren drei Stellen — alle „dokumentiert ≠
implementiert".

### 1. LLM-Fehler waren unsichtbar — der schwerste der drei
`__varg_llm_infer` endete auf `parse_response(&res).unwrap_or(res)`. Scheiterte der Aufruf, kam
die Fehlermeldung des Providers **als Antwort** zurück:

```
agent would store: [{ "error": "Network error: ... 127.0.0.1:11434 ..." }]
```

Ein Agent legte den Netzwerkfehler als Modellantwort ins Memory und arbeitete damit weiter. `or`
und `?` kompilierten nicht, weil der Rückgabetyp `String` war. Dieselbe Klasse wie `parse_int` →
stumme 0, nur im **Kernversprechen** der Sprache.

`llm_infer` und `llm_chat` liefern jetzt `Result<string, string>`; der Fehler trägt die
Provider-Antwort, damit der Grund erhalten bleibt. `llm_chat` lässt den Kontext bei einem Fehler
mit dem User-Turn und **ohne** Assistant-Turn zurück — ein Retry wiederholt also keine
Phantom-Antwort. Beide sind in der Signatur-Tabelle (Drift-Lock greift) und in der
`NETWORK_ONLY`-Ausnahmeliste, weil ohne Provider nichts Deterministisches prüfbar ist.

### 2. DI funktionierte überall außer im Entry-Agenten
`agent Main { public Main(ILog l) … }` leakte rustcs „no associated item `new`". Zwei Ursachen:
`main()` rief `Name::new()`, während der Codegen den Konstruktor unter dem **Agentennamen**
emittiert — und der Entry-Point-Picker hielt einen Konstruktor namens `Main` für die
Einstiegs*methode* und rief `instance.Main()`.

Jetzt: parameterloser Konstruktor läuft vor `Run()`; ein Konstruktor **mit** Parametern wird mit
einer Meldung abgelehnt, die den funktionierenden Weg nennt (DI eine Ebene tiefer). Konstruktoren
sind aus der Entry-Methoden-Auswahl ausgeschlossen.

### 3. `import crate` — die dokumentierte Form ging nicht
`import crate serde_json;` (genau so in REFERENCE) → „expected Assign": die Version war Pflicht.
Mit Version → „duplicate key", weil serde_json schon im generierten `Cargo.toml` steht. Also
scheiterte ausgerechnet der Import der gewöhnlichsten Crates.

Version ist jetzt optional (`*` = neueste kompatible). Immer vorhandene Crates werden übersprungen;
die **bedingt** vorhandenen (tokio, chrono, rand) werden erst dedupliziert, wenn feststeht, ob sie
überhaupt eingezogen wurden — sonst hätte ein `import crate tokio;` in einem nicht-async-Programm
die Abhängigkeit weggekürzt, die der generierte `use tokio;` braucht.

### Neue Sondenkategorie: `probes/reject-at-build/`
Punkt 2 ist eine Eigenschaft der Codegenerierung, nicht der Typen — `vargc check` kann sie nicht
sehen. In den Typechecker gehört sie nicht: ein **Nicht**-Entry-Agent darf Konstruktor und `Run`
gleichzeitig haben, die Regel dort wäre ein False Positive. Also prüft der Runner jetzt eine dritte
Kategorie: `check` akzeptiert, `build` lehnt ab, **und** die Meldung ist unsere, nicht rustcs.
Negativ verifiziert (WRONG-REASON schlägt an).

### Stand danach
1241 Unit-Tests (default) / 1393 (`--features full`), 0 Failures · Golden **37/37** · Probes
**54/54**.

### Was die Sonde nicht abgedeckt hat
Multimodal, Checkpoint/Resume, HITL, Rate-Limiting und der Self-Improve-Loop blieben ungetestet.
Nach der Trefferquote hier (drei Funde in einem Querschnitt) wären dort weitere zu erwarten.

---

## Sonde über die restlichen Bereiche (Stufe 23)

Multimodal, Checkpoint/Resume, HITL, Rate-Limiting, Self-Improve — die fünf, die Stufe 22
ausdrücklich **nicht** abgedeckt hatte. Erwartung war „weitere Funde"; sie hat sich bestätigt.

### Was trägt
Checkpoint/Resume (voller Rundlauf inkl. `age`/`clear`), Self-Improve (record/recall/stats),
Multimodal-Base64-Rundlauf, und HITL: kompiliert, rendert die Prompts und degradiert bei
geschlossenem stdin sauber (`approval=false`, `choice=-1`, Exit 0) statt zu hängen.

### Zwei echte Bugs
| Bug | Wirkung |
|---|---|
| **`rate_limit_reset` löschte den Limiter** (`remove(key)`), statt ihn aufzufüllen. `try_acquire` liefert für einen unbekannten Key `false` — also verweigerte der Limiter nach einem Reset **für immer**, ununterscheidbar von „gedrosselt". | stilles Fehlverhalten |
| **`image_load`/`audio_load` bei fehlender Datei**: `fs::read(..).unwrap_or_default()` ergab ein **Null-Byte-Bild** mit aus der Endung geratenem Format. Ein Tippfehler im Dateinamen reiste unbemerkt bis zum Vision-Modell. | stiller Falschwert |

Beide gefixt: Reset füllt den Bucket auf; Laden liefert `Result`.

### Drei Doku-Abweichungen
- `ratelimiter_acquire(rl, "user_123") // bool` — real **ein** Argument, **blockiert**, liefert
  nichts. Per-Key-Buckets, wie die Doku sie suggeriert, gibt es nicht: ein Bucket pro Limiter.
  `try_acquire` ist die berichtende Variante.
- `image_load("photo.png", cap)` — real ein Argument; die Capability muss im Scope sein, nicht
  übergeben werden (wie bei `fs_read`). **Kein OCAP-Loch** — verifiziert, dass der Aufruf ohne
  Token abgelehnt wird.
- `checkpoint_open` braucht `FileAccess`; die Doku zeigte kein `unsafe`.
- Dazu präzisiert: der `@[RateLimit]`-Bucket ist nach **Methode und Thread** geschlüsselt, nicht
  nach einem Aufrufer-Key. Jeder gespawnte Agent hat sein eigenes Kontingent — das drosselt einen
  Worker, nicht das Programm gegenüber einer API.

### Ein Drift, den erst die neue Prüfung sichtbar machte
`image_load` fallibel zu machen erzeugte einen **neuen** rustc-Leak: ein unbehandeltes `Result`
als Builtin-**Argument**. Statt das punktuell zu flicken, ist daraus die allgemeine Regel geworden
(dieselbe Form wie bei `print` und Arithmetik). Beim Messen fiel sie sofort über `json_parse`:
der Typechecker typisierte es als `Result`, der Codegen emittiert `unwrap_or(Value::Null)` — die
`Result`-Typisierung war **nie** benutzbar (`json_parse(x) or d` konnte gar nicht kompilieren) und
blieb unsichtbar, bis ein unbehandeltes Result zum Fehler wurde. Typ an die Realität angeglichen.

Wieder haben zwei Tests das alte Verhalten festgeschrieben (`test_image_load_missing_path`
behauptete das stille Leerbild, `test_stdlib_json_parse_type` den Drift) — beide korrigiert.

### Offen gelassen
`json_parse` liefert bei kaputtem JSON still `Null`. Das ist bewusst so gebaut (die Accessoren
sollen ohne Unwrap funktionieren) und nehmen auch rohe Strings — es fallibel zu machen wäre eine
Design-Entscheidung, keine Bugfix. Notiert, nicht geändert.

### Schritt 3 — `json_parse` fallibel

Gleiche Frage eine Ebene hoeher: `from_str(..).unwrap_or(Value::Null)` machte aus einem kaputten
Dokument ein leeres. Jede spaetere Lesung meldete die Schluessel dann als schlicht abwesend — der
Parse-Fehler war **nicht beobachtbar**. Jetzt `Result<JsonValue, string>` mit serdes Meldung
(Zeile/Spalte). Behandlung: `?` (Auto-Wrapping ueber den Erfolgstyp), `.is_err()`, oder gar nicht
parsen — die Accessoren nehmen rohe JSON-Strings ohnehin.

Beim Umbau der Aufrufstellen gemessen: von 5 `json_parse`-Aufrufen im Repo waren **4 ueberfluessig**.

Zwei Tests schrieben wieder das alte Verhalten fest — `test_codegen_json_parse` verlangte
woertlich `unwrap_or`, also den stillen Default; `test_stdlib_json_parse_type` hatte den Drift
schon zweimal protokolliert (erst Result ohne Codegen-Deckung, dann korrigiert auf den blanken
Wert, was den Default festnagelte). Beide korrigiert; damit sind es **sieben** Tests in diesem
Projekt, die den Bug beschrieben statt ihn zu finden.

### Beifang: ein flakiger Test

`counts_by_status_track_transitions` bestand einzeln und fiel im Gesamtlauf — die Agent-Registry
ist prozessglobal, und ein Zaehler ueber die *ganze* Registry laesst sich nicht vorher/nachher
lesen, waehrend parallele Tests Statuswechsel einstreuen. Die statusaendernden Tests des Moduls
teilen sich jetzt eine Mutex.


### Stand danach
1241 Unit-Tests (default) / 1394 (`--features full`) · Golden **38/38** · Probes **55/55**.

---

## Nullable statt eingebackener Defaults (Stufe 24)

Stufe 23 hatte notiert, dass die JSON-Accessoren fünf Situationen auf **eine** Antwort abbilden.
Diese Stufe hat das aufgelöst — in der Reihenfolge 1-2-3, weil Schritt 2 ohne Schritt 1 eine
Falle gewesen wäre.

### Schritt 1 — `or` auf Option reparieren

`or` lowerte immer zu `unwrap_or_else(|_| …)`. Das passt zu `Result` (der Fehler geht in die
Closure), nicht zu `Option` (die Closure bekommt nichts). Solange das nicht ging, war jedes
Nullable in Varg unbenutzbar: der einzige Weg, es aufzulösen, kompilierte nicht.

Dabei fiel ein Drift auf, den kein Test sah: der Typechecker typisierte `first`/`last` als
**Element**, der Codegen emittiert `.first().cloned()` — also ein `Option`. `print xs.first()`
gab die ganze Zeit `Some(...)` aus. Der Test dazu (`test_first_returns_element_type`) behauptete
den falschen Typ und hielt den Drift damit unsichtbar; korrigiert. `pop` bleibt Element-typisiert,
weil der Codegen es tatsächlich auspackt.

### Schritt 2 — Die Accessoren entkoppeln

`json_get` filterte mit `as_str()` und defaultete auf `""`. Damit waren ununterscheidbar:
fehlender Schlüssel, Zahl, Bool, verschachteltes Objekt, kaputtes Dokument — und ein echt leerer
String. `json_get_array` verwarf zusätzlich still jedes Nicht-String-Element, machte also aus
`[1, 2]` ein leeres Array.

Zwei orthogonale Defekte, zwei Antworten:

| | vorher | jetzt |
|---|---|---|
| Wert fehlt | `""` | `null` — die **einzige** Situation, die `null` ergibt |
| Wert ist Zahl/Bool/Objekt | `""` | rendert als Text (`42`, `true`, `{"b":1}`) |
| Wert ist leerer String | `""` | `""` — ein vorhandener Wert, unterscheidbar von `null` |
| Array mit Zahlen | `[]` | `["1", "2"]` |
| `json_get_int` auf `"42"` | `0` | `null` — ein String ist keine Zahl |

### Drei rustc-Leaks, die die Umstellung sichtbar gemacht hat

Nullable war im Compiler nur halb verdrahtet. Was vorher nicht auffiel, weil es kaum ein Nullable
gab, traf jetzt jede Zeile:

1. **Verkettung**: `"x: " + json_get(d, "k")` erreichte rustc als Display-Trait-Fehler über
   `Option<String>`. Jetzt über `__varg_fmt` gerendert.
2. **Arithmetik/Vergleich**: `json_get_int(a, "n") * 2` → *„cannot multiply `Option<i64>` by
   `{integer}`"*. Jetzt eine Typechecker-Meldung, die auf `or` zeigt — gleiche Regel wie beim
   unbehandelten `Result`. `== null` und `!= null` bleiben erlaubt, das ist Vargs Art zu fragen.
3. **Darstellung**: ein Optional rendert über Rusts `Debug`, also `Some("x")` / `None` — die
   Vokabeln der Wirtssprache in der Ausgabe einer Sprache, die dasselbe `x == null` schreibt.
   Jetzt: der Wert, oder `null`.

### Ein Diagnose-Bug als Beifang

Der Fehler-Span wird ermittelt, indem der Typechecker den Namen **im Quelltext sucht** (das AST
hat keine Spans pro Ausdruck) — mit einem blanken `str::find`. Also traf ein Fehler über `check`
das Wort *checked* in einem Doc-Kommentar 36 Zeilen weiter oben, und der Caret zeigte auf Prosa.
Die Suche überspringt jetzt Kommentare und String-Literale und verlangt Bezeichner-Grenzen.

### Offen — und größer als es aussieht *(erledigt in Stufe 25)*

**Lambda-Rümpfe werden gar nicht typgeprüft.** Gefunden, weil der neue Nullable-Guard in einem
MCP-Handler nicht griff. Gemessen: undeklarierte Variablen, `5 > "x"`, `true + 1` — innerhalb
eines Lambdas meldet der Typechecker **nichts**. Das trifft Varg an der empfindlichsten Stelle,
denn `http_route`, `ws_route`, `mcp_server_register`, Pipeline-Schritte und `map`/`filter` sind
alle Lambdas: genau dort, wo Agenten-Code steht. Bewusst nicht in dieser Stufe angefasst — das
Binden untypisierter Parameter ohne Falschmeldungen ist eine eigene, sorgfältige Runde.

### Stand danach
1243 Unit-Tests (default) / 1396 (`--features full`) · Golden **38/38** · Probes **58/58** ·
18 Programme (examples + spikes + dashboard) bauen.

---


## Lambda-Rümpfe typprüfen (Stufe 25)

Der offene Punkt aus Stufe 24. Die Erwartung war "Lambda-Rümpfe werden nicht besucht" — die
Messung ergab etwas anderes und Wichtigeres.

### Die Ursache war nicht, was sie zu sein schien

Die Rümpfe **wurden** die ganze Zeit gelaufen (`Expression::Lambda` ruft `check_block`). Was
fehlte, war eine Zeile weiter oben: Der Walk in Caller und Argumente eines Aufrufs war seinerzeit
eingebaut worden, um ein **OCAP-Schlupfloch** zu schließen (`list.map((x) => fs_read(x))` erreichte
`check_ocap` nie). Er meldete deshalb *nur* Capability-Verletzungen und verwarf alles andere —
ausdrücklich, damit vorher gültige Programme gültig bleiben. Dieses `_ => {}` warf die
Lambda-Fehler gleich mit weg.

### Der Fund war größer als Lambdas

Dieselbe Zeile verschluckte **jeden** Fehler in einem Builtin-Argument. Gemessen:

```
print BOOM;                 -> gefangen
print to_upper(BOOM);       -> ging durch
print to_upper(trim(BOOM)); -> ging durch
```

`to_upper(BOOM)` kam sauber durch den Typechecker und wurde zu `self.to_uppercase()` kompiliert —
rustc meldete dann, `&mut A` habe keine solche Methode. Eine Meldung über generierten Code, die
weder den undeklarierten Namen nennt noch irgendetwas, das im Quelltext steht.

Auf der **Caller**-Seite dasselbe: `zs.map((m) => BOOM).len()` war blind, `zs.map((m) => BOOM)`
nicht — der Fehler verschwand, sobald das Ergebnis Empfänger eines weiteren Aufrufs wurde.

### Die Regel

Eine Funktion, an einer Stelle formuliert (`error_is_not_an_inference_gap`), für Caller und
Argumente gleichermaßen. Ein Fehler wird gemeldet, wenn er **keine Inferenzlücke** ist:

- **alles aus einem Lambda-Rumpf** — ein Lambda-Argument ist kein Wert mit unsicherem Typ,
  sondern ein Block eigenen Codes;
- **jeder unauflösbare Name** — Namensauflösung ist keine Inferenz.

Alles Übrige bleibt verworfen, damit vorher gültige Programme gültig bleiben. Eine Ausnahme war
nötig: freie Builtins werden als Methodenaufruf auf einem **synthetischen `self`** geparst, das
außerhalb einer Agent-Methode (`fn main()`) auf nichts auflöst — ohne die Ausnahme wäre jeder
Builtin-Aufruf in einer normalen Funktion ein Fehler über einen Empfänger geworden, den niemand
geschrieben hat. Genau das haben vier Beispielprogramme sofort gemeldet.

### Was jetzt greift

Fehlerklasse × Position, jeweils auf Methodenebene und im Lambda gemessen — beide Spalten
identisch: undeklarierte Variable, `5 > "x"`, `true + 1`, unbehandeltes `Result`,
Nullable-Arithmetik, String-Methode auf einer Zahl, Division durch 0, unbekanntes Feld.

Positionen: Builtin-Methode, freies Builtin (`http_route`), User-Methode, freie Funktion,
Block-Rumpf, Ausdrucks-Rumpf, verschachtelte Lambdas (dreifach), Array-Literal, Rückgabewert,
`retry`, `try/catch`, `if`, `foreach` — alle gefangen.

### Preis: 36 Test-Fixtures

Die Unit-Tests bauen AST-Fragmente von Hand, mit `Expression::Identifier("srv")` als Argument,
ohne dass `srv` je deklariert wurde. Sie liefen nur durch, **weil** der Verwurf sie deckte. Jetzt
sagen die Fixtures, was sie immer meinten (`checker_with(&["srv"])`). Zwei bauten ein ganzes
Programm, in dem an ein undeklariertes `worker` gesendet wurde — dort wird der Agent jetzt
gespawnt, wie das Programm es die ganze Zeit beschrieb.

### Stand danach
1243 Unit-Tests (default) / 1396 (`--features full`) · Golden **38/38** · Probes **62/62** ·
18 Programme bauen. Keine einzige Falschmeldung über Golden, Beispiele, Spikes und Dashboard.

---


## Die Arity-Lücke bei Empfänger-Builtins (Stufe 26)

Der offene Punkt aus Stufe 25: `to_upper()` ging auf jeder Ebene durch. Beim Nachmessen war es
nicht eine Lücke, sondern zwei — und die größere war nicht die Arity.

### Die freie Schreibweise existiert gar nicht

String- und Collection-Builtins sind **Methoden auf dem Wert**. `to_upper("abc")` — die Form, die
in den meisten Sprachen richtig wäre — kompilierte zu `self.to_uppercase()`, weil freie Builtins
mit synthetischem `self`-Empfänger geparst werden. rustc meldete dann *„no method named
`to_uppercase` for `&mut A`"*: eine Beschwerde über generierten Code, über einen Empfänger, den
niemand geschrieben hat.

Über die Familie gemessen: **11 Schreibweisen leakten rustc**, 6 lieferten eine Arity-Meldung über
die *Methoden*form (`replace("a","b","c")` → „erwartet 2 Argumente, gefunden 3"), die weder den
eigentlichen Fehler benennt noch einen Ausweg. Genau eine trägt wirklich: `len`/`length`.

### Überzählige Argumente wurden still verworfen

`"a".to_upper("x")` kam durch den Typechecker **und lief**. Diese Zweige geben einen Typ zurück,
ohne `args` je anzusehen, und der Codegen wirft das Argument weg. Betraf 15 Builtins.

### Die Regel

Am vorhandenen `RECEIVER_METHODS`-Block, zwei Prüfungen und eine Ausnahme:

- Empfänger ist das synthetische `self` → *„`to_upper` ist eine Methode auf dem Wert, keine freie
  Funktion — schreib `value.to_upper()`"*. Die Meldung zeigt die richtige Aufrufform, mit oder
  ohne Argumentpunkte, je nach Builtin.
- Echter Empfänger und ein Builtin ohne Argumente → *„nimmt keine Argumente, aber 1 wurde
  übergeben"*.
- `len`/`length` sind ausgenommen und bekommen pro Schreibweise ihre eigene Zahl: `len(xs)` nimmt
  den Wert, `xs.len()` nimmt nichts.

Der **Pipe-Operator bleibt unberührt** — `name |> to_upper()` schiebt den linken Wert als Empfänger
ein, ist also die Methodenform. Das war der Fall, der beim Umbau hätte kaputtgehen können; er ist
jetzt in `golden/progs/builtins_string.varg` festgenagelt, zusammen mit `len` in beiden Formen.

### Die Regel aus Stufe 25 musste mitwachsen

Die ersten drei Fälle (`chars(...)`, `keys(...)`, `split(...)`) leakten weiter — sie standen in
Ketten wie `chars("abc").len()`, wo der Fehler im **Caller** sitzt und von
`error_is_not_an_inference_gap` verworfen wurde. Die beiden neuen Varianten sind dort ergänzt: eine
Methode als freie Funktion aufzurufen ist definitiver Fehlgebrauch, keine Inferenzlücke.

### Und wieder wich die Doku ab

`VARG_AGENT_GUIDE.md` zeigte die freie Form an **vier** Stellen (`trim(item)`, `to_upper(cleaned)`,
zwei Pipeline-Schritte) — keine davon kompilierte je. Korrigiert.

### Stand danach
1243 Unit-Tests (default) / 1396 (`--features full`) · Golden **38/38** · Probes **66/66** ·
18 Programme bauen.

---


## Builtins, die eine Antwort erfunden haben (Stufe 27)

Die Liste aus Stufe 24. Beim Nachmessen war sie länger — und zwei Einträge waren keine stillen
Defaults, sondern schlimmer.

### Der klarste Fall

`split_once("ab", "=")` lieferte `("", "")`. Genau dasselbe liefert `split_once("=", "=")` —
das erfolgreiche Trennen in zwei leere Hälften. **Erfolg und Misserfolg waren dieselben zwei
Werte.** Jetzt Nullable.

### Kein Default, ein Absturz

`time_format(0, "%Q")` **nahm das Programm mit runter**: chrono paniert aus seiner
`Display`-Implementierung heraus bei einem unbekannten Spezifikator („a Display implementation
returned an error unexpectedly"). Das Muster wird jetzt vorher geprüft.

### Kein Default, ein Datenverlust

`json_set("not json", "a", "1")` gab `{"a":1}` zurück. Das unparsbare Dokument wurde durch ein
leeres ersetzt, und der Schreibvorgang meldete Erfolg — der Aufrufer glaubt, er hat sein Dokument
geändert; zurück kam ein anderes, in dem alles Übrige fehlt. `json_merge` verwarf die kaputte
Seite still, was aussah wie ein Merge, der nichts zu tun hatte. Beide melden jetzt, **welche**
Seite nicht parst (dafür kam `try_as_json` dazu: `as_json` macht aus kaputtem Text `Value::Null`,
was für die *lesenden* Accessoren richtig ist, für die *schreibenden* aber „ist kein Objekt"
statt „parst nicht" ergeben hätte).

### Die Aufteilung

| Nullable — „da ist nichts" | Fallibel — „etwas ging schief" |
|---|---|
| `char_at` (Index hinter dem Ende) | `time_format` (ungültiges Muster) |
| `split_once` (kein Trenner) | `json_set` / `json_merge` (Dokument parst nicht) |
| `path_parent` / `path_extension` / `path_stem` | `exe_path` |
| | `readline_read` (EOF/Ctrl-C — das `Result` gab es längst, der Codegen warf es weg) |

`json_stringify`/`_pretty` behalten ihren Default und sagen jetzt im Code **warum**: ein
`serde_json::Value` kann nicht scheitern (weder Nicht-String-Schlüssel noch NaN/Inf sind darin
darstellbar). Ein dokumentiert unerreichbarer Default ist etwas anderes als ein übersehener.

### Fünf undokumentierte Dubletten entfernt

`file_read`, `file_write`, `to_json`, `from_json`, `time_now`: **null** Doku-Zeilen, **null**
Aufrufe in irgendeinem Programm — und jede eine schlechtere Ausgabe der dokumentierten Variante.
`file_read` gab die **Fehlermeldung als Dateiinhalt** zurück, `file_write` rief `.unwrap()`.
OCAP griff immerhin bei beiden (geprüft). Sie werden jetzt namentlich abgelehnt, mit der
Ersatz-Funktion in der Meldung — nicht kommentarlos gelöscht. Präzedenz: die Lügen-SSE-API in
Stufe 9.

Dabei mussten die OCAP-Tests umziehen: `file_read`/`file_write` waren dort ausgerechnet die
Musterbeispiele für Capability-Prüfung. Jetzt auf `fs_read`/`fs_write`, die dieselbe
`FileAccess`-Prüfung durchlaufen — und deren Fallibilität die Fixture gleich mit sichtbar machte.

### Beifang: der zweite flakige Test

Wie schon `counts_by_status_track_transitions` bestand `test_provider_detect_claude_alias` isoliert
und fiel im Gesamtlauf: vier Tests setzen dieselbe **Umgebungsvariable** `VARG_LLM_PROVIDER` und
löschen sie wieder, parallel. Elf umgebungsberührende Tests des Moduls teilen sich jetzt eine Mutex.

### Stand danach
1243 Unit-Tests (default) / 1396 (`--features full`) · Golden **38/38** · Probes **71/71** ·
18 Programme bauen.

---


## Programme über mehrere Verzeichnisse (Stufe 28)

Die Frage war, ob sich in Varg ein **vollwertiges System** schreiben lässt und nicht nur ein
Programm. Also wurde eines geschrieben: 206 Zeilen, eine Domänenschicht, eine Speicherschicht,
Agenten mit Nachrichten, Konfigurationskaskade, Tracing und eine HTTP-API — verteilt auf
`domain/`, `store/`, `agents/`, `api/` und eine Einstiegsdatei.

Es ließ sich zuerst **nicht übersetzen**, und zwar an sechs unabhängigen Stellen. Alle sechs
betrafen genau die Konstruktionen, mit denen ein Programm über eine Datei hinauswächst. Jede ist
jetzt gefixt und liegt als laufendes Golden-Programm (`system_wiring`) im Netz.

### 1. Jedes Verzeichnis war eine Insel

`store/repo.varg` mit `import domain.task;` schlug fehl: Importe wurden **nur** gegen das
Verzeichnis der importierenden Datei aufgelöst. Ein Modul konnte damit ausschließlich seine
eigenen Nachbarn sehen — das Gegenteil dessen, wofür man ein System auf Verzeichnisse verteilt.
Jetzt wird zusätzlich gegen das Verzeichnis der Einstiegsdatei gesucht, Nachbar zuerst.

### 2. Ein Parameter verlor gegen ein gleichnamiges Feld — still

Der schlimmste Fund der Runde, weil er kompiliert, läuft und die **falsche Antwort** gibt:

```
agent A { string name; public void greet(string name) { print "hello " + name; } }
a.greet("the argument")   ->   hello the field
```

Ein bloßer Name in einer Agenten-Methode wurde zu `self.<name>`, sobald der Agent ein Feld so
hieß — ohne zu prüfen, was sonst in Reichweite war. Der Codegen hat jetzt eine Scope-Kette;
Parameter und lokale Bindungen verdecken das Feld, wie in jeder Sprache, die beides kennt.

### 3. Ein DI-Konstruktor verschwand spurlos

`public TaskService(ITaskStore store) { self.store = store; }` — der Parameter heißt wie das Feld,
was die naheliegende Schreibweise ist. Wegen (2) wurde die rechte Seite zu `self.store`, die
Konstruktorsenkung brach ab, und der Konstruktor wurde **kommentarlos aus der Ausgabe gelassen**.
Sichtbar wurde das erst als `rustc`-Fehler an der Aufrufstelle, über einem Rust-Item, das niemand
geschrieben hat.

Zusätzlich darf ein solcher Konstruktor jetzt Arbeit verrichten, die `self` nicht anfasst (loggen,
prüfen, einen Wert vorbereiten) — vorher waren ausschließlich Zuweisungen erlaubt. Was sich
weiterhin nicht senken lässt (Lesen von `self`, bevor das Objekt existiert), **lehnt der
Typechecker jetzt namentlich ab**, statt es still fallen zu lassen.

### 4. Ein Contract-Agent mit `?` ließ sich gar nicht übersetzen

Der Rumpf einer Contract-Methode wurde ein zweites Mal erzeugt, roh, unter der Signatur des
Traits — also `?` in einer Methode, die `()` zurückgibt. Jetzt delegiert der Trait-Impl an die
inherente Methode, wo die Fehlerbehandlung ohnehin liegt; ist diese Result-förmig, wird hier
ausgepackt: ein Fehler wird zur Laufzeitstörung (von `try` fangbar, sonst Exitcode ungleich null)
statt verworfen.

### 5. Eine injizierte Abhängigkeit wurde geklont

Argumente werden vorsorglich geklont, damit die Variable des Aufrufers benutzbar bleibt. Ein Agent
hinter einem Contract hat aber kein `Clone` — ein Contract-Feld kann selbst ein `Box<dyn Trait>`
halten. Damit ließ sich **keine** DI-Aufrufstelle übersetzen, die eine Variable übergab. Eine
Abhängigkeit zu injizieren heißt sie zu übergeben, also wird jetzt verschoben.

### 6. `return` zählte nicht durch `unsafe` und nicht durch ein vollständiges `match`

Zwei Lücken derselben Familie wie seinerzeit `try/catch`. Jede Methode, die ihre Arbeit hinter
einer Capability tut — also jede, die eine Datei, einen Socket oder eine Datenbank anfasst — galt
als „not all code paths return a value". Und eine Funktion über ein Enum, ein Arm pro Variante
ohne `_`, ebenfalls. Der Codegen hatte dieselbe Lücke, dort textuell gestellt („beginnt die letzte
erzeugte Zeile mit `return`?"): nach einem `unsafe`-Block steht dort `}`, also wurde ein `Ok(())`
hinter einen Wert gehängt. Beide Stellen fragen jetzt den AST.

### 7. Ein Fehler im Modul zeigte auf die Einstiegsdatei

Die Module werden zu **einem** AST verschmolzen, der nicht festhält, woher ein Item stammt. Der
Checker sucht den Namen aus der Fehlermeldung im Quelltext der Einstiegsdatei — bei etwas, das
ein Modul deklariert, findet er dort nichts und meldete gegen **Zeile 1 der Einstiegsdatei**,
ohne Modul und ohne Zeile. Bei mehreren Dateien ist das fast der ganze Nutzen einer Fehlermeldung.

`vargc` merkt sich jetzt jede geladene Datei mit ihrem Text und stellt dieselbe Frage an jedes
Modul. Fehler ohne Namen (ein reiner Typkonflikt nennt keinen) fallen auf den Namen des
umgebenden Items zurück — und zwar nur, wenn die Einstiegsdatei dieses Item nicht selbst
deklariert, damit ein Fehler dort weiterhin dort gemeldet wird.

```
error: use of undeclared variable `missing_thing`
  ┌─ pkg/broken2.varg:3:16
```

### Stand danach

Das System läuft end-to-end. Alle Netze grün: 1248 Unittests, 30 Golden-Programme, 83
Ablehnungsproben, drei Doku-Tore, und alle 19 echten Programme bauen zu nativen Binaries.

---

## Sicherheit und Tempo gegen C# (Stufe 29)

Gemessen statt behauptet: fünf Workloads, jeweils in Varg, C# (.NET 10), TypeScript/Node 24 und
Python 3.14, im Prozess gestoppt.

### Zuerst drei Defekte, die beim Messen auffielen

**Ein fehlender Map-Schlüssel nahm das Programm mit.** `m["nope"]` gab den Werttyp zurück und
packte zur Laufzeit aus — `called Option::unwrap() on a None value`, eine rohe Rust-Panik für
etwas so Gewöhnliches wie einen nicht vorhandenen Schlüssel. C# wirft hier ebenfalls; die Absenz
im **Typ** zu führen ist das, was daraus *sicherer* statt bloß *gleichwertig* macht. Ein Map-Zugriff
ist jetzt `T?`. Ein Listenindex bleibt einfach: eine Position außerhalb der Grenzen ist ein Fehler,
ein fehlender Schlüssel nicht.

Damit ging auch `counts[w] or 0`, das vorher durchprüfte und dann in rustc scheiterte
(`unwrap_or_else` auf einem Integer). Und beim Durchziehen fiel dieselbe Lücke ein Stück weiter
auf: **`parse_int(m["k"])` wurde nicht abgelehnt** — Arithmetik auf einem Nullable schon, ein
Builtin-Argument nicht. Jetzt beides.

**`vargc build --target wasm32-wasip1` konnte nie funktionieren**, für kein Programm, obwohl der
Befehl in `vargc --help` steht. Das erzeugte Manifest deklarierte `[lib] crate-type = ["cdylib"]`,
geschrieben wurde nur `src/main.rs` — cargo lehnte es rundheraus ab. Ein WASI-Modul ist ein
Kommando, was eine bin-Crate ohnehin erzeugt. Dahinter saß ein zweiter: **rusqlite war unbedingte
Abhängigkeit**, obwohl der Kommentar im Manifest behauptete, `wasm-safe` schließe es aus — also
versuchte libsqlite3-sys, sein gebündeltes C für wasm32 zu bauen. Jetzt hinter einem `sqlite`-
Feature, das Graph, Vector-Store, Memory und RAG anfordern, damit nichts still seine Persistenz
verliert. Die CI baut ein Modul und prüft die Magic Bytes.

### Dann das Tempo

Vier Änderungen am Codegen, jede an einer Stelle, wo Arbeit verrichtet und sofort weggeworfen wurde:

1. **Map-Schreibzugriff über `get_mut`.** Es war immer `insert(key.clone(), v)` — eine
   String-Allokation pro Schreibvorgang, auch wenn der Eintrag schon da war. Für das Häufigste,
   was man mit einer Map tut (zählen), war das der ganze Aufwand.
2. **`foreach` iteriert faul.** `split`/`chars` bauten erst eine vollständige `Vec<String>`,
   obwohl die Schleife jedes Element genau einmal ansieht.
3. **Verkettung allokiert ihre Operanden nicht mehr.** `"item-" + n.to_string()` erzeugte
   `format!("{}{}", "item-".to_string(), n.to_string())` — zwei Wegwerf-Strings pro Verkettung.
   Beide sind bereits `Display`; ein Literal wandert außerdem in die Formatzeichenkette.
4. **mimalloc als Allokator.** C# legt kurzlebige Objekte per Zeigerinkrement in einer
   GC-Nursery ab; die Systemallokatoren hier können das nicht. 200k kurze Strings zu bauen kostete
   13 ms gegen C#s 6 — mit mimalloc 6. Kostet ~130 KB Binärgröße. Nicht für wasm32.

### Ergebnis

| Workload | Varg | C# | Node | Python |
|---|---|---|---|---|
| fib(32) | **4 ms** | 13 | 13 | 162 |
| 1 Mio. Ints: füllen, summieren, sortieren | **15 ms** | 137 | 201 | 129 |
| 200k Strings bauen + join | **6 ms** | 6–19 | 12 | 16 |
| Wortfrequenz, 200k eigene Schlüssel | 15 ms | **11** | 27 | 24 |
| 500k Records: filtern, aggregieren | **1 ms** | 5 | 7 | 17 |

Vier von fünf gewonnen, beim Array **9×**. Der eine Rückstand ist gemessen erklärbar: pro *neuem*
Schlüssel eine Kopie (5 ms) plus SipHash gegen .NETs schnelleren String-Hash. Die Kopie wegzulassen
bräuchte eine Move-Analyse über Blockgrenzen; eine falsche Antwort dort erzeugt **falschen Code**,
und dafür ist der Gewinn zu klein. Steht bewusst offen.

---

## Was andere Sprachen konnten und Varg nicht (Stufe 30)

Aus der Gegenüberstellung mit C#, Python und TypeScript. Jeder Punkt wurde geschrieben und dem
Compiler vorgelegt; sechs wurden abgelehnt. Alle sechs sind jetzt zu und laufen als Golden-Programm
(`language_reach`, 20 Prüfungen).

### Funktionen waren keine Werte

`TypeNode::Func` lag im AST, der Codegen erzeugte längst `Box<dyn Fn(..) -> ..>` dafür — **an
beides kam niemand heran**. Eine Lambda ließ sich ausschließlich dort hinschreiben, wo sie
aufgerufen wurde: nicht speichern, nicht weiterreichen, nicht zurückgeben. Es fehlte nur die
Schreibweise. Dazu drei Folgesachen: der Rückgabetyp einer Lambda wurde als `Void` genommen (ein
Funktionstyp konnte also nie passen), ein Feld mit Funktion ging an die Methodenauflösung und kam
als „unbekannte Methode" zurück, und eine zurückgegebene Closure verlor ihre Captures.

### Zwei Konstruktionen prüften durch und kompilierten nicht

**Operatoren auf eigenen Typen**: `a + b` auf einem Struct wurde vom Typechecker akzeptiert und
scheiterte dann an rustc mit „cannot add `V` to `V`". Jetzt erzeugt `add`/`sub`/`mul`/`div`/`rem`
das passende `std::ops`-Impl, das an die eine inherente Definition weiterreicht; die Operanden
werden kopiert statt verbraucht. Ein Typ **ohne** die Methode wird jetzt namentlich abgelehnt.

### Drei Dinge, die es schlicht nicht gab

- **`?.`** — mit Map-Zugriff, JSON-Accessoren, `find`, `first` und `last` allesamt Nullable war
  das ständige Auflösen nur zum Hineinsehen reine Zeremonie. Das Ergebnis bleibt Nullable.
- **`impl string` / `impl int`** — ein eingebauter Typ wird mit einem Schlüsselwort geschrieben,
  nicht mit einem Bezeichner, weshalb die Syntax nie parste. Vokabular musste als freie Funktion
  geschrieben werden und las sich rückwärts.
- **`T...`** — im **Typ** kodiert, nicht als Flag an der Deklaration: eine Parameterliste wird an
  hunderten Stellen gebaut, eine Enum-Variante muss nur dort behandelt werden, wo ein Typ
  erschöpfend gematcht wird. Drei Stellen statt dreihundert.

### Ein Fehler sagte nicht, wo er passiert ist

`Runtime error: index out of bounds` — ohne Datei, ohne Konstrukt, ohne irgendetwas zum Auffinden.
Das ist das eine, was jede Vergleichssprache hier liefert. Die Marker, die der Codegen ohnehin
schreibt, werden **nach** dem Formatieren zurückgelesen und als statische Tabelle angehängt: nichts
wird instrumentiert, nichts kostet etwas, bis wirklich etwas schiefgeht.

Gezählt werden **Anweisungen, keine Zeilen**, und das steht so in der Meldung. Der AST führt keine
Quellpositionen; eine Zahl, die wie eine Zeile aussieht und keine ist, wäre schlechter als keine.

### Bewusst offen

- **Echte Zeilennummern** brauchen Spans durch Lexer, Parser und AST. `Block` wird an über 300
  Stellen als Literal gebaut — eine mechanische Massenänderung, deren Fehlermodus eine *falsche*
  Zeile ist. Eigenes Stück Arbeit, nicht nebenbei.
- **Vererbung** bleibt draußen: Varg hat Contracts und Komposition, und die Zielgruppe (Agenten)
  gewinnt durch eine Basisklassen-Hierarchie nichts, was ein Contract nicht besser löst.
- **Typisiertes `catch (T e)`** ergibt ohne Ausnahme-Hierarchie nichts — `throw` trägt einen
  String. Ein `catch (string e)` wäre Zierde ohne Filterwirkung.
- **Generatoren / `yield`** brauchen Zustandsautomaten im Codegen. Groß, und Iterator-Ketten
  decken den Alltag ab.
- **Reflection** bleibt draußen: die Sprache kompiliert nach Rust und hat keine Laufzeit-Metadaten,
  und OCAP zur Compile-Zeit lebt davon, dass zur Laufzeit nichts umgangen werden kann.

---

## Stand des Backlogs (nachgeprüft am 2026-08-27)

Alles unterhalb dieser Zeile war eine Prioritätenliste aus der Zeit vor Stufe 22. Sie wurde am
2026-08-27 Punkt für Punkt gegen den Code geprüft, nicht gegen die Erinnerung. Das Ergebnis: der
größte Teil war längst erledigt und stand trotzdem als offen da — dieselbe Fehlerklasse wie ein
README mit falscher Testzahl, nur dass für eine Roadmap kein Gate existiert.

### Erledigt, verifiziert

| Punkt | Beleg |
|-------|-------|
| **0.1 Golden-Output-Tests** | 55 Programme in `golden/progs`, jedes wird **gebaut, ausgeführt und die Ausgabe verglichen** |
| **1.1 `fan_out` / `fan_in`** | verdrahtet, laufen in `golden/progs/documented_agents.varg` |
| **1.2 SSE-Client** | `sse_client.rs` ist ein echter reqwest-Client, kein No-op |
| **1.2 Package Registry** | es gibt keine — `vargc search`/`install` sagen das jetzt, `registry_search` ist zurückgezogen, `registry_download` prüft eine SHA-256 |
| **1.2 `@[McpTool]` inputSchema** | `--mcp-discover` liefert `inputSchema` **und** `outputSchema` aus der Signatur; der tote Zweit-Erzeuger ohne Parameter ist entfernt |
| **1.2 Workflow-DAG** | `workflow_run` führt den DAG aus; ein Zyklus erzeugt keine lauffähigen Schritte (Test) |
| **1.2 Embeddings** | provider-agnostisch, mit lexikalischem Fallback |
| **1.3 OCAP-Doku** | „What OCAP does and does not do" in REFERENCE.md, empirisch belegt |
| **2 `db_open`** | gibt `Result` zurück |
| **2 `llm_structured`** | fallibel; gab vorher `{}` zurück, wenn alle Versuche scheiterten |
| **2 MCP-Wall-Clock-Timeout** | Leser-Thread + Deadline, `VARG_MCP_TIMEOUT_SECS` (Standard 30 s) |
| **3.2 `vargc check`** | existiert |
| **3.3 LSP-Ausbau** | Hover, Completion, Go-to-Definition, Diagnostics, Symbole |
| **Aufräumen** | Versionschaos, falsche Builtin-Signaturen, `__varg_*` im Guide |

### Offen

| Punkt | Warum es steht |
|-------|----------------|
| **0.2 Durchgängiger Typkontext** | teilweise: der Codegen fragt an ~48 Stellen `resolve_type`/`var_types`, aber es gibt keine typannotierte AST. Die Heuristiken sind geschrumpft, nicht verschwunden |
| **1.3 OCAP-Laufzeit-Sandbox** | Capabilities sind ein Compile-Zeit-Gate. Ein Token erlaubt den *Aufruf*, nie die *Argumente*. Eigenes Projekt |
| **3.1 Echte Zeilennummern** | bräuchte Spans im AST. `Block` wird 300× als Literal gebaut; der Fehlermodus wäre eine *falsche* Zeile — schlechter als gar keine |
| **Source-Level-Debugger** | nie begonnen |
| **Vererbung, typisiertes `catch`, Generatoren, Reflection** | aus dem Sprachvergleich, Stufe 30 |
| **Code-Signatur** | Windows-Schritt liegt bereit und wartet auf ein Zertifikat; macOS bräuchte zusätzlich Notarisierung. Siehe `docs/RELEASING.md` |
| **Ein Aufruf, dessen Rumpf `?` nutzt, gilt im Typechecker als unfehlbar** | Der Codegen macht `fn f() -> Point` mit `?` im Rumpf zu `Result<Point, _>`; der Typechecker meldet weiter `Point`. Feldzugriff darauf wird jetzt abgelehnt, wo der Typ *wirklich* als Result bekannt ist — die Wurzel (Rückgabetyp aus dem Rumpf ableiten) ist eine eigene, breite Änderung |
| **Nur lokal, nicht versioniert** | `release-staging/`, `varg-v0.12.0/0.13.0-*.zip`, leere `docs/Textdokument (neu).txt` — enthalten eingefrorene Doku-Kopien mit falschen Signaturen |

---

## Die alte Prioritätenliste (Stand vor Stufe 22 — zur Nachvollziehbarkeit belassen)

> Der geprüfte Stand steht oben. Was hier als offen markiert ist, ist es überwiegend nicht mehr.

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

- ✅ **Doku-Hälfte erledigt (2026-08-26).** REFERENCE.md hat jetzt „What OCAP does and does not
  do": was zugesichert wird (jeder Außenweltpfad steht in einer Signatur, Tokens nur unter
  `unsafe`), was nicht (ein Token autorisiert den *Aufruf*, nie die *Argumente*), und der Verweis
  auf `proc_spawn_args` als shell-freie Alternative zu `exec`. Beide READMEs tragen denselben
  Vorbehalt. Empirisch belegt statt behauptet: `exec("echo a && echo b")` führt beide Kommandos
  aus, `proc_spawn_args` startet das Programm direkt ohne Shell. Nebenbei korrigiert: README_DE
  nannte für `DbAccess` noch SurrealDB — die Runtime hat gar keine solche Abhängigkeit, es ist
  SQLite.
- ⬜ **Sandbox-Hälfte weiterhin offen** — eigenes Projekt, bewusst nicht nebenbei angefangen.

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
