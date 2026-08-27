# Varg Benchmark Results

> Varg vs Python vs C# vs TypeScript
> Machine: Windows 11 (AMD64), measured 2026-08-27
> Runs per benchmark: 30 (median taken)

## Summary

| Benchmark | Varg | Python | C# | TypeScript |
|-----------|------|--------|----|------------|
| Fibonacci(35) - Pure Compute | 16.0ms | 700.0ms | 53.0ms | 53.0ms |
| Data Pipeline - Collections | 1.0ms | 9.0ms | 12.0ms | 4.0ms |
| JSON Processing - Strings/Alloc | 1.0ms | 2.0ms | 21.5ms | <1ms |
| Word frequency, 200k distinct - Alloc/Hash | 35.0ms | 27.0ms | 64.0ms | 36.0ms |

---

## Fibonacci(35) - Pure Compute

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 268 B | 217 B | 291 B | 292 B |
| Lines of Code | 12 | 10 | 13 | 10 |
| LLM Tokens (est.) | ~64 | ~54 | ~69 | ~70 |
| Build Time | 488ms | N/A (interpreted) | 796ms | N/A (interpreted) |
| Wall time, median | 39ms | 739ms | 94ms | 121ms |
| Wall time, p95 | 42ms | 789ms | 99ms | 132ms |
| **Computation time** | **16.0ms** | **700.0ms** | **53.0ms** | **53.0ms** |
| vs Python | **43.8x** | **1.0x** | **13.2x** | **13.2x** |

## Data Pipeline - Collections

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 1090 B | 654 B | 992 B | 809 B |
| Lines of Code | 35 | 25 | 26 | 22 |
| LLM Tokens (est.) | ~272 | ~163 | ~248 | ~202 |
| Build Time | 581ms | N/A (interpreted) | 786ms | N/A (interpreted) |
| Wall time, median | 24ms | 45ms | 54ms | 70ms |
| Wall time, p95 | 26ms | 50ms | 63ms | 85ms |
| **Computation time** | **1.0ms** | **9.0ms** | **12.0ms** | **4.0ms** |
| vs Python | **9.0x** | **1.0x** | **0.8x** | **2.2x** |

## JSON Processing - Strings/Alloc

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 913 B | 747 B | 892 B | 618 B |
| Lines of Code | 27 | 22 | 25 | 18 |
| LLM Tokens (est.) | ~221 | ~186 | ~223 | ~154 |
| Build Time | 568ms | N/A (interpreted) | 773ms | N/A (interpreted) |
| Wall time, median | 24ms | 47ms | 62ms | 67ms |
| Wall time, p95 | 26ms | 52ms | 66ms | 78ms |
| **Computation time** | **1.0ms** | **2.0ms** | **21.5ms** | **<1ms** |
| vs Python | **2.0x** | **1.0x** | **0.1x** | **>2x** |

## Word frequency, 200k distinct - Alloc/Hash

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 858 B | 323 B | 751 B | 425 B |
| Lines of Code | 28 | 12 | 21 | 11 |
| LLM Tokens (est.) | ~214 | ~80 | ~187 | ~106 |
| Build Time | 629ms | N/A (interpreted) | 777ms | N/A (interpreted) |
| Wall time, median | 68ms | 85ms | 125ms | 123ms |
| Wall time, p95 | 82ms | 88ms | 130ms | 139ms |
| **Computation time** | **35.0ms** | **27.0ms** | **64.0ms** | **36.0ms** |
| vs Python | **0.8x** | **1.0x** | **0.4x** | **0.8x** |

---

## Token Efficiency (LLM Cost)

How many tokens does each language need for equivalent functionality?

| Benchmark | Varg | Python | C# | TypeScript |
|-----------|------|--------|----|------------|
| Fibonacci(35) - Pure Compute | ~64 | ~54 | ~69 | ~70 |
| Data Pipeline - Collections | ~272 | ~163 | ~248 | ~202 |
| JSON Processing - Strings/Alloc | ~221 | ~186 | ~223 | ~154 |
| Word frequency, 200k distinct - Alloc/Hash | ~214 | ~80 | ~187 | ~106 |
| **Total** | **~771** | **~483** | **~727** | **~532** |

---

## Key Takeaways

- Varg compiles to a native binary, so its computation time sits with the compiled languages rather than the interpreted ones.
- `wordfreq` is here because Varg loses it: a map keyed by strings copies the key on every new entry, and the hash is SipHash. A suite holding only what a language wins is advertising.
- Build time is a full Rust compilation. It is a cost of the toolchain, not of the program, and is listed on its own line for that reason.
- Token counts are an estimate at four characters per token, not a tokeniser run.
- Python and Node are timed through their interpreter, which is how they are used. Varg and C# are timed as their built artifacts, so nothing here includes a compiler or an SDK launcher.
