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
| **MCP-Server-Tools** | Tool-Handler ist Echo-Stub | Varg-Lambda als echten Tool-Handler verdrahten (analog `orchestrator_run_all`); `@[McpTool]` sollte `inputSchema` erzeugen |
| **Workflow-DAG** | reiner Status-Tracker, kein Runner, keine Zyklenerkennung | Runner + Zyklenerkennung ergänzen, sonst als „Tracker" (nicht „Engine") dokumentieren |
| **Embeddings** (`embed`, `llm_embed_batch`) | ohne `GEMINI_API_KEY` nur Zeichen-Hash | Klar dokumentieren; optional echten lokalen Embedder (z. B. `fastembed`) einbinden |

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
