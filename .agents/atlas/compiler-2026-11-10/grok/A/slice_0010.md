# slice_0010

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0073 | checks_multiplicity_linear_validation.rs:489-494 | REACHABLE | Recorded ownership events with no matching `LinearPlace` are skipped with `continue`, and several downstream paths discard or loosen checks. |
| A0074 | checks_multiplicity_permission_events.rs:1961 | REACHABLE | Case-transition exhaustiveness treats subjects as equal when `display_name` strings match, not when handles or storage differ. |
| A0075 | checks_operators.rs:1856 | REACHABLE | Resolved Index/Range operator uses whose expression is not `Indexed` skip `requires` proving, and float destination checking returns early on many shapes. |
| A0076 | checks_ranges.rs:6074-6079 | REACHABLE | Unresolved collection operand types are treated as builtin geometry, so custom-operator `requires` can be skipped without ranges discharge. |
| A0077 | checks_ranges_facts.rs:4830-4833 | REACHABLE | Integer/length roster lookup falls back to name-only match after the symbol pass. |
| A0078 | checks_ranges.rs:11163-11230 | REACHABLE | The indexed gate does not detect the same indexed nodes the checker walks (e.g. recast-indexed), so whole states can skip indexed checking. |
| A0079 | checks_ranges_facts.rs:3717 | REACHABLE | `invalidate_assignment_bounds` does not retire `integer_fields` on aliased assignment paths the way call invalidation does. |
| A0080 | checks_termination.rs:6729-6731 | REACHABLE | Empty state lists yield `Proven(None)`, and proof machines bypass ranking judgment in the cited recursive admission path. |

## A0073

**Digest:** area_08, GROUP Scalar values and ownership — recorded events with no tracked place skipped (`:489-494`, `:574-579`, `:679-684`); `:3654` discards commit; `:4472-4525` mapped on identity alone.

**Cited sites (read, current checkout):** Lines match digest at commit `457adf4a11f` basenames under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`.

**Enclosing functions (read):**

- `apply_recorded_state_entry_events` (`checks_multiplicity_linear_validation.rs:479-501`): for each StateEntry Establish event, finds `place` by `root` and path slice; `else { continue; }` at 492-493.
- `apply_recorded_statement_events` (`:503-619`): same `find`/`continue` at 574-578 before applying Transfer/Consume/Establish/Restore.
- `apply_recorded_exit_dispositions` (`:653-686`): at 679-684, consumption runs only when `find` succeeds; no diagnostic on miss.
- `complete_body_custody` (`:3620-3670`): `let _ = content_transformations::commit(...)` at 3654-3660.
- `append_unresolved_state_result_mapping_diagnostics` (`:4438-4527`): mapping success via `mapped_from_call` (4472-4493), `mapped_by_checked_outcome` (4495-4510), `mapped_by_join` (4511-4524).

**Callers (search `rg '\bapply_recorded_state_entry_events\b' /agent/repos/omega`, count mode — read):** 1 definition, 2 import lines, 2 production calls (`linear_validation.rs:4238`, `permission_events.rs:1611`).

**Search `rg '\bapply_recorded_statement_events\b' /agent/repos/omega` (read):** 1 definition, 2 import lines, 2 production calls (`linear_validation.rs:4288`, `permission_events.rs:1654`).

**Search `rg '\bcomplete_body_custody\b' omega-rust` (read):** 1 definition, 2 call sites (`linear_validation.rs:3353`, `:3544`).

**Upstream (read):** `validate_linear_permission_events` (`:4193-...`) is invoked from `check_linear_obligations` (`checks_multiplicity.rs:2517`), which is invoked from `check_checked_facts` (`checks.rs:12109`). `content_transformations::commit` (`checks_multiplicity.rs:930-1067`) returns `Result<(), &'static str>` and can `Err` from `prepare` (e.g. `:57`, `:100`, `:145` — read).

