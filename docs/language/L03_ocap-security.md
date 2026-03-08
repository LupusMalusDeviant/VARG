# L03: OCAP Security (Capability-Based Security)

Sicherheit in Varg basiert vollständig auf OCAP (Object-Capability Model). Anstatt ein manifestbasiertes Rechte-System (wie bei Android/iOS) oder User-Berechtigungen (wie Linux) zu verwenden, bindet Varg Rechte physisch an fälschungssichere Kryptographie-Tokens innerhalb der Sprach-Syntax.

## 1. Das Konzept
Ein Agent oder eine Funktion kann eine privilegierte Aktion (Netzwerk-Request, FileSystem-I/O) nur dann ausführen, wenn er das zugehörige OCAP-Token in der eigenen Methodensignatur deklariert und an die System-Schnittstelle weiterreicht.

## 2. Token als Sprachkonzept
Tokens sind spezielle `struct` Derivate, die der Compiler erkennt. Sie können nicht manuell instanziiert (`new NetworkToken()`) werden, sondern werden vom OS (Ring 0) an den Agenten beim Spawnen vererbt.

```csharp
public agent WebAgent {
    // Der Compiler zwingt den Aufrufer (OS), das Token beim Constructor mitzugeben
    public WebAgent(NetworkAccess netToken) { ... }

    public Result<string, Error> Fetch(string url, NetworkAccess netToken) {
        // Die interne Netzwerk-Lib verlangt das netToken. Ohne Token -> Compiler Error!
        return HttpClient.Get(url, netToken);
    }
}
```

## 3. Sandboxing & Compiler-Garantien
In Phase 4 des Compilers (`type_checker.rs`) muss ein OCAP-Visitor laufen.
* **Prüfung:** Der Compiler sucht jeden externen Aufruf (z.B. API, DB).
* **Validierung:** Er prüft die erforderlichen Capabilities der aufgerufenen Funktion.
* **Verifikation:** Er stellt sicher, dass das Token vom Caller Scope aus sichtbar und nicht mutiert ist.
* **Ergebnis:** Jeder kompilierte `.varg` Agent ist BY DEFINITION speichersicher und in seiner Sandbox gefangen. Es gibt keine Laufzeit-Sicherheitslücken, da Verstöße Compile-Time-Fehler sind.
