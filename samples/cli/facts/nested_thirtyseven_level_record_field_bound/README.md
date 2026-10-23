# Fact: 37-Level Nested Record Leaf Field Bound Preservation Across All Nine Places

```omega
// Fact: a 37-level nested structural record RootRec { l1: Level1 { l2: Level2 { l3: Level3 { l4: Level4 { l5: Level5 { l6: Level6 { l7: Level7 { l8: Level8 { l9: Level9 { l10: Level10 { l11: Level11 { l12: Level12 { l13: Level13 { l14: Level14 { l15: Level15 { l16: Level16 { l17: Level17 { l18: Level18 { l19: Level19 { l20: Level20 { l21: Level21 { l22: Level22 { l23: Level23 { l24: Level24 { l25: Level25 { l26: Level26 { l27: Level27 { l28: Level28 { l29: Level29 { l30: Level30 { l31: Level31 { l32: Level32 { l33: Level33 { l34: Level34 { l35: Level35 { l36: Level36 { val: u64 } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } preserves the leaf field bound 40u64 <= root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.l18.l19.l20.l21.l22.l23.l24.l25.l26.l27.l28.l29.l30.l31.l32.l33.l34.l35.l36.val <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.
```

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83` (row authoring); pin and landing
  claim `17a632a7386c4975b1937cb7395fa688` (`factsr1-pins-co`).
- **Governing specifications:**
  - `wiki/spec/layouts/plans.md` (Layout plans, record geometry, nested field alignment and displacement)
  - `wiki/spec/terminal-psi/structural_access.md` (Structural access paths, compile-time multi-level projections)
  - `wiki/spec/terminal-psi/integer_certificates.md` (Integer domain certificates, exact subtraction non-underflow)
  - `wiki/spec/build/calling_plans.md` (StatePlan / CallPlan parameter and return placement)

## Places

| Place | Positive (`Expected exit: 0`) | Reject Twin (`may overflow `u64``) |
|---|---|---|
| `(root)` | [main.omg](main.omg) | *(N/A - root fact witness)* |
| `local` | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| `field` | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| `parameter` | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| `call_result` | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| `match_join` | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| `const_name` | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| `other_state` | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| `other_machine` | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| `target_body` | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, declares
`Expected exit: 0`, and carries an authored `expected_native_exit.txt` (`0`).

Match the fact sentence beside every place: each positive place is pinned as `tests/omega/pass/facts/nested_thirtyseven_level_record_field_bound_through_<place>_exit` and each reject twin as `tests/omega/fail/facts/nested_thirtyseven_level_record_field_bound_through_<place>_rejected`, byte-equal to the place and `invalid/` programs beside them (main.omg, build.omg, and the exit or rejection oracle).

## The Oracle

Execute all positive places with:

```sh
omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/main.omg
omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/local/main.omg
omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/field/main.omg
omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/parameter/main.omg
omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/call_result/main.omg
omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/match_join/main.omg
omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/const_name/main.omg
omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/other_state/main.omg
omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/other_machine/main.omg
omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `val == 60u64`, `res == 60u64`, `headroom == 20u64`, `margin == 20u64`, and `headroom + margin == 40u64`. The transition `probe() == 40 { true -> done() _ -> bad() }` exits 0. If nested field projection or exact interval calculations evaluate incorrectly, execution transitions to `state bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, leaf `val` upper bound is widened to `81u64` so `res` can reach `81u64 > 80u64`. Proof obligation checking statically detects that `80u64 - res` can underflow `u64` below zero, failing closed with `may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/nested_thirtyseven_level_record_field_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
<gen>/omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/main.omg
<gen>/omega run --both samples/cli/facts/nested_thirtyseven_level_record_field_bound/<place>/main.omg      # all nine places
<gen>/omega --check samples/cli/facts/nested_thirtyseven_level_record_field_bound/<place>/invalid/main.omg # all nine twins
```

The sample author's earlier record (`sample_native_gate.py` on `windows_x86_64` with a
shared `omega-latest.exe`) reports 10/10 samples meeting their documented oracle and 9/9
intended-invalid controls matching `may overflow \`u64\``. That host and binary are not
available here, so the row's pinned evidence is the macOS arm64 measurement below.

Measured on macOS arm64 with the `factsr1` shared producer built from base
`4c23cc3391c` (`omega` sha256 `83c7ae2a45442b1ceb6f5476a6f8ed4c4af9eabbd85e682a3ed7254c7fec94cf`, `corpus_runner` sha256 `a1de848f1baca0837d28285d9da6da9acdd7639081c083743f76862a409bdb3d`):

- `omega run --both` reports `native exit 0` and `interp exit: 0` for the root and all
  nine places (10/10), Terminal Psi and native execution agreeing.
