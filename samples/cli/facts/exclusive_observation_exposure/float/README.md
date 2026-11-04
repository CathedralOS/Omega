# Keep a float observation after a non-writing call

A saved observation that kept is 7 stays valid across a complete non-writing
exclusive call. Each place reads `item.kept`, calls `expose` through an exclusive
borrow, then consumes the saved equality. `agree` and the nested `check` entry
require both `observed == current` and `observed == 7.0`. Runtime checks happen after the
consuming call; they supply no proof premise for it.

This migration belongs to `BUILD-OWNED-TARGET-SELECTION` in
[TASKS.md](../../../../../TASKS.md). The broader saved-value rules remain under
`STATE-LOCAL-VALUE-FRONTIER`. The governing rules are
[mutation and subject identity](../../../../../wiki/spec/language/state_contracts.md#mutation-and-subject-identity),
[complete mutation frames](../../../../../wiki/spec/language/dependent_values.md#mutation-frames),
and [Build-owned target selection](../../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection).

`expose` reads its argument and makes no write. Its complete empty write summary
preserves the observation. The existing call-result, forwarding-machine and
selected-implementation examples borrow `other`; the other places borrow `kept`.
Those original choices remain explicit. Each twin makes an overlapping write of
`8.0` to `kept`. It must refuse the consuming equality, or the forwarding helper's
unchanged guarantee that its saved result still equals the live field.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [write](local/invalid/main.omg) |
| field | [program](field/main.omg) | [write](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [write](parameter/invalid/main.omg) |
| call result | [program](call_result/main.omg) | [write](call_result/invalid/main.omg) |
| match join | [program](match_join/main.omg) | [write](match_join/invalid/main.omg) |
| named constant | [program](const_name/main.omg) | [write](const_name/invalid/main.omg) |
| nested machine | [program](other_state/main.omg) | [write](other_state/invalid/main.omg) |
| another machine | [program](other_machine/main.omg) | [write](other_machine/invalid/main.omg) |
| selected implementation | [program](target_body/main.omg) | [write](target_body/invalid/main.omg) |

The [original root](main.omg) and [its twin](invalid/main.omg) remain as additional
witnesses. `local` supplies the standard discoverable place. Each sample's
Main/Build pair matches its pin under
`tests/omega/{pass,fail}/facts/exclusive_observation_exposure_through_float_*`.

The selected place calls the ordinary `FloatObservation::observe` requirement.
`SelectedFloatObservation` retains the original `item.kept == 7.0` entry contract;
four explicit Build rows select it. Its body proves the saved equality before
returning. The public requirement introduces no private result guarantee.

Both match inputs execute. Their arms carry route markers 17 and 23 beside the
saved float, so a wrong arm or wildcard choice changes the final result. The
program checks the live fields after consuming the saved fact. Identical-result
conditionals in the other helpers are removed without assuming that an
unconstrained `other` float is finite.

The requirements established by the programs are:

- Construction and entry contracts establish the exact value 7.0. The known
  complete empty frame preserves it; an unknown frame would retain its write
  ceiling.
- The saved float is a snapshot of the original field value. A write to the live
  field invalidates their equality while leaving the snapshot's value intact.
- Calls substitute the actual field, both match arms retain the same saved
  equality, and the nested machine proves its arrival contract before entry.
- Exclusive access remains an ordinary checked loan. A write summary grants no
  access authority, and no IEEE self-comparison is treated as universally true.
- Terminal Psi retains these calls and branches. Build selects the ordinary
  implementation while preserving the existing `Ends` field layout and pointer
  argument/f64 result calling plan.
- Matching-host native execution must preserve the live values, stack homes,
  branch choices and final exit-zero result, and agree with Terminal execution.
  Every positive and the root bind all four hosted targets.

The unchanged baseline at published source `9566f10e0d29` was measured with its
authenticated optimized Linux ARM64 producer in
[run 41784485616](https://github.com/CathedralOS/Omega/actions/runs/41784485616):
nine positives checked, nine twins reached their intended refusals, nine corpus
and nine ordinary native/Terminal pairs exited zero, and the root exited zero
in both engines. All 127 frozen original files and 302 distinct original/copied
input hashes were verified. The temporary wrapper omitted its final summary
assignment, so `summary.passed` remains false even though the workflow and all
five observed stages completed. The raw reports and that limitation remain in
`build/exclusive-float-selected-20261104/baseline-9566`.

This source migration, the added standard local path and the observable match
still need focused checking, matching-host execution and independent review.
The broader parent requirements remain open.
