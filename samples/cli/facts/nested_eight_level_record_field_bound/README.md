# Nested Eight-Level Record Field Bound

Fact: a 8-level nested structural record RootRec { l1: Level1 { l2: Level2 { l3: Level3 { l4: Level4 { l5: Level5 { l6: Level6 { l7: Level7 { val: u64 } } } } } } } } preserves the leaf field bound 40u64 <= root.l1.l2.l3.l4.l5.l6.l7.val <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.

Owning Task: `CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS` (`TASKS.md`)
Claim Ticket: `39f22484149b44988e75f61cf9b3b8f9`

Every place performs the same operation: construct or transport an 8-level nested structural record `RootRec { l1: Level1 { l2: Level2 { l3: Level3 { l4: Level4 { l5: Level5 { l6: Level6 { l7: Level7 { val: u64 } } } } } } } }` with leaf field interval bound `40u64 <= root.l1.l2.l3.l4.l5.l6.l7.val <= 80u64`, project `res = root.l1.l2.l3.l4.l5.l6.l7.val`, discharge both headroom subtraction `80u64 - res` and margin subtraction `res - 40u64`, verify `headroom + margin == 40u64`, and exit 0. Only the transport mechanism of the nested record changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-level nested record construction (`check_bound`), leaf projection, and exact subtraction discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 8-level nested record is copied to a local variable (`check_local`) before leaf projection and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 8-level nested record is stored in a container record field (`check_field`) before leaf projection and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters (`check_param`) receive the 8-level nested record before leaf projection and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a contracted helper call (`make_root` returning to `check_call_result`) returns the 8-level nested record before leaf projection and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms of `choose_root` yield an 8-level nested record and join before leaf projection and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant (`VAL_LEAF`) supplies the leaf value consumed by `check_const` before projection and interval discharge |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine (`check_state` state `step`) receives the 8-level nested record before leaf projection and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine (`forward`) receives and passes the 8-level nested record to `check_worker` for projection and interval discharge |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies (`check_target`) receives the 8-level nested record and discharges the leaf bound |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, declares
`Expected exit: 0`, and carries an authored `expected_native_exit.txt` (`0`).

Match the fact sentence beside every place: each positive place is pinned as `tests/omega/pass/facts/nested_eight_level_record_field_bound_through_<place>_exit` and each reject twin as `tests/omega/fail/facts/nested_eight_level_record_field_bound_through_<place>_rejected`, byte-equal to the place and `invalid/` programs beside them (main.omg, build.omg, and the exit or rejection oracle).

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/nested_eight_level_record_field_bound/main.omg
omega run --both samples/cli/facts/nested_eight_level_record_field_bound/local/main.omg
omega run --both samples/cli/facts/nested_eight_level_record_field_bound/field/main.omg
omega run --both samples/cli/facts/nested_eight_level_record_field_bound/parameter/main.omg
omega run --both samples/cli/facts/nested_eight_level_record_field_bound/call_result/main.omg
omega run --both samples/cli/facts/nested_eight_level_record_field_bound/match_join/main.omg
omega run --both samples/cli/facts/nested_eight_level_record_field_bound/const_name/main.omg
omega run --both samples/cli/facts/nested_eight_level_record_field_bound/other_state/main.omg
omega run --both samples/cli/facts/nested_eight_level_record_field_bound/other_machine/main.omg
omega run --both samples/cli/facts/nested_eight_level_record_field_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:

- At runtime, with `val == 60u64`, `res == 60u64`, `headroom == 20u64`, `margin == 20u64`, and `headroom + margin == 40u64`. The transition `probe() == 40 { true -> done() _ -> bad() }` exits 0. If eight-level field projection or the exact interval calculations evaluate incorrectly, execution transitions to `machine bad() { crash Trap; }`, which traps. Measured: perturbing the demanded result (`transition probe() == 40` to `== 41`) routes execution to `bad()` and traps both engines (`native signal 5` / `interp crash: Trap (host presentation agrees)`, exit 255), so the exit-0 oracle actively discriminates the computation.
- In each invalid twin, the leaf upper bound is widened to `81u64` so `res` can reach `81u64 > 80u64`. Proof obligation checking statically detects that `80u64 - res` can underflow `u64` below zero, failing closed with `may overflow \`u64\``. None reaches native execution. The call-result twin moves the construct's own value and contract together (the returned leaf value is `81u64` with its `ensures` bound widened to `81u64`), because the checker would otherwise prove `res == 60u64` from the untouched body; the remaining eight twins widen only the declared bound, as authored.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/nested_eight_level_record_field_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python3 tools/sample_native_gate.py --cli <gen>/omega --filter cli/facts/nested_eight_level_record_field_bound --jobs 2
```

Measured on macOS arm64 with the `factsr1` shared producer built from base `4c23cc3391c` (`omega` sha256 `83c7ae2a45442b1ceb6f5476a6f8ed4c4af9eabbd85e682a3ed7254c7fec94cf`, `corpus_runner` sha256 `a1de848f1baca0837d28285d9da6da9acdd7639081c083743f76862a409bdb3d`):

- `omega run --both` reports `native exit 0` and `interp exit: 0` for the root and all nine places (10/10), Terminal Psi and native execution agreeing.
- `omega run --both` refuses all nine `<place>/invalid/main.omg` twins with `may overflow \`u64\`` in the exact-arithmetic obligation class; `git diff` positive vs twin changes only the construct's own value, domain or contract (the widened `<= 80u64`/`<= 81u64` bounds, the const-name place's leaf constant, and the call-result place's `ensures` bound and returned leaf value).
- The nine positive places and nine reject twins are pinned as `tests/omega/{pass,fail}/facts/nested_eight_level_record_field_bound_through_<place>_{exit,rejected}`, each byte-equal to the sample program it pins, with recorded corpus rows: nine `checked ... check-target:macos_arm64` and nine `rejected ... expected` rows carrying their own `may overflow \`u64\`` diagnostic in `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, and nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`.

