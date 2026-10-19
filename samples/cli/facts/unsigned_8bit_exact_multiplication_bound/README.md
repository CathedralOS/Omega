# Unsigned 8-bit exact multiplication bound

Fact: an 8-bit unsigned u8 exact multiplication of bounded inputs (lhs >= 10u8 && lhs <= 15u8, rhs >= 8u8 && rhs <= 12u8) establishes prod >= 80u8 && prod <= 180u8 so exact 8-bit subtractions 180u8 - prod and prod - 80u8 are proved overflow-free across all nine places.

Owning Task: `NUMERIC-SCALARS` / `CONSERVATION-CONTRACT` (`TASKS.md`)
Claim Ticket: `a8b19284e36d4018bc72de5138f36a11`

Every positive place and reject twin tests computing exact multiplication on 8-bit unsigned `u8` values.
When the inputs are bounded to intervals `lhs >= 10u8 && lhs <= 15u8` and `rhs >= 8u8 && rhs <= 12u8`, exact unsigned multiplication `lhs * rhs` statically proves the post-multiplication interval `prod >= 80u8 && prod <= 180u8` (where `180 <= 255`, proving absence of multiplication overflow).
Subsequent exact 8-bit subtractions `180u8 - prod` and `prod - 80u8` are proved underflow-free without requiring any post-hoc caller equality guard before the operations, evaluating to `60u8 + 40u8 = 100u8` for `lhs == 12u8, rhs == 10u8` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with bounded unsigned u8 exact multiplication and dual margin subtractions |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded unsigned product is copied into a local binding before exact subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded unsigned product is stored into and read from a record field before exact subtractions |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded unsigned product is passed as an explicit machine parameter before exact subtractions |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | bounded unsigned product is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the bounded unsigned product |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply upper and lower ceilings for exact subtractions |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the bounded unsigned product into a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the bounded unsigned product |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing the multiplication and subtractions |

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

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens the multiplier precondition bound from `rhs <= 12u8` to `rhs <= 13u8`.
When `rhs == 13u8`, `prod` can reach `15 * 13 = 195u8 > 180u8`, which would make `180u8 - prod` underflow `u8` (potential underflow wrap to `256 - 15 = 241`).
The compiler strictly fails closed at compile time:

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `error: exact arithmetic in machine \`probe\` state \`entry\` local \`headroom\` may overflow \`u8\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` |
| `field` | native 0 / interp 0 | `error: exact arithmetic in machine \`probe\` state \`entry\` local \`headroom\` may overflow \`u8\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` |
| `parameter` | native 0 / interp 0 | `cannot prove requires contract for call consume_bounds from probe: prod >= 80 && prod <= 180` |
| `call_result` | native 0 / interp 0 | `cannot prove ensures contract for exit from compute_prod at statement 0: result >= 80 && result <= 180` |
| `match_join` | native 0 / interp 0 | `cannot prove requires contract for call choose from probe: prod >= 80 && prod <= 180` |
| `const_name` | native 0 / interp 0 | `error: exact arithmetic in machine \`probe\` state \`entry\` local \`headroom\` may overflow \`u8\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` |
| `other_state` | native 0 / interp 0 | `cannot prove requires contract for call consume from probe: prod >= 80 && prod <= 180` |
| `other_machine` | native 0 / interp 0 | `cannot prove requires contract for call forward_prod from probe: prod >= 80 && prod <= 180` |
| `target_body` | native 0 / interp 0 | `error: exact arithmetic in machine \`target_probe\` state \`entry\` local \`headroom\` may overflow \`u8\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` |

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Caller contract preconditions `lhs >= 10u8 && lhs <= 15u8` and `rhs >= 8u8 && rhs <= 12u8` establish bounded operand intervals. Unsigned exact multiplication `lhs * rhs` synthesizes post-multiplication interval `[80, 180]` directly from interval arithmetic rules without post-hoc caller equality guards.
- **Identity and lifetime**: The computed interval fact `prod in [80, 180]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `prod in [80, 180]` composes directly with 8-bit unsigned exact subtractions `headroom = 180u8 - prod` and `margin = prod - 80u8` to statically discharge underflow safety, proving `headroom in [0, 100]` and `margin in [0, 100]` without any preceding post-hoc caller equality guard.
- **Soundness boundaries**: In every reject twin, widening the multiplier bound to `rhs <= 13u8` permits `prod` to reach `195u8`, causing `180u8 - prod` to potentially underflow below zero and triggering compile-time rejection under decision 17 (or contract violation on boundary transmission), verifying that underflow-unsafe paths cannot pass unchecked.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Unsigned 8-bit exact multiplication and subtraction operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`) integer arithmetic representations (`IntegerMode::Exact`) accompanied by integer multiplication certificates (`prod <= 255`), while proof annotations are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent 8-bit unsigned integers in 1-byte stored width (`IntegerAt 1-byte`, align 1) across all four hosted ABI calling conventions (CL/DL on `windows_x86_64`, DIL/SIL on `linux_x86_64`, W0/W1 on `macos_arm64` and `linux_arm64`), returning 8-bit unsigned results zero-extended in AL / W0.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects 8-bit unsigned multiplication instructions: `mul reg8` / `imul eax, ...` on `x86_64` and zero-extended `mul w0, w0, w1` masked to 8 bits on `arm64`, followed by 8-bit subtraction `sub al, ...`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
