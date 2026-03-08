# O02: SurrealDB als Dateisystem (`/sys/fs/surreal`)

Klassische Betriebssysteme nutzen hierarchische Dateisysteme (Ext4, NTFS) und Strings für Pfade (`/var/log`). KI-Agenten benötigen Vektoren und Graphen-Relationen. Das Varg AI-OS löst das Dateisystem ab und ersetzt es vollständig durch **SurrealDB**.

## 1. Das Konzept
SurrealDB fungiert als Multi-Model-Datenbank. Jeder OS-Daemon und User-Agent nutzt sie als zentralen, persistenten Speicherblock.
* **Graph-Relations:** Ersetzt Ordnerstrukturen. Ein Document-Node ist über eine `BELONGS_TO` Relation mit einem Agent-Node verbunden.
* **Vector-Store:** Jeder Text/Kontext, der gespeichert wird, erhält automatisch ein Embedding (Dense Vector), das auf Datenbank-Ebene mit ANN (Approximate Nearest Neighbor) durchsuchbar ist.

## 2. Speicherorte (Tabelle)
* `agent_memory`: Speichert alle Hibernation-States der Agenten inkl. Vektoreinbettung ihres letzten Kontexts.
* `documents`: Entspricht dem klassischen `/home/user/Documents`. Dateiinhalte liegen hier als Nodes.
* `system_logs`: Strukturierte JSON-Logs aller OCAP-Zugriffe.

## 3. Native OS-Funktionen (Varg)
Ein System-Agent in Varg muss keine externen Treiber laden.

```csharp
system agent ContextManager {
    public void EjectContext(Context ctx) {
        // Native Syntax! Übersetzt in SurrealDB RPC Call
        query INSERT INTO agent_memory {
            context_id: ctx.Id,
            embed: ctx.Vectorize(),
            timestamp: time::now()
        };
    }
}
```

## 4. Sicherheit
SurrealDB wird im Embedded Mode (RocksDB) oder als lokaler gRPC-Daemon hochgefahren. Zugriffe von Ring 3 Agenten auf Tabellen, die ihnen nicht gehören, werden durch Record-Level-Permissions (RLS) in SurrealDB blockiert.
