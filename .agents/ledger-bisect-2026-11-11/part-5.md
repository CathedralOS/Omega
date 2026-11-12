# Ledger bisect part 5

PIN: `51e8c45c793007f211d0d9baee7de16767172444`

## Slice (unchanged)

| prior rows | measured | fixed at PIN | still broken at PIN |
| ---: | ---: | ---: | ---: |
| 34 | 34 | 23 | **11** |

Still-broken rows form **4** attribution clusters below (11 rows total).

---

## Cluster B — domain pattern (accepted)

| | |
|---|---|
| Witness | `pass/facts/domain_pattern_match_bound_through_field_exit` |
| First bad (PIN class) | `66ee9999011ed557510ddd18f20ceca6cb4aa5ae` (#1582) |
| Parent | `aea7dbe2ae380673377b4a49b91b173e01e3137c` |
| Witness @ parent / @ bad | 0 0 0 / 1 1 1 |

Member: `pass/facts/domain_pattern_match_bound_through_field_exit`.

---

## Cluster A1 — statement-successor, ledger break / `incomplete` (#1909)

**Only these five rows** are attributed to `d26fa952d579d52642970560c719d24fa2a89f8a` (parent `82d2247b96e872df79f26ec9b7af2fdec0153ce7`).

Witness `pass/facts/selected_native_code_layout_through_other_state_exit` at `d26fa952d579d52642970560c719d24fa2a89f8a`: exit **125, 125, 125** under strict PIN comparison (not exit 1 — lowering/`incomplete`, not the PIN statement-successor class).

| fixture | first ledger break | diagnostic at `d26fa952` |
| --- | --- | --- |
| `pass/facts/selected_native_code_layout_through_const_name_exit` | `d26fa952d579d52642970560c719d24fa2a89f8a` | `incomplete` (lowering) |
| `pass/facts/selected_native_code_layout_through_field_exit` | `d26fa952d579d52642970560c719d24fa2a89f8a` | `incomplete` |
| `pass/facts/selected_native_code_layout_through_local_exit` | `d26fa952d579d52642970560c719d24fa2a89f8a` | `incomplete` |
| `pass/facts/selected_native_code_layout_through_other_state_exit` | `d26fa952d579d52642970560c719d24fa2a89f8a` | `incomplete` |
| `pass/facts/selected_native_code_layout_through_target_body_exit` | `d26fa952d579d52642970560c719d24fa2a89f8a` | `incomplete` |

Parent `82d2247b96e872df79f26ec9b7af2fdec0153ce7`: witness **0 0 0**; all five match fixed PIN ledger (`checked`).

**First commit showing current PIN diagnostic class** (all ten statement rows): `33cefbc6c241048ff6f6c583acc5aca9b0176ec5` (#2238), parent `cdb14cc0c3e5b5bf3ea887f1c9d35737cab677d2`. Pin-class witness (`pass/facts/selected_native_code_layout_through_other_state_exit`) at parent/bad from completed manual bisect bracket: **0 at parent, 1 at bad** (see `/tmp/cluster-a-bisect-logs/*.log` probes `cdb14cc… rc=0`, `33cefbc6… rc=1`). **Mechanism not established** between #2238 float realization and statement-successor diagnostics (backend/target `float_realization.rs`; no quoted psi statement-successor hunk on this commit).

**Quoted hunk — ledger break at `d26fa952` (#1909)** (`fused.rs` / `successors.rs`, same as prior report).

---

## Cluster A2 — statement-successor, PIN class (#2238)

**Five rows** still ledger-good at `d26fa952`; first-bad for PIN broken outcome (bisect good=`d26fa952d579d52642970560c719d24fa2a89f8a`, bad=PIN, pin-class test):

| witness | first bad | PR | parent |
| --- | --- | --- | --- |
| each row (self witness) | `33cefbc6c241048ff6f6c583acc5aca9b0176ec5` | 2238 | `cdb14cc0c3e5b5bf3ea887f1c9d35737cab677d2` |

Members (one cluster — identical bisect result):  
`pass/calls/native_code_layout_discarded_exit`,  
`pass/facts/selected_native_code_layout_through_match_join_exit`,  
`pass/facts/source_indexed_family_alpha_positive`,  
`pass/facts/source_indexed_family_recursive_index_positive`,  
`pass/proofs/erased_mathematical_initializers`.

Bisect artifacts: `bisect_remain_*.json`, logs under `/tmp/cluster-a-bisect-logs/`.

**Mechanism:** mechanism not established (see Cluster A1).

---

## Not attributed as first-bad

- PIN `51e8c45c793007f211d0d9baee7de16767172444` — range endpoint only.
- Ledger-only `f3612364ffbb9489d7be1409397df6867fcbebc6` (#2228) — not used as first-bad.

---

## Method

- Fixed PIN ledger: `git show 51e8c45c793007f211d0d9baee7de16767172444:tests/omega/corpus_outcomes.txt`
- Per-commit `CARGO_TARGET_DIR=target/verify-<sha-prefix>`, worktrees, `OMEGA_CORPUS_JOBS=1`
- Exit **1** = exact PIN status + normalized diagnostic; **125** = any other outcome
- Pin-class bisect: `pin_diag_test.py` / `bisect_step_pin_diag.sh`; ledger-range bisect: `bisect_pin_manual.py` from `d26fa952d579d52642970560c719d24fa2a89f8a`

Verification: `verify_cluster_a_key.txt`, `verify_statement_cluster.txt`, `verify_domain_cluster.txt`.
