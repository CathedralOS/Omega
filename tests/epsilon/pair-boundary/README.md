# Epsilon evaluator pair-arena boundary

This explicit slow gate executes the direct evaluator-level status-252 witness
for the canonical Epsilon evaluator edge: the refusal the
[evaluator entry profile](../../../bootstrap/4_epsilon/EVALUATOR_ENTRY.md)
classifies as the section-10 outer `Incomplete(cumulative immutable pair
nodes, 3422453760, 3422453761)`. The lower chain's exact/adjacent pair boundary is
pinned separately by [tests/gamma/heap-boundary](../../gamma/heap-boundary/README.md);
this gate witnesses that same counter through the Epsilon evaluator itself.

Run from the repository root on macOS arm64, or Windows x64 in Git Bash:

```sh
sh tests/epsilon/pair-boundary/run.sh
```

Both routes require `python3`, Git's Unix tools on Windows, and the
corresponding checked-in Alpha seed. macOS also requires `codesign`. The gate
materializes the bound packed evaluator closure, the bound canonical entry,
the bound Delta compiler and support section, and the bound Gamma evaluator;
reconstructs the canonical 729,060-byte receipt (`DCREQ`, profile 1,
`ConformanceBytesV1`); then frames each fixture in a version-1 EREQ envelope
(ExactConsoleV1 profile, exact source section, empty stdin, bound closure
digest). The host only frames bytes and compares status/stdout/stderr; it does
not model pair allocation.

The two fixtures differ only in the loop bound:

- [`writes_admitted.epsilon`](writes_admitted.epsilon) — 64 iterations of the
  sparse-write loop, then `Console.write_byte(65)`: canonical `Exit(0)`
  observation with stdout `A`. Each of the 256 writes inserts a fresh leaf
  into the `[i32; 2147483647]` receiver array and rebuilds one bounded
  canonical-midpoint path — `O(log E)` new cumulative pair nodes per write —
  so the admitted side exercises the same allocation shape the refused side
  drives to exhaustion.
- [`writes_exhaustion.epsilon`](writes_exhaustion.epsilon) — the same loop
  with no exit arm. Every iteration allocates; the arena is cumulative and
  cannot reclaim, so the run ends only when a write attempts the
  3,422,453,761st pair: status 252, empty stdout, empty stderr. No Epsilon
  observation is published — that is the witness. The refusal lands inside
  whichever phase reaches the boundary; this workload reaches it in
  execution.

Both fixtures stay far inside every other counter: no output is written on
the refused path (the 4,194,304-byte publication bound never binds), the
state loop is proper-tail (the 256-context bound never grows), nesting stays
shallow (no census or stack refusal), and the sequential-plus-stride index
schedule keeps every write below the declared extent (no `Bounds` trap).
Empty stdout on the refused case additionally witnesses that evaluator-owned
exhaustion publishes no prefix.

`fixtures.tsv` pins each fixture's bytes, digest, expected status, and exact
stdout. The gate rejects missing or unlisted fixtures. A per-case watchdog
defaults to 7,200 seconds and a `--case` selector supports isolated
measurement:

```sh
OMEGA_EPSILON_PAIR_SECONDS=10800 sh tests/epsilon/pair-boundary/run.sh \
    --case writes_exhaustion.epsilon
```

A timeout is a failing observation without a language judgment. A slower host
can raise the allowance; the bound itself does not move. Keep this gate
separate from the routine evaluator-entry gate and delete it only when
another selected-evaluator gate subsumes the evaluator-level pair-exhaustion
refusal.

On macOS arm64 at the recorded revision the receipt reconstruction took
221.744 seconds, the admitted case finished in 8.794 seconds, and the refused
case ran 4,250.673 seconds before returning status 252 with empty stdout and
empty stderr; these are host measurements, not profile bounds.
