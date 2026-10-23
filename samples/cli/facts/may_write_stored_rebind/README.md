# May-write stored rebind

The chosen mutable input is stored in a reference and loaded back, the write goes through that reloaded referent, and the disjoint index-bound fact on the other input stays.

The reference starts at a scratch local. `choose` returns one of the two mutable inputs, and that result is stored back into the reference. A later load of the reference is the write target. The other input starts at `1` and stays there. Indexing `[4, 7, 8, 9]` with that preserved bound reads `7`. Exit is `0` only when the chosen input is `9` and the index reads `7`. Each reject twin changes only `pick_chosen` from `true` to `false`, so the write overlaps the index bound.

The programs do not depend on `source/library/std`.

This witnesses the stored-rebind row of **R5** in `TASKS.md`: caller-visible may-write summaries under [state contracts](../../../../wiki/spec/language/state_contracts.md). The write frame records the referent after the rebind. It does not close finite write cycles or parameter-permutation write sets.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the other input is a local |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the other input is a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the other input arrives as a mutable parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call returns the preserved index bound |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the preserved index |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `LIMIT` names the index bound |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives the index |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | another machine receives the index |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body reloads the index |

## Execution

Every positive place binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, and declares `Expected exit: 0`.

## Reject twins

Each `invalid/` project is `intended-invalid`. The source differs only by passing `false` to `choose`. The overlapping index bound is then stale.

## Pinned fixtures

Positive places are byte-identical to `tests/omega/pass/facts/may_write_stored_rebind_through_<place>_exit`. Reject twins are byte-identical to `tests/omega/fail/facts/may_write_stored_rebind_through_<place>_overlapping`. Recorded rows: nine `checked` rows (with `check-target:macos_arm64`) and nine `rejected` rows carrying their own `cannot prove` fragment in `corpus_outcomes.txt`; nine `built ... exit:0` rows in `corpus_native_macos_arm64.txt`; nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `corpus_both_macos_arm64.txt`.

## Requirements and evidence

Parent outcome: **R5** (`TASKS.md`), the stored-rebind row.

1. **Establishment.** `other` is initialized to `1`, or a parameter requires `other < 4`, before the write. The index uses that bound. No later equality guard re-establishes it.
2. **Identity and lifetime.** The bound names the other mutable input. The reference is rebound from scratch to the chosen input, then loaded. The reject twin rebinds onto the other input and retires the bound.
3. **Composition.** `choose` returns one input. That result is stored in the reference and loaded before `overwrite`. The preserved bound is then used by a local, field, parameter, call result, match join, const bound, state arrival, forwarding machine, or target body.
4. **Soundness boundaries.** The summary is a may-write footprint. It does not grant a loan, a move, or a linear claim. The overlapping candidate cannot keep the stale index bound.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`).** The rebind, the write through the loaded referent, and the proved index lower into Terminal Psi and execute with the interpreter on `omega run --both`.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`).** Each positive binds `ProgramEntry` on all four hosted targets. The assignment installs the reference descriptor, not a store through the previous referent.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`).** Native execution stores `9` through the rebound referent and reloads the other input as the index.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission).** `omega run --both` agrees on exit `0` for every positive place.
