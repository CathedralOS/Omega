# Closed literal arguments through a quotient

Owning task: **PROOF-KERNEL-CORE** (`TASKS.md`).

This row checks that `Quotient::lift` passes the exact closed argument `3i32` to its selected representative operation across the nine transport places. The operation's precondition, theorem premises and authored result stay the same as its representative relation moves to explicit identity.

One sentence: `Closed literal argument 3i32 in Quotient::lift satisfies representative precondition amount == 3i32 and produces 3i32.`

`Representative` wraps an `i32`. Its `equivalent` relation states complete-value identity with `core::Identity<Representative>`; the conformance supplies the relation's reflexivity, symmetry and transitivity laws. The separate `amount == 3i32` precondition remains ordinary integer comparison.

## Existing witnesses and controls

| Place | Positive witness | Invalid control | Corpus pass fixture | Corpus fail fixture |
| --- | --- | --- | --- | --- |
| Local | [local/main.omg](local/main.omg) | [invalid_local/main.omg](invalid_local/main.omg) | `pass/facts/quotient_closed_literal_through_local_exit` | `fail/facts/quotient_closed_literal_through_local_rejected` |
| Field | [field/main.omg](field/main.omg) | [invalid_field/main.omg](invalid_field/main.omg) | `pass/facts/quotient_closed_literal_through_field_exit` | `fail/facts/quotient_closed_literal_through_field_rejected` |
| Parameter | [parameter/main.omg](parameter/main.omg) | [invalid_parameter/main.omg](invalid_parameter/main.omg) | `pass/facts/quotient_closed_literal_through_parameter_exit` | `fail/facts/quotient_closed_literal_through_parameter_rejected` |
| Call result | [call_result/main.omg](call_result/main.omg) | [invalid_call_result/main.omg](invalid_call_result/main.omg) | `pass/facts/quotient_closed_literal_through_call_result_exit` | `fail/facts/quotient_closed_literal_through_call_result_rejected` |
| Match join | [match_join/main.omg](match_join/main.omg) | [invalid_match_join/main.omg](invalid_match_join/main.omg) | `pass/facts/quotient_closed_literal_through_match_join_exit` | `fail/facts/quotient_closed_literal_through_match_join_rejected` |
| Const name | [const_name/main.omg](const_name/main.omg) | [invalid_const_name/main.omg](invalid_const_name/main.omg) | `pass/facts/quotient_closed_literal_through_const_name_exit` | `fail/facts/quotient_closed_literal_through_const_name_rejected` |
| Nested machine | [other_state/main.omg](other_state/main.omg) | [invalid_other_state/main.omg](invalid_other_state/main.omg) | `pass/facts/quotient_closed_literal_through_other_state_exit` | `fail/facts/quotient_closed_literal_through_other_state_rejected` |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [invalid_other_machine/main.omg](invalid_other_machine/main.omg) | `pass/facts/quotient_closed_literal_through_other_machine_exit` | `fail/facts/quotient_closed_literal_through_other_machine_rejected` |
| Target body | [target_body/main.omg](target_body/main.omg) | [invalid_target_body/main.omg](invalid_target_body/main.omg) | `pass/facts/quotient_closed_literal_through_target_body_exit` | `fail/facts/quotient_closed_literal_through_target_body_rejected` |

The root [main.omg](main.omg) is another existing witness for the same automatic lift with the closed literal `3i32`.

## Required checks

- **Establishment:** Check the selected equivalence laws and the exact congruence theorem. The lift must establish the representative's `amount == 3i32` precondition and both call-legality premises under the actual arguments.
- **Identity and custody:** Keep the complete Representative relation, the quotient's selected relation and the exact `i32` literal. The source declares custody-free `[copy]` carriers; it does not authorize observing or extracting an arbitrary quotient representative.
- **Composition:** Each positive transports the same quotient value into the lifted operation and checks that its result is `3`. Each invalid twin changes only the lifted literal to `4i32` and must reject because the representative's precondition is not established.
- **Lowering:** Erase the checked theorem uses and lower the lift through the ordinary representative call, retaining the literal argument, result flow, layout and calling plan. Terminal Psi and native execution must agree with the authored oracle.
- **Targets:** Every positive Build binds ProgramEntry for `linux_x86_64`, `linux_arm64`, `macos_arm64` and `windows_x86_64`. The target-body place selects ordinary named implementations through Build and checks the existing x86-64/AArch64 tags, so a wrong selection traps.

## Validation status

Positive acceptance requires `omega run --both` with exit `0` on all four hosted targets, plus the intended rejection for every invalid twin. Exact-source runs or applicable retained evidence must identify the producer and target under `tools/testing.md`.

This source migration retains source, operand, Build, mirror and twin-difference audits. It does not supply compiler proof or runtime acceptance. The changed corpus fixtures still need clean recording on the integrated producer. Historical Linux x86-64 observations belong to the legacy source recorded by those runs; they do not establish that this migrated source passed. Check their consumer-based applicability before any reuse.
