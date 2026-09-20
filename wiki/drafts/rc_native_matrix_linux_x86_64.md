# RC native matrix — linux_x86_64 row

Witnessed row of the release-candidate native matrix on the Linux x86-64
host. Recorded at revision `6ef64f6dd6` (2026-09-20), host
`x86_64-unknown-linux-gnu`, pinned toolchain `nightly-2026-09-04`,
cargo-nextest (mbx unavailable). Every command below was executed on this
host at that revision; legs that only cross-emit here are marked.

Verdict: **red** — 24 pass / 14 fail across 38 legs, identical to the
`0977a4249e` recording: every green leg stays green and all 14 sysv legs
still refuse at product admission with `native-artifact production
requires one exact selected program entry` — hosted `ProgramEntry` is the
only bindable root slot and admits no visible parameters or result, so
the param-carrying boundary-machine fixtures cannot select an entry at
all. The ENTRY-CONTENT-ROOTS residual owning that refusal is still open
on the board.

## Baseline gates

| leg | command | result |
|-----|---------|--------|
| Bootstrap topology/path hygiene | `sh tools/bootstrap/check-chain-hygiene.sh` | pass (exit 0) |
| Retired-domain corpus audit | `cargo nextest run -p compiler --test canary_suite -E 'test(=surface_and_targets::retired_domain_when_surface_is_absent_from_authored_corpus)'` | pass (3.5s) |

## Native legs (canary_suite, `-p compiler --test canary_suite`)

| leg | filter | result |
|-----|--------|--------|
| Hosted receiver bridge | `hosted_receiver_linux` substring (module `entry_and_abi::hosted_receiver_linux`, 5 tests) | 5/5 |
| Hosted receiver, aarch64 cross-emit | same run (module `hosted_receiver_linux_arm64`, 3 tests) | 3/3 |
| SysV x86-64 entry ABI | `sysv_entry_abi` substring (14 tests) | 0/14 |
| Entry calling policy | `cargo nextest run -p compiler --test calling_policy_plans -E 'test(/^linux_entry::/)'` (module covers linux_x86_64 + linux_arm64 surfaces, 6 tests) | 6/6 |
| Source-evaluated native realization | `cargo nextest run -p compiler --test source_evaluated_native_realization -E 'test(/linux_/) and not test(/arm64/)'` (8 tests) | 8/8 |

- `linux_free_unit_entry_runs_and_completes_with_status_zero` remains the
  green free-entry leg; the two bare-interface rejection legs and both
  provisioning legs are now green on both hosts' modules.
- Executed-on-host evidence that remains green: `linux_dynamic_execution::
  boundary_requirement_executes_its_foreign_call_on_linux_x64` runs the
  foreign call natively; GOT/PLT import-slot custody and dynamic ELF custody
  legs pass; `linux_dynamic_realization::import_bearing_linux_compiler_route`
  is slow (~66s) but passes.
- `hosted_receiver_linux_arm64` legs now compile the fixture fine and are
  cross-emit-only — they are green everywhere since they never execute an
  aarch64 binary.

## Failure families

The 14 failures now reduce to one recorded residual:

1. **Param-carrying boundary entry cannot select a root** (14 sysv legs).
   Every fixture compiles through checking cleanly — the exact-arithmetic
   obligation was respelled onto operands with legacy scalar range annotations,
   and the borrowed-storage result transfers now construct the returned record
   from storage fields — and all 14 legs refuse at product admission with
   `native-artifact production requires one exact selected program entry`.
   These are recorded outcomes for the unchanged fixtures, not acceptance of the
   removed annotation syntax. `REMOVE-BRACKETED-RANGE-ANNOTATIONS` on the
   [board](../../TASKS.md) owns their migration to contracts or named domains.
   `builder.roots.bind(linux_x86_64::ProgramEntry, Main::main)` does not
   fix them: the hosted `ProgramEntry` schema (`HostedApplication`,
   `visible_parameters: None`) rejects a machine that declares arrival
   parameters or a result — the fixtures' whole point. A param/result-
   carrying boundary entry needs a root surface that does not exist yet.
   Owner family: **ENTRY-CONTENT-ROOTS** mechanism work, not fixture
   spelling. The aarch64 siblings fail identically (11/11 at entry
   selection), so the boundary-ABI corpus shares this one residual.

   Closed legs since the `e76d715c8e` row:
   - `linux_hosted_receiver::linux_hosted_receiver_{normal_return_
     provisions_zii_storage_and_console,explicit_exit_process_preserves_
     its_distinct_outcome}` — the `Service<Console> in Bound` residue was
     respelled between `e76d715c8e` and `0977a4249e`; both legs pass
     natively (~73s/~73s at this revision).
   - The seven exact-arithmetic aggregate-entry fixtures and six
     borrowed-storage result fixtures were respelled in this revision
     (bounded operands / storage-field construction); their refusal moved
     from checking to the uniform entry-selection diagnostic above.
   - `sysv_hfa_entry_argument_packs_eightbytes_into_xmm_registers` shares
     the same residual — no `build.omg` root bind is addable because no
     admitted slot accepts its signature.

## Row gaps

- Adjacent entry legs sampled for context and not counted above:
  `program_entries_and_image_validation` x2 and `aarch64_entry_abi` x11
  fail with the same entry-selection residual.
- `calling_policy_plans` counts include the module's linux_arm64 policy
  legs (host-independent plan evaluation); they are green.
- Not run: `canary_suite` full corpus (recorded red elsewhere),
  `workspace --lib` baseline, macOS/Windows/UEFI host legs (other rows),
  UEFI runtime (no QEMU/firmware on this host).
