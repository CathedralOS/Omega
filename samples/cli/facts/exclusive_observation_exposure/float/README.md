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

The example migration and float equality repair were published in
[PR #1799](https://github.com/CathedralOS/Omega/pull/1799). The four-host results
below executed source `967c651da77a`; they are retained observations, not runs
of a later merged source. [Integration run 41809079515](https://github.com/CathedralOS/Omega/actions/runs/41809079515)
then refused all ten positives because the bundled float operations had no
selected default provider. Its ten overlapping-write controls still reached
their intended refusals.

[PR #1811](https://github.com/CathedralOS/Omega/pull/1811) owns that configuration
repair. Its first [focused probe](https://github.com/CathedralOS/Omega/actions/runs/41830804519)
refused all seven cases at the old inline-member grammar, before any intended
control or program engine ran. The ordinary core Build route and grammar repair
are under fresh qualification. The separate [Build-use controls](build_use/README.md)
check that the root Build can use the completed defaults before configuring
its application. The original nine places and their runtime oracles remain
unchanged; the broader tasks stay open.

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

The programs check the following requirements:

- Construction and entry contracts establish the exact value 7.0. The known
  complete empty write summary preserves it. Unknown write summaries retain
  their separate acceptance.
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

The compiler used to reject the saved equality inside the selected
implementation. Checking that implementation retained two declarations of the
same toolchain float operation, and the comparison checker required exactly one
local declaration. It now accepts those copies only when their complete,
authenticated operation contracts agree. Float format, source ownership, operand
meaning and storage versions retain their existing checks. Ordinary authored
operator selection is unchanged.

The earlier four-host observations use optimized compiler source
`967c651da77a7584799c8c663b0a4c542c9d7eb6`:

| Host | Focused run | Status |
| --- | --- | --- |
| Linux x86-64 | [41799623238](https://github.com/CathedralOS/Omega/actions/runs/41799623238) | Complete and audited |
| Linux ARM64 | [41798660299](https://github.com/CathedralOS/Omega/actions/runs/41798660299) | Complete and audited |
| Windows x86-64 | [41799629779](https://github.com/CathedralOS/Omega/actions/runs/41799629779) | Complete and audited |
| macOS ARM64 | [41799626458](https://github.com/CathedralOS/Omega/actions/runs/41799626458) | Complete and audited |

Each host checked ten positives and ten intended refusals. The ten
corpus positives and ten ordinary projects each ran with native and Terminal
exit 0. Ten ordinary invalid projects reached their intended refusals, and the
root also ran with both exits at 0. The standard nine places and the retained
root account for those ten positives. Each audit verifies 140 frozen authored
inputs and 316 distinct authored/copied input hashes against the actual compiler
source; the root's ordinary CLI report contains 245 copied input hashes.

The [earlier run at source 01d36a](https://github.com/CathedralOS/Omega/actions/runs/41789435329)
retains the selected implementation's actual refusal. A separate unchanged-source
diagnostic reproduced it without the helper call, isolating the float-operation
lookup from mutation handling. Independent source review accepted the migration
at `01d36a` and the repair at `967c65`. The final records and documentation reuse those exact compiler and program
bytes. This documentation revision itself has no inferred execution.

Supplemental [checks on the repaired compiler](https://github.com/CathedralOS/Omega/actions/runs/41801509493)
accept the equality-premise example and retain four intended NaN/lookalike
refusals. The private-projection control stops at an earlier proof error both
before and after this repair; its intended privacy check remains unverified.
Its program, oracle, false summaries and existing golden rows are preserved.
Three migrated twins now name different callers or statement positions in their
actual diagnostics. Their earlier complete record blocks remain in the source
archive, while the current rows record only the newly observed messages. Every
unrelated row, comment and diagnostic remains unchanged.
The source archive on PR #1799 retains both measured revisions through squash
and branch cleanup. No execution at a later documentation or main commit is
inferred from these results.

The following baseline is historical and does not qualify the changed source.

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

The broader parent requirements remain open.
