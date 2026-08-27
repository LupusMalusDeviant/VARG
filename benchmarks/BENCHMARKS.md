# Varg Benchmark Results

> Varg vs Python vs C# vs TypeScript
> Machine: Windows 11 (AMD64), measured 2026-08-27
> Runs per benchmark: 30 (median taken)

## Summary

| Benchmark | Varg | Python | C# | TypeScript |
|-----------|------|--------|----|------------|
| Fibonacci(35) - Pure Compute | 16.0ms | 695.5ms | 53.0ms | 53.0ms |
| Data Pipeline - Collections | 1.0ms | 9.0ms | 11.0ms | 4.0ms |
| JSON Processing - Strings/Alloc | 1.0ms | 2.0ms | 21.0ms | <1ms |
| Word frequency, 200k distinct - Alloc/Hash | 15.0ms | 27.0ms | 64.0ms | 36.0ms |

---

## Fibonacci(35) - Pure Compute

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 268 B | 217 B | 291 B | 292 B |
| Lines of Code | 12 | 10 | 13 | 10 |
| LLM Tokens (est.) | ~64 | ~54 | ~69 | ~70 |
| Build Time | 3332ms | N/A (interpreted) | 2314ms | N/A (interpreted) |
| Wall time, median | 38ms | 734ms | 95ms | 118ms |
| Wall time, p95 | 41ms | 743ms | 98ms | 132ms |
| **Computation time** | **16.0ms** | **695.5ms** | **53.0ms** | **53.0ms** |
| vs Python | **43.5x** | **1.0x** | **13.1x** | **13.1x** |

## Data Pipeline - Collections

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 1090 B | 654 B | 992 B | 809 B |
| Lines of Code | 35 | 25 | 26 | 22 |
| LLM Tokens (est.) | ~272 | ~163 | ~248 | ~202 |
| Build Time | 574ms | N/A (interpreted) | 916ms | N/A (interpreted) |
| Wall time, median | 23ms | 45ms | 53ms | 69ms |
| Wall time, p95 | 26ms | 48ms | 58ms | 82ms |
| **Computation time** | **1.0ms** | **9.0ms** | **11.0ms** | **4.0ms** |
| vs Python | **9.0x** | **1.0x** | **0.8x** | **2.2x** |

## JSON Processing - Strings/Alloc

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 913 B | 747 B | 892 B | 618 B |
| Lines of Code | 27 | 22 | 25 | 18 |
| LLM Tokens (est.) | ~221 | ~186 | ~223 | ~154 |
| Build Time | 582ms | N/A (interpreted) | 864ms | N/A (interpreted) |
| Wall time, median | 23ms | 47ms | 62ms | 64ms |
| Wall time, p95 | 27ms | 51ms | 66ms | 79ms |
| **Computation time** | **1.0ms** | **2.0ms** | **21.0ms** | **<1ms** |
| vs Python | **2.0x** | **1.0x** | **0.1x** | **>2x** |

## Word frequency, 200k distinct - Alloc/Hash

| Metric | Varg | Python | C# | TypeScript |
|--------|------|--------|----|------------|
| Source Size | 1403 B | 323 B | 751 B | 425 B |
| Lines of Code | 34 | 12 | 21 | 11 |
| LLM Tokens (est.) | ~350 | ~80 | ~187 | ~106 |
| Build Time | 611ms | N/A (interpreted) | 859ms | N/A (interpreted) |
| Wall time, median | 47ms | 84ms | 125ms | 123ms |
| Wall time, p95 | 51ms | 90ms | 127ms | 138ms |
| **Computation time** | **15.0ms** | **27.0ms** | **64.0ms** | **36.0ms** |
| vs Python | **1.8x** | **1.0x** | **0.4x** | **0.8x** |

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
- Python and Node are timed through their interpreter, which is how they are used. Varg and C# are timed as their built artifacts, so nothing here includes a compiler or an SDK launcher.
