#!/usr/bin/env python3
"""Does the documentation compile?

Four rounds of this session found the same class of defect: a signature in REFERENCE.md or
VARG_AGENT_GUIDE.md that the compiler does not accept. Nothing caught it, because nothing ever
fed the documentation to the compiler. Someone reading the page writes what it says and gets an
error -- and for an agent generating code from these pages, every wrong signature is a wrong
program.

Every ```csharp block is fed to `vargc check`. Most are fragments, so each is tried in three
shapes and passes if any of them checks out:

  1. as written (a complete program),
  2. wrapped in an agent method,
  3. the same inside `unsafe {}`, so a capability gate is not the reason it fails.

Failures are then split, because they do not all mean the same thing:

  PLACEHOLDER  the snippet uses a name it never declares (`print $"{x}"`). Illustrative.
  FRAGMENT     a signature, an operator table, an annotation shown on its own.
  REAL         written as complete code and does not compile. This is the failure that matters,
               and the exit code is driven by it alone.

REAL failures that name a helper the reader is expected to write are listed in ALLOWED below,
each with its reason. Anything else fails the check.

Usage:  VARGC=/path/to/vargc python check.py [--list]
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
DOCS = ("REFERENCE.md", "VARG_AGENT_GUIDE.md")

VARGC = os.path.abspath(os.environ.get("VARGC") or os.path.join(
    ROOT, "varg-compiler", "target", "release", "vargc.exe"))

# Blocks that call a helper the surrounding prose asks the reader to define. The name is what
# makes them unresolvable in isolation, and defining it in the snippet would obscure the point
# being made. Keyed by the identifier the error names, so a block going wrong for any *other*
# reason still fails.
ALLOWED = {
    "add": "arithmetic helper, defined by the reader in the paragraph above",
    "getTuple": "stands in for any function returning a tuple",
    "Method": "a method signature shown on its own, not a call",
    "risky_call": "stands in for whatever the reader wraps in try/catch",
    "Worker": "the agent being spawned is the reader's own",
    "deploy": "stands in for the action behind an approval gate",
    "execute_step": "stands in for the reader's workflow step",
}

WRAPS = [
    ("as-is", "%s\n"),
    ("in-method", "agent DocProbe {\n    public void Run() {\n%s\n    }\n}\n"),
    ("in-unsafe",
     "agent DocProbe {\n    public void Run() {\n        unsafe {\n%s\n        }\n    }\n}\n"),
]

TMP = os.path.join(HERE, ".blocks")


def blocks(path):
    raw = io.open(path, encoding="utf-8", newline="").read().replace("\r\n", "\n")
    out, lines, i = [], raw.split("\n"), 0
    while i < len(lines):
        m = re.match(r"^```(\w*)\s*$", lines[i])
        if m:
            lang, start, i = m.group(1), i + 1, i + 1
            body = []
            while i < len(lines) and not lines[i].startswith("```"):
                body.append(lines[i])
                i += 1
            if lang.lower() in ("csharp", "varg", "cs"):
                out.append((start + 1, "\n".join(body)))
        i += 1
    return out


def check(src, tag):
    f = os.path.join(TMP, tag + ".varg")
    io.open(f, "w", encoding="utf-8", newline="\n").write(src)
    r = subprocess.run([VARGC, "check", f], capture_output=True, text=True, errors="replace")
    txt = re.sub(r"\x1b\[[0-9;]*m", "", (r.stdout or "") + (r.stderr or ""))
    return ("no type errors" in txt), txt


def classify(err):
    if "undeclared variable" in err:
        return "PLACEHOLDER"
    if "unexpected token" in err or "unexpected end of file" in err:
        return "FRAGMENT"
    return "REAL"


def allowed_for(err):
    for name, reason in ALLOWED.items():
        if "`%s`" % name in err:
            return name, reason
    return None, None


def main():
    if not os.path.exists(VARGC):
        print("VARGC not found: %s" % VARGC)
        print("Build it with: cargo build --release -p vargc")
        return 2
    os.makedirs(TMP, exist_ok=True)

    rows, unexpected = [], []
    for doc in DOCS:
        for n, (line, body) in enumerate(blocks(os.path.join(ROOT, doc))):
            if not body.strip():
                continue
            ok, err = False, ""
            for _, tmpl in WRAPS:
                good, txt = check(tmpl % body, "b_%s_%d" % (doc[:3].lower(), n))
                if good:
                    ok = True
                    break
                m = re.search(r"error: (.+)", txt)
                err = m.group(1).strip() if m else txt.strip().split("\n")[0]
            kind = "" if ok else classify(err)
            rows.append((doc, line, ok, kind, err))
            if kind == "REAL":
                name, _ = allowed_for(err)
                if not name:
                    unexpected.append((doc, line, err))

    total = len(rows)
    ok = sum(1 for r in rows if r[2])
    print("documentation blocks: %d   compile: %d" % (total, ok))
    for kind in ("PLACEHOLDER", "FRAGMENT", "REAL"):
        print("   %-12s %3d" % (kind, sum(1 for r in rows if r[3] == kind)))

    if "--list" in sys.argv:
        print()
        for doc, line, good, kind, err in rows:
            if kind:
                print("   %-20s :%-5d %-11s %s" % (doc[:20], line, kind, err[:80]))

    if unexpected:
        print("\n--- documented code that does not compile ---")
        for doc, line, err in unexpected:
            print("   %s:%d" % (doc, line))
            print("      %s" % err)
        print("\nFix the documentation, or add the helper name to ALLOWED with its reason.")
        return 1

    print("\n--- docs: OK (no documented signature contradicts the compiler) ---")
    return 0


if __name__ == "__main__":
    sys.exit(main())
