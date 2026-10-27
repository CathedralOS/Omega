# Checked predicate disjoint loans

A checked distinct-index predicate authorizes two mutable element loans that write 17 and 53 to the original distinct caller elements.

Owner: **BORROW-PROOF-CONVERGENCE** in [TASKS.md](../../../../TASKS.md).
This row establishes relational consequences of an ordinary pure, total checked
predicate at loan formation. Its body states `first_index != second_index`;
its name and its Boolean result type establish no separation. The complete
integer carriers seed symbolic body interpretation. Only relations proved on
every true return reach borrow compatibility, substituted onto the exact actuals.

The consuming `replace` operation retains its two independent element loans,
then writes 17 and 53 to the original caller array. Every place transports the
same selected first index into that invocation and consumes its relation through
a local alias, record projection, parameter, completed result, match join,
constant, nested arrival, separate machine or target-scoped body. The runtime comparison observes
both updates; it does not establish the static separation required by the loans.
Every reject twin changes only the predicate's own body from `!=` to `<=`.
That weaker fact permits equal indices and must reject the same loans.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg), [exclusive call](local/exclusive_call/main.omg), [record predicate](local/record_predicate/main.omg) | [insufficient predicate](local/invalid/main.omg), [call twin](local/exclusive_call/invalid/main.omg), [record twin](local/record_predicate/invalid/main.omg) |
| field | [program](field/main.omg), [exclusive call](field/exclusive_call/main.omg), [record predicate](field/record_predicate/main.omg) | [insufficient predicate](field/invalid/main.omg), [call twin](field/exclusive_call/invalid/main.omg), [record twin](field/record_predicate/invalid/main.omg) |
| parameter | [program](parameter/main.omg), [exclusive call](parameter/exclusive_call/main.omg), [record predicate](parameter/record_predicate/main.omg) | [insufficient predicate](parameter/invalid/main.omg), [call twin](parameter/exclusive_call/invalid/main.omg), [record twin](parameter/record_predicate/invalid/main.omg) |
| call_result | [program](call_result/main.omg), [exclusive call](call_result/exclusive_call/main.omg), [record predicate](call_result/record_predicate/main.omg) | [insufficient predicate](call_result/invalid/main.omg), [call twin](call_result/exclusive_call/invalid/main.omg), [record twin](call_result/record_predicate/invalid/main.omg) |
| match_join | [program](match_join/main.omg), [exclusive call](match_join/exclusive_call/main.omg), [record predicate](match_join/record_predicate/main.omg) | [insufficient predicate](match_join/invalid/main.omg), [call twin](match_join/exclusive_call/invalid/main.omg), [record twin](match_join/record_predicate/invalid/main.omg) |
| const_name | [program](const_name/main.omg), [exclusive call](const_name/exclusive_call/main.omg), [record predicate](const_name/record_predicate/main.omg) | [insufficient predicate](const_name/invalid/main.omg), [call twin](const_name/exclusive_call/invalid/main.omg), [record twin](const_name/record_predicate/invalid/main.omg) |
| other_state | [program](other_state/main.omg), [exclusive call](other_state/exclusive_call/main.omg), [record predicate](other_state/record_predicate/main.omg) | [insufficient predicate](other_state/invalid/main.omg), [call twin](other_state/exclusive_call/invalid/main.omg), [record twin](other_state/record_predicate/invalid/main.omg) |
| other_machine | [program](other_machine/main.omg), [exclusive call](other_machine/exclusive_call/main.omg), [record predicate](other_machine/record_predicate/main.omg) | [insufficient predicate](other_machine/invalid/main.omg), [call twin](other_machine/exclusive_call/invalid/main.omg), [record twin](other_machine/record_predicate/invalid/main.omg) |
| target_body | [program](target_body/main.omg), [exclusive call](target_body/exclusive_call/main.omg), [record predicate](target_body/record_predicate/main.omg) | [insufficient predicate](target_body/invalid/main.omg), [call twin](target_body/exclusive_call/invalid/main.omg), [record twin](target_body/record_predicate/invalid/main.omg) |

