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

The [computed-argument control](controls/computed_actuals/main.omg) retains independently formed arithmetic actuals. The [qualified-field control](controls/qualified_actuals/main.omg) retains two distinct exact selector fields from the input record. The [compensating-increase control](controls/invalid_compensating_increase/main.omg) preserves the full sum while decreasing one cell; it must reject.

## Requirements

- Establishment: each helper's index bounds and each exact arithmetic operation
  form from the invocation's existing inputs. Neither the proposed rank nor its
  desired bound supplies a premise.
- Identity and lifetime: a captured read retains its exact declaration-call path
  as well as its root and projection. Equal source handles under different
  actual telescopes do not identify the observations.
- Composition: calls enter a fresh formal namespace with only the reads
  belonging to that invocation; nested paths retain their remaining context.
  All component edges must preserve rank and every complete cycle must descend.
- Soundness: the nonprogress twins and compensating-increase control refuse.
  Whole-value custody, access, purity, totality, input qualification and operand
  formation remain independent obligations.
- Psi 06–08: accepted coordinates and their supplying call paths are retained in
  the ordinary checked arrival plan. Existing declared-call execution consumes
  the same complete initialized input and exact selected helper contract.
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

The source and oracle are frozen before compiler edits on base
`c3ee25e981f669580c8f7f27cf2b794d44d837d8`. Producer identities, diagnostics and
actual command observations are retained in the workbench's ignored
`build/advance-evidence/` and summarized in the publication commit.

The parent outcome remains open for the other declared call/custody mechanisms,
remaining ranking forms, product-compiler parity and source migration.

The optimized baseline producer has SHA-256 `7e90bff12392df03f23785f20f85d4d3de425845b41c1105ea4bae18b01a0245` and opt-level 1. Its one-call control checks; the unchanged two-call positive refuses with `a computed natural rank application has no independent entry formation`. The earlier copy-predicate probe and environment setup refusals remain separate historical attempts.
