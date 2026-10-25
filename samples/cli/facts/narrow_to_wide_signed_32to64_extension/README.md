# Narrow to Wide Signed 32-to-64 Extension

Fact: sign-extending a bounded 32-bit signed i32 value (narrow >= 70000i32 && narrow <= 90000i32) to i64 (narrow as i64) preserves the exact interval (wide >= 70000i64 && wide <= 90000i64), discharging exact 64-bit signed subtractions 90000i64 - wide and wide - 70000i64 across all nine places.

Every place performs the same operation: sign-extend bounded 32-bit signed operand (`narrow in [70000, 90000]`) to 64-bit signed integer via `narrow as i64`, establishing the synthesized post-extension interval `wide in [70000, 90000]`, discharge both headroom subtraction `90000i64 - wide` and margin subtraction `wide - 70000i64`, verify `headroom + margin == 20000i64`, and exit 0. Only the transport mechanism of the operands and sign-extension computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Pinned Pass Fixture | Pinned Fail Fixture | Transport Mechanism |
| --- | --- | --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | — | — | root 32-bit to 64-bit signed sign extension, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_signed_32to64_extension_through_local` | `tests/omega/fail/facts/narrow_to_wide_signed_32to64_extension_unproved_at_local` | the 32-bit signed operand is sign-extended into a local binding before verification |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_signed_32to64_extension_through_field` | `tests/omega/fail/facts/narrow_to_wide_signed_32to64_extension_unproved_at_field` | the sign-extended 64-bit signed value is stored in a record field before verification |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_signed_32to64_extension_through_parameter` | `tests/omega/fail/facts/narrow_to_wide_signed_32to64_extension_unproved_at_parameter` | machine parameter receives the sign-extended 64-bit value before verification |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_signed_32to64_extension_through_call_result` | `tests/omega/fail/facts/narrow_to_wide_signed_32to64_extension_unproved_at_call_result` | callee function sign-extends the bounded 32-bit signed operand and returns the 64-bit result before verification |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_signed_32to64_extension_through_match_join` | `tests/omega/fail/facts/narrow_to_wide_signed_32to64_extension_unproved_at_match_join` | both match arms yield bounded 32-bit values and join before sign-extending and verification |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_signed_32to64_extension_through_const_name` | `tests/omega/fail/facts/narrow_to_wide_signed_32to64_extension_unproved_at_const_name` | named constant supplies bounded 32-bit signed operand before sign extension and verification |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_signed_32to64_extension_through_other_state` | `tests/omega/fail/facts/narrow_to_wide_signed_32to64_extension_unproved_at_other_state` | successor machine receives sign-extended 64-bit value before verification |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_signed_32to64_extension_through_other_machine` | `tests/omega/fail/facts/narrow_to_wide_signed_32to64_extension_unproved_at_other_machine` | an intermediate forwarding machine receives and passes the sign-extended 64-bit value to verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/narrow_to_wide_signed_32to64_extension_through_target_body` | `tests/omega/fail/facts/narrow_to_wide_signed_32to64_extension_unproved_at_target_body` | each of four target-scoped machine bodies sign-extends bounded 32-bit signed operand and verifies |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/narrow_to_wide_signed_32to64_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_32to64_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_32to64_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_32to64_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_32to64_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_32to64_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_32to64_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_32to64_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_32to64_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_32to64_extension/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `narrow == 80000i32`, sign extension produces `wide == 80000i64`, `headroom == 10000i64`, `margin == 10000i64`, and `headroom + margin == 20000i64`. The check `sum - 20000i64` evaluates to `0i64`, returning `0i32`. The transition `probe() == 0 { true -> done() _ -> bad() }` exits 0. If sign extension or exact subtractions evaluate incorrectly, execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `narrow` upper bound is widened to `90001i32` so `wide` can reach `90001i64 > 90000i64`. Downstream verification contract checking statically detects that `wide <= 90000` cannot be proved, failing closed with `cannot prove requires contract for call`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/narrow_to_wide_signed_32to64_extension/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\cargo-target\debug\omega-latest.exe --filter cli/facts/narrow_to_wide_signed_32to64_extension --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** Target-neutral compile-time 32-bit to 64-bit signed sign extension bounds preservation (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, sign extension (`narrow as i64`) on bounded 32-bit signed `i32` values (`narrow >= 70000i32 && narrow <= 90000i32`) establishes and preserves the exact post-extension interval (`wide >= 70000i64 && wide <= 90000i64`), discharging exact 64-bit signed subtractions `90000i64 - wide` and `wide - 70000i64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis tracks signed integer value intervals across cast and sign-extension expressions (`narrow as i64`). Bounded signed input `narrow in [70000, 90000]` maps losslessly to `wide in [70000, 90000]`, establishing `wide >= 70000i64 && wide <= 90000i64` unconditionally. This discharges the precondition contract of `verify_wide` and proves exact signed subtractions `90000i64 - wide` and `wide - 70000i64` safe from arithmetic overflow without post-hoc caller guards.
  - **Identity and lifetime:** The 32-bit signed operand and 64-bit signed sign-extended scalar interval bounds retain their precision, two's complement sign preservation, and value identity across copies into local variables, record fields (`WideBox.val`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 32-bit to 64-bit sign extension composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming verification operation, evaluating to `0i32` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`narrow <= 90000i32` vs `narrow <= 90001i32`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `narrow <= 90001i32`: interval tracking establishes `wide in [70000, 90001]`, so precondition `wide <= 90000` on the consuming call fails proof. Compilation strictly rejects with `cannot prove requires contract for call`. No unproved out-of-contract call reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral sign-extension from `i32` to `i64` lowers into canonical target-neutral Terminal Psi (`PSITERM\0`) sign-extension instructions, erasing mathematical proof obligations while certifying signed integer arithmetic modes. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 4-byte stored width and 4-byte alignment for `i32` convert to 8-byte stored width and 8-byte alignment for `i64` (and `WideBox` size 8, align 8), validating against target layout plans. Target `CallPlan`/`StatePlan` assign 64-bit arguments and return values to 64-bit integer registers (`rax`/`rcx`/`rdx` on `x86_64`, `x0` on `arm64`).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA sign extension instructions: `movsxd r64, r/m32` on `x86_64` and `sxtw x, w` (signed extend word) on `arm64`, register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions (`movsxd`/`sxtw`) and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Vector sign extension across SIMD lanes and checked narrowing conversions with dynamic saturation.
