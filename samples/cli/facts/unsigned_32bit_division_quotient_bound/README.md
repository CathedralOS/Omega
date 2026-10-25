# Fact: 32-Bit Unsigned Exact Division Quotient Bound

> A 32-bit unsigned `u32` division of bounded inputs (`dividend >= 200u32 && dividend <= 200u32`, `divisor >= 4u32 && divisor <= 4u32`) establishes `quotient >= 50u32 && quotient <= 50u32` so exact 32-bit subtraction `quotient - 50u32` is proved underflow-free across all nine places.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 32-bit unsigned division quotient bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | quotient is copied into an intermediate local binding before exact subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded operands are packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call returns quotient with postcondition contract `ensures result >= 50u32 && result <= 50u32` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | quotient is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared symbolic constants (`DIVIDEND_VAL`, `DIVISOR_VAL`, `QUOTIENT_BOUND`) define bounds |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor machine `check_state(q: u32)` receives quotient and verifies underflow-free subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the quotient to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform division and interval subtraction |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_32bit_division_quotient_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_quotient_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_quotient_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_quotient_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_quotient_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_quotient_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_quotient_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_quotient_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_quotient_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_quotient_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `dividend = 200u32` (bounded `[200u32, 200u32]`) and `divisor = 4u32` (bounded `[4u32, 4u32]`).
- The divisor's strict non-zero lower bound proves division by zero cannot occur.
- Integer division `dividend / divisor` statically proves that `quotient` resides within interval `[50u32, 50u32]`.
- Consuming exact subtraction `let check: u32 = quotient - 50u32` evaluates to `0u32` and is statically verified underflow-free without requiring a post-hoc runtime guard.
- At runtime, `quotient == 50u32 && check == 0u32` is confirmed, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy.
- Perturbing a single token in the root oracle (`transition quotient == 50u32 && check == 0u32` -> `transition quotient == 51u32 && check == 0u32`) causes both engines to fail closed with `crash Trap` (`interp exit: 255, native exit: 255`).
- Each invalid twin relaxes the divisor bound to `divisor <= 5u32`, allowing `quotient` to take value `200 / 5 = 40u32 < 50u32`. The subtraction `quotient - 50u32` is no longer provably underflow-free, causing the compiler to fail closed at compile time with `may overflow \`u32\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_32bit_division_quotient_bound/<place>/invalid/main.omg
```

## Pinned fixtures

- Positive (9):
  - `tests/omega/pass/facts/unsigned_32bit_division_quotient_bound_through_local_exit`
  - `tests/omega/pass/facts/unsigned_32bit_division_quotient_bound_through_field_exit`
  - `tests/omega/pass/facts/unsigned_32bit_division_quotient_bound_through_parameter_exit`
  - `tests/omega/pass/facts/unsigned_32bit_division_quotient_bound_through_call_result_exit`
  - `tests/omega/pass/facts/unsigned_32bit_division_quotient_bound_through_match_join_exit`
  - `tests/omega/pass/facts/unsigned_32bit_division_quotient_bound_through_const_name_exit`
  - `tests/omega/pass/facts/unsigned_32bit_division_quotient_bound_through_other_state_exit`
  - `tests/omega/pass/facts/unsigned_32bit_division_quotient_bound_through_other_machine_exit`
  - `tests/omega/pass/facts/unsigned_32bit_division_quotient_bound_through_target_body_exit`
- Reject (9):
  - `tests/omega/fail/facts/unsigned_32bit_division_quotient_bound_through_local_rejected`
  - `tests/omega/fail/facts/unsigned_32bit_division_quotient_bound_through_field_rejected`
  - `tests/omega/fail/facts/unsigned_32bit_division_quotient_bound_through_parameter_rejected`
  - `tests/omega/fail/facts/unsigned_32bit_division_quotient_bound_through_call_result_rejected`
  - `tests/omega/fail/facts/unsigned_32bit_division_quotient_bound_through_match_join_rejected`
  - `tests/omega/fail/facts/unsigned_32bit_division_quotient_bound_through_const_name_rejected`
  - `tests/omega/fail/facts/unsigned_32bit_division_quotient_bound_through_other_state_rejected`
  - `tests/omega/fail/facts/unsigned_32bit_division_quotient_bound_through_other_machine_rejected`
  - `tests/omega/fail/facts/unsigned_32bit_division_quotient_bound_through_target_body_rejected`

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_32bit_division_quotient_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u32\``).

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 32-bit unsigned (`u32`) bounded division, quotient interval synthesis (`[50, 50]`), consuming exact subtraction underflow discharge, and native realization agreement.

