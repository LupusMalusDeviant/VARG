#!/usr/bin/env python3
"""Benchmark Runner: Varg vs Python vs C# vs TypeScript"""

import os
import subprocess
import time
import platform
import statistics
import json
from pathlib import Path

PROJ = Path(__file__).parent.parent
VARGC = PROJ / "varg-compiler" / "target" / "release" / "vargc.exe"
VARG_CWD = PROJ / "varg-compiler"  # vargc must run from here (resolves crates/ relative to CWD)
# Thirty, not five. A median of five samples on a machine that is also doing other things says
# little, and nothing at all about spread — which is the number that tells a real difference from
# noise. Every sample is kept; see `raw` below.
RUNS = 30

# Every measurement taken, keyed by "<benchmark>/<language>". A benchmark that reports one figure
# cannot be argued with, which is the problem.
raw = {}
_current = {"bench": "?", "lang": "?"}


def describe(samples):
    """Median, p95 and the range — from every sample, not one number standing in for them."""
    if not samples:
        return None
    ordered = sorted(samples)
    p95 = ordered[min(len(ordered) - 1, int(round(0.95 * (len(ordered) - 1))))]
    return {
        "median": round(statistics.median(ordered), 1),
        "p95": round(p95, 1),
        "min": round(ordered[0], 1),
        "max": round(ordered[-1], 1),
        "runs": len(ordered),
    }

results = {}

def quoted(path):
    """A command that runs one file, safe for a path with spaces."""
    return chr(34) + str(path) + chr(34)


def artifact(directory, stem):
    """The native binary `vargc build` produced, with or without a Windows suffix."""
    for candidate in (directory / (stem + ".exe"), directory / stem):
        if candidate.exists():
            return candidate
    raise SystemExit("no binary for %s in %s — did the build really succeed?" % (stem, directory))


def dotnet_artifact(proj_dir, stem):
    """A command that runs the compiled C#, preferring the apphost over the SDK launcher.

    Only `dotnet <dll>` exists on platforms without an apphost, and it still skips the project
    re-check that `dotnet run` performs.
    """
    for exe in sorted(proj_dir.glob("bin/Release/*/" + stem + ".exe")):
        return quoted(exe)
    for dll in sorted(proj_dir.glob("bin/Release/*/" + stem + ".dll")):
        return "dotnet " + quoted(dll)
    raise SystemExit("no build output for %s under %s" % (stem, proj_dir))


def run_cmd(cmd, cwd=None, timeout=300, env_extra=None):
    """Run a command and return (stdout, elapsed_ms)."""
    env = os.environ.copy()
    env["DOTNET_NOLOGO"] = "1"
    env["DOTNET_CLI_TELEMETRY_OPTOUT"] = "1"
    if env_extra:
        env.update(env_extra)
    start = time.perf_counter()
    r = subprocess.run(cmd, capture_output=True, text=True, cwd=cwd, timeout=timeout, shell=True, env=env)
    elapsed = (time.perf_counter() - start) * 1000
    return r.stdout.strip(), r.stderr.strip(), elapsed, r.returncode

def measure_exec(cmd, cwd=None, runs=RUNS):
    """Run command multiple times, return (median_wall_ms, median_self_ms)."""
    wall_times = []
    self_times = []
    for i in range(runs):
        stdout, stderr, elapsed, rc = run_cmd(cmd, cwd=cwd)
        if rc != 0:
            if i == 0:
                print(f"    ERROR: {stderr[:200]}")
            return None, None
        wall_times.append(elapsed)
        # Parse self-reported time from output (line: "Time: XXXms")
        for line in stdout.split("\n"):
            if line.strip().startswith("Time:"):
                try:
                    ms = int(line.strip().replace("Time:", "").replace("ms", "").strip())
                    self_times.append(ms)
                except ValueError:
                    pass
    # Keep everything, then report the middle of it.
    raw["%s/%s" % (_current["bench"], _current["lang"])] = {
        "wall_ms": [round(x, 1) for x in wall_times],
        "self_ms": self_times,
        "wall": describe(wall_times),
        "self": describe(self_times),
    }
    wall = statistics.median(wall_times)
    self_t = statistics.median(self_times) if self_times else None
    return wall, self_t

