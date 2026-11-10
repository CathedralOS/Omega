slice_0048

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0365 | calls.rs:19367-19376 | CLOSED | Empty-suffix primitive/Unit/dyn arms add no leaves by design; non-empty suffix returns None (read 19373-19374). |
| A0366 | calls.rs:22354-22367 | CLOSED | Exclusive-ref parameter target makes `reference_value_replaces_binding` false so write-frame logic treats assignment as through-ref, not slot rebind (read 22354-22374). |
| A0367 | calls.rs:26047-26056 | UNDECIDED | Coarse leaf with invalid root is copied through only when `include_shared`; did not trace whether ill-formed leaves can reach `instantiate_result_origins` from typed input. |
| A0368 | calls.rs:26142-26187 | REACHABLE | Path roots match parameter names before locals while flow semantics shadow parameters with locals (flow.rs:2193), so same-spelled paths can bind to the parameter branch incorrectly. |
| A0369 | calls.rs:22409-22418 | REACHABLE | `expression_sources_before_statement` requires `incomplete_calls.is_empty()` but not `!template_dependent`, unlike `is_complete` (26396-26397), so template-dependent prefixes can reach pipeline consumers. |
| A0370 | calls.rs:28393-28399 | UNDECIDED | `may_select` with no matching leaf `continue`s; other `None` paths error at 28400-28407; did not prove whether `continue` hides a real missing leaf. |
| A0371 | calls.rs:29624-29630,29833,30387-30389 | UNDECIDED | Mixed passthrough rules (`output_identity?`, empty sources when `lifetime_access`, non-`Symbolic` in `capture_source`); did not trace all callers to program accept/reject boundary. |
| A0372 | calls.rs:31632-31635 | CLOSED | `static_type_context_is_unreachable` (type_system.rs:1682-1683) marks dead expressions; skipping effect walk is intentional, not missing validation on reachable code (read). |

### A0365

1. Read `reference_leaves` in full at calls.rs:18980-19381 (read).
2. Search `reference_leaves\(` under `omega-rust`: 21 hits (read grep; definitions + call sites in calls.rs, reference_result_custody.rs, flow_expression.rs, unit_validation.rs).
3. Read caller `written_paths` calls.rs:18705-18723; it returns `None` if `reference_leaves` returns `None`, otherwise projects leaf paths (read). Did not read all 21 call sites.
4. Input on accepting branch: pending queue item whose `reference` is a primitive, `Unit`, or `DynamicTrait` and `suffix` is empty (read 19367-19376).
5. **Verdict: CLOSED.** Non-empty suffix returns `None` (19373-19374). These types carry no caller-tracked storage leaves; empty aggregate is expected (area_13.md:189 dyn write-free, read). Ill-shaped field suffixes on structs take other arms that fail closed before this arm.

Unsure: none material for this arm.

### A0366

1. Read `reference_value_replaces_binding` in full at calls.rs:22335-22375 (read).
2. Search `reference_value_replaces_binding` under `omega-rust/psi`: 6 hits, all in calls.rs (read grep).
3. Read `replaces_binding` calls.rs:22214-22246 → `assignment_rebinds_parameter` 22201-22212; read assignment write predicate caller calls.rs:6782-6807 (`statement_assignment_may_write_place` region, read 6770-6811).
4. Input: assignment whose canonical target is a state parameter with non-shared reference shell (read 22354-22365).
5. **Verdict: CLOSED.** Returning `false` means the assignment is not classified as rebinding the parameter slot; upstream uses that for write-through vs rebind (6800-6807). `reference_shell` access distinguishes shared vs exclusive (22362-22364, read).

Unsure: did not read every callsite of `replaces_binding` beyond write-frame and binding-stability paths (6562-6604, read partially).

### A0367

1. Read `instantiate_result_origins` in full at calls.rs:25954-26103 (read).
2. Search `instantiate_result_origins\(` under `omega-rust`: 3 hits in calls.rs (25915, 25202, 23362) (read grep).
3. Read `instantiate_edge_origins` call at 25915-25938 (read); did not read full edge-walk acceptance logic.
4. Input: reference leaf with `include_shared == true`, invalid `origin.place().source.root`, `CollectionCoarse` precision (read 26047-26050).
5. **Verdict: UNDECIDED.** Branch copies the leaf without `instantiate_source` (26051-26055). I did not establish whether typed programs can supply invalid-root coarse leaves at call boundaries, or whether later merge steps error.

Unsure: provenance of invalid `root` on `CollectionCoarse` origins (grep shows many writers; not traced per path).

### A0368

