# Signed 8-Bit Shift Left Bound

Fact: an 8-bit signed i8 left shift of a bounded input (val >= 25i8 && val <= 25i8) by 2u32 bits establishes shifted >= 100i8 && shifted <= 100i8 so exact 8-bit signed subtraction shifted - 100i8 is proved overflow-free across all nine places.

Owning Task: `SIGNED-8BIT-SHIFT-LEFT`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that an 8-bit signed `i8` left shift of a bounded input (`val >= 25i8 && val <= 25i8`) by `2u32` bits establishes the exact shifted interval (`shifted >= 100i8 && shifted <= 100i8`) across all nine standard places. The compiler statically derives interval shift bounds (`[25, 25] << 2 = [100, 100]`), verifying that the outcome does not overflow the signed 8-bit range `[-128, 127]`, which directly discharges exact downstream subtraction `shifted - 100i8 == 0i8` and the contract `requires s >= 100i8 && s <= 100i8` of `verify_i8_shifted` without underflow, overflow, or runtime guard checks across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness left-shifting bounded i8 local and discharging downstream shift contract | [`main.omg`](main.omg) | - |
| local | `local/` | the shift operand and shifted result are bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the operand is wrapped in a ShiftCarrier record field before left-shifting and quotient check | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the shifted result as an explicit parameter under requires contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the computed shift under postcondition ensures result >= 100i8 && result <= 100i8 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the computed shift before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | named constants BASE_VAL and SHIFT_BITS define bounded i8 operands before shifting | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| another state | `other_state/` | an internal state transition transfers the shifted value to successor state step with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine computes and forwards the shifted value under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies computes the shift and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/signed_8bit_shift_left_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_shift_left_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_shift_left_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_shift_left_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_shift_left_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_shift_left_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_shift_left_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_shift_left_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_shift_left_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_shift_left_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the input bound is widened to `[20, 25]` (yielding shifted result down to `80i8`), failing to discharge `requires s >= 100i8 && s <= 100i8`. Check each twin with:

```sh
omega --check samples/cli/facts/signed_8bit_shift_left_bound/local/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_shift_left_bound/field/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_shift_left_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_shift_left_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_shift_left_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_shift_left_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_shift_left_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_shift_left_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_shift_left_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_i8_shifted from ...: shifted >= 100 && shifted <= 100 (needs fact \`s >= 100 && s <= 100\` here; establish it before the call via a prior contract guarantee, domain membership, or guard)`

## Pinned Fixtures

The row is pinned under
`tests/omega/pass/facts/signed_8bit_shift_left_bound_through_*_exit` and
`tests/omega/fail/facts/signed_8bit_shift_left_bound_through_*_rejected`. Each
positive fixture is a byte-identical copy of its `<place>/main.omg`; the
place's `build.omg` carries a fixture-unique `Build::application` name, binds
`ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`,
`macos_arm64`, `linux_arm64`), and records native exit 0 with Terminal Psi
agreement. Each reject twin is a byte-identical copy of its
`<place>/invalid/main.omg` and pins its widened-bound (`[20, 25]`) refusal:
`cannot prove requires contract for call verify_i8_shifted` together with the
`shifted >= 100 && shifted <= 100` fact demand and its `s >= 100 && s <= 100`
requirement spelling.

## Requirements and Evidence

- **Parent outcome:** `SIGNED-8BIT-SHIFT-LEFT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for 8-bit signed `i8` left shift bound interval synthesis (`val >= 25i8 && val <= 25i8 << 2u32` -> `shifted >= 100i8 && shifted <= 100i8`), discharging exact downstream subtraction `shifted - 100i8` and contract `requires s >= 100i8 && s <= 100i8` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#exact-subtraction), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Left-shifting bounded 8-bit signed integer `val in [25, 25]` by `2u32` bits provably synthesizes the exact shifted interval `[100, 100]`. Subsequent exact subtraction `check = shifted - 100i8` evaluates to `0i8` without overflow or underflow. The Omega arithmetic solver derives this exact range certificate directly from the operand bounds, verifying that the shifted value does not exceed the signed 8-bit maximum `127i8` and discharging `verify_i8_shifted(s)` requires contract `s >= 100i8 && s <= 100i8` without post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** 8-bit signed integer values retain exact 1-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions without loss of sign or precision.
- **Composition:** 8-bit signed left shift bound preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the value bound to `[20, 25]` (shifted reaching `80i8 < 100i8`), causing `verify_i8_shifted` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_i8_shifted ... (needs fact \`s >= 100 && s <= 100\` here)`. The compiler soundly refuses any program where 8-bit left shift intervals fail to guarantee downstream contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** 8-bit signed left shift expressions and integer range certificates lower into canonical Terminal Psi (`PSITERM\0`). Shift operations preserve integer certificates with shift count bounds (`< 8`) through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<i8>() == 1`, `align_of<i8>() == 1`, and `size_of<ShiftCarrier>() == 1`. In `CallPlan` and `StatePlan`, 8-bit signed integers are allocated appropriate byte register or stack lanes with standard integer calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 8-bit signed left-shift instructions: byte left shift (`shl r8, imm8` on x86_64) or logical shift left (`lsl w, w, #imm`) on arm64 architectures. Exact subtraction selects `subb`/`sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Signed left shifts by dynamic/variable shift amounts, shift right arithmetic modes (`>>`), 8-bit wrapping/saturating shifts, and SIMD packed byte vector shifts remain governed under the wider numeric value roadmap.