def file_size(path):
    """Return file size in bytes."""
    return os.path.getsize(path) if os.path.exists(path) else 0

def token_estimate(path):
    """Estimate LLM tokens (cl100k_base ~ chars/4)."""
    if not os.path.exists(path):
        return 0
    with open(path, "r", encoding="utf-8") as f:
        text = f.read()
    return len(text) // 4

def find_binary(bench_dir, name):
    """Find compiled binary in varg output."""
    # Varg creates a temp project, binary might be elsewhere
    # Check common locations
    for ext in [".exe", ""]:
        for d in [bench_dir, bench_dir / "target" / "release", bench_dir / "target" / "debug"]:
            p = d / f"{name}{ext}"
            if p.exists():
                return p
    return None

# ============================================================
# BENCHMARKS
# ============================================================

benchmarks = [
    {"name": "fib", "dir": "fib", "desc": "Fibonacci(35) - Pure Compute"},
    {"name": "data", "dir": "data", "desc": "Data Pipeline - Collections"},
    {"name": "json_bench", "dir": "json_bench", "desc": "JSON Processing - Strings/Alloc"},
    # The one Varg loses. A suite holding only what a language wins is advertising, and a reader
    # cannot tell which it has without a case that goes the other way.
    {"name": "wordfreq", "dir": "wordfreq", "desc": "Word frequency, 200k distinct - Alloc/Hash"},
]

print("=" * 70)
print("  VARG BENCHMARK SUITE")
print("  Varg vs Python vs C# vs TypeScript")
print("=" * 70)
print()

