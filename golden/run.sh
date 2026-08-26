#!/usr/bin/env bash
# Golden-output safety net for compiler changes: build + run each program in progs/ and
# diff its stdout against expected/<name>.expected. Catches SILENT miscompilation (wrong
# runtime output that still compiles) — the class that slipped past 1131 unit tests (B1).
#
# Usage:
#   VARGC=/path/to/vargc ./run.sh            # verify against golden
#   VARGC=/path/to/vargc ./run.sh --update   # (re)capture golden from current output
#
# Timestamps/dates are normalised so date-printing programs stay deterministic.
set -uo pipefail
VARGC="${VARGC:?set VARGC to a vargc binary (e.g. …/target/release/vargc.exe)}"
cd "$(dirname "$0")"
mkdir -p expected
norm() { sed -E 's/[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:.+-]+/<TS>/g'; }
fail=0
for v in progs/*.varg; do
  base="$(basename "${v%.varg}")"
  # Reset persisted state so programs that write named SQLite/graph/vector DBs are idempotent.
  rm -f ./*.db ./*.graph.db ./*.vector.db 2>/dev/null || true
  rm -rf ./golden_registry_cache 2>/dev/null || true
  if ! "$VARGC" build "$v" >/dev/null 2>&1; then echo "BUILD-FAIL  $base"; fail=1; continue; fi
  # vargc emits the binary into the current directory as <name>.exe.
  exe="./${base}.exe"
  if [ ! -x "$exe" ] && [ -x "${exe%.exe}" ]; then exe="${exe%.exe}"; fi  # POSIX: no .exe
  got="$("$exe" 2>/dev/null | norm)"
  # PIPESTATUS[0], not $?: after a pipe $? belongs to `norm`, so the check would have passed
  # for every program no matter how it ended.
  code="${PIPESTATUS[0]}"
  exp_file="expected/${base}.expected"
  # A program that fails has to say so with its exit code. That was not checked, and a `?`
  # propagating out of the entry point used to end the program silently with status 0 — the
  # output simply stopped and everything downstream read it as success. A program with an
  # `expected/<name>.exit` file must exit with exactly that code.
  exit_file="expected/${base}.exit"
  if [ -f "$exit_file" ]; then
    want_code="$(cat "$exit_file")"
    if [ "$code" != "$want_code" ]; then
      echo "EXIT-FAIL   $base (exit $code, want $want_code)"; fail=1; continue
    fi
  fi
  if [ "${1:-}" = "--update" ]; then
    printf '%s\n' "$got" > "$exp_file"; echo "UPDATED     $base"; continue
  fi
  exp="$(norm < "$exp_file" 2>/dev/null)"
  if [ "$got" = "$exp" ]; then
    echo "PASS        $base"
  else
    echo "FAIL        $base"; diff <(printf '%s\n' "$exp") <(printf '%s\n' "$got") | head -12; fail=1
  fi
done
[ "$fail" = 0 ] && echo "--- golden: ALL PASS ---" || echo "--- golden: FAILURES ---"
exit $fail