**Input that reaches the branch:** A checked-body machine state whose recorded flow permission events include Establish/Transfer on `(root, segments)` that the linear replay `places` vector never tracked (mismatched root/path or missing `reserve_owner_places`), so replay applies no consumption/establishment update. Separately, a body completion where `commit` returns `Err` while `complete_body_custody` continues (`:3654`). Separately, a call-result establish event whose callee/caller transfer shares `claim_identity` but not the correct result path, so `append_unresolved_state_result_mapping_diagnostics` treats it as mapped (`:4525-4526`).

**Verdict rationale:** Smallest harmful outcome in plain words: a program that should fail linear validation because a recorded move or body-conservation commit failed can still pass stage-05 checks when events do not match any tracked place (silent `continue`), when `commit` errors are dropped, or when mapping is closed by claim identity alone without path agreement.

## A0074

**Digest:** area_08 — `:1961`, `:1972`: two different places with the same spelling would pass as one subject.

**Enclosing function (read):** `case_transition_run_is_exhaustive` (`checks_multiplicity_permission_events.rs:1938-1999`).

**Callers (search `rg '\bcase_transition_run_is_exhaustive\b' /agent/repos/omega` — read):** 1 definition, 1 production use (`checks_multiplicity.rs:5486`) inside permission scheduling when transitions are not exhausted.

**Trace (read):** For each transition arm, subject label is `program.expression_table.display_name(subject)` (`:1972`, `:1961`). If `subject_label` is already set, mismatch is detected only when `*expected != label` (`:1963-1966`, `:1973-1977`). There is no comparison of expression handles, canonical places, or symbols beyond the string label.

**Input that reaches the branch:** A state with multiple `When` transition guards that test case membership on two distinct expression sites that share the same display name (e.g. different locals or fields rendered identically).

**Verdict rationale:** Two different subjects with the same spelling are treated as one exhaustiveness subject, so a non-exhaustive case transition run can be accepted and `PermissionStep::MissedGuard` skipped (`checks_multiplicity.rs:5485-5488` — read).

## A0075

**Digest:** area_08 — `:1856` Index/Range not `Indexed` skipped; `:190-192`, `:251-255`, `:332` float-format mismatch not reported.

**Enclosing functions (read):**

- `selected_binary_requires_diagnostics` (`checks_operators.rs:1825-1899+`): at 1849-1858, Index/Range spelling with non-`Indexed` expression uses `_ => true` so the loop `continue`s before `check_requires_facts`.
- `check_destination` (`:181-279`): invalid value/destination returns at 190-192; non F32/F64 primitive destination returns at 251-255; `result_primitive` binary non-arithmetic arm returns `None` at 332, preventing the mismatch diagnostic at 274-277.

**Callers (search `rg '\bselected_binary_requires_diagnostics\b' omega-rust` — read):** 1 definition, 1 call (`checks_operators.rs:2852` in `check_operator_resolution`).

**Search `rg '\bcheck_destination\b' checks_operators.rs` (read):** internal to `destinations::check` and nested recursion within same module (`:36`, `:121`, `:156`, `:221`, `:239`).

**Upstream (read):** `check_operator_resolution` (`:2835`) from `checks.rs:12162`.

**Input that reaches the branch:** (1) A resolved operator use spelled as Index or Range on a non-`Indexed` expression (e.g. a named call carrying slice spelling) skips operator `requires` proof. (2) An operator expression delivering F32 into F64 (or vice versa) where `result_primitive` cannot resolve a float type (unsupported expression shape falls through to `_ => None` at 332) while the destination is a float primitive.

**Verdict rationale:** Programs that should be rejected for unproven operator `requires` or implicit float narrowing can be accepted when those early exits apply.

## A0076

**Digest:** area_08 — custom-operator index with unresolved operand type; `requires` nobody discharges.

**Enclosing functions (read):**

