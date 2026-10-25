# Signed 32-bit wrapping multiplication bound

Fact: 32-bit signed wrapping multiplication lhs *% rhs with bounded interval 50i32 <= prod <= 200i32 establishes 50i32 <= prod <= 200i32 across all nine places so exact subtractions 200i32 - prod and prod - 50i32 are safe.

Owning Task: `NUMERIC-SCALARS` / `CONSERVATION-CONTRACT` (`TASKS.md`)

Every positive place and reject twin tests 32-bit signed wrapping multiplication (`i32 in Wrapping`).
When bounded operands `lhs >= 10i32 && lhs <= 20i32` and `rhs >= 5i32 && rhs <= 10i32` are cast into the Wrapping arithmetic domain (`lhs as i32 in Wrapping` and `rhs as i32 in Wrapping`), their modular multiplication `wl * wr` produces a bounded result in `[50, 200]`.
Casting back to exact signed `i32` statically preserves the interval `prod >= 50i32 && prod <= 200i32`.
Subsequent exact subtractions `headroom = 200i32 - prod` and `margin = prod - 50i32` inside `verify_prod` are proved underflow-free and non-negative, evaluating to `80i32 + 70i32 = 150i32` for `lhs == 15i32` and `rhs == 8i32` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with bounded i32 operands wrapping-multiplied and dual margin subtractions |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | wrapping-multiplied 32-bit signed value is copied into a local binding before exact subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | wrapping-multiplied 32-bit signed value is stored into and read from a record field before exact subtractions |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | wrapping-multiplied 32-bit signed value is passed as an explicit machine parameter before exact subtractions |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | wrapping-multiplied 32-bit signed value is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the wrapping-multiplied 32-bit signed value |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply upper and lower ceilings for exact subtractions |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the wrapping-multiplied 32-bit signed value into a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the wrapping-multiplied 32-bit signed value |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing the wrapping multiplication and verification |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/signed_32bit_wrapping_multiplication_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_multiplication_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(15i32, 8i32)` computes `prod = 15i32 *% 8i32 = 120i32`.
Because `lhs in [10, 20]` and `rhs in [5, 10]`, wrapping multiplication statically establishes `prod in [50, 200]`.
Subsequent exact subtractions in `verify_prod` (`headroom = 200i32 - prod = 80i32` and `margin = prod - 50i32 = 70i32`) evaluate to `80i32 + 70i32 = 150i32`.
The transition guard `combined == 150i32` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
A 1-token perturbation of the demanded result (`transition combined == 150i32` to `transition combined == 151i32`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens the narrow input precondition bound on `lhs` from `lhs <= 20i32` to `lhs <= 21i32`.
When `lhs == 21i32` and `rhs == 10i32`, `prod` reaches `21 * 10 = 210i32 > 200i32`, which violates the contract precondition `value <= 200i32` required by `verify_prod`.
The compiler strictly fails closed at compile time:

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| `local` | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod from probe: local_prod >= 50 && local_prod <= 200` |
| `field` | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod from probe: holder.prod >= 50 && holder.prod <= 200` |
| `parameter` | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call consume_prod from probe: prod >= 50 && prod <= 200` |
| `call_result` | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract postcondition | `cannot prove ensures contract for exit from wrap_mul at statement 3: result >= 50 && result <= 200` |
| `match_join` | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call choose from probe: prod >= 50 && prod <= 200 && 120 >= 50 && 120 <= 200` |
| `const_name` | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod from probe: prod >= 50 && prod <= 200` |
| `other_state` | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call consume from probe: prod >= 50 && prod <= 200` |
| `other_machine` | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call forward_prod from probe: prod >= 50 && prod <= 200` |
| `target_body` | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod from target_probe: prod >= 50 && prod <= 200` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/signed_32bit_wrapping_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection.

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_32bit_wrapping_multiplication_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_32bit_wrapping_multiplication_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_32bit_wrapping_multiplication_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_32bit_wrapping_multiplication_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_32bit_wrapping_multiplication_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_32bit_wrapping_multiplication_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_32bit_wrapping_multiplication_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_32bit_wrapping_multiplication_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_32bit_wrapping_multiplication_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/signed_32bit_wrapping_multiplication_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_32bit_wrapping_multiplication_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_32bit_wrapping_multiplication_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_32bit_wrapping_multiplication_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_32bit_wrapping_multiplication_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_32bit_wrapping_multiplication_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_32bit_wrapping_multiplication_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_32bit_wrapping_multiplication_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_32bit_wrapping_multiplication_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence (AGENTS.md)

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `CONSERVATION-CONTRACT` ([TASKS.md](../../../../TASKS.md)).
- **Ticket:** `e5f111e9eb9745b6ac80c7ba5eaf1ca0`.
- **Establishment** ([numeric values](../../../../wiki/spec/language/numeric_values.md), [invocation contracts](../../../../wiki/spec/language/state_contracts.md), [domains](../../../../wiki/spec/language/domains.md)): Caller contract preconditions `lhs >= 10i32 && lhs <= 20i32` and `rhs >= 5i32 && rhs <= 10i32` establish bounded 32-bit signed intervals. Casting operands into `i32 in Wrapping` evaluates wrapping modular multiplication modulo $2^{32}$, statically proving the absence of wrapping and establishing the exact post-multiplication interval `[50, 200]` without post-hoc caller equality guards.
- **Identity and lifetime** ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), [lifetimes](../../../../wiki/spec/language/lifetimes.md)): The computed interval fact `prod in [50, 200]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition** ([machines](../../../../wiki/spec/language/machines.md), [evaluation](../../../../wiki/spec/language/evaluation.md), [patterns](../../../../wiki/spec/language/patterns.md)): In each place, the synthesized interval `prod in [50, 200]` composes directly with `verify_prod` contract preconditions and subsequent exact subtractions `headroom = 200i32 - prod` and `margin = prod - 50i32` to statically discharge underflow safety and non-negativity without post-hoc caller equality guards.
- **Soundness boundaries** ([proof contracts](../../../../wiki/spec/proofs/contracts.md), [mathematical values](../../../../wiki/spec/terminal-psi/mathematical_values.md)): In every reject twin, widening the input bound to `lhs <= 21i32` permits `prod` to reach `210i32`, which fails closed under contract requires preconditions or ensures postconditions across boundary transmissions, verifying that out-of-interval values cannot satisfy safety bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)** ([product](../../../../wiki/spec/terminal-psi/product.md), [encoding](../../../../wiki/spec/terminal-psi/encoding.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)): Wrapping multiplication in the Wrapping domain lowers to target-neutral Terminal Psi `WrappingIntegerMultiply` (`PSITERM\0`) alongside integer conversion instructions, while proof annotations are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** ([provider selection](../../../../wiki/spec/build/provider_selection.md), [layout plans](../../../../wiki/spec/layouts/plans.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)): Provider layout plans represent 4-byte `i32` in its stored width (`IntegerAt 4-byte`) across all four hosted ABI calling conventions (CL on `windows_x86_64`, DIL on `linux_x86_64`, W0 on `macos_arm64` and `linux_arm64`), returning 32-bit signed results in EAX / W0.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** ([storage](../../../../wiki/spec/resources/storage.md), [activation storage](../../../../wiki/spec/resources/activation_storage.md), [machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md)): Backend lowering selects target ISA 32-bit modular multiplication instructions: `imul eax, ecx` on `x86_64` and `mul w0, w0, w1` on `arm64`, followed by exact subtractions `sub ...`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** ([machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md), [executable installation](../../../../wiki/spec/build/executable_installation.md)): Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
