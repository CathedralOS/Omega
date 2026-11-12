# Ledger bisect part 15 (k=15, N=40)

Measured PIN `51e8c45c793007f211d0d9baee7de16767172444`.

## Slice summary

- Prior-report clusters in slice: **6** (18 member rows)
- Fixed on PIN: **16**
- Still broken at PIN (vs PIN ledger): **2**

## Still-broken clusters at PIN (regrouped by first diagnostic)

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 1 | pass | normal call `decode` … completed call result has no complete reference set… | `pass/facts/fixed_layout_byte_roundtrip_through_target_body_exit` | `c417f61c95ba` | 1880 | Psi borrow/reference custody (`invocation_results`, `returned_carriers`, `completed_backings`): completed call results no longer get a complete reference set when array contracts are preserved across calls (#1880). | bisected (fixed PIN context in /tmp) |
| 1 | pass | cannot prove default-domain field requirement for call check from main::entry: p… | `pass/facts/resident_backing_plain_ram_exit` | `0b8008fc683a` | 2430 | Checker place-origin summary for helper-returned layout views: `owned_storage_may_hold` routes view formation through `invocation_result_origin`, so writes through the returned view no longer overlap caller facts needed for default-domain field requirements (#2430). | bisected (fixed PIN context in /tmp) |

## Prior-report clusters (slice index % 40 == 15)

### Cluster index 15 (9 fixed, 1 still broken)
- Prior diagnostic: normal call `fill` (Handle { arena_index: 13523, generation: 1 }) in machine `select` state `entry` statement 0: complet…
- Witness (prior): `fail/facts/foreign_reference_result_through_call_result_rejected`
- `fail/facts/foreign_reference_result_through_call_result_rejected`: **fixed_on_main** — normal call `fill` (Handle { arena_index: 13523, generation: 1 }) in machine `select` state `entry` 
- `fail/facts/foreign_reference_result_through_const_name_rejected`: **fixed_on_main** — normal call `fill` (Handle { arena_index: 13527, generation: 1 }) in machine `journey` state `entry`
- `fail/facts/foreign_reference_result_through_field_rejected`: **fixed_on_main** — normal call `fill` (Handle { arena_index: 13526, generation: 1 }) in machine `journey` state `entry`
- `fail/facts/foreign_reference_result_through_local_rejected`: **fixed_on_main** — normal call `fill` (Handle { arena_index: 13523, generation: 1 }) in machine `journey` state `entry`
- `fail/facts/foreign_reference_result_through_match_join_rejected`: **fixed_on_main** — normal call `fill` (Handle { arena_index: 13525, generation: 1 }) in machine `first_choice` state `e
- `fail/facts/foreign_reference_result_through_other_machine_rejected`: **fixed_on_main** — normal call `fill` (Handle { arena_index: 13525, generation: 1 }) in machine `journey` state `entry`
- `fail/facts/foreign_reference_result_through_other_state_rejected`: **fixed_on_main** — normal call `fill` (Handle { arena_index: 13523, generation: 1 }) in machine `journey` state `entry`
- `fail/facts/foreign_reference_result_through_parameter_rejected`: **fixed_on_main** — normal call `fill` (Handle { arena_index: 13525, generation: 1 }) in machine `journey` state `entry`
- `fail/facts/foreign_reference_result_through_target_body_rejected`: **fixed_on_main** — normal call `fill` (Handle { arena_index: 13523, generation: 1 }) in machine `journey` state `entry`
- `pass/facts/fixed_layout_byte_roundtrip_through_target_body_exit`: **still_broken** — normal call `decode` (Handle { arena_index: 13643, generation: 1 }) in machine `roundtrip` state `en

### Cluster index 55 (3 fixed, 0 still broken)
- Prior diagnostic: domain constraint `Below` for the const leaf value is false; failed domain `Below`…
- Witness (prior): `fail/build/retained_domain_and_service_rows_invalid_index`
- `fail/build/retained_domain_and_service_rows_invalid_index`: **fixed_on_main** — domain constraint `Below` for the const leaf value is false; failed domain `Below`
- `fail/domains/logical_subject_false_constant`: **fixed_on_main** — domain constraint `Small` for the const leaf value is false; failed domain `u64::Small`
- `fail/facts/scoped_scalar_initializer_through_const_name_rejected`: **fixed_on_main** — domain constraint `BelowIndex` for the const leaf value is false; failed domain `BelowIndex`

### Cluster index 95 (2 fixed, 0 still broken)
- Prior diagnostic: state `write_pairs` receives `main` as mutable more than once…
- Witness (prior): `pass/recast/runtime_inline_slice_view_mutable_write_exit`
- `pass/recast/runtime_inline_slice_view_mutable_write_exit`: **fixed_on_main** — 
- `pass/recast/runtime_interior_slice_view_mutable_write_exit`: **fixed_on_main** — 

### Cluster index 135 (0 fixed, 1 still broken)
- Prior diagnostic: cannot prove default-domain field requirement for call check from main::entry: parameter pair requires Pair::Label…
- Witness (prior): `pass/facts/resident_backing_plain_ram_exit`
- `pass/facts/resident_backing_plain_ram_exit`: **still_broken** — cannot prove default-domain field requirement for call check from main::entry: parameter pair requir

### Cluster index 175 (1 fixed, 0 still broken)
- Prior diagnostic: division by zero in machine `quotient` state `entry` return value: the divisor is provably zero…
- Witness (prior): `fail/arithmetic/indexed_divisor_copied_selector_storage_write`
- `fail/arithmetic/indexed_divisor_copied_selector_storage_write`: **fixed_on_main** — division by zero in machine `quotient` state `entry` return value: the divisor is provably zero, whi

### Cluster index 215 (1 fixed, 0 still broken)
- Prior diagnostic: terminal-artifact production failed: Lowering(InvalidUnitMachinePlan…
- Witness (prior): `pass/host/process_exit_selected_frontier`
- `pass/host/process_exit_selected_frontier`: **fixed_on_main** — 

## Fixed-on-main members

- fail/facts/foreign_reference_result_through_call_result_rejected
- fail/facts/foreign_reference_result_through_const_name_rejected
- fail/facts/foreign_reference_result_through_field_rejected
- fail/facts/foreign_reference_result_through_local_rejected
- fail/facts/foreign_reference_result_through_match_join_rejected
- fail/facts/foreign_reference_result_through_other_machine_rejected
- fail/facts/foreign_reference_result_through_other_state_rejected
- fail/facts/foreign_reference_result_through_parameter_rejected
- fail/facts/foreign_reference_result_through_target_body_rejected
- fail/build/retained_domain_and_service_rows_invalid_index
- fail/domains/logical_subject_false_constant
- fail/facts/scoped_scalar_initializer_through_const_name_rejected
- pass/recast/runtime_inline_slice_view_mutable_write_exit
- pass/recast/runtime_interior_slice_view_mutable_write_exit
- fail/arithmetic/indexed_divisor_copied_selector_storage_write
- pass/host/process_exit_selected_frontier

## Bisect notes (Lead correction #2)

- PIN ledger + witness observations frozen under `/tmp/ledger-bisect-part-15/` (`pin_corpus_outcomes.txt`, per-witness `context_*.json`); bisect steps never compared against the per-revision checkout ledger.
- Ledger-only first-parent commits (`corpus_outcomes.txt` / `*.md` only) skipped with exit **125**.
- **`fixed_layout` / foreign-reference cluster:** first parent where the witness matches PIN-broken class is `c417f61c95ba2b5078d36f367dc9b3c8e2ea0e30` (#1880). Known-good probe in range: `2c180f24dbfe764285463ae71c1c454c7bcf7420` (rc=0). Prior ledger-only landing (e.g. f3612364 / #2228) is not used.
- **`resident_backing` cluster:** first parent matching PIN-broken class after ledger stamp `7c05fde3989273ffc82dd31925dfdfa2058be6ce` is `0b8008fc683a6b1057336ff1d7375aebee4e1a7b` (#2430).

## Commands

- Host: linux/x86_64, `OMEGA_CORPUS_JOBS=1`
- Direct runner: `OMEGA_CORPUS_FIXTURE_FILTER` + `OMEGA_CORPUS_RECORDS` (18-row slice)
- Bisect: `git bisect start --first-parent <PIN> <good>` + fixed `/tmp/ledger-bisect-part-15/` context
