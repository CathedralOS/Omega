# Signed 8-Bit Division Quotient Bound

Fact: an 8-bit signed i8 division of bounded inputs (dividend >= 90i8 && dividend <= 90i8, divisor >= 3i8 && divisor <= 3i8) establishes quotient >= 30i8 && quotient <= 30i8 so exact 8-bit signed subtraction quotient - 30i8 is proved overflow-free across all nine places.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` ([`TASKS.md`](../../../../TASKS.md))
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that an 8-bit signed `i8` division of bounded inputs (`dividend >= 90i8 && dividend <= 90i8` and `divisor >= 3i8 && divisor <= 3i8`) establishes the exact quotient interval (`quotient >= 30i8 && quotient <= 30i8`) across all nine standard places. The compiler statically derives interval division bounds (`[90, 90] / [3, 3] = [30, 30]`), which directly discharges exact downstream subtraction `quotient - 30i8 == 0i8` and the contract `requires q >= 30i8 && q <= 30i8` of `verify_i8_quotient` without underflow, overflow, or runtime guard checks across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness dividing bounded i8 locals and discharging downstream quotient contract | [`main.omg`](main.omg) | - |
| local | `local/` | the division operands and quotient are bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the operands are wrapped in a DivisorCarrier record field before division and quotient check | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the quotient as an explicit parameter under requires contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the computed quotient under postcondition ensures result >= 30i8 && result <= 30i8 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the computed quotient before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | named constants BASE_DIVIDEND and BASE_DIVISOR define bounded i8 operands before division | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| another state | `other_state/` | an internal state transition transfers the quotient to successor state step with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine computes and forwards the quotient under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies computes the quotient and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/signed_8bit_division_quotient_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_division_quotient_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_division_quotient_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_division_quotient_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_division_quotient_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_division_quotient_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_division_quotient_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_division_quotient_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_division_quotient_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_division_quotient_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the divisor bound is widened to `5i8` (yielding quotient down to `18i8`), failing to discharge `requires q >= 30i8 && q <= 30i8`. Check each twin with:

```sh
omega --check samples/cli/facts/signed_8bit_division_quotient_bound/local/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_division_quotient_bound/field/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_division_quotient_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_division_quotient_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_division_quotient_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_division_quotient_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_division_quotient_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_division_quotient_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/signed_8bit_division_quotient_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_i8_quotient from ...: quotient >= 30 && quotient <= 30 (needs fact \`q >= 30 && q <= 30\` here; establish it before the call via a prior contract guarantee, domain membership, or guard)`

## Pinned Fixtures

The row is pinned under
`tests/omega/pass/facts/signed_8bit_division_quotient_bound_through_<place>_exit`
and
`tests/omega/fail/facts/signed_8bit_division_quotient_bound_through_<place>_rejected`
for the nine places (`local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, `target_body`). Each positive
fixture is a byte-identical copy of its `<place>/main.omg` and `build.omg` plus
`expected_native_exit.txt` = 0; it binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and
records native exit 0 with Terminal Psi agreement. Each reject twin is a
byte-identical copy of its `<place>/invalid/main.omg` and `build.omg` plus the
authored `expected.txt` fragments
`cannot prove requires contract for call verify_i8_quotient` and
`q >= 30 && q <= 30`; positive versus twin `git diff` changes only the
`divisor` bound (`divisor <= 3i8` to `divisor <= 5i8`, or
`const BASE_DIVISOR: i8 = 3i8` to `5i8`) with the matching intermediate
contract endpoint, which is the fact's own domain.

Measured on macOS arm64 with the factsr1 shared generation producer
`gen-9b001b8f6af` (omega sha256
`6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`,
corpus_runner sha256
`cee00bbdce17fc300b548aca249c2e9e2a44e59c17e97186055da76c3947444e`):

- `omega run --both` over the root and the nine places: 10/10 report
  `native exit 0` and `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 refuse with
  `cannot prove requires contract for call verify_i8_quotient ...: quotient >= 30 && quotient <= 30 (needs fact \`q >= 30 && q <= 30\` here; ...)`.
- The three corpus legs were recorded with the fixture and golden changes
  staged at the published wave base `9b001b8f6afa`, so every row is stamped
  `@9b001b8f6afa+wt`: `tests/omega/corpus_outcomes.txt` gained 36 rows
  (18 `checked` carrying `check-target:macos_arm64` and 18 `rejected` with
  their diagnostic lines), `tests/omega/corpus_native_macos_arm64.txt` gained
  18 `built ... exit:0` rows, and `tests/omega/corpus_both_macos_arm64.txt`
  gained 18 `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows.
  Every golden diff is a pure addition (numstat 60/0, 18/0, 18/0), all three
  legs recorded 0 unimplemented rows, and no added row records a lost success.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): "machine/state parameters `requires`, results `ensures`, locals the facts from initializers, guards and calls". This row is one customer group's nine-place transport and execution of that settled proposition-based scalar range contract; `ARITHMETIC-POLICY-REALIZATION` owns the underlying numeric-policy realization.
- **Requirement established:** End-to-end nine-place fact preservation for 8-bit signed `i8` division quotient interval synthesis (`dividend >= 90i8 && dividend <= 90i8`, `divisor >= 3i8 && divisor <= 3i8` -> `quotient >= 30i8 && quotient <= 30i8`), discharging exact downstream subtraction `quotient - 30i8` and contract `requires q >= 30i8 && q <= 30i8` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#exact-subtraction), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Dividing bounded 8-bit signed integers `dividend in [90, 90]` by `divisor in [3, 3]` provably synthesizes the exact quotient interval `[30, 30]`. Subsequent exact subtraction `check = quotient - 30i8` evaluates to `0i8` without overflow or underflow. The Omega arithmetic solver derives this exact range certificate directly from the operand bounds, discharging `verify_i8_quotient(q)` requires contract `q >= 30i8 && q <= 30i8` without post-hoc caller equality guards (`transition x == 30`).
- **Identity and lifetime:** 8-bit signed integer values retain exact 1-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions without loss of sign or precision.
- **Composition:** 8-bit signed division quotient bound preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the divisor bound to `5i8` (quotient reaching `18i8`), causing `verify_i8_quotient` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_i8_quotient ... (needs fact \`q >= 30 && q <= 30\` here)`. The compiler soundly refuses any program where 8-bit division quotient intervals fail to guarantee downstream contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** 8-bit signed division expressions and integer range certificates lower into canonical Terminal Psi (`PSITERM\0`). Division operations preserve integer certificates through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<i8>() == 1`, `align_of<i8>() == 1`, and `size_of<DivisorCarrier>() == 2`. In `CallPlan` and `StatePlan`, 8-bit signed integers are allocated appropriate byte register or stack lanes with standard integer calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 8-bit signed division instructions: byte-to-word sign extension (`cbw` into `ax` on x86_64) followed by byte-size signed integer division (`idiv r8`), or sign-extended word division (`sdiv w`) on arm64 architectures. Exact subtraction selects `subb`/`sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Signed division by negative divisors, dynamic division by non-constant intervals, 8-bit division remainder bounds, and floating-point quotients remain governed under the wider numeric value roadmap.
