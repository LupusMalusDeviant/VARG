"""The fourth gate: do the claims in prose hold?

The code blocks in the documentation were checked against the compiler for three releases while
the prose beside them drifted. The README badge said 953 tests, the table fifteen lines below it
said 1,126, and the truth was 1,264 — two wrong numbers, in one file, contradicting each other.
Nothing looked, because nothing had been asked to.

Only claims with a single mechanical source of truth are checked here:

  * the version, which lives in vargc's Cargo.toml
  * `vargc <command>`, which the compiler itself lists
  * repository paths, which either exist or do not

A number nothing can verify does not belong in the documentation in the first place, which is why
the counts that used to sit in the README's table were either measured and pinned or removed.
"""

import io
import json
import os
import re
import subprocess

PROSE_DOCS = ("README.md", "README_DE.md")

# Words that follow `vargc` in prose without being commands.
NOT_COMMANDS = {
    "is", "on", "in", "to", "and", "or", "was", "will", "can", "the", "a", "it",
    "needs", "puts", "writes", "prints", "reports", "builds", "runs", "compiles",
    "itself", "binary", "from", "with", "for", "at", "by", "as", "that", "which",
    "would", "does", "did", "has", "have", "had", "no", "not", "only", "still",
}


def declared_version(root):
    manifest = os.path.join(root, "varg-compiler", "crates", "vargc", "Cargo.toml")
    if not os.path.exists(manifest):
        return None
    for line in io.open(manifest, encoding="utf-8").read().splitlines():
        m = re.match(r'\s*version\s*=\s*"([^"]+)"', line)
        if m:
            return m.group(1)
    return None


def real_subcommands(vargc):
    """What `vargc` prints as its usage is what `vargc` accepts."""
    try:
        r = subprocess.run([vargc], capture_output=True, text=True, errors="replace")
    except OSError:
        return set()
    text = re.sub(r"\x1b\[[0-9;]*m", "", (r.stdout or "") + (r.stderr or ""))
    return set(re.findall(r"^\s{2}vargc\s+([a-z][a-z-]*)", text, re.M))


def _lines(root, doc):
    path = os.path.join(root, doc)
    if not os.path.exists(path):
        return []
    return io.open(path, encoding="utf-8", errors="replace").read().splitlines()


def check_version(root, docs):
    """The version stated for Varg itself has to be the one vargc reports."""
    want = declared_version(root)
    if not want:
        return ["could not read the version from vargc/Cargo.toml"]
    out = []
    # Only where the line is talking about a Varg release: a Rust edition, a crate version or a
    # .NET target on the same line is somebody else's number.
    tells_of_varg = re.compile(r"varg[-\s]?v?\d|version\s*\|", re.I)
    for doc in docs:
        for n, line in enumerate(_lines(root, doc), 1):
            if not tells_of_varg.search(line):
                continue
            for found in re.findall(r"\b(\d+\.\d+\.\d+)\b", line):
                if found != want:
                    out.append("%s:%d says %s where vargc is %s" % (doc, n, found, want))
    return out


def check_commands(root, docs, vargc):
    """A command named in the documentation has to be one the compiler answers to."""
    known = real_subcommands(vargc)
    if not known:
        return ["could not ask %s for its commands" % vargc]
    out = []
    for doc in docs:
        for n, line in enumerate(_lines(root, doc), 1):
            for word in re.findall(r"vargc\s+([a-z][a-z-]*)", line):
                if word in known or word in NOT_COMMANDS:
                    continue
                out.append("%s:%d mentions `vargc %s`, which is not a command" % (doc, n, word))
    return out


def check_paths(root, docs):
    """A file the documentation points at has to be there."""
    out = []
    # Backticked paths that look like repository files, not shell fragments or URLs.
    looks_like_path = re.compile(r"`([A-Za-z0-9_./-]+\.(?:varg|rs|toml|md|py|sh|yml))`")
    for doc in docs:
        for n, line in enumerate(_lines(root, doc), 1):
            for candidate in looks_like_path.findall(line):
                if candidate.startswith(("http", "./", "~")) or " " in candidate:
                    continue
                # A bare file name is a name, not a path; only a path says where it is.
                if "/" not in candidate:
                    continue
                # A path whose first segment is not a directory of this repository is describing
                # somebody else's layout — `store/repo.varg` in the section about how a dotted
                # import maps to a file, for instance. Only what claims to be here is checked.
                head = candidate.split("/")[0]
                if not os.path.exists(os.path.join(root, head)):
                    continue
                if not os.path.exists(os.path.join(root, candidate)):
                    out.append("%s:%d points at %s, which is not there" % (doc, n, candidate))
    return out



