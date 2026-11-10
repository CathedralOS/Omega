# slice_0017

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0125 | flow_call_phases.rs:775-777,1394-1399,2068-2069,2088 | REACHABLE | Three call-phase branches treat temporaries, parameter referents, and unmatched mutations as surviving without tracing a separate closing proof. |
| A0126 | flow_calls.rs:420-424 | REACHABLE | Carry transfer skips mismatched places when rendered labels are equal (read). |
| A0127 | flow_exits.rs:1433-1436 | UNDECIDED | Ambiguous outgoing rows store invalid handles; major exit consumers guard `returned_value`, but whether ordinary programs produce zero or duplicate outgoing rows was not established. |
| A0128 | flow_expression.rs:601-604,720 | REACHABLE | Reference projection without stored capture still returns an operand after `withhold_projected_pointer` clears referents (read). |
| A0129 | flow_mutation.rs:1491-1500,1512 | REACHABLE | Failed `state_local_declaration` makes `place_requires_local_write_origin` false, so assignment uses `StorageRegion::Declared` on the direct place (read). |
| A0130 | flow_mutation.rs:4612 | REACHABLE | Failed alias closure falls back to unclosed `places` via `.or(Some(places))` despite full-invalidation comment at FM:4571 (read). |
| A0131 | flow_mutation.rs:4593,3879,3897,4047-4051,1283-1289 | REACHABLE | Non-assignments, fact-only/byte-predicate/empty-frame calls, and image-storage origins contribute no storage writes or aliases here (read). |
| A0132 | flow_observations.rs:1257-1269 | REACHABLE | `descriptor_context_agrees` vacuously passes when a context has zero descriptor observation rows (read). |

## A0125

Digest (area_09, read): FCP sites — temporary-rooted sources survive writes; `ParameterReferent` incoming references always survive a call; unmatched mutation does not affect the fact.

