# Unsigned conversion through completed calls

Fact: zero-extending a bounded 16-bit unsigned u16 value (narrow >= 300u16 && narrow <= 500u16) to u32 (narrow as u32) preserves the exact interval (wide >= 300u32 && wide <= 500u32), discharging exact 32-bit subtractions 500u32 - wide and wide - 300u32 across all nine places.

This existing row discharges the remaining source-to-execution prerequisite for
**BUILD-OWNED-TARGET-SELECTION**: retire this fact's target-prefixed `exercise`
family while keeping the original scalar operation, contracts and nine places.
It also witnesses completed value-only conversion calls retaining their exact
value when passed to a private nested machine. It adds no new width or extension
mechanism and does not close the parent migration or numeric requirements.

## Programs and controls

| Place | Positive Program | Invalid Reject Twin | Pinned Pass Fixture | Pinned Fail Fixture | Carrier / Transport |
| --- | --- | --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | — | — | 16-to-32 zero extension establishes bound and discharges subtractions directly |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_local_exit` | `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_local_rejected` | widened result bound to local variable |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_field_exit` | `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_field_rejected` | widened result stored in and projected from record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_parameter_exit` | `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_parameter_rejected` | widened result passed as explicit machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_call_result_exit` | `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_call_result_rejected` | widened result returned across machine call boundary |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_match_join_exit` | `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_match_join_rejected` | widened result converged across match join arms |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_const_name_exit` | `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_const_name_rejected` | narrow input provided by named compile-time constant |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_other_state_exit` | `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_other_state_rejected` | widened result forwarded across state machine transition |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_other_machine_exit` | `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_other_machine_rejected` | widened result forwarded through intermediate machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_target_body_exit` | `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_target_body_rejected` | ordinary checked providers selected by explicit Build target rows |

The original root reproduction remains in [`main.omg`](main.omg). Positive
programs declare `Expected exit: 0` and bind the four original hosted
`ProgramEntry` profiles. The target place additionally binds macOS x86 and uses
ordinary `SelectedProbe` implementations with explicit five-target Build rows.
Both implementations retain the original `exercise` body and its public value
contract. `SelectedBodyCheck` requires architecture tag 17 or 29; its synchronous
entry is explicit in the public `invokes` ceiling and both implementations.

The runtime equality check only selects the authored exit. It contributes no
premise to the earlier `verify_wide` call or exact subtractions. The invalid
twins change their own input, source bound or result contract; their intended
reasons remain in `invalid/expected.txt` and the matching fail fixture. They do
not add caller guards to obtain the positive fact.

## Requirements and evidence

- **Establishment:** `extend_u16` admits its narrow argument under 300 through
  500 and performs the ordinary representable integer conversion. The widened
  value supplies the exact subtractions' bounds before the runtime exit check.
- **Identity and lifetime:** the completed call names the exact checked callee
  and completed immutable actuals. Scalar value identity crosses the private
  arrival; no initializer replay or lexical parent assumption supplies a fact.
- **Composition:** the same bound and operations pass through the nine places.
  Private arrivals retain every incoming path, while ordinary public provider
  callers use their published requirements rather than a selected body's facts.
- **Soundness boundaries:** conditional normal-result identity does not prove a
  call's entry requirements, termination, absence of crashes or conversion fit.
  Existing conversion formation still proves representability under every
  arithmetic policy. Reference and noninteger carrier-changing conversions keep
  their separate rules; effects and observations still prevent value-only call
  identity.
- **Checked-to-Terminal Psi:** the original conversion, scalar consumers,
  private calls and target-selection guards must lower and execute together.
  Mathematical contract reasoning supplies no executable replacement operation.
- **Settlement, layouts and calling plans:** explicit Build rows join exact
  ordinary requirements and implementations. Integer carriers and call inputs
  retain their typed widths; this row makes no new aggregate or ABI-boundary claim.
- **Instructions, homes and activation:** native execution must agree with the
  Terminal interpreter on the authored result. This row does not infer a
  particular instruction encoding from a source cast or claim a new register
  pressure boundary.
- **Encoding and emission:** retain the actual native and Terminal products for
  target runs; the same Terminal artifact must install across target profiles.
  Matching-host observations do not establish foreign-host native execution.

## Validation frontier

A fresh optimized producer from published `9f3fe016359d991862b61e46cf9bd7dc77e5734b`
wrongly refused the unchanged `other_state` positive: it could not establish
`verify_wide`'s `forwarded >= 300 && forwarded <= 500` requirement. All other eight
positive fixtures checked and all nine invalid fixtures rejected for their
intended reason. Five independent representability controls also rejected.
The value-only call closure omitted ordinary value-preserving conversions,
leaving the completed `extend_u16` result opaque during private arrival proof.

The repaired optimized producer is `cd55079f79d67a2855f2bd4d6f51616e65325d4c`;
subsequent source-only provider signature separators and fixture metadata do not
change its Rust/library inputs. The original root and all nine positive places
now produce actual native and Terminal exits 0, and all nine invalid twins still
reject for their intended obligations. The signed/narrow policy-conversion
canary also produces 0 in both engines. Five representability controls and six
normal-return/termination controls continue to reject.

A separate public-input composition witness,
[`converted_call_value_private_arrival_exit`](../../../../tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg),
checks the same nested transfer under an arbitrary `narrow` parameter with the
300-through-500 entry requirement. Its body cannot specialize from `main`'s
constant caller. The fresh baseline wrongly refuses it; the repaired compiler
checks it and both engines observe 0. Its
[reject twin](../../../../tests/omega/fail/calls/converted_call_value_private_arrival_rejected/main.omg)
widens its own input contract to 501 and still rejects.

The target place executes with exit 0 on macOS ARM and x86/Rosetta. Its unchanged
Terminal artifact installs with exit 0 under all five declared target profiles;
wrong-probe and wrong-whole selection controls produce typed Terminal Traps.
Actual emitted products and exact raw commands are retained in the qualification
record. Windows/Linux native execution, native Trap behavior, a new numeric-width
mechanism and the full parent migration remain unverified by this row.

Specifications: [numeric values](../../../../wiki/spec/language/numeric_values.md),
[invocation contracts](../../../../wiki/spec/language/state_contracts.md),
[provider selection](../../../../wiki/spec/build/provider_selection.md), and
[effects](../../../../wiki/spec/language/effects.md).
