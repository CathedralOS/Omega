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

All 10 members are **good** at parent (`checked`). At first-bad, **5** flip to `incomplete` (not PIN statement-successor text yet); **5** stay ledger-good — own subcluster:

| fixture | parent `82d2247b96e872df79f26ec9b7af2fdec0153ce7` | bad `d26fa952d579d52642970560c719d24fa2a89f8a` |
| --- | --- | --- |
| `pass/calls/native_code_layout_discarded_exit` | good | good |
| `pass/facts/selected_native_code_layout_through_const_name_exit` | good | bad (incomplete) |
| `pass/facts/selected_native_code_layout_through_field_exit` | good | bad (incomplete) |
| `pass/facts/selected_native_code_layout_through_local_exit` | good | bad (incomplete) |
| `pass/facts/selected_native_code_layout_through_match_join_exit` | good | good |
| `pass/facts/selected_native_code_layout_through_other_state_exit` | good | bad (incomplete) |
| `pass/facts/selected_native_code_layout_through_target_body_exit` | good | bad (incomplete) |
| `pass/facts/source_indexed_family_alpha_positive` | good | good |
| `pass/facts/source_indexed_family_recursive_index_positive` | good | good |
| `pass/proofs/erased_mathematical_initializers` | good | good |

**Mechanism (quoted hunks from `d26fa952d579d52642970560c719d24fa2a89f8a`)**

`fused.rs` — drop reverse-reachability retention of the recursive component only; keep every discovered terminal target:

```diff
@@ -60,106 +65,9 @@ pub(in crate::execution::terminal_unit) fn fused_states<'program>(
     }
-    // Discovery establishes forward reachability. Reverse reachability back
-    // to the root keeps exactly its machine component. Include all states of
-    // each retained owner, including acyclic finishing states.
-    let mut owners = fused
-        .iter()
-        .map(|(owner, _)| owner.symbol)
-        .collect::<Vec<_>>();
-    owners.dedup(); // Discovery appends each owner's complete state roster once.
-    if owners.len() <= 1 {
-        return fused;
-    }
-    let mut predecessors = Vec::new();
-    ...
-    fused.retain(|(owner, _)| retained[owners.iter().position(...).unwrap()]);
     fused
```

`successors.rs` — mutable-home successors use `local_storage::occupant` instead of `StructuralResult` from `source`:

```diff
@@ -378,10 +378,15 @@ pub(in crate::execution::terminal_unit) fn successor_bindings(
                 if place.segments.is_empty()
                     && let Some(symbol) = place.root.as_symbol()
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

**Mechanism (quoted hunk from `66ee9999011ed557510ddd18f20ceca6cb4aa5ae`)** — `result_facts.rs` stops copying validation expression intervals onto completed values as flow facts:

```diff
@@ -182,36 +182,6 @@ impl Execution<'_, '_, '_> {
                 })
             });
-        // Checking already judged this completed expression under every clean
-        // incoming environment. Publish its resulting enclosure on the value,
-        // so constructors, returns and later transports read that record rather
-        // than rediscovering which source construct produced it. Missing i64
-        // endpoints mean the actual carrier's extreme, never a fabricated cap.
-        if let Some(interval) =
-            validation::expression_interval(self.context.expression_intervals, expression)
-            && let Some(primitive) = self.program.primitive_type_reference(reference)
-            && let Some(mut bounds) = validation::carrier_integer_range(primitive)
-        {
-            let carrier_bounds = bounds.clone();
-            let has_call_bounds = integer_bounds.is_some();
-            if let Some(minimum) = interval.low {
-                bounds.minimum = bounds
-                    .minimum
-                    .max(numerics::bignum::BigInt::from_i64(minimum));
-            }
-            if let Some(maximum) = interval.high {
-                bounds.maximum = bounds
-                    .maximum
-                    .min(numerics::bignum::BigInt::from_i64(maximum));
-            }
-            if let Some(previous) = integer_bounds.take() {
-                bounds.minimum = bounds.minimum.max(previous.minimum);
-                bounds.maximum = bounds.maximum.min(previous.maximum);
-            }
-            if bounds.minimum <= bounds.maximum && (has_call_bounds || bounds != carrier_bounds) {
-                integer_bounds = Some(bounds);
-            }
-        }
         let zero_fields = if matches!(
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