- `has_builtin_collection_geometry` (`checks_ranges.rs:6069-6092`): at 6074-6078, if `collection` is not in the type table, returns `true`; loop exhaustion at 6091 also returns `true`.
- `ranges_seam_owns` (`:6057-6067`): returns true when first indexed operand type is missing or has builtin geometry (via `is_none_or` + `has_builtin_collection_geometry`).
- Context in selected-operator discharge (`:6020-6026`): non-builtin geometry returns `Ok(None)` for alternate path; combined with operators check skip when seam owns.

**Callers (search `rg '\branges_seam_owns\b' omega-rust` — read):** re-exported/imported in same file and used from `checks_operators.rs:1854` and further ranges code (6 references in `checks_ranges.rs` per grep).

**Upstream (read):** `selected_binary_requires_diagnostics` skips Index/Range `requires` when `ranges_seam_owns` is true (`checks_operators.rs:1853-1855`).

**Input that reaches the branch:** An `Indexed` use whose collection operand type cannot be resolved in the type reference table (invalid/missing chain), so `has_builtin_collection_geometry` returns true at 6078, `ranges_seam_owns` is true, and operator `requires` for the custom index operator is not diagnosed in `selected_binary_requires_diagnostics`.

**Verdict rationale:** A program with an index operator whose `requires` clauses are never discharged can compile when the operand type is unresolved and treated as builtin geometry.

## A0077

**Digest:** area_08 — shadowed or same-named binding with different symbol could return another binding's value or length.

**Enclosing function (read):** `roster_lookup` (`checks_ranges_facts.rs:4816-4834`), used by `local_length`, `local_integer`, `field_integer` (`:4849-4870` — read).

**Callers:** `roster_lookup` is private; callers are methods on `RangeFacts` in the same module (read at `:4849-4870`, `:4938`).

**Upstream (read):** `check_indexed_accesses` builds per-state `RangeFacts` and drives proof (`checks_ranges.rs:10964+`); integer facts are consumed during range checking (inferred from `field_integer` / `local_integer` accessors — read definitions only).

**Trace (read):** Symbol match is tried first in reverse (`:4821-4827`). If that fails, name fallback at 4830-4833 matches any roster row with the same `local_name` string without requiring symbol equality when `name` is provided.

**Input that reaches the branch:** A lookup by `(symbol, name)` where the symbol does not match the intended roster entry but an older shadowed row with the same name remains in the vector.

**Verdict rationale:** Range proofs can read another binding's recorded integer or length when only the name matches.

## A0078

**Digest:** area_08 — gate walker and checker may not enumerate the same nodes.

**Enclosing functions (read):**

- Gate: `state_contains_indexed_expression` (`:11163-11176`), `statement_contains_indexed_expression` (`:11178-11230`), `expression_contains_indexed` (`:11232-11249`).
- Checker: `check_statement` → `transfer_statement_facts` (`:7893-8001`, `:7259+`); `check_expression` → `ExpressionTransfer::transfer` (`:7042-7063`, `:5130+`); `expression_has_effect` (`:5032-5051`).

**Callers (search `rg '\bstate_contains_indexed_expression\b' /agent/repos/omega` — read):** 1 definition, 1 use (`checks_ranges.rs:10996` in `check_indexed_accesses`).

**Search `rg '\bcheck_indexed_accesses\b' omega-rust` (read):** definition at `:10964`, caller `checks.rs:12168`.

**Trace (read):**

- Gate `expression_contains_indexed` detects only `ExpressionNode::Indexed` or `Call` with valid `layout_query.nonempty_view` (`:11237-11244`).
- Checker `ExpressionTransfer` visits `ExpressionNode::Cast` with `cast.form.is_recast()` and an `Indexed` value by transferring collection and index (`:5301-5307` — read).
- Gate does not treat recast-wrapped indexed nodes as indexed (`expression_contains_indexed` has no Cast arm).
- `check_indexed_accesses` skips entire states when `has_indexed` is false (`:11084-11086` — read).

