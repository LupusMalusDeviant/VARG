# probes/ — the rejection half of the safety net

`golden/` proves valid programs still compile and still compute the right thing.
This proves the other direction: **invalid programs are rejected, and rejected by
Varg's own front end rather than by rustc** against generated Rust the author
never wrote.

## Why it exists

Every leak found so far had the same shape: the front end accepted something it
should not have, so the mistake surfaced as a Rust error about generated code.
Sweeping the whole accumulated set is also what caught two regressions
introduced by typechecker work, and an `abs()` that truncated floats — none of
which the unit suite reported.

## The contract

The check is `vargc check` (parse + typecheck, no codegen, ~40 ms per program).
That makes it exact: **if `check` accepts a program that is not valid, the
mistake reaches rustc.**

Each program declares what must happen, in its first lines:

```
// @probe reject: takes 2 argument(s)
```

The expected message fragment is not optional. Without it a probe would pass as
long as *something* failed, which silently accepts a rejection for entirely the
wrong reason.

`known-rustc-leak/` holds documented exceptions — invalid programs that rustc
catches rather than us:

```
// @probe rustc-leak: why this one is allowed through
```

Both halves are asserted: `check` must accept it and the build must fail. If the
front end ever learns to catch one, the probe fails with `NOW-CAUGHT` and asks
for it to be moved to `reject/`. The exception list cannot rot silently.

## Running it

```bash
VARGC=varg-compiler/target/release/vargc bash probes/run.sh
```

Exit code is 0 only if every probe holds. CI runs this before the golden suite,
because it is cheap and a front end that stopped catching something is worth
knowing before spending minutes on full builds.

## Adding one

Write the smallest program that exhibits the mistake, give it the `@probe`
directive naming the message, and check that the runner reports `PASS`. If you
are fixing a defect, add the probe *before* the fix and watch it fail first —
otherwise you have not proven it tests anything.
