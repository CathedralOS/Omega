# May-write nested helper

An outer helper that calls an inner helper to pick one of two mutable inputs and write through that result preserves a disjoint index-bound fact on the other input.

`choose` returns either mutable `u64`. `relay` returns that result. `overwrite` writes `9` through the nested result while evaluating an operand of `add`. The other input starts at `1` and stays there. Indexing `[4, 7, 8, 9]` with that preserved bound reads `7`. Exit is `0` only when the marker stays `1`, the chosen slot becomes `9`, and the index reads `7`. Each reject twin changes only the Boolean passed to `relay` from `true` to `false`.

This witnesses nested-helper mutation summaries under
[invocation contracts](../../../../wiki/spec/language/state_contracts.md).
`BUILD-OWNED-TARGET-SELECTION` in `TASKS.md` owns this example's migration
to an ordinary implementation selected by Build.
That migration must preserve both input bounds and the same write/index operation;
it does not close the wider mutation-summary requirements.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | the other input is a local of `main` |
| local | [`local/main.omg`](local/main.omg) | [`invalid_local/main.omg`](invalid_local/main.omg) | the other input is a local |
| field | [`field/main.omg`](field/main.omg) | [`invalid_field/main.omg`](invalid_field/main.omg) | the other input is a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`invalid_parameter/main.omg`](invalid_parameter/main.omg) | the other input arrives as a mutable parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`invalid_call_result/main.omg`](invalid_call_result/main.omg) | a call returns the preserved index bound |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`invalid_match_join/main.omg`](invalid_match_join/main.omg) | both match arms yield the preserved index |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`invalid_const_name/main.omg`](invalid_const_name/main.omg) | `LIMIT` names the index bound |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`invalid_other_state/main.omg`](invalid_other_state/main.omg) | a successor machine receives the index |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`invalid_other_machine/main.omg`](invalid_other_machine/main.omg) | another machine receives the index |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`invalid_target_body/main.omg`](invalid_target_body/main.omg) | Build selects the checked implementation that reloads the index |

## Execution

Every positive place, including root `main.omg`, binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, and declares `Expected exit: 0`. None depends on `source/library/std`.

```sh
omega run --both samples/cli/facts/may_write_nested_helper/local/main.omg
```