def check_unrun_table(root, unrunnable):
    """The documented list of what CI never runs has to be that list.

    Saying which builtins are unexercised is only worth something if the statement is kept in step
    with the exemption table the gate actually uses. Written by hand it would drift the way every
    other number in this documentation drifted.
    """
    path = os.path.join(root, "REFERENCE.md")
    if not os.path.exists(path):
        return []
    text = io.open(path, encoding="utf-8", errors="replace").read()
    start = text.find("## What CI Proves, and What It Does Not")
    if start < 0:
        return ["REFERENCE.md has no section saying what CI does not run"]
    end = text.find("## Getting Started", start)
    section = text[start:end if end > 0 else len(text)]

    named = set(re.findall(r"`([a-z_][a-z_0-9]*)`", section))
    named = {n for n in named if n in unrunnable or n in _ALL_MENTIONED}
    want = set(unrunnable)
    missing = sorted(want - named)
    extra = sorted(n for n in named if n not in want and n in _ALL_MENTIONED)

    out = []
    for n in missing:
        out.append("REFERENCE.md does not list `%s` among what CI never runs" % n)
    for n in extra:
        out.append("REFERENCE.md lists `%s` as never run, but a golden program runs it" % n)
    return out


# Filled by the caller: every builtin name the compiler knows, so a word in prose that merely
# looks like one is not mistaken for a claim.
_ALL_MENTIONED = set()


# The order of the rows in the README performance tables, against the benchmark names in
# results.json. A table row that moves without this moving is caught as a mismatch, which is the
# point: the numbers are not allowed to drift away from the run that produced them.
_PERF_ROWS = ("fib", "data", "json_bench", "wordfreq")
_PERF_COLUMNS = ("Varg", "C#", "TypeScript", "Python")


def _stated(cell):
    """The number a table cell claims, or None if it claims nothing."""
    cell = cell.replace("*", "").strip()
    if cell.startswith("<1"):
        return 0.0
    m = re.match(r"^([0-9]+(?:\.[0-9]+)?)\s*ms$", cell)
    return float(m.group(1)) if m else None


def check_benchmark_table(root, docs):
    """Every performance figure in the READMEs has to be one the checked-in suite produced.

    The README used to carry two performance tables that disagreed with each other, and the
    headline one was measured from sources that no longer existed anywhere in the repository —
    nobody could have rerun it. Numbers in a README are claims like any other.
    """
    measured_path = os.path.join(root, "benchmarks", "results.json")
    if not os.path.exists(measured_path):
        return ["benchmarks/results.json is missing, so no performance claim can be checked"]
    summary = json.load(io.open(measured_path, encoding="utf-8")).get("summary", {})

    out = []
    for doc in docs:
        path = os.path.join(root, doc)
        if not os.path.exists(path):
            continue
        rows = [
            line for line in io.open(path, encoding="utf-8", errors="replace").read().split(chr(10))
            if line.startswith("|") and " ms" in line
        ]
        if not rows:
            continue
        if len(rows) != len(_PERF_ROWS):
            out.append("%s has %d performance rows, the suite measures %d"
                       % (doc, len(rows), len(_PERF_ROWS)))
            continue
        for bench, line in zip(_PERF_ROWS, rows):
            cells = [c for c in line.split("|")[1:] if c.strip()]
            for lang, cell in zip(_PERF_COLUMNS, cells[1:]):
                claimed = _stated(cell)
                if claimed is None:
                    out.append("%s: %s/%s states %r, which is not a time"
                               % (doc, bench, lang, cell.strip()))
                    continue
                actual = summary.get(bench, {}).get(lang, {}).get("self_ms")
                if actual is None:
                    out.append("%s claims a time for %s/%s, which the suite did not measure"
                               % (doc, bench, lang))
                elif abs(float(actual) - claimed) > 0.5:
                    out.append("%s says %s/%s is %gms; the last run measured %gms"
                               % (doc, bench, lang, claimed, float(actual)))
    return out