Each place also contains an `exclusive_call/` production with the same
predicate, indices, writes and oracle. Instead of retaining local element
loans, it forms both exclusive arguments for `write_pair` in one call. Its
reject twin weakens only the same predicate body; a callee argument roster
must consume the same established separation as local loan formation.
These pairs are pinned as `_through_<place>_exclusive_call_{exit,rejected}`.

The `unary_predicate/` production in each place fixes the first index to zero
and carries `nonzero(second_index) == true`. The same existing checked interval
explorer used by guard narrowing supplies its true-return lower bound; the
shared relation owner publishes that bound with binary predicate relations.
Its twin changes only this required predicate to `permits_zero`, whose body
admits zero and therefore cannot justify the same two loans. These pairs pin
`_through_<place>_unary_predicate_{exit,rejected}`. Necessary interval hulls
never replace an authored public contract: exact envelope conversion retains
its independent converse check.

The `record_predicate/` production passes a shared `Indices` record to the
ordinary checked predicate. Its two scalar fields retain their exact record
identity and complete typed projection paths; another record with the same field
names supplies no relation. All nine places carry that record into the same two
loan writes, and each twin weakens only `!=` to `<=`. Predicate consequences do
not authorize stored-reference traversal or mutable observation replay. Caller
establishment uses a complete equivalent scalar formula and the existing
source-ordered captured operands; a necessary relation alone cannot establish
the predicate. These pairs are pinned as
`_through_<place>_record_predicate_{exit,rejected}`. The
[record exclusive-call control](controls/record_exclusive_call/main.omg) and
[weakened twin](controls/record_exclusive_call/invalid/main.omg) also consume
that relation in two simultaneously formed `write_pair` arguments. Execution qualification of
this production is tracked separately from the earlier scalar observations.

The pairs are pinned under `tests/omega/{pass,fail}/facts/` as
`checked_predicate_disjoint_loans_through_<place>_{exit,rejected}`.
All positives bind all four hosted `ProgramEntry` targets and declare exit 0.

## Requirements and evidence

- **Establishment:** the ordinary checked `distinct` body, evaluated under full
  carrier bounds, proves disequality on every true return. The callee's requires
  predicate is established at each call; the body does not assume its own goal.
- **Identity and lifetime:** each consequence names the exact caller actuals at
  the available formation scope. Loan occurrence, backing, access, child closure
  and lifetime remain owned by existing resource checking. A relation cannot
  create a permission or retarget a captured selector.
- **Composition:** local, field, parameter, completed result, both match
  alternatives, constant, nested machine, another machine and target-scoped body
  preserve the selected index and the same consuming operation. The
  [composed predicate](controls/composed_predicate/main.omg) reverses actuals
  through another checked helper and uses a disjunction of strict comparisons.
