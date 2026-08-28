# Varg Benchmark Results

> Varg vs Python vs C# vs TypeScript
> Machine: Windows 11 (AMD64), measured 2026-08-28
> Runs per benchmark: 30 (median taken)

## Summary

| Benchmark | Varg | Python | C# | TypeScript |
|-----------|------|--------|----|------------|
| Fibonacci(35) - Pure Compute | 16.0ms | 720.5ms | 53.0ms | 53.0ms |
| Data Pipeline - Collections | 1.0ms | 9.0ms | 12.0ms | 4.0ms |
| JSON Processing - Strings/Alloc | 1.0ms | 2.0ms | 21.0ms | <1ms |
| Word frequency, 200k distinct - Alloc/Hash | 15.0ms | 27.0ms | 58.0ms | 36.0ms |

---

## Fibonacci(35) - Pure Compute

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 268 B | 217 B | 291 B | 292 B |
| Lines of Code | 12 | 10 | 13 | 10 |
| LLM Tokens (est.) | ~64 | ~54 | ~69 | ~70 |
| Build Time | 452ms | N/A (interpreted) | 759ms | N/A (interpreted) |
| Wall time, median | 31ms | 753ms | 86ms | 111ms |
| Wall time, p95 | 39ms | 1118ms | 92ms | 125ms |
| Wall time, cold (1st run) | 62ms | 791ms | 92ms | 119ms |
| Start-up | 15ms | 32ms | 33ms | 58ms |
| Artifact | 256 KB | 0 KB | 164 KB | 0 KB |
| **Computation time** | **16.0ms** | **720.5ms** | **53.0ms** | **53.0ms** |
| vs Python | **45.0x** | **1.0x** | **13.6x** | **13.6x** |

## Data Pipeline - Collections

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 1090 B | 654 B | 992 B | 809 B |
| Lines of Code | 35 | 25 | 26 | 22 |
| LLM Tokens (est.) | ~272 | ~163 | ~248 | ~202 |
| Build Time | 512ms | N/A (interpreted) | 755ms | N/A (interpreted) |
| Wall time, median | 16ms | 37ms | 46ms | 61ms |
| Wall time, p95 | 16ms | 41ms | 50ms | 67ms |
| Wall time, cold (1st run) | 39ms | 41ms | 52ms | 73ms |
| Start-up | 15ms | 28ms | 34ms | 57ms |
| Artifact | 269 KB | 1 KB | 165 KB | 1 KB |
| **Computation time** | **1.0ms** | **9.0ms** | **12.0ms** | **4.0ms** |
| vs Python | **9.0x** | **1.0x** | **0.8x** | **2.2x** |

## JSON Processing - Strings/Alloc

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 913 B | 747 B | 892 B | 618 B |
| Lines of Code | 27 | 22 | 25 | 18 |
| LLM Tokens (est.) | ~221 | ~186 | ~223 | ~154 |
| Build Time | 519ms | N/A (interpreted) | 754ms | N/A (interpreted) |
| Wall time, median | 16ms | 39ms | 54ms | 59ms |
| Wall time, p95 | 17ms | 40ms | 59ms | 73ms |
| Wall time, cold (1st run) | 46ms | 44ms | 61ms | 63ms |
| Start-up | 15ms | 37ms | 33ms | 59ms |
| Artifact | 329 KB | 1 KB | 164 KB | 1 KB |
| **Computation time** | **1.0ms** | **2.0ms** | **21.0ms** | **<1ms** |
| vs Python | **2.0x** | **1.0x** | **0.1x** | **>2x** |

## Word frequency, 200k distinct - Alloc/Hash

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 1403 B | 323 B | 751 B | 425 B |
| Lines of Code | 34 | 12 | 21 | 11 |
| LLM Tokens (est.) | ~350 | ~80 | ~187 | ~106 |
| Build Time | 560ms | N/A (interpreted) | 792ms | N/A (interpreted) |
| Wall time, median | 39ms | 78ms | 111ms | 116ms |
| Wall time, p95 | 40ms | 82ms | 130ms | 130ms |
| Wall time, cold (1st run) | 68ms | 81ms | 115ms | 120ms |
| Start-up | 24ms | 51ms | 53ms | 80ms |
| Artifact | 300 KB | 0 KB | 164 KB | 0 KB |
| **Computation time** | **15.0ms** | **27.0ms** | **58.0ms** | **36.0ms** |
| vs Python | **1.8x** | **1.0x** | **0.5x** | **0.8x** |

---

## Token Efficiency (LLM Cost)

How many tokens does each language need for equivalent functionality?

| Benchmark | Varg | Python | C# | TypeScript |
|-----------|------|--------|----|------------|
| Fibonacci(35) - Pure Compute | ~64 | ~54 | ~69 | ~70 |
| Data Pipeline - Collections | ~272 | ~163 | ~248 | ~202 |
| JSON Processing - Strings/Alloc | ~221 | ~186 | ~223 | ~154 |
| Word frequency, 200k distinct - Alloc/Hash | ~350 | ~80 | ~187 | ~106 |
| **Total** | **~907** | **~483** | **~727** | **~532** |

---

## Key Takeaways

- Varg compiles to a native binary, so its computation time sits with the compiled languages rather than the interpreted ones.
- `wordfreq` is here because Varg loses it: a map keyed by strings copies the key on every new entry, and the hash is SipHash. A suite holding only what a language wins is advertising.
- Build time is a full Rust compilation. It is a cost of the toolchain, not of the program, and is listed on its own line for that reason.
- Token counts are an estimate at four characters per token, not a tokeniser run.
- `results.json` also holds, per workload and language: the first run on its own (cold), the runs after it (warm), start-up time as wall minus the program's own measurement, and the size of what has to be distributed. A native binary and a script are not comparable by size, which is why they are recorded rather than ranked.
- Python and Node are timed through their interpreter, which is how they are used. Varg and C# are timed as their built artifacts, so nothing here includes a compiler or an SDK launcher.