def _declared_versions(root):
    """Every dependency version the workspace declares, by crate name."""
    found = {}
    for base, _dirs, files in os.walk(os.path.join(root, "varg-compiler")):
        if "target" in base.split(os.sep):
            continue
        if "Cargo.toml" not in files:
            continue
        text = io.open(os.path.join(base, "Cargo.toml"), encoding="utf-8",
                       errors="replace").read()
        for line in text.split(chr(10)):
            line = line.strip()
            if line.startswith("#") or "=" not in line:
                continue
            name = line.split("=", 1)[0].strip()
            if not re.match(r"^[a-z][a-z0-9_-]*$", name):
                continue
            m = re.search(r'version\s*=\s*"([0-9][^"]*)"', line)
            if not m:
                m = re.match(r'^[a-z][a-z0-9_-]*\s*=\s*"([0-9][^"]*)"$', line)
            if m:
                found.setdefault(name, m.group(1))
    return found


def check_module_count(root, docs):
    """The runtime-module count in prose has to be the number the crate declares.

    README.md carried two of them — 40 in the statistics table, 35 in the section below it —
    which is the same defect as the test counts that disagreed, in the same document.

    Only the two shapes that state a count are read: the statistics row, and the sentence form.
    A first attempt matched any number near the words and flagged a table cell listing a crate's
    line count, which is how a gate earns the habit of being ignored.
    """
    lib = os.path.join(root, "varg-compiler", "crates", "varg-runtime", "src", "lib.rs")
    if not os.path.exists(lib):
        return []
    text = io.open(lib, encoding="utf-8", errors="replace").read()
    real = len(set(re.findall(r"pub mod ([a-z_0-9]+);", text)))

    shapes = (
        r"^\|\s*Runtime[- ][Mm]odule[ns]?\s*\|\s*([0-9]+)\s*\|",   # the statistics row
        r"has ([0-9]+) runtime modules",                              # the sentence form
        r"hat ([0-9]+) Runtime-Module",
    )
    out = []
    for doc in docs:
        path = os.path.join(root, doc)
        if not os.path.exists(path):
            continue
        body = io.open(path, encoding="utf-8", errors="replace").read()
        for shape in shapes:
            for stated in re.findall(shape, body, re.M):
                if int(stated) != real:
                    out.append("%s says %s runtime modules; varg-runtime declares %d"
                               % (doc, stated, real))
    return sorted(set(out))


def check_dependency_versions(root, docs):
    """A crate version named in prose has to be the one the workspace actually builds against.

    The runtime-module table names its backends with versions. Upgrading printpdf left the table
    saying the old one, and nothing noticed — the same shape as the test counts that disagreed
    with each other, and the same fix: compare it to the source of truth.
    """
    declared = _declared_versions(root)
    if not declared:
        return ["no Cargo.toml found under varg-compiler, so no version claim can be checked"]

    out = []
    for doc in docs:
        path = os.path.join(root, doc)
        if not os.path.exists(path):
            continue
        text = io.open(path, encoding="utf-8", errors="replace").read()
        # "axum 0.7", "rusqlite 0.31 (bundled)", "printpdf 0.12"
        for name, stated in re.findall(r"\b([a-z][a-z0-9_-]{2,})\s+([0-9]+\.[0-9]+(?:\.[0-9]+)?)\b", text):
            real = declared.get(name)
            if real is None:
                continue
            # A doc may name fewer components than the manifest: "0.7" against "0.7.1" agrees.
            parts = stated.split(".")
            if real.split(".")[: len(parts)] != parts:
                out.append("%s says %s %s; the workspace builds against %s"
                           % (doc, name, stated, real))
    return out


def prose_matches_the_code(root, vargc, code_docs=(), unrunnable=None, all_builtins=()):
    global _ALL_MENTIONED
    _ALL_MENTIONED = set(all_builtins)
    docs = PROSE_DOCS + tuple(code_docs)
    problems = (check_version(root, docs) + check_commands(root, docs, vargc)
                + check_paths(root, docs)
                + check_benchmark_table(root, ("README.md", "README_DE.md"))
                + check_dependency_versions(root, docs)
                + check_module_count(root, ("README.md", "README_DE.md")))
    if unrunnable is not None:
        problems += check_unrun_table(root, unrunnable)
    if problems:
        print("")
        print("--- documentation that does not match the code ---")
        for p in sorted(set(problems)):
            print("   %s" % p)
        print("")
        print("Fix the documentation. These are all claims with one mechanical answer.")
        return 1
    print("prose: versions, commands, paths, dependencies and performance figures match the code")
    return 0
