# Part 22 first-bad verification

PIN: `51e8c45c793007f211d0d9baee7de16767172444`

Strict test: exit **0** = meets fixed PIN ledger expectation; exit **1** = PIN broken status (`rejected` for these witnesses) under `VERIFY_STRICT_DIAG=1`. Each build uses `target/verify-<sha>/` (one commit-scoped build at a time).

## Cluster c1

- Witness: `pass/borrows/returned_slice_record_reference_custody_exit`
- Good anchor (full): `576c5c9433a95c1f4dcb4f18197f408afbfe92cd` — psi: let checking workers find type identities without taking a lock (#1603)
- First bad (full): `f2c75870ebaf890328b787fa6d8ed50e868edbb8`
- First parent (full): `18fc751d006008859b8c23f100456b58c89a02db` — psi: stop preparing a program twice when evaluating its layouts (#1693)
- Merge subject: psi: keep helper-returned views usable across calls that may pause (#1672)
- PR from merge subject: **1672** (confirmed in merge commit message)

### Witness stability (strict, 3×)

- **parent** `18fc751d006008859b8c23f100456b58c89a02db`: **0,0,0**
- **first-bad** `f2c75870ebaf890328b787fa6d8ed50e868edbb8`: **1,1,1**

### Diagnostic at first-bad vs PIN

- status=rejected
- diag=returned projection source permission is unavailable: the saved descriptor has no complete checked borrowing source
- diag_norm=returned projection source permission is unavailable: the saved descriptor has no complete checked borrowing source

First-bad shows the **same diagnostic class** as PIN observation (not a different failure family).

### Good anchor check (strict, 3×)

- **good-anchor** `576c5c9433a95c1f4dcb4f18197f408afbfe92cd`: **0,0,0**

### Member flip table (strict exit codes)

| fixture | parent | first-bad |
| --- | ---: | ---: |
| `pass/borrows/returned_slice_record_reference_custody_exit` | 0 | 1 |
| `pass/borrows/runtime_returned_stored_view_reborrow_exit` | 1 | 1 |
| `pass/slices/runtime_mutable_slice_result_whole_forward_exit` | 0 | 0 |
| `pass/wire/wire_preserving_decode_relay_exit` | 0 | 1 |

**Non-flippers (split):**

- `runtime_returned_stored_view_reborrow_exit`: **1|1** at c1 first parent / first-bad (no flip on witness boundary). Same strict-**1** at c1 good anchor `576c5c9433a95c1f4dcb4f18197f408afbfe92cd` — member PIN-class breakage **predates** the witness good anchor; not introduced at `f2c75870ebaf890328b787fa6d8ed50e868edbb8`.
- `runtime_mutable_slice_result_whole_forward_exit`: **0|0** at c1 first parent / first-bad. Member first-bad on first-parent chain after witness first-bad: `77271bfb91344cd3d3788e1d8563796983dfbf13` (parent `2476d1be2203009f1b32b8164bb6224c562c3bd5`, PR **#1794**).

### Diff hunk (first-bad)

`omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks/borrows/resources/formation_sources/returned_projections.rs` (new file):

```diff
@@ -0,0 +1,322 @@
+//! A helper's field view must keep the reference that actually supplied it.
+pub(super) fn validate(
+    program: &dyn validation::CheckingInput,
+    facts: &CheckFacts,
+    ...
+) -> Result<(), Vec<Diagnostic>> {
+    let capture = formation_capture(facts, activation.formation_value, row.formation_expression)
+        .ok_or_else(|| {
+            vec![Diagnostic::error(
+                "returned view lost its completed invocation capture",
+            )]
+        })?;
```

## Cluster c2

- Witness: `pass/build/explicit_provider_storage_extent_exit`
- Good anchor (full): `bed3b818b169b1ff86fe30b5ec27fda873ffed5f` — psi: migrate 22 console-provider fixtures to build-owned bindings (#1050)
- First bad (full): `55f4a84e523c0f1b64f7d72d80f4cb83e84e7017`
- First parent (full): `e888ffb81d847e6cbc5472f74164131b4f3d2af6` — loc: twelve-crate qualified-path collapse (batch X, −2919) (#1078)
- Merge subject: psi: preserve indexed observations through normal call completion (#1074)
- PR from merge subject: **1074** (confirmed)

### Witness stability (strict, 3×)

- **parent** `e888ffb81d847e6cbc5472f74164131b4f3d2af6`: **0,0,0**
- **first-bad** `55f4a84e523c0f1b64f7d72d80f4cb83e84e7017`: **1,1,1**

### Diagnostic at first-bad vs PIN

- status=rejected
- diag=anonymous address value cannot prove fit in `addr::Bound` in machine `SelectedImage::retained_suffix_start` state `retained_suffix_start` return value
- diag_norm=anonymous address value cannot prove fit in `_` in machine `_` state `_` return value

### Good anchor check (strict, 3×)

- **good-anchor** `bed3b818b169b1ff86fe30b5ec27fda873ffed5f`: **0,0,0**

### Member flip table

| fixture | parent | first-bad |
| --- | ---: | ---: |
| `pass/build/explicit_provider_storage_extent_exit` | 0 | 1 |

### Diff hunk (first-bad)

`omega-rust/psi/compilation/checking/src/machine_calls/calls/write_frames/demand/normal_subjects.rs`:

```diff
@@ -26,16 +26,25 @@ pub(crate) struct InvocationReferenceSubject {
 impl<'program> CallFrameResolver<'program> {
-    pub(crate) fn expression_call_reference_subjects(
+    pub(crate) fn call_reference_subjects(
         &self,
         machine: &'program Machine,
-        expression: ExpressionHandle,
+        site: crate::CallSite<'program>,
         parameters: &[StateParameter],
     ) -> Option<Vec<InvocationReferenceSubject>> {
-        let writes = self
-            .operation_write_frame(machine, expression)
-            .into_complete_paths()?;
-        self.expression_entry_reference_subjects(machine, expression, parameters)?
+        let (writes, entries) = match site {
+            crate::CallSite::Expression { expression, .. } => (
+                self.operation_write_frame(machine, expression),
+                self.expression_entry_reference_subjects(machine, expression, parameters)?,
+            ),
+            crate::CallSite::Statement(call) => (
+                self.may_write_frame(machine, call),
+                self.statement_entry_reference_subjects(machine, call, parameters)?,
+            ),
+            _ => return None,
+        };
+        let writes = writes.into_complete_paths()?;
+        entries
```

## Cluster c3

- Witness: `pass/termination/runtime_named_state_remainder_endpoint_exit`
- Good anchor (full): `6f4ad2d1e228899c7cd4a54b80a6f591764c0794` — psi: restore calls with callbacks and fixed array lengths (#1824)
- First bad (full): `0a85200733ef1f32849c682936c53c22fdff3226`
- First parent (full): `6f4ad2d1e228899c7cd4a54b80a6f591764c0794` — psi: restore calls with callbacks and fixed array lengths (#1824) (same as good anchor)
- Merge subject: psi: check reach rule on termination promises and spell helper rankings (#1825)
- PR from merge subject: **1825** (confirmed)

### Witness stability (strict, 3×)

- **parent** `6f4ad2d1e228899c7cd4a54b80a6f591764c0794`: **0,0,0**
- **first-bad** `0a85200733ef1f32849c682936c53c22fdff3226`: **1,1,1**

### Diagnostic at first-bad vs PIN

- status=rejected
- diag=terminating machine walk reaches recursive component `iterate`, which has no `terminates by` ranking -- spell one on that component
- diag_norm=terminating machine walk reaches recursive component `_`, which has no `_` ranking -- spell one on that component

### Good anchor check (strict, 3×)

- **good-anchor** `6f4ad2d1e228899c7cd4a54b80a6f591764c0794`: **0,0,0**

### Member flip table

| fixture | parent | first-bad |
| --- | ---: | ---: |
| `pass/termination/runtime_named_state_remainder_endpoint_exit` | 0 | 1 |

### Diff hunk (first-bad)

`omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks/termination.rs`:

```diff
@@ -343,6 +343,35 @@ fn admit_machine_rankings_with_topology(
         }
+        // A promise covers everything the machine runs. A recurrent component
+        // it reaches with no ranking of its own has unknown progress, so the
+        // promise cannot be checked; nesting does not lend it the caller's
+        // ranking (termination: one joint ranking per component, and unknown
+        // progress dependencies prevent a checked guarantee).
+        if claimed
+            && diagnostic.is_empty()
+            && let Some(component) = topology
+                .as_ref()
+                .and_then(|topology| topology.unranked_component_reached_from(program, position))
+        {
+            local_terminates = false;
+            entry_rank_formed = false;
+            ...
+            diagnostic = format!(
+                "terminating machine {} reaches recursive component {names}, which has no \
+                 `terminates by` ranking -- spell one on that component",
+                machine_name(program, machine.symbol)
+            );
+        }
```
