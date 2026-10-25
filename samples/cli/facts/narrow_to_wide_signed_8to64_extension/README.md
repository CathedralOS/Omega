# Narrow to Wide Signed 8-to-64 Extension

Sign-extending a bounded 8-bit signed `i8` value (`narrow >= 30i8 && narrow <= 100i8`) to `i64` (`narrow as i64`) preserves the exact interval (`wide >= 30i64 && wide <= 100i64`), discharging exact 64-bit signed subtractions `100i64 - wide` and `wide - 30i64` across all nine places.

Every place performs the same operation: define bounded signed 8-bit input `narrow` (`30..100`), perform narrow-to-wide sign extension `wide: i64 = narrow as i64`, transport the widened 64-bit value through the place, pass `wide` to `verify_wide(wide: i64)` which requires `wide >= 30 && wide <= 100`, execute exact 64-bit subtractions `headroom = 100 - wide` and `margin = wide - 30`, compute `(headroom - margin - 10) as i32 == 0i32`, and verify in `main` that `res == 0` exiting 0. Only the transport mechanism of the widened value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-bit to 64-bit signed sign extension evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both the narrow i8 operand and sign-extended i64 value are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the sign-extended i64 value is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the sign-extended i64 value is forwarded as a machine call parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the sign-extended i64 value is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the sign-extended i64 value so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant i8 value is sign-extended to i64 |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the sign-extended i64 value to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the sign-extended i64 value through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 8-bit to 64-bit sign extension and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/narrow_to_wide_signed_8to64_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to64_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to64_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to64_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to64_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to64_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to64_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to64_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to64_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to64_extension/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `narrow as i64` with input `60i8` produces `60i64` within static interval `30i64..100i64`. Passing `wide` to `verify_wide(wide)` satisfies its contract `wide >= 30 && wide <= 100`. `verify_wide` evaluates exact subtractions `headroom = 100 - 60 == 40i64` and `margin = 60 - 30 == 30i64`, computes `diff = headroom - margin == 10i64`, and returns `(diff - 10) as i32 == 0i32`. In `main`, `transition res == 0` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition res == 0` to `transition res == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin widens the `narrow` bound (e.g. `narrow <= 101i8` or `const NARROW_CONST: i8 = 101;`), so `narrow as i64` reaches `101i64 > 100i64`. Calling `verify_wide(wide)` fails closed at compile time under the **Contract precondition** obligation class with `cannot prove requires contract for call verify_wide`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/narrow_to_wide_signed_8to64_extension/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\cargo-target\debug\omega.exe --filter cli/facts/narrow_to_wide_signed_8to64_extension --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_wide`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_local_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_field_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_parameter_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_call_result_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_match_join_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_const_name_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_other_state_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_other_machine_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to64_extension_through_local_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to64_extension_through_field_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to64_extension_through_parameter_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to64_extension_through_call_result_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to64_extension_through_match_join_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to64_extension_through_const_name_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to64_extension_through_other_state_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to64_extension_through_other_machine_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to64_extension_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for narrow-to-wide 8-bit to 64-bit signed sign extension (`i8 as i64`, [numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)). Specifically, sign-extending a bounded 8-bit signed `i8` value (`narrow >= 30i8 && narrow <= 100i8`) to `i64` (`narrow as i64`) preserves the exact interval (`wide >= 30i64 && wide <= 100i64`), discharging downstream contract requirement `wide >= 30 && wide <= 100` and exact 64-bit signed subtractions `100i64 - wide` and `wide - 30i64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Narrow 8-bit signed integer preconditions (`requires narrow >= 30 && narrow <= 100`) establish that the input value is within `[30, 100]`. Performing narrow-to-wide sign extension `narrow as i64` losslessly widens the scalar representation into 64-bit signed space while preserving the mathematical interval `[30, 100]`, discharging downstream requirement `wide >= 30 && wide <= 100` and exact subtractions `100 - wide` and `wide - 30` without requiring any post-hoc caller equality guard (`transition x == 60`).
  - **Identity and lifetime:** The widened 64-bit integer value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The widened 64-bit signed interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_wide(wide)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`narrow <= 100` vs `narrow <= 101`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `narrow <= 101` is specified: the widened interval expands to `[30, 101]`, which exceeds the required domain `wide >= 30 && wide <= 100`, strictly rejecting with `cannot prove requires contract for call verify_wide`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Narrow-to-wide sign extension operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving lossless 8-bit to 64-bit sign-extension casts and 64-bit arithmetic certificates (`[-2^63, 2^63 - 1]`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 1-byte (`i8`) to 8-byte (`i64`) layout conversion validates (`size_of`: 1 -> 8, `align_of`: 1 -> 8), and target `CallPlan`/`StatePlan` assign 64-bit register lanes (`rcx`/`rdx`/`r8`/`r9` on Windows x64; `rdi`/`rsi`/`rdx` on System V; `x0`..`x7` on ARM64) and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA sign-extension instructions: `movsx r64, r/m8` on `x86_64` (e.g. `movsbq`) and `sxtb x, w` on `arm64`, assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA sign extension instructions encode into machine bytes (REX.W + `0F BE /r` on `x86_64`; `sxtb` on `arm64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Floating-point conversions from 8-bit integers (`i8 as f32`, `i8 as f64`), vector SIMD sign-extension instructions (`pmovsxbq`), and saturating integer conversions.
