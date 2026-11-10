slice_0017

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0125 | FCP:775-777; FCP:1394-1399; FCP:2068-2069,2088 | CLOSED | Each branch is documented flow policy; callers either drop overstated facts or treat non-matches as non-invalidating. |
| A0126 | FC:420-424 | CLOSED | Label equality is the intentional spelling fallback inside the one-linear-in/one-linear-out carry path. |
| A0127 | FX:1433-1436 | CLOSED | Ambiguous outgoing rows store invalid handles; contract/exit consumers require `operands.is_valid` before proof. |
| A0128 | FE:601-604,720 | CLOSED | `withhold_projected_pointer` deliberately drops referents for shell-typed projections without stored captures. |
| A0129 | FM:1491-1500,1512 | CLOSED | Failed pre-index local lookup makes `place_requires_local_write_origin` false so writes stay at the syntactic binding. |
| A0130 | FM:4612,4571 | REACHABLE | Alias closure `None` falls back to unexpanded `places`, so aliases may keep facts other paths would retire. |
| A0131 | FM:4593;FM:3879;FM:1283-1289 | CLOSED | Empty write vectors and skipping image-storage alias edges match stated non-write / no-alias semantics. |
| A0132 | FO:1257-1269 | CLOSED | Zero descriptor observation rows make `Iterator::all` vacuously true before stricter namespace rejoin elsewhere. |

### A0125 (FCP:775-777; FCP:1394-1399; FCP:2068-2069,2088)

1. Read `ArgumentSources::observation_survives` in full (`flow_call_phases.rs:721-811`); at `775-777` (read), `PlaceRoot::Expression(_)` returns `true` without calling `source_survives`.
2. Read `incoming_reference_survives_call` in full (`flow_call_phases.rs:1386-1428`); at `1394-1399` (read), `IncomingReferenceOrigin::ParameterReferent` always returns `true`.
3. Read `append_call_referent_field_domain_facts` in full (`flow_call_phases.rs:1682-2150`); at `2058-2069` and `2084-2088` (read), `matching_mutation_for_fact_place` returning `None` makes the `prepared_writes.all` closure return `true` (“mutation does not affect this fact”).
4. Search `\bobservation_survives\(` under `omega-rust/`: 5 files, ~11 call sites (read grep); production callers include `retain_guarantees` (`flow_call_phases.rs:851-852`), `append_call_referent_field_domain_facts` (`1759`), `append_call_parameter_domain_facts` (`flow_calls.rs:687`), plus validation `arithmetic_domains.rs` (different API).
5. Read `retain_guarantees` (`814-893`): overstated `observation_survives` can keep contract contexts; `contract_parameter_position` `Err(())` at `855` (read) drops the fact.
6. Search `incoming_reference_survives_call`: 2 hits, both `flow_call_phases.rs:1363` inside `install_returned_reference_custody` (read); caller `flow_expression.rs:9807` (read); parallel retain rule `flow.rs:6155-6159` (read).
7. Read `apply_call_invalidations` (`1235-1306`): `filter_contexts_after_mutations` with `None` from `matching_mutation_for_fact_place` leaves facts unchanged (`flow.rs:7214-7228`, read).

**Input to accepting branches:** literal or temporary argument projections whose resolved place roots are expressions; incoming references whose origin is `ParameterReferent`; callee write contributions where no mutation matches a field-domain fact place during guarantee restoration.

**Verdict: CLOSED** — Expression-root and `ParameterReferent` paths are stated custody/observation policy, not missing guards. Unmatched mutations not invalidating facts mirrors `context_survival` (`flow.rs:7214-7216`). Over-retained guarantees still pass through `retain_guarantees` stale filtering and downstream proof obligations.

**Unsure:** No end-to-end proof that every consumer of overstated `observation_survives` re-checks storage; only traced `retain_guarantees` and parameter-domain publication (`flow_calls.rs:685-688`).

### A0126 (FC:420-424)

1. Read `append_one_to_one_call_carry_facts` in full (`flow_calls.rs:317-445`).
2. At `420-424` (read): after `places_match` fails, matching `fact_label == source_label` still enqueues carry transfers.
3. Search `append_one_to_one_call_carry_facts(`: 1 production caller, `flow_calls.rs:122` (read).
4. Read caller gates: linear return type (`332-336`), exactly one linear parameter argument (`373-375`), `canonical_place_to_fact_place_in_state` for source place (`376-386`).

**Input:** one-to-one linear call where pre-call carry facts sit on a place that does not `places_match` the linear argument but shares `display_name` with that argument.

**Verdict: CLOSED** — Transfers run only in the narrow one-linear-in/one-linear-out path; label equality is the spelling fallback when place rebasing differs (area_09 digest, read). `checks_multiplicity` / linear obligation checks are separate gates.

**Unsure:** Did not enumerate every diagnostic that would reject a label collision with distinct storage.

### A0127 (FX:1433-1436)

1. Read `append_state_exit_facts` in full (`flow_exits.rs:1398-1548`).
2. At `1433-1436` (read): if filtered `outgoing` is not exactly one row, `returned_value` and `completed_storage` become `Handle::invalid()`.
3. Search `append_state_exit_facts(`: callers `flow.rs:4853`, `flow_exits.rs:1632` (read).
4. Read consumers: `checks_contracts_exits.rs:4052-4054`, `8136-8138` (read) return `None` when `exit.returned_value` is invalid; `3863-3871` (read) falls back to operand expression only if valid.

**Input:** state exit with zero or multiple matching `FlowStateOutgoingFact` rows for the same transition coordinates.

