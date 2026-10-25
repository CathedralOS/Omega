# Nested Twenty-Six-Level Record Field Bound

Fact: a 26-level nested structural record RootRec { l1: Level1 { l2: Level2 { l3: Level3 { l4: Level4 { l5: Level5 { l6: Level6 { l7: Level7 { l8: Level8 { l9: Level9 { l10: Level10 { l11: Level11 { l12: Level12 { l13: Level13 { l14: Level14 { l15: Level15 { l16: Level16 { l17: Level17 { l18: Level18 { l19: Level19 { l20: Level20 { l21: Level21 { l22: Level22 { l23: Level23 { l24: Level24 { l25: Level25 { val: u64 } } } } } } } } } } } } } } } } } } } } } } } } } } preserves the leaf field bound 40u64 <= root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.l18.l19.l20.l21.l22.l23.l24.l25.val <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.

Every place performs the same operation: construct or transport a 26-level nested structural record `RootRec { l1: Level1 { l2: Level2 { l3: Level3 { l4: Level4 { l5: Level5 { l6: Level6 { l7: Level7 { l8: Level8 { l9: Level9 { l10: Level10 { l11: Level11 { l12: Level12 { l13: Level13 { l14: Level14 { l15: Level15 { l16: Level16 { l17: Level17 { l18: Level18 { l19: Level19 { l20: Level20 { l21: Level21 { l22: Level22 { l23: Level23 { l24: Level24 { l25: Level25 { val: u64 } } } } } } } } } } } } } } } } } } } } } } } } } }` with leaf field interval bound `40u64 <= root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.l18.l19.l20.l21.l22.l23.l24.l25.val <= 80u64`, project `res = root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.l18.l19.l20.l21.l22.l23.l24.l25.val`, discharge both headroom subtraction `80u64 - res` and margin subtraction `res - 40u64`, verify `headroom + margin == 40u64`, and exit 0. Only the transport mechanism of the nested record changes across places.

## Specification References

