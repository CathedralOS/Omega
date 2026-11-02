# Repeated declared rank-call inputs

Owner: **NESTED-MACHINE-UNIFICATION** in [TASKS.md](../../../../../../../../../../TASKS.md).

> A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

This is another production inside the existing joint-rank fact. The nearest
production is [one indexed call](../controls/public_call/main.omg). The measure
now calls the same helper twice with different actual indices. Both applications
share the helper's authored `input.values[index]` expression; their supplying
call telescopes remain distinct. Their sum decreases when slot zero decreases
and slot one remains unchanged. A decrease in slot zero offset by an increase
in slot one must not establish strict descent.

Every place transports the same record into `count`. The active invocation
advances the caller's counter seven times; the empty invocation leaves it
unchanged. The caller runs both neighbor values 919 and 37, checks that the
active word becomes zero and that the neighbor and canary 641 survive, and
observes counter 3→10→17. The field place also checks envelope canaries 373 and
587. Every twin changes only decrement 1 to decrement 0. All four hosted entry
bindings are authored, and target-body implementations retain distinct tags.

| Place | Positive | Nonprogress twin |
| --- | --- | --- |
| local | [source](main.omg) | [source](invalid/main.omg) |
| field | [source](../../../../../field/declared_measure/computed_record/selected_value/repeated_actuals/main.omg) | [source](../../../../../field/declared_measure/computed_record/selected_value/repeated_actuals/invalid/main.omg) |
| parameter | [source](../../../../../parameter/declared_measure/computed_record/selected_value/repeated_actuals/main.omg) | [source](../../../../../parameter/declared_measure/computed_record/selected_value/repeated_actuals/invalid/main.omg) |
| call_result | [source](../../../../../call_result/declared_measure/computed_record/selected_value/repeated_actuals/main.omg) | [source](../../../../../call_result/declared_measure/computed_record/selected_value/repeated_actuals/invalid/main.omg) |
| match_join | [source](../../../../../match_join/declared_measure/computed_record/selected_value/repeated_actuals/main.omg) | [source](../../../../../match_join/declared_measure/computed_record/selected_value/repeated_actuals/invalid/main.omg) |
| const_name | [source](../../../../../const_name/declared_measure/computed_record/selected_value/repeated_actuals/main.omg) | [source](../../../../../const_name/declared_measure/computed_record/selected_value/repeated_actuals/invalid/main.omg) |
| other_state | [source](../../../../../other_state/declared_measure/computed_record/selected_value/repeated_actuals/main.omg) | [source](../../../../../other_state/declared_measure/computed_record/selected_value/repeated_actuals/invalid/main.omg) |
| other_machine | [source](../../../../../other_machine/declared_measure/computed_record/selected_value/repeated_actuals/main.omg) | [source](../../../../../other_machine/declared_measure/computed_record/selected_value/repeated_actuals/invalid/main.omg) |
| target_body | [source](../../../../../target_body/declared_measure/computed_record/selected_value/repeated_actuals/main.omg) | [source](../../../../../target_body/declared_measure/computed_record/selected_value/repeated_actuals/invalid/main.omg) |

The [computed-argument control](controls/computed_actuals/main.omg) retains
independently formed arithmetic actuals. The
[qualified-field control](controls/qualified_actuals/main.omg) retains two
distinct exact selector fields from the input record. The
[nested public-call control](controls/nested_public_calls/main.omg) derives its
result through another independently checked helper contract. The
[constructor control](controls/constructed_actuals/main.omg) also exercises
explicit successor construction; the primary nine places retain copy and
indexed writeback. The
[compensating-increase control](controls/invalid_compensating_increase/main.omg)
preserves the full sum while decreasing one cell; it must reject.

The [Boolean-dispatch control](controls/boolean_dispatch/main.omg) preserves
the same sum and the same 7/919 bounds: it returns slot one when slot zero is
zero, and otherwise adds both slots. Its condition contains a helper call.
The complete entry-observation graph owns that call's operands and receipts;
those operands do not acquire a synthetic statement origin. Its
[zero-step twin](controls/boolean_dispatch/invalid/main.omg) must still reject
cycle descent.

The missing/partial-bound controls refuse unformed arithmetic; the missing
premise and unformed-child controls refuse helper preconditions, including a
canceled child operation. The no-result-relation control cannot replace a
public postcondition with a helper's private body. The stale-poststate control
cannot reuse an entry equality after its source field changes. Corpus-only
store controls distinguish a proved unequal or ordered selector from an
overlapping or unknown selector before preserving an untouched sibling cell.

## Requirements

