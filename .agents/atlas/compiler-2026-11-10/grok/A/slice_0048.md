# slice_0048

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0365 | omega-rust/psi/compilation/validation/src/calls.rs:19367-19376 | UNDECIDED | Primitive, Unit, or `DynamicTrait` at empty suffix ends the pending walk with no reference leaves; whether a call site still passes acceptance without those leaves not traced. |
| A0366 | omega-rust/psi/compilation/validation/src/calls.rs:22354-22367 | UNDECIDED | `reference_value_replaces_binding` returns false for non-`Shared` parameter places; whether that lets an unsound program compile, versus intentional write-through semantics, not traced to accept/reject. |
| A0367 | omega-rust/psi/compilation/validation/src/calls.rs:26047-26056 | UNDECIDED | With `include_shared`, invalid-root `CollectionCoarse` leaves are copied through unchanged; no end-to-end witness from live call instantiation to acceptance read. |
| A0368 | omega-rust/psi/compilation/validation/src/calls.rs:26161-26186 | UNDECIDED | `normalize_state_relative_path` resolves a root by parameter name before scanning `LocalData`; language shadowing rules not read. |
| A0369 | omega-rust/psi/compilation/validation/src/calls.rs:22409-22418 | UNDECIDED | `expression_sources_before_statement` filters `incomplete_calls` but not `template_dependent`; consumers not traced for wrong sources on template-dependent prefixes. |
| A0370 | omega-rust/psi/compilation/validation/src/calls.rs:28393-28399 | UNDECIDED | `collect_exit` skips a `may_select` slot when sources are `None` and no matching leaf exists; whether exit inventory can omit the leaf for another reason not read. |
| A0371 | omega-rust/psi/compilation/validation/src/calls.rs:29624-29630,29833,30387-30389 | UNDECIDED | Pass-through and empty-`sources` paths in `instantiate_sources` / `observations` / `capture_source`; traced one refusal at `instantiate_sources` caller only (`28599`), not through to program acceptance. |
| A0372 | omega-rust/psi/compilation/validation/src/calls.rs:31632-31635,31666,31679 | UNDECIDED | Effect walk returns early on unreachable expressions/patterns; ordinal alignment with `CallSiteTraversal` (`43567`, `43623`, per digest) not traced. |

## A0365

**Cited site (read):** `reference_leaves` in `omega-rust/psi/compilation/validation/src/calls.rs:18980-19381`. Lines `19367-19376` match the atlas cite at this checkout (not re-checked with `git show 457adf4a11f:`).

Inside the `while pending.pop()` loop, after `TypeReferenceNode::Reference` handling at `19160-19208`, the type match arm at `19367-19376` accepts `primitive_type_reference(reference)`, `TypeReferenceNode::Unit`, or `TypeReferenceNode::DynamicTrait` when `suffix` is empty (`19373-19375` returns `None` only if suffix non-empty). The arm records no `references`, `moves`, or `cases` entries; the loop may exit with `Some(leaves)` still default-empty for that branch (`19379-19380`).

**Callers (read):** `rg '\breference_leaves\(' omega-rust` → 16 matches: 10 call/definition sites in `calls.rs` (including `18980` def), 5 in `reference_result_custody.rs` (different `reference_leaves` method on another type), 1 in `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:1`. Production aggregate walk uses include `written_paths` `18714`, `instantiate_moves` `23470`, `completed_result` `29858`, others in `calls.rs`.

**Upstream (read):** Reference-shaped `expected_type` is handled earlier in the same loop (`19160-19208`) via `inputs.reference` and may push `ReferenceLeaf` rows with or without `origin`. The `19367` arm applies when the dequeued `reference` is primitive, unit, or dynamic-trait, not when the outer call type is `TypeReferenceNode::Reference` (that takes the `19160` arm).

**Input (inferred):** An aggregate decomposition whose final pending item is typed as a scalar primitive, `()`, or `dyn Trait` at the root suffix, so the walk completes without reference leaves for that subtree.

**Verdict:** UNDECIDED — whether any accepting path (write frames, slot exit, borrow roster) treats missing leaves here as “no references to track” when the language still requires tracking for the same call argument.

## A0366

