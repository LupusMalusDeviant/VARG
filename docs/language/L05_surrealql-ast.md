# L05: Native SurrealQL Integration

Das flache Dateisystem ist im AI-OS tot. Alle Zustände (Agents), alle Dokumente (Context) und alle Vektoren (RAG) liegen in SurrealDB. Normalerweise kommunizieren Sprachen via C-Strings über einen Datenbanktreiber. **Varg nicht.** SurrealQL ist nativer Bestandteil des AST.

## 1. Warum nativer AST?
* **Compile-Time Checks:** Wenn der Query "SELECT name FROM agent_memory" lautet, prüft der Varg-TypeChecker (Rust), ob die Tabelle `agent_memory` in der Schema-Definition existiert und ob `name` den korrekten Typ returnt.
* **Keine SQL-Injection:** Tokens werden direkt in prepared Statements des Backends (SurrealDB RPC) umgewandelt. C-Strings müssen vom OS nicht geparst werden.

## 2. Syntax-Implementierung
Der Lexer ignoriert String-Quotes für Queries, wenn sie dem Keyword `query` folgen.

```csharp
// Varg-Syntax: Typisiert und nativer Sprachpfad
Result<List<Embedding>, Error> GetSimilarContext(Tensor target, DbAccess token) {
    // Der Compiler erzeugt hierfür den Abstract Syntax Tree für SurrealQL!
    var records = query SELECT * FROM context WHERE vector_distance(embed, $target) < 0.1;
    return records;
}
```

## 3. Rust-Compiler Anforderungen
* **Phase 1 (Lexer):** Keywords wie `SELECT`, `DELETE`, `UPDATE`, `FROM`, `WHERE` sind reservierte Keywords im Datenbank-Kontext.
* **Phase 2 (AST):** Erstellung eines `SurrealQueryNode`, der Relationen identifiziert.
* **Phase 4 (Type Checker):** Prüft Typ-Sicherheit der zurückgegebenen Surreal-Results anhand der erwarteten Struktur (Zuweisung an `var records` oben).