- **Soundness:** [equal actuals](controls/equal_actuals/invalid/main.omg),
  [an unrelated always-true predicate](controls/unrelated_predicate/invalid/main.omg)
  and [an overlapping whole window](controls/wide_overlap/invalid/main.omg)
  reject. [Circular formation](controls/circular_formation/invalid/main.omg)
  cannot use a predicate result to establish that predicate call's own partial
  operands. [A changed selector](controls/stale_selector/invalid/main.omg)
  invalidates the entry relation before loan formation. Singleton disequality
  does not establish separation of wider windows. An
  [ordered result](controls/ordered_result/invalid/main.omg) can equal the other
  index even when its source differs: disequality transports only through an
  exact completed-result equation, never through another inequality. The
  [reversed equation](controls/reversed_equality/main.omg) remains positive;
  [a shifted middle](controls/nonzero_middle_shift/invalid/main.omg),
  [a partial true branch](controls/partial_true_branch/invalid/main.omg),
  [mixed signed carriers](controls/mixed_signed_carriers/invalid/main.omg),
  and [an overloaded comparison](controls/overloaded_comparison/invalid/main.omg)
  grant no missing separation. A [predicate requiring exclusive arguments](controls/borrow_formation/invalid/main.omg)
  cannot use its desired truth to form those borrow arguments or bypass
  observation-free specification-call admission. The
  [signed nonzero control](controls/signed_nonzero/main.omg) keeps both -1 and
  +1 valid; [nonzero does not mean positive](controls/signed_nonzero_not_positive/invalid/main.omg).
  A [disconnected implementation requirement](controls/disconnected_public_envelope/invalid/main.omg)
  cannot stand for its convex hull in a public contract. A
  [stronger pair predicate](controls/relational_public_envelope/invalid/main.omg)
  cannot be replaced by its necessary disequality without proving the converse. A
  [recursive changed-goal control](controls/recursive_changed_goal/invalid/main.omg)
  returns true whenever its second index is nonzero, independently of the first.
  A decreasing first index cannot justify reusing a lemma about the original
  pair unless its instantiated conclusion is still the requested conclusion.
  The record production retains [two exact roots](controls/record_two_roots/main.omg)
  and [nested owned paths](controls/record_nested_path/main.omg); an
  [unrelated root](controls/record_unrelated_root/invalid/main.omg) or
  [sibling path](controls/record_unrelated_path/invalid/main.omg) supplies no
  separation. [A saved field snapshot](controls/record_snapshot/main.omg)
  retains its meaning after the original changes, while a
  [stale original](controls/record_stale_field/invalid/main.omg) and a
  [stronger predicate](controls/record_stronger/invalid/main.omg) cannot
  establish the caller requirement. The
  [operand-time snapshot](controls/record_operand_snapshot/main.omg) and its
  [equal-input twin](controls/record_operand_snapshot/invalid/main.omg) bind
  the constructed record before a later argument mutates its source.
  Unsupported/effectful bodies and exhausted proof work supply no evidence.
- **Psi 05–07:** admitted loans and stores use the existing Terminal Psi route;
  pure proof reasoning introduces no runtime authority or proof replay. Public
  `run --both` observes interpreted execution as well as native execution.
- **Omega 00–02:** ordinary array geometry and call plans retain original caller
  backing and select the two exact cells. This row qualifies no additional ABI lane.
- **Omega 03–06:** native addressing, stores and activation homes must preserve
  the two writes and their order through the existing realization path.
- **Omega 07–09 and emission:** native exit 0 must agree with Terminal exit 0 on
  the available macOS ARM64 host. Four bindings establish target eligibility;
  matching-host Windows and Linux execution remain separate qualification.