**Verdict: CLOSED** — Invalid handles are not treated as proof carriers; consumers fail closed by skipping or refusing judgments requiring a captured return operand.

**Unsure:** Did not read every exit check arm for paths that treat invalid as success.

### A0128 (FE:601-604,720)

1. Read `call_slot_values` and `withhold_projected_pointer` in full (`flow_expression.rs:568-723`).
2. At `601-604` (read): reference shell, `reference_captures_at_path` absent, still returns `withhold_projected_pointer(actual, path)`.
3. At `720` (read): non-empty projection clears `referents`, sets `value_source`.
4. Search `withhold_projected_pointer`: only `flow_expression.rs` (read); `call_slot_values` callers `440`, `450`, `3445` (read).

**Input:** call operand typed as reference shell, projection path present, no snapshot captures at that path.

**Verdict: CLOSED** — Comment at `639-644` (read): field window without own descriptor; operand kept with referents dropped by design.

**Unsure:** Did not trace every consumer of operands missing referents for false-positive acceptance.

### A0129 (FM:1491-1500,1512)

1. Read `place_requires_local_write_origin` and `rebase_exact_local_place` in full (`flow_mutation.rs:1484-1557`).
2. At `1497-1501` (read): `state_local_declaration` miss returns `false`.
3. At `1519-1526` (read): `!place_requires_local_write_origin` ⇒ `Some(place)` unchanged.
4. Read `assignment_storage_places` use at `1397-1404` (read): same false ⇒ `StorageRegion::Declared(direct)` without referent rebase.
5. Search `place_requires_local_write_origin`: 5 hits, all `flow_mutation.rs` (read).

**Input:** write place whose root fails `state_local_declaration(..., ordinal < statement_index)` (e.g. binding not yet live at this index).

**Verdict: CLOSED** — “Failed lookup” is the pre-declaration / non-reference-local path; recording at the syntactic place is intentional, not silent acceptance of an invalid program.

**Unsure:** Did not prove `place_at` never supplies illegal roots for reference locals that need referent rebasing.

### A0130 (FM:4612,4571)

1. Read `statement_storage_writes` (`flow_mutation.rs:4573-4613`) and doc at `4569-4571` (read).
2. At `4604-4612` (read): `close_storage_places_over_aliases(...).or(Some(places))` keeps direct assignment places when closure returns `None`.
3. Read `close_storage_places_over_aliases` (`1247-1266`, read): returns `None` if `resolver` is `None` (`1263`).
4. Search `statement_storage_writes`: used from `flow.rs:5345`, `8610`, checks modules (read grep counts).

**Input:** assignment with resolved `places`, alias closure fails (`None`), flow still records only direct `places` in `completed_writes` (`flow.rs:5368-5376`, read).

**Verdict: REACHABLE** — Doc calls for full invalidation on unresolved origin (`4571`, read), but `or(Some(places))` under-invalidates alias sites, so facts tied to aliases may survive a write a complete closure would retire; I did not trace a separate check that catches all such cases.

**Unsure:** Frequency of `close_storage_places_over_aliases` returning `None` with a populated `call_frames` resolver in production builds.

### A0131 (FM:4593; FM:3879,3897; FM:1283-1289)

1. Read `statement_storage_writes` (`4593-4595`, read): non-`Assignment` ⇒ `Some(Vec::new())`.
2. Read `call_mutation_writes` (`3879-3900`, read): `invocation_is_fact_only` and byte-predicate calls ⇒ empty writes; `4047-4050` (read) empty write-frame builtins ⇒ empty writes.
3. Read `close_storage_places_over_origins` (`1283-1289`, read): `is_image_storage` origins skip alias expansion.

**Input:** non-assignment statements, fact-only/byte-predicate/empty-frame calls, image-storage write origins in alias closure.

**Verdict: CLOSED** — Each site records “no storage mutation” or “no extra alias,” consistent with comments and operand-write paths for real effects (`flow.rs:5358-5367`, read).

**Unsure:** Did not read every alternate mutation enumerator for the same statement kinds.

### A0132 (FO:1257-1269)

1. Read `descriptor_context_agrees` in full (`flow_observations.rs:1257-1269`).
2. At `1263-1268` (read): `for_context(context).all(...)` is vacuously true when the roster is empty.
3. Search `descriptor_context_agrees`: `flow_operand_contexts.rs:365`, `checks_contracts_prover.rs:1003` (read).
4. Read `operand_contexts` (`349-378`, read): stable-copy operands bypass descriptor filtering (`361-362`); otherwise contexts must agree; invocation intersection follows (`368-378`).

**Input:** semantic context handle with no `descriptor_context_observations` rows for that context.

**Verdict: CLOSED** — Vacuous agreement means no descriptor rejoin obligation; non-stable operands still need namespace agreement via `descriptor_observation_namespace` when required (read `1277-1302`, not fully traced to prover errors).

**Unsure:** Whether any prover treats vacuous `descriptor_context_agrees` as sufficient proof without also requiring `descriptor_observation_namespace`.

### What I did not read

- Full `checks_contracts_exits` exit-proof driver beyond sampled `returned_value` guards.
- Full `filter_contexts_after_mutations` / `context_survival` dependency closure past `7228`.
- Validation `CallFrameResolver` failures beyond `close_storage_places_over_aliases:1263`.
- Entire `append_call_referent_field_domain_facts` guarantee `restores` implementation.
- Corpus or runtime behavior; read-only, no build.

SLICE COMPLETE
