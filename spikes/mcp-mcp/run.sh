#!/usr/bin/env bash
# Build and run the MCP-MCP spike.
#
#   VARGC=/path/to/vargc ./run.sh          # headless router demo (deterministic output)
#   VARGC=/path/to/vargc ./run.sh --ui     # serve the control UI on http://127.0.0.1:8710
#
# The children are Varg MCP servers too, so the whole thing is self-contained — no npm, no network.
set -uo pipefail
VARGC="${VARGC:?set VARGC to a vargc binary (e.g. …/target/release/vargc.exe)}"
cd "$(dirname "$0")"

for p in child_echo child_math mcp_mcp mcp_mcp_ui; do
  echo "building $p …"
  "$VARGC" build "$p.varg" >/dev/null || { echo "BUILD FAILED: $p"; exit 1; }
done

exe() { [ -x "./$1.exe" ] && echo "./$1.exe" || echo "./$1"; }

if [ "${1:-}" = "--ui" ]; then
  echo
  echo "MCP-MCP UI → http://127.0.0.1:8710   (Ctrl-C to stop)"
  exec "$(exe mcp_mcp_ui)"
fi

echo
"$(exe mcp_mcp)"