Canonical authority: [loan compatibility and captured premises](../../../../wiki/spec/terminal-psi/loans.md),
[invocation contracts](../../../../wiki/spec/language/state_contracts.md), and
[checked predicate consequences](../../../../wiki/spec/language/dependent_values.md#collection-invariants),
[structural predicate paths](../../../../wiki/spec/terminal-psi/structural_predicates.md), and
[fact identity and lifetime](../../../../wiki/spec/terminal-psi/fact_validity.md).

## Recorded baseline and acceptance

Fresh optimized producer `16b381462327a3f7d1eb8c7218bdb81114a8a10a` rejects all nine
local-loan places with overlapping-loan diagnostics and all nine exclusive-call
places with `as mutable more than once`. A source-identical scratch
control changing only the predicate requirement to direct `first_index != second_index`
checks and runs with native/Terminal exit 0 on macOS ARM64. Frozen source,
producer hashes and raw observations are retained in the lane qualification and
claim records; the compiler implementation changes after this observation.

Run the public acceptance with `omega run --both <place>/main.omg` after ordinary
package preparation, then `python3 tools/corpus_gate.py --both --filter checked_predicate_disjoint_loans`.
The parent retains remaining domain, runtime-projection and same-statement/theorem
premise acceptance beyond the relations established by this row.

The repaired optimized producer `13375dc5d9c41501f4be4584f52d9f8847823940`
observed all nine places on macOS ARM64 with native exit 0 and Terminal exit 0,
including both exclusive-call and unary-predicate productions of this same fact.
The root and composed, reversed-actual and signed controls also agree; all 43
fact-sensitive invalid programs reject with their authored diagnostics. Source,
producer digests and separate engine/rejection streams remain in the fixed lane
qualification record. These observations do not claim execution on another host
or closure of the parent capability's remaining premise kinds.

Fresh optimized producer `16fbd2f862483ecc41ba2e7e44e6cb1a5cccbfd5` also
passes the retained corpus route: all 30 positive fixtures execute with native
and Terminal exit 0, and all 43 reject fixtures retain their authored reasons.
The retained target audit distinguishes demonstrated duplicate captured storage
from unknown dynamic geometry; exact source-to-target operands, access, live
parent lineage, moves and descriptor dependencies remain independently checked.
Seven focused custody tests pass, and the five affected historical slice/helper
fixtures lose no behavior against the fixed baseline. The ranked recursive
predicate/helper-theorem core separately checks on baseline and repaired source;
that control is checking evidence, not a runtime claim.

An actual merge conflict with main was resolved at
`91838745b51bb7cae56a606372c70fa87b4d715d`, retaining the shared consequence
reader and main's structural interval fallback. Its fresh optimized producer
repeats all 74 public observations and the 73 checking/30 dual-engine corpus
observations successfully; the seven custody tests and five fixed-baseline
comparisons also pass. Two focused consumers preserve main's domain-match field
interval and reborrowed stored-view parameter behavior, including their reject
controls. Earlier producer observations retain their original identities.

The shared-record production was frozen against fresh optimized current-main
`7810a10b28a58ad23f342fd1bb8bc3560eb9ed4e`: the predicate program refused
both the loans and caller establishment, while the direct field relation ran
with native and Terminal exit 0. Producer
`39b908e3a33f98afcdd7169e0b1bcfcc8ec7b2e0` establishes the owned integer-field
observation mechanism through all nine places and their weakened-predicate
twins. Complete scoped retained checking observes 45 accepted programs and 59
intended rejects; all 45 executable fixtures agree at native/Terminal exit 0.
The public program route also supplies 46 dual-engine successes and 59 intended
rejects, including the root program and exact root/path, saved snapshot,
source-ordered operand, sibling-write and exclusive-call controls. The final
operand oracle also observes the later argument's resulting field value.

These are source-specific records: the final operand-oracle pair replaces its
earlier observations with focused fresh public and retained runs. Other inputs
and compiler components retain their verified hashes. Five affected historical
fixtures lose no behavior against the recorded baseline; the ranked recursive
helper-theorem core separately checks. Checking and native goldens record only
the 31 new fixtures and their 15 executable positives. Source bundles, producer
profiles, per-program input/oracle hashes and raw engine/rejection streams are
retained in qualification evidence. Execution is measured on macOS ARM64;
other hosted targets have bindings and source checking, not claimed execution.

This closes common owned integer-field observations and complete owned field
paths for this fact. It does not close the parent structured-callee/domain,
runtime-projection or same-statement/theorem acceptance. Stored-reference
crossings and other predicate control forms retain their own admission and
validity obligations. Immutable formals have write restrictions distinct from
owned local contents; caller facts use captured value/storage epochs rather
than replaying an immutable local's aggregate initializer.

An actual PR291 conflict with main `e4d9c74577f9ed221697b73113b045220364f9de`
was resolved at `a0a9721ff70b7998760011d1e6feaf0c59ac62ed`, retaining the
predicate-subject export and all incoming exports. All ten implementation blobs
of this repair remain identical to the previously reviewed source. A fresh
optimized integrated producer repeats all 105 public observations, all 104
checking outcomes and all 45 native/Terminal agreements successfully, with no
golden moves. The five fixed-baseline comparisons and ranked theorem-core
checking also pass. Earlier producer observations retain their historical
identities; this integrated validation covers the incoming compiler, library,
readiness and native-pipeline changes.

The fixed-array production `*/indexed_predicate/` keeps the same fact and
operation while observing `positions[0]` and `positions[1]` inside the borrowed
record. Fresh optimized main `20c47069543ba51860a4c9082664e1d8cac70195`
refuses both this predicate and its direct indexed relation. Optimized producer
`229a6a754e52807de50c94b355ed93abb62b808a` runs all nine places with native
and Terminal exit 0; changing only `!=` to `<=` retains the overlap refusals.
The producer obtains elements through the existing collection observation
identity, proves each finite selector against the actual length, and refuses
ambiguous observation paths. The consumer checks every fixed-index prefix
against that prefix's declared extent; a typed element path alone supplies no
bounds evidence.

The `controls/indexed_*` programs distinguish two array fields with identical
index coordinates, saved scalar-element snapshots, stale original contents,
and operands captured before a later argument writes the original array.
Their reject controls retain overlap, caller-predicate or exact index-bound
formation diagnostics. This establishes fixed integer-element arrays inside
owned records; it does not establish arbitrary runtime-length enumeration,
stored-reference traversal, arbitrary nested collection element shapes, or the
parent's remaining structured-callee/domain and theorem acceptance. Engine
observations remain specific to macOS ARM64.

Fresh final optimized producer
`15cba48fc19157be657b41db21ac47ad3995ddc2` repeats the complete fact's
130 public observations: 58 native/Terminal exit-zero agreements and 72
fact-sensitive rejects. The retained corpus checks 57 positives and 72 rejects;
all 57 executable fixtures agree at native/Terminal exit 0. The five affected
historical consumers lose no behavior, and the ranked recursive theorem core
still checks. Scoped golden recording adds only the 25 new checking fixtures
and 12 new native fixtures. These fresh observations supersede no historical
producer identities; final golden/README changes preserve the producer's
compiler, library and per-program input/oracle bytes.

## Runtime-selected elements

The `*/runtime_index_predicate/` production keeps the same two-element array,
loans and writes. The record also carries the two runtime selectors; `distinct`
checks both selector bounds before comparing the selected array elements.
Runtime reads retain the exact collection and evaluated selectors. They do not
become static projection keys or reference permissions.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/runtime_index_predicate/main.omg) | [weakened predicate](local/runtime_index_predicate/invalid/main.omg) |
| field | [program](field/runtime_index_predicate/main.omg) | [weakened predicate](field/runtime_index_predicate/invalid/main.omg) |
| parameter | [program](parameter/runtime_index_predicate/main.omg) | [weakened predicate](parameter/runtime_index_predicate/invalid/main.omg) |
| call_result | [program](call_result/runtime_index_predicate/main.omg) | [weakened predicate](call_result/runtime_index_predicate/invalid/main.omg) |
| match_join | [program](match_join/runtime_index_predicate/main.omg) | [weakened predicate](match_join/runtime_index_predicate/invalid/main.omg) |
| const_name | [program](const_name/runtime_index_predicate/main.omg) | [weakened predicate](const_name/runtime_index_predicate/invalid/main.omg) |
| other_state | [program](other_state/runtime_index_predicate/main.omg) | [weakened predicate](other_state/runtime_index_predicate/invalid/main.omg) |
| other_machine | [program](other_machine/runtime_index_predicate/main.omg) | [weakened predicate](other_machine/runtime_index_predicate/invalid/main.omg) |
| target_body | [program](target_body/runtime_index_predicate/main.omg) | [weakened predicate](target_body/runtime_index_predicate/invalid/main.omg) |

