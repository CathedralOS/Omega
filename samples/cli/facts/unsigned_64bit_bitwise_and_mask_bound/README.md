# Unsigned 64-Bit Bitwise AND Mask Bound

Fact: 64-bit unsigned bitwise AND mask raw & 63u64 with bounded interval 40u64 <= masked <= 63u64 establishes 40u64 <= masked <= 63u64 across all nine places so exact subtractions 63u64 - masked and masked - 40u64 are safe.

Owning Task: `UNSIGNED-64BIT-BITWISE-AND-MASK`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that 64-bit unsigned bitwise AND mask (`raw & 63u64`) combined with an explicit bounded interval guard (`40u64 <= masked <= 63u64`) establishes the exact interval (`40u64 <= masked <= 63u64`) across all nine standard places. The compiler statically derives that masking by `63u64` (bitmask `0x3F`) guarantees the result cannot exceed `63u64`, and the dominating guard constrains `masked` to `[40, 63]`. This directly discharges exact downstream 64-bit unsigned subtractions `headroom = 63u64 - masked` and `margin = masked - 40u64` and downstream contract `requires value >= 40u64 && value <= 63u64` of `verify_masked` without underflow, overflow, or runtime crash traps across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness applying 64-bit bitwise AND mask with interval guard and discharging exact subtractions | [`main.omg`](main.omg) | - |
| local | `local/` | the guarded masked value is bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the guarded masked value is stored in a MaskCarrier record field before extraction and contract verification | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the guarded masked value as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the guarded masked value under postcondition ensures result >= 40u64 && result <= 63u64 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the guarded masked value before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | named constants RAW_VAL and MASK_VAL define operands for bitwise AND mask before guarded subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| another state | `other_state/` | an internal state transition transfers the guarded masked value to a successor state with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine forwards the guarded masked value to the caller under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies computes bitwise AND mask and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the mask is widened to `127u64` and the interval guard to `64u64` (or constants evaluated to `64u64`), failing to discharge `requires value >= 40u64 && value <= 63u64`. Check each twin with:

```sh
omega --check samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/local/invalid/main.omg
omega --check samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/field/invalid/main.omg
omega --check samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/unsigned_64bit_bitwise_and_mask_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_masked`

## Requirements and Evidence

- **Parent outcome:** `UNSIGNED-64BIT-BITWISE-AND-MASK` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for 64-bit unsigned bitwise AND mask (`u64`, `raw & 63u64`) constrained by a bounded interval guard (`40u64 <= masked <= 63u64`), discharging exact 64-bit unsigned subtractions `63u64 - masked` and `masked - 40u64` and downstream contract `requires value >= 40u64 && value <= 63u64` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Performing 64-bit unsigned bitwise AND with a constant mask `63u64` (bitmask `0x3F`) guarantees that all higher bits (bits 6..63) are zero, statically bounding the result to `[0, 63]`. Dominating transition guard `masked >= 40u64 && masked <= 63u64` establishes the bounded sub-interval `[40, 63]` on the true branch. The Omega arithmetic solver derives this exact range certificate directly from the mask geometry and guard condition, verifying that `masked` satisfies `verify_masked(value: u64)` requires contract `value >= 40u64 && value <= 63u64` and discharging exact 64-bit unsigned subtractions `63u64 - masked` and `masked - 40u64` without requiring post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The 64-bit unsigned integer value maintains 8-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions, transitioning cleanly between bitwise mask operations and guarded exact interval carrier identity.
- **Composition:** Guarded bitwise AND mask interval preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the mask to `127u64` and guard to `64u64` (or setting constants yielding `64u64`), causing `verify_masked` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_masked` because `64u64` violates the contract upper bound `value <= 63u64`. The compiler soundly refuses any program where bitwise mask bounds cannot guarantee arrival contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Bitwise AND expressions lower into canonical Terminal Psi (`PSITERM\0`) as bitwise AND operations without overflow trap edges. Downstream transition guards synthesize integer range certificates retaining `[40, 63]` through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<u64>() == 8`, `align_of<u64>() == 8`, and `size_of<MaskCarrier>() == 8`. In `CallPlan` and `StatePlan`, 64-bit unsigned integers are allocated standard 64-bit general-purpose register lanes (`rcx`/`rdi`/`x0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 64-bit unsigned bitwise AND instructions (`and r64, imm32` / `and r64, r64` on x86_64, `and xd, xn, #imm` / `and xd, xn, xm` on arm64), and exact subtraction selects 64-bit `sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Bitwise operations on signed integers (`i64`), bitwise OR/XOR/NOT bound tracking, variable-mask bound derivation, and bitfield projection geometry remain governed under the wider bitwise and numeric value roadmap.
