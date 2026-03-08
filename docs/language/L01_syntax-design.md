# L01: Syntax Design (.varg)

Varg ist eine extrem performante, KI-native Systems-Language. Die Syntax orientiert sich an modernem C#, gekoppelt mit der Speichersicherheit von Rust.

## 1. Grundlegende Syntax & Typisierung
* **C#-Stil:** Keine Einrückungs-Abhängigkeiten (Python), sondern strikte geschweifte Klammern `{}` und Semikola `;`.
* **Strikte Typisierung:** Jede Variable und Methode muss eine explizite Typsignatur besitzen. Keine implizite `var`-Inferenz in Deklarationen.
* **Sichtbarkeits-Modifier:** `public`, `private`, `internal` bestimmen streng die Sichtbarkeit.

## 2. Interface First: Contracts
Interfaces und Werkzeuge werden über das Schlüsselwort `contract` definiert. Der Compiler nutzt diese Definitionen automatisch, um LLM-gesteuerte JSON-Schemas für das Tool-Calling zu generieren.

```csharp
public contract WebSearcher {
    Result<string, Error> Search(string query);
}
```

## 3. Native KI-Primitive
Strings sind keine Prompts. Arrays sind keine Vektoren. Varg verankert KI-Konzepte als native, streng geprüfte Datentypen:
* `Prompt`: Ein speziell codierter Typ für LLM-Instruktionen.
* `Context`: Verwaltet Konversations-Historien und Tokens.
* `Tensor`: Mathematisches Fundament für neuronale Verarbeitung.
* `Embedding`: Nativer Array-Typ für vektorisiertes Wissen.

## 4. Struct vs Agent
Varg unterscheidet hart zwischen passivem Speicher und aktiven System-Einheiten:
* `struct`: Herkömmliche DTOs, Speichereinheiten. Konnten Funktionen haben, sind aber synchron.
* `agent`: Ein asynchron operierender Daemon/Actor, der Nachrichten empfängt, Zustände bewahrt und Kontext evaluiert.

## 5. Fehlerbehandlung
Varg verzichtet auf verdeckte, den Stack abwickelnde `Exceptions`, die dem Betriebssystem-Design schaden könnten.
* Verwendung des Monads `Result<T, Error>`.
* Compiler-Zwang: Der Return-Wert `Result` MUSS vom Aufrufer überprüft oder entpackt werden (analog zu Rusts `#[must_use]`).

## 6. Ökosystem & Hardware-Zuweisung
Um den Anforderungen der KI-Verteilung gerecht zu werden, ergänzt Varg die Syntax um zwei Deployment-Konzepte:
* **Git-First Import:** Module werden direkt über URLs importiert (z.B. `import "github.com/ai-os/stdlib/network"`).
* **Hardware-Zuweisung (@target):** Funktionen oder Agenten können spezifische Prozessoren erzwingen (z.B. `@target("NPU")`). Der OS-Load-Balancer hält sich zwingend an diesen Syntax-Marker.
