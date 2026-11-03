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
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the other input is a local |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the other input is a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the other input arrives as a mutable parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call returns the preserved index bound |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the preserved index |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `LIMIT` names the index bound |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the index |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | another machine receives the index |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Build selects the checked implementation that reloads the index |

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

The original programs and oracles are frozen against published main
`04085b59f9c5227e69c5af03a29aa20e061e0e74`. Optimized baseline source
`a4bbda39bf53e9a65d15cc9612a7feb304423573` changes only the README; its compiler
and executable inputs equal that base. Run
[41549165807](https://github.com/CathedralOS/Omega/actions/runs/41549165807)
wrongly rejected all nine valid places because the nested write discarded the
untouched input's bound. The twins rejected, and no positive reached an engine.

The repair records a Boolean choice when it is saved, including the private
locals used to test transitions. A later store advances the current value;
a reference that already formed keeps its captured choice. The shared producer
of returned-reference origins supplies this information to write analysis.
Unknown choices, opaque regions and complete loan obligations remain conservative.
The target example has one checked `NestedProbe` implementation and four Build
selection rows. Its public contract retains both input bounds. The join example
runs both arms and observes an independent marker identifying the chosen path.

Run [41560734861](https://github.com/CathedralOS/Omega/actions/runs/41560734861)
validated optimized source `533e32a150157a3956bd4a4ea752df788308d10e`:
20 positive fixtures checked, all 31 invalid controls rejected, and all 20
native/Terminal comparisons observed exit `0` and agreement. The 23 public
projects produced 11 runtime passes and 12 intended rejections. This includes
all nine places, their twins, root `main`, and the declaration-then-store control.
The source checks use `macos_arm64`; actual runtime observations cover
`linux_x86_64`. Other hosted targets remain unverified at runtime.

The exact observed source is retained in a verified Git bundle published with
[part 1](https://github.com/CathedralOS/Omega/pull/1571#issuecomment-6593497212)
and [part 2](https://github.com/CathedralOS/Omega/pull/1571#issuecomment-6593497564)
of the PR evidence. That archive preserves the observed source and its earlier pass. Review found a
further missing-initializer lookup in the reference-choice walk. A guard now
retains an unknown choice for a declaration-only reference; new controls declare
and assign that reference before returning it, or reject its use before a store.
Run [41565055825](https://github.com/CathedralOS/Omega/actions/runs/41565055825)
confirmed that the valid declared-then-assigned reference still lost the untouched
bound. The original local case passed both engines, and all four invalid controls
rejected for their intended obligations. The next repair records the assigned
reference's choice before replacing the old binding; writes through a reference
keep its choice, while exposed or ambiguous replacements remain unknown.
Run [41576089167](https://github.com/CathedralOS/Omega/actions/runs/41576089167)
validated the original 20 positives in both engines and all 33 invalid controls,
but the declared-then-assigned reference still wrongly lost the bound. A focused trace confirmed that the earlier write summary discarded the reference
before saved-choice analysis ran. The next repair captures replacement pointer
contents through the same stored-reference transfer used for record fields, then
replaces the local slot after right-hand-side effects. It does not read the old
pointer or discard writes through a reference. Full acceptance and publication
remain unfinished.

Run [41584866204](https://github.com/CathedralOS/Omega/actions/runs/41584866204)
cleared the original bound-checking failure. The unchanged reference-local case
then reached lowering, which lacked its exact first receiving establishment.
The other 22 positives passed both engines and all 36 invalid controls rejected
correctly; public projects did not run. The next repair authenticates the first
installed reference from its complete checked loan activation. It retains all
requirements for retiring an already installed pointer. Acceptance is unfinished.

Run [41586945387](https://github.com/CathedralOS/Omega/actions/runs/41586945387)
cleared that first-installation gate. The frozen reference-local case then stopped
when returning the stored pointer: value production recognized initializers but
ignored the later store. The other 22 positives and all 36 invalid controls kept
their outcomes. The next repair removes that initializer-only representation
gate; source and flow checking still require an established value before use.

Run [41589081212](https://github.com/CathedralOS/Omega/actions/runs/41589081212)
cleared the return-value representation gate. Lowering next counted the empty
home declaration as a completed value needing disposal. The other 22 positives
passed both engines, and all 39 invalid controls rejected for their intended
reasons. The next repair distinguishes storage declaration from completed value
at the disposition owner. Actual stored values retain every cleanup obligation.

The dynamic-return adjunct reproduced its exact descriptor-reconstruction refusal
on the frozen baseline in
[41558699182](https://github.com/CathedralOS/Omega/actions/runs/41558699182).
That checking-leg failure predates this repair and remains open outside this row;
no baseline engine observation or adjunct success is claimed. The broader Build
migration and runtime checks on the other three hosted targets remain open.
See [PR 1571](https://github.com/CathedralOS/Omega/pull/1571) for detailed evidence.