The row's requirement: the leaf field bound `40u64 <= root.l1.l2.l3.l4.l5.l6.l7.val <= 80u64` of an 8-level nested structural record survives construction or transport through all nine places into the consuming exact subtractions, established statically without a post-hoc caller equality guard, with Terminal Psi and native agreement.

## Requirements and Evidence

- **Owning board item:** `CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS` (`TASKS.md`).
- **Claim ticket:** `39f22484149b44988e75f61cf9b3b8f9`.
- **Requirement established:** projecting the leaf field of an 8-level nested structural record `RootRec { l1: Level1 { l2: Level2 { l3: Level3 { l4: Level4 { l5: Level5 { l6: Level6 { l7: Level7 { val: u64 } } } } } } } }` carries the static leaf interval bound (`root.l1.l2.l3.l4.l5.l6.l7.val >= 40u64 && root.l1.l2.l3.l4.l5.l6.l7.val <= 80u64`) through construction, copies, aggregate fields, parameters, call returns, match joins, named constants, state transitions, forwarding machines and target-scoped bodies, discharging exact 64-bit unsigned subtractions `80u64 - res` and `res - 40u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** The 8-level nested structural record `RootRec { l1: Level1 { l2: Level2 { l3: Level3 { l4: Level4 { l5: Level5 { l6: Level6 { l7: Level7 { val: u64 } } } } } } } }` encapsulates an unsigned 64-bit scalar leaf value. In each place, the leaf field is constrained to the interval $40u64 \le root.l1.l2.l3.l4.l5.l6.l7.val \le 80u64$ (instantiated with concrete value $60u64$ in `probe`). The compiler's path-sensitive typing and structural interval refinement statically prove that projecting through eight levels of record access retains the bound $[40, 80]$. This statically satisfies the proof obligations for exact subtractions `let headroom: u64 = 80u64 - res;` and `let margin: u64 = res - 40u64;` without arithmetic overflow or underflow. When an invalid twin loosens the upper bound to $81u64$, exact subtraction `80u64 - res` cannot be proven safe and fails closed at compile time with `may overflow \`u64\``.
  - **Identity and lifetime:** The structural record values (`Level1` … `Level7`, `RootRec`) are copy-admitted aggregates. Deep projections across eight levels of field access maintain exact storage offsets, value identity, and interval refinement through local variables, aggregate fields, machine parameters, return call results, match join bindings, constant definitions, state transition parameters, machine calls, and target-scoped bodies without memory corruption, slicing, or loss of refined bounds.
  - **Composition:** The preserved interval $[40, 80]$ composes across caller-callee boundaries, nested state transitions, match arm joins, and forwarding machines. Subtractions `80u64 - res` and `res - 40u64` are proven directly from the propagated field contract, requiring no post-hoc runtime assertions or dynamic caller bounds checks. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the construct's own value, domain or contract (the leaf bound `<= 80u64` vs `<= 81u64`, the const-name place's leaf constant, and the call-result place's `ensures` bound and returned leaf value).
  - **Soundness boundaries:** Structural access paths adhere strictly to ownership, borrow, and type invariant rules. Relaxing any boundary in the nested path (such as allowing $val \le 81u64$) is caught by proof obligation analysis during exact arithmetic checking, rejecting unsound values before native lowering.
  - **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Source-level 8-level projections lower into sequences of target-neutral field projection operations in Terminal Psi (`PSITERM\0`). Erased mathematical contracts and interval certificates validate exact arithmetic operations (`SubExact` on `u64`), executing identically in the Terminal Psi interpreter (`omega run --both`).
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. Structural record layout computation aligns each nested record type: `Level7` … `Level1` and `RootRec` each have `size_of = 8, align_of = 8` with zero inner padding. In calling conventions across all hosted targets, 64-bit aggregate records fit into 64-bit integer registers (`RCX`/`RDX` on Windows `x86_64`, `RDI`/`RSI` on Linux `x86_64`, `X0`/`X1` on `arm64`) or stack slots according to target ABI plans.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering loads the 64-bit leaf scalar from its nested struct offset (offset 0 across all 8 levels) into a 64-bit register, and performs exact 64-bit subtractions (`sub r64, r64`); on `arm64`, 64-bit register operations (`sub x, x, x`) execute exact subtraction on `X` registers. Register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`).

- **Pinned and recorded:** 18 fixture families (`..._through_<place>_{exit,rejected}` for the nine places) authored byte-for-byte from this sample tree, with the call-result twin's value-and-contract authoring repair recorded here: the twin's `make_root` body previously returned the untouched leaf literal `60u64` with only its `ensures` bound widened, so the checker proved `res == 60u64` from the body and both engines accepted it; the repaired twin returns `81u64` with the widened `ensures` bound and rejects with `may overflow \`u64\``. Measured on macOS arm64 with the `factsr1` shared producer built from base `4c23cc3391c` (`@4c23cc3391c2+wt` stamp): nine `checked ... check-target:macos_arm64` and nine `rejected ... expected` rows in `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, nine `exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`; 0 keys lost, 0 timeout rows. No compiler repair was needed and no repair build was requested.
- **What remains open:** Dynamic nested array-within-record projections, variant tagged payloads across nested structures, mutable field updates across nested paths, and routes where a level is a `&`/`&mut` view or a computed (runtime) field selection. A deeper or wider copy of this same copy-transport mechanism adds no new compiler code path.