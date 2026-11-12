# Ledger bisect part 5

PIN: `51e8c45c793007f211d0d9baee7de16767172444`

## Prior slice (cluster indices 5, 45, 85, 125, 165, 205 on report #2359)

| prior rows | measured members | fixed at PIN | still broken at PIN |
| ---: | ---: | ---: | ---: |
| 34 (recovered; index-5 row (+22) via report batch corpus) | 34 | 23 | 11 |

At PIN these rows re-cluster into **2** still-broken diagnostics (11 rows); **23** prior-slice rows are fixed on main.

## First-bad verification

### Cluster A — statement-successor (10 rows at PIN)

| | |
|---|---|
| **First bad** | `d26fa952d579d52642970560c719d24fa2a89f8a` (PR #1909) |
| **First-parent good** | `82d2247b96e872df79f26ec9b7af2fdec0153ce7` |
| **Bisect good used** | `5c6c16d4d1867bcac99343285c5074169d7cbfbc` |
| **Witness** | `pass/facts/selected_native_code_layout_through_other_state_exit` |

| commit | witness run1 | run2 | run3 |
| --- | ---: | ---: | ---: |
| `d26fa952d579d52642970560c719d24fa2a89f8a` | 1 | 1 | 1 |
| `82d2247b96e872df79f26ec9b7af2fdec0153ce7` | 0 | 0 | 0 |

All 10 members are **good** at parent and **not all bad** at first-bad. Five members stay ledger-good at `d26fa952d579d52642970560c719d24fa2a89f8a` (subcluster; see `.agents/ledger-bisect-2026-11-11/verify_statement_cluster.txt`). Witness triplet is **deterministic** (three runs, isolated `target/verify-<sha>` trees, fixed PIN ledger).

**Mechanism (quoted hunks)** — terminal fusion widens `fused_states` and changes successor binding to use `local_storage::occupant` instead of `StructuralResult` source plan:

```diff
--- a/omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution/terminal_unit/state_graph/fused.rs
+++ b/omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution/terminal_unit/state_graph/fused.rs
@@ -38,11 +43,11 @@ pub(in crate::execution::terminal_unit) fn fused_states<'program>(
-    // Discover the recursive component, not every transitively reachable callee.
+/// Fuse every statically resolved terminal call into the caller's activation.
 pub(in crate::execution::terminal_unit) fn fused_states<'program>(
@@ -58,110 +63,13 @@ pub(in crate::execution::terminal_unit) fn fused_states<'program>(
-    // Discovery establishes forward reachability. Reverse reachability back
-    // to the root keeps exactly its machine component.
-    let mut owners = fused
-        .iter()
-        .map(|(owner, _)| owner.symbol)
-        .collect::<Vec<_>>();
-    ...
-    fused.retain(|(owner, _)| retained[owners.iter().position(...).unwrap()]);
```

```diff
--- a/omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution/terminal_unit/state_graph/successors.rs
+++ b/omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution/terminal_unit/state_graph/successors.rs
@@ -373,20 +373,25 @@ pub(in crate::execution::terminal_unit) fn successor_bindings(
-                    && let Some(CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal }) =
-                        super::super::local_storage::source(program, source, operations, symbol)
+                    && let Some(binding_ordinal) =
+                        super::super::local_storage::occupant(program, source, operations, symbol)
```

### Cluster B — domain pattern (1 row at PIN)

| | |
|---|---|
| **First bad** | `66ee9999011ed557510ddd18f20ceca6cb4aa5ae` (PR #1582) |
| **First-parent good** | `aea7dbe2ae380673377b4a49b91b173e01e3137c` (exit **0**, not skipped) |
| **Narrow good** | `12a1feb4aa100ab41afd8cc59f53969c2ffc4009` |
| **Narrow bad bracket** | `647e274b14274e6d0e8a386df2ab66e7dd3e4f8d` |
| **Witness** | `pass/facts/domain_pattern_match_bound_through_field_exit` |

| commit | witness run1 | run2 | run3 |
| --- | ---: | ---: | ---: |
| `66ee9999011ed557510ddd18f20ceca6cb4aa5ae` | 1 | 1 | 1 |
| `aea7dbe2ae380673377b4a49b91b173e01e3137c` | 0 | 0 | 0 |

Sole member: good at parent (`checked`), bad at first-bad (`rejected`, domain construction diagnostic).

**Narrowed range (full-range skip):** `git bisect run` from PIN `51e8c45c793007f211d0d9baee7de16767172444` to distant good `5c6c16d4d1867bcac99343285c5074169d7cbfbc` aborted with `git bisect skip` exit code -1 after many steps returned **125** — ledger-only commits (only `tests/omega/corpus_outcomes.txt` / `*.md`) and unbuildable ancient hosts (not PIN-classification skips). Manual probes: `12a1feb4aa100ab41afd8cc59f53969c2ffc4009` → 0, `647e274b14274e6d0e8a386df2ab66e7dd3e4f8d` → 1. Narrow bisect between those SHAs → `66ee9999011ed557510ddd18f20ceca6cb4aa5ae`.

**Mechanism (quoted hunks)** — flow stops publishing validation expression intervals onto completed values:

```diff
--- a/omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow/expression/result_facts.rs
+++ b/omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow/expression/result_facts.rs
@@ -180,40 +180,10 @@ impl Execution<'_, '_, '_> {
-        if let Some(interval) =
-            validation::expression_interval(self.context.expression_intervals, expression)
-            && let Some(primitive) = self.program.primitive_type_reference(reference)
-            ...
-            if bounds.minimum <= bounds.maximum && (has_call_bounds || bounds != carrier_bounds) {
-                integer_bounds = Some(bounds);
-            }
-        }
```

## Still-broken clusters at PIN (summary)

| rows | witness | breaking commit | PR | status |
| ---: | --- | --- | --- | --- |
| 10 | `pass/facts/selected_native_code_layout_through_other_state_exit` | `d26fa952d579d52642970560c719d24fa2a89f8a` | 1909 | bisected (partial member flip at first-bad; see verify file) |
| 1 | `pass/facts/domain_pattern_match_bound_through_field_exit` | `66ee9999011ed557510ddd18f20ceca6cb4aa5ae` | 1582 | bisected |

## Fixed on main at PIN (23 rows)

Listed in `.agents/ledger-bisect-2026-11-11/part-5.tsv` (`fixed_at_pin`).

## Commands

- Host: Linux x86_64, `OMEGA_CORPUS_JOBS=1`, direct `corpus_runner`, fixed PIN ledger via `git show 51e8c45c793007f211d0d9baee7de16767172444:tests/omega/corpus_outcomes.txt`
- Verification artifacts: `verify_statement_cluster.txt`, `verify_domain_cluster.txt`
