# Narrow to Narrow Truncation 64-to-16 Mask Bound

Fact: narrowing a masked unsigned 64-bit integer to u16 and zero-extending back to u64 ((val & 63u64) as u16 as u64) preserves the exact mask upper bound (truncated <= 63u64), discharging exact subtraction 63u64 - truncated across all nine places.

Owning Task: `INTEGER-TRUNCATION-MASK`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that narrowing a masked unsigned 64-bit integer to `u16` and zero-extending back to `u64` (`(val & 63u64) as u16 as u64`) preserves the exact mask upper bound (`truncated <= 63u64`) across all nine standard places. The compiler statically derives that clearing the upper 58 bits constrains the scalar to `[0, 63]`, which fits strictly within `u16` (`[0, 65535]`) across the narrowing truncation `as u16` and subsequent zero-extension `as u64`, discharging exact unsigned subtraction `63u64 - truncated` without underflow or overflow across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Pinned fixtures

The nine positive places and nine reject twins are pinned as
`tests/omega/pass/facts/narrow_to_narrow_truncation_64to16_mask_bound_through_<place>_exit`
and `tests/omega/fail/facts/narrow_to_narrow_truncation_64to16_mask_bound_through_<place>_rejected`.
Each twin authors `expected.txt` carrying the fragment ``may overflow `u64` ``.
`tests/omega/corpus_native_windows_x86_64.txt` records nine real
`built ... exit:0` rows, one per place.

### The oracle discriminates

`0` is strictly discriminative in both directions, and the two directions
differ in kind:

- **Runtime trap.** Holding the mask at `63u64` (so the bound and its proof
  still hold) and changing only the input (`466u64` to `467u64`) makes the
  observed `diff` 44 rather than 45. Both engines abort: `native exit` / trap and
  `interp crash: Trap`, and the two agree.
- **Static refusal.** Using the truncated value where `64u64 <= truncated` is
  required refuses at compile time: `cannot prove requires contract for call
  needs_wide from exercise: 64 <= truncated`. The bound the masked truncation
  establishes is therefore load-bearing, not decorative.
- **Static refusal on a widened mask or range-admitting contract.** Widening the
  mask to `64u64`, or admitting a `requires`/`ensures` range up to `64u64`,
  defeats the certificate and refuses with
  `exact arithmetic in machine ... may overflow \`u64\``. All nine reject twins
  use this class.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness truncating (val & 63u64) as u16 as u64 via helper call and discharging subtraction | [`main.omg`](main.omg) | - |
| local | `local/` | the masked and u16-truncated scalar is bound to a local variable before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the masked and u16-truncated scalar is stored in a HalfwordCarrier record field before extraction | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the masked and u16-truncated scalar as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the masked and u16-truncated scalar under postcondition ensures result <= 63u64 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield masked u16-truncated values before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | a named constant HALFWORD_MASK defines the 63u64 mask operand before u16 truncation and subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | an terminal machine transfer transfers the masked and u16-truncated scalar to a successor machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine truncates and forwards the masked scalar to the caller | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies receives the masked and u16-truncated scalar and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/local/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/field/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the mask or contract bound is widened to `64u64`, causing exact subtraction `63u64 - truncated` to fail closed. Check each twin with:

```sh
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/local/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/field/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to16_mask_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: exact arithmetic in machine ... may overflow \`u64\``

## Requirements and Evidence

- **Parent outcome:** `INTEGER-TRUNCATION-MASK` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for 64-to-16-bit narrowing integer truncation with mask-bounded interval preservation (`(val & 63u64) as u16 as u64`) establishing the exact upper bound (`truncated <= 63u64`), discharging exact subtraction `63u64 - truncated` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#exact-subtraction), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Applying bitwise mask `val & 63u64` to any `u64` value `val` constrains the value to `0 <= x <= 63u64`, and narrowing truncation `as u16` followed by zero-extension `as u64` preserves that exact interval `[0, 63]` since `63 <= 65535`. The Omega arithmetic solver derives this exact range certificate directly from the mask and narrowing/widening casts without requiring manual runtime assertions or post-hoc caller equality guards (`transition x == 100`). Exact subtraction `63u64 - truncated` is proved non-negative and free of underflow or overflow.
- **Identity and lifetime:** The truncated scalar value `truncated` transitions cleanly from a 64-bit masked value through a 16-bit halfword carrier (`u16`) back to a 64-bit unsigned integer (`u64`) without loss of value identity across expression evaluations, local variable assignments, parameter passing, and state transitions.
- **Composition:** Masked 64-to-16-bit truncation bound preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant mask application (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the mask or bound to `64u64`, causing `63u64 - truncated` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `exact arithmetic in machine ... may overflow \`u64\`` because `63u64 - 64u64` violates the unsigned range invariant for exact subtraction. The compiler soundly refuses any program where masked truncation bounds cannot guarantee non-negative outcomes for exact subtraction.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Bitwise AND expressions, narrowing `as u16` casts, widening `as u64` zero-extension casts, and exact range certificates lower cleanly into canonical Terminal Psi (`PSITERM\0`). Integer certificates retain the narrow range `[0, 63]` through intermediate optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<u16>() == 2`, `size_of<u64>() == 8`, and `size_of<HalfwordCarrier>() == 8`. In `CallPlan` and `StatePlan`, 64-bit integer values pass in ABI general-purpose registers (`rcx`/`rdi`/`x0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 64-bit bitwise AND (`and reg, 63`), 64-to-16 truncation and 16-to-64 zero-extension (`movzx r64, r16` on x86_64 / `uxth` on arm64), and 64-bit exact subtraction (`sub`). Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Dynamic symbolic masks with non-constant bit-widths, signed truncation sign-extension bounds, and multi-word integer mask certificates remain governed under the wider `INTEGER-TRUNCATION-MASK` roadmap.