- Establishment: each helper's index bounds and each exact arithmetic operation
  form from the invocation's existing inputs. Neither the proposed rank nor its
  desired bound supplies a premise.
- Identity and lifetime: a captured read retains its exact declaration-call path
  as well as its root and projection. Equal source handles under different
  actual telescopes do not identify the observations.
- Composition: calls enter a fresh formal namespace with only the reads
  belonging to that invocation; nested paths retain their remaining context.
  Captured record roots remain available beside their scalar observations when
  a public postcondition supplies another call. Call normalization preserves an
  existing field denotation when the selector becomes a literal.
  All component edges must preserve rank and every complete cycle must descend.
- Soundness: the nonprogress twins and compensating-increase control refuse.
  Whole-value custody, access, purity, totality, input qualification and operand
  formation remain independent obligations.
- Psi 06–08: accepted coordinates and their supplying call paths are retained in
  the ordinary checked arrival plan. Existing declared-call execution consumes
  the same complete initialized input and exact selected helper contract.
  Callee-context reads do not become duplicate caller-body captures; direct
  declaration reads remain available to the ordinary expression builder.
- Omega 00–02: ordinary array/record geometry, aggregate return and calling plans
  carry both words and their canary. All four hosted target bindings remain
  explicit; selected target implementations use ordinary Build provider rows.
- Omega 03–06: returned records, unchanged neighbors and caller-owned counters
  make lost storage or incorrect placement observable. These runs add no portable
  stack-bound certification.
- Omega 07–09: acceptance requires actual Linux x86-64 native/Terminal exit-0
  agreement and fact-sensitive rejection controls. Other-host physical execution
  remains unmeasured.

## Evidence and remaining scope

The initial repeated-call probe was recorded against base
`c3ee25e981f669580c8f7f27cf2b794d44d837d8`. The nine primary places retain the
original record copy and indexed writeback. Additional controls were frozen as
formation, composition and dispatch gaps were isolated, before their respective
repairs. Producer identities, source identities, diagnostics and actual command
observations are retained in the workbench's ignored `build/advance-evidence/`
and summarized in the publication commit.

The parent outcome remains open for the other declared call/custody mechanisms,
remaining ranking forms, product-compiler parity and source migration.

The optimized baseline producer has SHA-256 `7e90bff12392df03f23785f20f85d4d3de425845b41c1105ea4bae18b01a0245` and opt-level 1. Its one-call control checks; the unchanged two-call positive refuses with `a computed natural rank application has no independent entry formation`. The earlier copy-predicate probe and environment setup refusals remain separate historical attempts.

The [unformed-arm control](controls/invalid_unformed_arm/main.omg) changes only
the zero-input arm selector from 1 to 2. The private positive-input state
cannot donate its unreachable-arm decision to the public entry application,
which admits zero and must form that arm independently.


## Qualified observations

The optimized compiler source is `c760f905800634aab6da0eec9f97f745aac7d753`.
The original nine-place copy and indexed writeback programs remain unchanged.

| Observation | Result |
| --- | --- |
| Sample gate | 32 projects: 14 positives execute native/Terminal exit 0/0; 18 intended rejections match. |
| Default corpus checking | 36 fixtures: 16 checked and 20 rejected; positive entry realization selected macOS ARM64, without physical execution. |
| Linux x86-64 corpus execution | 16 positives: native exit 0, Terminal exit 0, and interpreter agreement. |
| Runtime falsifiers | Wrong counter and wrong selected-provider expectation each produce native signal 4 and agreeing Terminal Trap. |
| Existing productions | Two positives execute in both engines; the borrowed zero-step twin rejects. |
| Unchanged peer selector source | Native/Terminal exit 0/0; its zero-step twin rejects. |
| Three-arm composition | Checking and native/Terminal exit 0/0. |

CLI SHA-256: `49aecfa59f0bb8471c24dd239684d8fc15a40322083350a32f1daed701ca9cdf`.
Corpus runner SHA-256: `b292bf93354ed8987c69997dce86c0e46c910fe6685067c867ec42c3a137d9cd`.
Bundled library tree: `0e12fe346c8e5eb1419538cacf183220f4f8574d`.
Checking and execution outcomes retain separate commits and producer identities.

Integration retains the complete read metadata and observation-root repair
published by [#1453](https://github.com/CathedralOS/Omega/pull/1453). The nested
public getter also needs the shared referent-snapshot projection published by
[#1500](https://github.com/CathedralOS/Omega/pull/1500): its exact two-file move
replaces the storage-only crossing, preserving readable access, carrier identity
and descriptor-only reference children. The targeted trace found the owned
snapshot hidden by the structural term's diagnostic display; no purity or
repeatability gate was relaxed.
