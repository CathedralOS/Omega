# Narrow-to-Wide Unsigned 16-to-32 Extension Bound

Fact: zero-extending a bounded 16-bit unsigned u16 value (narrow >= 300u16 && narrow <= 500u16) to u32 (narrow as u32) preserves the exact interval (wide >= 300u32 && wide <= 500u32), discharging exact 32-bit subtractions 500u32 - wide and wide - 300u32 across all nine places.

The original root reproduction is preserved in [`main.omg`](main.omg) and [`build.omg`](build.omg).

## Places

The nine places carry the 16-bit to 32-bit unsigned zero extension bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

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
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_target_body_exit` | `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_target_body_rejected` | widened result verified across target-scoped machine bodies |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/narrow_to_wide_unsigned_16to32_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_16to32_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_16to32_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_16to32_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_16to32_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_16to32_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_16to32_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_16to32_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_16to32_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_16to32_extension/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded `0` exit is strictly discriminative:
- At runtime, with `narrow == 400u16`, zero extension produces `wide == 400u32`, `upper_headroom == 500u32 - 400u32 == 100u32`, `lower_headroom == 400u32 - 300u32 == 100u32`, and `diff == 100i32 - 100i32 == 0i32`. The transition `check == 0` routes to `machine done() {}`, exiting 0. A 1-token perturbation of the demanded value (`transition check == 0` to `transition check == 1` in `main.omg`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each place has an `invalid/` twin (`sample.json` + `expected.txt`) that breaks only the construct's own contract, input, or domain (never a post-hoc caller guard):
  - **Obligation class (`local`, `field`, `parameter`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`):** Call contract precondition (`cannot prove requires contract`) when `narrow` upper bound is widened to `501u16` (or `const NARROW: u16 = 501`) so `wide` can reach `501u32 > 500u32`, violating `requires wide >= 300 && wide <= 500` on `verify_wide`.
  - **Obligation class (`call_result`):** Call contract postcondition (`cannot prove ensures contract for produce_wide`) when `produce_wide` widens `narrow <= 501u16` while declaring `ensures result >= 300 && result <= 500`. Proof obligation checking fails closed at compile time and none reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/narrow_to_wide_unsigned_16to32_extension/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract postcondition | `cannot prove ensures contract for produce_wide` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_local_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_field_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_parameter_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_call_result_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_match_join_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_const_name_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_other_state_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_other_machine_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_16to32_extension_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_local_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_field_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_parameter_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_call_result_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_match_join_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_const_name_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_other_state_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_other_machine_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_16to32_extension_through_target_body_rejected/`

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/narrow_to_wide_unsigned_16to32_extension --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract`).

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Ticket:** `13caab91430143a982392c2b5f2eed6c`.
- **Requirement established:** 16-bit to 32-bit unsigned integer zero-extension bounds proof and realization (`Psi 05–07` / `Omega 00–09`, `[numeric values](wiki/spec/language/numeric_values.md)` & `[integer certificates](wiki/spec/terminal-psi/integer_certificates.md)`), verifying that zero-extending a bounded `u16` scalar (`narrow >= 300u16 && narrow <= 500u16`) via `narrow as u32` preserves the interval `wide >= 300u32 && wide <= 500u32`, which proves that exact 32-bit unsigned subtractions `500u32 - wide` and `wide - 300u32` are provably underflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 16-to-32 unsigned zero extension `narrow as u32` on `narrow: u16` with entry contract `requires narrow >= 300 && narrow <= 500` statically establishes the postcondition interval `[300, 500]` in `u32`. The compiler statically models the cast as lossless unsigned widening into 32-bit unsigned range `[0, 4294967295]`, proving that `wide >= 300 && wide <= 500` holds. The consuming helper `verify_wide(wide: u32) -> i32 requires wide >= 300 && wide <= 500` discharges exact unsigned subtractions `500 - wide` (evaluating in `[0, 200]`) and `wide - 300` (evaluating in `[0, 200]`) without underflow or truncation.
- **Identity and lifetime:** The widened 32-bit unsigned scalar preserves its invariant `[300, 500]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The exact widening bound composes across all 9 standard places to satisfy the consuming requirement `wide >= 300 && wide <= 500` and discharge `500 - wide` and `wide - 300`:
  - `local`: widened result is bound to a local variable `local_wide` before verification.
  - `field`: widened result is stored in `ExtCarrier.wide` record field and projected out.
  - `parameter`: widened result is passed to machine parameter `check_param(val: u32)`.
  - `call_result`: helper machine `produce_wide()` widens and returns the result across call boundary with ensures contract.
  - `match_join`: widened result converges through match arms into a unified value.
  - `const_name`: input `NARROW` is declared as a compile-time named constant `const NARROW: u16 = 400;`.
  - `other_state`: widened result is passed across a transition edge into successor machine `step(forwarded: u32)`.
  - `other_machine`: widened result is forwarded through intermediate machine `runner()`.
  - `target_body`: widened result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening the `narrow` upper bound to `narrow >= 300 && narrow <= 501` (where `wide` reaches `501u32 > 500u32`), proving that downstream contract verification `requires wide >= 300 && wide <= 500` strictly fails closed when the post-widening bound exceeds 500.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) under the **Contract precondition** obligation class with `cannot prove requires contract`, confirming that the proof engine soundly detects that when `wide` can be 501, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. Unsigned widening casts are lowered with verified lossless widening certificates, preserving interval bounds and erasing proof tags. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 2-byte `u16` to 4-byte `u32` scalar layout conversion and alignment, while calling plans assign 32-bit register homes (`eax`/`edi`/`esi` on x86_64, `w0`/`w1` on arm64) for passing values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA zero-extension instructions: `movzx r32, r/m16` on `x86_64` and `uxth w, w` on `arm64`. Unsigned subtraction lowers to 32-bit `sub`, and register allocation assigns register homes and disjoint stack activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime sign/zero extension of arbitrary bitfield slices, 64-bit to 128-bit unsigned widening, and SIMD zero-extension unpack instructions.
