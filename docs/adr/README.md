# Architecture Decision Records

Chronologisches Verzeichnis aller Architekturentscheidungen dieses Repos. Jede Zeile führt zu einem einzelnen ADR.

## Was ist ein ADR?

Ein Architecture Decision Record dokumentiert eine einzelne, wichtige Architekturentscheidung — inklusive des damaligen Kontexts, der betrachteten Optionen und der bewusst in Kauf genommenen Konsequenzen. ADRs sind unveränderliche Zeitkapseln: Sie werden nicht überschrieben, sondern bei Revisionen durch neue ADRs ersetzt (Supersede).

## Status-Legende

- **Vorgeschlagen** — in Review, noch nicht beschlossen.
- **Akzeptiert** — beschlossen und gültig.
- **Abgelehnt** — Vorschlag wurde verworfen (bleibt als Lernerfahrung im Log).
- **Veraltet** — durch ein neueres ADR ersetzt.

## Decision Log

| Nr. | Titel | Status | Datum | Ersetzt durch |
|-----|-------|--------|-------|----------------|
| [0001](./0001-runtime-skill-architektur-varg-agenten.md) | Runtime-Skill-Architektur für kompilierte Varg-Agenten | Vorgeschlagen | 2026-07-14 | — |

*(Neue Einträge chronologisch, jüngste unten.)*

## Beitragen

Neue ADRs werden über den `adr-writer`-Skill erzeugt. Manuell geht auch:

1. Nächste Nummer ermitteln (höchste existierende + 1, 4-stellig).
2. Datei anlegen unter `docs/adr/<NNNN>-<slug>.md`.
3. Template aus `adr-writer/references/template.md` folgen.
4. Diesen Index um einen Eintrag erweitern.
