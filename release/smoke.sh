#!/usr/bin/env bash
# Does a *packaged* Varg actually work?
#
# The release CI tested `vargc run` before packaging, inside a full repository checkout, where the
# runtime crates it needs happen to be lying around. The archive it then shipped did not contain
# them, and the generated Cargo.toml pointed at paths on the build machine — so v2.2.0 could
# report its version, type-check a program, and then fail to build a single one. Every check
# passed and the product did not work.
#
# This runs against an *extracted archive*, copied somewhere else first, with the repository
# nowhere in sight.
#
# Usage:
#   release/smoke.sh /path/to/extracted/varg-vX.Y.Z-platform
set -euo pipefail

STAGE="${1:?usage: smoke.sh <extracted release directory>}"
STAGE="$(cd "$STAGE" && pwd)"

EXE="vargc"
[ -x "$STAGE/vargc.exe" ] && EXE="vargc.exe"
[ -x "$STAGE/$EXE" ] || { echo "no $EXE in $STAGE"; exit 1; }

# Somewhere else entirely: a build that only works next to its own source tree is not a release.
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cp -r "$STAGE" "$WORK/varg"
cd "$WORK"
V="$WORK/varg/$EXE"

step() { printf '  %-34s' "$1"; }
ok()   { echo "ok"; }

step "vargc --version";        "$V" --version >/dev/null;                      ok
step "vargc new agent hello";  "$V" new agent hello >/dev/null;                ok
step "vargc check hello.varg"; "$V" check hello.varg | grep -q "no type errors"; ok

step "vargc build hello.varg"
out="$("$V" build hello.varg 2>&1)" || { echo "FAILED"; echo "$out" | tail -20; exit 1; }
ok

step "the program runs"
bin="./hello"; [ -x "./hello.exe" ] && bin="./hello.exe"
[ -x "$bin" ] || { echo "FAILED — no binary at $bin"; exit 1; }
"$bin" >/dev/null
ok

# The original defect wrote a manifest pointing at a directory on the build machine. Comparing
# path *text* means fighting Windows and MSYS spellings of the same place; whether the directory
# is actually there is the question that matters, and it is the one that was answered wrongly.
step "dependency paths resolve"
manifest=".vargc_cache/Cargo.toml"
if [ -f "$manifest" ]; then
    grep -o 'path = "[^"]*"' "$manifest" | sed 's/path = "//; s/"$//' | while IFS= read -r p; do
        [ -d "$p" ] || { echo "FAILED"; echo "    manifest points at $p, which is not there"; exit 1; }
    done
fi
ok

echo "  release works from an extracted archive"
