# Fact: 32-Bit Unsigned Exact Arithmetic Bound and Interval Synthesis

> Adding two bounded 32-bit unsigned `u32` values (`lhs >= 20u32 && lhs <= 40u32` and `rhs >= 30u32 && rhs <= 50u32`) establishes the exact post-addition interval (`sum >= 50u32 && sum <= 90u32`), discharging both `90u32 - sum` and `sum - 50u32` across all nine places.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 32-bit unsigned interval arithmetic and boundary subtraction |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded `u32` operands and sum are transported through intermediate local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded `u32` operands are packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded `u32` operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call computes addition and returns `u32` with postcondition contract |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | addition result is synthesized and joined across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared symbolic constants (`LHS_MIN`, `RHS_MAX`, `SUM_MAX`) define bounds and subtrahends |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor state `check_sum(s: u32)` receives sum and verifies arithmetic interval |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the sum to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped implementations across all four platforms compute addition and interval subtraction |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_32bit_exact_arithmetic_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_arithmetic_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `lhs = 30u32` (within `[20u32, 40u32]`) and `rhs = 40u32` (within `[30u32, 50u32]`).
- Addition `lhs + rhs` statically proves that `sum` resides within interval `[50u32, 90u32]` without 32-bit unsigned overflow (`40 + 50 = 90 <= 4294967295`).
- Exact subtractions `headroom = 90u32 - sum` and `margin = sum - 50u32` are statically proved not to underflow because `sum <= 90u32` and `sum >= 50u32`.
- At runtime, `sum == 70u32`, `headroom == 20u32`, and `margin == 20u32` are verified, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy.
- Each invalid twin widens the `rhs` bound to `rhs <= 51u32`, allowing `sum` to reach `40 + 51 = 91u32 > 90u32`. The exact subtraction `90u32 - sum` is no longer provably safe, and the compiler fails closed at compile time with `may overflow \`u32\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_32bit_exact_arithmetic_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_32bit_exact_arithmetic_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u32\``).

## Pinned Fixtures

The row is pinned under
`tests/omega/pass/facts/unsigned_32bit_exact_arithmetic_bound_through_*_exit`
and
`tests/omega/fail/facts/unsigned_32bit_exact_arithmetic_bound_through_*_rejected`.
Each positive fixture is a byte-identical copy of its `<place>/main.omg` plus
the place's `build.omg`; it binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and records
native exit 0 with Terminal Psi agreement. Each reject twin is a byte-identical
copy of its `<place>/invalid/main.omg` and pins the ``may overflow `u32` ``
diagnostic fragment (the `rhs` bound widened from 50 to 51, or `RHS_MAX`
changed from 50 to 51, and nothing else).

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 32-bit unsigned (`u32`) bounded addition, interval synthesis (`[50, 90]`), upper-bound and lower-bound exact subtraction discharge, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Entry contracts `requires lhs >= 20u32 && lhs <= 40u32` and `requires rhs >= 30u32 && rhs <= 50u32` establish non-negative bounded intervals for both 32-bit unsigned operands. Exact addition `sum = lhs + rhs` composes the interval endpoints (`[20 + 30, 40 + 50] = [50, 90]`), which directly discharges the proof obligations for both `90u32 - sum` and `sum - 50u32` without needing post-hoc caller equality guards.
  - **Identity and lifetime:** The scalar `u32` values preserve their numeric identity and bounds through copies and movements. In each place (locals, nominal struct fields `Operands`, function parameters, return values, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval premises `sum >= 50u32 && sum <= 90u32` remain valid.
  - **Composition:** Bounded arithmetic composes seamlessly across language constructs. In `call_result`, `add_operands` guarantees postcondition `ensures result >= 50u32 && result <= 90u32`; in `match_join`, both branch expressions (`lhs + rhs` and `rhs + lhs`) establish the identical interval across the join point; in `other_state` and `other_machine`, the interval is verified against incoming contracts.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the upper bound is relaxed to `rhs <= 51u32`. Because the synthesized interval `[50, 91]` exceeds `90u32`, `let headroom: u32 = 90u32 - sum;` fails closed with diagnostic `exact arithmetic in machine ... local headroom may overflow \`u32\``. No underflowing subtraction is permitted to generate code.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source arithmetic expressions lower into target-neutral Terminal Psi (`PSITERM\\0`) integer operations with exact arithmetic certificates. Proof obligations are satisfied and erased while preserving computational semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 4-byte stored width for `u32` (`size_of = 4`, align 4). Calling plans allocate 32-bit sub-registers (`ecx`/`edx` on Windows x86_64, `edi`/`esi` on Linux x86_64, `w0`/`w1` on AArch64) according to target ABIs.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 32-bit ISA arithmetic instructions (`add r32, r32` and `sub r32, r32` on x86_64, `add w-reg, w-reg, w-reg` and `sub w-reg, w-reg, w-reg` on AArch64). Register allocation places variables into 32-bit scratch/argument registers with bounded activation claims and zero-extended register semantics.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- Non-linear integer multiplication intervals (`u32 * u32`).
- Signed 32-bit (`i32`) mixed-polarity interval addition and subtraction bounds.
- Floating-point interval constraints (`f32`/`f64`).
