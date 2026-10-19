# Fact: 8-Bit Unsigned Saturating Arithmetic Bound

> 8-bit unsigned saturating addition lhs +| rhs with bounded interval 50u8 <= sum <= 100u8 establishes 50u8 <= sum <= 100u8 across all nine places so exact subtractions 100u8 - sum and sum - 50u8 are safe.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 8-bit unsigned saturating addition bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | sum is stored in intermediate local bindings before exact interval subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded operands are packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call returns saturating addition sum with postcondition contract `ensures result >= 50u8 && result <= 100u8` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | sum is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared symbolic constants define operand intervals and bound limits |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor state `check_state(s: u8)` receives sum and verifies interval subtractions |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the sum to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform saturating addition and interval verification |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_8bit_saturating_arithmetic_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_arithmetic_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `lhs = 60u8` (bounded `[40u8, 80u8]`) and `rhs = 15u8` (bounded `[10u8, 20u8]`).
- The saturating addition sum interval is statically proved: `min = 40 + 10 = 50u8`, `max = 80 + 20 = 100u8`.
- 8-bit unsigned saturating addition evaluates to `75u8` and is verified overflow-free.
- Consuming exact subtractions `let headroom: u8 = 100u8 - sum` (`25u8`) and `let margin: u8 = sum - 50u8` (`25u8`) are statically proved safe in interval `[0u8, 50u8]`.
- At runtime, `sum == 75u8 && headroom + margin == 50u8` confirms exact arithmetic agreement, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy.
- Each invalid twin widens the `lhs` bound to `lhs <= 85u8`, allowing `sum` to take value `85 + 20 = 105u8 > 100u8`. The headroom subtraction `100u8 - sum` fails closed at compile time with diagnostic `Exact arithmetic operation in machine ... may overflow \`u8\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_8bit_saturating_arithmetic_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_8bit_saturating_arithmetic_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u8\``).

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 8-bit unsigned (`u8`) bounded saturating addition, sum interval synthesis (`[50, 100]`), consuming exact subtractions (`100 - sum` and `sum - 50`), non-negative range verification, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded entry contracts `requires lhs >= 40u8 && lhs <= 80u8` and `requires rhs >= 10u8 && rhs <= 20u8` establish non-overlapping unsigned operand bounds. Saturating addition `a + b` in domain `Saturating` synthesizes interval `[40 + 10, 80 + 20] = [50, 100]`, directly discharging consuming exact subtractions `100u8 - sum` and `sum - 50u8` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime:** The `u8` sum scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal record fields `Operands`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval `sum >= 50u8 && sum <= 100u8` is preserved intact.
  - **Composition:** Bounded saturating addition composes through calling conventions and control flow. In `call_result`, `compute_sum` formally proves postcondition `ensures result >= 50u8 && result <= 100u8`; in `match_join`, both arms establish identical sum intervals across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the `lhs` bound is widened to `lhs <= 85u8`. Because the sum interval `[50, 105]` admits `sum = 105u8`, `let headroom: u8 = 100u8 - sum;` fails closed with diagnostic `Exact arithmetic operation in machine ... may overflow \`u8\``. No out-of-range subtraction escapes into runtime execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Unsigned 8-bit saturating addition lowers into target-neutral Terminal Psi (`PSITERM\0`) integer operations with saturating addition arithmetic certificates (`IntegerRange { min: 50, max: 100 }`). Proof obligations for bounded arithmetic and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 1-byte stored width for `u8` (`size_of = 1`, `align_of = 1`). Calling plans allocate standard ABI 8-bit register slices (`cl`/`dl` on Windows x86_64, `dil`/`sil` on Linux x86_64, `w0`/`w1` on AArch64).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 8-bit saturating addition instructions: on x86_64, scalar saturated addition clamping or `paddusb` / branchless comparison clamping; on AArch64, `uqadd` (unsigned saturating addition) or addition with scalar saturation ceiling. Register allocation assigns register homes with bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- 16-bit, 32-bit, and 64-bit unsigned saturating addition interval bounds.
- Saturating subtraction bound intervals (`a - b` in `Saturating`).
- Signed saturating arithmetic bound synthesis.
