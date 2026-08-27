#!/usr/bin/env bash
# Run cargo audit, and check that every advisory we skip is still one we may skip.
#
# CI used to carry `cargo audit --ignore RUSTSEC-... --ignore RUSTSEC-...` and nothing else. An
# ignore flag records a decision without its reason, so it survives the reason: the day a crate
# moves into the build graph, or a writer gains a parser, the flag keeps quiet. Each entry below
# names the precondition that makes it safe, and a command that fails when the precondition stops
# holding.
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

# ── RUSTSEC-2026-0187 — stack overflow in lopdf on deeply nested PDF objects ─────────────────
# lopdf *is* compiled, through printpdf, under the `pdf` feature. The advisory is in the parser.
# Varg's PDF surface only writes: pdf_create, pdf_add_section, pdf_add_text, pdf_save,
# pdf_to_base64. Nothing hands a PDF to the library to read, so the parser is never entered on
# input from anywhere. printpdf 0.12 carries a fixed lopdf but rewrites its whole page API; that
# upgrade is open work, not a one-line bump.
if grep -nE "load_from|load_mem|Document::load|from_reader" crates/varg-runtime/src/pdf.rs >/dev/null 2>&1; then
    echo "FAIL: pdf.rs now reads PDFs; RUSTSEC-2026-0187 can no longer be ignored."
    fail=1
else
    echo "ok   pdf.rs only writes PDFs (RUSTSEC-2026-0187 unreachable)"
fi

# ── Everything else has to be clean ──────────────────────────────────────────────────────────
echo
cargo audit --ignore RUSTSEC-2026-0235 --ignore RUSTSEC-2026-0187 || fail=1

exit $fail
