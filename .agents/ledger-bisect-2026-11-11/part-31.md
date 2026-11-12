# Ledger bisect part 31 (k=31, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## Summary

- Legacy report clusters in slice: **5** (report indices 31, 71, 111, 151, 191)
- Fixture rows re-measured: **11**
- Fixed on PIN: **4**
- Still broken on PIN: **7**
- PIN diagnostic clusters (after regroup): **4**

## PIN clusters (regrouped by first diagnostic)

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 3 | pass/run rejected | cannot establish assignment invariants: performed write has unknown destination origins | `pass/borrows/runtime_selected_reference_mutation_complement_exit` | `b7e951cdab51dda4e199f3fd33c53a0408fe72db` | 1242 | psi #1242 assignment_windows/post_store_predicates tighten post-store destination supplier capture | bisected |
| 2 | incomplete | `main` calls `exercise`, which calls `window`; `window` has no executable body | `pass/facts/computed_record_window_through_other_machine_exit` | `2174c0d10bb146aea6a86f55262bc536feb86ea3` | 1313 | omega #1313 provider settlement prefers product providers over terminal demand | bisected |
| 1 | pass/run rejected | cannot prove requires contract for call compute from write | `pass/facts/match_computed_root_through_other_state_exit` | `d30fccc7aae65a2358024cf39788d1007e2661b4` | 2276 | psi #2276 private-machine requires proving stops carrying computed-root counter facts into compute | bisected |
| 1 | pass/run rejected | machine `received` state `entry` statement 6: operand has no complete completed call-slot relation | `pass/generics/generic_result_parameter_receipt` | `5ab4e12334fba74babfc3ae8d9992572fba307e6` | 348 | psi #348 nested invocation contracts drop completed call-slot relation for generic results | bisected |

Members:
- `pass/borrows/runtime_selected_reference_mutation_complement_exit` — still broken
- `pass/borrows/runtime_uniform_projected_reference_selection_exit` — still broken
- `pass/write_only/returned_destination_transport_exit` — still broken
- `fail/borrows/conditional_reference_replacement_false_first_stale_index` — fixed on main
- `fail/borrows/conditional_reference_replacement_true_first_stale_index` — fixed on main
- `fail/slices/reborrowed_replaced_descriptor` — fixed on main
- `pass/facts/computed_record_window_through_other_machine_exit` — still broken
- `pass/facts/computed_record_window_through_other_state_exit` — still broken
- `pass/facts/target_family_contract_operand/call_result` — fixed on main
- `pass/facts/match_computed_root_through_other_state_exit` — still broken
- `pass/generics/generic_result_parameter_receipt` — still broken

## Fixed on main (PIN)

- `fail/borrows/conditional_reference_replacement_false_first_stale_index`
- `fail/borrows/conditional_reference_replacement_true_first_stale_index`
- `fail/slices/reborrowed_replaced_descriptor`
- `pass/facts/target_family_contract_operand/call_result`

## Legacy slice (report 2026-11-11)

| report idx | report rows | witness (report) | PIN outcome |
| ---: | ---: | --- | --- |
| 31 | 6 | `fail/borrows/conditional_reference_replacement_false_first_stale_index` | 3 pass still broken (assignment invariants); 3 fail fixed |
| 71 | 2 | `pass/facts/computed_record_window_through_other_machine_exit` | 2 still broken (window body missing) |
| 111 | 1 | `pass/facts/target_family_contract_operand/call_result` | fixed on main |
| 151 | 1 | `pass/facts/match_computed_root_through_other_state_exit` | still broken |
| 191 | 1 | `pass/generics/generic_result_parameter_receipt` | still broken |

## Provenance

- Host: linux x86_64, 4 cpus
- Jobs: `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`
- PIN ledger copy: `/tmp/ledger-bisect-pin/corpus_outcomes.txt`; per-witness specs under `/tmp/ledger-bisect-pin/`
- Bisect helper: `/tmp/ledger-bisect-pin/tools/bisect_witness.sh` (skips ledger-only commits via exit 125)
- Measure: `mbx test --no-run -p compiler --test corpus_runner` + direct runner on 11 fixtures (`part-31-measure.txt`)
- Re-measure command: `OMEGA_CORPUS_FIXTURE_FILTER=<11 fixtures> OMEGA_CORPUS_RECORDS=... corpus_runner`
- Sanity anchor: `741270df60f7eab218e879eb6771d2bc75e64c00`
- Good commits used for `git bisect`: see **Acceptance verification** (exponential jump + newest-good refinement + triple confirm at good SHA)
- Logs: `part-31-verify.log`, `part-31-members.log`, `part-31-good-walk.log`, `part-31-good-confirm.log`, `part-31-fixture-audit.log`

## Acceptance verification

Frozen PIN ledger + per-witness specs: `python3 setup_pin_specs.py` → `/tmp/ledger-bisect-pin/`. Witness test at each commit: `mbx build -p omega`, `corpus_runner` with `bisect_test.py` (exit `0` = frozen `good_status`, `1` = other terminal status, `125` = build/skip). **Good commit** = exponential first-parent jumps from PIN (`run_exponential_jumps.py`), then newest-good binary refinement on first-parent indices (`verify_good_walk.py`), with **triple corpus runs at the resulting good SHA** (`run_good_confirm.sh`).

