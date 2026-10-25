# Signed 32-Bit Wrapping Subtraction Bound

Fact: 32-bit signed wrapping subtraction lhs -% rhs with bounded interval 40i32 <= diff <= 100i32 establishes 40i32 <= diff <= 100i32 across all nine places so exact subtractions 100i32 - diff and diff - 40i32 are safe.

Owning Task: `SIGNED-32BIT-WRAPPING-SUBTRACTION`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md#overflow-policies), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that 32-bit signed wrapping subtraction (`lhs - rhs` on `i32 in Wrapping`) combined with an explicit bounded interval guard (`40i32 <= diff <= 100i32`) establishes the exact interval (`40i32 <= diff <= 100i32`) across all nine standard places. The compiler statically derives that the dominating guard constrains `diff` to `[40, 100]`, which directly discharges exact downstream 32-bit signed subtractions `sub_hi = 100i32 - diff` and `sub_lo = diff - 40i32` and downstream contract `requires value >= 40i32 && value <= 100i32` of `verify_diff` without underflow, overflow, or runtime crash traps across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin | Obligation Class | Pinned Pass Fixture | Pinned Reject Fixture |
| --- | --- | --- | --- | --- | --- | --- | --- |
| root | `.` | canonical root witness subtracting wrapping i32 values with interval guard and discharging exact subtractions | [`main.omg`](main.omg) | - | Contract precondition + exact subtraction (`100i32 - diff`, `diff - 40i32`) | - | - |
| local | `local/` | the guarded difference is bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition (`requires value >= 40i32 && value <= 100i32`) | `tests/omega/pass/facts/signed_32bit_wrapping_subtraction_bound_through_local_exit` | `tests/omega/fail/facts/signed_32bit_wrapping_subtraction_bound_through_local_rejected` |
| field | `field/` | the guarded difference is stored in a DiffCarrier record field before extraction and contract verification | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition (`requires value >= 40i32 && value <= 100i32`) | `tests/omega/pass/facts/signed_32bit_wrapping_subtraction_bound_through_field_exit` | `tests/omega/fail/facts/signed_32bit_wrapping_subtraction_bound_through_field_rejected` |
| parameter | `parameter/` | a helper machine receives the guarded difference as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition (`requires value >= 40i32 && value <= 100i32`) | `tests/omega/pass/facts/signed_32bit_wrapping_subtraction_bound_through_parameter_exit` | `tests/omega/fail/facts/signed_32bit_wrapping_subtraction_bound_through_parameter_rejected` |
| call result | `call_result/` | a producer machine returns the guarded difference under postcondition ensures result >= 40i32 && result <= 100i32 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition (`requires value >= 40i32 && value <= 100i32`) | `tests/omega/pass/facts/signed_32bit_wrapping_subtraction_bound_through_call_result_exit` | `tests/omega/fail/facts/signed_32bit_wrapping_subtraction_bound_through_call_result_rejected` |
| match join | `match_join/` | both branches of a boolean match expression yield the guarded difference before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition (`requires value >= 40i32 && value <= 100i32`) | `tests/omega/pass/facts/signed_32bit_wrapping_subtraction_bound_through_match_join_exit` | `tests/omega/fail/facts/signed_32bit_wrapping_subtraction_bound_through_match_join_rejected` |
| const name | `const_name/` | named constants LHS_VAL and RHS_VAL define operands for wrapping subtraction before guarded subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition (`requires value >= 40i32 && value <= 100i32`) | `tests/omega/pass/facts/signed_32bit_wrapping_subtraction_bound_through_const_name_exit` | `tests/omega/fail/facts/signed_32bit_wrapping_subtraction_bound_through_const_name_rejected` |
| a nested machine | `other_state/` | an terminal machine transfer transfers the guarded difference to a successor machine with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition (`requires value >= 40i32 && value <= 100i32`) | `tests/omega/pass/facts/signed_32bit_wrapping_subtraction_bound_through_other_state_exit` | `tests/omega/fail/facts/signed_32bit_wrapping_subtraction_bound_through_other_state_rejected` |
| another machine | `other_machine/` | an associated Relay machine forwards the guarded difference to the caller under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition (`requires value >= 40i32 && value <= 100i32`) | `tests/omega/pass/facts/signed_32bit_wrapping_subtraction_bound_through_other_machine_exit` | `tests/omega/fail/facts/signed_32bit_wrapping_subtraction_bound_through_other_machine_rejected` |
| another target body | `target_body/` | each of the four hosted target bodies computes wrapping subtraction and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition (`requires value >= 40i32 && value <= 100i32`) | `tests/omega/pass/facts/signed_32bit_wrapping_subtraction_bound_through_target_body_exit` | `tests/omega/fail/facts/signed_32bit_wrapping_subtraction_bound_through_target_body_rejected` |

