# Agent-Scaffolding-Benchmark

Reproduzierbare Task-Suite, die das **Agenten-Substrat** von Varg misst (Memory-Retrieval,
Runtime-Latenz, Footprint, OCAP) — **nicht** die LLM-Intelligenz. Gedacht als Grundlage für
einen fairen Vergleich gegen andere Agenten-Frameworks (z. B. Hermes/Python).

## Inhalt

- **`mem_recall.varg`** — Memory-Retrieval-Qualität: 16 Docs (mit Same-Topic-Distraktoren),
  10 paraphrasierte Queries; misst recall@1, recall@3, MRR über den Vector-Store.
- **`run_runtime.sh`** — Runtime-Substrat: Startup-Latenz (Varg nativ vs. Python trivial und
  mit typischen Imports) + Binary-Größe.
- **`RESULTS.md`** — ausgefüllte Scorecard (Varg-Zahlen) mit ehrlichen Hermes-Platzhaltern.

## Ausführen

```bash
# Memory-Retrieval (Default: lokales lexikalisches Embedding):
vargc run mem_recall.varg
# semantisch (echte Embeddings, s. Feature C1):
VARG_EMBED_PROVIDER=ollama vargc run mem_recall.varg

# Runtime-Metriken (VARGC = Pfad zu einem vargc-Binary; python im PATH):
VARGC="…/varg-compiler/target/release/vargc.exe" ./run_runtime.sh
```

## Wichtig (Ehrlichkeit)

- Misst **Scaffolding**, nicht End-to-End-Task-Erfolg (der bräuchte ein Live-LLM + Judge).
- Hermes-Spalten in `RESULTS.md` sind **nicht gemessen** (bräuchten eine lokale Hermes-
  Installation) — keine erfundenen Zahlen.
- Kleiner Datensatz → Trend-Indikator, keine statistisch harte Aussage.