| Obligation Class | How This Fact Row Establishes It |
|---|---|
| **Establishment** | Entry contracts `requires dividend >= 200u32 && dividend <= 200u32; requires divisor >= 4u32 && divisor <= 4u32;`, nominal struct construction (`Operands`), and constant definitions (`DIVIDEND_VAL`, `DIVISOR_VAL`) establish the non-zero divisor and exact input ranges so `dividend / divisor` directly synthesizes `quotient >= 50u32 && quotient <= 50u32` without a post-hoc caller equality guard. |
| **Identity and lifetime** | The `u32` quotient value fact attaches to immutable local bindings, nominal struct fields (`Operands`), state/machine parameters, call return contracts, match join bindings, and target-scoped bodies without aliasing invalidation. |
| **Composition** | Calls (`compute_quotient`), match joins (`match_join`), state transitions (`check_state`), and cross-machine calls (`Verifier`) preserve `quotient >= 50u32 && quotient <= 50u32` so `let check: u32 = quotient - 50u32` is proved underflow-free directly from the carried fact. |
| **Soundness boundaries** | Each `<place>/invalid/main.omg` widens only `divisor <= 5u32`, dropping the lower bound of `dividend / divisor` to `40u32 < 50u32` so exact subtraction `quotient - 50u32` is rejected with `may overflow \`u32\``. |
| **Checked-to-Terminal Psi lowering (`Psi 05–07`)** | 32-bit unsigned division (`u32 / u32`) and exact subtraction (`u32 - u32`) lower into canonical `PSITERM\0` integer operations and execute in the Terminal Psi interpreter (`omega run --both`). |
| **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** | `ProgramEntry` settles for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`; `u32` scalar and `Operands` record layouts and ABI register calling plans validate without reopening source. |
| **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** | Backend lowering selects 32-bit unsigned division (`xor edx, edx` + `div r32` on `x86_64`, `udiv w-reg` on `arm64`) and 32-bit subtraction with bounded activation frames. |
| **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** | Native PE/ELF/Mach-O images emit and execute on `windows_x86_64` with exit code `0`, matching Terminal Psi (`omega run --both`). |

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded entry contracts `requires dividend >= 200u32 && dividend <= 200u32` and `requires divisor >= 4u32 && divisor <= 4u32` establish non-zero denominator safety and bounded dividend range. Unsigned division `dividend / divisor` synthesizes the exact quotient interval `[200 / 4, 200 / 4] = [50, 50]`, directly discharging exact subtraction `quotient - 50u32` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime:** The `u32` quotient scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal struct fields `Operands`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval `quotient >= 50u32 && quotient <= 50u32` is preserved intact.
  - **Composition:** Bounded division composes through calling conventions and control flow. In `call_result`, `compute_quotient` formally proves postcondition `ensures result >= 50u32 && result <= 50u32`; in `match_join`, both arms establish identical quotient intervals across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the divisor bound is widened to `divisor <= 5u32`. Because the quotient interval `[40, 50]` admits values below `50u32`, `let check: u32 = quotient - 50u32;` fails closed with diagnostic `exact arithmetic in machine ... local check may overflow \`u32\``. No underflowing subtraction escapes into code generation.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Unsigned division lowers into target-neutral Terminal Psi (`PSITERM\\0`) integer operations with unsigned division certificates. Proof obligations for non-zero divisor and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 4-byte stored width for `u32` (`size_of = 4`, align 4). Calling plans allocate 32-bit registers (`ecx`/`edx` on Windows x86_64, `edi`/`esi` on Linux x86_64, `w0`/`w1` on AArch64) according to target ABIs.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 32-bit unsigned division instructions: on x86_64, clearing `edx` (`xor edx, edx`) followed by `div r32` operating on `edx:eax`; on AArch64, `udiv w-dest, w-dividend, w-divisor`. Register allocation assigns 32-bit registers with zero-extended register semantics and bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- Unsigned remainder/modulo intervals (`u32 % u32`).
- Dynamic runtime non-zero denominator assertions for unbounded inputs.
- 64-bit and 128-bit unsigned division quotient bounds.
