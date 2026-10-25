# Narrow to Wide Signed 16-to-64 Extension

Fact: sign-extending a bounded 16-bit signed i16 value (narrow >= 300i16 && narrow <= 500i16) to i64 (narrow as i64) preserves the exact interval (wide >= 300i64 && wide <= 500i64), discharging exact 64-bit signed subtractions 500i64 - wide and wide - 300i64 across all nine places.

Every place performs the same operation: sign-extend bounded 16-bit signed operand (`narrow in [300, 500]`) to 64-bit signed integer via `narrow as i64`, establishing the synthesized post-extension interval `wide in [300, 500]`, discharge both headroom subtraction `500i64 - wide` and margin subtraction `wide - 300i64`, verify `headroom + margin == 200i64`, and exit 0. Only the transport mechanism of the operands and sign-extension computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 16-bit to 64-bit signed sign extension, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 16-bit signed operand is sign-extended into a local binding before verification |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the sign-extended 64-bit signed value is stored in a record field before verification |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | machine parameter receives the sign-extended 64-bit value before verification |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | callee function sign-extends the bounded 16-bit signed operand and returns the 64-bit result before verification |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 16-bit values and join before sign-extending and verification |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant supplies bounded 16-bit signed operand before sign extension and verification |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor machine receives sign-extended 64-bit value before verification |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes the sign-extended 64-bit value to verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies sign-extends bounded 16-bit signed operand and verifies |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/narrow_to_wide_signed_16to64_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to64_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to64_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to64_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to64_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to64_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to64_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to64_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to64_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to64_extension/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `narrow == 400i16`, sign extension produces `wide == 400i64`, `headroom == 100i64`, `margin == 100i64`, and `headroom + margin == 200i64`. The check `sum - 200i64` evaluates to `0i64`, returning `0i32`. The transition `res == 0` / `probe() == 0` routes to `machine done() {}`, exiting 0. A 1-token perturbation of the demanded result (`transition res == 0` to `transition res == 1` in `main.omg`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- In each invalid twin, `narrow` upper bound is widened to `501i16` so `wide` can reach `501i64 > 500i64`. Downstream verification contract checking statically detects that `wide <= 500` cannot be proved, failing closed under the **Contract precondition** obligation class with `cannot prove requires contract for call`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/narrow_to_wide_signed_16to64_extension/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/narrow_to_wide_signed_16to64_extension --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_local_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_field_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_parameter_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_call_result_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_match_join_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_const_name_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_other_state_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_other_machine_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to64_extension_through_local_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to64_extension_through_field_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to64_extension_through_parameter_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to64_extension_through_call_result_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to64_extension_through_match_join_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to64_extension_through_const_name_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to64_extension_through_other_state_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to64_extension_through_other_machine_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to64_extension_through_target_body_rejected/`

The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `09a5b8efcb7c42b685d6175251542e3a`.
- **Requirement established:** Target-neutral compile-time 16-bit to 64-bit signed sign extension bounds preservation (`Psi 05–07`, [numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)) and native realization (`Omega 00–09`). Specifically, sign extension (`narrow as i64`) on bounded 16-bit signed `i16` values (`narrow >= 300i16 && narrow <= 500i16`) establishes and preserves the exact post-extension interval (`wide >= 300i64 && wide <= 500i64`), discharging exact 64-bit signed subtractions `500i64 - wide` and `wide - 300i64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis tracks signed integer value intervals across cast and sign-extension expressions (`narrow as i64`). Bounded signed input `narrow in [300, 500]` maps losslessly to `wide in [300, 500]`, establishing `wide >= 300i64 && wide <= 500i64` unconditionally. This discharges the precondition contract of `verify_wide` and proves exact signed subtractions `500i64 - wide` and `wide - 300i64` safe from arithmetic overflow without post-hoc caller guards.
  - **Identity and lifetime:** The 16-bit signed operand and 64-bit signed sign-extended scalar interval bounds retain their precision, two's complement sign preservation, and value identity across copies into local variables, record fields (`WideBox.val`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 16-bit to 64-bit sign extension composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming verification operation, evaluating to `0i32` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`narrow <= 500i16` vs `narrow <= 501i16`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `narrow <= 501i16`: interval tracking establishes `wide in [300, 501]`, so precondition `wide <= 500` on the consuming call fails proof. Compilation strictly rejects under the **Contract precondition** obligation class with `cannot prove requires contract for call`. No unproved out-of-contract call reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral sign-extension from `i16` to `i64` lowers into canonical target-neutral Terminal Psi (`PSITERM\0`) sign-extension instructions, erasing mathematical proof obligations while certifying signed integer arithmetic modes. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 2-byte stored width and 2-byte alignment for `i16` convert to 8-byte stored width and 8-byte alignment for `i64` (and `WideBox` size 8, align 8), validating against target layout plans. Target `CallPlan`/`StatePlan` assign 64-bit arguments and return values to 64-bit integer registers (`rax`/`rcx`/`rdx` on `x86_64`, `x0` on `arm64`).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA sign extension instructions: `movsx r64, r/m16` on `x86_64` and `sxth x, w` (signed extend halfword) on `arm64`, register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions (`movsx`/`sxth`) and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Vector sign extension across SIMD lanes and checked narrowing conversions with dynamic saturation.
