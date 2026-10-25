# Borrowed storage restoration window

Fact: retiring a borrow restoration window restores full owner access and post-borrow invariant value >= 100u64 && value <= 100u64 for exact subtraction value - 100u64.

Every positive place and reject twin defines or accesses a mutable `Cell` record containing `value: u64`.
Passing `&mut cell` to a mutating machine (`write_cell`) opens a borrowed-storage window that mutates `cell.value = 100u64`.
Upon return from the call, the borrow window completes and retires, restoring full owner access to the identical storage home.
The owner observes the restored value under invariant `val >= 100u64 && val <= 100u64`, discharging exact subtraction `v - 100u64` with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with local cell mutated across borrow restoration window |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | local binding receives restored cell before reading value |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the cell is stored in a record field and borrowed via projection |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the restored cell is passed as an immutable borrow to an observer machine |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the restored cell is returned from a dedicated constructor machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression read the value of the restored cell |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the written value and subtraction bound |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the restored value into a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine wraps the borrow restoration window |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body for the write call |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/borrowed_restoration_window/main.omg
omega run --both samples/cli/facts/borrowed_restoration_window/local/main.omg
omega run --both samples/cli/facts/borrowed_restoration_window/field/main.omg
omega run --both samples/cli/facts/borrowed_restoration_window/parameter/main.omg
omega run --both samples/cli/facts/borrowed_restoration_window/call_result/main.omg
omega run --both samples/cli/facts/borrowed_restoration_window/match_join/main.omg
omega run --both samples/cli/facts/borrowed_restoration_window/const_name/main.omg
omega run --both samples/cli/facts/borrowed_restoration_window/other_state/main.omg
omega run --both samples/cli/facts/borrowed_restoration_window/other_machine/main.omg
omega run --both samples/cli/facts/borrowed_restoration_window/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, the mutating call sets `cell.value = 100u64`.
Restoring the cell allows owner access to `val = cell.value`.
The transition guard `val >= 100u64 && val <= 100u64` statically proves that `v - 100u64` has no underflow or overflow,
transitioning to `done(val)` with exit code 0.
If any value is corrupted or the borrow write fails, the guard evaluates to false, transitioning to `bad() { crash Trap; }`.
Measured 1-token perturbation on `local/main.omg` (`cell.value = 100u64;` -> `cell.value = 99u64;`) traps both engines:
`native exit -1073741795` (`STATUS_ILLEGAL_INSTRUCTION` on `windows_x86_64`, `native signal 5` on `macos_arm64`) and `interp crash: Trap (host presentation agrees)`.

### Invalid controls, reject twins, and obligation class

Each place has an `invalid/` reject twin that creates a competing local borrow (`let held: &mut Cell = &mut cell;`)
held live across the write call (`held.value = 99u64` used after the call).
Because the storage is already borrowed by `held`, attempting to loan `cell` to `write_cell` inside the active borrow window violates loan exclusivity and fails closed at compile time:

```
while local borrow `held` is still active
```

specifically:

```
error: state `write_cell` receives `cell` while local borrow `held` is still active (borrowed at statement 1; its last use is at statement 3)
```

Removing the competing borrow restores clean compilation and zero exit.
The negative obligation is a static loan-exclusivity and borrow-window gate: overlapping mutable loans are decided and refused at compile time before any runtime realization, so no runtime trap is reachable for the loan-exclusivity violation itself, while the positive side's restored write is verified at runtime by both the Terminal Psi interpreter and native execution. Each place is pinned twice in the corpus goldens—on the check/outcome side (`tests/omega/corpus_outcomes.txt`) and on the native execution side (`tests/omega/corpus_native_windows_x86_64.txt` as `built ... exit:0`).

### Host-pinned macOS ARM64 native rows

The nine positives were measured and recorded on this host with

```sh
OMEGA_LIBRARY_ROOT=$PWD/source/library python3 tools/corpus_gate.py --native --record --filter facts/borrowed_restoration_window
```

