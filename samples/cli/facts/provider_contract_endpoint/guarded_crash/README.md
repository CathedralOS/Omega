# Prove a bound helper cannot crash before running it

A helper used to calculate a range limit must be safe for the input the compiler
gives it. Dividing seven by one is allowed; dividing by zero must reject before
the helper runs.

The selected provider makes limit() equal to 7; requires value <= limit() preserves the bound needed to prove 7 - value.

This is another production of `provider_contract_endpoint`, owned by
**CANONICALIZE-SCALAR-RANGE-CONTRACTS**. It discharges the requirement to reject
crash-capable bound helpers at admission, including when a missing executable
body would otherwise obscure the reason.

`limit()` divides the selected provider's seven by `DIVISOR`. The division
helper publishes `crashes Trap denominator == 0u64`; `limit()` publishes the
corresponding guard on `DIVISOR`. Every positive sets the divisor to one. Its
twin changes only that constant to zero. Both retain the same provider, bound,
transported value, consuming subtraction, and Build selections.

The compiler must establish that no crash route remains before evaluating the
constant expression. A guarded declaration is allowed when its actual arguments
disprove every route. A missing summary is unknown, and a possible route rejects.
The published contract remains intact. Evaluation does not gain permission to
narrow an opaque declaration's contract.

| Place | Positive | Reject twin | Pins |
| --- | --- | --- | --- |
| Local | [program](local/main.omg) | [zero divisor](invalid_local/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_guarded_crash_through_local_exit/main.omg), [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_guarded_crash_through_local_rejected/main.omg) |
| Field | [program](field/main.omg) | [zero divisor](invalid_field/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_guarded_crash_through_field_exit/main.omg), [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_guarded_crash_through_field_rejected/main.omg) |
| Parameter | [program](parameter/main.omg) | [zero divisor](invalid_parameter/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_guarded_crash_through_parameter_exit/main.omg), [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_guarded_crash_through_parameter_rejected/main.omg) |
| Call result | [program](call_result/main.omg) | [zero divisor](invalid_call_result/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_guarded_crash_through_call_result_exit/main.omg), [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_guarded_crash_through_call_result_rejected/main.omg) |
| Match join | [program](match_join/main.omg) | [zero divisor](invalid_match_join/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_guarded_crash_through_match_join_exit/main.omg), [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_guarded_crash_through_match_join_rejected/main.omg) |
| Constant name | [program](const_name/main.omg) | [zero divisor](invalid_const_name/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_guarded_crash_through_const_name_exit/main.omg), [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_guarded_crash_through_const_name_rejected/main.omg) |
| Nested machine | [program](other_state/main.omg) | [zero divisor](invalid_other_state/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_guarded_crash_through_other_state_exit/main.omg), [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_guarded_crash_through_other_state_rejected/main.omg) |
| Other machine | [program](other_machine/main.omg) | [zero divisor](invalid_other_machine/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_guarded_crash_through_other_machine_exit/main.omg), [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_guarded_crash_through_other_machine_rejected/main.omg) |
| Selected implementation | [program](target_body/main.omg) | [zero divisor](invalid_target_body/main.omg) | [pass](../../../../../tests/omega/pass/facts/provider_contract_endpoint_guarded_crash_through_target_body_exit/main.omg), [fail](../../../../../tests/omega/fail/facts/provider_contract_endpoint_guarded_crash_through_target_body_rejected/main.omg) |

Each positive must pass `omega run --both` at exit zero. Each twin must reject
during source checking with ``constant expression `limit()` can crash with [Trap]``
and `a build-time value must be formed without an unhandled crash route`.
An interpreted trap or unrelated lowering failure does not establish admission.
The [ordinary runtime call](controls/runtime_zero_divisor/main.omg) deliberately
uses zero and must trap in both engines. Together with the safe runtime calls in
the nine positives, it checks both branches and ensures constant admission does
not prohibit a declared runtime crash.

The [original crashing endpoint](../controls/crashing_endpoint/main.omg) also
must reject during admission.

The governing rules are [invocation admission](../../../../../wiki/spec/language/evaluation.md#invocation-admission)
and [guarded crashes](../../../../../wiki/spec/language/effects.md#guarded-crashes).

The applicable obligations are:

- **Establishment:** the checked invocation substitutes the divisor and proves
  the crash guard false before computing the bound. The twin removes that proof
  by supplying zero. A successful later result cannot establish safety.
- **Identity and lifetime:** the summary belongs to the exact checked body and
  invocation. The transported scalar is immutable; this proof grants no storage
  or borrow authority.
- **Composition:** the same bound justifies `7u64 - value` through all nine
  places. No caller equality guard establishes that subtraction's safety.
- **Soundness:** failure admission is independent of termination, service reach,
  custody and pending call requirements. Those obligations remain necessary.
  Unknown evidence cannot stand for an empty crash row.
- **Source to Terminal:** the producer retains the complete guarded routes.
  The canonical format preserves them, and both source and saved-artifact
  evaluators must admit the invocation before executing its body.
- **Layout and calls:** the field place checks nonzero neighbors 31 and 47.
  Ordinary scalar argument and result placement must preserve seven.
- **Instructions and native execution:** every positive verifies the exact
  subtraction and a runtime `limit() == 7u64`. The match place executes inputs
  selecting seven and three and checks differences zero and four. The selected
  implementation checks architecture tags 17 and 29. A wrong branch, field
  offset or implementation must trap.

All four hosted targets have authored `ProgramEntry` bindings. Execution claims
are limited to the host actually observed; bindings alone do not prove native
execution on the other hosts.

Before the repair, the frozen optimized R13 producer ran all nine positives with
native and Terminal exit zero. All nine twins reached an interpreted `Trap`
during constant calculation. That observation establishes the missing admission
check; it does not count as the required rejection before execution. Fresh repair
qualification is recorded in the
[admission measurement](../../../../../wiki/drafts/measurements/constant_call_failure_admission_2026_11_02.md).

The repaired optimized producer runs all nine positives with native and Terminal
exit zero. Each twin rejects before execution and names the `Trap` route through
`limit`; the original crashing endpoint names the same route. The runtime-only
zero-divisor control still produces native signal 5 and Terminal `Trap` on
macOS ARM64. The array and layout customers retain their authored exits zero
and 70. Before integration, 32 corpus fixtures qualified, including saved-policy
execution. The integrated corpus checks 15 positives and rejects 16 invalid
controls; the saved-policy positive hits the same transport failure on exact
main and candidate. That failure is retained explicitly in the measurement,
not counted as current execution. Independent review and normal main landing
remain separate publication requirements.
