# Admit the call before computing its bound

A bound helper can call another function only after establishing that call's
precondition. These programs distinguish a precondition proved from an
independent calculation from one that relies on the helper's unfinished result.
The resulting bound must still justify subtraction.

The selected provider makes limit() equal to 7; requires value <= limit() preserves the bound needed to prove 7 - value.

This is another production of the existing `provider_contract_endpoint` fact,
owned by **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. Its exact requirement is to
reject a helper whose invocation admission depends on that same pending helper
result, without rejecting an independently proved invocation.

In every positive, `limit()` first calls `need(7u64)`, whose requirement is
`value <= independent_limit()`. The separate `independent_limit()` calls
`Endpoint::upper<19u64>()` and returns seven. Its result must be computed and the
requirement proved before running `limit()`, which later calls
`Endpoint::upper<13u64>()`. In each twin, only that requirement changes to
`value <= limit()`: computing the bound now needs the bound itself to admit
`need(7u64)`. Both helpers, their exact selections and all runtime checks are
otherwise identical. The provider still returns seven, and the input,
transported bound, exact subtraction and Build selections are unchanged.
A later successful return cannot retroactively authorize an earlier call.

| Place | Positive source | Reject source | Positive pin | Reject pin |
| --- | --- | --- | --- | --- |
| Local | [main](local/main.omg), [Build](local/build.omg) | [main](local/invalid/main.omg), [Build](local/invalid/build.omg) | [main](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_local_exit/main.omg), [Build](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_local_exit/build.omg) | [main](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_local_rejected/main.omg), [Build](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_local_rejected/build.omg) |
| Field | [main](field/main.omg), [Build](field/build.omg) | [main](field/invalid/main.omg), [Build](field/invalid/build.omg) | [main](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_field_exit/main.omg), [Build](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_field_exit/build.omg) | [main](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_field_rejected/main.omg), [Build](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_field_rejected/build.omg) |
| Parameter | [main](parameter/main.omg), [Build](parameter/build.omg) | [main](parameter/invalid/main.omg), [Build](parameter/invalid/build.omg) | [main](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_parameter_exit/main.omg), [Build](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_parameter_exit/build.omg) | [main](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_parameter_rejected/main.omg), [Build](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_parameter_rejected/build.omg) |
| Call result | [main](call_result/main.omg), [Build](call_result/build.omg) | [main](call_result/invalid/main.omg), [Build](call_result/invalid/build.omg) | [main](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_call_result_exit/main.omg), [Build](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_call_result_exit/build.omg) | [main](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_call_result_rejected/main.omg), [Build](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_call_result_rejected/build.omg) |
| Match join | [main](match_join/main.omg), [Build](match_join/build.omg) | [main](match_join/invalid/main.omg), [Build](match_join/invalid/build.omg) | [main](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_match_join_exit/main.omg), [Build](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_match_join_exit/build.omg) | [main](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_match_join_rejected/main.omg), [Build](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_match_join_rejected/build.omg) |
| Const name | [main](const_name/main.omg), [Build](const_name/build.omg) | [main](const_name/invalid/main.omg), [Build](const_name/invalid/build.omg) | [main](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_const_name_exit/main.omg), [Build](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_const_name_exit/build.omg) | [main](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_const_name_rejected/main.omg), [Build](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_const_name_rejected/build.omg) |
| Nested machine | [main](other_state/main.omg), [Build](other_state/build.omg) | [main](other_state/invalid/main.omg), [Build](other_state/invalid/build.omg) | [main](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_other_state_exit/main.omg), [Build](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_other_state_exit/build.omg) | [main](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_other_state_rejected/main.omg), [Build](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_other_state_rejected/build.omg) |
| Other machine | [main](other_machine/main.omg), [Build](other_machine/build.omg) | [main](other_machine/invalid/main.omg), [Build](other_machine/invalid/build.omg) | [main](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_other_machine_exit/main.omg), [Build](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_other_machine_exit/build.omg) | [main](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_other_machine_rejected/main.omg), [Build](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_other_machine_rejected/build.omg) |
| Selected implementation | [main](target_body/main.omg), [Build](target_body/build.omg) | [main](target_body/invalid/main.omg), [Build](target_body/invalid/build.omg) | [main](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_target_body_exit/main.omg), [Build](../../../../../tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_target_body_exit/build.omg) | [main](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_target_body_rejected/main.omg), [Build](../../../../../tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_target_body_rejected/build.omg) |

