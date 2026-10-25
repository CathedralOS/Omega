# Narrow to Wide Unsigned 8-to-32 Extension

Fact: zero-extending a bounded 8-bit unsigned u8 value (narrow >= 40u8 && narrow <= 160u8) to u32 (narrow as u32) preserves the exact interval (wide >= 40u32 && wide <= 160u32), discharging exact 32-bit subtractions 160u32 - wide and wide - 40u32 across all nine places.

Every place performs the same operation: zero-extend bounded 8-bit unsigned operand (`narrow in [40, 160]`) to 32-bit unsigned integer via `narrow as u32`, establishing the synthesized post-extension interval `wide in [40, 160]`, discharge both headroom subtraction `160u32 - wide` and margin subtraction `wide - 40u32`, verify `probe() == 0`, and exit 0. Only the transport mechanism of the operands and zero-extension computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-bit to 32-bit unsigned zero extension, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the zero-extended u32 value is held in a local variable before discharging subtraction bounds |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the zero-extended u32 value is carried in a record field before discharging subtraction bounds |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the zero-extended u32 value is passed as an explicit machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a function call returns the zero-extended u32 value before discharging subtraction bounds |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms produce the zero-extended u32 value joining into subtraction bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply the narrow u8 operand for zero-extension |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the zero-extended u32 value and discharges subtraction bounds |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine sends the zero-extended u32 value to the verifying machine to discharge bounds |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies performs u8-to-u32 zero-extension and discharges subtraction bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to32_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to32_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to32_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to32_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to32_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to32_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to32_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to32_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to32_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_8to32_extension/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `narrow == 100u8`, zero extension produces `wide == 100u32`, `headroom == 60u32`, `margin == 60u32`, returning `0i32`. The transition `probe() == 0 { true -> done() _ -> bad() }` routes to `machine done() {}`, exiting 0. A 1-token perturbation of the demanded result (`transition probe() == 0` to `transition probe() == 1` in `main.omg`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- In each invalid twin, `narrow` upper bound is widened to `161u8` so `wide` can reach `161u32 > 160u32`. Exact subtraction `160u32 - wide` statically detects potential underflow below zero, failing closed under the **Exact arithmetic overflow** obligation class with ``may overflow `u32` ``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/narrow_to_wide_unsigned_8to32_extension/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Exact arithmetic overflow | ``may overflow `u32` `` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Exact arithmetic overflow | ``may overflow `u32` `` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Exact arithmetic overflow | ``may overflow `u32` `` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Exact arithmetic overflow | ``may overflow `u32` `` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Exact arithmetic overflow | ``may overflow `u32` `` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Exact arithmetic overflow | ``may overflow `u32` `` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Exact arithmetic overflow | ``may overflow `u32` `` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Exact arithmetic overflow | ``may overflow `u32` `` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Exact arithmetic overflow | ``may overflow `u32` `` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/narrow_to_wide_unsigned_8to32_extension --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (``may overflow `u32` ``).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to32_extension_through_local_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to32_extension_through_field_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to32_extension_through_parameter_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to32_extension_through_call_result_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to32_extension_through_match_join_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to32_extension_through_const_name_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to32_extension_through_other_state_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to32_extension_through_other_machine_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_unsigned_8to32_extension_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to32_extension_through_local_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to32_extension_through_field_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to32_extension_through_parameter_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to32_extension_through_call_result_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to32_extension_through_match_join_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to32_extension_through_const_name_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to32_extension_through_other_state_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to32_extension_through_other_machine_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_unsigned_8to32_extension_through_target_body_rejected/`

The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `bef9038886664837877c72a64a8d10bc`.
- **Requirement established:** Target-neutral compile-time 8-bit to 32-bit unsigned zero-extension bounds preservation (`Psi 05–07`, [numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)) and native realization (`Omega 00–09`). Specifically, zero-extending a bounded 8-bit unsigned `u8` value (`narrow >= 40u8 && narrow <= 160u8`) to `u32` (`narrow as u32`) preserves the exact interval (`wide >= 40u32 && wide <= 160u32`), discharging exact 32-bit subtractions `160u32 - wide` and `wide - 40u32` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Zero-extending a bounded 8-bit unsigned `u8` value (`narrow >= 40u8 && narrow <= 160u8`) to 32-bit unsigned `u32` via `narrow as u32` preserves the numeric value and statically establishes the interval `[40, 160]` on the 32-bit destination. This interval directly discharges the exact unsigned 32-bit subtraction proof obligations for both headroom (`160u32 - wide >= 0`) and margin (`wide - 40u32 >= 0`). Widening the source interval (e.g. `narrow <= 161u8`, allowing `wide = 161u32`) causes `160u32 - wide` to potentially underflow, failing closed at compile time with ``may overflow `u32` ``.
  - **Identity and lifetime:** The zero-extended unsigned 32-bit integer scalar retains its identity and exact numerical interval across all storage carriers and place projections without loss, unwanted sign extension, or storage corruption.
  - **Composition:** The verified interval flows through local variables, record fields (`WideContainer`), machine parameters, call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies, discharging both `160u32 - wide` and `wide - 40u32` across all nine places without post-hoc caller guards.
  - **Soundness boundaries:** Exact integer arithmetic requires compile-time proof of non-underflow and non-overflow. In invalid twins, widening the source bound allows values outside `[40, 160]`, correctly triggering compile-time rejection under the **Exact arithmetic overflow** obligation class (``may overflow `u32` ``) rather than accepting runtime modular wrap-around.
  - **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives the unsigned zero-extension operation (`Cast` from `u8` to `u32`) and subsequent subtraction operations with interval bounds and arithmetic modes intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Zero extension converts a 1-byte unsigned scalar (`size_of = 1, align_of = 1`) into a 4-byte unsigned scalar (`size_of = 4, align_of = 4`). Standard calling conventions place 32-bit parameters in 32-bit register lanes (`ECX` on Windows `x86_64`, `EDI` on Linux `x86_64`, `W0` on `arm64`) and return 32-bit scalars in `EAX` / `W0`.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects `movzx r32, r/m8` (REX without W; build bytes `40 0F B6`; zero-extending an 8-bit register or byte memory operand into a 32-bit general-purpose register, implicitly zeroing bits 32..63). On `arm64`, backend lowering selects the 64-bit UBFM zero-extend of bits 0..7, which `otool` prints as `ubfx xN, xN, #0, #8` (the `uxtb` alias with an `X` destination), not a 32-bit `uxtb w, w`. Subsequent subtractions run on the 64-bit carrier (`subq` / `sub xN`).
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.

- **What remains open:** Vector zero-extension across SIMD lanes and checked narrowing conversions with dynamic saturation.