**Cited site (read):** `reference_subjects::reference_value_replaces_binding` `calls.rs:22335-22375`. `22354-22367`: `canonical_place_from_expression_in_state` on `target`; if the place is a state parameter whose `reference_shell` is not `Shared` (`22361-22364`), the function returns `false` (`22367`), so the assignment is not classified as rebinding the binding.

**Callers (read):** `rg '\breference_value_replaces_binding\b' omega-rust` → 6 matches, all in `calls.rs`: definition `22335`; uses `6594-6601` (statement scan), `6800`, `22238` (`replaces_binding`), `22272` (`replaces_reference_binding`), `22297` (`assignment_stored_value_type`).

**Downstream (read):** When `reference_value_replaces_binding` is false, `assignment_stored_value_type` `22297-22308` uses `reference_shell(...).referee` as the stored type. `replaces_binding` / `replaces_reference_binding` feed `state_write_walk` (`26732`, per unit_091 digest).

**Input (read):** Assignment whose target canonicalizes to a parameter root with exclusive or mutable reference shell, and a value that is not an atomic-once target (`22343-22352`).

**Verdict:** UNDECIDED — traced only that parameter write-through is the implemented branch; did not read whether a program that should be rejected still reaches `CheckedTrees` acceptance because of this classification.

## A0367

**Cited site (read):** `instantiate_result_origins` `calls.rs:25954-26103`. Per reference leaf, after static/output/result-home handling (`26033-26045`), `26047-26056`: if `include_shared && !origin.place().source.root.is_valid() && origin.place().precision == FramePathPrecision::CollectionCoarse`, push `ReferenceLeaf` unchanged and `continue`, skipping parameter binding (`26057-26059`).

**Callers (read):** `rg '\binstantiate_result_origins\b' omega-rust/psi/compilation/validation/src/calls.rs` → 4 matches: def `25954`; calls `23362`, `25202`, `25915` (`instantiate_edge_origins`).

**Parameter (read):** `include_shared` is threaded from callers (e.g. edge instantiation `25915`); grep `include_shared: true` in `state_write_walk` / related paths in this file (`26367`, `26605-26607`).

**Input (inferred):** A callee result-reference leaf marked `CollectionCoarse` with an invalid parameter root, propagated with `include_shared` true across a call or transition edge.

**Verdict:** UNDECIDED — branch is reachable in code when the flags and leaf shape hold; no program traced from typed input through this push to a documented accept-without-diagnostic outcome.

## A0368

**Cited site (read):** `state_paths::normalize_state_relative_path` `calls.rs:26161-26188`. `26170-26178`: root string matched against `state_parameters` by `parameter.name.as_str() == root` first; `26180-26187`: only if no parameter match, scan `StatementNode::LocalData` for `local.name.as_str() == root` and return `Some(None)` for locals.

**Callers (read):** `rg '\bnormalize_state_relative_path\b' omega-rust` → 4 matches: def `26161`; production use `16016` in `infer_state_write_frame` (opaque on `None` at `16019-16020`); module import `39097`.

**Related (read):** `relative_state_path_is_visible` `26142-26158` also treats parameter name match before local name (`26149-26156`).

**Input (inferred):** A written path string whose root spelling equals both a parameter name and a later `LocalData` name in the same state.

**Verdict:** UNDECIDED — mis-resolution would require permitted name shadowing; shadowing rules in spec/guides not read, so neither REACHABLE nor CLOSED.

## A0369

**Cited site (read):** `expression_sources_before_statement` `calls.rs:22396-22433`. `22409-22418`: `collected_prefix_with_readiness_at(..., index, true)` then `.filter(|prefix| prefix.incomplete_calls.is_empty())`; no test on `prefix.template_dependent`. `CollectedStatementPrefix::is_complete` is `!template_dependent && incomplete_calls.is_empty()` (`26396-26397`, read).

**Callers (read):** `rg '\bexpression_sources_before_statement\b' omega-rust` → 3 matches in `calls.rs` (`22396` def, `22385`, `14901` via `reference_expression_sources_before_statement`). External: `checks_contracts_calls.rs:1170`, `checks_current_scalar_values.rs:4265` (read call sites only, not full consumer logic).

**Related entry (read):** `statement_reference_transfer` uses `collected_prefix_with_readiness_at` at `14763-14770` without `is_complete()` filter (unit_091 notes same family).

**Input (inferred):** Prefix snapshot at statement index with `template_dependent == true` and `incomplete_calls` empty, feeding `value_origins` at `22419-22428`.

