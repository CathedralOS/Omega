# Ledger bisect part 5

PIN: `51e8c45c793007f211d0d9baee7de16767172444`

Branch head before this correction: `cc5d98cb95bf0948d7c362f5c65f53d1498a9429` (`git rev-parse HEAD` and `git ls-remote origin work/ledger-bisect-part-5`).

## Slice

| prior rows | measured | fixed at PIN | still broken at PIN |
| ---: | ---: | ---: | ---: |
| 34 | 34 | 23 | **11** |

| cluster | rows | first commit that stops matching the PIN-good ledger | first commit showing the PIN diagnostic class |
| --- | ---: | --- | --- |
| B domain | 1 | `66ee9999011ed557510ddd18f20ceca6cb4aa5ae` (#1582) | same commit |
| A1 statement | 5 | `d26fa952d579d52642970560c719d24fa2a89f8a` (#1909), status `incomplete` | `7198f568763c6dae7d02668838a1616c631c70aa` (#1818) |
| A2 statement | 5 | `7198f568763c6dae7d02668838a1616c631c70aa` (#1818) | same commit |

`33cefbc6c241048ff6f6c583acc5aca9b0176ec5` (#2238) is not a first-bad. With a wiped `CARGO_TARGET_DIR=/tmp/bisect-target-<full sha>`, its first parent `cdb14cc0c3e5b5bf3ea887f1c9d35737cab677d2` already shows the PIN statement-successor class (witness pin exits **1 1 1**, all 10 rows `pin_class=true`). The two `corpus_runner` binaries differ (`sha256` `c14ca20f…` vs `47ff2e9f…`), so this is not one shared binary.

---

## Cluster B — domain (accepted)

Witness `pass/facts/domain_pattern_match_bound_through_field_exit`.

| | |
|---|---|
| First bad | `66ee9999011ed557510ddd18f20ceca6cb4aa5ae` (#1582) |
| Parent | `aea7dbe2ae380673377b4a49b91b173e01e3137c` |
| Witness exits | parent **0 0 0**, bad **1 1 1** |

---

## Cluster A1 — five rows, ledger break at #1909

Fresh targets (directory removed before the build):

| commit | target dir | witness pin exits | witness ledger exits | status |
| --- | --- | --- | --- | --- |
| `82d2247b96e872df79f26ec9b7af2fdec0153ce7` (parent) | `/tmp/bisect-target-82d2247b96e872df79f26ec9b7af2fdec0153ce7` | **0 0 0** | **0 0 0** | `checked` |
| `d26fa952d579d52642970560c719d24fa2a89f8a` (#1909) | `/tmp/bisect-target-d26fa952d579d52642970560c719d24fa2a89f8a` | **0 0 0** | **125 125 125** | `incomplete` |

Pin exit 0 means the observation is not the PIN class. Ledger exit 125 means it also does not match the fixed PIN-good expectation (`checked`). All five A1 rows are `incomplete` at `d26fa952d579d52642970560c719d24fa2a89f8a` and `checked` at the parent. Diagnostic at the break: `` `main` calls `witness`; `witness` has no executable body: this compiler does not lower a transition of it yet ``.

Rows: `pass/facts/selected_native_code_layout_through_{const_name,field,local,other_state,target_body}_exit`.

### Hunk that drops the old fusion cutoff (`d26fa952d579d52642970560c719d24fa2a89f8a`)

`omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution/terminal_unit/state_graph/fused.rs` (`@@ -60,106 +65,9`). Discovery no longer keeps only the reverse-reachable recursive component:

```diff
@@ -60,106 +65,9 @@ pub(in crate::execution::terminal_unit) fn fused_states<'program>(
             }
         }
     }
-    // Discovery establishes forward reachability. Reverse reachability back
-    // to the root keeps exactly its machine component. Include all states of
-    // each retained owner, including acyclic finishing states.
-    let mut owners = fused
-        .iter()
-        .map(|(owner, _)| owner.symbol)
-        .collect::<Vec<_>>();
-    owners.dedup();
-    if owners.len() <= 1 {
-        return fused;
-    }
-    // ... predecessor walk, retained[], moves_activation_claim ...
-    fused.retain(|(owner, _)| {
-        owners
-            .iter()
-            .position(|symbol| *symbol == owner.symbol)
-            .is_some_and(|owner_index| retained[owner_index])
-    });
     fused
 }
```

`successors.rs` (`@@ -378,10 +378,15`), same commit. A mutable home binds the current occupant instead of a `StructuralResult` source:

```diff
@@ -378,10 +378,15 @@ pub(in crate::execution::terminal_unit) fn successor_bindings(
                 if place.segments.is_empty()
                     && let Some(symbol) = place.root.as_symbol()
-                    && let Some(CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal }) =
-                        super::super::local_storage::source(program, source, operations, symbol)
+                    && let Some(binding_ordinal) =
+                        super::super::local_storage::occupant(program, source, operations, symbol)
```

---

## PIN class and cluster A2 — `#1818`, not `#2238`

`git bisect start --first-parent 51e8c45c793007f211d0d9baee7de16767172444 d26fa952d579d52642970560c719d24fa2a89f8a`, each step wiping `/tmp/bisect-target-<full sha>`. `d26fa952d579d52642970560c719d24fa2a89f8a` is a valid PIN-class good (pin exits **0 0 0**). Bisect stopped with only skipped commits left between:

- `5c85123a85d4e9d43bf771ff9db4f09a053995ee` docs: ratify invariant repair through existing contracts (#1964) — diff is `TASKS.md` and `wiki/**/*.md` only, so the step returned 125
- `7198f568763c6dae7d02668838a1616c631c70aa` compiler: keep saved references usable after pointer replacement (#1818) — tested pin=1

Fresh rebuild of the docs commit and of `#1818`:

| commit | other_state pin | other_state ledger | discarded_exit pin | discarded_exit ledger |
| --- | --- | --- | --- | --- |
| `5c85123a85d4e9d43bf771ff9db4f09a053995ee` (first parent of #1818) | **0 0 0** | 125 125 125 (`incomplete`, not PIN class) | **0 0 0** | **0 0 0** |
| `7198f568763c6dae7d02668838a1616c631c70aa` (#1818) | **1 1 1** | **1 1 1** | **1 1 1** | **1 1 1** |

All 10 statement rows are PIN class at `7198f568763c6dae7d02668838a1616c631c70aa`. At its parent, the five A1 rows are still `incomplete` (not PIN class) and the five A2 rows are still `checked`.

A2 therefore first stops matching the PIN-good ledger at `#1818`, not at `#1909`. Fresh `d26fa952d579d52642970560c719d24fa2a89f8a` still has those five `checked`: `pass/calls/native_code_layout_discarded_exit`, `pass/facts/selected_native_code_layout_through_match_join_exit`, `pass/facts/source_indexed_family_alpha_positive`, `pass/facts/source_indexed_family_recursive_index_positive`, `pass/proofs/erased_mathematical_initializers`.

### Hunk (`7198f568763c6dae7d02668838a1616c631c70aa`)

`omega-rust/psi/compilation/validation/src/machine_calls/calls/write_frames/demand.rs` (`@@ -427,63 +449,46`). Calls whose effects are empty and whose incomplete suppliers do not `requires_relation` used to return `Ok(Vec::new())`. That return is gone; the fallthrough now goes through `into_complete_effects`, which refuses when `incomplete_calls` is non-empty:

```diff
@@ -427,63 +449,46 @@ impl<'program> CallFrameResolver<'program> {
-        if transfer.effects.is_empty()
-            && transfer
-                .incomplete_calls
-                .iter()
-                .all(|failure| !failure.requires_relation)
-        {
-            return if transfer.template_dependent {
-                Err(TemplateDependent)
-            } else {
-                Ok(Vec::new())
-            };
-        }
-        if !transfer.incomplete_calls.is_empty() {
-            return Err(Unavailable(
-                "stored call has no complete statement-successor effects: an invocation relation is unavailable",
-            ));
-        }
+        transfers.fallthrough().into_complete_effects()
```

`slot_relations.rs` (`@@ -175,6 +175,24`) adds that refusal:

```diff
+    pub fn into_complete_effects(
+        self,
+    ) -> Result<Vec<StoredReferenceEffect>, StoredReferenceRelationRefusal> {
+        if !self.incomplete_calls.is_empty() {
+            return Err(StoredReferenceRelationRefusal::Unavailable(
+                "stored call has no complete statement-successor effects: an invocation relation is unavailable",
+            ));
+        }
```

---

## Method

- Fixed ledger: `git show 51e8c45c793007f211d0d9baee7de16767172444:tests/omega/corpus_outcomes.txt`
- One build at a time. Each commit: `rm -rf /tmp/bisect-target-<40-char sha>` then `CARGO_TARGET_DIR` set to that path.
- Pin exit 1 only when status and normalized diagnostic match the PIN observation. Ledger exit 0 only when the row still meets the PIN-good status. Anything else is 125, except a build or ledger-only/docs-only step, which bisect skips.
- Evidence: `.agents/ledger-bisect-2026-11-11/fresh/`