- `omega --check` refuses all nine `<place>/invalid/main.omg` twins with
  `may overflow \`u64\`` in the exact-arithmetic obligation class; `git diff` positive vs
  twin changes only the construct's own value, domain or contract (the widened
  `80u64`/`<= 81u64` bounds on each place's `requires`/`ensures` lines, the const-name
  place's leaf constant `60u64`/`81u64`, and the header's place line).
- The nine positive places and nine reject twins are pinned as
  `tests/omega/{pass,fail}/facts/nested_thirtyseven_level_record_field_bound_through_<place>_{exit,rejected}`, each
  byte-equal to the sample program it pins, with recorded corpus rows: nine
  `checked ... check-target:macos_arm64` and nine `rejected ... expected` rows carrying
  their own `may overflow \`u64\`` diagnostic in `tests/omega/corpus_outcomes.txt`, nine
  `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, and nine
  `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in
  `tests/omega/corpus_both_macos_arm64.txt`.

The row's requirement: the leaf field bound `40u64 <= root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.l18.l19.l20.l21.l22.l23.l24.l25.l26.l27.l28.l29.l30.l31.l32.l33.l34.l35.l36.val <= 80u64` of a
37-level nested structural record survives construction or transport through
all nine places into the consuming exact subtractions, established statically without a
post-hoc caller equality guard, with Terminal Psi and native agreement.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time multi-level nested record projection bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, projecting the leaf field of a 37-level nested structural record `RootRec` preserves the static leaf interval bound (`root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.l18.l19.l20.l21.l22.l23.l24.l25.l26.l27.l28.l29.l30.l31.l32.l33.l34.l35.l36.val >= 40u64 && root.l1.l2.l3.l4.l5.l6.l7.l8.l9.l10.l11.l12.l13.l14.l15.l16.l17.l18.l19.l20.l21.l22.l23.l24.l25.l26.l27.l28.l29.l30.l31.l32.l33.l34.l35.l36.val <= 80u64`), discharging exact 64-bit unsigned subtractions `80u64 - res` and `res - 40u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis tracks leaf field constraints through 37 levels of structural record nesting (`RootRec` -> `Level1` -> `Level2` -> ... -> `Level36` -> `val`). The static leaf bound `[40, 80]` is established at record construction or verified via machine `requires`/`ensures` contracts. The established interval proves `res >= 40u64` and `res <= 80u64` unconditionally and guarantees `res` fits within `u64` (`>= 0 && <= 18446744073709551615`), directly proving that exact subtractions `80u64 - res >= 0` and `res - 40u64 >= 0` cannot underflow below zero without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 64-bit scalar leaf value and its interval bounds retain their precision, 64-bit stored width, domain tag, and non-negative invariants across 37 levels of structural record nesting, copies into local variables, container record fields (`Holder`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies without memory corruption, slicing, or loss of refined bounds.
  - **Composition:** The 37-level nested record projections compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtractions `80u64 - res` and `res - 40u64`, evaluating to `40u64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`val <= 80u64` vs `val <= 81u64`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `val <= 81u64`: interval analysis establishes `res in [40, 81]`, so exact subtraction `80u64 - res` can underflow `u64` below zero (`bounds [-1, 40] lie outside [0, 18446744073709551615]`). Compilation strictly rejects with `may overflow \`u64\``. Subtraction is statically certified against 64-bit unsigned underflow (`res <= 80`). No unproved boundary violation reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 37-level nested record projection and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing field access paths (`ProjectField` chains across 37 levels) while certifying exact integer arithmetic certificates (`SubExact` on `u64`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 64-bit stored width (8 bytes, align 8) for `u64` and nested record layouts (`Level36` through `Level1` and `RootRec`, each size 8 align 8) validate against target layout plans with zero inner padding and cumulative field offset 0 at each level. Target `CallPlan`/`StatePlan` assign record values to register lanes or indirect aggregate stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 64-bit displacement loads and subtractions (`sub r64, r64` on `x86_64`, `sub x, x, x` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **Pinned and recorded:** 18 fixture families (`..._through_<place>_{exit,rejected}`
  for the nine places) authored byte-for-byte from this sample tree. Measured on macOS
  arm64 with the `factsr1` shared producer built from base `4c23cc3391c` (stamp `@4c23cc3391c2+wt`):
  nine `checked ... check-target:macos_arm64` and nine `rejected ... expected` rows in
  `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in
  `tests/omega/corpus_native_macos_arm64.txt`, and nine
  `exit:0 interpreter-exit:0 interpreter:agrees` rows in
  `tests/omega/corpus_both_macos_arm64.txt`; 0 fixture keys lost, 0 timeout rows. No
  compiler repair was needed.
- **What remains open:** Dynamic nested array-within-record projections, variant tagged payloads across nested structures, and mutable field updates across nested paths.
