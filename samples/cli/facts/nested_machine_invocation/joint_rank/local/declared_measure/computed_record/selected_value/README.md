# Declaration-local selector coordinates

Owner: **NESTED-MACHINE-UNIFICATION** in [TASKS.md](../../../../../../../../../TASKS.md).
This is another production of the existing [joint private rank fact](../../../../README.md),
using an indexed read rather than a statically located record field.

> A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

`Progress::Remaining(value)` reads `value.values[value.selected]`. Its source
formal is distinct from both `count`'s `remaining` and `forward`'s `pending`.
An expression handle identifies the selected operation; it cannot supply the
actual selector's value. The demanded read now retains its supplying telescope
and rejoins the invocation's admitted coordinate by captured selector equality.
The chosen coordinate keeps its existing type, storage and entry-bound receipts
through checking and ordinary scalar lowering. No new runtime operation is used. The governing rules are [invocation contracts](../../../../../../../../../wiki/spec/language/state_contracts.md) and [ranking and subject preservation](../../../../../../../../../wiki/spec/language/termination.md).

Every program selects both slots. Each active invocation advances the caller's
counter seven times; each empty invocation leaves it unchanged. The counter
starts at 3, reaches 10, then reaches 17 after the second active invocation.
The caller checks the selected word is zero, neighbor 919 is untouched, selector
is unchanged, and canary 641 survives. The field place also checks canaries 373
and 587 around its record. Every twin changes only decrement 1 to decrement 0.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](main.omg) | [nonprogress twin](invalid/main.omg) |
| field | [source](../../../../field/declared_measure/computed_record/selected_value/main.omg) | [nonprogress twin](../../../../field/declared_measure/computed_record/selected_value/invalid/main.omg) |
| parameter | [source](../../../../parameter/declared_measure/computed_record/selected_value/main.omg) | [nonprogress twin](../../../../parameter/declared_measure/computed_record/selected_value/invalid/main.omg) |
| call_result | [source](../../../../call_result/declared_measure/computed_record/selected_value/main.omg) | [nonprogress twin](../../../../call_result/declared_measure/computed_record/selected_value/invalid/main.omg) |
| match_join | [source](../../../../match_join/declared_measure/computed_record/selected_value/main.omg) | [nonprogress twin](../../../../match_join/declared_measure/computed_record/selected_value/invalid/main.omg) |
| const_name | [source](../../../../const_name/declared_measure/computed_record/selected_value/main.omg) | [nonprogress twin](../../../../const_name/declared_measure/computed_record/selected_value/invalid/main.omg) |
| other_state | [source](../../../../other_state/declared_measure/computed_record/selected_value/main.omg) | [nonprogress twin](../../../../other_state/declared_measure/computed_record/selected_value/invalid/main.omg) |
| other_machine | [source](../../../../other_machine/declared_measure/computed_record/selected_value/main.omg) | [nonprogress twin](../../../../other_machine/declared_measure/computed_record/selected_value/invalid/main.omg) |
| target_body | [source](../../../../target_body/declared_measure/computed_record/selected_value/main.omg) | [nonprogress twin](../../../../target_body/declared_measure/computed_record/selected_value/invalid/main.omg) |

The target-body place uses ordinary named implementations selected by Build.
The x86 entry expects tag 17; the ARM entry expects 29. All four hosted target
bindings are authored. These observations are Linux x86-64 native/Terminal runs;
other-host physical execution is not established here.

## Composition and refusal controls

- [Public call](controls/public_call/main.omg) gives the declaration a checked
  helper's public result equation. Its index belongs to that helper's explicitly
  supplied input. The established owned-input route retains complete record
  identity, copies and the short shared call loan independently of scalar reads.
  The component returns the completed record to its caller; the same storage
  and counter oracle checks both active and empty paths. Its
  [twin](controls/public_call/invalid/main.omg) removes only the decrement.
- [Computed selector](controls/computed_selector/main.omg) selects
  `1 - value.selected`, with independently formed arithmetic and collection
  bounds. Its [twin](controls/computed_selector/invalid/main.omg) removes only
  the decrement. Neither spelling nor the physical slot supplies identity.
- [Missing bounds](controls/missing_bound/main.omg) removes only the entry's
  selector prerequisite. It rejects while forming the specification index;
  a desired rank cannot form its own partial operand.
- [Distinct call contexts](controls/distinct_call_contexts/main.omg) invokes one
  public indexed requirement at two different actual indices. Its proposed sum
  is unchanged by moving one unit between the cells. Distinct supplying contexts
  must not collapse into a single decreasing read. It remains a refusal after
  the repeated-call repair. The [constructed conserved-sum control](repeated_calls/controls/conserved_sum/invalid/main.omg)
  independently exercises the same cycle-descent boundary.

## Requirements and evidence

- Establishment: each entry independently proves `selected < 2` before the
  indexed requirement forms. Declaration binding supplies no arithmetic premise.
- Identity and lifetime: exact formal and field identities select captured
  scalar observations. A reference freezes its binding, not its mutable contents;
  ordinary write invalidation and call-cut capture remain in force.
- Composition: private arrivals reorder the record and counter. Selector fields
  in structural actuals are substituted simultaneously from their completed
  captured values. All edges participate in every-cycle strict descent.
- Soundness: storage/read authority and whole-value copy/loan admission remain
  separate. The nonprogress, missing-bound and distinct-context controls refuse.
- Psi 06–08: selected actual coordinates and their index-bound receipts feed the
  existing checked execution plans and canonical Terminal interpreter. No source
  target prefix, ambient parent fact or hidden runtime input is introduced.
- Omega 00–02: ordinary array/record geometry, borrowed arguments and the owned
  control's aggregate return use existing calling plans. The field canaries and
  returned record make layout mistakes observable.
- Omega 03–06: the caller reads its original counter and backing after completion.
  This is execution evidence for these cycles, not a new portable WCSU claim.
- Omega 07–09: all nine places and both positive composition controls execute
  with native/Terminal exit 0 on Linux x86-64. Corrupting the first expected count
  or swapping the Linux provider to the ARM implementation produces native signal
  4 and Terminal `Trap`, confirming the runtime falsifiers detect wrong behavior.

Fixed base `bc2c778d57d90b75587d484a6a882ad2cecac9a4` refuses all nine positives
with a source-complete-ranking-witness diagnostic. The original local probe is
unchanged. The repaired optimized CLI has SHA-256
`04a963b216a903f5fbff39c2040572a44e3ac220fdfa72cb050f95fce03ac74a` and opt-level 1.
The 23-case replay ledger retains the actual producer and source hashes; the
additional distinct-context refusal and runtime falsifiers have separate records
under the workbench's ignored `build/advance-evidence/`. The corpus pins the nine
places, their twins and controls as `facts/nested_machine_declared_selector_*`.

The final optimized corpus runner has SHA-256
`7af64a16697d558a123ac9efcb55f4e5a0bb2148bd2518ba3d27fba57e6f54c5`
and was built from source checkpoint `b3e852cfac94dcbfcd0c1031057eda046e87cc50`.
It records 11 positive native/Terminal exit-0 agreements and 13 intended
rejections. Three existing joint-rank positive controls also return exit 0 in
both engines, and their three rejection twins retain the expected diagnostics.
Only the 24 new fixture rows are added to `tests/omega/corpus_outcomes.txt`.

The [repeated-call production](repeated_calls/README.md) now retains selector
occurrences under distinct actual telescopes across the same nine places.
The parent retains whole-value snapshots through mutable borrowed inputs, other
declared body forms, broader custody/resource acceptance, product-compiler parity
and application migration. These productions do not close that parent outcome.
