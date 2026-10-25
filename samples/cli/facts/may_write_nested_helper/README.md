# May-write nested helper

An outer helper that calls an inner helper to pick one of two mutable inputs and write through that result preserves a disjoint index-bound fact on the other input.

`choose` returns either mutable `u64`. `relay` returns that result. `overwrite` writes `9` through the nested result while evaluating an operand of `add`. The other input starts at `1` and stays there. Indexing `[4, 7, 8, 9]` with that preserved bound reads `7`. Exit is `0` only when the marker stays `1`, the chosen slot becomes `9`, and the index reads `7`. Each reject twin changes only the Boolean passed to `relay` from `true` to `false`.

This witnesses the nested-helper part of **R5** in `TASKS.md`: caller-visible may-write summaries under [invocation contracts](../../../../wiki/spec/language/state_contracts.md). It does not close nested-machine results, codec arguments, finite write cycles, parameter-permutation write sets, or parser convergence.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | the other input is a local of `main` |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the other input is a local |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the other input is a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the other input arrives as a mutable parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call returns the preserved index bound |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the preserved index |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `LIMIT` names the index bound |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the index |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | another machine receives the index |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body reloads the index |

## Execution

Every positive place, including root `main.omg`, binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, and declares `Expected exit: 0`. None depends on `source/library/std`.

```sh
omega run --both samples/cli/facts/may_write_nested_helper/local/main.omg
```

The same command covers `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, and `target_body`.

## Reject twins

Each `invalid/` project is `intended-invalid`. The source differs only by passing `false` to `relay`. The overlapping index bound is then stale. The pinned diagnostic fragment is `cannot prove`.

## Pinned fixtures

Positive places are byte-identical to `tests/omega/pass/facts/may_write_nested_helper_through_<place>_exit`. Reject twins are byte-identical to `tests/omega/fail/facts/may_write_nested_helper_through_<place>_overlapping`. Recorded rows: nine `checked` rows (with `check-target:macos_arm64`) and nine `rejected` rows carrying their own `cannot prove` fragment in `corpus_outcomes.txt`; nine `built ... exit:0` rows in `corpus_native_macos_arm64.txt`; nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `corpus_both_macos_arm64.txt`.

## Requirements and evidence

Parent outcome: **R5** (`TASKS.md`). This folder establishes nested-helper may-write summaries.

1. **Establishment.** `other` is initialized to `1`, or a parameter requires `other < 4`, before the write. The index uses that bound. No later equality guard re-establishes it.
2. **Identity and lifetime.** The bound names the other mutable input. Writing the chosen input invalidates only that input. The reject twin writes the other input and retires the bound.
3. **Composition.** `relay` calls `choose`, `overwrite` writes through that nested result, and `add` evaluates that write as an operand. The preserved bound is then used by a local, field, parameter, call result, match join, const bound, state arrival, forwarding machine, or target body.
4. **Soundness boundaries.** The summary is a may-write footprint. It does not grant a loan, a move, or a linear claim. The overlapping candidate cannot keep the stale index bound.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`).** The nested choice, operand write, and proved index lower into Terminal Psi and execute with the interpreter on `omega run --both`.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`).** Each positive binds `ProgramEntry` on all four hosted targets. The two `u64` inputs and the fixed byte array use ordinary value and reference lanes.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`).** Native execution stores `9` through the chosen referent and reloads the other input as the index.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission).** `omega run --both` agrees on exit `0` for every positive place.
