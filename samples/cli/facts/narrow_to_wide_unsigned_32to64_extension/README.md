# Narrow to Wide Unsigned 32-to-64 Extension

Fact: zero-extending a bounded 32-bit unsigned u32 value (narrow >= 70000u32 && narrow <= 90000u32) to u64 (narrow as u64) preserves the exact interval (wide >= 70000u64 && wide <= 90000u64), discharging exact 64-bit subtractions 90000u64 - wide and wide - 70000u64 across all nine places.

Owning Task: `NARROW-WIDE-U32-TO-U64`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that zero-extending a bounded 32-bit unsigned `u32` value (`narrow >= 70000u32 && narrow <= 90000u32`) to `u64` (`narrow as u64`) preserves the exact interval (`wide >= 70000u64 && wide <= 90000u64`) across all nine standard places. The compiler statically derives that zero-extension across the upper-32 boundary into 64 bits preserves non-negativity and bounds, verifying that `wide` falls strictly within `[70000, 90000]`, which directly discharges exact downstream 64-bit subtractions `90000u64 - wide` and `wide - 70000u64` without underflow or overflow across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Pinned fixtures

The nine positive places and nine reject twins are pinned as
`tests/omega/pass/facts/narrow_to_wide_unsigned_32to64_extension_through_<place>_exit`
and `tests/omega/fail/facts/narrow_to_wide_unsigned_32to64_extension_unproved_at_<place>`.
Each twin authors `expected.txt` carrying the fragment ``may overflow `u64` ``.
`tests/omega/corpus_native_windows_x86_64.txt` records nine real
`built ... exit:0` rows, one per place.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness zero-extending bounded u32 local into u64 and discharging exact 64-bit subtractions | [`main.omg`](main.omg) | - |
| local | `local/` | the bounded u32 is zero-extended to a u64 local variable before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the bounded u32 is wrapped in a NarrowCarrier record field before extraction and extension to u64 | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the zero-extended u64 scalar as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the zero-extended u64 scalar under postcondition ensures result >= 70000u64 && result <= 90000u64 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the zero-extended u64 scalar before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | named constant BASE_NARROW defines a bounded u32 operand before zero-extension | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | an terminal machine transfer transfers the zero-extended u64 scalar to a successor machine with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine zero-extends and forwards the u64 scalar to the caller under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies zero-extends the u32 scalar and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the narrow bound is widened to `90001u32` (`90001u64`), causing exact subtraction `90000u64 - wide` to fail closed. Check each twin with:

```sh
omega --check samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/local/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/field/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/parameter/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/call_result/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/match_join/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/const_name/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/other_state/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/other_machine/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_unsigned_32to64_extension/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: exact arithmetic in machine ... may overflow `u64``

## Requirements and Evidence

- **Parent outcome:** `NARROW-WIDE-U32-TO-U64` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for narrow-to-wide unsigned 32-bit to 64-bit zero extension (`narrow as u64`) preserving the exact interval (`wide >= 70000u64 && wide <= 90000u64`), discharging exact 64-bit subtractions `90000u64 - wide` and `wide - 70000u64` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#narrow-to-wide-signzero-extension), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Zero-extending bounded 32-bit unsigned integer `narrow in [70000, 90000]` to `u64` via `narrow as u64` provably preserves the exact interval `[70000, 90000]` on the 64-bit destination. Exact subtractions `90000u64 - wide` and `wide - 70000u64` evaluate without underflow or overflow. The Omega arithmetic solver derives this exact range certificate directly from the zero-extension operation without requiring manual runtime assertions or post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The zero-extended scalar transitions from a 4-byte stored width representation (`u32`) to an 8-byte stored width representation (`u64`) with upper 32 bits statically guaranteed zero, maintaining 64-bit unsigned integer value identity across expression evaluations, local variable assignments, parameter passing, and state transitions.
- **Composition:** Zero-extension bound preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the upper bound to `90001u32` (`90001u64`), causing exact subtraction `90000u64 - wide` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `exact arithmetic in machine ... may overflow `u64`` because `90000u64 - 90001u64` violates the unsigned non-negative range invariant for exact subtraction. The compiler soundly refuses any program where zero-extended bounds cannot guarantee non-negative outcomes for exact subtraction.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Zero-extension cast expressions (`as u64`) and integer range certificates lower cleanly into canonical Terminal Psi (`PSITERM\0`). The narrow range `[70000, 90000]` is preserved in the Terminal Psi integer certificate through intermediate optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<u32>() == 4`, `align_of<u32>() == 4`, `size_of<u64>() == 8`, `align_of<u64>() == 8`, and `size_of<NarrowCarrier>() == 4`. In `CallPlan` and `StatePlan`, 32-bit values pass in 32-bit register/stack lanes and widen to 64-bit general-purpose registers (`rcx`/`rdi`/`x0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects unsigned 32-to-64-bit zero-extension across the upper-32 boundary: 32-bit register moves (`mov r32, r/m32` implicitly zeroing bits 32..63 of the 64-bit register) on x86_64 and unsigned word extension (`uxtw x, w` or 32-bit `w` register write zeroing bits 32..63 of `x`) on arm64 architectures. Exact subtraction selects 64-bit `sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Signed 32-to-64 sign extension (`i32 as i64`), narrow-to-wide floating point conversions, and SIMD vector integer widening remain governed under the wider numeric value roadmap.
