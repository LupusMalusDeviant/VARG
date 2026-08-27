#!/usr/bin/env bash
# Run cargo audit, and check that the one advisory we skip is still one we may skip.
#
# CI used to carry `cargo audit --ignore RUSTSEC-... --ignore RUSTSEC-...` and nothing else. An
# ignore flag records a decision without its reason, so it survives the reason: the day a crate
# moves into the build graph, the flag keeps quiet. The entry below names the precondition that
# makes it safe, and a command that fails when the precondition stops holding.
#
# RUSTSEC-2026-0187, a stack overflow in lopdf's parser, used to be the second entry. printpdf is
# on 0.12 now and brings lopdf 0.44, which is past the fix, so the exception is gone rather than
# left standing on a reason that has expired.
set -uo pipefail
cd "$(dirname "$0")/.."/varg-compiler || exit 1

fail=0

# ── RUSTSEC-2026-0235 — out-of-bounds read in rkyv when validating archives ──────────────────
# rkyv is in Cargo.lock only because rust_decimal declares it as an optional dependency. duckdb,
# the only crate that pulls rust_decimal, does not enable that feature, so rkyv is never compiled.
# If that changes, the advisory becomes reachable and this must be reconsidered.
if [ -n "$(cargo tree -i rkyv --features full --target all 2>/dev/null)" ]; then
    echo "FAIL: rkyv is now in the build graph; RUSTSEC-2026-0235 can no longer be ignored."
    fail=1
else
    echo "ok   rkyv is not compiled (RUSTSEC-2026-0235 unreachable)"
fi

# ── Everything else has to be clean ──────────────────────────────────────────────────────────
echo
cargo audit --ignore RUSTSEC-2026-0235 || fail=1

exit $fail
