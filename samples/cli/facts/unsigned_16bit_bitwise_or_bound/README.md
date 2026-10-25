# Unsigned 16-Bit Bitwise OR Bound

Fact: 16-bit unsigned bitwise OR base | 32u16 with bounded interval 40u16 <= combined <= 63u16 establishes 40u16 <= combined <= 63u16 across all nine places so exact subtractions 63u16 - combined and combined - 40u16 are safe.

Owning Task: `UNSIGNED-16BIT-BITWISE-OR`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that 16-bit unsigned bitwise OR (`base | 32u16`) combined with an explicit bounded interval guard (`40u16 <= combined <= 63u16`) establishes the exact interval (`40u16 <= combined <= 63u16`) across all nine standard places. The compiler statically derives that the dominating guard constrains `combined` to `[40, 63]`, which directly discharges exact downstream 16-bit unsigned subtractions `headroom = 63u16 - combined` and `margin = combined - 40u16` and downstream contract `requires value >= 40u16 && value <= 63u16` of `verify_combined` without underflow, overflow, or runtime crash traps across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin | Obligation Class | Pinned Pass Fixture | Pinned Reject Fixture |
| --- | --- | --- | --- | --- | --- | --- | --- |
| root | `.` | canonical root witness applying 16-bit bitwise OR with interval guard and discharging exact subtractions | [`main.omg`](main.omg) | - | Contract precondition + exact subtraction (`63u16 - combined`, `combined - 40u16`) | - | - |
| local | `local/` | the guarded combined value is bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition (`requires value >= 40u16 && value <= 63u16`) | `tests/omega/pass/facts/unsigned_16bit_bitwise_or_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_or_bound_through_local_rejected` |
| field | `field/` | the guarded combined value is stored in an OrCarrier record field before extraction and contract verification | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition (`requires value >= 40u16 && value <= 63u16`) | `tests/omega/pass/facts/unsigned_16bit_bitwise_or_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_or_bound_through_field_rejected` |
| parameter | `parameter/` | a helper machine receives the guarded combined value as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition (`requires value >= 40u16 && value <= 63u16`) | `tests/omega/pass/facts/unsigned_16bit_bitwise_or_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_or_bound_through_parameter_rejected` |
| call result | `call_result/` | a producer machine returns the guarded combined value under postcondition ensures result >= 40u16 && result <= 63u16 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition (`requires value >= 40u16 && value <= 63u16`) | `tests/omega/pass/facts/unsigned_16bit_bitwise_or_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_or_bound_through_call_result_rejected` |
| match join | `match_join/` | both branches of a boolean match expression yield the guarded combined value before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition (`requires value >= 40u16 && value <= 63u16`) | `tests/omega/pass/facts/unsigned_16bit_bitwise_or_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_or_bound_through_match_join_rejected` |
| const name | `const_name/` | named constants BASE_VAL and OR_VAL define operands for bitwise OR before guarded subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition (`requires value >= 40u16 && value <= 63u16`) | `tests/omega/pass/facts/unsigned_16bit_bitwise_or_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_or_bound_through_const_name_rejected` |
| a nested machine | `other_state/` | an terminal machine transfer transfers the guarded combined value to a successor machine with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition (`requires value >= 40u16 && value <= 63u16`) | `tests/omega/pass/facts/unsigned_16bit_bitwise_or_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_or_bound_through_other_state_rejected` |
| another machine | `other_machine/` | an associated Relay machine forwards the guarded combined value to the caller under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition (`requires value >= 40u16 && value <= 63u16`) | `tests/omega/pass/facts/unsigned_16bit_bitwise_or_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_or_bound_through_other_machine_rejected` |
| another target body | `target_body/` | each of the four hosted target bodies computes bitwise OR and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition (`requires value >= 40u16 && value <= 63u16`) | `tests/omega/pass/facts/unsigned_16bit_bitwise_or_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_or_bound_through_target_body_rejected` |

## Discrimination

- **Runtime oracle discrimination:** Perturbing a single token in [`main.omg`](main.omg) (`transition rc == 0u16` -> `transition rc == 1u16`) causes both engines under `omega run --both` to trap at runtime (`native exit -1073741795`, `interp crash: Trap (host presentation agrees)`), confirming the `0` exit oracle actively discriminates the computed bitwise OR value (`20u16 | 32u16 == 52u16`, `headroom = 63u16 - 52u16 == 11u16`, `margin = 52u16 - 40u16 == 12u16`).
- **Static proof discrimination:** Widening the interval guard upper bound from `63u16` to `64u16` in each `<place>/invalid/main.omg` causes static verification to fail closed with `cannot prove requires contract for call verify_combined`.

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/unsigned_16bit_bitwise_or_bound/main.omg
omega run --both samples/cli/facts/unsigned_16bit_bitwise_or_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_16bit_bitwise_or_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_16bit_bitwise_or_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_16bit_bitwise_or_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_16bit_bitwise_or_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_16bit_bitwise_or_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_16bit_bitwise_or_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_16bit_bitwise_or_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_16bit_bitwise_or_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the interval guard is widened to `64u16` (or constants evaluated to `64u16`), failing to discharge `requires value >= 40u16 && value <= 63u16`. Check each twin with:

```sh
omega --check samples/cli/facts/unsigned_16bit_bitwise_or_bound/local/invalid/main.omg
omega --check samples/cli/facts/unsigned_16bit_bitwise_or_bound/field/invalid/main.omg
omega --check samples/cli/facts/unsigned_16bit_bitwise_or_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/unsigned_16bit_bitwise_or_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/unsigned_16bit_bitwise_or_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/unsigned_16bit_bitwise_or_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/unsigned_16bit_bitwise_or_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/unsigned_16bit_bitwise_or_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/unsigned_16bit_bitwise_or_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_combined`

## Requirements and Evidence

- **Parent outcome:** `UNSIGNED-16BIT-BITWISE-OR` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for 16-bit unsigned bitwise OR (`u16`, `base | 32u16`) constrained by a bounded interval guard (`40u16 <= combined <= 63u16`), discharging exact 16-bit unsigned subtractions `63u16 - combined` and `combined - 40u16` and downstream contract `requires value >= 40u16 && value <= 63u16` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Performing 16-bit unsigned bitwise OR with literal `32u16` (bitmask `0x0020`) sets bit 5 ([numeric values](../../../../wiki/spec/language/numeric_values.md), [invocation contracts](../../../../wiki/spec/language/state_contracts.md), [proof contracts](../../../../wiki/spec/proofs/contracts.md), [domains](../../../../wiki/spec/language/domains.md)). Dominating transition guard `combined >= 40u16 && combined <= 63u16` establishes the bounded interval `[40, 63]` on the true branch. The Omega arithmetic solver derives this exact range certificate directly from the guard condition, verifying that `combined` satisfies `verify_combined(value: u16)` requires contract `value >= 40u16 && value <= 63u16` and discharging exact 16-bit unsigned subtractions `63u16 - combined` and `combined - 40u16` without requiring post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The 16-bit unsigned integer value maintains 2-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions, transitioning cleanly between bitwise OR operations and guarded exact interval carrier identity ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), [lifetimes](../../../../wiki/spec/language/lifetimes.md), [ownership](../../../../wiki/spec/language/ownership.md)).
- **Composition:** Guarded bitwise OR interval preservation composes through all 9 standard places ([machines](../../../../wiki/spec/language/machines.md), [evaluation](../../../../wiki/spec/language/evaluation.md), [patterns](../../../../wiki/spec/language/patterns.md), [structural predicates](../../../../wiki/spec/terminal-psi/structural_predicates.md)): local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the guard to `64u16` (or setting constants yielding `64u16`), causing `verify_combined` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_combined` because `64u16` violates the contract upper bound `value <= 63u16` ([mathematical bindings](../../../../wiki/spec/proofs/mathematical_bindings.md), [authority](../../../../wiki/spec/resources/authority.md)). The compiler soundly refuses any program where bitwise OR bounds cannot guarantee arrival contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Bitwise OR expressions lower into canonical Terminal Psi (`PSITERM\0`) as bitwise OR operations without overflow trap edges ([product](../../../../wiki/spec/terminal-psi/product.md), [encoding](../../../../wiki/spec/terminal-psi/encoding.md), [calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md), [control flow](../../../../wiki/spec/terminal-psi/control_flow.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)). Downstream transition guards synthesize integer range certificates retaining `[40, 63]` through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) ([provider selection](../../../../wiki/spec/build/provider_selection.md), [layout plans](../../../../wiki/spec/layouts/plans.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)). Target layout planning evaluates `size_of<u16>() == 2`, `align_of<u16>() == 2`, and `size_of<OrCarrier>() == 2`. In `CallPlan` and `StatePlan`, 16-bit unsigned integers are allocated 16-bit general-purpose register lanes (`cx`/`di`/`w0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 16-bit unsigned bitwise OR instructions (`or ax, imm16` / `or r16, r16` on x86_64, `orr w0, w0, #imm` / `orr w0, w0, w1` on arm64), and exact subtraction selects 16-bit `sub` ([storage](../../../../wiki/spec/resources/storage.md), [activation storage](../../../../wiki/spec/resources/activation_storage.md), [machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md)). Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`) ([machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md), [executable installation](../../../../wiki/spec/build/executable_installation.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)).

## What remains open

Bitwise OR on signed integers (`i16`), non-constant bitwise operations, bitwise XOR/AND-NOT combinations, and SIMD packed bitwise operations remain governed under the wider numeric value roadmap.