## Discrimination

- **Runtime oracle discrimination:** Perturbing a single token in [`main.omg`](main.omg) (`transition rc == 0i32` -> `transition rc == 1i32`) causes both engines under `omega run --both` to trap at runtime (`native exit -1073741795`, `interp crash: Trap (host presentation agrees)`), confirming the `0` exit oracle actively discriminates the computed wrapping difference (`100i32 - 30i32 == 70i32` in `i32 in Wrapping`, `(100 - 70) + (70 - 40) - 60 == 0`).
- **Static proof discrimination:** Widening the interval guard upper bound from `100i32` to `101i32` in each `<place>/invalid/main.omg` causes static verification to fail closed with `cannot prove requires contract for call verify_diff`.

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/signed_32bit_wrapping_subtraction_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_subtraction_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the interval guard is widened to `101i32` (or operands adjusted to evaluate to `101i32`), failing to discharge `requires value >= 40i32 && value <= 100i32`. Check each twin with:

```sh
omega --check samples/cli/facts/signed_32bit_wrapping_subtraction_bound/local/invalid/main.omg
omega --check samples/cli/facts/signed_32bit_wrapping_subtraction_bound/field/invalid/main.omg
omega --check samples/cli/facts/signed_32bit_wrapping_subtraction_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/signed_32bit_wrapping_subtraction_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/signed_32bit_wrapping_subtraction_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/signed_32bit_wrapping_subtraction_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/signed_32bit_wrapping_subtraction_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/signed_32bit_wrapping_subtraction_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/signed_32bit_wrapping_subtraction_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_diff`

## Requirements and Evidence

- **Parent outcome:** `SIGNED-32BIT-WRAPPING-SUBTRACTION` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for 32-bit signed wrapping subtraction (`i32 in Wrapping`, `lhs - rhs`) constrained by a bounded interval guard (`40i32 <= diff <= 100i32`), discharging exact 32-bit signed subtractions `100i32 - diff` and `diff - 40i32` and downstream contract `requires value >= 40i32 && value <= 100i32` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#overflow-policies), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Performing 32-bit signed subtraction under the wrapping arithmetic domain (`i32 in Wrapping`) computes modular two's-complement subtraction modulo $2^{32}$. Dominating transition guard `diff >= 40i32 && diff <= 100i32` establishes the non-wrapping interval `[40, 100]` on the true branch. The Omega arithmetic solver derives this exact range certificate directly from the guard branch condition, verifying that `diff` satisfies `verify_diff(value: i32)` requires contract `value >= 40i32 && value <= 100i32` and discharging exact 32-bit subtractions `100i32 - diff` and `diff - 40i32` without requiring post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The wrapping 32-bit signed integer value maintains 4-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions, transitioning cleanly between wrapping domain behavior and guarded exact interval carrier identity.
- **Composition:** Guarded wrapping subtraction interval preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the upper bound to `101i32` (or adjusting operand constants to evaluate to `101i32`), causing `verify_diff` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_diff` because `101i32` violates the contract upper bound `value <= 100i32`. The compiler soundly refuses any program where guarded wrapping bounds cannot guarantee arrival contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Wrapping subtraction expressions lower into canonical Terminal Psi (`PSITERM\0`) as `WrappingIntegerSubtract` operations without overflow trap edges. Downstream transition guards synthesize integer range certificates retaining `[40, 100]` through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<i32>() == 4`, `align_of<i32>() == 4`, and `size_of<DiffCarrier>() == 4`. In `CallPlan` and `StatePlan`, 32-bit signed integers are allocated standard 32-bit general-purpose register lanes (`ecx`/`edi`/`w0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 32-bit signed wrapping subtraction instructions: `sub r32, r/m32` without overflow trap assertions on x86_64 and `sub w, w, w` on arm64 architectures. Exact subtraction also selects 32-bit `sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Wrapping multiplication (`*%`), 64-bit signed wrapping arithmetic (`i64 in Wrapping`), floating-point overflow policies, and SIMD modular vector arithmetic remain governed under the wider numeric value roadmap.
