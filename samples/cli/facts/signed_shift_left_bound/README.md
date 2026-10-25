# Signed shift left bound

Fact: left shift on a bounded signed i64 value value >= 25i64 && value <= 25i64 by 2 establishes shifted >= 100i64 && shifted <= 100i64 for exact subtraction shifted - 100i64.

Every positive place and reject twin tests left shifting a signed `i64` value by 2.
When the operand is bounded to interval `val >= 25i64 && val <= 25i64`, exact left shift `val << 2` statically proves the post-shift interval `shifted >= 100i64 && shifted <= 100i64`.
Subsequent exact subtraction `shifted - 100i64` is proved free of overflow/underflow and evaluates to `0i64`, terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with bounded signed i64 left shifted by 2 |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded signed operand is copied into a local binding before left shift |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded signed operand is stored into and read from a record field before shift |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded signed operand is passed as an explicit parameter to shift_left |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | bounded signed operand is returned from a dedicated provider machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression produce the bounded signed operand |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the bounded signed operand |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the bounded signed operand into a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the bounded signed operand |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body for the shift |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/signed_shift_left_bound/main.omg
omega run --both samples/cli/facts/signed_shift_left_bound/local/main.omg
omega run --both samples/cli/facts/signed_shift_left_bound/field/main.omg
omega run --both samples/cli/facts/signed_shift_left_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_shift_left_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_shift_left_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_shift_left_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_shift_left_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_shift_left_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_shift_left_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, the input value `25i64` is shifted left by 2 (`25 * 4 = 100i64`).
The transition guard `shifted >= 100i64 && shifted <= 100i64` verifies the exact outcome,
transitioning to `done(shifted)` where exact subtraction `s - 100i64` proves 0 without underflow.
If any bit corruption or shift miscalculation occurs, the guard evaluates to false, transitioning to `bad() { crash Trap; }`,
producing native signal 5 and interpreter crash Trap (exit 255).

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that weakens the shift's own contract: it omits the operand bound
`requires val >= 25i64 && val <= 25i64` and the derived `ensures result >= 100i64 && result <= 100i64`
from the machine(s) that compute or forward the shift in that place (`forward_param`, `dispatch_shift`, and
the four target bodies included). Every other line is byte-identical to the positive: the carrier
(`local` binding, record field, parameter, return value, match join, named constant, successor machine,
forwarding machine, target body), the `shifted` guard, the `done` subtraction and `bad` Trap stay the same.
Without the bound, exact integer shift left on `i64` may overflow and fails closed at checked compilation:

```
may overflow `i64`
```

specifically:

```
error: exact arithmetic in machine `shift_left` state `entry` terminal expression may overflow `i64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation).
```

Restoring the operand bounds restores clean compilation and zero exit.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`)
- **Requirement established:** Exact 64-bit signed left shift of a bounded input (`val >= 25i64 && val <= 25i64`) by `2` bits statically establishes `shifted >= 100i64 && shifted <= 100i64`, so the consuming exact subtraction `shifted - 100i64` is proved overflow-free and evaluates to `0i64` across all nine places, without any post-hoc caller equality guard, with Terminal Psi and native execution agreeing (`omega run --both`: root plus nine places, `native exit 0`, `interp exit: 0`).
- **Pinned fixtures:**
  - Positive passes: `tests/omega/pass/facts/signed_shift_left_bound_through_<place>_exit/` (nine, with `build.omg` binding `ProgramEntry` for all four hosted targets and `expected_native_exit.txt` `0`)
  - Negative controls: `tests/omega/fail/facts/signed_shift_left_bound_through_<place>_rejected/` (nine, each carrying `expected.txt` `may overflow \`i64\``)
  - Goldens: nine `checked` + nine `rejected` rows in `tests/omega/corpus_outcomes.txt`; nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`; nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`

### Specification and Lowering Layers

1. **Establishment:** Establishing interval precondition `val >= 25i64 && val <= 25i64` and applying constant shift count `2` statically proves the post-shift interval `result >= 100i64 && result <= 100i64` directly from arithmetic intervals without any post-hoc caller equality guard (`transition x == 100`).
2. **Identity and lifetime:** The signed operand interval and post-shift interval fact remain bound to the exact SSA value across copy into locals, record fields, parameter passing, return values, match join unification, constant naming, and state transitions across all 9 places.
3. **Composition:** The bounded signed shift composes through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation `s - 100i64`. In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the shift's own contract clauses: the operand bound `requires val >= 25i64 && val <= 25i64` and the derived `ensures result >= 100i64 && result <= 100i64` are omitted from the machine(s) that compute or forward the shift, while the carrier, the `shifted` guard, the `done` subtraction and the `bad` Trap are byte-identical.
4. **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at checked compilation (native compile failed, exit 200) with `error: exact arithmetic in machine \`shift_left\` state \`entry\` terminal expression may overflow \`i64\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` when the signed operand is unbounded; the four target-scoped bodies of `target_body` each report it, and no unproved interval fact reaches native execution. Restoring the operand bound restores clean compilation and the zero exit.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact signed left shift operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`) integer arithmetic operations (`IntegerMode::Exact`) accompanied by integer certificates while mathematical proof annotations are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry for `i64` (size 8, align 8) validates, and `CallPlan`/`StatePlan` lower integer carriers across registers and stack slots according to target calling conventions.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects native integer shift instructions (`shl` with constant immediate on `x86_64`, `lsl` on `arm64`) and integer subtraction (`sub`), assigns register homes and disjoint stack-frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan` ceiling.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve to concrete offsets, basic blocks and branch targets relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets (`windows_x86_64` and `macos_arm64`).

- **What remains open:** Shift counts proven at runtime rather than by a constant literal, non-`i64` signed widths (the 8/16/32-bit signed and unsigned shift-left rows are separate facts), and foreign non-hosted targets stay outside this row.