Establishment interprets the ordinary checked predicate under complete declared
carriers. Its bounds cannot be justified by its own desired truth: the
[unguarded read control](controls/runtime_index_formation/invalid/main.omg)
rejects both partial reads. A [false bounds guard](controls/runtime_index_bounds_guard/invalid/main.omg)
and an [additional false conjunct](controls/runtime_index_stronger_guard/invalid/main.omg)
retain the caller's full predicate obligation. Necessary disequality alone is
not an equivalent public requirement.

Identity and lifetime use immutable owned/shared entry formals or the existing
completed-operand observations. The source call independently checks its full
reference types and access; matching bare storage carriers grants no permission.
[Captured operands](controls/runtime_index_operand_snapshot/main.omg) preserve
both selected values before a later argument writes the original selector;
the [captured-equal twin](controls/runtime_index_operand_snapshot/invalid/main.omg)
still rejects even when that later write makes the original unequal.
[Array paths](controls/runtime_index_array_path/main.omg) and
[record roots](controls/runtime_index_record_root/main.omg) retain their complete
identities; their [wrong-path](controls/runtime_index_array_path/invalid/main.omg)
and [wrong-root](controls/runtime_index_record_root/invalid/main.omg) twins reject.
An [exclusive selector write](controls/runtime_index_mutable_selector/invalid/main.omg)
cannot reuse an immutable entry relation.