for bench in benchmarks:
    name = bench["name"]
    bdir = Path(__file__).parent / bench["dir"]
    print(f"\n--- {bench['desc']} ---\n")
    results[name] = {}

    # --- Source sizes ---
    for lang, ext in [("Varg", ".varg"), ("Python", ".py"), ("C#", ".cs"), ("TypeScript", ".ts")]:
        src = bdir / f"{name}{ext}"
        sz = file_size(src)
        tok = token_estimate(src)
        results[name].setdefault(lang, {})["source_bytes"] = sz
        results[name][lang]["tokens"] = tok
        results[name][lang]["source_lines"] = len(open(src, encoding="utf-8").readlines()) if src.exists() else 0

    # --- Varg (must run from varg-compiler/ dir) ---
    print(f"  [Varg] Building {name}.varg ...")
    varg_src = bdir.resolve() / f"{name}.varg"
    _, stderr, build_time, rc = run_cmd(f'"{VARGC}" build "{varg_src}"', cwd=VARG_CWD)
    if rc != 0:
        print(f"    BUILD FAILED: {stderr[:200]}")
        results[name]["Varg"]["build_ms"] = None
        results[name]["Varg"]["exec_ms"] = None
    else:
        results[name]["Varg"]["build_ms"] = round(build_time)
        print(f"    Build: {build_time:.0f}ms")
        # `vargc build` above already produced a native binary. This used to measure
        # `vargc run`, which re-enters the compiler on every one of the 30 samples: the wall
        # figure was the cache check plus cargo, not the program. Run the artifact.
        print(f"  [Varg] Running {name} (x{RUNS}) ...")
        _current.update(bench=name, lang="Varg")
        varg_exe = artifact(VARG_CWD, name)
        wall, self_t = measure_exec(quoted(varg_exe), cwd=VARG_CWD)
        if wall is not None:
            results[name]["Varg"]["exec_ms"] = round(wall)
            results[name]["Varg"]["self_ms"] = self_t
            print(f"    Wall (median): {wall:.0f}ms | Self-reported: {self_t}ms")
        else:
            results[name]["Varg"]["exec_ms"] = None
            results[name]["Varg"]["self_ms"] = None
            print(f"    EXEC FAILED")

    # --- Python ---
    print(f"  [Python] Running {name}.py (x{RUNS}) ...")
    py_src = bdir / f"{name}.py"
    results[name]["Python"]["build_ms"] = 0  # interpreted
    _current.update(bench=name, lang="Python")
    wall, self_t = measure_exec(f'python "{py_src}"', cwd=bdir)
    if wall is not None:
        results[name]["Python"]["exec_ms"] = round(wall)
        results[name]["Python"]["self_ms"] = self_t
        print(f"    Wall (median): {wall:.0f}ms | Self-reported: {self_t}ms")
    else:
        results[name]["Python"]["exec_ms"] = None
        results[name]["Python"]["self_ms"] = None
        print(f"    EXEC FAILED")

    # --- C# ---
    print(f"  [C#] Setting up {name}.cs ...")
    cs_proj_dir = bdir / "cs_proj"
    if not (cs_proj_dir / f"{name}_cs.csproj").exists():
        os.makedirs(cs_proj_dir, exist_ok=True)
        # Create minimal .csproj
        csproj = f"""<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net10.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
  </PropertyGroup>
</Project>"""
        with open(cs_proj_dir / f"{name}_cs.csproj", "w") as f:
            f.write(csproj)
        # Copy source
        import shutil
        shutil.copy(bdir / f"{name}.cs", cs_proj_dir / "Program.cs")

    # Build
    _, stderr, build_time, rc = run_cmd(f'dotnet build "{cs_proj_dir / f"{name}_cs.csproj"}" -c Release -v q', cwd=cs_proj_dir)
    if rc != 0:
        print(f"    BUILD FAILED: {stderr[:200]}")
        results[name]["C#"]["build_ms"] = None
        results[name]["C#"]["exec_ms"] = None
    else:
        results[name]["C#"]["build_ms"] = round(build_time)
        print(f"    Build: {build_time:.0f}ms")
        # `dotnet run` re-checks the project and starts the SDK host on every sample: it
        # measured about 1.1 s against a program reporting 15 ms of its own. Run what the
        # build produced, the same way Varg is run.
        print(f"  [C#] Running {name} (x{RUNS}) ...")
        _current.update(bench=name, lang="C#")
        cs_exe = dotnet_artifact(cs_proj_dir, name + "_cs")
        wall, self_t = measure_exec(cs_exe, cwd=cs_proj_dir)
        if wall is not None:
            results[name]["C#"]["exec_ms"] = round(wall)
            results[name]["C#"]["self_ms"] = self_t
            print(f"    Wall (median): {wall:.0f}ms | Self-reported: {self_t}ms")
        else:
            results[name]["C#"]["exec_ms"] = None
            results[name]["C#"]["self_ms"] = None
            print(f"    EXEC FAILED")

    # --- TypeScript (via Node.js --experimental-strip-types) ---
    print(f"  [TypeScript] Running {name}.ts (x{RUNS}) ...")
    ts_src = bdir / f"{name}.ts"
    results[name]["TypeScript"]["build_ms"] = 0  # JIT
    _current.update(bench=name, lang="TypeScript")
    wall, self_t = measure_exec(f'node --experimental-strip-types "{ts_src}"', cwd=bdir)
    if wall is not None:
        results[name]["TypeScript"]["exec_ms"] = round(wall)
        results[name]["TypeScript"]["self_ms"] = self_t
        print(f"    Wall (median): {wall:.0f}ms | Self-reported: {self_t}ms")
    else:
        results[name]["TypeScript"]["exec_ms"] = None
        results[name]["TypeScript"]["self_ms"] = None
        print(f"    EXEC FAILED")


# ============================================================
# GENERATE REPORT
# ============================================================

print("\n\n" + "=" * 70)
print("  GENERATING REPORT")
print("=" * 70)

report = """# Varg Benchmark Results

> Varg vs Python vs C# vs TypeScript
> Machine: {machine}, measured {date}
> Runs per benchmark: {runs} (median taken)

## Summary

"""