The `main.omg` and `build.omg` files in each sample and its corpus pin are exact
copies. All positives declare `Expected exit: 0` and bind all four hosted
`ProgramEntry` targets. Rejection pins require the diagnostic fragment
`cannot establish its own pending target condition`; a setup error or unrelated
refusal does not establish that obligation.

The governing rules are [invocation admission](../../../../../wiki/spec/language/evaluation.md#invocation-admission),
[call and completion contracts](../../../../../wiki/spec/language/state_contracts.md),
and [scalar propositions](../../../../../wiki/spec/language/domains.md#declaration-and-membership).
Formation must establish each call requirement before evaluation. No pending
result supplies its own premise, even if evaluating the body happens to return
the expected number.

The applicable requirements are:

- **Establishment:** the separate provider invocation computes the independent
  bound and admits `need(7u64)`, then the helper's exact selected provider
  establishes seven. The twin removes only the independent admission premise. Caller guards do not establish subtraction's
  bound.
- **Identity and lifetime:** the pending condition belongs to this exact helper
  invocation and its recorded provider selection. Another invocation's result
  cannot discharge it. The transported scalars are immutable values and grant
  no storage or borrow authority.
- **Composition:** the same bound reaches `7u64 - value` through all nine places,
  including parameter and result contracts, constant initialization, both match
  branches, and selected implementations.
- **Soundness:** a call must establish its precondition before using the
  conclusions it promises. A successful later return cannot establish an earlier
  premise. This row exercises that ordering. The
  [guarded-call examples](../guarded_crash/README.md) separately check that a
  helper cannot crash before it is evaluated as a bound.
- **Source to Terminal:** retained call requirements must survive into the
  canonical evaluation job. After admission, the checked calls and transported
  result must execute with the same meaning in Terminal Psi.
- **Layout and calls:** the field place checks nonzero neighbors 31 and 47;
  ordinary scalar argument/result placement retains seven across helper calls.
  The selected implementation uses Build and checks architecture tags 17 and 29.
- **Instructions, storage and native emission:** every `main()` checks both the
  exact subtraction result and a runtime `limit() == 7u64`. The match place
  executes inputs selecting seven and three, then checks differences zero and
  four; taking one arm twice traps. A wrong field offset overwrites a checked
  neighbor, and a wrong target body fails its tag check. Native and Terminal
  execution must agree at zero for every positive.

The historical R6 prototype source checks are retained in
`build/scalar-continue-20261102/admission-prototype-before_call_literal-r6.json`
and `admission-prototype-circular_call_literal-r6.json`. Both commands exited
zero: the positive checked, and the invalid call was wrongly accepted. Those
are source-check observations for the small literal-bound prototypes, not
execution evidence or baseline results for this strengthened production. The
original authored row and its baseline remain frozen in the claim evidence;
version two replaces the independent literal premise with the separate selected
provider result. Its frozen source manifest and new baseline records identify
that change without rewriting earlier observations.

The frozen optimized R6 compiler accepts all nine positives and runs each with
native exit zero and Terminal exit zero on macOS ARM64. It also wrongly accepts
all nine twins. The strengthened sources, their hashes, the ordinary package
updates, and all command results are retained in
`pending-call-v2-authored-source-manifest.json` and
`pending-call-v2-baseline-summary.json` under the claim evidence directory,
`build/scalar-continue-20261102`. Those are baseline observations, not results
from the compiler repair.

The optimized R13 compiler checks all nine positives and runs each with native
exit zero and Terminal exit zero on macOS ARM64. Every twin rejects during its
normal package update with `cannot establish its own pending target condition`.
A later direct check of these rejected projects stops at the compiler-identity
review fence; that setup refusal is not counted as proof of the obligation.
The original circular control also rejects with both expected diagnostic
fragments. Raw results and the exact compiler inputs are retained in the claim
evidence under `pending-admission-r13-integrated-qualification-*`.

This closes circular call admission for this fact. The guarded-crash row covers
crash-capable helpers. The broader scalar-range migration remains open. Execution
on the other hosted targets is unverified.