1. Read `relative_state_path_is_visible` and `normalize_state_relative_path` in full at calls.rs:26142-26188 (read).
2. Search `relative_state_path_is_visible` under `omega-rust/psi/compilation/validation`: 8 hits in calls.rs (read grep).
3. Read consumer `summarize_*` path at 11491-11497 and `normalize_state_relative_path` at 16016-16020 (read). Read flow doc flow.rs:2188-2194 stating locals shadow parameters (read).
4. Input: `relative` path whose root string equals a parameter name; also equals a later local name (read 26149-26156 vs 26155-26157 ordering).
5. **Verdict: REACHABLE.** Smallest rejected case: a state where a local shadows a same-named parameter (allowed per flow.rs:2193) but a write-frame relative path uses that spelled root; validation classifies it as the parameter (`Some(true)` at 26153, or `$Pn` at 26175-26178) instead of the shadowing local (`Some(false)` at 26155-26156), mis-attributing visibility/normalization.

Unsure: whether name-path resolution elsewhere always uses symbols so string roots never hit shadowed locals (not fully traced).

### A0369

1. Read `expression_sources_before_statement` in full at calls.rs:22396-22433 (read); read `pending_prefix` block calls.rs:26793-26841 (read).
2. Search `expression_sources_before_statement` under `omega-rust`: 3 hits in calls.rs (read grep); consumers checks_contracts_calls.rs:1170, checks_current_scalar_values.rs:4265 (read 1170-1174 only).
3. Read `CollectedStatementPrefix::is_complete` calls.rs:26396-26398 (read) vs filter at 22418 (read).
4. Input: prefix with `template_dependent == true` and `incomplete_calls.is_empty()` (read 22409-22418, 26795-26797).
5. **Verdict: REACHABLE.** Smallest case: statement index where slot-relation transfer is template-dependent but call failures list is empty; `expression_sources_before_statement` still yields origins to `reference_expression_sources_before_statement` while `is_complete()` would be false.

Unsure: whether `value_origins` (22419-22428) always fails or sanitizes on template-dependent prefixes (read 22708-22742 only partially relative to this query).

### A0370

1. Read `collect_exit` in full at calls.rs:28356-28429 (read).
2. Search `collect_exit\(` under validation: 2 hits (definition + calls.rs:28234 caller) (read grep).
3. Read caller loop calls.rs:28233-28244: failure of `collect_exit` aborts with `None` (read).
4. Input: exit slot with `may_select == true` and `selected_slot_sources` returns `None` while no reference leaf matches `may_select` and segments (read 28386-28398).
5. **Verdict: UNDECIDED.** `continue` skips recording exit effects for that slot (28398). Whether that corresponds to “unchanged” soundly vs hiding a missing leaf depends on inventory construction (28210-28230, read but not proved end-to-end).

Unsure: alternate reasons `selected_slot_sources` returns `None` beyond the `may_select` branch (35347+, read header only).

### A0371

1. Read `instantiate_sources`, `observations`, and `Inputs::capture_source` at calls.rs:29615-29834, 30381-30389 (read); skimmed `capture_source` continuation 30429-30472 (read).
2. Search `instantiate_sources\b` under `omega-rust`: 2 hits in calls.rs (29615, 28597 caller) (read grep).
3. Did not read caller at 28597 in full.
4. Inputs: (a) `output_identity()` on origin (29624-29629), (b) empty `sources` with `supplied.lifetime_access` (29833), (c) non-`Symbolic` `origin` in `capture_source` (30387-30388).
5. **Verdict: UNDECIDED.** (a) uses `invocation_output_parameter?` before passthrough (29625, read). (b) and (c) accept partial data without the `has_captured_namespace` / builtin-coordinate checks used in `observations` (29813-29818, read); did not trace whether downstream refuses incomplete slot sources.

Unsure: behavior at calls.rs:28597 when `instantiate_sources` returns sparse vectors.

### A0372

1. Read `expression` walker at calls.rs:31622-31705 (read), including unreachable skips 31666-31679 (read).
2. Search `static_type_context_is_unreachable` under `omega-rust/psi/compilation/validation/src/calls.rs`: 8 hits (read grep); definition type_system.rs:1682-1683 (read).
3. Early return `Some(())` at 31632-31635 drops effect collection for unreachable expressions (read).
4. Input: expression or match arm/pattern marked unreachable by static type context (read).
5. **Verdict: CLOSED.** This is deliberate dead-code pruning in the slot-relation walker, not acceptance of a reachable program with missing checks; unreachable classification is owned by typed-trees (`static_type_contexts.is_unreachable`, read 1682-1683).

Unsure: misclassification of reachability in the type context engine (not investigated).

### What I did not read

- Full bodies of all 21 `reference_leaves` call sites.
- Callers of `instantiate_sources` beyond the grep hit at 28597.
- Complete `value_origins` / `declared_initializer_origins` under template-dependent prefixes.
- Pipeline rules forbidding or allowing parameter/local name collisions beyond flow.rs:2193.
- Invalid-root `CollectionCoarse` leaf construction paths end-to-end.

SLICE COMPLETE
