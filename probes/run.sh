#!/usr/bin/env bash
# Rejection sweep: the backward half of the safety net.
#
# golden/ proves that valid programs still compile and still compute the right thing. This proves
# the opposite direction: that invalid programs are rejected, and rejected by Varg's own front end
# rather than by rustc against generated Rust the author never wrote.
#
# The check is `vargc check` (parse + typecheck, no codegen, ~40ms each). That makes the contract
# exact: if `check` accepts a program that is not valid, the mistake reaches rustc. Every leak
# found so far was of that shape.
#
# Each program declares what must happen:
#   // @probe reject: <substring the error message must contain>
#   // @probe rustc-leak: <why this one is allowed through to rustc>
#
# Naming the expected message matters — without it a probe passes as long as *something* fails,
# which silently accepts a rejection for entirely the wrong reason.
#
# Usage:
#   VARGC=/path/to/vargc ./run.sh
set -uo pipefail
VARGC="${VARGC:?set VARGC to a vargc binary (e.g. …/target/release/vargc.exe)}"
cd "$(dirname "$0")"

strip_ansi() { sed -E 's/\x1b\[[0-9;]*m//g'; }

fail=0
checked=0

# ── Programs that must be rejected by the front end ───────────────────────────
for v in reject/*.varg; do
  base="$(basename "${v%.varg}")"
  want="$(sed -n 's|^// @probe reject: *||p' "$v" | head -1)"
  if [ -z "$want" ]; then
    echo "NO-DIRECTIVE  $base — add a '// @probe reject: <message fragment>' line"
    fail=1; continue
  fi
  out="$("$VARGC" check "$v" 2>&1)"
  status=$?
  checked=$((checked + 1))
  if [ $status -eq 0 ]; then
    echo "NOT-REJECTED  $base — accepted by the front end, so this reaches rustc"
    fail=1
  elif ! printf '%s' "$out" | strip_ansi | grep -qF -- "$want"; then
    echo "WRONG-REASON  $base"
    echo "              wanted a message containing: $want"
    echo "              got: $(printf '%s' "$out" | strip_ansi | grep -aoE 'error:? .{0,90}' | head -1)"
    fail=1
  else
    echo "PASS          $base"
  fi
done

# ── Rejected at build time rather than by `check` ─────────────────────────────
# A few rules are properties of code generation, not of the program's types — which agent the
# runtime constructs, for instance. `check` cannot see those, but they must still be *our* error
# with *our* wording, not a leaked rustc one. Both halves are asserted: check accepts, build
# rejects, and the message says what was wrong.
for v in reject-at-build/*.varg; do
  [ -e "$v" ] || continue
  base="$(basename "${v%.varg}")"
  want="$(sed -n 's|^// @probe reject-at-build: *||p' "$v" | head -1)"
  if [ -z "$want" ]; then
    echo "NO-DIRECTIVE  $base — add a '// @probe reject-at-build: <message fragment>' line"
    fail=1; continue
  fi
  checked=$((checked + 1))
  if ! "$VARGC" check "$v" >/dev/null 2>&1; then
    echo "NOW-CAUGHT    $base — the front end rejects this at check time now; move it to reject/"
    fail=1
    continue
  fi
  out="$("$VARGC" build "$v" 2>&1)"
  if [ $? -eq 0 ]; then
    echo "NOT-REJECTED  $base — the build accepts this now"
    fail=1
  elif printf '%s' "$out" | grep -qa "src.main.rs"; then
    echo "RUSTC-LEAK    $base — rejected by rustc, not by us"
    fail=1
  elif ! printf '%s' "$out" | strip_ansi | grep -qF -- "$want"; then
    echo "WRONG-REASON  $base"
    echo "              wanted a message containing: $want"
    fail=1
  else
    echo "PASS (build)  $base"
  fi
done

# ── Documented exceptions ─────────────────────────────────────────────────────
# These are invalid but caught by rustc rather than by us. Asserting both halves keeps the
# exception list from rotting: if the front end ever learns to catch one, this fails and says so.
for v in known-rustc-leak/*.varg; do
  [ -e "$v" ] || continue
  base="$(basename "${v%.varg}")"
  why="$(sed -n 's|^// @probe rustc-leak: *||p' "$v" | head -1)"
  if [ -z "$why" ]; then
    echo "NO-DIRECTIVE  $base — add a '// @probe rustc-leak: <reason>' line"
    fail=1; continue
  fi
  checked=$((checked + 1))
  if ! "$VARGC" check "$v" >/dev/null 2>&1; then
    echo "NOW-CAUGHT    $base — the front end rejects this now; move it to reject/ with its message"
    fail=1
  elif "$VARGC" build "$v" >/dev/null 2>&1; then
    echo "NOT-REJECTED  $base — neither the front end nor rustc rejects this any more"
    fail=1
  else
    echo "PASS (leak)   $base — $why"
  fi
done

echo "--- probes: $checked checked ---"
[ "$fail" = 0 ] && echo "--- probes: ALL PASS ---" || echo "--- probes: FAILURES ---"
exit $fail
