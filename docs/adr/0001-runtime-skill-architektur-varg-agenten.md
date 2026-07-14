# ADR-0001: Runtime-Skill-Architektur für kompilierte Varg-Agenten

- **Status:** Vorgeschlagen
- **Datum:** 2026-07-14
- **Autor:** LupusMalus
- **Konsultiert:** Codebase-Analyse + Latenz-Messungen (2026-07-14)

## Kontext und Problemstellung

Das strategische Ziel ist ein Varg-basierter Agent, der sich mit **Hermes Agent** (Nous Research) messen kann. Hermes' Signatur-Feature ist ein geschlossener Lern-Loop: Der Agent schreibt nach gelösten Aufgaben zur Laufzeit **autonom neue Python-Tools** sowie **Skill-Dokumente** und verbessert sie im Gebrauch. In interpretiertem Python ist „Skill schreiben und sofort ausführen" trivial — der Code wird zur Laufzeit eingelesen und ausgeführt.

Varg kompiliert dagegen zu **nativen Binaries**. Damit entsteht eine strukturelle Spannung: Ein laufender Varg-Agent kann **kein neues In-Process-Varg-Lambda zur Laufzeit synthetisieren**. Die vorhandene Tool-Registrierung (`mcp_server_register` mit `(args) => result`, verdrahtet über `gen_str_handler`) verlangt den Handler zur **Compile-Zeit** — sie taugt für vordefinierte Tools, nicht für zur Laufzeit erdachte. Neue ausführbare Logik erfordert in einer kompilierten Sprache zwingend entweder **Neukompilierung** oder **Delegation an einen Interpreter/Fremdprozess**.

Empirische Messungen auf der Zielmaschine (Runtime-Dependencies bereits gebaut, `.vargc_cache` warm):

| Pfad | Latenz |
|---|---|
| `vargc build` eines Skills (Neukompilierung, Deps gecacht) | ~737 ms |
| Start eines fertigen nativen Binaries | ~127 ms |
| `exec`/Subprozess-Round-trip (Logik im Fremdprozess) | ~24 ms |

Vorhandene Bausteine: `self_improve` (Feedback-Loop: `record_success`/`record_failure`/`recall` — Lernen aus Ergebnissen, **nicht** Skill-Erzeugung), `proc_spawn`, `mcp_connect` (externe MCP-Server anbinden), `mcp_server` (eigene Tools mit kompilierten Handlern), sowie ein bereits existierender `cdylib`-Codegen-Pfad (aktuell für WASM) — aber **kein** `libloading`.

**Kernfrage:** Wie erzeugt und nutzt ein *kompilierter* Varg-Agent Skills (inkl. neuer ausführbarer Logik) zur Laufzeit, ohne das Compile-Modell zu verleugnen — und ohne dabei die Stärken von Varg (native Performance, OCAP-Sicherheit, MCP-Nativität) aufzugeben?

## Anforderungen

### Funktional

- Der Agent kann zur Laufzeit **Wissens-Skills** (wie man eine Aufgabe mit bestehenden Tools löst) erzeugen, speichern und wiederverwenden.
- Der Agent kann zur Laufzeit **neue ausführbare Tools** verfügbar machen, deren Logik er selbst formuliert hat.
- Bestehende Infrastruktur (`self_improve`, `mcp_*`, `proc_spawn`, Agent-Memory) soll integriert, nicht ersetzt werden.

### Nicht-Funktional

- **Skill-Erzeugungs-Latenz:** niedrig genug, dass gelegentliche Skill-Erzeugung den Agent-Loop nicht ausbremst (Richtwert: << 1 s ist akzeptabel, Sekunden bis Minuten nicht).
- **Sicherheit:** dynamisch erzeugte Logik darf die OCAP-Garantien nicht aushebeln; unvertrauenswürdig generierter Code muss einschränkbar bleiben.
- **Komplexität / Wartbarkeit:** möglichst keine zweite eingebettete Sprache, keine fragile ABI/Dynamic-Loading-Infrastruktur als Pflichtpfad.
- **Differenzierung:** dort, wo Varg gegenüber Hermes gewinnen kann (native Perf, Compile-Zeit-Capabilities), soll die Architektur diesen Vorteil zugänglich machen — ohne ihn zur Voraussetzung zu machen.

## Betrachtete Optionen

### Option A: Delegation an Interpreter/Subprozess bzw. externe MCP-Tools

