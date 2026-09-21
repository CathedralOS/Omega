# PRIME-COUNTER-REMAINDER-LEGALIZATION — re-verification ledger

Board row: `TASKS.md` `**PRIME-COUNTER-REMAINDER-LEGALIZATION.**` (:12259) —
resolved mined stub re-mining the remainder-legalization half of the resolved
PRIME-COUNTER-I32-REMAINDER row (swept `50559da3ab9b5`). Re-verified on this
host (linux x86-64) at `90df29812c` by Zergling-126 (claim `b5012401`,
draft path only; the subject-path claim `3c0dc052` on
`samples/cli/arithmetic/prime_counter` was probed and released untouched).

## Landed state — confirmed

`3c1ead6df4` still holds: non-u64 exact divide/remainder selects the signed
i64 entries. `LegalizedExactIntegerOperator::Remainder` dispatches on the
`unsigned_divide` flag — `ExactRemainderI64` for signed, `ExactRemainderU64`
for unsigned — in both the construction and validation mirrors:

- `target-operations-to-selected-instructions/src/selection/construction/scalar_graph.rs:1063`
- `target-operations-to-selected-instructions/src/selection/validation/scalar_graph.rs:1141`

so `prime_counter`'s `i32` `n % d` runtime modulo
(`samples/cli/arithmetic/prime_counter/main.omg:61`, plus the `% 22` modular
tally at :100) legalizes to a native artifact. Prior host evidence stands:
`6d00135b89` ran the i32 `-17 % 5` canary to exit 70 and the sample suite
drives prime_counter to exit 8 on linux x86-64.

## Residual disposition — unchanged

The measured benchmark row (`benchmark.py measure` for prime_counter,
`--expected-exit 8`) belongs to BENCHMARK-PRIME-COUNTER-ROW and stays fenced:
`tools/benchmark`/`records/` and `wiki/drafts/benchmarks.md` under
BENCHMARK-ROW-RESUMPTION / BENCHMARK-HOST-ROW-MATRIX claims. No independent
slice remains under this name; the correct in-fence artifact is this ledger.
