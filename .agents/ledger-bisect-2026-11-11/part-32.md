# Ledger bisect part 32 (k=32, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`.

## Slice summary (prior report clusters 32, 72, 112, 152, 192)

| Metric | Count |
| --- | ---: |
| Slice members (unique) | 11 |
| Fixed on PIN | 7 |
| Still broken on PIN | 4 |
| Still-broken clusters (re-grouped by PIN first diagnostic) | 2 |

Re-measure: direct `corpus_runner`, `OMEGA_CORPUS_JOBS=1`, commit-scoped `CARGO_TARGET_DIR=target/bisect-<full-sha>` per `bisect_witness.sh`.

## Clusters on PIN

| rows | PIN diagnostic class | witness | bisect first-bad (compiler) | PR | first commit with PIN diagnostic class | PR |
| ---: | --- | --- | --- | ---: | --- | ---: |
| 2 | `machine swap_pending` stores … normal-exit backing relation | `pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap` | `4faf6413467dd7a7f6cf7e62ba77d1c607591f18` | 1718 | `741270df60f7eab218e879eb6771d2bc75e64c00` | 2343 |
| 2 | `repair` has no executable body (structural operand custody) | `pass/facts/recoverable_replacement_window_through_other_machine_exit` | `f6b76a817aa80d8e4e986e2ff2fc9bfbbe4e3c12` | 220 | `741270df60f7eab218e879eb6771d2bc75e64c00` | 2343 |