**Input that reaches the branch:** A state whose only indexed access appears under a recast indexed carrier (or otherwise only on checker transfer paths not mirrored in the gate predicate), so `state_contains_indexed_expression` is false and the state is not checked, while a full compile path could still hit indexed semantics elsewhere.

**Verdict rationale:** Index bounds checking can be omitted for states the gate does not flag, even though the statement transfer would have visited the indexed nodes if checking had run.

## A0079

**Digest:** area_08 — field integers on assignment through aliased path not traced.

**Enclosing function (read):** `invalidate_assignment_bounds` (`checks_ranges_facts.rs:3717-3825`); contrast `invalidate_call_writes` (`:3854+`), which clears `integer_fields` at 3950 (read).

**Callers (search `rg '\binvalidate_assignment_bounds\b' omega-rust` — read):** 1 definition, 1 call (`checks_ranges.rs:7329` from `transfer_statement_facts` on assignments).

**Trace (read):** Assignment transfer calls `invalidate_assignment_bounds` before `assign_field_integer` (`checks_ranges.rs:7329`, `:7358-7360`). `invalidate_assignment_bounds` clears `boolean_locals`, invalidates relational bounds via `overlaps`/`covers` derived from `statement_storage_writes` and display-name fallbacks (`:3749-3815`), but never calls `integer_fields.clear()` or `forget_field_integer` (read full function 3717-3825). Call-write invalidation explicitly clears `integer_fields` at 3950.

**Input that reaches the branch:** An assignment to a field path that aliases storage under a name-based write frame where `covers`/`overlaps` do not retire an existing `integer_fields` row, then a new field integer is recorded.

**Verdict rationale:** Stale field integer facts can survive an assignment invalidation when the write path is aliased by label rather than by the field symbol the roster uses.

## A0080

**Digest:** area_08 — machine with no states is `Proven(None)`; `:7903-7918` proof machines get no ranking judgment here.

**Enclosing functions (read):**

- `machine_decrease_outcome_with` (`checks_termination.rs:6722-6731`): `states.first()` none → `DecreaseOutcome::Proven(None)`.
- `infer_checked_recursive_machine_terminations` (`:7883-7918`): when `proof_only.is_proof_machine`, builds members via `infer_checked_proof_recursive_machines` and returns `Ok(members)` without calling `machine_decrease_outcome`.

**Callers (search `rg '\bmachine_decrease_outcome\b' omega-rust` — read):** 3 references in `checks_termination.rs` (`:6714` definition, `:297`, `:8081`, `:8348`).

**Search `rg '\binfer_checked_recursive_machine_terminations\b' omega-rust` (read):** definition only in termination module (no other grep hits in omega-rust besides export).

**Upstream (read):** `infer_machine_checked_summary` (`:8335-8361`) treats cyclic machines with implementation witness as terminating when `machine_decrease_outcome` is `Proven(_)`, including `Proven(None)` (`:8347-8350`).

**Input that reaches the branch:** (1) A machine with an empty `machine_states` list reaches ranking and is proven without a decrease witness. (2) A proof-classified machine body admitted recursively via `:7903-7918` without the ranking outcome path used for ordinary checked bodies.

**Verdict rationale:** Termination can be established without decrease evidence for empty machines, and proof machines can be admitted in the recursive path without the ranking judgment performed for ordinary machines.

## What I did not read

- `git show 457adf4a11f:` paths (cited lines matched on current checkout without relocation).
- Full bodies of `validate_linear_permission_events` after the permission-schedule loop, full `content_transformations::prepare`, and full `infer_checked_proof_recursive_machines`.
- Corpus/samples proving each REACHABLE scenario is realizable on current `main`.
- Stage-04 validation that might reject empty machines before stage 05.
- Other `omega-rust` crates outside the cited call chains.

SLICE COMPLETE

