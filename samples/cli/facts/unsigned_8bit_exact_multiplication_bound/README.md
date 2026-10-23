# Unsigned 8-bit exact multiplication bound

Fact: an 8-bit unsigned u8 exact multiplication of bounded inputs (lhs >= 10u8 && lhs <= 15u8, rhs >= 8u8 && rhs <= 12u8) establishes prod >= 80u8 && prod <= 180u8 so exact 8-bit subtractions 180u8 - prod and prod - 80u8 are proved overflow-free across all nine places.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`) -- machine/state parameters `requires`, results `ensures`, locals the facts from initializers, guards and calls. This row is one customer group's nine-place transport and execution of that settled proposition-based scalar range contract; `ARITHMETIC-POLICY-REALIZATION` owns the underlying numeric-policy realization (the exact-arithmetic proof obligation).
Claim Ticket: `9d6d8beb2a2a421082d06bc344b27513` (owner `Jarod / factsr1-pins-fe`).

Every positive place and reject twin tests computing exact multiplication on 8-bit unsigned `u8` values.
When the inputs are bounded to intervals `lhs >= 10u8 && lhs <= 15u8` and `rhs >= 8u8 && rhs <= 12u8`, exact unsigned multiplication `lhs * rhs` statically proves the post-multiplication interval `prod >= 80u8 && prod <= 180u8` (where `180 <= 255`, proving absence of multiplication overflow).
Subsequent exact 8-bit subtractions `180u8 - prod` and `prod - 80u8` are proved underflow-free without requiring any post-hoc caller equality guard before the operations, evaluating to `60u8 + 40u8 = 100u8` for `lhs == 12u8, rhs == 10u8` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism | Pinned Pass Fixture | Pinned Reject Fixture |
| --- | --- | --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with bounded unsigned u8 exact multiplication and dual margin subtractions | — | — |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded unsigned product is copied into a local binding before exact subtractions | `tests/omega/pass/facts/unsigned_8bit_exact_multiplication_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_multiplication_bound_through_local_rejected` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded unsigned product is stored into and read from a record field before exact subtractions | `tests/omega/pass/facts/unsigned_8bit_exact_multiplication_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_multiplication_bound_through_field_rejected` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded unsigned product is passed as an explicit machine parameter before exact subtractions | `tests/omega/pass/facts/unsigned_8bit_exact_multiplication_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_multiplication_bound_through_parameter_rejected` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | bounded unsigned product is returned from a dedicated helper machine | `tests/omega/pass/facts/unsigned_8bit_exact_multiplication_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_multiplication_bound_through_call_result_rejected` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the bounded unsigned product | `tests/omega/pass/facts/unsigned_8bit_exact_multiplication_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_multiplication_bound_through_match_join_rejected` |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply upper and lower ceilings for exact subtractions | `tests/omega/pass/facts/unsigned_8bit_exact_multiplication_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_multiplication_bound_through_const_name_rejected` |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the bounded unsigned product into a successor state | `tests/omega/pass/facts/unsigned_8bit_exact_multiplication_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_multiplication_bound_through_other_state_rejected` |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the bounded unsigned product | `tests/omega/pass/facts/unsigned_8bit_exact_multiplication_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_multiplication_bound_through_other_machine_rejected` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing the multiplication and subtractions | `tests/omega/pass/facts/unsigned_8bit_exact_multiplication_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_multiplication_bound_through_target_body_rejected` |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/unsigned_8bit_exact_multiplication_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_multiplication_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(12u8, 10u8)` computes `12u8 * 10u8 = 120u8`.
Because `lhs in [10, 15]` and `rhs in [8, 12]`, multiplication interval synthesis statically proves `prod in [80, 180]`.
Subsequent exact subtractions `headroom = 180u8 - prod` (`60u8`) and `margin = prod - 80u8` (`40u8`) are proved underflow-free, and their sum `headroom + margin` evaluates to `100u8`.
The transition guard `combined == 100u8` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
If multiplication or subtraction evaluation is corrupted, the guard fails and transitions to `bad() { crash Trap; }`, terminating with exit 255.
Measured on macOS arm64: perturbing the root's single comparison (`transition combined == 100u8` -> `== 101u8`) exits 255 reporting `native signal 5` and `interp crash: Trap (host presentation agrees)`, while the unperturbed root exits 0 in both engines.

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens the multiplier precondition bound from `rhs <= 12u8` to `rhs <= 13u8`.
When `rhs == 13u8`, `prod` can reach `15 * 13 = 195u8 > 180u8`, which would make `180u8 - prod` underflow `u8` (potential underflow wrap to `256 - 15 = 241`).
The compiler strictly fails closed at compile time; `omega run --both` on each twin exits 200 after the check leg refuses it, and the corpus check leg records the same rejection:

