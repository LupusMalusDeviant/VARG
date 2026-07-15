# Skill-Spike: Layer 3 — promote-to-native (Proof-of-Concept zu ADR-0001)

Belegt die **optionale dritte Schicht** aus
[ADR-0001](../../docs/adr/0001-runtime-skill-architektur-varg-agenten.md): ein reifer,
stabiler Skill wird von einem delegierten Skript (Layer 2) zu **nativem, OCAP-abgesichertem
Varg-Code** „promotet". Das ist Vargs eigentlicher Differentiator gegenüber Hermes'
interpretiertem Python — der kompilierte Skill läuft nativ **und** durchläuft Vargs
Capability-Prüfung zur Compile-Zeit.

## Was der Spike zeigt

| Schritt | Mechanismus |
|---|---|
| `[promote]` | Agent generiert zur Laufzeit Varg-Quelltext, ruft `vargc` via `proc_spawn_args` auf und kompiliert den Skill zu einem **nativen Binary** (~0.5–0.8 s warm; erster Build kalt = Deps-Kompilierung). |
| `[run-native]` | Agent führt das native Binary aus und erfasst dessen (sauberen) stdout. |
| `[ocap]` | Ein **unsicherer** Skill (`fs_read` ohne `FileAccess`-Token) wird von `vargc` zur **Compile-Zeit ABGELEHNT** und gar nicht erst promotet. Genau das kann Hermes' Python nicht. |

## Ausführen

Der Agent ruft `vargc` zur Laufzeit auf — der Pfad kommt aus `VARGC_BIN`.

```bash
# 1. Ein vargc-Binary bereitstellen (muss in varg-compiler/target/release bleiben,
#    da es die Compiler-Crates relativ zu sich selbst findet):
cargo build --release -p vargc          # -> varg-compiler/target/release/vargc.exe

# 2. Spike bauen und ausführen (VARGC_BIN = ROHER Pfad, ohne Anführungszeichen):
export VARGC_BIN="…/varg-compiler/target/release/vargc.exe"
vargc build promoter.varg
./promoter.exe
```

Erwartete Ausgabe:

```
=== Layer 3: promote-to-native (ADR-0001) ===
[promote] Skill 'shout' zu NATIVE kompiliert in 619ms
[run-native] -> HELLO FROM A NATIVE VARG SKILL!
[ocap] unsicherer Skill von vargc ABGELEHNT (rc=1) - nicht promotet
[ocap] -> Vargs Compile-Zeit-Capabilities; Hermes' Python-Tools haben das nicht
=== fertig ===
```

## Gefundene/behobene Bugs beim Bauen dieses Spikes

Der Spike hat drei latente Bugs aufgedeckt, die hier mitbehoben wurden:

- **`exec("literal")`** erzeugte einen Borrow eines Temporaries (E0716) — `exec` kompilierte
  nur mit einer Variable als Argument. (Fix aus dem Layer-2-Spike.)
- **`proc_spawn`/`proc_spawn_args`** pipeten stderr, drainten es aber nie → Deadlock bei
  Kindprozessen mit viel stderr (cargo/vargc). Jetzt wird stderr geerbt.
- **Bootstrap-Banner** (`[VargOS] Bootstrapping Runtime...`) ging auf **stdout** und
  verschmutzte die Ausgabe — schlecht für Varg-Binaries als komponierbare Tools. Jetzt auf
  stderr; stdout bleibt sauber.

## Plattform-Hinweis

Die Zeile `exec(".\\skill_shout.exe")` nutzt die Windows-Pfadform. Der Spike ist auf Windows
demonstriert; auf POSIX wäre es `./skill_shout`.

## Offen (aus ADR-0001)

- **Promotion-Kriterien:** ab welcher Nutzungshäufigkeit/Stabilität ein Skill promotet wird.
- **`libloading`** statt Subprozess, um promotete Skills in-process zu laden.
- **Warmer Build-Cache** (`CARGO_TARGET_DIR`), damit die erste Promotion nicht kalt baut.
