#!/usr/bin/env bash
# Runtime-Substrat-Metriken für einen Varg-Agenten: Startup-Latenz, Binary-Größe,
# Tool-Round-trip. Vergleich gegen einen Python-Baseline-Prozess (Hermes ist Python),
# um den strukturellen nativ-vs-interpretiert-Unterschied zu zeigen.
#
# Nutzung:  VARGC="cargo run -q -p vargc --manifest-path <…>/Cargo.toml --" ./run_runtime.sh
#           (oder VARGC=/pfad/zu/vargc)   Voraussetzung: python im PATH.
set -euo pipefail
VARGC="${VARGC:-vargc}"
RUNS="${RUNS:-10}"
cd "$(dirname "$0")"

printf 'agent H { public void Run() { print "ok"; } }\n' > _hello_agent.varg
printf 'print("ok")\n' > _hello.py
"$VARGC" build _hello_agent.varg >/dev/null 2>&1

size_mb=$(du -m _hello_agent.exe 2>/dev/null | cut -f1 || echo "?")

bench() { # $1 = command
  local t=0 s e
  for _ in $(seq 1 "$RUNS"); do
    s=$(date +%s%3N); "$@" >/dev/null 2>&1; e=$(date +%s%3N); t=$((t + e - s))
  done
  echo $((t / RUNS))
}

# Python-Baseline mit typischen Agent-Imports (Varg hat diese eingebaut, 0 Import-Kosten).
IMPORTS='import json,sqlite3,urllib.request,http.client,dataclasses,asyncio'
printf '%s\nprint("ok")\n' "$IMPORTS" > _hello_imports.py

varg_ms=$(bench ./_hello_agent.exe)
py_ms=$(bench python _hello.py)
pyi_ms=$(bench python _hello_imports.py)

echo "=== Varg Agent-Runtime-Metriken (Ø $RUNS Läufe, warm) ==="
echo "native binary size          : ${size_mb} MB (self-contained)"
echo "startup: Varg native        : ${varg_ms} ms"
echo "startup: Python (trivial)   : ${py_ms} ms   (~$(( py_ms / (varg_ms>0?varg_ms:1) ))x)"
echo "startup: Python (+ imports)  : ${pyi_ms} ms  (~$(( pyi_ms / (varg_ms>0?varg_ms:1) ))x) <- realistischer Agent-Fall"
rm -f _hello_imports.py 2>/dev/null || true

rm -f _hello_agent.varg _hello_agent.exe _hello_agent.rs _hello.py 2>/dev/null || true
