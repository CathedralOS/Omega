# Unsigned 8-Bit Bitwise OR Bound

Fact: 8-bit unsigned bitwise OR base | 32u8 with bounded interval 40u8 <= combined <= 63u8 establishes 40u8 <= combined <= 63u8 across all nine places so exact subtractions 63u8 - combined and combined - 40u8 are safe.

Owning Task: `UNSIGNED-8BIT-BITWISE-OR`
Claim Ticket: `13fc6f8b7601447a964ae4199dcb7194` (`antigravity/w39-2`)
Obligation Class: Contract precondition (`requires value >= 40u8 && value <= 63u8` on `verify_combined`)
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that 8-bit unsigned bitwise OR (`base | 32u8`) combined with an explicit bounded interval guard (`40u8 <= combined <= 63u8`) establishes the exact interval (`40u8 <= combined <= 63u8`) across all nine standard places. The compiler statically derives that the dominating guard constrains `combined` to `[40, 63]`, which directly discharges exact downstream 8-bit unsigned subtractions `headroom = 63u8 - combined` and `margin = combined - 40u8` and downstream contract `requires value >= 40u8 && value <= 63u8` of `verify_combined` without underflow, overflow, or runtime crash traps across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin | Pinned Corpus Pass | Pinned Corpus Fail |
| --- | --- | --- | --- | --- | --- | --- |
| root | `.` | canonical root witness applying 8-bit bitwise OR with interval guard and discharging exact subtractions | [`main.omg`](main.omg) | - | - | - |
| local | `local/` | the guarded combined value is bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_or_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_or_bound_through_local_rejected` |
| field | `field/` | the guarded combined value is stored in an OrCarrier record field before extraction and contract verification | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_or_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_or_bound_through_field_rejected` |
| parameter | `parameter/` | a helper machine receives the guarded combined value as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_or_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_or_bound_through_parameter_rejected` |
| call result | `call_result/` | a producer machine returns the guarded combined value under postcondition ensures result >= 40u8 && result <= 63u8 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_or_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_or_bound_through_call_result_rejected` |
| match join | `match_join/` | both branches of a boolean match expression yield the guarded combined value before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_or_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_or_bound_through_match_join_rejected` |
| const name | `const_name/` | named constants BASE_VAL and OR_VAL define operands for bitwise OR before guarded subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_or_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_or_bound_through_const_name_rejected` |
| a nested machine | `other_state/` | an terminal machine transfer transfers the guarded combined value to a successor machine with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_or_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_or_bound_through_other_state_rejected` |
| another machine | `other_machine/` | an associated Relay machine forwards the guarded combined value to the caller under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_or_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_or_bound_through_other_machine_rejected` |
| another target body | `target_body/` | each of the four hosted target bodies computes bitwise OR and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_or_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_or_bound_through_target_body_rejected` |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/unsigned_8bit_bitwise_or_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_or_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_or_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_or_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_or_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_or_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_or_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_or_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_or_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_or_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`. Perturbing the demanded oracle check in `main.omg` (`transition rc == 0u8` -> `transition rc == 1u8`) causes both engines to trap (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`), proving that the oracle discriminates.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the interval guard is widened to `64u8` (or constants evaluated to `64u8`), failing to discharge `requires value >= 40u8 && value <= 63u8`. Check each twin with:

```sh
omega --check samples/cli/facts/unsigned_8bit_bitwise_or_bound/local/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_bitwise_or_bound/field/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_bitwise_or_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_bitwise_or_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_bitwise_or_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_bitwise_or_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_bitwise_or_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_bitwise_or_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/unsigned_8bit_bitwise_or_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_combined`

## Requirements and Evidence

- **Parent outcome:** `UNSIGNED-8BIT-BITWISE-OR` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for 8-bit unsigned bitwise OR (`u8`, `base | 32u8`) constrained by a bounded interval guard (`40u8 <= combined <= 63u8`), discharging exact 8-bit unsigned subtractions `63u8 - combined` and `combined - 40u8` and downstream contract `requires value >= 40u8 && value <= 63u8` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Performing 8-bit unsigned bitwise OR with literal `32u8` (bitmask `0x20`) sets bit 5. Dominating transition guard `combined >= 40u8 && combined <= 63u8` establishes the bounded interval `[40, 63]` on the true branch. The Omega arithmetic solver derives this exact range certificate directly from the guard condition, verifying that `combined` satisfies `verify_combined(value: u8)` requires contract `value >= 40u8 && value <= 63u8` and discharging exact 8-bit unsigned subtractions `63u8 - combined` and `combined - 40u8` without requiring post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The 8-bit unsigned integer value maintains 1-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions, transitioning cleanly between bitwise OR operations and guarded exact interval carrier identity.
- **Composition:** Guarded bitwise OR interval preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the guard to `64u8` (or setting constants yielding `64u8`), causing `verify_combined` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_combined` because `64u8` violates the contract upper bound `value <= 63u8`. The compiler soundly refuses any program where bitwise OR bounds cannot guarantee arrival contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Bitwise OR expressions lower into canonical Terminal Psi (`PSITERM\0`) as bitwise OR operations without overflow trap edges. Downstream transition guards synthesize integer range certificates retaining `[40, 63]` through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<u8>() == 1`, `align_of<u8>() == 1`, and `size_of<OrCarrier>() == 1`. In `CallPlan` and `StatePlan`, 8-bit unsigned integers are allocated 8-bit general-purpose register lanes (`cl`/`dil`/`w0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 8-bit unsigned bitwise OR instructions (`or al, imm8` / `or r8, r8` on x86_64, `orr w0, w0, #imm` / `orr w0, w0, w1` on arm64), and exact subtraction selects 8-bit `sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Bitwise OR on signed integers (`i8`), non-constant bitwise operations, bitwise XOR/AND-NOT combinations, and SIMD packed bitwise operations remain governed under the wider numeric value roadmap.
