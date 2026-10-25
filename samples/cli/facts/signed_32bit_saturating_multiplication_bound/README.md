# Fact: 32-Bit Signed Saturating Multiplication Bound

> 32-bit signed saturating multiplication lhs *| rhs with bounded interval 50i32 <= prod <= 200i32 establishes 50i32 <= prod <= 200i32 across all nine places so exact subtractions 200i32 - prod and prod - 50i32 are safe.

## Places & witnesses

- **Claim ticket:** `8ebe4a7e9c0640c8b5c820261dd31355` (`antigravity/w35-2`)
- **Obligation class:** Exact integer cast representability (`let check_h: u32 = headroom as u32` after `let headroom: i32 = 200i32 - prod`)

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Pinned Corpus Pass | Pinned Corpus Fail | Note |
|---|---|---|---|---|---|
| root | [`main.omg`](main.omg) | - | - | - | Root package probe exercising 32-bit signed saturating multiplication bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_saturating_multiplication_bound_through_local_exit` | `tests/omega/fail/facts/signed_32bit_saturating_multiplication_bound_through_local_rejected` | product is stored in intermediate local bindings before exact interval subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_saturating_multiplication_bound_through_field_exit` | `tests/omega/fail/facts/signed_32bit_saturating_multiplication_bound_through_field_rejected` | bounded operands are packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_saturating_multiplication_bound_through_parameter_exit` | `tests/omega/fail/facts/signed_32bit_saturating_multiplication_bound_through_parameter_rejected` | bounded operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_saturating_multiplication_bound_through_call_result_exit` | `tests/omega/fail/facts/signed_32bit_saturating_multiplication_bound_through_call_result_rejected` | helper call returns saturating multiplication product with postcondition contract `ensures result >= 50i32 && result <= 200i32` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_saturating_multiplication_bound_through_match_join_exit` | `tests/omega/fail/facts/signed_32bit_saturating_multiplication_bound_through_match_join_rejected` | product is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_saturating_multiplication_bound_through_const_name_exit` | `tests/omega/fail/facts/signed_32bit_saturating_multiplication_bound_through_const_name_rejected` | declared symbolic constants define operand intervals and bound limits |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_saturating_multiplication_bound_through_other_state_exit` | `tests/omega/fail/facts/signed_32bit_saturating_multiplication_bound_through_other_state_rejected` | successor machine `check_state(p: i32)` receives product and verifies interval subtractions |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_saturating_multiplication_bound_through_other_machine_exit` | `tests/omega/fail/facts/signed_32bit_saturating_multiplication_bound_through_other_machine_rejected` | forwarding machine passes the product to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_saturating_multiplication_bound_through_target_body_exit` | `tests/omega/fail/facts/signed_32bit_saturating_multiplication_bound_through_target_body_rejected` | target-scoped implementations across all four platforms perform saturating multiplication and interval verification |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_32bit_saturating_multiplication_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `lhs = 15i32` (bounded `[10i32, 20i32]`) and `rhs = 8i32` (bounded `[5i32, 10i32]`).
- The saturating multiplication product interval is statically proved: `min = 10 * 5 = 50i32`, `max = 20 * 10 = 200i32`.
- 32-bit signed saturating multiplication evaluates to `120i32` and is verified overflow-free.
- Consuming exact subtractions `let headroom: i32 = 200i32 - prod` (`80i32`) and `let margin: i32 = prod - 50i32` (`70i32`) are statically proved safe in interval `[0i32, 150i32]`.
- Exact unsigned casts `let check_h: u32 = headroom as u32` and `let check_m: u32 = margin as u32` confirm non-negative bounds without underflow or overflow.
- At runtime, `prod == 120i32 && headroom + margin == 150i32 && check_h + check_m == 150u32` confirms exact arithmetic agreement, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, causing both engines to trap: `native exit -1073741795` / `interp crash: Trap (host presentation agrees)` when perturbing `transition res == 0` to `transition res == 1` in `main.omg`) on discrepancy.
- Each invalid twin widens the `lhs` bound to `lhs <= 21i32`, allowing `prod` to take value `21 * 10 = 210i32 > 200i32`. The headroom subtraction `200i32 - prod` yields `-10i32`, which fails closed at compile time during exact conversion to `u32` with diagnostic `Exact integer cast ... from \`i32\` to \`u32\` is not provably representable`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_32bit_saturating_multiplication_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_32bit_saturating_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`not provably representable`).

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 32-bit signed (`i32`) bounded saturating multiplication, product interval synthesis (`[50, 200]`), consuming exact subtractions (`200 - prod` and `prod - 50`), non-negative range verification, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded entry contracts `requires lhs >= 10i32 && lhs <= 20i32` and `requires rhs >= 5i32 && rhs <= 10i32` establish non-overlapping signed operand bounds. Saturating multiplication `a * b` in domain `Saturating` synthesizes interval `[10 * 5, 20 * 10] = [50, 200]`, directly discharging consuming exact subtractions `200i32 - prod` and `prod - 50i32` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime:** The `i32` product scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal record fields `Operands`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval `prod >= 50i32 && prod <= 200i32` is preserved intact.
  - **Composition:** Bounded saturating multiplication composes through calling conventions and control flow. In `call_result`, `compute_sat_prod` formally proves postcondition `ensures result >= 50i32 && result <= 200i32`; in `match_join`, both arms establish identical product intervals across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the `lhs` bound is widened to `lhs <= 21i32`. Because the product interval `[50, 210]` admits `prod = 210i32`, `let headroom: i32 = 200i32 - prod;` yields `-10i32`, which fails closed with diagnostic `Exact integer cast in machine ... from \`i32\` to \`u32\` is not provably representable`. No negative value escapes into unsigned domains.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Signed 32-bit saturating multiplication lowers into target-neutral Terminal Psi (`PSITERM\0`) integer operations with saturating multiplication arithmetic certificates (`IntegerRange { min: 50, max: 200 }`). Proof obligations for bounded arithmetic and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 4-byte stored width for `i32` (`size_of = 4`, `align_of = 4`). Calling plans allocate standard ABI 32-bit register slices (`ecx`/`edx` on Windows x86_64, `edi`/`esi` on Linux x86_64, `w0`/`w1` on AArch64).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 32-bit saturating multiplication instructions: on x86_64, 32-bit `imul` with overflow check and signed saturation clamp ceiling (`i32::MAX`/`i32::MIN`); on AArch64, `smull` (signed multiply long) check or branchless saturation clamp. Register allocation assigns register homes with bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- 64-bit and 128-bit signed saturating multiplication bounds.
- Negative operand interval saturating multiplication (e.g. `lhs < 0` or `rhs < 0`).
- Saturating arithmetic bounds with inputs near saturation thresholds.
