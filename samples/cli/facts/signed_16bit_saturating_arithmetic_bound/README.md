# Signed 16-Bit Saturating Arithmetic Bound

Fact: 16-bit signed saturating addition lhs +| rhs with bounded interval 350i16 <= sum <= 600i16 establishes 350i16 <= sum <= 600i16 across all nine places so exact subtractions 600i16 - sum and sum - 350i16 are safe.

Owning Task: `SIGNED-16BIT-SATURATING-ARITHMETIC`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md#overflow-policies), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that 16-bit signed saturating addition (`lhs + rhs` on `i16 in Saturating` / `lhs +| rhs`) combined with an explicit bounded interval guard (`350i16 <= sum <= 600i16`) establishes the exact interval (`350i16 <= sum <= 600i16`) across all nine standard places. The compiler statically derives that the dominating guard constrains `sum` to `[350, 600]`, which directly discharges exact downstream 16-bit signed subtractions `sub_hi = 600i16 - value` and `sub_lo = value - 350i16` and downstream contract `requires value >= 350i16 && value <= 600i16` of `verify_sum` without underflow, overflow, or runtime crash traps across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness adding saturating i16 values with interval guard and discharging exact subtractions | [`main.omg`](main.omg) | - |
| local | `local/` | the guarded sum is bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the guarded sum is stored in a SumCarrier record field before extraction and contract verification | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the guarded sum as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the guarded sum under postcondition ensures result >= 350i16 && result <= 600i16 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the guarded sum before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | named constants LHS_VAL and RHS_VAL define operands for saturating addition before guarded subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| another state | `other_state/` | an internal state transition transfers the guarded sum to a successor state with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine forwards the guarded sum to the caller under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies computes saturating addition and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/signed_16bit_saturating_arithmetic_bound/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_arithmetic_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the interval guard is widened to `601i16`, failing to discharge `requires value >= 350i16 && value <= 600i16`. Check each twin with:

```sh
omega --check samples/cli/facts/signed_16bit_saturating_arithmetic_bound/local/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_saturating_arithmetic_bound/field/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_saturating_arithmetic_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_saturating_arithmetic_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_saturating_arithmetic_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_saturating_arithmetic_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_saturating_arithmetic_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_saturating_arithmetic_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_saturating_arithmetic_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_sum from ...: ... (needs fact `value >= 350 && value <= 600` here; establish it before the call via a prior contract guarantee, domain membership, or guard)`

## Requirements and Evidence

- **Parent outcome:** `SIGNED-16BIT-SATURATING-ARITHMETIC` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for 16-bit signed saturating addition (`i16 in Saturating`, `lhs + rhs` / `lhs +| rhs`) constrained by a bounded interval guard (`350i16 <= sum <= 600i16`), discharging exact 16-bit signed subtractions `600i16 - sum` and `sum - 350i16` and downstream contract `requires value >= 350i16 && value <= 600i16` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#overflow-policies), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Performing 16-bit signed addition under the saturating arithmetic domain (`i16 in Saturating`) clamps mathematical sum results to the representable range `[-32768, 32767]`. Dominating transition guard `sum >= 350i16 && sum <= 600i16` establishes the interval `[350, 600]` on the true branch. The Omega arithmetic solver derives this exact range certificate directly from the guard branch condition, verifying that `sum` satisfies `verify_sum(value: i16)` requires contract `value >= 350i16 && value <= 600i16` and discharging exact 16-bit subtractions `600i16 - sum` and `sum - 350i16` without requiring post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The saturating 16-bit signed integer value maintains 2-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions, transitioning cleanly between saturating domain behavior and guarded exact interval carrier identity.
- **Composition:** Guarded saturating addition interval preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the upper bound to `601i16`, causing `verify_sum` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_sum` because `601i16` violates the contract upper bound `value <= 600i16`. The compiler soundly refuses any program where guarded saturating bounds cannot guarantee arrival contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Saturating addition expressions lower into canonical Terminal Psi (`PSITERM\0`) as `SaturatingIntegerAdd` operations without overflow trap edges. Downstream transition guards synthesize integer range certificates retaining `[350, 600]` through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<i16>() == 2`, `align_of<i16>() == 2`, and `size_of<SumCarrier>() == 2`. In `CallPlan` and `StatePlan`, 16-bit signed integers are allocated standard 16-bit general-purpose register lanes (`cx`/`di`/`w0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 16-bit signed saturating addition sequences or clamp routines on x86_64 and arm64 architectures (`sqadd` / scalar clamp). Exact subtraction selects 16-bit `sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Saturating subtraction (`lhs -| rhs`), saturating multiplication, 32-bit saturating arithmetic (`i32 in Saturating`), unsigned saturating arithmetic (`u16 in Saturating`), and SIMD packed vector saturation remain governed under the wider numeric value roadmap.
