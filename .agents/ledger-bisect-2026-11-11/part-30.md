# Ledger bisect part 30 (k=30, N=40)

Measured commit: `51e8c45c793007f211d0d9baee7de16767172444` (PIN). Host: cloud VM linux x86_64. `OMEGA_CORPUS_JOBS=1`, one build/corpus at a time.

## Slice summary (prior report clusters index % 40 == 30)

| Prior idx | Rows | Witness (prior) | On PIN |
| ---: | ---: | --- | --- |
| 30 | 6 | `pass/facts/recoverable_replacement_window_through_const_name_exit` | still broken (6) |
| 70 | 2 | `pass/facts/opaque_independent_inherited_result_through_local_exit` | split: 1 broken + 1 fixed path (see below) |
| 110 | 1 | `fail/layouts/resident_field_write_preserves_qualification` | **fixed on main** |
| 150 | 1 | `pass/contracts/borrowed_descriptor_guarded_end_exit` | **fixed on main** |
| 190 | 1 | `pass/effects/gui_semantic_services` | still broken |

Re-clustered still-broken at PIN (by first diagnostic): 4 clusters below (in progress).

## Clusters (still broken at PIN)

| Rows | Kind | Diagnostic on PIN | Witness | Breaking commit | PR | Mechanism | Status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 6 | pass/run rejected (incomplete) | `main` calls `repair`; `repair` has no executable body… | `pass/facts/recoverable_replacement_window_through_const_name_exit` | pending | | pending | bisecting |
| 1 | pass/run rejected (incomplete) | `AddressBridgeProvider::classified_make` has no executable body… | `pass/facts/opaque_independent_inherited_result_through_local_exit` | pending | | pending | queued |
| 1 | pass/run rejected (incomplete) | `CellReading::Reading::observed` has no executable body… | `pass/traits/runtime_qualified_owned_requirement_calls_exit` | pending | | pending | queued |
| 1 | pass/run rejected | `machine pace` omits `invokes DarwinClock` from its published synchronous-invocation ceiling… | `pass/effects/gui_semantic_services` | pending | | pending | queued |

## Members

### Cluster recoverable-repair (6)

- `pass/facts/recoverable_replacement_window_through_const_name_exit` still broken
- `pass/facts/recoverable_replacement_window_through_field_exit` still broken
- `pass/facts/recoverable_replacement_window_through_local_exit` still broken
- `pass/facts/recoverable_replacement_window_through_other_state_exit` still broken
- `pass/facts/recoverable_replacement_window_through_parameter_exit` still broken
- `pass/ownership/runtime_borrowed_match_repeated_repair_exit` still broken

### Cluster opaque-inherited (1)

- `pass/facts/opaque_independent_inherited_result_through_local_exit` still broken
- `pass/traits/runtime_qualified_owned_requirement_calls_exit` still broken (split diagnostic at PIN; own cluster)

### Cluster resident-field (fixed)

- `fail/layouts/resident_field_write_preserves_qualification` fixed on main

### Cluster borrowed-descriptor (fixed)

- `pass/contracts/borrowed_descriptor_guarded_end_exit` fixed on main

### Cluster gui-semantics (1)

- `pass/effects/gui_semantic_services` still broken

## Fixed on main at PIN

- `fail/layouts/resident_field_write_preserves_qualification`
- `pass/contracts/borrowed_descriptor_guarded_end_exit`

## Commands

```bash
git fetch --unshallow origin
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null
mbx test --no-run -p compiler --test corpus_runner < /dev/null
# direct runner (OMEGA_CORPUS_FIXTURE_FILTER + OMEGA_CORPUS_RECORDS)
bash .agents/ledger-bisect-2026-11-11/run_one_bisect.sh <witness> <good> 51e8c45c793007f211d0d9baee7de16767172444
```
