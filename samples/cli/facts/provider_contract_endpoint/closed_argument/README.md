# Closed arguments before a provider bound

A helper can use the number chosen by a match in its input requirement. Here,
the true arm supplies 254, anonymous addition produces 255, and the u8 parameter
lands that value before its Wrapping addition produces 1. The helper's input
requirement therefore establishes value <= 1, which justifies its ordinary call
requiring value <= 257.

This is another production of **CANONICALIZE-SCALAR-RANGE-CONTRACTS** under the
existing provider endpoint fact. Before computing the provider bound,
limit() must admit that helper call with input 1. The selected provider, input 7
and exact 7u64 - value operation are unchanged. Every place copies the original
fact sentence.

The historical [endpoint helper customer](../../../../../tests/omega/pass/generics/declared_range_endpoint_inference/README.md)
exposed the same refusal after its five original input/result clauses were
restored. Those clauses, explicit generic arguments, original bodies and exit-70
oracle remain part of the acceptance.

| Place | Positive | Provider-6 reject | Exact corpus pins |
| --- | --- | --- | --- |
| Local | [program](local/main.omg) / [Build](local/build.omg) | [twin](local/invalid/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_closed_argument_through_local_exit/main.omg) / [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_closed_argument_through_local_rejected/main.omg) |
| Field | [program](field/main.omg) / [Build](field/build.omg) | [twin](field/invalid/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_closed_argument_through_field_exit/main.omg) / [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_closed_argument_through_field_rejected/main.omg) |
| Parameter | [program](parameter/main.omg) / [Build](parameter/build.omg) | [twin](parameter/invalid/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_closed_argument_through_parameter_exit/main.omg) / [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_closed_argument_through_parameter_rejected/main.omg) |
| Call result | [program](call_result/main.omg) / [Build](call_result/build.omg) | [twin](call_result/invalid/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_closed_argument_through_call_result_exit/main.omg) / [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_closed_argument_through_call_result_rejected/main.omg) |
| Match join | [program](match_join/main.omg) / [Build](match_join/build.omg) | [twin](match_join/invalid/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_closed_argument_through_match_join_exit/main.omg) / [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_closed_argument_through_match_join_rejected/main.omg) |
| Const name | [program](const_name/main.omg) / [Build](const_name/build.omg) | [twin](const_name/invalid/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_closed_argument_through_const_name_exit/main.omg) / [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_closed_argument_through_const_name_rejected/main.omg) |
| Nested machine | [program](other_state/main.omg) / [Build](other_state/build.omg) | [twin](other_state/invalid/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_closed_argument_through_other_state_exit/main.omg) / [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_closed_argument_through_other_state_rejected/main.omg) |
| Other machine | [program](other_machine/main.omg) / [Build](other_machine/build.omg) | [twin](other_machine/invalid/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_closed_argument_through_other_machine_exit/main.omg) / [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_closed_argument_through_other_machine_rejected/main.omg) |
| Selected target implementation | [program](target_body/main.omg) / [Build](target_body/build.omg) | [twin](target_body/invalid/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_closed_argument_through_target_body_exit/main.omg) / [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_closed_argument_through_target_body_rejected/main.omg) |

The field place retains nonzero neighbors 31 and 47. The match place still
executes both bounded values 7 and 3, producing differences 0 and 4. The target
place retains its selected implementations, invocation ceilings and tags 17
and 29. Each Build binds all four hosted entries.

## What execution must show

Every main also checks that the prerequisite returns 257 and that limit()
returns 7. argument_result(true) must return 1; argument_result(false) must
return 3. Both calls run. A wrong arm, mathematical addition after erasing the
u8 Wrapping policy, or wrong helper return sends main to Trap. These assertions
do not establish the source bound; the helper's checked input requirement does.

Each row twin changes only the selected provider's result from 7 to 6.
The unchanged captured input 7 must fail bounded's input requirement. An
unrelated refusal in the prerequisite does not count as this rejection.

## Focused controls

