# Ledger bisect part 25 (k=25, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## Slice and counts

Prior report clusters with index `% 40 == 25` (6 clusters, 14 member rows). Re-measured all members on the PIN with direct `corpus_runner` against a **fixed** copy of `tests/omega/corpus_outcomes.txt` at the PIN (not per-revision ledger during bisect).

| Class | Count |
| --- | ---: |
| fixed on main (PIN) | 8 |
| still broken (PIN) | 6 |
| **slice rows** | **14** |

After re-clustering still-broken rows by first diagnostic at the PIN: **2 clusters** (6 rows).

## Clusters (PIN, largest first)

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 5 | pass/run regression | returned view has no complete formation frontier | `pass/slices/runtime_indexed_descriptor_replacement_call_exit` | `f2c75870ebaf890328b787fa6d8ed50e868edbb8` | 1672 | psi helper-returned view authentication now requires a complete formation frontier for slice replacement returns | bisected |
| 1 | pass/run regression | `main` calls `update`; `update` has no executable body … stored reference dependencies | `pass/layouts/placed_primitive_returned_backing` | `c417f61c95ba2b5078d36f367dc9b3c8e2ea0e30` | 1880 | checked-unit / stored-reference lowering changes leave private helper `update` incomplete instead of reaching `checked` | bisected |

### Cluster 0 members (`c0-formation-frontier`)

- `pass/slices/runtime_indexed_descriptor_replacement_call_exit` still broken PIN: rejected — returned view has no complete formation frontier
- `pass/slices/runtime_nested_stored_view_return_exit` still broken PIN: rejected — returned view has no complete formation frontier
- `pass/slices/runtime_nested_view_candidate_backings_exit` still broken PIN: rejected — returned view has no complete formation frontier
- `pass/slices/runtime_stored_view_direct_window_argument_exit` still broken PIN: rejected — returned view has no complete formation frontier
- `pass/slices/runtime_stored_view_dynamic_selection_exit` still broken PIN: rejected — returned view has no complete formation frontier

### Cluster 1 members (`c1-placed-update-incomplete`)

- `pass/layouts/placed_primitive_returned_backing` still broken PIN: incomplete — `main` calls `update`; `update` has no executable body (stored reference dependencies)

## Fixed on main (PIN) — prior slice rows

- `fail/facts/slice_view_element_read_bound_through_other_machine_rejected`
- `fail/slices/stored_view_backing_mutated_while_returned_live`
- `fail/slices/stored_view_container_read_while_returned_live`
- `fail/traits/public_guarantee_wrong_formal`
- `pass/borrow/runtime_write_only_exclusive_reference_nested_call_operand_exit`
- `pass/dependent/state_arrival_byte_predicate_guarded_compile`
- `pass/recast/runtime_inline_shared_slice_record_read_exit`
- `pass/text/runtime_owned_string_byte_view_exit`

## Prior slice cluster mapping (report indices 25, 65, 105, 145, 185, 225)

| prior idx | prior rows | outcome at PIN |
| ---: | ---: | --- |
| 25 | 7 | 5 still broken → `c0-formation-frontier`; 2 fail twins fixed |
| 65 | 3 | all fixed |
| 105 | 1 | still broken → `c1-placed-update-incomplete` |
| 145 | 1 | fixed |
| 185 | 1 | fixed |
| 225 | 1 | fixed |

## Bisect methodology (post lead correction)

- `/tmp/pin_corpus_outcomes.txt` — ledger frozen at PIN.
- `/tmp/pin_bisect_expect.json` — per-witness PIN bad status + normalized first diagnostic.
- `bisect_test.py` — exit **1** when witness reproduces PIN break; **0** when tier-satisfying good outcome per frozen ledger; **125** otherwise.
- `bisect_witness.sh` — fresh `mbx build -p omega` + `corpus_runner` each step; **125** on ledger-only commits (`corpus_outcomes.txt` / `*.md` only).
- Good commit: walk first-parent from PIN until test returns **0** (not ledger stamps). Bisect: `git bisect start --first-parent`, mark PIN bad, good commit good, `git bisect run`.

### Sanity

| witness | PIN (51e8c45) | 741270df | first bad | before 741270? |
| --- | :---: | :---: | --- | :---: |
| `…indexed_descriptor_replacement…` | bad (1) | bad (1) | f2c75870 (#1672) | yes |
| `placed_primitive_returned_backing` | bad (1) | bad (1) | c417f61 (#1880) | yes |

Neither first-bad commit equals the PIN. Stability at good/bad/PIN ends: deterministic on spot checks (3×0 at `5797c45a64c`, 3×1 at `f2c75870ebaf` and PIN for cluster 0; `d012ed8f0ba` good, PIN bad for cluster 1).

## Commands

```bash
cd /agent/repos/omega
cp tests/omega/corpus_outcomes.txt /tmp/pin_corpus_outcomes.txt
# batch re-measure (14 fixtures):
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx test --no-run -p compiler --test corpus_runner < /dev/null
export OMEGA_CORPUS_ROOT=$PWD OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_JOBS=1 \
  OMEGA_CORPUS_FIXTURE_FILTER="$(paste -sd, /tmp/part25_fixtures.txt)" \
  OMEGA_CORPUS_RECORDS=/tmp/pin_part25_observed.txt
"$RUNNER" < /dev/null

export PIN_LEDGER=/tmp/pin_corpus_outcomes.txt PIN_BISECT_EXPECT=/tmp/pin_bisect_expect.json
export BISECT_WITNESS=pass/slices/runtime_indexed_descriptor_replacement_call_exit
git bisect start --first-parent
git bisect bad 51e8c45c793007f211d0d9baee7de16767172444
git bisect good 5797c45a64c3930e77eccbcc1c4381f8df37a48c
git bisect run .agents/ledger-bisect-2026-11-11/bisect_witness.sh

export BISECT_WITNESS=pass/layouts/placed_primitive_returned_backing
git bisect reset 51e8c45c793007f211d0d9baee7de16767172444
git bisect start --first-parent
git bisect bad 51e8c45c793007f211d0d9baee7de16767172444
git bisect good d012ed8f0baeeda2dc5870d15016f58953852ddd
git bisect run .agents/ledger-bisect-2026-11-11/bisect_witness.sh
```

Host: linux x86_64 cloud VM. Jobs: `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`.
