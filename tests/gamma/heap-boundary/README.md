# Selected Gamma heap boundary

This explicit slow gate runs ordinary authored Gamma programs against the
selected Beta-authored evaluator. It requires Python 3 and the selected Alpha
seed: macOS arm64 additionally requires `codesign`; Windows x64 uses Git Bash.
Windows execution has not yet been validated. Run from the repository root:

```sh
sh tests/gamma/heap-boundary/run.sh
```

The AlphaBootstrapV5 evaluator owns pair bytes
`[0x20400000, 0x2000000000)`. Each pair occupies 40 bytes, admitting exactly
3,422,453,760 pairs with no unusable tail. This is a private evaluator
capacity, not a new Gamma operation or a Delta compiler-resource limit.

At this arena size an exact-boundary run is not executable within a gate
budget on any host — the fixtures below are the executable edge: every one
allocates past the entire retired V4 arena (40,265,318 pairs) and completes,
which is only possible because the arena grew. Refusal precision at the new
count is discharged by the profile's preflight argument and the unchanged
countable bound, not by an executable adjacent case.

| Fixture | Pair allocations | Exact observation |
| --- | ---: | --- |
| `old_ceiling_adjacent.gamma` | 20,132,660 | status 0, byte `A` |
| `scalar_exact.gamma` | 40,265,318 | status 0, byte `A` |
| `scalar_adjacent.gamma` | 40,265,319 | status 0, bytes `BA` |
| `application_exact.gamma` | 40,265,317 loop pairs plus one result pair | status 0, byte `A` |
| `application_adjacent.gamma` | 40,265,318 loop pairs plus one result pair | status 0, byte `A` |

All loops are proper-tail calls, with one authored `pair` per iteration. The
scalar result allocates no pair; its final byte is appended by the evaluator.
The application result explicitly allocates `(pair 0 1)`. The scalar-adjacent
case buffers `B` before allocating past the former arena and finishes with
`A`, so `BA` also witnesses that evaluator-owned output is published only
after result validation. Every observation
requires empty stderr. The old-ceiling control exceeds the previous
20,132,659-pair maximum without changing its source when it runs on either
evaluator.

The host pins and reads all fixtures before execution, frames exact bytes, and
compares literal status/output expectations. It does not extract evaluator
functions, alter counters, or model pair allocation. `evaluator.tsv` pins the
selected Beta source and tape; `functional-evaluator.sh` separately reconstructs
that tape through the trusted Beta compiler.

Each child has a 1,200-second watchdog, not a language fuel limit. A timeout is
a failing observation without a language judgment. A slower host can increase
the observation allowance; an exact selector supports isolated measurement:

```sh
OMEGA_GAMMA_HEAP_SECONDS=2400 sh tests/gamma/heap-boundary/run.sh \
    --case application_exact.gamma
```

These same commands work in Git Bash on Windows. A missing or unknown selector
fails rather than passing an empty selection. Keep the full heap witnesses
separate from the routine 20/30-second evaluator gate. Delete this gate only
when another selected-evaluator gate subsumes full heap capacity, adjacent
refusal, and buffered-prefix suppression.