# Summary table (self-reported computation time)
report += "| Benchmark | Varg | Python | C# | TypeScript |\n"
report += "|-----------|------|--------|----|------------|\n"
for bench in benchmarks:
    name = bench["name"]
    row = f"| {bench['desc']} |"
    for lang in ["Varg", "Python", "C#", "TypeScript"]:
        ms = results[name].get(lang, {}).get("self_ms")
        if ms == 0:
            # Same as the per-benchmark tables: the programs count whole milliseconds, so a
            # zero is the clock running out of resolution, not a measurement of nothing.
            row += " <1ms |"
        elif ms is not None:
            row += f" {ms}ms |"
        else:
            ms2 = results[name].get(lang, {}).get("exec_ms")
            row += f" {ms2}ms* |" if ms2 is not None else " FAIL |"
    report += row + "\n"

report += "\n---\n\n"

# Detailed per-benchmark
for bench in benchmarks:
    name = bench["name"]
    report += f"## {bench['desc']}\n\n"
    report += "| Metric | Varg | Python | C# | TypeScript |\n"
    report += "|--------|------|--------|----|------------|\n"

    # Source size
    row = "| Source Size |"
    for lang in ["Varg", "Python", "C#", "TypeScript"]:
        b = results[name].get(lang, {}).get("source_bytes", 0)
        row += f" {b} B |"
    report += row + "\n"

    # Lines
    row = "| Lines of Code |"
    for lang in ["Varg", "Python", "C#", "TypeScript"]:
        l = results[name].get(lang, {}).get("source_lines", 0)
        row += f" {l} |"
    report += row + "\n"

    # Tokens
    row = "| LLM Tokens (est.) |"
    for lang in ["Varg", "Python", "C#", "TypeScript"]:
        t = results[name].get(lang, {}).get("tokens", 0)
        row += f" ~{t} |"
    report += row + "\n"

    # Build time
    row = "| Build Time |"
    for lang in ["Varg", "Python", "C#", "TypeScript"]:
        ms = results[name].get(lang, {}).get("build_ms")
        if ms is None:
            row += " FAIL |"
        elif ms == 0:
            row += " N/A (interpreted) |"
        else:
            row += f" {ms}ms |"
    report += row + "\n"

    # Wall time around the process: for the two compiled languages this is the built binary,
    # not the compiler or the SDK launcher.
    row = "| Wall time, median |"
    for lang in ["Varg", "Python", "C#", "TypeScript"]:
        ms = results[name].get(lang, {}).get("exec_ms")
        row += f" {ms}ms |" if ms is not None else " FAIL |"
    report += row + "\n"

    # A median on its own hides whether a gap is real. p95 over the same 30 samples.
    row = "| Wall time, p95 |"
    for lang in ["Varg", "Python", "C#", "TypeScript"]:
        d = raw.get(f"{name}/{lang}", {}).get("wall")
        row += f" {d['p95']:.0f}ms |" if d else " - |"
    report += row + "\n"

    # What the program measured around its own work.
    row = "| **Computation time** |"
    for lang in ["Varg", "Python", "C#", "TypeScript"]:
        ms = results[name].get(lang, {}).get("self_ms")
        if ms is None:
            row += " - |"
        elif ms == 0:
            # The programs time themselves in whole milliseconds. Zero means "under the
            # resolution of the clock", which is not the same as "instant" — say so.
            row += " **<1ms** |"
        else:
            row += f" **{ms}ms** |"
    report += row + "\n"

    # Speed comparison vs Python (using self-reported time)
    py_ms = results[name].get("Python", {}).get("self_ms")
    if py_ms is not None and py_ms > 0:
        row = "| vs Python |"
        for lang in ["Varg", "Python", "C#", "TypeScript"]:
            ms = results[name].get(lang, {}).get("self_ms")
            if ms is not None and ms > 0:
                ratio = py_ms / ms
                row += f" **{ratio:.1f}x** |"
            elif ms is not None and ms == 0:
                # This used to read ">100x" whatever the numbers were. All the samples support
                # is that the work took under a millisecond, so the ratio is above py_ms/1.
                row += f" **>{py_ms:.0f}x** |"
            else:
                row += " - |"
        report += row + "\n"

    report += "\n"