- [Prerequisite input 2](controls/prerequisite_input/invalid/main.omg) changes
  only limit's helper actual from 1 to 2; its [Build](controls/prerequisite_input/invalid/build.omg)
  is unchanged. The header's true expression wraps to 1, so this call must
  reject. Choosing the false header arm by mistake would yield 3 and silently
  admit 2. Its [pin](../../../../../tests/omega/fail/facts/provider_contract_endpoint_closed_argument_prerequisite_input_rejected/main.omg)
  checks the prerequisite's own obligation.
- [Ordinary argument evaluation](controls/anonymous_landing/main.omg) and its
  [Build](controls/anonymous_landing/build.omg) retain a separately measured
  working body path with the same true-1/false-3 runtime checks. Its
  [pin](../../../../../tests/omega/pass/facts/provider_contract_endpoint_closed_argument_anonymous_landing_exit/main.omg)
  remains a preservation control.
- [Anonymous initial landing](controls/anonymous_landing/invalid/main.omg) and
  its [Build](controls/anonymous_landing/invalid/build.omg) change only the
  true arm from 254 to 255. The complete anonymous result is then 256 and
  cannot first land in u8. A Wrapping destination cannot repair that initial
  value. The [pin](../../../../../tests/omega/fail/facts/provider_contract_endpoint_closed_argument_anonymous_landing_rejected/main.omg)
  requires a formation refusal, not a later malformed-operation error.

## Requirements and evidence

- Establishment: every invocation proves the complete prerequisite and provider
  contracts before consuming their values. Anonymous arithmetic preserves the
  selected operator; classification alone proves no operation safe.
- Identity and lifetime: the exact selected match value enters the declared u8
  parameter. Its later Wrapping result is the actual stored value, not the
  mathematical pre-wrap sum. Named, typed and previously landed values retain
  their own carriers and policies.
- Composition: the same prerequisite precedes the selected provider in every
  place. The resulting bound travels through the ordinary call, field, join or
  selected implementation into the unchanged subtraction.
- Soundness: call preconditions, numeric representability, operator selection,
  termination and invocation admission remain independent debts. Neither an
  entry hypothesis nor the conclusion being proved establishes its own partial
  operand.
- Checked-to-Terminal lowering: ordered matches, parameter landing, wrapping
  arithmetic and scalar values must survive ordinary lowering without source
  shape recognition.
- Provider and ABI settlement: Build selects the existing provider and target
  bodies. Normal scalar calling plans and field placement retain the values
  and nonzero neighbors.
- Native realization and emission: every authored positive must execute at
  zero in both Terminal and native execution. Wrong branch, arithmetic, field
  location or target selection must fail the authored runtime checks.

The governing rules are [numeric landing and arithmetic](../../../../../wiki/spec/language/numeric_values.md#literal-landing-and-destinations),
[invocation admission](../../../../../wiki/spec/language/evaluation.md#invocation-admission)
and [ordered matches](../../../../../wiki/spec/language/patterns.md).

## Qualification frontier

The retained optimized 420805 compiler (SHA-256
203f5fe996b80a514855c66bc9ba426f8f8937b9e62b395adc0bf78195ce2b29)
wrongly refuses the frozen local prerequisite prototype: it loses the header
bound and cannot prove the relaxed <= 257 call. The provider-6 and input-2
prototypes currently hit that earlier gap and receive no intended-rejection
credit. The ordinary body control already runs at zero in both engines; its
256-to-u8 twin rejects only with a late VerifiedOperationMalformed error,
which does not establish initial-landing enforcement.

The nine transported sources and exact pins are frozen but are not yet
qualified with a repaired compiler. Raw baseline records and source hashes
remain under build/scalar-continue-20261102/endpoint-contracts-*. This frontier
must be replaced with actual repaired-source outcomes before landing.

This production does not close the historical 1,598-position audit, reconcile
unrelated RangeValue declarations, retire scalar suffix parsing, or qualify
native execution on every hosted architecture. The parent task's remaining
acceptance stays open.
