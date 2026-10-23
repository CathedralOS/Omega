# Nested Four-Level Record Field Bound

Fact: a 4-level nested structural record RootRec { outer: Outer { mid: Middle { inner: Inner { val: u64 } } } } preserves the leaf field bound 40u64 <= root.outer.mid.inner.val <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.

Owning Task: `CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS` (`TASKS.md`)
Claim Ticket: `01bcf18121304ba0bf9eef826cd05865`

Every place performs the same operation: construct or transport a 4-level nested structural record `RootRec { outer: Outer { mid: Middle { inner: Inner { val: u64 } } } }` with leaf field interval bound `40u64 <= root.outer.mid.inner.val <= 80u64`, project `res = root.outer.mid.inner.val`, discharge both headroom subtraction `80u64 - res` and margin subtraction `res - 40u64`, verify `headroom + margin == 40u64`, and exit 0. Only the transport mechanism of the nested record changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 4-level nested record construction, leaf projection, and exact subtraction discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the nested record is copied into a local variable before projecting and checking bounds |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the nested record is placed inside a container record field (`Container.root`) before projecting and checking bounds |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the nested record is passed as an explicit parameter before projecting and checking bounds |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a contracted helper call returns the nested record before projecting and checking bounds |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the nested record and join before projecting and checking bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the leaf value of the nested record before projecting and checking bounds |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives the nested record before projecting and checking bounds |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine sends the nested record to the verifying machine to discharge bounds |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies receives the nested record and discharges the leaf bound |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, declares
`Expected exit: 0`, and carries an authored `expected_native_exit.txt` (`0`).

Match the fact sentence beside every place: each positive place is pinned as `tests/omega/pass/facts/nested_four_level_record_field_bound_through_<place>_exit` and each reject twin as `tests/omega/fail/facts/nested_four_level_record_field_bound_through_<place>_rejected`, byte-equal to the place and `invalid/` programs beside them (main.omg, build.omg, and the exit or rejection oracle).

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/nested_four_level_record_field_bound/main.omg
omega run --both samples/cli/facts/nested_four_level_record_field_bound/local/main.omg
omega run --both samples/cli/facts/nested_four_level_record_field_bound/field/main.omg
omega run --both samples/cli/facts/nested_four_level_record_field_bound/parameter/main.omg
omega run --both samples/cli/facts/nested_four_level_record_field_bound/call_result/main.omg
omega run --both samples/cli/facts/nested_four_level_record_field_bound/match_join/main.omg
omega run --both samples/cli/facts/nested_four_level_record_field_bound/const_name/main.omg
omega run --both samples/cli/facts/nested_four_level_record_field_bound/other_state/main.omg
omega run --both samples/cli/facts/nested_four_level_record_field_bound/other_machine/main.omg
omega run --both samples/cli/facts/nested_four_level_record_field_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:

- At runtime, with `val == 60u64`, `res == 60u64`, `headroom == 20u64`, `margin == 20u64`, and `headroom + margin == 40u64`. The transition `probe() == 0 { true -> done() _ -> bad() }` exits 0. If four-level field projection or the exact interval calculations evaluate incorrectly, execution transitions to `state bad() { crash Trap; }`, which traps. Measured: perturbing the demanded result (`transition probe() == 0` to `== 1`) routes execution to `bad()` and traps both engines (`native signal 5` / `interp crash: Trap (host presentation agrees)`, exit 255), so the exit-0 oracle actively discriminates the computation.
- In each invalid twin, the leaf upper bound is widened to `81u64` so `res` can reach `81u64 > 80u64`. Proof obligation checking statically detects that `80u64 - res` can underflow `u64` below zero, failing closed with `may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/nested_four_level_record_field_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python3 tools/sample_native_gate.py --cli <gen>/omega --filter cli/facts/nested_four_level --jobs 2
```

Measured on macOS arm64 with the `factsr1` shared producer built from base `595562a958b` (`omega` sha256 `5c805564eb160e5714885a302f42b97dc34c85e59d877a96dbe685ec98c20fbd`, `corpus_runner` sha256 `fa7c9b282deaefd7f38eafd3b4ea07d960ad1eb7e679d23253fef05ed8ff30dd`):

- `omega run --both` reports `native exit 0` and `interp exit: 0` for the root and all nine places (10/10), Terminal Psi and native execution agreeing.
- `omega --check` refuses all nine `<place>/invalid/main.omg` twins with `may overflow \`u64\`` in the exact-arithmetic obligation class; `git diff` positive vs twin changes only the construct's own value, domain or contract (the widened `80u64`/`<= 81u64` bounds, the const-name place's leaf constant, and the call-result place's `ensures` bound and returned value).
- The nine positive places and nine reject twins are pinned as `tests/omega/{pass,fail}/facts/nested_four_level_record_field_bound_through_<place>_{exit,rejected}`, each byte-equal to the sample program it pins, with recorded corpus rows: nine `checked ... check-target:macos_arm64` and nine `rejected ... expected` rows carrying their own `may overflow \`u64\`` diagnostic in `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, and nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`.

The row's requirement: the leaf field bound `40u64 <= root.outer.mid.inner.val <= 80u64` of a 4-level nested structural record survives construction or transport through all nine places into the consuming exact subtractions, established statically without a post-hoc caller equality guard, with Terminal Psi and native agreement.

