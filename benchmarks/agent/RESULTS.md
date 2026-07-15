# Varg Agent-Scaffolding — Benchmark-Scorecard

> Stand: 2026-07-15 · Varg v1.0.0 · Maschine: Windows 11 · lokale Läufe

## Was das misst — und was NICHT

Dieser Benchmark misst das **Agenten-Substrat** (Runtime, Memory/Retrieval, Footprint,
Sicherheit) — **nicht** die Intelligenz des LLM. Die „Denkleistung" eines Agenten kommt
aus dem Modell, das Varg *aufruft*; sie ist zwischen zwei Frameworks identisch, wenn sie
dasselbe Modell nutzen. Vergleichbar (und hier gemessen) ist nur das **Scaffolding**:
wie gut/schnell/sicher die Hülle Tools, Memory und Ausführung bereitstellt.

„Schlägt Hermes" ist also **nur auf diesen Substrat-Achsen** eine sinnvolle Aussage — nicht
auf End-Task-Fähigkeit (die das Modell dominiert).

## Ehrlichkeitshinweis zur Hermes-Spalte

Die Hermes-Zahlen sind hier **nicht gemessen** — das erfordert eine lokale Hermes-Installation
(Python) + dieselben Tasks. Die Spalte ist mit `— (lokal zu messen)` markiert und mit der
Mess-Methode versehen, damit sie reproduzierbar gefüllt werden kann. **Es werden keine
Hermes-Zahlen erfunden.**

## Ergebnisse

### 1. Memory-Retrieval-Qualität (`mem_recall.varg`)

16 Dokumente (mit Same-Topic-Distraktoren), 10 paraphrasierte Queries mit bekanntem Gold-Doc.

| Metrik | Varg (lokales lexikal. Embedding) | Varg (`VARG_EMBED_PROVIDER=ollama/openai`) | Hermes (FTS5/BM25) |
|---|---|---|---|
| recall@1 | **6/10 (0.60)** | — (höher erwartet; nicht gemessen ohne Provider) | — (lokal zu messen) |
| recall@3 | **9/10 (0.90)** | — | — |
| MRR | **0.733** | — | — |

*Lesart:* Das **Default**-Embedding ist lexikalisch (n-gram-Hash) — vergleichbar in der Klasse
mit Hermes' FTS5/BM25. Die 0.60 recall@1 zeigen die Grenze bei starken Paraphrasen; mit einem
echten semantischen Provider (Feature C1: Ollama/OpenAI/Gemini) sollte recall@1 deutlich steigen.
Der faire Apples-to-Apples-Vergleich ist *Varg-lexikal* vs *Hermes-FTS5*.

### 2. Runtime-Substrat (`run_runtime.sh`)

Alle Werte **warm** gemessen (Ø 12 Läufe) — faire Bedingungen für beide Seiten.

| Metrik | Varg | Python-Baseline (Hermes-Klasse) |
|---|---|---|
| Startup, **trivial** (leerer Agent) | **29 ms** | 43 ms → ~1,5× (marginal, beide vom OS-Prozess-Start dominiert) |
| Startup, **+ typische Agent-Imports** (`json, sqlite3, urllib, http.client, asyncio`) | **~30 ms** (eingebaut, 0 Import-Kosten) | 180–260 ms → **~5–9× schneller** (Lauf-Varianz) |
| Binary-Größe | **~2 MB** (self-contained, nativ) | — (Python-Env, GB-Bereich; lokal zu messen) |
| Delegierter Tool-Round-trip (Layer 2, `exec`) | **~24 ms** | — |
| Skill-Promotion zu nativem Code (Layer 3, warm) | **~0,6 s** | n/a (Hermes hat keine native Promotion) |

*Lesart (ehrlich):* Für ein **triviales** Programm ist der Startup vergleichbar (~1,5×) — beide
zahlen die OS-Prozess-Erzeugung. Der **strukturelle** Vorteil zeigt sich bei **Import-Last**:
Varg linkt json/sqlite/http statisch (0 Import-Kosten), Python zahlt sie bei *jedem* Spawn.
Für Hermes' „zero-context-cost subagent pipelines" (viele frische Python-Prozesse) summiert
sich das → hier ~9× schnellerer Kaltstart pro Subagent.
*(Eine frühere „~4×"-Zahl war ein Cold-Cache-Artefakt und wurde korrigiert.)*

### 3. Sicherheit (OCAP) — qualitativ

| Eigenschaft | Varg | Hermes |
|---|---|---|
| Capability-Prüfung zur Compile-Zeit | **Ja** (OCAP; unsicherer Skill wird abgelehnt, s. `spikes/skill-layer3`) | Nein (interpretiertes Python, keine Compile-Gate) |

## Reproduzieren

```bash
# Memory-Retrieval:
vargc run mem_recall.varg
#   optional semantisch:  VARG_EMBED_PROVIDER=ollama vargc run mem_recall.varg

# Runtime-Metriken:
VARGC="…/target/release/vargc.exe" ./run_runtime.sh
```

## Grenzen dieses Benchmarks

- Misst **Substrat**, nicht End-to-End-Task-Erfolg (der bräuchte ein Live-LLM + Agent-Loop + Judge).
- Hermes-Spalten sind Platzhalter (nicht gemessen).
- Datensatz ist klein (16 Docs) — Trend-Indikator, keine statistisch harte Aussage.
- Nächster Ausbau: End-to-End-Task-Suite (Tool-Use + Multi-Step) mit LLM-Judge, gegen einen
  echten Hermes-Lauf.
