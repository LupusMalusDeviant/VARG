#!/usr/bin/env python3
"""Two gates on the documentation: does it compile, and does it cover the language?

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

The second gate asks a different question: is every builtin the compiler knows mentioned
anywhere at all? 117 were reachable and undocumented, including the ones a web-facing agent
reaches for first — ws_route, sse_open, http_response_json. Retired builtins are exempt, read
from the compiler's own retirement table so retiring one exempts it automatically.

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


UNDECLARED = re.compile(r"undeclared variable `([A-Za-z_]\w*)`")
ARITY = re.compile(r"expected `[^`]*argument")


def arity_behind_placeholders(body, tag):
    """An arity error hiding behind an undeclared name in the same block.

    A block is classified by its first error, so one placeholder masks everything after it. That
    is how `fs_write("trace.json", json, files)` sat in the guide unnoticed: `files` was
    undeclared, the block read as illustrative, and the extra argument was never reached.

    Undeclared names are declared away one at a time and the block re-checked. Only an *arity*
    complaint is escalated, because how many arguments a call takes does not depend on what the
    placeholder would have held -- any other error might just be the stand-in having the wrong
    type, which is not something to fail a build over.
    """
    decls = []
    for _ in range(12):
        src = ("agent DocProbe {\n    public void Run() {\n        unsafe {\n"
               + "\n".join(decls) + "\n" + body + "\n        }\n    }\n}\n")
        good, txt = check(src, tag + "_p")
        if good:
            return None
        m = re.search(r"error: (.+)", txt)
        err = m.group(1).strip() if m else ""
        if ARITY.search(err):
            return err
        u = UNDECLARED.search(err)
        if not u:
            return None
        decls.append('var %s = "";' % u.group(1))
    return None


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
            if kind == "PLACEHOLDER":
                hidden = arity_behind_placeholders(body, "b_%s_%d" % (doc[:3].lower(), n))
                if hidden:
                    kind, err = "REAL", hidden
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

    return coverage()


# ── Second gate: is every builtin mentioned at all? ──────────────────────────

TC = os.path.join(ROOT, "varg-compiler", "crates", "varg-typechecker", "src", "lib.rs")

# Retired by a rule rather than by the RETIRED table, so their names are still known to the
# compiler. Each is rejected with a pointer to what replaced it; documenting them would be
# advertising an API that refuses to compile.
RETIRED_ELSEWHERE = {
    "sse_stream": "rejected in favour of sse_open/sse_push",
    "sse_send": "rejected in favour of sse_open/sse_push",
    "sse_close": "rejected in favour of sse_shutdown",
}


def coverage():
    """Fail on a builtin no document mentions.

    117 of them were reachable and undocumented, including the ones a web-facing agent needs
    first -- `ws_route`, `sse_open`, `http_response_json`. A builtin nobody can find is a
    builtin nobody uses, and for an agent generating code from these pages it may as well not
    exist.
    """
    src = io.open(TC, encoding="utf-8", newline="").read().replace("\r\n", "\n")
    body = src[:src.index("mod tests {")] if "mod tests {" in src else src

    # The retirement table is the authority, so retiring a builtin exempts it automatically.
    retired = set(RETIRED_ELSEWHERE)
    m = re.search(r"const RETIRED: &\[\(&str, &str, &str\)\] = &\[(.*?)\n                \];",
                  body, re.S)
    if m:
        retired |= set(re.findall(r'\("([a-z_0-9]+)"', m.group(1)))

    names = {n for n in re.findall(r'method_name == "([a-z_0-9]+)"', body)
             if not n.startswith("__varg")}

    doctext = ""
    for d in DOCS:
        doctext += io.open(os.path.join(ROOT, d), encoding="utf-8", newline="").read()

    missing = sorted(n for n in names - retired
                     if not re.search(r"\b%s\b" % re.escape(n), doctext))

    print("\nbuiltins: %d   retired: %d   undocumented: %d"
          % (len(names), len(retired), len(missing)))
    if missing:
        print("\n--- builtins no document mentions ---")
        for n in missing:
            print("   %s" % n)
        print("\nDocument them, or retire them so the compiler says what to use instead.")
        return 1

    print("\n--- docs: OK (every builtin is documented, and none contradicts the compiler) ---")
    return 0


if __name__ == "__main__":
    sys.exit(main())
