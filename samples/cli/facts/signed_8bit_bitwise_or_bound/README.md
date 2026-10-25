# Signed 8-Bit Bitwise OR Bound

Fact: 8-bit signed bitwise OR base | 32i8 with bounded interval 40i8 <= combined <= 63i8 establishes 40i8 <= combined <= 63i8 across all nine places so exact subtractions 63i8 - combined and combined - 40i8 are safe.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`), the settled proposition-based scalar range contract ("machine/state parameters `requires`, results `ensures`, locals the facts from initializers, guards and calls"); the row's fact family is `SIGNED-8BIT-BITWISE-OR`.
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that 8-bit signed bitwise OR (`base | 32i8`) combined with an explicit bounded interval guard (`40i8 <= combined <= 63i8`) establishes the exact interval (`40i8 <= combined <= 63i8`) across all nine standard places. The compiler statically derives that the dominating guard constrains `combined` to `[40, 63]`, which directly discharges exact downstream 8-bit signed subtractions `headroom = 63i8 - combined` and `margin = combined - 40i8` and downstream contract `requires value >= 40i8 && value <= 63i8` of `verify_combined` without underflow, overflow, or runtime crash traps across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness applying 8-bit signed bitwise OR with interval guard and discharging exact subtractions | [`main.omg`](main.omg) | - |
| local | `local/` | the guarded combined value is bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the guarded combined value is stored in an OrCarrier record field before extraction and contract verification | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the guarded combined value as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the guarded combined value under postcondition ensures result >= 40i8 && result <= 63i8 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the guarded combined value before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | named constants BASE_VAL and OR_VAL define operands for bitwise OR before guarded subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | an terminal machine transfer transfers the guarded combined value to a successor machine with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine forwards the guarded combined value to the caller under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies computes bitwise OR and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/signed_8bit_bitwise_or_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_or_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_or_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_or_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_or_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_or_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_or_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_or_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_or_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_or_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the interval guard is widened to `64i8` (or constants evaluated to `64i8`), failing to discharge `requires value >= 40i8 && value <= 63i8`. Check each twin with:

```sh
omega --check samples/cli/facts/signed_8bit_bitwise_or_bound/local/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_bitwise_or_bound/field/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_bitwise_or_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_bitwise_or_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_bitwise_or_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_bitwise_or_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_bitwise_or_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_bitwise_or_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_bitwise_or_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with the fact's own obligation, authored in each
`<place>/invalid/expected.txt` as:
`cannot prove requires contract for call verify_combined`

(The CLI prints the diagnostic with its `error: ` prefix; the corpus fragment is
the unprefixed spelling so the recorded diagnostic matches it.)

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_8bit_bitwise_or_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_8bit_bitwise_or_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its sample place's
`main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a byte-identical
copy of its place twin's `main.omg`/`build.omg`/`expected.txt` (the row's own `cannot prove requires
contract for call verify_combined` fragment; the nine stale `error: `-prefixed sample fragments were
normalized to the unprefixed spelling the corpus diagnostic matcher compares). Positive versus twin
`git diff` changes only the guard bound (`combined <= 63i8` to `<= 64i8`, with the matching
`requires`/`ensures` bounds in the `parameter`, `call_result`, `other_state` and `other_machine`
places and the `const_name` constants), which is the fact's own domain. Each place and each
`<place>/invalid/` carries the `.gitignore` set the reference row `samples/cli/facts/length/` uses.

Measured on macOS arm64 with the factsr1 shared generation producer `gen-9b001b8f6af`
(revision `9b001b8f6afa`, omega sha256 `6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`,
corpus_runner sha256 `cee00bbdce17fc300b548aca249c2e9e2a44e59c17e97186055da76c3947444e`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 refuse (exit 1) with their authored
  `cannot prove requires contract for call verify_combined` fragment.
- The three corpus legs were recorded with the fixture and golden changes staged at the published
  base `9b001b8f6afa`, so every row stamps `@9b001b8f6afa+wt`: the two pinned rows gained 36 outcome
  rows in `tests/omega/corpus_outcomes.txt` (18 per row: `checked` carrying
  `check-target:macos_arm64` and `rejected` with diagnostic continuation lines), 18 rows in
  `tests/omega/corpus_native_macos_arm64.txt` (`built ... exit:0`) and 18 rows in
  `tests/omega/corpus_both_macos_arm64.txt` (`built ... exit:0 interpreter-exit:0
  interpreter:agrees`). Each golden diff is a pure addition (0 deletions); all three legs exited 0
  with 0 unimplemented rows and 0 rows recording a lost success beyond the pre-existing
  memory-canary pins (2 `regressed-from` rows in the native leg).

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`), the settled proposition-based scalar range contract: "machine/state parameters `requires`, results `ensures`, locals the facts from initializers, guards and calls". This row is one customer group's nine-place transport and execution of that contract for 8-bit signed bitwise OR; the row's fact family is `SIGNED-8BIT-BITWISE-OR`.
- **Requirement established:** End-to-end nine-place fact preservation for 8-bit signed bitwise OR (`i8`, `base | 32i8`) constrained by a bounded interval guard (`40i8 <= combined <= 63i8`), discharging exact 8-bit signed subtractions `63i8 - combined` and `combined - 40i8` and downstream contract `requires value >= 40i8 && value <= 63i8` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Performing 8-bit signed bitwise OR with literal `32i8` (bitmask `0x20`) sets bit 5 while preserving sign bit 7 as 0 for positive operands. Dominating transition guard `combined >= 40i8 && combined <= 63i8` establishes the bounded positive sub-interval `[40, 63]` on the true branch. The Omega arithmetic solver derives this exact range certificate directly from the guard condition, verifying that `combined` satisfies `verify_combined(value: i8)` requires contract `value >= 40i8 && value <= 63i8` and discharging exact 8-bit signed subtractions `63i8 - combined` and `combined - 40i8` without requiring post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The 8-bit signed integer value maintains 1-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions, transitioning cleanly between signed bitwise OR operations and guarded exact interval carrier identity.
- **Composition:** Guarded signed bitwise OR interval preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the guard to `64i8` (or setting constants yielding `64i8`), causing `verify_combined` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_combined` because `64i8` violates the contract upper bound `value <= 63i8`. The compiler soundly refuses any program where bitwise OR bounds cannot guarantee arrival contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Bitwise OR expressions lower into canonical Terminal Psi (`PSITERM\0`) as bitwise OR operations without overflow trap edges. Downstream transition guards synthesize integer range certificates retaining `[40, 63]` through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<i8>() == 1`, `align_of<i8>() == 1`, and `size_of<OrCarrier>() == 1`. In `CallPlan` and `StatePlan`, 8-bit signed integers are allocated 8-bit general-purpose register lanes (`cl`/`dil`/`w0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 8-bit bitwise OR instructions (`or al, imm8` / `or r8, r8` on x86_64, `orr w0, w0, #imm` / `orr w0, w0, w1` on arm64), and exact subtraction selects 8-bit `sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Bitwise OR with negative sign-extended operands (`base < 0`), non-constant bitwise operations, signed shift/rotate bound composition, and SIMD packed byte operations remain governed under the wider numeric value roadmap.
