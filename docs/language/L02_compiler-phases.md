# L02: Compiler Phases (TDD Masterplan)

Der `Varg`-Compiler (geschrieben in Rust) wird anhand dieser strikten 5 Phasen unter Verwendung von Test-Driven Development (TDD) implementiert.

## Phase 1: Lexer (Tokenisierung)
* **Ziel:** Den `.varg` Quellcode in verwertbare Tokens zerlegen.
* **Technologie:** Crate `logos`.
* **Regeln:**
  * Regex-Definitionen für Keywords (z.B. `contract`, `agent`, `Prompt`, `unsafe`).
  * Erkennung von Symbolen (`{`, `;`, `->`, etc.).
  * *TDD:* Jeder Token-Typ bekommt zuerst einen Unittest, der Strings wie `public contract` prüft, bevor der Logos-Regex geschrieben wird.

## Phase 2: Abstract Syntax Tree (AST)
* **Ziel:** Die hierarchische, typisierte Repräsentation des Quellcodes im Speicher aufbauen.
* **Technologie:** Rust (Structs und Enums).
* **Regeln:**
  * Erstellung von Knoten wie `Program`, `ContractDef`, `AgentDef`, `MethodDecl`, `TypeNode`.
  * Einbindung von System-Nodes: `UnsafeBlock`, `ExternDecl` (FFI).
  * *TDD:* Dummy-Bäume manuell instanziieren, um Lifetime- und Boxing-Issues compilerweit zu prüfen, ohne Code zu parsen.

## Phase 3: Recursive-Descent Parser
* **Ziel:** Den linearen Token-Stream in den hierarchischen AST übersetzen.
* **Technologie:** Handgeschriebener Parser.
* **Regeln:**
  * Saubere Fehlermeldungen (`Result<AstNode, ParseError>`) bei Syntaxverstößen.
  * Schrittweise Implementierung: `parse_type`, `parse_declaration`, `parse_contract`.
  * *TDD:* Minimal `.varg`-Scripts als Testcases. Z.B. ein leerer Contract. Das Parsing muss einen validen AST liefern.

## Phase 4: Semantic Analysis & Type Checker
* **Ziel:** Logische Integrität und Sicherheit prüfen (OCAP / Typsicherheit).
* **Technologie:** Visitor Pattern.
* **Regeln:**
  * Rückgabetypen mit der Deklaration abgleichen.
  * Verbotene Zuweisungen blockieren (z.B. `User Agent` nutzt `unsafe`).
  * Capability-Prüfung (OCAP): Werden Tool-Tokens korrekt durchgereicht?
  * *TDD:* Schreibe Code, der syntaktisch korrekt, aber semantisch falsch ist (Type Mismatch). Der Unittest MUSS rot werfen.

## Phase 5: Rust Code Generation (Transpilation)
* **Ziel:** Erzeugung von nativem Maschinencode über den Umweg der Rust-Transpilation.
* **Technologie:** Eigener Rust-Code-Generator (`varg-codegen`).
* **Ablauf:**
  1. Der typgeprüfte AST wird zu validem Rust-Quellcode übersetzt.
  2. Varg-Agents → Rust Structs mit `impl`-Blöcken.
  3. Varg-Contracts → Rust Traits.
  4. Der generierte Code wird in `.vargc_cache/` als Cargo-Projekt abgelegt.
  5. `cargo build --release` kompiliert das Ergebnis zu nativen Binaries.
* **Vorteile:**
  * Nutzt Rusts gesamtes Ökosystem und Crate-Registry.
  * Rusts Borrow Checker dient als zusätzliche Sicherheitsschicht.
  * Schnellere Iterationszyklen als ein eigenes LLVM-Backend.
* **Regeln:**
  * Jeder AST-Knoten muss eine definierte Rust-Übersetzung haben.
  * Runtime-Helfer (Networking, Crypto, LLM, DB) werden als Funktionen injiziert.
  * *TDD:* Generierter Rust-Code muss kompilieren und korrekte Ergebnisse liefern.

### Phase 5b: MLIR / LLVM Backend (Zukunftsvision)
* **Ziel:** Direktes LLVM-Backend ohne Rust-Zwischenschritt.
* **Technologie:** Crate `melior` (MLIR Bindings) oder `inkwell` (LLVM Bindings).
* **Wann:** Erst relevant, wenn der Varg-Compiler stabil ist und die Rust-Toolchain-Abhängigkeit entfallen soll.
* **Vorteile:** Kein `rustc`/`cargo` zur Laufzeit nötig, schnellere Kompilierung, volle Kontrolle über Codegen.
