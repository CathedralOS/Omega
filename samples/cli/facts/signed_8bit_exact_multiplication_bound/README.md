# Signed 8-Bit Exact Multiplication Bound

An 8-bit signed `i8` exact multiplication of bounded inputs (`lhs >= 5i8 && lhs <= 10i8`, `rhs >= 6i8 && rhs <= 10i8`) establishes `prod >= 30i8 && prod <= 100i8` so exact 8-bit signed subtraction `100i8 - prod` is proved overflow-free across all nine places.

Every place performs the same operation: define bounded signed 8-bit inputs `lhs` (`5..10`) and `rhs` (`6..10`), evaluate exact 8-bit multiplication `prod: i8 = lhs * rhs`, transport the product value through the place, pass `prod` to `verify_prod(prod: i8)` which requires `prod >= 30 && prod <= 100`, execute exact subtraction `100 - prod` returning `headroom`, compute `headroom - 28 == 0i32`, and verify in `main` that `res == 0` exiting 0. Only the transport mechanism of the product value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-bit signed exact multiplication evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both the bounded signed i8 operands and their product result are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the product result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the product result is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the product result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the product result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant rhs multiplies the bounded lhs |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the product result to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the product result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 8-bit signed exact multiplication and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_8bit_exact_multiplication_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `lhs * rhs` with inputs `8i8` and `9i8` establishes product `72i8` within static interval `30i8..100i8`. Passing `prod` to `verify_prod(prod)` satisfies its contract `prod >= 30 && prod <= 100`. `verify_prod` evaluates exact subtraction `100 - 72 == 28i8` and returns `(28 - 28) as i32 == 0i32`. In `main`, `transition res == 0` transitions to `state done() {}`, terminating with native exit 0. If multiplication or contract satisfaction returns an unexpected value, execution branches to `state bad() { crash Trap; }`, causing exit 255.
- Each invalid twin widens the `rhs` bound (e.g. `rhs <= 11i8` or `const RHS: i8 = 11`), so `lhs * rhs` can reach `110i8 > 100i8`. Calling `verify_prod(prod)` fails closed at compile time with `cannot prove requires contract for call verify_prod`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_8bit_exact_multiplication_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_8bit_exact_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_prod`).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_8bit_exact_multiplication_bound_through_*` and
`tests/omega/fail/facts/signed_8bit_exact_multiplication_bound_unproved_*` fixtures.
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 8-bit signed (`i8`) exact multiplication bounds (`[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, exact 8-bit signed multiplication on bounded inputs (`lhs >= 5i8 && lhs <= 10i8`, `rhs >= 6i8 && rhs <= 10i8`) statically establishes `prod >= 30i8 && prod <= 100i8`, discharging downstream contract requirement `prod >= 30 && prod <= 100` and exact subtraction `100i8 - prod` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires lhs >= 5 && lhs <= 10 && rhs >= 6 && rhs <= 10`) synthesize the exact product interval `[5 * 6, 10 * 10] = [30, 100]` directly upon evaluation, falling completely within the representable signed 8-bit integer domain `[-128, 127]`. This discharges downstream precondition `prod >= 30 && prod <= 100` and exact subtraction `100 - prod` without requiring any post-hoc caller equality guard (`transition x == 72`).
  - **Identity and lifetime:** The synthesized 8-bit product value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 8-bit signed product interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_prod(prod)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the `rhs` bound (`rhs <= 10` vs `rhs <= 11`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `rhs <= 11` is specified: the product interval expands to `[30, 110]`, which fails the required domain `prod >= 30 && prod <= 100`, strictly rejecting with `cannot prove requires contract for call verify_prod`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact 8-bit signed multiplication operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving concrete 8-bit signed multiplication certificates (`[-128, 127]`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 8-bit signed integer layout plan (`IntegerAt`: `i8` size 1, alignment 1) validates, and target `CallPlan`/`StatePlan` assign 8-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 8-bit multiplication instructions (`imul r8` on `x86_64`; sign-extended `mul w` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `imul` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Signed widening multiplication certificates (`i8 * i8 -> i16`), vector SIMD 8-bit packed multiplications, and saturating multiplication conversions.
