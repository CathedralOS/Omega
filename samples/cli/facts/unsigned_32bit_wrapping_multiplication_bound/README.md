# Fact: 32-Bit Unsigned Wrapping Multiplication Bound

> 32-bit unsigned wrapping multiplication lhs *% rhs with bounded interval 50u32 <= prod <= 200u32 establishes 50u32 <= prod <= 200u32 across all nine places so exact subtractions 200u32 - prod and prod - 50u32 are safe.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 32-bit unsigned wrapping multiplication bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | product is stored in intermediate local bindings before exact interval subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded operands are packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call returns wrapping multiplication product with postcondition contract `ensures result >= 50u32 && result <= 200u32` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | product is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared symbolic constants define operand intervals and bound limits |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor state `check_state(p: u32)` receives product and verifies interval subtractions |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the product to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform wrapping multiplication and interval verification |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_32bit_wrapping_multiplication_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `lhs = 15u32` (bounded `[10u32, 20u32]`) and `rhs = 8u32` (bounded `[5u32, 10u32]`).
- The wrapping multiplication product interval is statically proved: `min = 10 * 5 = 50u32`, `max = 20 * 10 = 200u32`.
- 32-bit unsigned wrapping multiplication evaluates to `120u32` and is verified overflow-free.
- Consuming exact subtractions `let headroom: u32 = 200u32 - prod` (`80u32`) and `let margin: u32 = prod - 50u32` (`70u32`) are statically proved safe in interval `[0u32, 150u32]`.
- At runtime, `prod == 120u32 && headroom + margin == 150u32` confirms exact arithmetic agreement, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy.
- Each invalid twin widens the `lhs` bound to `lhs <= 21u32`, allowing `prod` to take value `21 * 10 = 210u32 > 200u32`. The headroom subtraction `200u32 - prod` fails closed at compile time with diagnostic `exact arithmetic in machine ... local \`headroom\` may overflow \`u32\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_32bit_wrapping_multiplication_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_32bit_wrapping_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u32\``).

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 32-bit unsigned (`u32`) bounded wrapping multiplication, product interval synthesis (`[50, 200]`), consuming exact subtractions (`200 - prod` and `prod - 50`), non-negative range verification, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded entry contracts `requires lhs >= 10u32 && lhs <= 20u32` and `requires rhs >= 5u32 && rhs <= 10u32` establish non-overlapping unsigned operand bounds. Wrapping multiplication `a * b` in domain `Wrapping` synthesizes interval `[10 * 5, 20 * 10] = [50, 200]`, directly discharging consuming exact subtractions `200u32 - prod` and `prod - 50u32` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime:** The `u32` product scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal record fields `Operands`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval `prod >= 50u32 && prod <= 200u32` is preserved intact.
  - **Composition:** Bounded wrapping multiplication composes through calling conventions and control flow. In `call_result`, `compute_wrap_prod` formally proves postcondition `ensures result >= 50u32 && result <= 200u32`; in `match_join`, both arms establish identical product intervals across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the `lhs` bound is widened to `lhs <= 21u32`. Because the product interval `[50, 210]` admits `prod = 210u32`, `let headroom: u32 = 200u32 - prod;` fails closed with diagnostic `exact arithmetic in machine ... local \`headroom\` may overflow \`u32\``. No underflowing subtraction escapes into runtime execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Unsigned 32-bit wrapping multiplication lowers into target-neutral Terminal Psi (`PSITERM\0`) integer operations with wrapping multiplication arithmetic certificates (`IntegerRange { min: 50, max: 200 }`). Proof obligations for bounded arithmetic and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 4-byte stored width for `u32` (`size_of = 4`, `align_of = 4`). Calling plans allocate standard ABI 32-bit register slices (`ecx`/`edx` on Windows x86_64, `edi`/`esi` on Linux x86_64, `w0`/`w1` on AArch64).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 32-bit integer multiplication instructions: on x86_64, two-operand 32-bit `imul r32, r/m32` (which computes identical lower 32-bit results for signed and unsigned wrapping multiplication); on AArch64, 32-bit `mul w_dest, w_lhs, w_rhs`. Register allocation assigns register homes with bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- 64-bit and 128-bit unsigned wrapping multiplication bounds.
- Signed wrapping multiplication bound intervals.
- Wrapping arithmetic bounds crossing integer overflow boundaries.
