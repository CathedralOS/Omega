# Ledger bisect part 21 (k=21, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## Summary

- Legacy report clusters in slice: **6** (report indices 21, 61, 101, 141, 181, 221)
- Fixture rows re-measured: **15**
- Fixed on PIN: **2**
- Still broken on PIN: **13**
- PIN diagnostic clusters (after regroup): **6**

## PIN clusters (regrouped by first diagnostic)

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 7 | pass/run rejected | cannot prove default-domain field requirement for return from write at statement 2: storage requ | `pass/facts/non_owning_reference_erasure_through_other_machine_exit` | `03397d546df477a8e4cc97db8896e20bae51a8cd` | 1227 | psi: elaborate bounded proofs through scoped continuations (#1227) (omega-rust/psi/compilation/proof | bisected |
Members:
- `pass/facts/non_owning_reference_erasure_through_call_result_exit` — still broken: cannot prove default-domain field requirement for return from write at statement 2: storage requires Storage::Issued
- `pass/facts/non_owning_reference_erasure_through_const_name_exit` — still broken: cannot prove default-domain field requirement for return from write at statement 2: storage requires Storage::Issued
- `pass/facts/non_owning_reference_erasure_through_field_exit` — still broken: cannot prove default-domain field requirement for return from write at statement 2: storage requires Storage::Issued
- `pass/facts/non_owning_reference_erasure_through_local_exit` — still broken: cannot prove default-domain field requirement for return from write at statement 2: storage requires Storage::Issued
- `pass/facts/non_owning_reference_erasure_through_other_machine_exit` — still broken: cannot prove default-domain field requirement for return from write at statement 1: storage requires Storage::Issued
- `pass/facts/non_owning_reference_erasure_through_other_state_exit` — still broken: cannot prove default-domain field requirement for return from write at statement 0: storage requires Storage::Issued
- `pass/facts/non_owning_reference_erasure_through_parameter_exit` — still broken: cannot prove default-domain field requirement for return from write at statement 1: storage requires Storage::Issued

| 2 | pass/run rejected | returned projection does not match this invocation and result path | `pass/borrows/dynamic_window_return_custody_exit` | `` |  | no_good_commit | propagated |
Members:
- `pass/borrows/dynamic_window_return_custody_exit` — still broken: returned projection does not match this invocation and result path
- `pass/borrows/runtime_slice_reference_result_exit` — still broken: returned projection does not match this invocation and result path

| 1 | incomplete | `X86Repair::repair` has no executable body: this compiler does not lower a statement of it yet ( | `pass/facts/recoverable_replacement_window_through_target_body_exit` | `` |  |  | pending |
Members:
- `pass/facts/recoverable_replacement_window_through_target_body_exit` — still broken: `X86Repair::repair` has no executable body: this compiler does not lower a statement of it yet (statement sequence: assignment: structural o

| 1 | pass/run rejected | cannot prove default-domain field requirement for return from ErasedWriteCode::write at statemen | `pass/facts/non_owning_reference_erasure_through_target_body_exit` | `` |  |  | pending |
Members:
- `pass/facts/non_owning_reference_erasure_through_target_body_exit` — still broken: cannot prove default-domain field requirement for return from ErasedWriteCode::write at statement 2: storage requires Storage::Issued

| 1 | pass/run rejected | inspection constant omitted its expected debug companion | `pass/terminal_psi/debug_identity_complete_module` | `` |  |  | pending |
Members:
- `pass/terminal_psi/debug_identity_complete_module` — still broken: inspection constant omitted its expected debug companion

| 1 | pass/run rejected | terminating machine drain reaches recursive component `step`, which has no `terminates by` ranki | `pass/control_flow/runtime_ranked_state_cycle_borrowed_record_exit` | `` |  |  | pending |
Members:
- `pass/control_flow/runtime_ranked_state_cycle_borrowed_record_exit` — still broken: terminating machine drain reaches recursive component `step`, which has no `terminates by` ranking -- spell one on that component


## Fixed on main (PIN)

- `fail/borrows/runtime_slice_result_retains_backing`
- `fail/facts/temporary_case_cleanup_through_other_machine_rejected`

## Sanity

741270df60f7 predates PIN; witnesses verified **bad** at PIN (`bisect_witness.sh` exit 1).
Good commits from `find_good_commit.py` (binary search on first-parent intro→PIN),
re-verified exit 0 before `git bisect start --first-parent`.

## Commands

```bash
cd /agent/repos/omega && git checkout 51e8c45c793007f211d0d9baee7de16767172444
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null
mbx test --no-run -p compiler --test corpus_runner < /dev/null
# OMEGA_CORPUS_FIXTURE_FILTER from part21_fixtures.txt
.agents/ledger-bisect-2026-11-11/run_bisect_cluster.sh <witness> <cN>
```

Host: linux cloud agent VM. `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`.
