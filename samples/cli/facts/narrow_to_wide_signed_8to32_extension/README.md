# Fact: 8-Bit to 32-Bit Signed Extension Interval Preservation

> sign-extending a bounded 8-bit signed i8 value (narrow >= 30i8 && narrow <= 100i8) to i32 (narrow as i32) preserves the exact interval (wide >= 30i32 && wide <= 100i32), discharging exact 32-bit signed subtractions 100i32 - wide and wide - 30i32 across all nine places.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 8-bit to 32-bit signed extension interval preservation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | extended wide scalar is copied into intermediate local bindings before interval subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | narrow input is stored in and projected from a nominal record field (`Stored`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | narrow operand is operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call returns extended wide value with postcondition contract `ensures result >= 30i32 && result <= 100i32` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | wide value is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared symbolic constants define narrow and wide bounds |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor machine `check_state(w: i32)` receives extended wide value and verifies interval subtractions |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the wide value to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform sign extension and interval subtractions |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Pinned fixtures

The nine positive places and nine reject twins are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to32_extension_through_local_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to32_extension_through_field_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to32_extension_through_parameter_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to32_extension_through_call_result_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to32_extension_through_match_join_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to32_extension_through_const_name_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to32_extension_through_other_state_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to32_extension_through_other_machine_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to32_extension_through_target_body_exit/`
- **Fail fixtures (`expected.txt` = `not provably representable`):**
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to32_extension_through_local_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to32_extension_through_field_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to32_extension_through_parameter_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to32_extension_through_call_result_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to32_extension_through_match_join_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to32_extension_through_const_name_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to32_extension_through_other_state_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to32_extension_through_other_machine_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to32_extension_through_target_body_rejected/`

`tests/omega/corpus_native_windows_x86_64.txt` records nine real `built ... exit:0` rows, one per place, and `tests/omega/corpus_outcomes.txt` records all 18 pass and fail outcomes.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/narrow_to_wide_signed_8to32_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to32_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to32_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to32_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to32_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to32_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to32_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to32_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to32_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to32_extension/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operand is `narrow = 60i8` (bounded `[30i8, 100i8]`).
- Sign-extending to `i32` (`let wide: i32 = narrow as i32`) statically preserves the exact value interval `[30i32, 100i32]`.
- Consuming exact subtractions `let headroom: i32 = 100i32 - wide` (`40i32`) and `let margin: i32 = wide - 30i32` (`30i32`) evaluate to non-negative quantities bounded in `[0i32, 70i32]`.
- Exact unsigned casts `let check_h: u32 = headroom as u32` and `let check_m: u32 = margin as u32` verify non-negative headroom/margin bounds without underflow or overflow.
- At runtime, `wide == 60i32 && headroom + margin == 70i32 && check_h + check_m == 70u32` confirms exact arithmetic agreement, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy. A 1-token perturbation of the demanded result in `main` (`transition res == 0` to `transition res == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin widens the `narrow` bound to `narrow <= 101i8`, allowing `wide` to take value `101i32 > 100i32`. The headroom subtraction `100i32 - wide` yields `-1i32`, which fails closed at compile time under the **Exact integer cast representability** obligation class during exact conversion to `u32` with diagnostic `Exact integer cast ... from \`i32\` to \`u32\` is not provably representable`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/narrow_to_wide_signed_8to32_extension/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Exact integer cast representability | `not provably representable` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Exact integer cast representability | `not provably representable` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Exact integer cast representability | `not provably representable` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Exact integer cast representability | `not provably representable` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Exact integer cast representability | `not provably representable` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Exact integer cast representability | `not provably representable` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Exact integer cast representability | `not provably representable` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Exact integer cast representability | `not provably representable` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Exact integer cast representability | `not provably representable` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/narrow_to_wide_signed_8to32_extension --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`not provably representable`).

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 8-bit to 32-bit signed sign extension (`i8 as i32`), interval preservation (`[30, 100]`), consuming exact 32-bit subtractions, non-negative range verification, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded entry contract `requires narrow >= 30i8 && narrow <= 100i8` establishes strict signed interval bounds. Signed widening cast `narrow as i32` preserves the interval `[30, 100]` in the target 32-bit domain without distortion, directly discharging consuming exact subtractions `100i32 - wide` and `wide - 30i32` and their non-negative representability without runtime checks or post-hoc equality guards.
  - **Identity and lifetime:** The extended `i32` scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal record fields `Stored`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval `wide >= 30i32 && wide <= 100i32` is preserved intact.
  - **Composition:** Signed extension composes through calling conventions and control flow. In `call_result`, `widen` formally proves postcondition `ensures result >= 30i32 && result <= 100i32`; in `match_join`, both arms establish identical wide intervals across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the `narrow` bound is widened to `narrow <= 101i8`. Because the wide interval `[30, 101]` admits `wide = 101i32`, `let headroom: i32 = 100i32 - wide;` yields `-1i32`, which fails closed with diagnostic `Exact integer cast in machine ... from \`i32\` to \`u32\` is not provably representable`. No negative headroom escapes into unsigned domains.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Signed widening lowers into target-neutral Terminal Psi (`PSITERM\0`) lossless integer conversions (`SignExtend { from: 8, to: 32 }`). Interval certificates are preserved and validated in Terminal Psi semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Layout geometry validates transformation from 1-byte stored width for `i8` (`size_of = 1`, `align_of = 1`) to 4-byte stored width for `i32` (`size_of = 4`, `align_of = 4`). Calling plans allocate standard ABI register homes (`ecx` on Windows x86_64, `edi` on Linux x86_64, `w0` on AArch64).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects the destination-width-independent `SignExtendI8`, which writes the full 64-bit normalized carrier: on x86_64, `movsx r64, r/m8` (REX.W, e.g. `movsx rax, cl`; AT&T `movsbq`), sign-extending the sign bit of the 8-bit input through bit 63; on AArch64, `sxtb xN, wN`. The `i32` destination adds only an ordinary `sxtw` carrier normalization at 32-bit boundaries; the extension itself is keyed on the source width. Register allocation assigns disjoint register homes with bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- 16-bit to 64-bit and 32-bit to 64-bit signed sign extension (`i16 as i64`, `i32 as i64`).
- Negative-domain sign extension interval verification (e.g. `narrow in [-100i8, -30i8]`).
- Mixed signed/unsigned widening conversions with signedness changes.
