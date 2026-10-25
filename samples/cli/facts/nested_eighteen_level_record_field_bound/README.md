# Nested Eighteen-Level Record Field Bound

Fact: an 18-level nested structural record RootRec { l1: Level1 { l2: Level2 { l3: Level3 { l4: Level4 { l5: Level5 { l6: Level6 { l7: Level7 { l8: Level8 { l9: Level9 { l10: Level10 { l11: Level11 { l12: Level12 { l13: Level13 { l14: Level14 { l15: Level15 { l16: Level16 { l17: Level17 { val: u64 } } } } } } } } } } } } } } } } } preserves the leaf field bound 40u64 <= root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.val <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.

Owning Task: `CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS` (`TASKS.md`)
Claim Ticket: `eec2300dd9b24505a7b0ae1ee4e2169c`

Every place performs the same operation: construct or transport an 18-level nested structural record `RootRec { l1: Level1 { l2: Level2 { l3: Level3 { l4: Level4 { l5: Level5 { l6: Level6 { l7: Level7 { l8: Level8 { l9: Level9 { l10: Level10 { l11: Level11 { l12: Level12 { l13: Level13 { l14: Level14 { l15: Level15 { l16: Level16 { l17: Level17 { val: u64 } } } } } } } } } } } } } } } } }` with leaf field interval bound `40u64 <= root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.val <= 80u64`, project `res = root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.val`, discharge both headroom subtraction `80u64 - res` and margin subtraction `res - 40u64`, verify `headroom + margin == 40u64`, and exit 0. Only the transport mechanism of the nested record changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 18-level nested record construction (`check_bound`), leaf projection, and exact subtraction discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 18-level nested record is copied to a local variable before leaf projection into `check_bound` and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 18-level nested record is stored in a container record field before leaf projection into `check_bound` and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive the 18-level nested record before leaf projection into `check_bound` and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a contracted helper call (`produce` returning to `check_bound`) returns the 18-level nested record before leaf projection and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms of `choose_root` yield an 18-level nested record and join before leaf projection and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant (`VAL_LEAF`) supplies the leaf value consumed by `check_bound` before projection and interval discharge |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine (`check_state` state `step`) receives the 18-level nested record before leaf projection and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine (`forward`) receives and passes the 18-level nested record to `check_worker` for projection and interval discharge |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies (`check_target`) receives the 18-level nested record and discharges the leaf bound |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, declares
`Expected exit: 0`, and carries an authored `expected_native_exit.txt` (`0`).

Match the fact sentence beside every place: each positive place is pinned as `tests/omega/pass/facts/nested_eighteen_level_record_field_bound_through_<place>_exit` and each reject twin as `tests/omega/fail/facts/nested_eighteen_level_record_field_bound_through_<place>_rejected`, byte-equal to the place and `invalid/` programs beside them (main.omg, build.omg, and the exit or rejection oracle).

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/nested_eighteen_level_record_field_bound/main.omg
omega run --both samples/cli/facts/nested_eighteen_level_record_field_bound/local/main.omg
omega run --both samples/cli/facts/nested_eighteen_level_record_field_bound/field/main.omg
omega run --both samples/cli/facts/nested_eighteen_level_record_field_bound/parameter/main.omg
omega run --both samples/cli/facts/nested_eighteen_level_record_field_bound/call_result/main.omg
omega run --both samples/cli/facts/nested_eighteen_level_record_field_bound/match_join/main.omg
omega run --both samples/cli/facts/nested_eighteen_level_record_field_bound/const_name/main.omg
omega run --both samples/cli/facts/nested_eighteen_level_record_field_bound/other_state/main.omg
omega run --both samples/cli/facts/nested_eighteen_level_record_field_bound/other_machine/main.omg
omega run --both samples/cli/facts/nested_eighteen_level_record_field_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:

- At runtime, with `val == 60u64`, `res == 60u64`, `headroom == 20u64`, `margin == 20u64`, and `headroom + margin == 40u64`. The transition `probe() == 40 { true -> done() _ -> bad() }` exits 0. If eighteen-level field projection or the exact interval calculations evaluate incorrectly, execution transitions to `machine bad() { crash Trap; }`, which traps. Measured: perturbing the demanded result (`transition probe() == 40` to `== 41`) routes execution to `bad()` and traps both engines (`native signal 5` / `interp crash: Trap (host presentation agrees)`, exit 255), so the exit-0 oracle actively discriminates the computation.
- In each invalid twin, the leaf upper bound is widened to `81u64` so `res` can reach `81u64 > 80u64`. Proof obligation checking statically detects that `80u64 - res` can underflow `u64` below zero, failing closed with `may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/nested_eighteen_level_record_field_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python3 tools/sample_native_gate.py --cli <gen>/omega --filter cli/facts/nested_eighteen_level_record_field_bound --jobs 2
```

Measured on macOS arm64 with the `factsr1` shared producer built from base `4c23cc3391c` (`omega` sha256 `83c7ae2a45442b1ceb6f5476a6f8ed4c4af9eabbd85e682a3ed7254c7fec94cf`, `corpus_runner` sha256 `a1de848f1baca0837d28285d9da6da9acdd7639081c083743f76862a409bdb3d`):

- `omega run --both` reports `native exit 0` and `interp exit: 0` for the root and all nine places (10/10), Terminal Psi and native execution agreeing.
- `omega run --both` refuses all nine `<place>/invalid/main.omg` twins with `may overflow \`u64\`` in the exact-arithmetic obligation class; `git diff` positive vs twin changes only the construct's own value, domain or contract (the widened `<= 80u64`/`<= 81u64` bounds, the const-name place's leaf constant, and the call-result place's `ensures` bound).
- The nine positive places and nine reject twins are pinned as `tests/omega/{pass,fail}/facts/nested_eighteen_level_record_field_bound_through_<place>_{exit,rejected}`, each byte-equal to the sample program it pins, with recorded corpus rows: nine `checked ... check-target:macos_arm64` and nine `rejected ... expected` rows carrying their own `may overflow \`u64\`` diagnostic in `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, and nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`.

The row's requirement: the leaf field bound `40u64 <= root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.val <= 80u64` of an 18-level nested structural record survives construction or transport through all nine places into the consuming exact subtractions, established statically without a post-hoc caller equality guard, with Terminal Psi and native agreement.

## Requirements and Evidence

- **Owning board item:** `CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS` (`TASKS.md`).
- **Claim ticket:** `eec2300dd9b24505a7b0ae1ee4e2169c`.
- **Requirement established:** projecting the leaf field of an 18-level nested structural record `RootRec { l1: Level1 { l2: Level2 { l3: Level3 { l4: Level4 { l5: Level5 { l6: Level6 { l7: Level7 { l8: Level8 { l9: Level9 { l10: Level10 { l11: Level11 { l12: Level12 { l13: Level13 { l14: Level14 { l15: Level15 { l16: Level16 { l17: Level17 { val: u64 } } } } } } } } } } } } } } } } }` carries the static leaf interval bound (`root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.val >= 40u64 && root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.val <= 80u64`) through construction, copies, aggregate fields, parameters, call returns, match joins, named constants, state transitions, forwarding machines and target-scoped bodies, discharging exact 64-bit unsigned subtractions `80u64 - res` and `res - 40u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).
- **Specification references:** [layout plans](wiki/spec/layouts/plans.md) (structural record layout geometry, cumulative field offset resolution, nested size/alignment) and [structural access](wiki/spec/terminal-psi/structural_access.md) (compile-time interval bound preservation through `ProjectField` chains across 18 levels).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis tracks leaf field constraints through 18 levels of structural record nesting (`RootRec` -> `Level1` -> … -> `Level17` -> `val`). The static leaf bound `[40, 80]` is established at record construction or verified via machine `requires`/`ensures` contracts. The established interval proves `res >= 40u64` and `res <= 80u64` unconditionally and guarantees `res` fits within `u64` (`>= 0 && <= 18446744073709551615`), directly proving that exact subtractions `80u64 - res >= 0` and `res - 40u64 >= 0` cannot underflow below zero without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 64-bit scalar leaf value and its interval bounds retain their precision, 64-bit stored width, domain tag, and non-negative invariants across 18 levels of structural record nesting, copies into local variables, container record fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies without memory corruption, slicing, or loss of refined bounds.
  - **Composition:** The 18-level nested record projections compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtractions `80u64 - res` and `res - 40u64`, evaluating to `40u64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the construct's own value, domain or contract (the operand bound `val <= 80u64` vs `val <= 81u64`, and the const-name place's leaf constant).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `val <= 81u64`: interval analysis establishes `res in [40, 81]`, so exact subtraction `80u64 - res` can underflow `u64` below zero (`bounds [-1, 40] lie outside [0, 18446744073709551615]`). Compilation strictly rejects with `may overflow \`u64\`` (`exact arithmetic in machine \`check_bound\` state \`entry\` local \`headroom\` may overflow \`u64\``). Subtraction is statically certified against 64-bit unsigned underflow (`res <= 80`). No unproved boundary violation reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral 18-level nested record projection and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing field access paths (`ProjectField` chains across 18 levels) while certifying exact integer arithmetic certificates (`SubExact` on `u64`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 64-bit stored width (8 bytes, align 8) for `u64` and nested record layouts (`Level17` … `Level1` and `RootRec` each `size_of = 8, align_of = 8`) validate against target layout plans with zero inner padding and cumulative field offset 0 to the scalar leaf. Target `CallPlan`/`StatePlan` assign record values to register lanes or indirect aggregate stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering loads the 64-bit leaf scalar from its nested struct offset (offset 0 across all 18 levels) into a 64-bit register and performs exact 64-bit subtractions (`sub r64, r64`); on `arm64`, 64-bit register operations (`sub x, x, x`) execute exact subtraction on `X` registers. Register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`).

- **Pinned and recorded:** 18 fixture families (`..._through_<place>_{exit,rejected}` for the nine places) authored byte-for-byte from this sample tree. Measured on macOS arm64 with the `factsr1` shared producer built from base `4c23cc3391c` (`@4c23cc3391c2+wt` stamp): nine `checked ... check-target:macos_arm64` and nine `rejected ... expected` rows in `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, nine `exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`; 0 keys lost, 0 timeout rows. No compiler repair was needed and no repair build was requested.
- **What remains open:** Dynamic nested array-within-record projections, variant tagged payloads across nested structures, mutable field updates across nested paths, and routes where a level is a `&`/`&mut` view or a computed (runtime) field selection. A deeper or wider copy of this same copy-transport mechanism adds no new compiler code path.