Composition retains the whole guarded condition on the target-body declaration.
Every comparison is independently necessary; the complete conjunction must also
prove the converse from fresh carriers. Each partial read separately forms from
earlier guards. The existing typed read planner substitutes the complete actual
telescope, collection and selectors while preserving the indexing occurrence.
Terminal production independently validates the resulting coordinate and bounds.
The guarded condition, ordinary calls, provider settlement, existing layout/call
plans and native emission all participate in the nine actual dual-engine runs.
Mathematical observation identity does not supply executable storage authority.

Fresh optimized published baseline `49d4d24247d738100c4988b4aa81b301744a969f`
refuses the original runtime-index predicate and direct relation. Complete
producer `b9c54be6f5763f7bf3c396a0ad951c23ba6e4200` supplies 158 fresh public
observations: 70 native/Terminal exit-zero agreements and 88 intended rejects.
Its retained corpus checks 69 positives and 88 rejects, and all 69 executable
fixtures agree at native/Terminal exit zero. Five historical consumers lose no
behavior; the ranked recursive control checks, and the stored-reference control
retains its intended requires refusal. Scoped recording adds only 28 checking
rows and 12 native rows. All four hosted entry bindings remain present;
execution here is measured on macOS ARM64.

This establishes the authored runtime selections in these owned fixed-array
predicate carriers. The parent structured-callee/domain, runtime-projection and
same-statement/theorem acceptance remains open. Arbitrary runtime-length
collections, stored-reference crossings and unsupported predicate/control or
static-environment forms are not established by this production.

## Composed checked helpers

