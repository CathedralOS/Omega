# Selected Gamma heap boundary

This explicit slow gate runs ordinary authored Gamma programs against the
selected Beta-authored evaluator. It requires Python 3 and the selected Alpha
seed: macOS arm64 additionally requires `codesign`; Windows x64 uses Git Bash.
Windows execution has not yet been validated. Run from the repository root:

```sh
sh tests/gamma/heap-boundary/run.sh
```

The AlphaBootstrapV3 evaluator owns pair bytes
`[0x10000000, 0x40000000)`. Each pair occupies 40 bytes, admitting exactly
20,132,659 pairs with eight unusable tail bytes. This is a private evaluator
capacity, not a new Gamma operation or a Delta compiler-resource limit.

| Fixture | Pair allocations | Exact observation |
| --- | ---: | --- |
| `old_ceiling_adjacent.gamma` | 5,033,165 | status 0, byte `A` |
| `scalar_exact.gamma` | 20,132,659 | status 0, byte `A` |
| `scalar_adjacent.gamma` | attempts 20,132,660 | status 3, empty stdout |
| `application_exact.gamma` | 20,132,658 loop pairs plus one result pair | status 0, byte `A` |
| `application_adjacent.gamma` | 20,132,659 loop pairs plus one refused result pair | status 252, empty stdout |

All loops are proper-tail calls, with one authored `pair` per iteration. The
scalar result allocates no pair; its final byte is appended by the evaluator.
The application result explicitly allocates `(pair 0 1)`. Both adjacent cases
write a buffered byte before exhausting the heap, so empty stdout also checks
that evaluator-owned failure does not publish a prefix. Every observation
requires empty stderr. The old-ceiling control exceeds the previous 5,033,164
pair maximum without changing its source when it runs on either evaluator.

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