producing nine rows in `tests/omega/corpus_native_macos_arm64.txt`, one per place
(`call_result`, `const_name`, `field`, `local`, `match_join`, `other_machine`,
`other_state`, `parameter`, `target_body`):

```
pass/facts/borrowed_restoration_window_through_<place>_exit built <ms>ms exit:0
```

The gate reports `clean — 9 fixtures ({'built': 9}) ... executed 9 (exit codes {'0': 9})`.
Producer: `omega` built at `opt-level = 1` (`omega --build-profile` reports
`{"opt_level":"1"}`), host `macos_arm64`.

### Obligation class: the window is statically checked, its restored value is observable at runtime

Both halves of this fact were probed directly, and they behave differently.

- **Static refusal (the loan discipline).** Holding a competing exclusive borrow `held` live
  across the write call is refused before lowering, at the call gate in
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/borrows/calls/conflicts.rs`.
  No probe reached a runtime trap on this path: the window violation cannot be seeded into
  execution because checking rejects the program first. That is the obligation's class, not a
  missing reachable trap.
- **Runtime trap (the restored value).** The restoration window's storage is genuinely
  observed, so the value it carries is a runtime fact. Corrupting only the value the callee
  stores — `cell.value = 99u64` instead of `100u64`, nothing else changed — makes the
  post-restoration guard false and traps in both engines: `native signal 5` /
  `interp crash: Trap` (exit 255). Restoring `100u64` returns `native exit 0` /
  `interp exit: 0`. A one-value delta flips both engines, so the use after restoration really
  depends on the restored storage rather than on a folded constant.

A post-restoration owner read through a still-live `held` loan is accepted and exits 0: the
check refuses the overlapping *call* and the overlapping *mutation*, while a later read whose
last use ends that loan does not overlap the window. That is precisely the discipline the nine
reject twins break.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`)
- **Pinned fixtures:**
  - Positive passes: `tests/omega/pass/facts/borrowed_restoration_window_through_<place>_exit/`
  - Negative controls: `tests/omega/fail/facts/borrowed_restoration_window_through_<place>_rejected/`

### Specification and Lowering Layers

1. **Establishment:** Passing a mutable borrow `&mut cell` to mutating callee `write_cell` stores `cell.value = 100u64`. Upon callee return, the loan window retires and restored owner access establishes post-borrow invariant `val >= 100u64 && val <= 100u64` directly from the restored storage state without any post-hoc caller equality guard (`transition x == 100`).
2. **Identity and lifetime:** The borrow tracks the exact storage identity and version of `cell` during the active loan window. The owner cannot access or reborrow `cell` while the borrow is active; once the call retires, the borrow restoration window closes and restores full owner access to the identical storage version across all 9 places.
3. **Composition:** The borrow restoration window composes through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation `v - 100u64`. In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in introducing a competing borrow (`let held: &mut Cell = &mut cell;`) that remains live across the write call site, keeping the construct, bound, and operation identical.
4. **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time (`--check`) with `error: state \`write_cell\` receives \`cell\` while local borrow \`held\` is still active` (or `receives \`holder.cell\`` / `state \`forward\` receives \`cell\``), enforcing borrow exclusivity and preventing overlapping mutable loans before storage restoration completes.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Borrowed references, call edges, and loan lifespans lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`) with explicit borrow restoration windows (`reborrow restored call uses`) and affine cleanup actions while erasing static proof annotations. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Record layout plans for `Cell` (size 8, align 8) validate, and `CallPlan`/`StatePlan` lower pointer and reference carriers across registers and stack slots according to target calling conventions.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 64-bit memory stores (`mov` / `str`), assigns register homes and disjoint stack-frame activation storage for `Cell`, and inserts entry prologues and multi-return restoration epilogues honoring the `StatePlan` ceiling.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements for record storage resolve to concrete offsets, basic blocks and branch targets relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets (`windows_x86_64` and `macos_arm64`).