Der Agent schreibt Skill-Logik als **Text** (Shell-/Python-Skript) und führt sie über `exec`/`proc_spawn` aus, oder er bindet **externe MCP-Server** (`mcp_connect`) als Tool-Quellen an. Die ausführbare Logik lebt außerhalb des kompilierten Binaries.

**Positiv:**
- Sofort verfügbar, keine Kompilierung (~24 ms Round-trip gemessen).
- Nutzt bestehende Builtins (`exec`, `proc_spawn`, `mcp_connect`) — keine neue Infrastruktur.
- Entspricht exakt Hermes' Modell („Python-Tools" = delegierte, interpretierte Logik) und ist damit funktional ebenbürtig.
- MCP-Delegation ist standardkonform und erschließt ein ganzes Tool-Ökosystem.

**Negativ:**
- Für diese Skills verlässt der Agent Vargs native Performance und (teilweise) die OCAP-Compile-Garantien — Sicherheit hängt an Prozess-Sandboxing.
- Abhängigkeit von einer externen Laufzeit (Shell/Python/MCP-Server) auf dem Host.
- Command-Injection-Risiko bei unsauberer Argument-Behandlung.

### Option B: `vargc`-zur-Laufzeit + dylib/Subprozess-Load

Der Agent generiert **Varg-Quelltext**, ruft `vargc` zur Laufzeit auf, kompiliert zu nativem Code (Binary oder `cdylib`) und lädt/führt ihn (via Subprozess oder `libloading`).

**Positiv:**
- Skills laufen mit **nativer Performance** und unter Vargs **OCAP-Garantien** — ein Differenzierungsmerkmal, das Hermes (Python) strukturell nicht bieten kann.
- Eine einzige Sprache (Varg) über den gesamten Agenten hinweg.
- `cdylib`-Codegen-Pfad existiert bereits (WASM) — teilweise Vorarbeit vorhanden.

**Negativ:**
- ~737 ms Latenz pro Skill (Deps gecacht); der allererste Build der Runtime dauert Minuten (einmalig).
- `libloading`/FFI-ABI-Komplexität und Sicherheits-/Stabilitätsrisiken beim Laden von zur Laufzeit erzeugtem Code in den eigenen Prozess.
- Toolchain-Abhängigkeit zur Laufzeit (`cargo`/`rustc` müssen auf dem Host vorhanden sein).

### Option C: Eingebettete Skript-Engine (z. B. Rhai)

Eine in-process interpretierte Skriptsprache; Skills werden als Rhai-Skript geschrieben und ohne Neukompilierung ausgeführt.

**Positiv:**
- Schnelle, in-process Ausführung ohne Subprozess- oder Compile-Overhead.
- Sandboxing-fähig (Rhai kann Fähigkeiten einschränken).

**Negativ:**
- Führt eine **zweite Sprache** in ein „eine-Sprache"-Projekt ein — Bruch mit der Varg-Vision.
- Zusätzliche schwergewichtige Dependency; doppelte Tool-/Typ-Welt (Varg vs. Rhai).
- MCP/Subprozess (Option A) deckt denselben Bedarf standardkonform ab, ohne neue Sprache.

### Option D: Skill-als-Dokument

Ein „Skill" ist ein **Markdown-Wissensdokument**, das dem LLM erklärt, wie eine Aufgabe mit **bestehenden** Tools zu lösen ist. Keine neue ausführbare Logik.

**Positiv:**
- In jeder Sprache trivial, keine Compile-/Laufzeit-Probleme.
- Deckt einen großen Teil von Hermes' Skill-Nutzen ab (auch Hermes speichert „Skill-Dokumente").
- Passt nahtlos zum vorhandenen `self_improve`- und Agent-Memory-Layer.

**Negativ:**
- Erzeugt **keine** neue ausführbare Fähigkeit — nur bessere Nutzung vorhandener Tools.
- Reicht allein nicht, um „autonome Tool-Erstellung" abzubilden.

## Vorschlag des Autors

Keine der Optionen ist allein ausreichend, aber sie sind komplementär. Der Nutzen von Hermes' Skill-Loop zerfällt in zwei Teile: **Wissen** („wie man X mit vorhandenen Tools tut") und **neue ausführbare Logik** („ein neues Tool Y"). Option D deckt den Wissensteil vollständig und billig ab. Für die ausführbare Logik ist Option A der pragmatische Standard — sie ist sofort, nutzt vorhandene Builtins und ist funktional das Äquivalent zu Hermes' Python-Tools. Option B sollte **nicht** der Default sein (Latenz + Loading-Komplexität), aber sie ist genau der Hebel, mit dem Varg Hermes *überholen* kann: ausgereifte, häufig genutzte Skills lassen sich zu nativem, OCAP-abgesichertem Code „promoten". Option C wird verworfen, weil MCP/Subprozess denselben Bedarf ohne eine zweite Sprache deckt.

