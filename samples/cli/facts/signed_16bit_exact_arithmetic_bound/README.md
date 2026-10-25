# Signed 16-Bit Exact Arithmetic Bound

Fact: adding two bounded 16-bit signed i16 values (lhs >= 100i16 && lhs <= 200i16 and rhs >= 150i16 && rhs <= 250i16) establishes the exact post-addition interval (sum >= 250i16 && sum <= 450i16), discharging a downstream contract requiring sum >= 250i16 && sum <= 450i16 (and exact subtraction 450i16 - sum) across all nine places.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`); `ARITHMETIC-POLICY-REALIZATION` owns the underlying numeric-policy realization.
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that adding two bounded 16-bit signed `i16` integers (`lhs >= 100i16 && lhs <= 200i16` and `rhs >= 150i16 && rhs <= 250i16`) establishes the exact post-addition interval (`sum >= 250i16 && sum <= 450i16`) across all nine standard places. The compiler statically derives interval addition bounds (`[100, 200] + [150, 250] = [250, 450]`), which directly discharges exact downstream subtraction `margin = 450i16 - sum` and the contract `requires v >= 0i16 && v <= 200i16` of `verify_i16_margin` without underflow, overflow, or runtime guard checks across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness adding bounded i16 locals and discharging downstream margin contract | [`main.omg`](main.omg) | - |
| local | `local/` | the addition operands and sum are bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the operands are wrapped in an OperandCarrier record field before addition and margin subtraction | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the sum as an explicit parameter under requires contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the computed sum under postcondition ensures result >= 250i16 && result <= 450i16 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the computed sum before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | named constants BASE_LHS and BASE_RHS define bounded i16 operands before addition | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | an terminal machine transfer transfers the sum to successor machine step with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine computes and forwards the sum under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies computes the sum and discharges margin subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/signed_16bit_exact_arithmetic_bound/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_arithmetic_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the rhs bound (and, where the place restates it, the consumer contract) is widened to `251i16` (`sum <= 451i16`), causing `margin = 450i16 - sum` to reach `-1i16` and failing to discharge `requires v >= 0i16 && v <= 200i16`; `const_name` widens its named operand constants to `200i16`/`251i16`, because that place has no parameter bound. The concrete inputs stay `150i16`/`200i16` in every twin, so each positive-vs-twin `git diff` changes only the bound or the constants. Check each twin with:

```sh
omega --check samples/cli/facts/signed_16bit_exact_arithmetic_bound/local/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_exact_arithmetic_bound/field/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_exact_arithmetic_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_exact_arithmetic_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_exact_arithmetic_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_exact_arithmetic_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_exact_arithmetic_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_exact_arithmetic_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/signed_16bit_exact_arithmetic_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_i16_margin from ...: margin >= 0 && margin <= 200 (needs fact \`v >= 0 && v <= 200\` here; establish it before the call via a prior contract guarantee, domain membership, or guard)`

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_16bit_exact_arithmetic_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_16bit_exact_arithmetic_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its sample place's
`main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a byte-identical copy
of its place twin's `main.omg`/`build.omg`/`expected.txt` (`cannot prove requires contract for call
verify_i16_margin` and `v >= 0 && v <= 200`). Positive versus twin `git diff` changes only the `rhs`
bound (`rhs <= 250i16` to `rhs <= 251i16`, with the restated consumer contract widened to `451i16`
where the place carries one), or - in `const_name` - the named operand constants (`150i16`/`200i16` to
`200i16`/`251i16`), which is the fact's own domain.

Re-measured on macOS arm64 with the factsr1 shared generation producer `gen-9b001b8f6af` (omega sha256
`6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`, corpus_runner sha256
`cee00bbdce17fc300b548aca249c2e9e2a44e59c17e97186055da76c3947444e`) after psi integer-range literal
handling, the scalar-predicate projection and the where-fact-interval proof paths moved past the
previous generation (gen-4c23cc3391c); the same 20 positive places and 18 twins were re-run against it.

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 refuse with `cannot prove requires contract for call
  verify_i16_margin ... (needs fact \`v >= 0 && v <= 200\` here)`.
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `9b001b8f6afa`, so every row is stamped `@9b001b8f6afa+wt`: `tests/omega/corpus_outcomes.txt` gained
  60 lines (36 rows: 18 `checked` carrying `check-target:macos_arm64` and 18 `rejected` with 24
  diagnostic continuation lines), `tests/omega/corpus_native_macos_arm64.txt` gained 18 `built ...
  exit:0` rows and `tests/omega/corpus_both_macos_arm64.txt` gained 18 `built ... exit:0
  interpreter-exit:0 interpreter:agrees` rows. Every golden diff is a pure addition (0 deletions); all
  three legs exited 0 with 0 unimplemented rows and 0 rows recording a lost success.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): "machine/state parameters `requires`, results `ensures`, locals the facts from initializers, guards and calls". This row is one customer group's nine-place transport and execution of that settled proposition-based scalar range contract; `ARITHMETIC-POLICY-REALIZATION` owns the underlying numeric-policy realization.
- **Requirement established:** End-to-end nine-place fact preservation for 16-bit signed `i16` addition interval synthesis (`lhs >= 100i16 && lhs <= 200i16` + `rhs >= 150i16 && rhs <= 250i16` -> `sum >= 250i16 && sum <= 450i16`) discharging exact subtraction `450i16 - sum` and downstream contract `v >= 0i16 && v <= 200i16` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#exact-subtraction), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Adding two bounded 16-bit signed integers `lhs in [100, 200]` and `rhs in [150, 250]` provably synthesizes the exact sum interval `[250, 450]`. Subsequent exact subtraction `margin = 450i16 - sum` inverts the interval to `[450 - 450, 450 - 250] = [0, 200]`. The Omega arithmetic solver derives this exact range certificate directly from the operand bounds, discharging `verify_i16_margin(v)` requires contract `v >= 0i16 && v <= 200i16` without post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** 16-bit signed integer values retain exact 2-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions without loss of sign or precision.
- **Composition:** 16-bit signed interval addition preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the rhs bound or postcondition to `251i16` (sum `<= 451i16`), causing `margin` to reach `-1i16` and failing closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_i16_margin ... (needs fact \`v >= 0 && v <= 200\` here)`. The compiler soundly refuses any program where 16-bit interval bounds fail to discharge the downstream requires contract.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** 16-bit signed arithmetic expressions and interval certificates lower into canonical Terminal Psi (`PSITERM\0`). Integer certificates retain the exact signed bounds `[250, 450]` and margin bounds `[0, 200]` through intermediate optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<i16>() == 2`, `align_of<i16>() == 2`, and `size_of<OperandCarrier>() == 4`. In `CallPlan` and `StatePlan`, 16-bit signed integers are allocated appropriate register or stack lanes with standard integer calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 16-bit signed addition and subtraction instructions (`add` / `sub` with word operand size or sign-extended register forms). Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Signed 16-bit wrapping/saturating arithmetic modes, non-constant dynamic interval bounds, and SIMD packed 16-bit integer vector operations remain governed under the wider numeric value roadmap.