- [Layout Plans](wiki/spec/layouts/plans.md) — structural record layout geometry, cumulative field offset resolution, and nested record size/alignment calculation.
- [Structural Access](wiki/spec/terminal-psi/structural_access.md) — compile-time interval bound preservation through chained field projections (`ProjectField` chains across 26 levels) without loss of static bounds.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 26-level nested record construction, leaf projection, and exact subtraction discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 26-level nested record is copied to a local variable before leaf projection and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 26-level nested record is stored in a container record field before leaf projection and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive the 26-level nested record before leaf projection and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return the 26-level nested record before leaf projection and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield a 26-level nested record and join before leaf projection and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply the leaf value of a 26-level nested record before projection and interval discharge |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the 26-level nested record before leaf projection and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes the 26-level nested record to leaf projection |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies receives the 26-level nested record and discharges leaf bound |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/nested_twentysix_level_record_field_bound/main.omg
omega run --both samples/cli/facts/nested_twentysix_level_record_field_bound/local/main.omg
omega run --both samples/cli/facts/nested_twentysix_level_record_field_bound/field/main.omg
omega run --both samples/cli/facts/nested_twentysix_level_record_field_bound/parameter/main.omg
omega run --both samples/cli/facts/nested_twentysix_level_record_field_bound/call_result/main.omg
omega run --both samples/cli/facts/nested_twentysix_level_record_field_bound/match_join/main.omg
omega run --both samples/cli/facts/nested_twentysix_level_record_field_bound/const_name/main.omg
omega run --both samples/cli/facts/nested_twentysix_level_record_field_bound/other_state/main.omg
omega run --both samples/cli/facts/nested_twentysix_level_record_field_bound/other_machine/main.omg
omega run --both samples/cli/facts/nested_twentysix_level_record_field_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `val == 60u64`, `res == 60u64`, `headroom == 20u64`, `margin == 20u64`, and `headroom + margin == 40u64`. The transition `probe() == 40 { true -> done() _ -> bad() }` exits 0. If nested field projection or exact interval calculations evaluate incorrectly, execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, leaf `val` upper bound is widened to `81u64` so `res` can reach `81u64 > 80u64`. Proof obligation checking statically detects that `80u64 - res` can underflow `u64` below zero, failing closed with `may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/nested_twentysix_level_record_field_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/nested_twentysix_level_record_field_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time multi-level nested record projection bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, projecting the leaf field of a 26-level nested structural record `RootRec { l1: Level1 { l2: Level2 { l3: Level3 { l4: Level4 { l5: Level5 { l6: Level6 { l7: Level7 { l8: Level8 { l9: Level9 { l10: Level10 { l11: Level11 { l12: Level12 { l13: Level13 { l14: Level14 { l15: Level15 { l16: Level16 { l17: Level17 { l18: Level18 { l19: Level19 { l20: Level20 { l21: Level21 { l22: Level22 { l23: Level23 { l24: Level24 { l25: Level25 { val: u64 } } } } } } } } } } } } } } } } } } } } } } } } } }` preserves the static leaf interval bound (`root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.l18.l19.l20.l21.l22.l23.l24.l25.val >= 40u64 && root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.l18.l19.l20.l21.l22.l23.l24.l25.val <= 80u64`), discharging exact 64-bit unsigned subtractions `80u64 - res` and `res - 40u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis tracks leaf field constraints through 26 levels of structural record nesting (`RootRec` -> `Level1` -> `Level2` -> `Level3` -> `Level4` -> `Level5` -> `Level6` -> `Level7` -> `Level8` -> `Level9` -> `Level10` -> `Level11` -> `Level12` -> `Level13` -> `Level14` -> `Level15` -> `Level16` -> `Level17` -> `Level18` -> `Level19` -> `Level20` -> `Level21` -> `Level22` -> `Level23` -> `Level24` -> `Level25` -> `val`). The static leaf bound `[40, 80]` is established at record construction or verified via machine `requires`/`ensures` contracts. The established interval proves `res >= 40u64` and `res <= 80u64` unconditionally and guarantees `res` fits within `u64` (`>= 0 && <= 18446744073709551615`), directly proving that exact subtractions `80u64 - res >= 0` and `res - 40u64 >= 0` cannot underflow below zero without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 64-bit scalar leaf value and its interval bounds retain their precision, 64-bit stored width, domain tag, and non-negative invariants across 26 levels of structural record nesting, copies into local variables, container record fields (`Holder`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies without memory corruption, slicing, or loss of refined bounds.
  - **Composition:** The 26-level nested record projections compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtractions `80u64 - res` and `res - 40u64`, evaluating to `40u64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`val <= 80u64` vs `val <= 81u64`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `val <= 81u64`: interval analysis establishes `res in [40, 81]`, so exact subtraction `80u64 - res` can underflow `u64` below zero (`bounds [-1, 40] lie outside [0, 18446744073709551615]`). Compilation strictly rejects with `may overflow \`u64\``. Subtraction is statically certified against 64-bit unsigned underflow (`res <= 80`). No unproved boundary violation reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 26-level nested record projection and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing field access paths (`ProjectField` chains across 26 levels) while certifying exact integer arithmetic certificates (`SubExact` on `u64`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 64-bit stored width (8 bytes, align 8) for `u64` and nested record layouts (`Level25` size 8 align 8, `Level24` size 8 align 8, `Level23` size 8 align 8, `Level22` size 8 align 8, `Level21` size 8 align 8, `Level20` size 8 align 8, `Level19` size 8 align 8, `Level18` size 8 align 8, `Level17` size 8 align 8, `Level16` size 8 align 8, `Level15` size 8 align 8, `Level14` size 8 align 8, `Level13` size 8 align 8, `Level12` size 8 align 8, `Level11` size 8 align 8, `Level10` size 8 align 8, `Level9` size 8 align 8, `Level8` size 8 align 8, `Level7` size 8 align 8, `Level6` size 8 align 8, `Level5` size 8 align 8, `Level4` size 8 align 8, `Level3` size 8 align 8, `Level2` size 8 align 8, `Level1` size 8 align 8, `RootRec` size 8 align 8) validate against target layout plans with zero inner padding and cumulative field offset 0 to the scalar leaf. Target `CallPlan`/`StatePlan` assign record values to register lanes or indirect aggregate stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 64-bit displacement loads and subtractions (`sub r64, r64` on `x86_64`, `sub x, x, x` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **Pinned and recorded:** 18 fixture families (`..._through_<place>_{exit,rejected}` for the nine places) authored byte-for-byte from this sample tree. Measured on macOS arm64 with the `factsr1` shared producer built from base `4c23cc3391c` (`@4c23cc3391c2+wt` stamp, omega sha256 83c7ae2a45442b1ceb6f5476a6f8ed4c4af9eabbd85e682a3ed7254c7fec94cf, runner sha256 a1de848f1baca0837d28285d9da6da9acdd7639081c083743f76862a409bdb3d): nine `checked ... check-target:macos_arm64` and nine `rejected ... expected` rows in `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, nine `exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`; 0 keys lost in any of the three goldens (outcomes 12764 -> 12800, native 4159 -> 4177, both 2555 -> 2573) and 0 timeout rows at a 900 s per-fixture ceiling. No compiler repair was needed. A later main-change validation run with the newer `gen-9b001b8f6af` producer (revision `9b001b8f6af`) re-measured all 36 fixtures clean at `OMEGA_CORPUS_JOBS=1` (`corpus_gate: clean — 36 fixtures ({'checked': 18, 'rejected': 18}), 0 rejected without their expected fragment, 0 fail fixtures did not reject`) after 62 commits and 98 Rust files of compiler movement past the measurement, so the recorded verdicts survive that movement; the run's 15 `perf_faster` rows are timing noise, because the golden was recorded at `jobs=2` on a loaded host and this run measured at `jobs=1`.
- **What remains open:** Dynamic nested array-within-record projections, variant tagged payloads across nested structures, and mutable field updates across nested paths.
