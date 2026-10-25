# Narrow to wide unsigned 8-bit to 16-bit extension

Fact: zero-extending a bounded 8-bit unsigned u8 value (narrow >= 40u8 && narrow <= 160u8) to u16 (narrow as u16) preserves the exact interval (wide >= 40u16 && wide <= 160u16), discharging exact 16-bit subtractions 160u16 - wide and wide - 40u16 across all nine places.

Owning Task: `NUMERIC-SCALARS` / `CONSERVATION-CONTRACT` (`TASKS.md`)

Every positive place and reject twin tests unsigned zero extension from 8-bit `u8` to 16-bit `u16` (`narrow as u16`).
When the 8-bit input is bounded to interval `narrow >= 40u8 && narrow <= 160u8`, zero-extending to `u16` statically preserves the interval `wide >= 40u16 && wide <= 160u16`.
Subsequent exact 16-bit subtractions `160u16 - wide` and `wide - 40u16` are proved underflow-free without requiring any post-hoc caller equality guard before the operations, evaluating to `60u16 + 60u16 = 120u16` for `narrow == 100u8` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with bounded u8 zero-extended to u16 and dual margin subtractions |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | zero-extended 16-bit value is copied into a local binding before exact subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | zero-extended 16-bit value is stored into and read from a record field before exact subtractions |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | zero-extended 16-bit value is passed as an explicit machine parameter before exact subtractions |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | zero-extended 16-bit value is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the zero-extended 16-bit value |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply upper and lower ceilings for exact subtractions |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the zero-extended 16-bit value into a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the zero-extended 16-bit value |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing the zero extension and subtractions |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to16_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to16_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to16_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to16_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to16_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to16_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to16_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to16_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to16_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to16_extension/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(100u8)` computes `wide = 100u8 as u16 = 100u16`.
Because `narrow in [40, 160]`, zero extension statically proves `wide in [40, 160]`.
Subsequent exact subtractions `headroom = 160u16 - wide` (`60u16`) and `margin = wide - 40u16` (`60u16`) are proved underflow-free, and their sum `headroom + margin` evaluates to `120u16`.
The transition guard `combined == 120u16` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
A 1-token perturbation of the demanded result (`transition combined == 120u16` to `transition combined == 121u16`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens the narrow input precondition bound from `narrow <= 160u8` to `narrow <= 161u8`.
When `narrow == 161u8`, `wide` reaches `161u16 > 160u16`, which would make `160u16 - wide` underflow `u16` (potential underflow wrap to `65535`).
The compiler strictly fails closed at compile time:

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| `local` | [`local/invalid/main.omg`](local/invalid/main.omg) | Exact arithmetic | `exact arithmetic in machine \`probe\` state \`entry\` local \`headroom\` may overflow \`u16\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation).` |
| `field` | [`field/invalid/main.omg`](field/invalid/main.omg) | Exact arithmetic | `exact arithmetic in machine \`probe\` state \`entry\` local \`headroom\` may overflow \`u16\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation).` |
| `parameter` | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call consume_wide from probe: wide >= 40 && wide <= 160` |
| `call_result` | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract postcondition | `cannot prove ensures contract for exit from extend_u8 at statement 0: result >= 40 && result <= 160` |
| `match_join` | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call choose from probe: wide >= 40 && wide <= 160 && 100 >= 40 && 100 <= 160` |
| `const_name` | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Exact arithmetic | `exact arithmetic in machine \`probe\` state \`entry\` local \`headroom\` may overflow \`u16\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation).` |
| `other_state` | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call consume from probe: wide >= 40 && wide <= 160` |
| `other_machine` | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call forward_wide from probe: wide >= 40 && wide <= 160` |
| `target_body` | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Exact arithmetic | `exact arithmetic in machine \`target_probe\` state \`entry\` local \`headroom\` may overflow \`u16\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation).` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/narrow_to_wide_unsigned_8to16_extension --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection.

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to16_extension_through_local_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to16_extension_through_field_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to16_extension_through_parameter_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to16_extension_through_call_result_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to16_extension_through_match_join_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to16_extension_through_const_name_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to16_extension_through_other_state_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to16_extension_through_other_machine_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to16_extension_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to16_extension_through_local_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to16_extension_through_field_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to16_extension_through_parameter_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to16_extension_through_call_result_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to16_extension_through_match_join_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to16_extension_through_const_name_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to16_extension_through_other_state_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to16_extension_through_other_machine_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to16_extension_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence (AGENTS.md)

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `9e9d37a107f04452974f5ea20f75ce6c`.

- **Establishment**: Caller contract precondition `narrow >= 40u8 && narrow <= 160u8` establishes a bounded 8-bit unsigned interval. Unsigned widening cast `narrow as u16` preserves the interval `[40, 160]` directly into the 16-bit integer domain without loss of bounds or post-hoc caller equality guards.
- **Identity and lifetime**: The computed interval fact `wide in [40, 160]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `wide in [40, 160]` composes directly with 16-bit unsigned exact subtractions `headroom = 160u16 - wide` and `margin = wide - 40u16` to statically discharge underflow safety, proving `headroom in [0, 120]` and `margin in [0, 120]` without any preceding post-hoc caller equality guard.
- **Soundness boundaries**: In every reject twin, widening the input bound to `narrow <= 161u8` permits `wide` to reach `161u16`, causing `160u16 - wide` to potentially underflow below zero and triggering compile-time rejection under decision 17 (or contract violation on boundary transmission), verifying that underflow-unsafe paths cannot pass unchecked.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Unsigned widening cast `as u16` and exact subtraction operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`) integer conversion and arithmetic representations (`IntegerMode::Exact`) accompanied by integer certificates, while proof annotations are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent 1-byte `u8` and 2-byte `u16` in their respective stored widths (`IntegerAt 1-byte` and `IntegerAt 2-byte`) across all four hosted ABI calling conventions (CL on `windows_x86_64`, DIL on `linux_x86_64`, W0 on `macos_arm64` and `linux_arm64`), returning 16-bit unsigned results in AX / W0.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects `ZeroExtendU8`. On `x86_64` that is `movzx r32, r/m8` (REX without W; build bytes `40 0F B6`), where the 32-bit destination implicitly zeroes bits 32..63. On `arm64` that is the 64-bit UBFM zero-extend of bits 0..7, which `otool` prints as `ubfx xN, xN, #0, #8` (the `uxtb` alias with an `X` destination), not `uxtb w, w`. Exact subtraction then runs on the full 64-bit carrier (`subq` / `sub xN`), not a 16-bit `sub ax` / `sub w0`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
