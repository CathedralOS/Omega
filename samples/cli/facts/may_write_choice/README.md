# May-write choice

Choosing one of two mutable inputs and writing through that result preserves a disjoint index-bound fact on the other input.

The helper `choose` returns either mutable `u64`. `overwrite` writes `9` through that result while evaluating an operand of `add`. The other input starts at `1` and stays there. Indexing `[4, 7, 8, 9]` with that preserved bound reads `7`. The runtime check also requires the chosen slot to become `9` and the operand marker to stay `1`. Exit is `0` only when all three hold. Each reject twin changes only `pick_chosen` from `true` to `false`, so the write overlaps the index bound and the stale bound must be rejected.

This witnesses **R5** in `TASKS.md`: caller-visible may-write summaries under [invocation contracts](../../../../wiki/spec/language/state_contracts.md). It does not close nested helpers, nested-machine results, codec arguments, finite write cycles, or parameter-permutation write sets.

[`written_selector/main.omg`](written_selector/main.omg) exercises the same
disjoint-bound preservation after `choose` replaces its Boolean selector. The
entry argument is `true`, but the stored `false` makes `other` the written
referent; the unchanged `chosen` still indexes the four-element array. The
runtime oracle also requires the written `other` to contain `9`. This
depends on the selector's current value, not its entry snapshot. It introduces
no additional transport matrix or caller guard that re-establishes the bound.

The focused fixtures under `tests/omega/{pass,fail}/borrow/` distinguish an
unchanged mutable selector, a literal replacement, and an assignment computed
from the old value. Unknown replacements and exclusive exposure retain the
complete possible-origin union, and a selected relation supplies no builtin
equality. The `mutated_selector_entry_actual_{case,guard}` controls index the
overwritten referent and must reject its stale bound. These controls add no
borrow authority and do not close the remaining R5 acceptance.

The captured-result fixture also forwards the selector through a nested helper,
saves the returned reference, changes the selector again, and writes through the
saved reference. The reference keeps its production-time choice. Internal writes
and returned-origin selection consume the same invocation values; different
callers cannot share a specialized write frame, and an earlier write cannot be
removed merely because the helper returns a different reference. A bare mutable
recast exposes the selector just as an explicit exclusive borrow does. These
are focused controls, not another place matrix.

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

Every positive place, including root `main.omg`, binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, and declares `Expected exit: 0`.

```sh
omega run --both samples/cli/facts/may_write_choice/main.omg
omega run --both samples/cli/facts/may_write_choice/local/main.omg
omega run --both samples/cli/facts/may_write_choice/field/main.omg
omega run --both samples/cli/facts/may_write_choice/parameter/main.omg
omega run --both samples/cli/facts/may_write_choice/call_result/main.omg
omega run --both samples/cli/facts/may_write_choice/match_join/main.omg
omega run --both samples/cli/facts/may_write_choice/const_name/main.omg
omega run --both samples/cli/facts/may_write_choice/other_state/main.omg
omega run --both samples/cli/facts/may_write_choice/other_machine/main.omg
omega run --both samples/cli/facts/may_write_choice/target_body/main.omg
```

The byte `7` is `items[1]`. A preserved other-input of `1` reads that byte. A write that fails to store `9` into the chosen input takes the `bad` arm and crashes `Trap`.

## Reject twins

Each `invalid/` project is `intended-invalid`. The source differs only by passing `false` to `choose`. The overlapping index bound is then stale.

| Place | Twin | Pinned diagnostic fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `cannot prove` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `cannot prove` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `cannot prove` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `cannot prove` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `cannot prove` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `cannot prove` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove` |

## Pinned fixtures

Positive places are byte-identical to `tests/omega/pass/facts/may_write_choice_through_<place>_exit`. Reject twins are byte-identical to `tests/omega/fail/facts/may_write_choice_through_<place>_overlapping`. The nine positives record native execution and interpreter agreement (`built ... exit:0 interpreter-exit:0 interpreter:agrees` at `94247f983231`); the nine twins record their `cannot prove` fragment.

## Requirements and evidence

Parent outcome: **R5** (`TASKS.md`).

1. **Establishment.** `other` is initialized to `1`, or a parameter requires `other < 4`, before the write. The index uses that bound. No later equality guard re-establishes it.
2. **Identity and lifetime.** The bound names the other mutable input. Writing the chosen input invalidates only that input. The reject twin writes the other input and retires the bound.
3. **Composition.** `choose` returns one input, `overwrite` writes through that result, and `add` evaluates that write as an operand. The preserved bound is then used by a local, field, parameter, call result, match join, const bound, state arrival, forwarding machine, or target body.
4. **Soundness boundaries.** The summary is a may-write footprint. It does not grant a loan, a move, or a linear claim. The overlapping candidate cannot keep the stale index bound.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`).** The choice, operand write, and proved index lower into Terminal Psi and execute with the interpreter on `omega run --both`.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`).** Each positive binds `ProgramEntry` on all four hosted targets. The two `u64` inputs and the fixed byte array use ordinary value and reference lanes.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`).** Native execution stores `9` through the chosen referent and reloads the other input as the index.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission).** `omega run --both` agrees on exit `0` for every positive place.
