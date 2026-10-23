# Unsigned 32-Bit Wrapping Arithmetic Bound

Fact: 32-bit unsigned wrapping addition lhs +% rhs with a bounded interval guard (50u32 <= sum <= 100u32) establishes 50u32 <= sum <= 100u32 across all nine places so exact subtractions 100u32 - sum and sum - 50u32 are safe.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): this row is one
customer group's nine-place transport and execution of the settled
proposition-based scalar range contract (the dominating guard establishes the
exact interval); `ARITHMETIC-POLICY-REALIZATION` owns the underlying wrapping
numeric-policy realization. Claim Ticket: `40b4111b25a243f38c5229aea497207e`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md#overflow-policies), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that 32-bit unsigned wrapping addition (`lhs + rhs` on `u32 in Wrapping`) combined with an explicit bounded interval guard (`50u32 <= sum <= 100u32`) establishes the exact interval (`50u32 <= sum <= 100u32`) across all nine standard places. The compiler statically derives that the dominating guard constrains `sum` to `[50, 100]`, which directly discharges exact downstream 32-bit unsigned subtractions `headroom = 100u32 - sum` and `margin = sum - 50u32` and downstream contract `requires value >= 50u32 && value <= 100u32` of `verify_sum` without underflow, overflow, or runtime crash traps across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness adding wrapping u32 values with interval guard and discharging exact subtractions | [`main.omg`](main.omg) | - |
| local | `local/` | the guarded sum is bound to local variables before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the guarded sum is stored in a SumCarrier record field before extraction and contract verification | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the guarded sum as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the guarded sum under postcondition ensures result >= 50u32 && result <= 100u32 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the guarded sum before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | named constants LHS_VAL and RHS_VAL define operands for wrapping addition before guarded subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| another state | `other_state/` | an internal state transition transfers the guarded sum to a successor state with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine forwards the guarded sum to the caller under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies computes wrapping addition and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Measured 2026-10-23 on macOS arm64 with the factsr1 shared generation producer
`gen-d3b0020bf6a` (`omega` sha256
`c6dff3ba4be3a38c720ee83b522d251743afde9c174ca6881390c5812920e344`, `corpus_runner`
sha256 `65225cae2bf62dec8f705339afaef606c0bac0e611c005dae984488e67ffa301`; 0 `.rs`
delta to the landing base `f28130ba26ee`): the root and all nine places report
`native exit 0` and `interp exit: 0` (10/10), and all nine twins exit 1 at checking
with the runner-emitted diagnostic below. The first measurement
(`gen-f1b0d3326d2` at `f1b0d3326d2`) agreed on all 38 runs; its golden rows were
re-recorded at the integrated base, because main had moved 25 `.rs` files.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the interval guard is widened to `101u32`, failing to discharge `requires value >= 50u32 && value <= 100u32`. Check each twin with:

```sh
omega --check samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/local/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/field/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/parameter/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/call_result/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/match_join/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/const_name/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/other_state/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/other_machine/invalid/main.omg
omega --check samples/cli/facts/unsigned_32bit_wrapping_arithmetic_bound/target_body/invalid/main.omg
```

Every reject twin fails closed with the runner-emitted
`cannot prove requires contract for call verify_sum from ...: ... (needs fact `value >= 50 && value <= 100` here; establish it before the call via a prior contract guarantee, domain membership, or guard)`

## Pinned Corpus Fixtures

Nine positive fixtures under `tests/omega/pass/facts/` and nine reject twins under
`tests/omega/fail/facts/`, each authored byte-for-byte from its place's `main.omg`
and `build.omg`; the positives also carry `expected_native_exit.txt` `0`:

- `unsigned_32bit_wrapping_arithmetic_bound_through_{local,field,parameter,call_result,match_join,const_name,other_state,other_machine,target_body}_exit`
- `unsigned_32bit_wrapping_arithmetic_bound_through_{local,field,parameter,call_result,match_join,const_name,other_state,other_machine,target_body}_rejected`

### Invalid controls and reject twins

Each place has an `invalid/` twin that widens only the guard's upper bound
(`sum <= 100u32` -> `sum <= 101u32`), so `verify_sum`'s requires contract
`value >= 50u32 && value <= 100u32` is no longer discharged. Every twin fails
closed at checking; the pinned fixture fragment is
`cannot prove requires contract for call verify_sum`.