The same command covers `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, and `target_body`.

## Reject twins

Each `invalid/` project is `intended-invalid`. The source differs only by passing `false` to `relay`. The overlapping index bound is then stale. The pinned diagnostic fragment is `cannot prove`.

## Related controls

- [Saved choice](controls/captured_choice_exit/main.omg) and its
  [overlapping-write twin](controls/captured_choice_overlapping/main.omg) declare
  then assign the Boolean, save the returned reference, change the Boolean, and
  expose its new value through a separate output. The index is read before the
  final runtime checks.
- [Unknown choice](controls/unknown_choice/main.omg) retains both possible write
  destinations and rejects the stale bound.
- [Live loan](controls/unselected_live_loan/main.omg) rejects reusing the other
  input while the result's common lifetime still holds its exclusive loan.
- The existing [view read before a store](../../../../tests/omega/fail/borrows/view_local_read_before_store/main.omg)
  rejects observation of a borrowed view whose descriptor has not been established.
- [Declared reference](controls/reference_local_exit/main.omg), its
  [overlapping-write twin](controls/reference_local_overlapping/main.omg), and
  [use before the first store](controls/reference_local_before_store/main.omg)
  keep declaration separate from establishing a pointer. The positive installs
  the chosen reference before returning it. The twin changes only that choice;
  the unestablished control omits its store.

## Pinned fixtures

Positive places match `tests/omega/pass/facts/may_write_nested_helper_through_<place>_exit`.
Reject twins match `tests/omega/fail/facts/may_write_nested_helper_through_<place>_overlapping`.
Their `main.omg` and `build.omg` files are identical. Older macOS native records
are historical evidence; the fresh results below identify their own source and host.

## Requirements and evidence

Parent outcome: `BUILD-OWNED-TARGET-SELECTION` (`TASKS.md`). The owned
requirement is the target-body source migration with its full public input
contract, while retaining this nine-place mutation-summary fact.

1. **Establishment.** `other` is initialized to `1`, or a parameter requires `other < 4`, before the write. The index uses that bound. No later equality guard re-establishes it.
2. **Identity and lifetime.** The bound names the other mutable input. Writing the chosen input invalidates only that input. The reject twin writes the other input and retires the bound.
3. **Composition.** `relay` calls `choose`, `overwrite` writes through that nested result, and `add` evaluates that write as an operand. The preserved bound is then used by a local, field, parameter, call result, match join, const bound, state arrival, forwarding machine, or target body.
4. **Soundness boundaries.** The summary is a may-write footprint. It does not grant a loan, a move, or a linear claim. The overlapping candidate cannot keep the stale index bound.
5. **Checked-to-Terminal Psi lowering.** The nested choice, operand write, and proved index lower into Terminal Psi and execute with the interpreter on `omega run --both`.
6. **Provider settlement, layout geometry, and calling plans.** Each positive binds `ProgramEntry` on all four hosted targets. Four Build rows select the same ordinary `NestedProbe` implementation, preserving both public input bounds. Its returned byte carries no reference relation across the requirement. The two `u64` inputs and fixed byte array retain their ordinary value and reference lanes.
7. **Instruction selection, register homes, and activation frames.** Native execution stores `9` through the chosen referent and reloads the other input as the index.
8. **Form encoding, branches, relocations, and native images.** `omega run --both` agrees on exit `0` for every positive place.

## Validation

[Run 41602883614](https://github.com/CathedralOS/Omega/actions/runs/41602883614)
passed on optimized source `f3f387161017c5072f0359b017f0259405bb3327`:
24 positive fixtures checked, all 39 invalid controls rejected, and all 24
native/Terminal comparisons agreed. The nine-place row and its added reference
controls observed exit `0`; one existing shared-view control retained its authored
exit `70`. The 26 public projects produced 12 runtime passes and 14 intended
rejections, including root, every place and twin, and the saved-choice and
reference-declaration controls.

Source checks use macOS arm64; actual runtime observations use Linux x86_64.
The other hosted runtimes remain unverified. The exact observed source archive
is published in [part 1](https://github.com/CathedralOS/Omega/pull/1571#issuecomment-6599293559),
[part 2](https://github.com/CathedralOS/Omega/pull/1571#issuecomment-6599294218), and
[part 3](https://github.com/CathedralOS/Omega/pull/1571#issuecomment-6599294893).
The posted bytes were retrieved and verified against the retained bundle.
That archive preserves source; the run’s engine reports provide execution evidence.

The original programs and oracles are frozen against published main
`04085b59f9c5227e69c5af03a29aa20e061e0e74`. Baseline source
`a4bbda39bf53e9a65d15cc9612a7feb304423573` changes only this README.
[Run 41549165807](https://github.com/CathedralOS/Omega/actions/runs/41549165807)
wrongly rejects all nine valid places because the nested write loses the
untouched input’s bound. The twins reject; no positive reaches an engine.
The declared-then-assigned reference is separately frozen at
`d1eb93a2690b1dce07f9469255e78e49ededad5a`;
[run 41565055825](https://github.com/CathedralOS/Omega/actions/runs/41565055825)
records the same incorrect loss before any of its subsequent repairs.

The compiler now saves Boolean choices and the inputs selected by completed
references. Later changes to a selector do not rewrite a saved choice. A local
reference replacement captures the new contents before right-hand-side effects
and installs them afterward. Storage declaration reserves a correctly typed
home; only a checked store establishes its pointer value. Final return moves
precede exit retirement, preventing a second disposal of the old local home.
Unknown choices and hidden reference regions remain conservative. Borrow
access, lifetime and cleanup obligations remain independent checks.

Earlier [run 41560734861](https://github.com/CathedralOS/Omega/actions/runs/41560734861)
passed on `533e32a150157a3956bd4a4ea752df788308d10e`, before the additional
reference-local controls. It observed 20 native/Terminal exit-zero agreements.
Its historical source archive remains in [part 1](https://github.com/CathedralOS/Omega/pull/1571#issuecomment-6593497212)
and [part 2](https://github.com/CathedralOS/Omega/pull/1571#issuecomment-6593497564).
Those observations do not stand in for final acceptance of the added controls.

The separate dynamic-return adjunct reproduces a descriptor-reconstruction
refusal on the fixed baseline in
[run 41558699182](https://github.com/CathedralOS/Omega/actions/runs/41558699182).
That checking-leg comparison establishes baseline attribution only; it makes no
claim about adjunct execution or later main revisions. The wider target-source
migration remains open in `TASKS.md`.
