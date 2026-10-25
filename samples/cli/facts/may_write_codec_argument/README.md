# May-write codec argument

Choosing one of two mutable inputs and writing through that result preserves a disjoint index-bound fact on the other input.

The helper `choose` returns either mutable `u64`. That result is the exclusive `written` argument of one synthesized `Blob::encode` call. The other input starts at `1` and stays there. Indexing `[4, 7, 8, 9]` with that preserved bound reads `7`. The runtime check also requires the chosen slot to become `5`, the cursor `encode` stores for this two-byte frame. Exit is `0` only when both hold. Each reject twin changes only `pick_chosen` from `true` to `false`, so the write overlaps the index bound and the stale bound must be rejected.

The programs are freestanding. They do not depend on `source/library/std`.

This witnesses the codec-argument row of **R5** in `TASKS.md`: caller-visible may-write summaries under [invocation contracts](../../../../wiki/spec/language/state_contracts.md). It does not close finite write cycles or parameter-permutation write sets.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
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

Every positive place binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, and declares `Expected exit: 0`.

```sh
omega run --both samples/cli/facts/may_write_codec_argument/local/main.omg
omega run --both samples/cli/facts/may_write_codec_argument/field/main.omg
omega run --both samples/cli/facts/may_write_codec_argument/parameter/main.omg
omega run --both samples/cli/facts/may_write_codec_argument/call_result/main.omg
omega run --both samples/cli/facts/may_write_codec_argument/match_join/main.omg
omega run --both samples/cli/facts/may_write_codec_argument/const_name/main.omg
omega run --both samples/cli/facts/may_write_codec_argument/other_state/main.omg
omega run --both samples/cli/facts/may_write_codec_argument/other_machine/main.omg
omega run --both samples/cli/facts/may_write_codec_argument/target_body/main.omg
```

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

Positive places are byte-identical to `tests/omega/pass/facts/may_write_codec_argument_through_<place>_exit`. Reject twins are byte-identical to `tests/omega/fail/facts/may_write_codec_argument_through_<place>_overlapping`. The nine positives record native execution and interpreter agreement (`built ... exit:0 interpreter-exit:0 interpreter:agrees` at `94247f983231`); the nine twins record their `cannot prove` fragment.

## Requirements and evidence

Parent outcome: **R5** (`TASKS.md`), the codec-argument row.

1. **Establishment.** `other` is initialized to `1`, or a parameter requires `other < 4`, before the write. The index uses that bound. No later equality guard re-establishes it.
2. **Identity and lifetime.** The bound names the other mutable input. The codec writes the chosen input through the exclusive cursor. The reject twin writes the other input and retires the bound.
3. **Composition.** `choose` returns one input, and that result is the exclusive argument of `Blob::encode`. The preserved bound is then used by a local, field, parameter, call result, match join, const bound, state arrival, forwarding machine, or target body.
4. **Soundness boundaries.** The summary is a may-write footprint. It does not grant a loan, a move, or a linear claim. The overlapping candidate cannot keep the stale index bound.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`).** The choice, the codec call, and the proved index lower into Terminal Psi and execute with the interpreter on `omega run --both`.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`).** Each positive binds `ProgramEntry` on all four hosted targets. The generated codec machine takes the exclusive reference result as `written: &mut u64`.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`).** Native execution stores the frame cursor through the chosen referent and reloads the other input as the index.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission).** `omega run --both` agrees on exit `0` for every positive place.
