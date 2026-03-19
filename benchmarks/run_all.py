#!/usr/bin/env python3
"""Benchmark Runner: Varg vs Python vs C# vs TypeScript"""

import os
import subprocess
import time
import statistics
import json
from pathlib import Path

PROJ = Path(__file__).parent.parent
VARGC = PROJ / "varg-compiler" / "target" / "release" / "vargc.exe"
VARG_CWD = PROJ / "varg-compiler"  # vargc must run from here (resolves crates/ relative to CWD)
RUNS = 5

results = {}

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
        # Use `vargc run` (includes compile, but uses cache for incremental)
        print(f"  [Varg] Running {name}.varg (x{RUNS}) ...")
        wall, self_t = measure_exec(f'"{VARGC}" run "{varg_src}"', cwd=VARG_CWD)
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
        # Run
        print(f"  [C#] Running {name}.cs (x{RUNS}) ...")
        wall, self_t = measure_exec(f'dotnet run --project "{cs_proj_dir / f"{name}_cs.csproj"}" -c Release', cwd=cs_proj_dir)
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
> Machine: Windows 11, measured {date}
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
        if ms is not None:
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

    # Wall time (includes process startup + compilation)
    row = "| Wall Time (total) |"
    for lang in ["Varg", "Python", "C#", "TypeScript"]:
        ms = results[name].get(lang, {}).get("exec_ms")
        row += f" {ms}ms |" if ms is not None else " FAIL |"
    report += row + "\n"

    # Self-reported time (pure computation)
    row = "| **Computation Time** |"
    for lang in ["Varg", "Python", "C#", "TypeScript"]:
        ms = results[name].get(lang, {}).get("self_ms")
        row += f" **{ms}ms** |" if ms is not None else " - |"
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
                row += " **>100x** |"
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
report += "- **Varg compiles to native Rust binaries** -- execution speed matches Rust/C level\n"
report += "- **Build time includes full Rust compilation** -- first build is slow, incremental builds faster\n"
report += "- **Token cost** -- Varg is more verbose than Python but compiles to native code; the extra tokens buy performance\n"
report += "- **Python** -- easiest to write but slowest to execute (interpreted)\n"
report += "- **C#/.NET** -- good balance of speed and productivity, but needs runtime\n"
report += "- **TypeScript/Node** -- V8 JIT gives good performance, but no native binary\n"
report += "\n> Note: Varg 'execution time' via `vargc run` includes compilation overhead.\n"
report += "> For production, use `vargc build` once, then run the native binary directly.\n"

report = report.replace("{date}", time.strftime("%Y-%m-%d"))
report = report.replace("{runs}", str(RUNS))

outpath = Path(__file__).parent / "BENCHMARKS.md"
with open(outpath, "w", encoding="utf-8") as f:
    f.write(report)

print(f"\nReport written to {outpath}")

# Also dump raw JSON
with open(Path(__file__).parent / "results.json", "w") as f:
    json.dump(results, f, indent=2)

print("Raw data written to results.json")
print("\nDone!")
