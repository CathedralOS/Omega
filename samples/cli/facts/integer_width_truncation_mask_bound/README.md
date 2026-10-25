# Integer Width Truncation Mask Bound

Fact: masking an unsigned 64-bit integer to its low byte (val & 255u64) establishes the exact byte-width upper bound (masked <= 255u64), discharging exact subtraction 255u64 - masked across all nine places.

Owning Task: `INTEGER-TRUNCATION-MASK`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that masking an unsigned 64-bit integer to its low byte (`val & 255u64`) establishes the exact byte-width upper bound (`masked <= 255u64`) across all nine standard places. The compiler statically derives that clearing the upper 56 bits constrains the scalar to the interval `[0, 255]`, discharging exact unsigned subtraction `255u64 - masked` without underflow or overflow across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Pinned fixtures

The nine positive places and nine reject twins are pinned as
`tests/omega/pass/facts/integer_width_truncation_mask_bound_through_<place>_exit`
and `tests/omega/fail/facts/integer_width_truncation_mask_bound_through_<place>_rejected`.
Each twin authors `expected.txt` carrying the fragment ``may overflow `u64` ``.
`tests/omega/corpus_native_macos_arm64.txt` records nine real
`built ... exit:0` rows, one per place.

### The oracle discriminates

`0` is strictly discriminative in both directions, and the two directions
differ in kind:

- **Runtime trap.** Holding the mask at `255u64` (so the bound and its proof
  still hold) and changing only the input (`466u64` to `467u64`) makes the
  observed `diff` 44 rather than 45. Both engines abort: `native signal 5` and
  `interp crash: Trap`, and the two agree.
- **Static refusal.** Using the masked value where `256u64 <= masked` is
  required refuses at compile time: `cannot prove requires contract for call
  needs_wide from exercise: 256 <= masked`. The bound the mask establishes is
  therefore load-bearing, not decorative.
- **Static refusal on a widened mask or range-admitting contract.** Widening the
  mask to `256u64`/`511u64`, or admitting a `requires`/`ensures` range up to
  `511u64`, defeats the certificate and refuses with
  `exact arithmetic in machine ... may overflow \`u64\``. All nine reject twins
  use this class.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness masking val & 255u64 into a local variable and discharging subtraction | [`main.omg`](main.omg) | - |
| local | `local/` | the masked scalar is bound to a local variable before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the masked scalar is stored in a ByteCarrier record field before extraction | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the masked scalar as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the masked scalar under postcondition ensures result <= 255u64 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield masked byte values before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | a named constant BYTE_MASK defines the 255u64 mask operand before subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | an terminal machine transfer transfers the masked scalar to a successor machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine masks and forwards the scalar to the caller | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies receives the masked scalar and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/integer_width_truncation_mask_bound/main.omg
omega run --both samples/cli/facts/integer_width_truncation_mask_bound/local/main.omg
omega run --both samples/cli/facts/integer_width_truncation_mask_bound/field/main.omg
omega run --both samples/cli/facts/integer_width_truncation_mask_bound/parameter/main.omg
omega run --both samples/cli/facts/integer_width_truncation_mask_bound/call_result/main.omg
omega run --both samples/cli/facts/integer_width_truncation_mask_bound/match_join/main.omg
omega run --both samples/cli/facts/integer_width_truncation_mask_bound/const_name/main.omg
omega run --both samples/cli/facts/integer_width_truncation_mask_bound/other_state/main.omg
omega run --both samples/cli/facts/integer_width_truncation_mask_bound/other_machine/main.omg
omega run --both samples/cli/facts/integer_width_truncation_mask_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the mask or contract bound is widened to `256u64`, causing exact subtraction `255u64 - masked` to fail closed. Check each twin with:

```sh
omega --check samples/cli/facts/integer_width_truncation_mask_bound/local/invalid/main.omg
omega --check samples/cli/facts/integer_width_truncation_mask_bound/field/invalid/main.omg
omega --check samples/cli/facts/integer_width_truncation_mask_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/integer_width_truncation_mask_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/integer_width_truncation_mask_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/integer_width_truncation_mask_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/integer_width_truncation_mask_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/integer_width_truncation_mask_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/integer_width_truncation_mask_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: exact arithmetic in machine ... may overflow \`u64\``

## Requirements and Evidence

- **Parent outcome:** `INTEGER-TRUNCATION-MASK` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for byte-width integer truncation masking (`val & 255u64`) establishing the exact upper bound (`masked <= 255u64`), discharging exact subtraction `255u64 - masked` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#exact-subtraction), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Applying bitwise mask `val & 255u64` to any `u64` value `val` statically guarantees that the high 56 bits are cleared to zero and the result is constrained to `0 <= result <= 255u64`. The Omega arithmetic solver derives this exact range certificate directly from the bitwise AND mask without requiring manual runtime assertions or post-hoc caller equality guards (`transition x == 100`). Exact subtraction `255u64 - masked` is proved non-negative and free of underflow or overflow.
- **Identity and lifetime:** The truncated scalar value `masked` maintains standard 64-bit unsigned integer value identity and carrier representation across expression evaluations, local variable assignments, parameter passing, and state transitions without loss of precision.
- **Composition:** Byte-width mask bound preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant mask application (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the mask or bound to `256u64`, causing `255u64 - masked` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `exact arithmetic in machine ... may overflow \`u64\`` because `255u64 - 256u64` violates the unsigned range invariant for exact subtraction. The compiler soundly refuses any program where masked bounds cannot guarantee non-negative outcomes for exact subtraction.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Bitwise AND expressions and exact range certificates lower cleanly into canonical Terminal Psi (`PSITERM\0`). Integer certificates retain the narrow range `[0, 255]` through intermediate optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<u64>() == 8` and `size_of<ByteCarrier>() == 8`. In `CallPlan` and `StatePlan`, 64-bit integer values pass in ABI general-purpose registers (`rcx`/`rdi`/`x0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 64-bit bitwise AND (`and reg, 255` on x86_64 / `and reg, reg, #255` on arm64 or `movzx` byte register instructions) and 64-bit exact subtraction (`sub`). Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Dynamic symbolic masks with non-constant bit-widths, signed truncation sign-extension bounds, and multi-word integer mask certificates remain governed under the wider `INTEGER-TRUNCATION-MASK` roadmap.