**FCP:775-777** (`read`, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_call_phases.rs`): Inside `ArgumentSources::observation_survives` (enclosing function read lines 721-796). After `place_at` builds `source`, if `source.root` is `PlaceRoot::Expression(_)`, the method returns `true` without calling `source_survives` (775-777).

**Callers:** repository search `observation_survives` under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees` — 8 matches total (read); production uses of `ArgumentSources::observation_survives` at `flow_call_phases.rs:852` (`retain_guarantees`), `flow_call_phases.rs:1759` (`append_call_referent_field_domain_facts`), `flow_calls.rs:687` (`append_call_parameter_domain_facts`).

**FCP:1394-1399** (`read`, same file): `incoming_reference_survives_call` (1386-1414) returns `true` immediately when `incoming.origin` is `IncomingReferenceOrigin::ParameterReferent { .. }`.

**Callers:** search `incoming_reference_survives_call` in `omega-rust` — 2 matches (definition + `install_returned_reference_custody` at 1363). `install_returned_reference_custody` is called from `flow_expression.rs:9807` (read).

**FCP:2068-2069,2088** (`read`, same file): Inside `append_call_referent_field_domain_facts` (1682+), closure over call writes at 2049+: `matching_mutation_for_fact_place` returning `None` yields `return true` (2067-2068); inner rematch `None` also `return true` (2087-2088).

**Callers:** search `append_call_referent_field_domain_facts` in `omega-rust` — 5 matches (re-export + `flow_calls.rs:156` + module wiring). Invocation is from `build_call_flow_fact` (`flow_calls.rs:39-171`, read).

**Reachable input (plain words):** A call whose ensured contract occurrence is rooted in a temporary expression, so a later store overlapping that temporary still leaves the ensure marked surviving (`observation_survives`). Separately, an incoming reference marked `ParameterReferent` keeps its borrow constraint through binding writes regardless of invalidation. Separately, a domain membership fact on storage where call-write observation does not match any modeled mutation is treated as unaffected by that write.

## A0126

Digest (area_09, read): carry fact transfers when rendered label equals the source label even if places differ.

**FC:420-424** (`read`, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_calls.rs`): In `append_one_to_one_call_carry_facts` (317-445), loop skips a context fact unless `semantic.places_match(program, fact_place, source_place)` **or** `fact_label == source_label` (419-424). Mismatched places with equal labels continue to `transfers` and are appended at the call ensures point (434-444).

**Callers:** search `append_one_to_one_call_carry_facts` — 2 matches (definition + `build_call_flow_fact` at 122).

**Reachable input (plain words):** A one-to-one linear call copies carry permission/origin from an entry context fact whose place differs from the linear argument’s place but whose `place_label` string equals the argument’s `display_name`, so carry facts attach to the call result place without place equality.

## A0127

Digest (area_09, read): exit with zero or several outgoing rows recorded with invalid `returned_value` and `completed_storage`.

**FX:1433-1436** (`read`, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_exits.rs`): In `append_state_exit_facts` (1398-1548), outgoing rows matching the contract exit are collected (1415-1432); `match outgoing.as_slice()` uses invalid handles for any count other than exactly one (1433-1435). Those handles are stored on `FlowExitFact` (1535-1536).

**Callers:** search `append_state_exit_facts(` — 3 matches: `flow.rs:4853`, `flow_exits.rs:1632`, plus definition.

**Downstream (read, partial):** `checks_contracts.rs:8244-8247` returns false when `!operands.is_valid(exit.returned_value)`; `checks_contracts_exits.rs:4052-4053` early-returns when invalid; `flow.rs:11508-11509` `selected_implementations` returns `None` for invalid operand handles; `flow.rs:1156` gates on `completed_storage.is_valid`.

**Undecided:** Whether a checked-body program produced by normal flow building can have a proof `contract_exit` with zero or multiple matching `outgoing` rows for the same `(statement_index, transition_target)` without already violating an earlier invariant was not traced. If such multiplicity is only internal inconsistency, user-visible “acceptance” may be CLOSED at consumers above; if multiplicity is reachable from ordinary states, invalid handles could leave obligations unproved without rejection.

## A0128

Digest (area_09, read): projected pointer with no stored capture kept with referents dropped.

**FE:601-604,720** (`read`, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_expression.rs`): In `call_slot_values` (568-637), reference-shell path with no `reference_captures_at_path` still returns `Some(vec![(self.withhold_projected_pointer(actual, path), ...)])` (601-604). `withhold_projected_pointer` (711-722) sets `value.referents = Default::default()` and inserts a new operand.

**Callers:** search `call_slot_values` in `omega-rust` — 4 matches, all `flow_expression.rs` (440, 450, 602, 3445).

**Reachable input (plain words):** A call operand that projects to a reference type along `path` but has no stored capture at that snapshot still yields a projected operand handle with empty referents, so later checks see a reference-typed value without referent roster.

## A0129

Digest (area_09, read): failed declaration lookup makes a place count as declared storage.

**FM:1491-1500,1512** (`read`, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_mutation.rs`): `place_requires_local_write_origin` returns `false` when `state_local_declaration` filter fails (1497-1500). In `assignment_storage_regions` (1357+), when `place_requires_local_write_origin` is false, early return `StorageRegion::Declared(place)` (1397-1404). `rebase_exact_local_place` (1512+) likewise returns `Some(place)` unchanged when the predicate is false (1519-1526). Storage-target branch with `direct` missing maps `frame_write_place` paths to `Declared` (1472-1477).

**Callers:** `place_requires_local_write_origin` — 5 matches in `flow_mutation.rs` (read); `assignment_storage_regions` — 3 matches in same file.

**Reachable input (plain words):** An assignment whose target place’s root symbol is not resolved as a pre-statement local (failed lookup at 1497-1500) still records writes at the authored place as declared storage, so a write intended to go through a reference binding could be treated as a write to the binding slot itself.

## A0130

Digest (area_09, read): `.or(Some(places))` keeps unclosed places when alias closure fails; comment at FM:4571 says unresolved origin needs full invalidation.

**FM:4612** (`read`, same file): `statement_storage_writes` (4573-4613) calls `close_storage_places_over_aliases` (4604-4611); on `None`, `.or(Some(places))` returns the pre-closure `places` vector (4612). `close_storage_places_over_aliases` returns `None` when `resolver` is missing or origins lookup fails (1247-1265, read).

**Callers:** search `statement_storage_writes` in `omega-rust` — 14 matches across `flow_mutation.rs`, `flow.rs`, `flow_context.rs`, execution plans, and checks (read grep count).

**Reachable input (plain words):** An assignment write whose alias closure cannot run (no call-frame resolver or no `local_write_origins_before_statement`) still publishes only the directly written place(s) for invalidation, not alias-expanded places the comment at 4571-4571 describes.

## A0131

Digest (area_09, read): non-assignment statements write nothing; fact-only, byte-predicate, empty-frame calls write nothing; image-storage origins add no alias.

**FM:4593** (`read`): `statement_storage_writes` returns `Some(Vec::new())` for non-`Assignment` statements (4593-4594).

**FM:3879,3897,4047-4051** (`read`): `call_mutation_writes` returns empty writes for fact-only invocations (3879-3880), byte-predicate calls (3897-3900), empty write-frame activation/accessor targets (4047-4050).

**FM:1283-1289** (`read`): `close_storage_places_over_origins` skips origins whose `source` is `is_image_storage` (1283-1288).

**Callers:** `call_mutation_writes` / `statement_storage_writes` traced from flow build and checks as above.

**Reachable input (plain words):** A state whose effect is only a non-assign statement, a fact-only call, a byte-predicate call, or a builtin with an empty write frame records no storage writes in these paths, so facts about storage not listed in observations are not retired here; image-backed write origins do not add alias places during closure.

## A0132

Digest (area_09, read): context with zero descriptor observation rows agrees.

**FO:1257-1269** (`read`, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_observations.rs`): `descriptor_context_agrees` (1257-1268) uses `.all(...)` on `descriptor_context_observations.for_context(context)`; an empty iterator makes `.all` true (1263-1268).

**Callers:** search `descriptor_context_agrees` in `omega-rust` — 5 matches: definition + `flow_operand_contexts.rs:365` + `flow.rs` re-export + `checks_contracts_prover.rs:1003` (read).

**Reachable input (plain words):** An operand context that never recorded descriptor observation rows passes `descriptor_context_agrees` and is retained in `checks_contracts_prover.rs:1002-1008` when proving shared-reference facts, without any descriptor rejoin evidence.

## What I did not read

- `git show 457adf4a11f:` paths (current checkout lines matched digest citations on read).
- Full bodies of every `exit.returned_value` consumer outside the cited guards (`checks_contracts_exits.rs`, `checks_contracts_writes.rs`, `validation/reference_result_custody.rs` beyond spot checks).
- `matching_mutation_for_fact_place` implementation (`flow.rs`, inferred only via call sites).
- Entire `build_call_flow_fact` / flow state machine drivers beyond caller edges cited.
- Corpus or sample programs that witness outgoing-row multiplicity or label-colliding carry transfers (no compile/run per atlas rules).

SLICE COMPLETE