Damit ergibt sich eine **geschichtete Architektur**, die das Compile-Modell nicht bekämpft, sondern als optionale Optimierungsstufe nutzt.

## Entscheidung

**Gewählte Option:** "Hybrid: D (Skill-Dokumente) + A (delegierte Tool-Ausführung) als Default, B (Compile-to-native) als optionale Promotion-Stufe"

Ausschlaggebend waren die Nicht-Funktionalen Anforderungen *niedrige Skill-Latenz* und *keine zweite Sprache*: Der Default-Pfad (D+A) erzeugt Skills in ~24 ms ohne neue Sprache und ist funktional ebenbürtig zu Hermes. Die native/OCAP-Promotion (B) wird bewusst als **opt-in** akzeptiert — ihre ~737 ms Latenz sind nur für stabile „heiße" Skills relevant, wo sie sich amortisiert und Varg einen Vorteil verschafft, den Hermes nicht hat. In-Process-Varg-Lambdas zur Laufzeit werden als unmöglich (kompilierte Sprache) verworfen; Rhai als zweite Sprache wird verworfen.

## Konsequenzen

### Positiv

- Funktionale Ebenbürtigkeit zu Hermes' Skill-Loop ohne Kampf gegen das Compile-Modell.
- Der Default-Pfad braucht **keine neue** Runtime-Infrastruktur — `exec`, `proc_spawn`, `mcp_connect`, `mcp_server` und `self_improve` existieren bereits.
- Vargs Alleinstellungsmerkmale (native Perf, OCAP) werden über die Promotion-Stufe zu einem *echten* Differenzierungsmerkmal statt zu einem Hindernis.
- Klare Schichtung erlaubt schrittweise Umsetzung (D → A → B), jede Stufe eigenständig nutzbar.

### Negativ

- Für delegierte Skills (Layer 2) gelten OCAP-Garantien nur eingeschränkt; Sicherheit verlagert sich auf Prozess-Sandboxing und saubere Argument-Behandlung.
- Zwei Ausführungsmodelle nebeneinander (delegiert vs. nativ-kompiliert) erhöhen die konzeptionelle Komplexität für Agenten-Autoren.
- Die Promotion-Stufe (B) bleibt an eine vorhandene Rust-Toolchain auf dem Host gebunden.

### Folge-Entscheidungen

- **Skill-Repräsentation:** konkretes Format für Skill-Dokumente + zugehörige Tool-Deskriptoren (vermutlich Erweiterung von `self_improve`/Agent-Memory).
- **Sandboxing-Politik für Layer 2:** wie werden delegierte Skripte/Prozesse eingeschränkt (Arbeitsverzeichnis, Netzwerk, Zeitlimit)?
- **Promotion-Kriterien für Layer 3:** ab welcher Nutzungshäufigkeit/Stabilität wird ein Skill nach nativ kompiliert; braucht es dafür `libloading` oder genügt Subprozess-Ausführung?
- **`@[McpTool]`-inputSchema-Generierung** als Voraussetzung, damit dynamische MCP-Tools sauber typisiert exponiert werden.

### Review

**Reality-Check geplant für:** 2026-08-25 (ca. 6 Wochen nach Entscheidung)

## Weitere Informationen

### Scope

Betrifft die Agenten-Laufzeitschicht von Varg (`varg-runtime` Module `self_improve`, `mcp_server`, `proc`, `mcp`) sowie die Frage, wie ein damit gebauter Agent Skills verwaltet. Nicht betroffen: Compiler-Frontend (Lexer/Parser/Typechecker) und die Sprachsyntax selbst.

### Tooling-Empfehlung

- Für Layer 3 (Promotion): der bestehende `cdylib`-Codegen-Pfad + optional `libloading`; alternativ zunächst Subprozess-Ausführung kompilierter Skill-Binaries (einfacher, kein FFI-Risiko).
- Für Layer 2: `proc_spawn_args` (vermeidet Shell-Interpolation) gegenüber `exec` bevorzugen, um Command-Injection zu reduzieren.

### Referenzen

- Hermes Agent (Nous Research): https://hermes-agent.nousresearch.com/ , https://github.com/nousresearch/hermes-agent
- Projekt-Roadmap: `OPTIMIZATIONS.md` (Abschnitt „Agent-Layer")