## Requirements and Evidence

- **Owning board item:** `CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS` (`TASKS.md`).
- **Claim ticket:** `01bcf18121304ba0bf9eef826cd05865`.
- **Requirement established:** projecting the leaf field of a 4-level nested structural record `RootRec { outer: Outer { mid: Middle { inner: Inner { val: u64 } } } }` carries the static leaf interval bound (`root.outer.mid.inner.val >= 40u64 && root.outer.mid.inner.val <= 80u64`) through construction, copies, aggregate fields, parameters, call returns, match joins, named constants, state transitions, forwarding machines and target-scoped bodies, discharging exact 64-bit unsigned subtractions `80u64 - res` and `res - 40u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** The 4-level nested structural record `RootRec { outer: Outer { mid: Middle { inner: Inner { val: u64 } } } }` encapsulates an unsigned 64-bit scalar leaf value. In each place, the leaf field is constrained to the interval $40u64 \le root.outer.mid.inner.val \le 80u64$ (instantiated with concrete value $60u64$ in probe). The compiler's path-sensitive typing and structural interval refinement statically prove that projecting through four levels of record access `root.outer.mid.inner.val` retains the bound $[40, 80]$. This statically satisfies the proof obligations for exact subtractions `let headroom: u64 = 80u64 - res;` and `let margin: u64 = res - 40u64;` without arithmetic overflow or underflow. When an invalid twin loosens the upper bound to $81u64$, exact subtraction `80u64 - res` cannot be proven safe and fails closed at compile time with `may overflow \`u64\``.
  - **Identity and lifetime:** The structural record values (`Inner`, `Middle`, `Outer`, `RootRec`) are copy-admitted aggregates. Deep projections across four levels of field access maintain exact storage offsets, value identity, and interval refinement through local variables, aggregate fields, machine parameters, return call results, match join bindings, constant definitions, state transition parameters, machine calls, and target-scoped bodies without memory corruption, slicing, or loss of refined bounds.
  - **Composition:** The preserved interval $[40, 80]$ composes across caller-callee boundaries, nested state transitions, match arm joins, and forwarding machines. Subtractions `80u64 - res` and `res - 40u64` are proven directly from the propagated field contract, requiring no post-hoc runtime assertions or dynamic caller bounds checks. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the construct's own value, domain or contract (the leaf bound `<= 80u64` vs `<= 81u64`, the const-name place's leaf constant, and the call-result place's `ensures` bound and returned value).
  - **Soundness boundaries:** Structural access paths adhere strictly to ownership, borrow, and type invariant rules. Relaxing any boundary in the nested path (such as allowing $val \le 81u64$) is caught by proof obligation analysis during exact arithmetic checking, rejecting unsound values before native lowering.
  - **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Source-level 4-level projections lower into sequences of target-neutral field projection operations in Terminal Psi (`PSITERM\0`). Erased mathematical contracts and interval certificates validate exact arithmetic operations (`SubExact` on `u64`), executing identically in the Terminal Psi interpreter (`omega run --both`).
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Structural record layout computation aligns each nested record type: `Inner` has `size_of = 8, align_of = 8`; `Middle` has `size_of = 8, align_of = 8`; `Outer` has `size_of = 8, align_of = 8`; `RootRec` has `size_of = 8, align_of = 8`. In calling conventions across all hosted targets, 64-bit aggregate records fit into 64-bit integer registers (`RCX`/`RDX` on Windows `x86_64`, `RDI`/`RSI` on Linux `x86_64`, `X0`/`X1` on `arm64`) or stack slots according to target ABI plans.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering loads the 64-bit leaf scalar from its nested struct offset (offset 0 across all 4 levels) into a 64-bit register (such as `rax`), and performs exact 64-bit subtractions (`sub`, `mov`) with constant immediates. On `arm64`, 64-bit register operations (`sub`, `mov`) execute exact subtraction on `X` registers. Activation frames maintain sufficient stack space for aggregate locals and spills.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves basic blocks and branches, linking entry routines and validating transitive machine-state footprints. Executables run natively and achieve bit-for-bit behavioral agreement with Terminal Psi, returning exit code 0 across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`).

- **Pinned and recorded:** 18 fixture families (`..._through_<place>_{exit,rejected}` for the nine places) authored byte-for-byte from this sample tree. Measured on macOS arm64 with the `factsr1` shared producer built from base `595562a958b` (`@595562a958b+wt` stamp): nine `checked ... check-target:macos_arm64` and nine `rejected ... expected` rows in `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, nine `exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`; 0 keys lost, 0 timeout rows. A later main-change validation run with the newer `gen-317bacd12b0` producer re-measured all 18 fixtures clean at `OMEGA_CORPUS_JOBS=2` (9 checked, 9 rejected, 0 rejected without their expected fragment, 0 moved rows), so the recorded verdicts survive the compiler movement after the measurement. No compiler repair was needed.
- **What remains open:** Dynamic nested array-within-record projections, variant tagged payloads across nested structures, mutable field updates across nested paths, and routes where a level is a `&`/`&mut` view or a computed (runtime) field selection. A deeper or wider copy of this same copy-transport mechanism adds no new compiler code path.