**Verdict:** UNDECIDED — template-dependent prefixes can reach this function (read); whether resulting sources let `reference_construction_preserves_source` or scalar current-value checks accept a bad program not traced.

## A0370

**Cited site (read):** `collect_exit` `calls.rs:28356-28409+`. `28386-28399`: `selected_slot_sources` may return `None`; if `slot.may_select` and no record leaf matches `may_select` and `local_segments` (`28394-28396`), `continue` (slot skipped). Otherwise `None` falls through to trace and `return None` (`28400-28407`).

**Callers (read):** `rg '\bcollect_exit\b' omega-rust/psi/compilation/validation/src/calls.rs` → 2 matches: def `28356`; call `28234` in slot-exit solve loop (`.or_else` refusal trace `28234-28243`).

**Input (read):** Exit inventory slot with `may_select` true, no selected sources, and no matching `may_select` leaf in the stored record for that slot’s segments.

**Verdict:** UNDECIDED — digest [091] marks plausible “unchanged slot” behavior but leaves open whether the record could be incomplete for an unrelated refusal; that alternate path not read.

## A0371

**Cited sites (read):**

- `instantiate_sources` `29615-29729`: `29624-29630` if `supplied.origin.source.output_identity()`, after `invocation_output_parameter(program, identity)?`, return one `InstantiatedSource` cloning `supplied` with empty substitutions.
- `observations` (same module): `29833` returns `(!sources.is_empty() || supplied.lifetime_access.is_some()).then_some(sources)` — empty `sources` allowed when `lifetime_access` present.
- `capture_source` `30381-30389`: non-`ReferenceSource::Symbolic` origins returned `Some(vec![origin])` unchanged (`30387-30388`).

**Callers (read):** `rg '\binstantiate_sources\b' omega-rust/psi/compilation/validation/src/calls.rs` → 2 matches: def `29615`; call `28597-28608` in slot-effect instantiation — `None` from `instantiate_sources` aborts with `trace_instantiation_refusal` and `return None` (`28599-28608`).

**Downstream (inferred, not traced):** `capture_source` and `observations` are used inside `result_origins` / slot capture paths in the same `stored_calls` region; whether `Some(empty)` or pass-through ever reaches a path that still marks a call accepted without diagnostics was not followed past `28597`.

**Verdict:** UNDECIDED — fail-closed at `28599` read for missing `instantiate_sources`; other cited pass-through/empty branches not connected to final acceptance.

## A0372

**Cited site (read):** `stored_calls::expression_effects::Evaluation::expression` `calls.rs:31622-31699+`. `31632-31635`: invalid expression or `static_type_context_is_unreachable(expression)` → immediate `Some(())`. `31666-31667`, `31679-31680`: unreachable match patterns / arm values `continue` without visiting. `31757-31767`: `And`/`Or` short-circuit when `static_boolean_value(left)` is deciding.

**Related digest (read):** area_13 / unit_092 note parallel skips in call-ordinal walk `calls.rs:43567`, `43623` and effect walk here; `output_identity` may re-derive ordinals (`33450`, per digest — not opened).

**Callers (inferred):** `expression` is the recursive effect walker inside `stored_calls` slot relation transfer (same module); not grep-counted separately from internal recursion.

**Input (read):** Expression or match arm that the typed tree marks unreachable via `static_type_context_is_unreachable`.

**Verdict:** UNDECIDED — dropping effects for unreachable nodes is intentional when marking is correct (read); whether a reachable call site could receive wrong ordinals because the effect walk and `CallSiteTraversal` diverge was not traced (digest [092] lists this as open/inferred).

## What I did not read

- `git show 457adf4a11f:omega-rust/psi/compilation/validation/src/calls.rs` line-by-line diff for cited ranges (current checkout lines matched cites on spot check).
- Language guide / spec chapters on parameter–local shadowing (A0368) or exclusive-reference parameter assignment semantics (A0366).
- Full bodies of `checks_contracts_calls.rs::reference_construction_preserves_source` and `checks_current_scalar_values.rs` scalar path beyond call sites (A0369).
- `CallSiteTraversal` and consumers that pair ordinals with effect walks (A0372).
- Corpus or sample programs witnessing REACHABLE acceptance for any row.

SLICE COMPLETE

