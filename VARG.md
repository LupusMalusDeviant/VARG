# Varg.md — Agent System & OS Rules
> Lies diese Datei vollständig, bevor du irgendetwas im Varg-Projekt tust.
> Dann lies die referenzierten Dokumente in der angegebenen Reihenfolge.

---

## Was du baust

**Varg (.varg)** und das dazugehörige **AI-OS**. 
Ein selbst-hostbares, modulares Betriebssystem und eine kompilierte Programmiersprache, die nativ für autonome KI-Agenten und Systems-Programming entwickelt wurden.

Stack: **Rust (Compiler & Transpilation Target) | Varg (AI-OS Daemons) | SurrealDB (Speicher/FS)**

---

## Absolute Regeln — niemals brechen

### Architektur & Trennung von Belangen

```text
1. STRICT SEPARATION OF CONCERNS.
   - Arbeitest du am Compiler (Lexer, AST, Parser, TypeChecker)? → Nutze Rust.
   - Arbeitest du am OS (Memory Mgmt, Agenten-Daemons, Interrupts)? → Nutze Varg (.varg).

2. OS & DATENBANK SIND EINS.
   - Herkömmliche Dateisysteme (Ext4, NTFS) existieren für Agenten nicht.
   - Alle Speicherung (Dokumente, RAG, Memory, States) erfolgt in SurrealDB (`/sys/fs/surreal`).
   - Keine C-Strings für DB-Queries! SurrealQL ist nativer Bestandteil des Varg-AST.

3. OCAP FIRST (CAPABILITY-BASED SECURITY).
   - Kein Dateizugriff, keine Netzwerkanfrage, keine SurrealDB-Transaktion ohne OCAP-Token!
   - Das Token muss zwingend als Parameter in der Methodensignatur übergeben werden.

4. PRIVILEGE RINGS (RING 0 vs RING 3).
   - `agent`: User Space (Ring 3). Läuft in der Sandbox. Darf KEIN `unsafe` nutzen.
   - `system agent`: Kernel Space (Ring 0). Darf Hardware ansprechen, Interrupts abfangen, FFI nutzen.
```

### KI-Spezifika

```text
5. NATIVE KI-DATENTYPEN.
   - Behandle `Prompt`, `Context`, `Tensor` und `Embedding` als primäre Sprach-Primitive.
   - Strings sind keine Prompts. Ein Type-Mismatch führt zum Compiler-Error.

6. INTERRUPT-DRIVEN & ACTOR MODEL.
   - Agenten kommunizieren asynchron über Messages. Kein blockierendes Async/Await.
   - Wenn der VRAM voll ist, greift Preemption: User Agenten werden unterbrochen, ihr State seriell nach SurrealDB verschoben (Hibernation).
```

### Git & Code-Qualität

```text
7. 100% TDD (Test-Driven Development) BEIM COMPILER.
   - Der Rust-Compiler wird schrittweise entwickelt. KEIN Feature ohne vorherigen Unittest.
   - Jeder AstNode, jedes Token und jede Parsing-Regel wird isoliert getestet.

8. KEINE MAGIC STRINGS IN AST-DEFINITIONEN.
   - SurrealQL-Keywords, Operatoren und Token haben feste Konstanten.
```

---

## Dokumente — wann lies was

### Die Sprache: Compiler & Syntax (Rust)

| Dokument | Inhalt | Wann lesen |
|----------|--------|------------|
| `docs/language/L01_syntax-design.md` | C#-meets-Rust Syntax, Typisierung, Interfaces (`contract`) | Vor dem Parsen von .varg Dateien |
| `docs/language/L02_compiler-phases.md` | Der 5-Phasen-Plan: Logos Lexer, AST, Parser, TypeChecker, Rust CodeGen | Bei der Entwicklung des rust-basierten Compilers |
| `docs/language/L03_ocap-security.md` | Implementierung von Zugriffstokens auf Compiler-Ebene | Beim Bau der Semantischen Analyse (Type Checker) |
| `docs/language/L04_systems-features.md` | `unsafe`, FFI (`extern "C"`), Pointer-Arithmetik | Vor dem Schreiben neuer Low-Level-Sprachfeatures |
| `docs/language/L05_surrealql-ast.md` | Integration von Nativer DB-Syntax in den Abstract Syntax Tree | Beim Aufbau des SurrealQL-Parsers in Rust |

### Das AI-OS: Laufzeitumgebung & Agenten (.varg / SurrealDB)

| Dokument | Inhalt | Wann lesen |
|----------|--------|------------|
| `docs/os/O01_os-architecture.md` | Ring 0 (system agent) vs Ring 3 (agent), Kernel-Architektur | Vor Beginn der OS-Schicht-Entwicklung |
| `docs/os/O02_surrealdb-fs.md` | Schemas, Vektorgraphen, Vector-Indizes, State-Storage | Bei jeder Form von I/O oder Speicherung im OS |
| `docs/os/O03_runtime-agents.md` | Actor Model, Asynchrones Message-Passing, Preemption | Bei der Modellierung von Agenten-Verhalten |
| `docs/os/O04_memory-mgmt.md` | VRAM Multiplexing, Hibernation, Zero-Copy Context Sharing | Beim Schreiben von Low-Level Memory-Agenten |
| `docs/os/O05_bootstrapping.md` | Phase 6 & 7: OS-Daemons in `.varg` umschreiben (Inception) | Beim Übergang von Rust in die fertig kompilierte Sprache |

---

## Schnell-Navigation: Aufgabe → Dokumente

```text
Ich baue am Rust-Parser           → /docs/language/L02_compiler-phases.md
Ich entwerfe Varg Syntax        → /docs/language/L01_syntax-design.md
Ich binde eine C-Library an `.varg`→ /docs/language/L04_systems-features.md
Ich verwalte Speicher-Out-of-Mem  → /docs/os/O04_memory-mgmt.md
Ich baue einen neuen OS Daemon    → /docs/os/O01_os-architecture.md + O03_runtime-agents.md
Ich brauche Persistenten Speicher → /docs/os/O02_surrealdb-fs.md + /docs/language/L05_surrealql-ast.md
```
