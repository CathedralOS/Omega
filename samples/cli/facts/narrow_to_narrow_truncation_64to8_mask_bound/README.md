# Narrow-to-Narrow Integer Truncation (64-to-8 Mask Bound)

Fact: masking an unsigned 64-bit integer to 5 bits and narrowing through u8 ((x & 31u64) as u8 as u64) preserves the exact upper bound (truncated <= 31u64), discharging exact subtraction 31u64 - truncated across all nine places.

Owning Task: `INTEGER-TRUNCATION-MASK`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that masking an unsigned 64-bit integer to 5 bits and narrowing through `u8` (`(x & 31u64) as u8 as u64`) preserves the exact upper bound (`truncated <= 31u64`) across all nine standard places. Because the masked interval `[0, 31]` lies strictly within the `u8` domain `[0, 255]`, narrowing to `u8` and widening back to `u64` preserves the exact sub-byte upper bound `31u64` rather than widening the certificate to `255u64`, discharging exact unsigned subtraction `31u64 - truncated` without underflow or overflow across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Pinned fixtures

The nine positive places and nine reject twins are pinned as
`tests/omega/pass/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_<place>_exit`
and `tests/omega/fail/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_<place>_rejected`.
Each twin authors `expected.txt` carrying the fragment ``may overflow `u64` ``.
`tests/omega/corpus_native_windows_x86_64.txt` records nine real
`built ... exit:0` rows, one per place.

### The oracle discriminates

`0` is strictly discriminative in both directions, and the two directions
differ in kind:

- **Runtime trap.** Holding the mask at `31u64` (so the bound and its proof
  still hold) and changing only the input (`466u64` to `467u64`) makes the
  observed `diff` 12 rather than 13. Both engines abort with `Trap`, and the
  two agree.
- **Static refusal.** Widening the mask or contract bound from `31u64` to
  `32u64` (`(x & 32u64) as u8 as u64`, still inside `u8` range `[0, 255]` but
  exceeding `31u64`) defeats the subtraction certificate and refuses at compile
  time with `exact arithmetic in machine ... may overflow \`u64\``. All nine
  reject twins use this class.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness masking and narrowing `(x & 31u64) as u8 as u64` and discharging subtraction | [`main.omg`](main.omg) | - |
| local | `local/` | the narrowed byte-truncated scalar is bound to a local variable before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the narrowed byte-truncated scalar is stored in a ByteCarrier record field before extraction | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the narrowed byte-truncated scalar as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the narrowed byte-truncated scalar under postcondition `ensures result <= 31u64` | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield narrowed byte-truncated values before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | a named constant `MASK_5BIT` defines the `31u64` mask operand before byte truncation and subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | an terminal machine transfer transfers the narrowed byte-truncated scalar to a successor machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated `Relay` machine masks, narrows through `u8`, and forwards the scalar to the caller | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies receives the narrowed byte-truncated scalar and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/local/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/field/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the mask or contract bound is widened to `32u64`, causing exact subtraction `31u64 - truncated` to fail closed. Check each twin with:

```sh
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/local/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/field/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/narrow_to_narrow_truncation_64to8_mask_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: exact arithmetic in machine ... may overflow \`u64\``

## Requirements and Evidence

- **Parent outcome:** `INTEGER-TRUNCATION-MASK` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for 64-to-8-bit narrowing integer truncation of a mask-bounded scalar (`(x & 31u64) as u8 as u64`) preserving the exact sub-byte upper bound (`truncated <= 31u64`), discharging exact subtraction `31u64 - truncated` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#exact-subtraction), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Masking `x & 31u64` constrains the operand to `[0, 31]`. Narrowing that value to `u8` (`as u8`) and widening to `u64` (`as u64`) preserves the tighter interval `[0, 31]` because `31 <= 255`. The Omega arithmetic solver derives `truncated <= 31u64` directly from the masked narrowing cast chain without post-hoc caller guards, discharging exact subtraction `31u64 - truncated`.
- **Identity and lifetime:** The narrowed and re-widened scalar `truncated` maintains its exact value and 64-bit unsigned carrier representation across local bindings, record fields, parameters, call results, match joins, and state transitions.
- **Composition:** Sub-byte mask bound preservation across `u64 -> u8 -> u64` truncation composes through all 9 standard places: `local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, and `target_body`. Each `<place>/invalid/main.omg` differs solely in widening the mask or contract bound to `32u64`, causing `31u64 - truncated` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `exact arithmetic in machine ... may overflow \`u64\`` because `31u64 - 32u64` violates the unsigned range invariant for exact subtraction.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Bitwise AND, `u64 -> u8` narrowing truncation, `u8 -> u64` zero extension, and exact range certificates lower cleanly into canonical Terminal Psi (`PSITERM\0`), and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<u64>() == 8` and `size_of<ByteCarrier>() == 8`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 64-bit bitwise AND, byte-lane truncation / zero-extension (`movzx` on x86_64 / `uxtb` on arm64), and 64-bit exact subtraction (`sub`), allocating disjoint stack activation homes within bounded WCSU claims.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Signed narrowing truncation across sub-byte mask bounds and dynamic non-constant mask widths remain governed under the wider `INTEGER-TRUNCATION-MASK` roadmap.
