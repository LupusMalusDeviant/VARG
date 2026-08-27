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


def prose_matches_the_code(root, vargc, code_docs=()):
    docs = PROSE_DOCS + tuple(code_docs)
    problems = check_version(root, docs) + check_commands(root, docs, vargc) + check_paths(root, docs)
    if problems:
        print("")
        print("--- documentation that does not match the code ---")
        for p in sorted(set(problems)):
            print("   %s" % p)
        print("")
        print("Fix the documentation. These are all claims with one mechanical answer.")
        return 1
    print("prose: versions, commands and paths match the code")
    return 0
