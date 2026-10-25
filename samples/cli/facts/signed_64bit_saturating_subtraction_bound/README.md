# Fact: 64-Bit Signed Saturating Subtraction Bound

> 64-bit signed saturating subtraction lhs -| rhs with bounded interval 40i64 <= diff <= 100i64 establishes 40i64 <= diff <= 100i64 across all nine places so exact subtractions 100i64 - diff and diff - 40i64 are safe.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 64-bit signed saturating subtraction bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | difference is stored within an intermediate local variable before arithmetic interval checks |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded operands are packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call returns saturating subtraction difference with postcondition contract `ensures result >= 40i64 && result <= 100i64` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | difference is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared symbolic constants define operand intervals and bound limits |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor machine `check_state(d: i64)` receives difference and verifies interval subtractions |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the difference to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform saturating subtraction and interval verification |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_64bit_saturating_subtraction_bound/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_subtraction_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `lhs = 100i64` (bounded `[80i64, 120i64]`) and `rhs = 30i64` (bounded `[20i64, 40i64]`).
- The saturating subtraction difference interval is statically proved: `min = 80 - 40 = 40i64`, `max = 120 - 20 = 100i64`.
- 64-bit signed saturating subtraction evaluates to `70i64` and is verified overflow-free.
- Consuming exact subtractions `let headroom: i64 = 100i64 - diff` (`30i64`) and `let margin: i64 = diff - 40i64` (`30i64`) are statically proved safe in interval `[0i64, 60i64]`.
- Exact unsigned casts `let check_h: u64 = headroom as u64` and `let check_m: u64 = margin as u64` confirm non-negative bounds without underflow or overflow.
- At runtime, `diff == 70i64 && headroom + margin == 60i64 && check_h + check_m == 60u64` confirms exact arithmetic agreement, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy. In `main`, `transition res == 0` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition res == 0` to `transition res == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin widens the `lhs` bound to `lhs <= 125i64`, allowing `diff` to take value `125 - 20 = 105i64 > 100i64`. The headroom subtraction `100i64 - diff` yields `-5i64`, which fails closed at compile time under the **Exact integer cast representability** obligation class during exact conversion to `u64` with diagnostic `Exact integer cast ... from \`i64\` to \`u64\` is not provably representable`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_64bit_saturating_subtraction_bound/<place>/invalid/main.omg
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
python tools/sample_native_gate.py --filter cli/facts/signed_64bit_saturating_subtraction_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`not provably representable`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_64bit_saturating_subtraction_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_subtraction_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_subtraction_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_subtraction_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_subtraction_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_subtraction_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_subtraction_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_subtraction_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_subtraction_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/signed_64bit_saturating_subtraction_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_subtraction_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_subtraction_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_subtraction_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_subtraction_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_subtraction_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_subtraction_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_subtraction_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_subtraction_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 64-bit signed (`i64`) bounded saturating subtraction, difference interval synthesis (`[40, 100]`), consuming exact subtractions (`100 - diff` and `diff - 40`), non-negative range verification, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded entry contracts `requires lhs >= 80i64 && lhs <= 120i64` and `requires rhs >= 20i64 && rhs <= 40i64` establish non-overlapping signed operand bounds. Saturating subtraction `a - b` in domain `Saturating` synthesizes interval `[80 - 40, 120 - 20] = [40, 100]`, directly discharging consuming exact subtractions `100i64 - diff` and `diff - 40i64` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime:** The `i64` difference scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal record fields `Operands`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval `diff >= 40i64 && diff <= 100i64` is preserved intact.
  - **Composition:** Bounded saturating subtraction composes through calling conventions and control flow. In `call_result`, `compute_sat_diff` formally proves postcondition `ensures result >= 40i64 && result <= 100i64`; in `match_join`, both arms establish identical difference intervals across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the `lhs` bound is widened to `lhs <= 125i64`. Because the difference interval `[40, 105]` admits `diff = 105i64`, `let headroom: i64 = 100i64 - diff;` yields `-5i64`, which fails closed with diagnostic `Exact integer cast in machine ... from \`i64\` to \`u64\` is not provably representable`. No underflowing subtraction escapes into unsigned domains.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Signed 64-bit saturating subtraction lowers into target-neutral Terminal Psi (`PSITERM\0`) integer operations with saturating subtraction certificates and bound interval `IntegerRange { min: 40, max: 100 }`. Proof obligations for bounded arithmetic and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 8-byte stored width for `i64` (`size_of = 8`, `align_of = 8`). Calling plans allocate standard ABI 64-bit register slices (`rcx`/`rdx` on Windows x86_64, `rdi`/`rsi` on Linux x86_64, `x0`/`x1` on AArch64).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 64-bit saturating subtraction instructions: on x86_64, scalar subtraction with signed saturation clamping or branchless `cmp`/`cmov` saturation logic; on AArch64, `sqsub` (signed saturating subtract) or subtraction with signed saturation clamp ceiling. Register allocation assigns register homes with bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- 128-bit signed saturating subtraction bound synthesis.
- Saturating addition bounds on signed 64-bit integers.
- Saturating arithmetic bounds with negative operand intervals.
