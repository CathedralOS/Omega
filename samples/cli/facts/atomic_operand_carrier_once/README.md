# Atomic operand carrier, checked once

Owner: `ARITHMETIC-POLICY-REALIZATION`, its `IMPLICIT-STORE-CONVERSION` clause
(one actionable diagnostic per authored conversion error). The item left
[TASKS.md](../../../../TASKS.md) when its acceptance passed.

Fact: An atomic value operand is checked once against the cell's `u32` carrier: a `u32` operand exchanges the resident, and an operand of another carrier reports one implicit conversion.

Every place runs the same `compare_exchange` on an `AtomicU32` holding 3. The
matching call (`expected` 3, new value 7) returns 3 and leaves 7; the
mismatched call (`expected` 3 again, new value 9) returns 7 and leaves 7. Both
run in every `main`, so a store on mismatch, a missing store, or a wrong prior
fails the exit-0 oracle. Only the route of the new value to the operand differs.
Each twin changes only that value's declared carrier from `u32` to `u64`.

| Place | Positive | Reject twin | Route of the new value |
| --- | --- | --- | --- |
| Local | [source](local/main.omg) | [twin](local/invalid/main.omg) | a `let` binding |
| Field | [source](field/main.omg) | [twin](field/invalid/main.omg) | a `[copy]` record field |
| Parameter | [source](parameter/main.omg) | [twin](parameter/invalid/main.omg) | a callee parameter |
| Call result | [source](call_result/main.omg) | [twin](call_result/invalid/main.omg) | a call's result |
| Match join | [source](match_join/main.omg) | [twin](match_join/invalid/main.omg) | the join of a `match` |
| Const name | [source](const_name/main.omg) | [twin](const_name/invalid/main.omg) | a named constant |
| Nested machine | [source](other_state/main.omg) | [twin](other_state/invalid/main.omg) | a nested machine's parameter |
| Other machine | [source](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) | two machines by terminal transfer |
| Target body | [source](target_body/main.omg) | [twin](target_body/invalid/main.omg) | an implementation selected by Build |

The pinned copies are `tests/omega/{pass,fail}/facts/atomic_operand_carrier_once_*`.
Each recorded row lists the exact diagnostic lines, so a duplicate returns as
a corpus move.

## Baseline

Measured with the optimized producer built at `0ff9f81b393`. The three checker
files involved are identical at the fixed base `ac2485d4d21`, and the recorded
row for `fail/atomics/atomic_operand_implicit_narrowing` already carried the
same duplicate at `ccdb99ca2214`.

| Case | Authored errors | Diagnostics before | After |
| --- | --- | --- | --- |
| Nine twins, in table order | 1 each | 2, 2, 2, 2, 3, **0**, 2, 2, 2 | 1 each |
| `store` operand | 1 | 2 | 1 |
| `fetch_add` operand | 1 | 2 | 1 |
| compare-exchange `expected` only | 1 | 1 | 1 |
| compare-exchange both operands | 2 | 3 | 2 |
| out-of-range literal | 1 | 2 | 1 |
| `7u64` literal | 1 | **0** | 1 |
| `atomic_operand_implicit_narrowing` | 2 | 3 | 2 |

The `const_name` twin and the `7u64` literal printed nothing and were accepted,
although an ordinary store of either is rejected as a suffixed literal landing
in a `u32` place. The `match_join` twin printed three lines for its single
operand.

## Cause and repair

Duplicates. `assignment_statements.rs` diagnosed every delivered atomic operand
in a loop and then diagnosed the established operand again through the ordinary
store path: the stored or swapped value, the fetch operand and the
compare-exchange replacement are all both delivered and established. Same text,
same location. The loop now skips the established operand, which the store path
owns exactly as it owns a plain assignment's value, independent of that value's
own arithmetic. The loop keeps the operands that path does not reach, such as
`expected`.

Missing check. `validate_suffix_landings` validated an atomic assignment's
value, which is the parser's generated carrier and never a literal, so a
suffixed literal, or a constant that expands to one, reached the cell
unchecked. It now validates each authored operand against the cell's resident
type.

Two operands that are both wrong still print two lines: they are two errors.
Suppressing identical text would hide a real second error.

## Controls

[Store](controls/store_operand/main.omg), [fetch](controls/fetch_operand/main.omg)
and [compare-exchange `expected`](controls/expected_operand/main.omg) each
report their one conversion once. [Both operands wrong](controls/both_operands/main.omg)
reports two, one per authored error. An [out-of-range literal](controls/literal_out_of_range/main.omg)
and a [`u64`-suffixed literal](controls/suffixed_literal/main.omg) report once
each.

## Evidence

Windows x86-64 only; producer rebuilt at `89c92b530e1` (`omega --build-profile`
reports `opt_level` 1).

- All nine positives: `omega run --both` native exit 0, Terminal Psi exit 0.
- All nine twins and six controls: the counts in the table above, each with the
  expected fragment.
- `corpus_gate --record --filter`: 25 outcomes (24 new rows and the narrowing
  fixture's row, now two lines). `--both --record`: 9 rows, each
  `exit:0 interpreter-exit:0 interpreter:agrees`.
- Regression filter over `fail/atomics`, `pass/atomics`,
  `signed_slice_index_upper_only` and `implicit_`: 78 fixtures, every fail
  fixture still rejects with its expected fragment. One diagnostics move is not
  from this change: `implicit_integer_policy_erasure_rejected` relabels one of
  its six lines from "terminal expression" to "return value".
- The item's acceptance filter under `--both`: 36 fixtures, 34 meet their
  oracle. `pass/proofs/numeric_state_contract_exit` does not, in either engine:
  five digit-bound requirements are unproved. The golden already records it as
  `regressed-from:checked`, so it predates this change.

## Requirements

- Composition: the same operand reaches the cell through nine routes and is
  diagnosed once in each; the route changes only where the carrier is declared.
- Soundness: the operand's own arithmetic obligations stay separate. An
  overflowing operand that also changes carrier still reports both.
- Psi 06-08 and Omega 00-09: the positives run through Terminal Psi, calling
  plans and native emission; a store on mismatch or a missing store changes the
  exit code.

Open: the other hosts were not run; `numeric_state_contract_exit` keeps the
item's acceptance unmet.