| Place | Positive `omega run --both` | Reject twin diagnostic (authored fragment) |
| --- | --- | --- |
| `local` | native 0 / interp 0 | ``may overflow `u8` `` |
| `field` | native 0 / interp 0 | ``may overflow `u8` `` |
| `parameter` | native 0 / interp 0 | `cannot prove requires contract for call consume_bounds from probe: prod >= 80 && prod <= 180` |
| `call_result` | native 0 / interp 0 | `cannot prove ensures contract for exit from compute_prod at statement 0: result >= 80 && result <= 180` |
| `match_join` | native 0 / interp 0 | `cannot prove requires contract for call choose from probe: prod >= 80 && prod <= 180` |
| `const_name` | native 0 / interp 0 | ``may overflow `u8` `` |
| `other_state` | native 0 / interp 0 | `cannot prove requires contract for call consume from probe: prod >= 80 && prod <= 180` |
| `other_machine` | native 0 / interp 0 | `cannot prove requires contract for call forward_prod from probe: prod >= 80 && prod <= 180` |
| `target_body` | native 0 / interp 0 | ``may overflow `u8` `` (four target bodies, four diagnostics) |

The `local`, `field`, `const_name` and `target_body` twins emit the exact-arithmetic obligation directly (decision 17, ``exact arithmetic in machine `<machine>` state `<state>` local `headroom` may overflow `u8` ``); the transporting twins refuse the forwarded interval instead. Each fixture's `expected.txt` carries the fragment the check route emits.

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/unsigned_8bit_exact_multiplication_bound_through_<place>_exit`
and `tests/omega/fail/facts/unsigned_8bit_exact_multiplication_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its place's
`main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a byte-identical copy
of its place twin's `main.omg`/`build.omg`/`expected.txt`. Positive versus twin `git diff` moves only
the fact's own bound (`rhs <= 12u8` to `rhs <= 13u8`), so a red place or twin names this fact's
establishment, not a shape recognizer.

Measured on macOS arm64 with the factsr1 shared generation producer `gen-f1b0d3326d2` (base
`f1b0d3326d2dcc6ee9aad44ecd2f177a88bebec4`; omega sha256
`4e3950e53eb15fbb3b54868f9c8964e23e7efc5bdced336e07f18eb27e28ac4a`, corpus_runner sha256
`c25372101a8cd22676fc23b3b80b2387a58609a988353552f158251c65650b96`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and `interp exit: 0`.
- `omega run --both` over the nine twins: 9/9 exit 200 at checking with their authored fragments
  (four ``may overflow `u8` ``, five forwarded-interval refusals); none reaches native execution.
- Perturbing the root oracle (one token: `transition combined == 100u8` -> `== 101u8`) exits 255 with
  `native signal 5` and `interp crash: Trap (host presentation agrees)`.
- `python3 tools/sample_gate.py --cli <generation omega> --filter unsigned_8bit_exact_multiplication_bound,unsigned_8bit_exact_subtraction_bound`
  records 38/38 projects checked on Darwin arm64 with 18 of these matched intended rejection controls.
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `f1b0d3326d2d`, so every row is stamped `@f1b0d3326d2d+wt`: `tests/omega/corpus_outcomes.txt` gained
  60 lines for this row's 18 fixtures (nine `checked` carrying `check-target:macos_arm64` and nine
  `rejected` carrying their diagnostics with the expected fragments satisfied),
  `tests/omega/corpus_native_macos_arm64.txt` gained nine `built ... exit:0` rows and
  `tests/omega/corpus_both_macos_arm64.txt` gained nine `built ... exit:0 interpreter-exit:0
  interpreter:agrees` rows. Every golden diff is a pure addition (0 deletions); the legs exited 0
  with 36/36, 18/18 and 18/18 fixtures done, 0 unimplemented rows and 0 rows recording a lost success.

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Caller contract preconditions `lhs >= 10u8 && lhs <= 15u8` and `rhs >= 8u8 && rhs <= 12u8` establish bounded operand intervals. Unsigned exact multiplication `lhs * rhs` synthesizes post-multiplication interval `[80, 180]` directly from interval arithmetic rules without post-hoc caller equality guards.
- **Identity and lifetime**: The computed interval fact `prod in [80, 180]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `prod in [80, 180]` composes directly with 8-bit unsigned exact subtractions `headroom = 180u8 - prod` and `margin = prod - 80u8` to statically discharge underflow safety, proving `headroom in [0, 100]` and `margin in [0, 100]` without any preceding post-hoc caller equality guard.
- **Soundness boundaries**: In every reject twin, widening the multiplier bound to `rhs <= 13u8` permits `prod` to reach `195u8`, causing `180u8 - prod` to potentially underflow below zero and triggering compile-time rejection under decision 17 (or contract violation on boundary transmission), verifying that underflow-unsafe paths cannot pass unchecked.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Unsigned 8-bit exact multiplication and subtraction operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`) integer arithmetic representations (`IntegerMode::Exact`) accompanied by integer multiplication certificates (`prod <= 255`), while proof annotations are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent 8-bit unsigned integers in 1-byte stored width (`IntegerAt 1-byte`, align 1) across all four hosted ABI calling conventions (CL/DL on `windows_x86_64`, DIL/SIL on `linux_x86_64`, W0/W1 on `macos_arm64` and `linux_arm64`), returning 8-bit unsigned results zero-extended in AL / W0.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects 8-bit unsigned multiplication instructions: `mul reg8` / `imul eax, ...` on `x86_64` and zero-extended `mul w0, w0, w1` masked to 8 bits on `arm64`, followed by 8-bit subtraction `sub al, ...`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all nine places and nine reject twins run in both engines with documented exit 0, their negative controls reject, and all 18 fixtures are pinned with recorded goldens. Adjacent mechanisms (16-bit and 32-bit exact multiplication, saturation and wrapping domains) keep their own rows.