The `*/composed_runtime_predicate/` production discharges the same
`BORROW-PROOF-CONVERGENCE` requirement through an ordinary checked helper:
`ordered(left, right)` returns `left < right || right < left`. The enclosing
predicate retains its runtime-selector bounds. Every place keeps the same fact,
two-element bound, original caller loans and writes of 17 and 53. The weak twin
changes only the helper's first `<` to `<=`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/composed_runtime_predicate/main.omg) | [weakened helper](local/composed_runtime_predicate/invalid/main.omg) |
| field | [program](field/composed_runtime_predicate/main.omg) | [weakened helper](field/composed_runtime_predicate/invalid/main.omg) |
| parameter | [program](parameter/composed_runtime_predicate/main.omg) | [weakened helper](parameter/composed_runtime_predicate/invalid/main.omg) |
| call_result | [program](call_result/composed_runtime_predicate/main.omg) | [weakened helper](call_result/composed_runtime_predicate/invalid/main.omg) |
| match_join | [program](match_join/composed_runtime_predicate/main.omg) | [weakened helper](match_join/composed_runtime_predicate/invalid/main.omg) |
| const_name | [program](const_name/composed_runtime_predicate/main.omg) | [weakened helper](const_name/composed_runtime_predicate/invalid/main.omg) |
| other_state | [program](other_state/composed_runtime_predicate/main.omg) | [weakened helper](other_state/composed_runtime_predicate/invalid/main.omg) |
| other_machine | [program](other_machine/composed_runtime_predicate/main.omg) | [weakened helper](other_machine/composed_runtime_predicate/invalid/main.omg) |
| target_body | [program](target_body/composed_runtime_predicate/main.omg) | [weakened helper](target_body/composed_runtime_predicate/invalid/main.omg) |

Establishment preserves exact immutable record/collection observations when a
helper receives scalar reads from an enclosing predicate. Freshening its scalar
formals cannot disconnect the requested relation from those observed values.
Eligible terminal helper calls use the ordinary checked call topology and the
same completed operand environment; source selection, formation, effects and
termination remain separate obligations.

Composition retains a complete equivalent condition. Each authored or derived
comparison must be independently necessary in the same full-carrier frame;
ordinary reads must then form under earlier retained guards, and the complete
conjunction must prove its converse without assuming the original predicate.
A returned match value carries bounds and disequality without choosing an order.
The observed Boolean judgment therefore proves the complete integer disjunction
from admitted captured premises, with every operand operation formed before any
counterfactual branch. Unsupported leaves and bounded search exhaustion grant
no evidence.

The [abstract stronger-helper control](controls/composed_runtime_hidden_guard/main.omg)
provides only bounds and disequality before requesting a helper with an extra
positive-left condition; it still rejects from `invoke`. The
[helper-formation control](controls/composed_runtime_requires_formation/main.omg)
cannot use the enclosing predicate's desired truth to establish the helper's
own precondition. Existing stale-value, snapshot, stored-reference, false-bound,
selected-operator and partial-operation controls retain their separate duties.
No proof or private observation identity supplies storage/access authority.

Both selected orders write different runtime results, surrounded by nonzero
caller canaries. The match arms return opposite orders. Late predicate calls
exercise true, equal-false and out-of-range-false outcomes after the loans;
those observations cannot establish earlier static facts. Complete target
requirements pass through Terminal Psi and the existing provider, layout, call,
encoding and native-image owners.

Fresh optimized producer `4238e68472a0269009c22e0a4d3a9c3a74c7e60a`
on `macos_arm64` records 178 public observations: 79 native/Terminal Psi
agreement runs and 99 fact-sensitive rejects. The retained corpus records
177 checking outcomes (78 accepted, 99 rejected) and 78 native/Terminal
agreements, with no wrong-reason or infrastructure failures. The scoped
addition is 20 checking records and nine native records; existing records
are unchanged. Five historical comparison consumers have no losses or gains;
the current disjoint/overlapping-window pair, ranked theorem, stored-reference
and partial-division controls preserve their intended outcomes.

The frozen current-main `3bef0d0cbda4cc41115bdeef27168cdefa7cd976`
local and target programs originally refused both the loans and enclosing
predicate application. Intermediate fixes retain their original producer
identities; complete acceptance above belongs to the final producer. Actual
checking/native source bundles are retained with qualification evidence.

This establishes the measured composed-helper production over these owned
fixed-array observations. The broad structured-callee/domain, runtime-projection
and same-statement/theorem acceptance remains open. It does not establish
arbitrary helper-internal operands, generic/static/evidence applications,
stored-reference crossings or arbitrary runtime-length collection predicates.
All four hosted entry bindings are present; actual execution here is macOS ARM64.
