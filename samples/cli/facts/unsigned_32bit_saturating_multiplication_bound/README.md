# Unsigned 32-Bit Saturating Multiplication Bound

Fact: 32-bit unsigned saturating multiplication lhs *| rhs with bounded interval 50u32 <= prod <= 200u32 establishes 50u32 <= prod <= 200u32 across all nine places so exact subtractions 200u32 - prod and prod - 50u32 are safe.

Owning Task: `UNSIGNED-32BIT-SATURATING-MULTIPLICATION`
Claim Ticket: `0a3c98d9730d4bd7b8a9fb3fe12993f0` (`antigravity/w36-2`)
Obligation Class: Contract precondition (`requires value >= 50u32 && value <= 200u32` on `verify_prod`)
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md#overflow-policies), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that 32-bit unsigned saturating multiplication (`lhs * rhs` on `u32 in Saturating`) combined with an explicit bounded interval guard (`50u32 <= prod <= 200u32`) establishes the exact interval (`50u32 <= prod <= 200u32`) across all nine standard places. The compiler statically derives that the dominating guard constrains `prod` to `[50, 200]`, which directly discharges exact downstream 32-bit unsigned subtractions `headroom = 200u32 - prod` and `margin = prod - 50u32` and downstream contract `requires value >= 50u32 && value <= 200u32` of `verify_prod` without underflow, overflow, or runtime crash traps across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin | Pinned Corpus Pass | Pinned Corpus Fail |
| --- | --- | --- | --- | --- | --- | --- |
| root | `.` | canonical root witness multiplying saturating u32 values with interval guard and discharging exact subtractions | [`main.omg`](main.omg) | - | - | - |
| local | `local/` | the guarded product is bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_saturating_multiplication_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_multiplication_bound_through_local_rejected` |
| field | `field/` | the guarded product is stored in a ProdCarrier record field before extraction and contract verification | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_saturating_multiplication_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_multiplication_bound_through_field_rejected` |
| parameter | `parameter/` | a helper machine receives the guarded product as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_saturating_multiplication_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_multiplication_bound_through_parameter_rejected` |
| call result | `call_result/` | a producer machine returns the guarded product under postcondition ensures result >= 50u32 && result <= 200u32 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_saturating_multiplication_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_multiplication_bound_through_call_result_rejected` |
| match join | `match_join/` | both branches of a boolean match expression yield the guarded product before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_saturating_multiplication_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_multiplication_bound_through_match_join_rejected` |
| const name | `const_name/` | named constants LHS_VAL and RHS_VAL define operands for saturating multiplication before guarded subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_saturating_multiplication_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_multiplication_bound_through_const_name_rejected` |
| a nested machine | `other_state/` | an terminal machine transfer transfers the guarded product to a successor machine with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_saturating_multiplication_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_multiplication_bound_through_other_state_rejected` |
| another machine | `other_machine/` | an associated Relay machine forwards the guarded product to the caller under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_saturating_multiplication_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_multiplication_bound_through_other_machine_rejected` |
| another target body | `target_body/` | each of the four hosted target bodies computes saturating multiplication and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_saturating_multiplication_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_multiplication_bound_through_target_body_rejected` |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`. Perturbing the demanded oracle check in `main.omg` (`transition rc == 0u32` -> `transition rc == 1u32`) causes both engines to trap (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`), proving that the oracle discriminates.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the interval guard is widened to `201u32` (or operands adjusted to evaluate to `201u32`), failing to discharge `requires value >= 50u32 && value <= 200u32`. Check each twin with:

```sh
omega --check samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/local/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/field/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_saturating_multiplication_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_prod`

## Requirements and Evidence

- **Parent outcome:** `UNSIGNED-32BIT-SATURATING-MULTIPLICATION` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for 32-bit unsigned saturating multiplication (`u32 in Saturating`, `lhs * rhs`) constrained by a bounded interval guard (`50u32 <= prod <= 200u32`), discharging exact 32-bit unsigned subtractions `200u32 - prod` and `prod - 50u32` and downstream contract `requires value >= 50u32 && value <= 200u32` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#overflow-policies), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Performing 32-bit unsigned multiplication under the saturating arithmetic domain (`u32 in Saturating`) clamps the product at `u32::MAX` ($2^{32} - 1$) on overflow. Dominating transition guard `prod >= 50u32 && prod <= 200u32` establishes the non-saturating interval `[50, 200]` on the true branch. The Omega arithmetic solver derives this exact range certificate directly from the guard branch condition, verifying that `prod` satisfies `verify_prod(value: u32)` requires contract `value >= 50u32 && value <= 200u32` and discharging exact 32-bit unsigned subtractions `200u32 - prod` and `prod - 50u32` without requiring post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The saturating 32-bit unsigned integer value maintains 4-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions, transitioning cleanly between saturating domain behavior and guarded exact interval carrier identity.
- **Composition:** Guarded saturating multiplication interval preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the upper bound to `201u32` (or adjusting operand constants to evaluate to `201u32`), causing `verify_prod` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_prod` because `201u32` violates the contract upper bound `value <= 200u32`. The compiler soundly refuses any program where guarded saturating bounds cannot guarantee arrival contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Saturating multiplication expressions lower into canonical Terminal Psi (`PSITERM\0`) as `SaturatingIntegerMultiply` operations without overflow trap edges. Downstream transition guards synthesize integer range certificates retaining `[50, 200]` through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<u32>() == 4`, `align_of<u32>() == 4`, and `size_of<ProdCarrier>() == 4`. In `CallPlan` and `StatePlan`, 32-bit unsigned integers are allocated standard 32-bit general-purpose register lanes (`ecx`/`edi`/`w0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 32-bit unsigned saturating multiplication sequences: widening multiply (`mul` on x86_64, `umull` on arm64) checking upper 32-bit product with conditional move or clamp to `0xFFFFFFFF`, and exact subtraction selects 32-bit `sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Signed saturating multiplication (`i32 in Saturating`), 64-bit saturating arithmetic (`u64 in Saturating`), floating-point overflow policies, and SIMD saturating vector arithmetic remain governed under the wider numeric value roadmap.
