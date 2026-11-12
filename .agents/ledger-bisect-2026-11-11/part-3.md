# Ledger bisect part 3/40

**PIN:** `51e8c45c793007f211d0d9baee7de16767172444` (held fixed; never moved)

**Branch:** `work/ledger-bisect-part-3` (measurement only; no PR; no ledger/compiler fixture edits)

**Methodology (lead correction #2):** Fixed copies in `/tmp/pin_corpus_outcomes.txt` and `/tmp/pin_witness_snapshot.json` from PIN; `bisect_witness.sh` skips first-parent commits whose diff is only `tests/omega/corpus_outcomes.txt` and/or `*.md`; exit `0` = meets PIN ledger expectation, `1` = PIN-class failure, `125` = build/skip/other. Stale `corpus_runner` binaries are avoided by selecting the newest `*/out/corpus_runner-*` after each rebuild.

**Host:** Cursor Cloud, Omega environment (`linux_x86_64` corpus host `check-target:macos_arm64` in records).

**Commands:** `OMEGA_CORPUS_JOBS=1`; direct `corpus_runner` via `.agents/ledger-bisect-2026-11-11/tools/run_fixtures_direct.sh` for PIN sweep; bisect via `setup_pin_bisect.sh`, `bisect_witness.sh`, `run_cluster_bisect.sh` / `git bisect run`.

## Prior-report slice (index % 40 == 3)

| prior_idx | rows (prior) | norm diagnostic class (prior) | witness | PIN @51e8c45 |
|---:|---:|---|---|---|
| 3 | 33 `(+30 truncated)` | unproved `requires` after write before return | `fail/arithmetic/widened_result_predicate_invalid_write` | **fixed** (33/33 in PIN re-measure) |
| 43 | 4 | `data_where` overflow / standing mutable recast | `fail/facts/data_where_standing_mutable_recast_through_call_result_rejected` | **fixed** (2/2 present in part slice; others not in this part's fixture list) |
| 83 | 1 (+ twin) | cannot prove `compute` requires (`counter == 0`) | `pass/facts/match_computed_root_through_other_machine_exit` | **broken** (bisected) |
| 123 | 1 | unresolved `CheckedOperator` (`==`) | `pass/facts/indexed_crash_guard_through_target_body_exit` | **broken** (bisected) |
| 163 | 1 | subslice end bound vs unknown slice length | `pass/facts/nested_result_extent_through_field_exit` | **broken** (bisected) |
| 203 | 1 | no retained loan on nested borrow write dest | `pass/borrows/nested_reference_write_disjoint_facts_exit` | **broken** (bisected) |

PIN re-measure on 36 fixtures from this slice: **31 fixed**, **5 still broken** (regrouped into four bisect clusters below).

## Bisect clusters (still broken on PIN)

### Cluster PIN-A (prior idx 83; 2 rows)

- **Kind:** `pass` / facts
- **Diagnostic:** cannot prove requires for `compute` from `apply` (needs `counter == 0`)
- **Witness:** `pass/facts/match_computed_root_through_other_machine_exit` (twin: `…_target_body_exit`)
- **Good:** `6f0eea318690404f6d4e3d962434415493382377` (#2284 tooling)
- **First bad:** `d30fccc7aae65a2358024cf39788d1007e2661b4` — **#2276** `psi: prove top-level private machines from their own requires`
- **Mechanism:** top-level private machine bodies are proved from their own `requires` only, so caller-established facts (e.g. `counter == 0`) are no longer available when checking the `compute` call.

### Cluster PIN-B (prior idx 123; 1 row)

- **Kind:** `pass` / facts
- **Diagnostic:** authored `==` `CheckedOperator` unresolved after checking
- **Witness:** `pass/facts/indexed_crash_guard_through_target_body_exit`
- **Good:** `48b23bf0bb3103a4991e0bee43e2f446dea54d28` (#1901)
- **First bad:** `2ab64136c02d7879635a92e3e078d7374bec7590` — **#1811** `build: choose default float implementations through Build`
- **Mechanism:** float operator defaults routed through Build left the fixture's equality operator without a resolved checked operator selection.

### Cluster PIN-C (prior idx 163; 1 row)

- **Kind:** `pass` / facts
- **Diagnostic:** cannot prove subslice range end within unknown slice length
- **Witness:** `pass/facts/nested_result_extent_through_field_exit`
- **Good:** `05812db44c356c4bb96427b4e8dd286a895f8128` (#1028)
- **First bad:** `7dc788d0669bc3efa164d72015254dab1abe24dc` — **#730** `psi: complete prefix case identities and native product gates`
- **Mechanism:** prefix case identity / product gating changes dropped the slice bound fact the subslice proof needs.

### Cluster PIN-D (prior idx 203; 1 row)

- **Kind:** `pass` / borrows
- **Diagnostic:** cannot resolve borrowed destination; incoming descriptor has no retained loan
- **Witness:** `pass/borrows/nested_reference_write_disjoint_facts_exit`
- **Good:** `dfece9ee0666386d0fceb27ede2549b3302d82c8` (#2502; parent `e2e7876a03e` is ledger-only on first-parent and skipped)
- **First bad:** `51e8c45c793007f211d0d9baee7de16767172444` — **#2505** `psi: check a body over a const value parameter once and carry its proofs`
- **Mechanism:** lowered-form generic value-parameter checking and certificate carry disturb nested reference write loan retention on the nested destination path.

## Fixed on PIN (no bisect)

**Cluster prior idx 3 (requires restore, 33 rows):** all measured members reject with expected `cannot complete return … unproved requires` on PIN (witness `fail/arithmetic/widened_result_predicate_invalid_write`).

**Cluster prior idx 43 (`data_where`, rows in this part's list):** `fail/facts/data_where_standing_mutable_recast_through_{call_result,field}_rejected` reject as expected on PIN.

**Other slice fixtures (31 total):** see `classify_pin.json` `fixed` array and `pin_measure.txt`.

## Sanity

| witness | @PIN exit |
|---|---:|
| match_computed (other_machine) | 1 |
| indexed_crash_guard | 1 |
| nested_result_extent | 1 |
| nested_reference_write | 1 |

Good commits returned `0` in spot checks after runner selection fix (match: parent `6f0eea`; borrow: `dfece9`).