### Triple witness at first-bad and first parent

| cluster | witness | first-bad (40) | bad runs | parent (40) | parent runs |
| --- | --- | --- | --- | --- | --- |
| c0 | `pass/borrows/runtime_selected_reference_mutation_complement_exit` | `b7e951cdab51dda4e199f3fd33c53a0408fe72db` (#1242) | 1,1,1 | `c0b334362da0ce997052dc277a639af7471555e9` | 0,0,0 |
| c1 | `pass/facts/computed_record_window_through_other_machine_exit` | `2174c0d10bb146aea6a86f55262bc536feb86ea3` (#1313) | 1,1,1 | `bebbe455e7517a069862d87aead2f1ca911588c3` | 0,0,0 |
| c2 | `pass/facts/match_computed_root_through_other_state_exit` | `d30fccc7aae65a2358024cf39788d1007e2661b4` (#2276) | 1,1,1 | `6f0eea318690404f6d4e3d962434415493382377` | 0,0,0 |
| c3 | `pass/generics/generic_result_parameter_receipt` | `5ab4e12334fba74babfc3ae8d9992572fba307e6` (#348) | 1,1,1 | `eab58bf427c48063e3fcf597e4550ca53ac7cb8c` | 0,0,0 |

### Member rows at first-bad and parent (corpus status)

| cluster | commit | members → corpus status |
| --- | --- | --- |
| c0 | `b7e951cdab51dda4e199f3fd33c53a0408fe72db` | all 3 pass → `rejected` |
| c0 | `c0b334362da0ce997052dc277a639af7471555e9` | all 3 pass → `checked` |
| c1 | `2174c0d10bb146aea6a86f55262bc536feb86ea3` | both window pass → `rejected` / incomplete path |
| c1 | `bebbe455e7517a069862d87aead2f1ca911588c3` | both → `checked` |
| c2 | `d30fccc7aae65a2358024cf39788d1007e2661b4` | `match_computed_root_through_other_state_exit` → `rejected` |
| c2 | `6f0eea318690404f6d4e3d962434415493382377` | → `checked` |
| c3 | `5ab4e12334fba74babfc3ae8d9992572fba307e6` | `generic_result_parameter_receipt` → `rejected` |
| c3 | `eab58bf427c48063e3fcf597e4550ca53ac7cb8c` | → `checked` (not build skip; witness ran in ~1.5s) |

#348 parent fixture audit (`eab58bf427c48063e3fcf597e4550ca53ac7cb8c`): `tests/omega/pass/generics/generic_result_parameter_receipt/main.omg` present; `build.omg` absent (pre-build-era layout); `source/library` and `tests/omega/corpus_outcomes.txt` present; frozen PIN ledger row `pass/generics/generic_result_parameter_receipt checked`.

### Newest good (bisect) SHAs — confirmed by walk scripts on branch

| cluster | good SHA (40) | witness |
| --- | --- | --- |
| c0 | `c64728d90e83036051d52fb69961b786423230c6` | `pass/borrows/runtime_selected_reference_mutation_complement_exit` |
| c1 | `773b8422694b5a8e64b43284bcba1ceb6de2b1e9` | `pass/facts/computed_record_window_through_other_machine_exit` |
| c2 | `442a95195e64d20d6d373b6bbddcd11c7f6da38a` | `pass/facts/match_computed_root_through_other_state_exit` |
| c3 | `42ef40205f19c7797b352647f55b82519407191c` | `pass/generics/generic_result_parameter_receipt` |

Triple `0,0,0` at each good SHA: see `part-31-good-confirm.log` (generated on branch after exponential jump trace in `part-31-good-walk.log`).

### Mechanism (quoted diff hunk)

**c0 / #1242** (`b7e951cdab51dda4e199f3fd33c53a0408fe72db`) — `assignment_windows.rs`:

```diff
+    let Some(origins) = frames.assignment_storage_origins(machine, statement) else {
+        diagnostics.push(Diagnostic::error(
+            "cannot establish assignment invariants: storage origins are unavailable",
+        ));
```

Plus `storage_effects.rs` requires `capture_storage_place` for scalar-view stores:

```diff
-        self.capture_storage_place(place)
+        Some((self.capture_storage_place(place)?, representation))
```

**c1 / #1313** (`2174c0d10bb146aea6a86f55262bc536feb86ea3`) — **mechanism not established** (provider settlement / execution settlement hunks do not cite `state graph: terminator: returned reference sources` or `execution_terminal_unit_state_graph.rs`).

**c2 / #2276** (`d30fccc7aae65a2358024cf39788d1007e2661b4`) — `validation/src/contract_entailment.rs`:

```diff
-                    program.external_invocation_roots()?;
-                    seen.last().copied()
-                })
+                // A machine outside any enclosing machine has no root here on
+                // purpose. Its body is proved from its own `requires`, however
+                // few callers it has, so knowing every call site must not make
+                // their arguments facts of the body.
+                root
```

**c3 / #348** (`5ab4e12334fba74babfc3ae8d9992572fba307e6`) — **mechanism not established** (large `call_cycles` proof-SCC refactor; no diff hunk references `completed call-slot relation` or `operand has no complete completed call-slot relation`).
