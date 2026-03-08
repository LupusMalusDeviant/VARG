# O03: Runtime Agents & Actor Model

In Varg ist **Nebenläufigkeit (Concurrency)** nicht durch geteilten Speicher und Mutexe (wie in C++) oder durch blockierendes `async/await` (wie in C#) geregelt, sondern durch das **Actor Model** (ähnlich wie in Erlang/Akka).

## 1. Der Agent als Actor
Ein `agent` ist ein Single-Threaded-Prozess, der eine Postbox (Message Queue) besitzt.
* Kein anderer Agent kann den internen Zustand (`private` Variablen) eines Agenten direkt mutieren.
* Jegliche Interaktion erfolgt asynchron über das Senden von Nachrichten.

```csharp
public agent SearchBot {
    // Wird asynchron vom OS in die Queue gelegt
    public Result<string, Error> HandleQuery(string q) {
        return "Found: " + q;
    }
}
```

## 2. Keine unsichtbaren Blockaden
Herkömmliche LLM-Requests blockieren Threads für mehrere Sekunden. Das AI-OS umgeht dies vollständig:
1. Agent A ruft LLM Inference auf.
2. OS nimmt Request entgegen, Agent A wird *Suspended* (nimmt aber keine OS-Ressourcen in Beschlag).
3. Wenn Token generiert sind, weckt das OS Agent A mit der Antwort.

## 3. Der OS-Orchestrator
Der Varg `KernelScheduler` entscheidet, welcher Agent auf welchem Rechner-Core läuft. Wenn ein Agent zu lange läuft (ohne I/O Yield), kann der Kernel ihn über einen Hardware-Interrupt zwingen, die CPU freizugeben (Preemptives Multitasking).
