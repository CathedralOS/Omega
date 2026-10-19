# Unsigned 8-Bit Saturating Subtraction Bound

Fact: 8-bit unsigned saturating subtraction lhs -| rhs with bounded interval 60u8 <= diff <= 160u8 establishes 60u8 <= diff <= 160u8 across all nine places so exact subtractions 160u8 - diff and diff - 60u8 are safe.

Owning Task: `UNSIGNED-8BIT-SATURATING-SUBTRACTION`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md#overflow-policies), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that 8-bit unsigned saturating subtraction (`lhs - rhs` on `u8 in Saturating`) combined with an explicit bounded interval guard (`60u8 <= diff <= 160u8`) establishes the exact interval (`60u8 <= diff <= 160u8`) across all nine standard places. The compiler statically derives that the dominating guard constrains `diff` to `[60, 160]`, which directly discharges exact downstream 8-bit unsigned subtractions `headroom = 160u8 - diff` and `margin = diff - 60u8` and downstream contract `requires value >= 60u8 && value <= 160u8` of `verify_diff` without underflow, overflow, or runtime crash traps across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness subtracting saturating u8 values with interval guard and discharging exact subtractions | [`main.omg`](main.omg) | - |
| local | `local/` | the guarded difference is bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the guarded difference is stored in a DiffCarrier record field before extraction and contract verification | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the guarded difference as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the guarded difference under postcondition ensures result >= 60u8 && result <= 160u8 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the guarded difference before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | named constants LHS_VAL and RHS_VAL define operands for saturating subtraction before guarded subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| another state | `other_state/` | an internal state transition transfers the guarded difference to a successor state with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine forwards the guarded difference to the caller under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies computes saturating subtraction and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the interval guard is widened to `161u8` (or operands adjusted to evaluate to `161u8`), failing to discharge `requires value >= 60u8 && value <= 160u8`. Check each twin with:

```sh
omega --check samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/local/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/field/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_saturating_subtraction_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_diff`

## Requirements and Evidence

- **Parent outcome:** `UNSIGNED-8BIT-SATURATING-SUBTRACTION` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for 8-bit unsigned saturating subtraction (`u8 in Saturating`, `lhs - rhs`) constrained by a bounded interval guard (`60u8 <= diff <= 160u8`), discharging exact 8-bit unsigned subtractions `160u8 - diff` and `diff - 60u8` and downstream contract `requires value >= 60u8 && value <= 160u8` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#overflow-policies), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Performing 8-bit unsigned subtraction under the saturating arithmetic domain (`u8 in Saturating`) clamps the difference at zero when `lhs < rhs`. Dominating transition guard `diff >= 60u8 && diff <= 160u8` establishes the non-saturating interval `[60, 160]` on the true branch. The Omega arithmetic solver derives this exact range certificate directly from the guard branch condition, verifying that `diff` satisfies `verify_diff(value: u8)` requires contract `value >= 60u8 && value <= 160u8` and discharging exact 8-bit unsigned subtractions `160u8 - diff` and `diff - 60u8` without requiring post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The saturating 8-bit unsigned integer value maintains 1-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions, transitioning cleanly between saturating domain behavior and guarded exact interval carrier identity.
- **Composition:** Guarded saturating subtraction interval preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the upper bound to `161u8` (or adjusting operand constants to evaluate to `161u8`), causing `verify_diff` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_diff` because `161u8` violates the contract upper bound `value <= 160u8`. The compiler soundly refuses any program where guarded saturating bounds cannot guarantee arrival contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Saturating subtraction expressions lower into canonical Terminal Psi (`PSITERM\0`) as `SaturatingIntegerSubtract` operations without overflow trap edges. Downstream transition guards synthesize integer range certificates retaining `[60, 160]` through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<u8>() == 1`, `align_of<u8>() == 1`, and `size_of<DiffCarrier>() == 1`. In `CallPlan` and `StatePlan`, 8-bit unsigned integers are allocated standard 8-bit general-purpose register byte lanes (`cl`/`dil`/`w0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 8-bit unsigned saturating subtraction sequences: comparison and subtraction with conditional select (`sub`/`cmovb`) or saturating primitive instructions, and exact subtraction selects 8-bit `sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Signed saturating subtraction (`i8 in Saturating`), 16-bit unsigned saturating subtraction (`u16 in Saturating`), saturating multiplication, and SIMD saturating vector arithmetic remain governed under the wider numeric value roadmap.