| Place | Reject twin diagnostic (runner-emitted, fragment satisfied) |
| --- | --- |
| `local` | `cannot prove requires contract for call verify_sum from exercise: v >= 50 && v <= 100 (needs fact \`value >= 50 && value <= 100\` here...) in state \`step\`` |
| `field` | `cannot prove requires contract for call verify_sum from exercise: carrier.sum >= 50 && carrier.sum <= 100 ...` |
| `parameter` | `cannot prove requires contract for call verify_sum from helper: s >= 50 && s <= 100 ...` |
| `call_result` | `cannot prove requires contract for call verify_sum from exercise: sum >= 50 && sum <= 100 ...` |
| `match_join` | `cannot prove requires contract for call verify_sum from exercise: joined >= 50 && joined <= 100 ...` |
| `const_name` | `cannot prove requires contract for call verify_sum from exercise: s >= 50 && s <= 100 ...` |
| `other_state` | `cannot prove requires contract for call verify_sum from exercise: s >= 50 && s <= 100 ... in state \`next_step\`` |
| `other_machine` | `cannot prove requires contract for call verify_sum from exercise: relayed >= 50 && relayed <= 100 ...` |
| `target_body` | `cannot prove requires contract for call verify_sum from exercise: s >= 50 && s <= 100 ...` (4 diagnostics, one per target-scoped body) |

### Golden rows

The three corpus legs were recorded with the generation producer above, the tree
staged at the integrated published base `f28130ba26ee`, so every row is stamped
`@f28130ba26ee+wt`:

- `tests/omega/corpus_outcomes.txt`: +18 rows -- 9 `checked` rows carrying
  `check-target:macos_arm64` and 9 `rejected` rows carrying their diagnostics with
  the pinned fragments satisfied -- plus 24 diagnostic continuation lines, 0 deletions;
- `tests/omega/corpus_native_macos_arm64.txt`: +9 rows, `built ... exit:0`;
- `tests/omega/corpus_both_macos_arm64.txt`: +9 rows,
  `built ... exit:0 interpreter-exit:0 interpreter:agrees`.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`); `ARITHMETIC-POLICY-REALIZATION` owns the underlying wrapping-policy realization.
- **Requirement established:** End-to-end nine-place fact preservation for 32-bit unsigned wrapping addition (`u32 in Wrapping`, `lhs + rhs`) constrained by a bounded interval guard (`50u32 <= sum <= 100u32`), discharging exact 32-bit unsigned subtractions `100u32 - sum` and `sum - 50u32` and downstream contract `requires value >= 50u32 && value <= 100u32` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#overflow-policies), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Performing 32-bit unsigned addition under the wrapping arithmetic domain (`u32 in Wrapping`) computes modular addition modulo $2^{32}$. Dominating transition guard `sum >= 50u32 && sum <= 100u32` establishes the non-wrapping interval `[50, 100]` on the true branch. The Omega arithmetic solver derives this exact range certificate directly from the guard branch condition, verifying that `sum` satisfies `verify_sum(value: u32)` requires contract `value >= 50u32 && value <= 100u32` and discharging exact 32-bit subtractions `100u32 - sum` and `sum - 50u32` without requiring post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The wrapping 32-bit unsigned integer value maintains 4-byte stored width representation across expression evaluations, local variable assignments, parameter passing, and state transitions, transitioning cleanly between wrapping domain behavior and guarded exact interval carrier identity.
- **Composition:** Guarded wrapping addition interval preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the upper bound to `101u32`, causing `verify_sum` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_sum` because `101u32` violates the contract upper bound `value <= 100u32`. The compiler soundly refuses any program where guarded wrapping bounds cannot guarantee arrival contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Wrapping addition expressions lower into canonical Terminal Psi (`PSITERM\0`) as `WrappingIntegerAdd` operations without overflow trap edges. Downstream transition guards synthesize integer range certificates retaining `[50, 100]` through optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<u32>() == 4`, `align_of<u32>() == 4`, and `size_of<SumCarrier>() == 4`. In `CallPlan` and `StatePlan`, 32-bit unsigned integers are allocated standard 32-bit general-purpose register lanes (`ecx`/`edi`/`w0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 32-bit unsigned wrapping addition instructions: `add r32, r/m32` without overflow trap assertions on x86_64 and `add w, w, w` on arm64 architectures. Exact subtraction selects 32-bit `sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Wrapping subtraction (`lhs -% rhs`), wrapping multiplication (`*%`), 64-bit wrapping arithmetic (`u64 in Wrapping`), signed wrapping arithmetic (`i32 in Wrapping`), and SIMD modular vector arithmetic remain governed under the wider numeric value roadmap.
