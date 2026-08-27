# Benchmarks

Five workloads in four languages, with the runner, the recorded results and every sample kept.

The performance figures quoted elsewhere in this repository come from here. Before this was
checked in they came from a directory that was ignored by git, so nobody outside the machine
that produced them could rerun anything or see what had been measured.

## Running it

```bash
python benchmarks/run_all.py
```

It needs `vargc` built in release (`cargo build --release -p vargc`), plus `python`, `node` and
the `dotnet` SDK on PATH. A missing toolchain is reported per language; the rest still run.

Two files are written:

- `BENCHMARKS.md` — the table, regenerated from scratch each run.
- `results.json` — the environment, the summary, and **every individual sample** under `raw`.

## What is measured

Each program times its own work and prints `Time: <n>ms`. The runner takes that figure and also
wall-clock time around the process, 30 times per language per workload, and reports the median,
the p95 and the range.

Varg and C# are timed as their **built artifacts**. The runner used to invoke `vargc run` and
`dotnet run`, which re-enter the compiler and the SDK launcher on every sample: C# measured about
1.1 s against a program reporting 15 ms of its own work. Python and Node are timed through their
interpreter, because that is how those languages are used.

Build time is recorded separately. It is a cost of the toolchain, not of the program.

## The workloads

| Directory | What it exercises |
|-----------|-------------------|
| `fib/` | Recursive calls, integer arithmetic |
| `data/` | Collection building, filter/map chains |
| `json_bench/` | String building, parse and re-serialise |
| `wordfreq/` | Map insertion keyed by strings, hashing |
| `token_compare/` | Source size for equivalent agent programs — not timed |

`wordfreq` was added because Varg lost it, and a suite containing only the workloads a language
wins is advertising. It no longer loses, and how that happened is worth recording, because the
obvious reading of the original result was wrong.

Timed in phases, the counting loop was already the faster of the two — 21 ms against Python's 26.
The whole gap was the final sort: 18 ms against 2. Python's `dict` preserves insertion order, so
`sorted()` received the keys in counting order, long ascending runs that Timsort merges almost for
free; Varg's `map` yielded hash order, effectively shuffled. Rust sorts the *same* keys in
counting order in 2 ms as well, and on genuinely shuffled input Python takes 30 ms against
Rust's 18.

JavaScript's `Map` also keeps insertion order, and C#'s `Dictionary` does in practice for an
insert-only run. Varg had no equivalent, so the four programs were not using comparable
structures — the benchmark was measuring that, not speed. `ordered_map<K, V>` was added to the
language, the Varg program uses it, and the workload went from 31 ms to 15.

Reading that as "the benchmark was changed until it won" would be fair if the change were
arbitrary. It is the opposite: the other three languages never had the choice, and Varg was the
odd one out.

## What the numbers are not

One machine, one session, nothing else forced idle. The ratios between languages are the part
worth reading; the absolute figures belong to the machine named at the top of `BENCHMARKS.md`
and in `results.json`.

Token counts are an estimate at four characters per token, not a tokeniser run.

These are microbenchmarks. They say nothing about how any of these languages behaves in a program
large enough to matter.
