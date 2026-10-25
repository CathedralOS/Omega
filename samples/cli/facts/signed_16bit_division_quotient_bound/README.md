# Signed 16-Bit Division Quotient Bound

Fact: a 16-bit signed i16 division of bounded inputs (dividend >= 400i16 && dividend <= 400i16, divisor >= 8i16 && divisor <= 8i16) establishes quotient >= 50i16 && quotient <= 50i16 so exact 16-bit signed subtraction quotient - 50i16 is proved overflow-free across all nine places.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`), with `ARITHMETIC-POLICY-REALIZATION` owning the numeric-policy realization
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that a 16-bit signed `i16` division of bounded inputs (`dividend >= 400i16 && dividend <= 400i16` and `divisor >= 8i16 && divisor <= 8i16`) establishes the exact quotient interval (`quotient >= 50i16 && quotient <= 50i16`) across all nine standard places. The compiler statically derives interval division bounds (`[400, 400] / [8, 8] = [50, 50]`), which directly discharges exact downstream subtraction `quotient - 50i16 == 0i16` and the contract `requires q >= 50i16 && q <= 50i16` of `verify_i16_quotient` without underflow, overflow, or runtime guard checks across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness dividing bounded i16 locals and discharging downstream quotient contract | [`main.omg`](main.omg) | - |
| local | `local/` | the division operands and quotient are bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the operands are wrapped in a DivisorCarrier record field before division and quotient check | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the quotient as an explicit parameter under requires contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the computed quotient under postcondition ensures result >= 50i16 && result <= 50i16 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the computed quotient before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | named constants BASE_DIVIDEND and BASE_DIVISOR define bounded i16 operands before division | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | an terminal machine transfer transfers the quotient to successor machine step with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine computes and forwards the quotient under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies computes the quotient and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/signed_16bit_division_quotient_bound/main.omg
omega run --both samples/cli/facts/signed_16bit_division_quotient_bound/local/main.omg
omega run --both samples/cli/facts/signed_16bit_division_quotient_bound/field/main.omg
omega run --both samples/cli/facts/signed_16bit_division_quotient_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_16bit_division_quotient_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_16bit_division_quotient_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_16bit_division_quotient_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_16bit_division_quotient_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_16bit_division_quotient_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_16bit_division_quotient_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the divisor bound is widened to `10i16` (yielding quotient down to `40i16`), failing to discharge `requires q >= 50i16 && q <= 50i16`. Check each twin with:

```sh
omega --check samples/cli/facts/signed_16bit_division_quotient_bound/local/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_division_quotient_bound/field/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_division_quotient_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_division_quotient_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_division_quotient_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_division_quotient_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_division_quotient_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_division_quotient_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_division_quotient_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_i16_quotient from ...: quotient >= 50 && quotient <= 50 (needs fact \`q >= 50 && q <= 50\` here; establish it before the call via a prior contract guarantee, domain membership, or guard)`

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_16bit_division_quotient_bound_through_<place>_exit` (nine positives: `main.omg`, `build.omg`, `expected_native_exit.txt`) and `tests/omega/fail/facts/signed_16bit_division_quotient_bound_through_<place>_rejected` (nine reject twins: `main.omg`, `build.omg`, `expected.txt`). Every fixture is a byte-identical copy of its place program, so the sample and the fixture cannot drift apart. The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Recorded from the shared opt-level 1 producer on macOS arm64: `corpus_outcomes.txt` gains nine `checked` rows carrying `check-target:macos_arm64` and nine `rejected ... expected` rows carrying their own diagnostic line, `corpus_native_macos_arm64.txt` gains nine `built ... exit:0` rows, and `corpus_both_macos_arm64.txt` gains nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`), with `ARITHMETIC-POLICY-REALIZATION` owning the numeric-policy realization.
- **Requirement established:** End-to-end nine-place fact preservation for 16-bit signed `i16` division quotient interval synthesis (`dividend >= 400i16 && dividend <= 400i16`, `divisor >= 8i16 && divisor <= 8i16` -> `quotient >= 50i16 && quotient <= 50i16`), discharging exact downstream subtraction `quotient - 50i16` and contract `requires q >= 50i16 && q <= 50i16` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#exact-subtraction), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Dividing bounded 16-bit signed integers `dividend in [400, 400]` by `divisor in [8, 8]` provably synthesizes the exact quotient interval `[50, 50]`. Subsequent exact subtraction `check = quotient - 50i16` evaluates to `0i16` without overflow or underflow. The Omega arithmetic solver derives this exact range certificate directly from the operand bounds, discharging `verify_i16_quotient(q)` requires contract `q >= 50i16 && q <= 50i16` without post-hoc caller equality guards (`transition x == 50`).
- **Identity and lifetime:** 16-bit signed integer values retain exact 2-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions without loss of sign or precision.
- **Composition:** 16-bit signed division quotient bound preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the divisor bound to `10i16` (quotient reaching `40i16`), causing `verify_i16_quotient` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_i16_quotient ... (needs fact \`q >= 50 && q <= 50\` here)`. The compiler soundly refuses any program where 16-bit division quotient intervals fail to guarantee downstream contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** 16-bit signed division expressions and integer range certificates lower into canonical Terminal Psi (`PSITERM\0`). Division operations preserve integer certificates through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<i16>() == 2`, `align_of<i16>() == 2`, and `size_of<DivisorCarrier>() == 4`. In `CallPlan` and `StatePlan`, 16-bit signed integers are allocated appropriate register or stack lanes with standard integer calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 16-bit signed division instructions: sign extension (`cwd` on x86_64) followed by word-size signed integer division (`idiv`), or signed division (`sdiv`) on arm64 architectures. Exact subtraction selects `subw`/`sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Signed division by negative divisors, dynamic division by non-constant intervals, division remainder bounds, and floating-point quotients remain governed under the wider numeric value roadmap.