Good anchors (walk / introduction): **A** `3bbd06849b998f4fef4ccdffb8ef1f6059c07def` (#1213); **B** `20fd485ca4623a46e9554846aadaeb7e47937493` (adds `tests/omega/pass/facts/recoverable_replacement_window_through_other_machine_exit/build.omg` in #220’s era commit titled `psi: restore borrowed values through retained alias results`).

### Cluster A — verification (`4faf64134` / #1718)

| Item | SHA | Value |
| --- | --- | --- |
| First-bad (bisect) | `4faf6413467dd7a7f6cf7e62ba77d1c607591f18` | #1718 |
| First parent of first-bad | `f2c75870ebaf890328b787fa6d8ed50e868edbb8` | — |
| Good anchor | `3bbd06849b998f4fef4ccdffb8ef1f6059c07def` | #1213 |
| PIN-class first-bad | `741270df60f7eab218e879eb6771d2bc75e64c00` | #2343 |
| PIN-class parent | `b5216ad46815a2509ac1fa06a1c1b55c4633412d` | #2352 |

**Witness 3× at first parent / first-bad (PIN test harness):** does **not** satisfy 0⁰/1¹ with PIN diagnostic. At parent and first-bad, all six runs were **exit=1**, **status=rejected**, first diagnostic class **overlap borrow** (`statement 12 mutates second_buffer.value while local borrow second is still active`), not PIN’s `normal-exit backing relation`.

**Witness 3× at PIN diagnostic class** (`741270df60f7`): **1, 1, 1** with first diagnostic `machine swap_pending stores a borrowed value without a complete normal-exit backing relation` (matches PIN).

**Members (parent vs `4faf64134`):**

| fixture | parent `f2c75870` | first-bad `4faf64134` |
| --- | --- | --- |
| `pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap` | bad (overlap borrow) | bad (overlap borrow) |
| `pass/facts/foreign_retained_shared_borrow_persistent_restore` | exit 125 / checked at probe | exit 125 / checked at probe |

**Mechanism (#1718)** — `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks/current_scalar_values.rs` re-anchors scalar values per write during replay (`anchor_at`, per-write retirement), changing what survives to later borrow/backing checks:

```diff
@@ -766,14 +766,22 @@ impl<'program> CurrentScalarValues<'program> {
-            match self.anchors.iter_mut().find(|(anchored, _)| *anchored == place) {
-                Some((_, anchor)) => *anchor = snapshot,
-                None => self.anchors.push((place, snapshot)),
-            }
+            self.anchor_at(place, snapshot);
+        }
+    }
+
+    fn anchor_at(&mut self, place: validation::CanonicalPlace, snapshot: ...) {
+        match self.anchors.iter_mut().find(|(anchored, _)| *anchored == place) {
+            Some((_, anchor)) => *anchor = snapshot,
+            None => self.anchors.push((place, snapshot)),
         }
     }
```

**PIN diagnostic class (#2343):** mechanism **not established** from a single attributed hunk (first-bad for class is `741270df60f7` / #2343; diagnostic is produced only from that commit onward in verification).

### Cluster B — verification (`f6b76a817aa` / #220)

| Item | SHA | Value |
| --- | --- | --- |
| First-bad (bisect) | `f6b76a817aa80d8e4e986e2ff2fc9bfbbe4e3c12` | #220 |
| First parent | `55c524c09bdeb382f714d48c7c04ab781ff72a8b` | — |
| Good anchor | `20fd485ca4623a46e9554846aadaeb7e47937493` | fixture introduction |
| PIN-class first-bad | `741270df60f7eab218e879eb6771d2bc75e64c00` | #2343 |

**Good anchor `20fd485ca46`:** witness **0, 0, 0** (`status=checked`); commit adds pass+fail recoverable `other_machine` fixtures (not “never-good”).

**Witness 3× at parent `55c524c09`:** **0, 0, 0** (`checked`).

**Witness 3× at first-bad `f6b76a817aa`:** **1, 1, 1**, but **not** PIN class — `terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "repair", reason: "selected whole-owner cleanup has a partially transferred occurrence" })`.

**Witness 3× at PIN class `741270df60f7`:** **1, 1, 1** with PIN incomplete text (`repair` has no executable body … structural operand custody).

**Members (parent vs `f6b76a817aa`):**

| fixture | parent | first-bad |
| --- | --- | --- |
| `pass/facts/recoverable_replacement_window_through_other_machine_exit` | good (checked) | bad (InvalidUnitMachinePlan) |
| `pass/facts/recoverable_replacement_window_through_call_result_exit` | exit 125 / checked | bad (InvalidUnitMachinePlan) |

**Mechanism (#220)** — structural argument / path lowering changes in checked→lowered preparation:

```diff
--- a/omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/expression_preparation/bindings/mod.rs
+++ b/omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/expression_preparation/bindings/mod.rs
@@ -541,6 +537,9 @@ impl ScalarBindings {
                         root_type_identity: checked.normalized_type_identity(alias.loan.root_type),
+                        root_reference_hops: 0,
                     }
@@ -765,6 +764,9 @@ impl ScalarBindings {
                 .map(|segment| match segment {
+                    checked_trees::CheckedUnitStructuralPathSegment::Referent => {
+                        Ok(checked_trees::CheckedStructuralPredicatePathSegment::Referent)
+                    }
```

(Repository path at #220; repair `InvalidUnitMachinePlan` is the observed failure mode at first-bad before PIN’s incomplete diagnostic.)

**PIN diagnostic class (#2343):** mechanism **not established** from an isolated hunk tied to the incomplete `repair` message.

## Fixed on PIN (7)

- `fail/borrows/dynamic_store_local_backing_escape`
- `fail/borrows/indexed_carrier_state_local_escape`
- `fail/facts/foreign_retained_shared_borrow_borrowed_slot_swap`
- `fail/facts/foreign_retained_shared_borrow_persistent_restore`
- `fail/termination/lexicographic_component_zero_step_rejected`
- `pass/facts/target_family_contract_operand/const_name` — fixture dir: `tests/omega/pass/facts/target_family_contract_operand/const_name/`
- `pass/recast/runtime_captured_guard_conjunction_exit`

## Commands

Host: linux/x86_64, 4 CPUs, `OMEGA_CORPUS_JOBS=1`, `CARGO_TARGET_DIR=$PWD/target/bisect-$(git rev-parse HEAD)`.

Logs: `/tmp/verify_a.log`, `/tmp/verify_b.log`, `/tmp/verify_a_pinclass.log`, `/tmp/verify_b_pinclass.log`, `/tmp/members_a.log`, `/tmp/members_b.log`, `/tmp/pin_class_bisect_a.log`.
