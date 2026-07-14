# Skill-Spike: Layer 1 + 2 (Proof-of-Concept zu ADR-0001)

Dieser Spike belegt praktisch, dass die in
[ADR-0001](../../docs/adr/0001-runtime-skill-architektur-varg-agenten.md) gewählte
Skill-Architektur in echtem Varg funktioniert: Ein **kompilierter** Varg-Agent erzeugt
zur **Laufzeit** ein neues Tool, führt es **delegiert** aus und legt das zugehörige
Wissen als Skill-Dokument ab — **ohne sich selbst neu zu kompilieren**.

## Was der Spike zeigt

| Schritt | ADR-Schicht | Mechanismus |
|---|---|---|
| `[create]` | **Layer 2** (delegiertes Tool) | Agent schreibt die Tool-Logik als `.py`-Skript per `fs_write`. Die Logik existierte zur Compile-Zeit des Agenten **nicht**. |
| `[run]` | **Layer 2** | Agent führt den neuen Skill via `exec("python …")` aus — kein Recompile. |
| `[recall]` | **Layer 1** (Skill-Dokument) | Agent legt „wie/wann nutze ich den Skill" im Agent-Memory ab und ruft es wieder ab. |
| `[learn]` | Feedback-Loop | `self_improve` hält den Erfolg fest. |

Im echten System kämen `script_body` und das Skill-Dokument vom LLM; der Spike
synthetisiert sie fest, um den **Mechanismus** zu isolieren.

## Ausführen

Voraussetzung: `python` im PATH (das delegierte Tool ist ein Python-Skript).

```bash
vargc run skill_agent.varg
```

Erwartete Ausgabe:

```
=== Varg Skill-Agent (ADR-0001 Layer 1 + 2) ===
[create] neues Tool zur Laufzeit geschrieben -> skill_shout.py
[run]    shout('hello agent') -> HELLO!
[recall] shout(text): schreibt GROSS und haengt '!' an. Nutze es fuer Betonung.
[learn]  success_rate=100%
=== fertig: Tool zur Laufzeit erzeugt, delegiert ausgefuehrt, gelernt ===
```

## Was der Spike bewusst NICHT zeigt

- **Layer 3 (promote-to-native):** das Kompilieren eines stabilen Skills zu nativem,
  OCAP-abgesichertem Code via `vargc` zur Laufzeit (ADR-0001, ~737 ms) — separater Spike.
- **Sandboxing** der delegierten Ausführung (Arbeitsverzeichnis/Netzwerk/Zeitlimit) —
  offene Folge-Entscheidung im ADR.
- Anbindung an ein echtes LLM (`script_body`/doc sind hier fest kodiert).
