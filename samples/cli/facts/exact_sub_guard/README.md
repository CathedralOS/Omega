# Exact sub guard

`remaining - 2` is exact u64 arithmetic because the entry `requires remaining <= 5`
bounds the value and the `remaining >= 2` transition guard proves the operand order,
preventing underflow. No domain, no `as`.

Each directory is one place, following the nine standard places: local, field, parameter,
call result, match join, const name, a nested machine, another machine, and another target body.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with local copy and transition guard |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | value is copied into a local variable before guarded subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | value is stored in and projected from record field `pocket.remaining` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit parameter carries bounded contract before guarded subtraction |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper call result supplies bounded value before subtraction |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms preserve and join bounded value before guarded subtraction |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant supplies bounded value before guarded subtraction |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor machine `deduct` receives bounded value across transition |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine supplies bounded value to consumer machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies verify bounded subtraction on each hosted platform |

## Execution

Every positive place (and the root `main.omg`) binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`.
`omega run --both` reports Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/exact_sub_guard/main.omg
omega run --both samples/cli/facts/exact_sub_guard/local/main.omg
omega run --both samples/cli/facts/exact_sub_guard/field/main.omg
omega run --both samples/cli/facts/exact_sub_guard/parameter/main.omg
omega run --both samples/cli/facts/exact_sub_guard/call_result/main.omg
omega run --both samples/cli/facts/exact_sub_guard/match_join/main.omg
omega run --both samples/cli/facts/exact_sub_guard/const_name/main.omg
omega run --both samples/cli/facts/exact_sub_guard/other_state/main.omg
omega run --both samples/cli/facts/exact_sub_guard/other_machine/main.omg
omega run --both samples/cli/facts/exact_sub_guard/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

`Main::main` verifies arithmetic correctness by checking `walk(4) == 2 && walk(1) == 1`.
If subtraction produces an incorrect value or underflows, the transition guard evaluates to `false`,
transitioning to `fail` which executes `crash Trap;` and terminates with a non-zero exit/signal.

### Invalid controls and reject twins

The `invalid` child of each place drops that operand-order proof and must reject exact
`remaining - 2`. Each one keeps the place's program shape and removes only the `remaining >= 2`
guard proof, so exact arithmetic checking fails closed with exit code 1 and diagnostic containing:

```
may overflow
```

Restoring the lower-bound guard makes each twin check and pass, proving that the twin breaks
this exact fact and nothing else.

```sh
omega --check samples/cli/facts/exact_sub_guard/local/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/field/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/parameter/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/call_result/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/match_join/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/const_name/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/other_state/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/other_machine/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/target_body/invalid/main.omg
```

## Requirements and Evidence

- **Parent outcome:** Exact integer arithmetic without underflow / [numeric values](../../../../wiki/spec/language/numeric_values.md).
- **Pinned fixtures:**
  - Positive passes: `tests/omega/pass/facts/exact_sub_guard_through_<place>_exit`
  - Negative controls: `tests/omega/fail/facts/exact_sub_guard_through_<place>_rejected`

### Specification and Lowering Layers

1. **Establishment:** The entry contract `requires remaining <= 5` bounds the scalar and the transition/contract guard `remaining >= 2` establishes the exact lower bound justifying exact unsigned subtraction `remaining - 2` without underflow. Safe exact subtraction is established statically by the guard and entry contracts at the point of subtraction, without any post-hoc caller equality guard (`transition x == 100`) after the operation completes.
2. **Identity and lifetime:** The guarded lower-bound fact remains bound to the exact scalar subject identity across local copy, field projection, parameter, call result, match join, const name, state transition, machine call, and target body. No intervening mutating write modifies or invalidates the scalar value between guard evaluation and subtraction.
3. **Composition:** The lower-bound fact composes through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`remaining - 2`), proving no underflow. `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the construct's guard/contract bound (omitting `remaining >= 2`), isolating the exact fact under test.
4. **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed with compilation error `may overflow` when the lower bound guard or contract is omitted so that underflow becomes possible (e.g. `walk(1)` would underflow `1 - 2`).
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The proved exact-subtraction certificate lowers into canonical `PSITERM\0` integer subtraction while erasing proof-only contracts (`requires remaining <= 5`), and executes identically in the Terminal Psi interpreter (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry (`u64` 8 bytes, alignment 8) and record layout validate target plans. `CallPlan` and `StatePlan` lower arguments and return carriers across registers and stack slots according to target calling conventions.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects native subtraction (`sub`) and comparison/branch instructions (`cmp`, `jge`/`b.ge`), assigns register homes and disjoint stack-frame storage, composes worst-case stack usage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan` frame ceiling.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve, basic blocks and guard branches relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets (`windows_x86_64` and `macos_arm64`).
