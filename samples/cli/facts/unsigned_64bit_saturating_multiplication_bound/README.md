# Fact: 64-Bit Unsigned Saturating Multiplication Bound

> 64-bit unsigned saturating multiplication lhs *| rhs with bounded interval 50u64 <= prod <= 200u64 establishes 50u64 <= prod <= 200u64 across all nine places so exact subtractions 200u64 - prod and prod - 50u64 are safe.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 64-bit unsigned saturating multiplication bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | product is stored in intermediate local bindings before exact interval subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded operands are packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call returns saturating multiplication product with postcondition contract `ensures result >= 50u64 && result <= 200u64` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | product is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared symbolic constants define operand intervals and bound limits |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor machine `check_state(p: u64)` receives product and verifies interval subtractions |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the product to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform saturating multiplication and interval verification |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_64bit_saturating_multiplication_bound/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `lhs = 15u64` (bounded `[10u64, 20u64]`) and `rhs = 8u64` (bounded `[5u64, 10u64]`).
- The saturating multiplication product interval is statically proved: `min = 10 * 5 = 50u64`, `max = 20 * 10 = 200u64`.
- 64-bit unsigned saturating multiplication evaluates to `120u64` and is verified overflow-free.
- Consuming exact subtractions `let headroom: u64 = 200u64 - prod` (`80u64`) and `let margin: u64 = prod - 50u64` (`70u64`) are statically proved safe in interval `[0u64, 150u64]`.
- At runtime, `prod == 120u64 && headroom + margin == 150u64` confirms exact arithmetic agreement, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy. A 1-token perturbation of the demanded result (`transition res == 0` to `transition res == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin widens the `lhs` bound to `lhs <= 21u64`, allowing `prod` to take value `21 * 10 = 210u64 > 200u64`. The headroom subtraction `200u64 - prod` fails closed at compile time under the **Exact arithmetic overflow** obligation class with diagnostic `exact arithmetic in machine ... local \`headroom\` may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_64bit_saturating_multiplication_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Exact arithmetic overflow | `may overflow \`u64\`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Exact arithmetic overflow | `may overflow \`u64\`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Exact arithmetic overflow | `may overflow \`u64\`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Exact arithmetic overflow | `may overflow \`u64\`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Exact arithmetic overflow | `may overflow \`u64\`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Exact arithmetic overflow | `may overflow \`u64\`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Exact arithmetic overflow | `may overflow \`u64\`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Exact arithmetic overflow | `may overflow \`u64\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Exact arithmetic overflow | `may overflow \`u64\`` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\cargo-target\debug\omega-latest.exe --filter cli/facts/unsigned_64bit_saturating_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/unsigned_64bit_saturating_multiplication_bound_through_local_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_multiplication_bound_through_field_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_multiplication_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_multiplication_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_multiplication_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_multiplication_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_multiplication_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_multiplication_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_multiplication_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/unsigned_64bit_saturating_multiplication_bound_through_local_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_multiplication_bound_through_field_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_multiplication_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_multiplication_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_multiplication_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_multiplication_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_multiplication_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_multiplication_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_multiplication_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 64-bit unsigned (`u64`) bounded saturating multiplication ([numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)), product interval synthesis (`[50, 200]`), consuming exact subtractions (`200 - prod` and `prod - 50`), non-negative range verification, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded entry contracts `requires lhs >= 10u64 && lhs <= 20u64` and `requires rhs >= 5u64 && rhs <= 10u64` establish non-overlapping unsigned operand bounds. Saturating multiplication `a * b` in domain `Saturating` synthesizes interval `[10 * 5, 20 * 10] = [50, 200]`, directly discharging consuming exact subtractions `200u64 - prod` and `prod - 50u64` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime:** The `u64` product scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal record fields `Operands`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval `prod >= 50u64 && prod <= 200u64` is preserved intact.
  - **Composition:** Bounded saturating multiplication composes through calling conventions and control flow. In `call_result`, `compute_sat_prod` formally proves postcondition `ensures result >= 50u64 && result <= 200u64`; in `match_join`, both arms establish identical product intervals across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the `lhs` bound is widened to `lhs <= 21u64`. Because the product interval `[50, 210]` admits `prod = 210u64`, `let headroom: u64 = 200u64 - prod;` fails closed with diagnostic `exact arithmetic in machine ... local \`headroom\` may overflow \`u64\``. No underflowing subtraction escapes into runtime execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Unsigned 64-bit saturating multiplication lowers into target-neutral Terminal Psi (`PSITERM\0`) integer operations with saturating multiplication arithmetic certificates (`IntegerRange { min: 50, max: 200 }`). Proof obligations for bounded arithmetic and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 8-byte stored width for `u64` (`size_of = 8`, `align_of = 8`). Calling plans allocate standard ABI 64-bit register slices (`rcx`/`rdx` on Windows x86_64, `rdi`/`rsi` on Linux x86_64, `x0`/`x1` on AArch64).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 64-bit saturating multiplication instructions: on x86_64, 64-bit multiplication (`mul` / `imul`) with overflow check and saturation ceiling clamping; on AArch64, `umulh` check or branchless saturation clamp. Register allocation assigns register homes with bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- 128-bit unsigned saturating multiplication bounds.
- Signed saturating multiplication bound intervals.
- Saturating arithmetic bounds with inputs near saturation thresholds.
