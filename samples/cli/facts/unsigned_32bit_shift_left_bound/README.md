# Unsigned 32-bit shift left bound

Fact: a 32-bit unsigned u32 logical left shift of a bounded input (val >= 20u32 && val <= 20u32) by 2u32 bits establishes shifted >= 80u32 && shifted <= 80u32 so exact 32-bit subtraction shifted - 80u32 is proved underflow-free across all nine places.

Owning Task: `NUMERIC-SCALARS` / `CONSERVATION-CONTRACT` (`TASKS.md`)
Claim Ticket: `e4a29817b35f4901ad98ef7218b24a33`

Every positive place and reject twin tests left shifting an unsigned 32-bit `u32` value by 2 bits.
When the input is bounded to interval `val >= 20u32 && val <= 20u32`, exact unsigned left shift `val << 2u32` statically proves the post-shift interval `shifted >= 80u32 && shifted <= 80u32`.
Subsequent exact 32-bit subtraction `shifted - 80u32` is proved underflow-free and evaluates to `0u32`, terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with bounded unsigned u32 left shifted by 2 |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded unsigned shifted result is copied into a local binding before exact subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded unsigned shifted result is stored into and read from a record field before exact subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded unsigned shifted result is passed as an explicit machine parameter before exact subtraction |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | bounded unsigned shifted result is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the bounded unsigned shifted result |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the subtrahend offset for exact subtraction |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the bounded unsigned shifted result into a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the bounded unsigned shifted result |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing the shift and exact subtraction |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/unsigned_32bit_shift_left_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_left_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_left_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_left_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_left_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_left_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_left_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_left_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_left_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_left_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, the input value `20u32` (`val >= 20u32 && val <= 20u32`) is logically left shifted by 2 bits (`20 << 2 = 80u32`).
The shift synthesis proves `shifted >= 80u32 && shifted <= 80u32`.
Subsequent exact subtraction `shifted - 80u32` evaluates to `0u32` and is statically proved free of underflow without requiring any post-hoc caller equality guard before the operation.
Testing `probe(20u32)` produces `check == 0u32`, which transitions to `done()`, terminating with native exit 0.
If bit corruption or incorrect shift evaluation occurs, the transition condition `check == 0u32` fails and transitions to `bad() { crash Trap; }`, terminating with exit 255.

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that weakens the precondition bound from `val >= 20u32 && val <= 20u32` to `val >= 15u32 && val <= 20u32`.
When `val == 15u32`, `shifted` evaluates to `15 << 2 = 60u32`, making `shifted - 80u32` underflow `u32` (potential underflow wrap to `2^32 - 20`).
The compiler strictly fails closed at compile time:

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `error: exact arithmetic in machine \`probe\` state \`entry\` local \`check\` may overflow \`u32\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` |
| `field` | native 0 / interp 0 | `error: exact arithmetic in machine \`probe\` state \`entry\` local \`check\` may overflow \`u32\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` |
| `parameter` | native 0 / interp 0 | `cannot prove requires contract for call subtract_and_return from probe: shifted >= 80 && shifted <= 80` |
| `call_result` | native 0 / interp 0 | `cannot prove ensures contract for exit from perform_shift at statement 0: result >= 80 && result <= 80` |
| `match_join` | native 0 / interp 0 | `cannot prove requires contract for call choose from probe: shifted >= 80 && shifted <= 80` |
| `const_name` | native 0 / interp 0 | `error: exact arithmetic in machine \`probe\` state \`entry\` local \`check\` may overflow \`u32\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` |
| `other_state` | native 0 / interp 0 | `cannot prove requires contract for call consume from probe: shifted >= 80 && shifted <= 80` |
| `other_machine` | native 0 / interp 0 | `cannot prove requires contract for call forward_shifted from probe: shifted >= 80 && shifted <= 80` |
| `target_body` | native 0 / interp 0 | `error: exact arithmetic in machine \`target_probe\` state \`entry\` local \`check\` may overflow \`u32\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` |

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Caller contract precondition `val >= 20u32 && val <= 20u32` establishes a precise unsigned singleton interval. Logical left shift `val << 2u32` synthesizes post-shift interval `[80, 80]`, carrying statically proved bounds directly from numeric arithmetic rules without post-hoc caller equality guards.
- **Identity and lifetime**: The computed interval fact `shifted in [80, 80]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `shifted in [80, 80]` composes directly with 32-bit unsigned exact subtraction `check = shifted - 80u32` to statically discharge underflow safety, proving `check in [0, 0]` without any preceding post-hoc caller equality guard.
- **Soundness boundaries**: In every reject twin, weakening the input bound to `val >= 15u32 && val <= 20u32` permits `shifted` to drop to `60u32`, causing `shifted - 80u32` to potentially underflow below zero and triggering compile-time rejection under decision 17 (or contract violation on boundary transmission), verifying that underflow-unsafe paths cannot pass unchecked.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Unsigned 32-bit logical left shift and exact subtraction operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`) integer arithmetic representations (`IntegerMode::Exact`) accompanied by integer certificates (shift count `< 32`), while proof annotations are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent 32-bit unsigned integers in 4-byte stored width (`IntegerAt 4-byte`, align 4) across all four hosted ABI calling conventions (ECX on `windows_x86_64`, EDI on `linux_x86_64`, W0 on `macos_arm64` and `linux_arm64`), returning 32-bit unsigned results in EAX / W0.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects 32-bit target ISA instructions: `shl eax, 2` / `shl reg32, imm8` on `x86_64` and `lsl w0, w0, #2` on `arm64`, followed by 32-bit subtraction `sub eax, 80` / `sub w0, w0, #80`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