# Token efficiency section
report += """---

## Token Efficiency (LLM Cost)

How many tokens does each language need for equivalent functionality?

"""
report += "| Benchmark | Varg | Python | C# | TypeScript |\n"
report += "|-----------|------|--------|----|------------|\n"
for bench in benchmarks:
    name = bench["name"]
    row = f"| {bench['desc']} |"
    for lang in ["Varg", "Python", "C#", "TypeScript"]:
        t = results[name].get(lang, {}).get("tokens", 0)
        row += f" ~{t} |"
    report += row + "\n"

varg_total = sum(results[b["name"]].get("Varg", {}).get("tokens", 0) for b in benchmarks)
py_total = sum(results[b["name"]].get("Python", {}).get("tokens", 0) for b in benchmarks)
cs_total = sum(results[b["name"]].get("C#", {}).get("tokens", 0) for b in benchmarks)
ts_total = sum(results[b["name"]].get("TypeScript", {}).get("tokens", 0) for b in benchmarks)

report += f"| **Total** | **~{varg_total}** | **~{py_total}** | **~{cs_total}** | **~{ts_total}** |\n"

report += "\n---\n\n"
report += "## Key Takeaways\n\n"
report += "- Varg compiles to a native binary, so its computation time sits with the compiled languages rather than the interpreted ones.\n"
report += "- `wordfreq` is here because Varg loses it: a map keyed by strings copies the key on every new entry, and the hash is SipHash. A suite holding only what a language wins is advertising.\n"
report += "- Build time is a full Rust compilation. It is a cost of the toolchain, not of the program, and is listed on its own line for that reason.\n"
report += "- Token counts are an estimate at four characters per token, not a tokeniser run.\n"
report += "- Python and Node are timed through their interpreter, which is how they are used. Varg and C# are timed as their built artifacts, so nothing here includes a compiler or an SDK launcher.\n"

report = report.replace("{date}", time.strftime("%Y-%m-%d"))
report = report.replace(
    "{machine}", f"{platform.system()} {platform.release()} ({platform.machine()})")
report = report.replace("{runs}", str(RUNS))

outpath = Path(__file__).parent / "BENCHMARKS.md"
with open(outpath, "w", encoding="utf-8") as f:
    f.write(report)

print(f"\nReport written to {outpath}")

# The numbers alone are not a measurement. A reader who cannot see the machine, the versions and
# the spread cannot tell whether a difference is real or whether it would hold anywhere else.
def tool_version(cmd):
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, shell=True, timeout=60)
        return (r.stdout + r.stderr).strip().splitlines()[0]
    except Exception:
        return "unavailable"


environment = {
    "recorded": time.strftime("%Y-%m-%d %H:%M"),
    "os": f"{platform.system()} {platform.release()} ({platform.machine()})",
    "cpu": platform.processor() or "unknown",
    "python": platform.python_version(),
    "vargc": tool_version(f'"{VARGC}" --version'),
    "rustc": tool_version("rustc --version"),
    "dotnet": tool_version("dotnet --version"),
    "node": tool_version("node --version"),
    "runs_per_measurement": RUNS,
    "build_flags": {
        "varg": "vargc build (release profile, opt-level 3)",
        "csharp": "dotnet build -c Release",
        "node": "node <file> (no flags)",
        "python": "python <file> (no flags)",
    },
    "timing": (
        "Two figures per run: wall time around the process, and the time the program measured "
        "around the work itself. The difference between them is startup cost."
    ),
    "caveat": (
        "One machine, one session, nothing else idle-enforced. Ratios between languages are the "
        "part worth reading; absolute numbers belong to this machine."
    ),
}

with open(Path(__file__).parent / "results.json", "w") as f:
    json.dump({"environment": environment, "summary": results, "raw": raw}, f, indent=2)

print("Raw data written to results.json")
print("\nDone!")
