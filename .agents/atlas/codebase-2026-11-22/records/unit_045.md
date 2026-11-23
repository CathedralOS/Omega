# Record: unit_045 (area 07-psi-semantics, crate terminal-semantics)

Paths are relative to `source-rust/subsystems/compiler/psi/semantics/src/` (written `S/`). Claims are "read" unless marked grepped or inferred. Counts come from the Grep tool in count mode over `source-rust` (or the stated folder). Every file in the unit tsv (51 files, 21,651 lines) was read in full. Crate position: these files sit in the `terminal-semantics` crate (crate-map section 7); not repeated here.

## S/bytes/byte_sequence_concatenation.rs (1-60)
1. Purpose: states two logical facts about a byte-sequence concatenation: capacity (left length + right length <= capacity) and the length equation (result length = left + right).
2. Shape: `byte_sequence_concatenation_capacity_goal` (:8), `byte_sequence_concatenation_length_equation` (:31).
3. In/out: reads one `Operation` of kind `EstablishByteSequenceConcatenation` (and a `StructuralEffectObservation::ByteSequenceLengthRead` for the equation); returns a `Proposition`.
4. Decides: equation exists only when the observation's source place equals the producer's result place and the path is empty (`:48`); otherwise `Ok(None)`. Keyed on place identity.
5. Cases by shape: lengths are always u64 (`:18`, `:51`).
7. Unused: grep of both names = 3 matches in 2 files (2 in this file, 1 re-export in `S/lib.rs`); no caller outside re-export found in `source-rust`.
9. Fail-open: none seen (a non-matching observation yields no equation).

## S/bytes/byte_sequence_predicates.rs (1-1812)
1. Purpose: decides which byte predicates (`AsciiOnly`, `NoNul`, `ValidUtf8`, `NonEmpty`, plus literal-held facts) are known of a byte place. The same file also holds the scalar constant-folding index (`ScalarConstants`, `MachineScalarIndex`) used elsewhere in the crate. Two unrelated jobs in one file.
2. Shape:
   - requirement: `byte_predicate_requirement` (:16); domain checks `byte_qualification_predicates_in` (:161), `primitive_byte_domain_theory_matches` (:135), `pure_byte_qualifications` (:205), `byte_predicate_carrier` (:185).
   - source resolution: `byte_sequence_source_predicates` (:923) with inner recursive `source_predicates` (:940, memo `Vec`, work budget `structural_places.len()+1` at :1221); `byte_sequence_argument_predicates` (:385); `incoming_byte_sequence_argument_predicates` (:410) and `unchanged_shared_view_predicates` (:447); `byte_transfer_qualifications_hold` (:532); `byte_copied_field_predicates_at_transfer` (:618); `byte_subslice_is_whole` (:826); `shared_entry_byte_predicates` (:1230).
   - call results: `authenticated_call_result` (:237), `byte_owned_call_result` (:277), `byte_view_call_result` (:293), `byte_owned_result_predicates` (:309).
   - goals and literals: `indexed_byte_store_goal` (:1259), `byte_literal_qualifications_hold(_in)` (:1290,:1299), `scalar_array_qualifications_hold` (:1321).
   - scalar constants: `ScalarConstants` (:1397, sorted `Vec<(ValueId, state)>`, states `Unseen/Active/Complete`), `ScalarConstantOrigin` (:1372), `scalar_operation_origin` (:1551), `scalar_operation_literal` (:1589), `MachineScalarIndex` (:1614), `constant_value` (:1759).
3. In/out: reads `TerminalModule`, `TerminalMachine` (places, operations, blocks, terminators, `contract.requires`), type and domain tables; returns `Option<ByteSequencePredicateSet>` (None means nothing known) or `Result<Option<bool>, &str>`.
4. Decides, keyed on place identity and `StructuralPlaceKind` (:959-1213), never on names:
   - block parameter of a shared view: a fact survives only if every incoming edge's argument establishes it (`:999-1048`), intersected per edge.
   - literal place: `holding(bytes)` only when exactly one `EstablishByteSequenceLiteral` produces it (`:1053-1068`).
   - subslice: keeps all predicates only if `byte_subslice_is_whole` (:826) finds `end == length`, a zero-constant start with one producer, and the immediately preceding `ByteSequenceLength` of the same source (`:837-893`); else `predicates.subslice()` (`:1131-1135`).
   - concatenation: predicates come from the operands unless some operation `may_change_owned_source` (`:1185`), then the declared annotation is used.
   - call results: trusted only through `authenticated_call_result` (:237): callee's public contract result must equal the operation's result on type, multiplicity and qualifications (`:263-273`).
   - domains: qualification ids strictly ordered (`:166`), each a complete primitive byte theory (`:135-159`).
   - scalar index: a value is known only if exactly one producer defines it (`:1769-1782`); cycles and budget exhaustion (8192 steps, `:1464`) give unknown.
5. Cases by shape:
   - `byte_copied_field_predicates_at_transfer` (:618): only a `StructuralLeafCopy` from a parameter with `Owned` or borrowed access, through `Field`/`FixedIndex` segments, into a `BoundedOwned` carrier, consumed by exactly one operation with whole `Owned` arguments, with no other operation touching the source and no jump/return carrying it (`:633-740`).
   - `incoming_byte_sequence_argument_predicates` (:410): whole-place arguments only (`:417`).
   - Literal scalars: only `IntegerConstant` and `BooleanConstant` (`:1600-1609`).
   - `byte_predicate_carrier` (:185): `ByteSequence` or `FixedArray` of u8 non-address.
6. Same job elsewhere: the "u8 non-address element" test appears here (:196-198), in `S/calls/call_composition/fixed_byte_view.rs:83-86` and via `byte_predicate_carrier` in `fixed_arrays.rs:61` (read; three sites). Fixed-array consequence derivation is in `bytes/fixed_arrays.rs:119`. Incoming-edge collection `incoming_arguments`/`arrivals_to` (:898,:914) does a job `crate::control_graph::forwarded_bindings` (`control_graph.rs:44`) also does; this file calls the latter at `:1678` and `content_transformations/quotients/origins.rs:11` uses it (not compared further, suspected overlap). Constant evaluation delegates to `crate::inhabitation::known_scalar_value_with` (:1526,:1538); grep of that name = 10 matches in 5 files.
7. Unused/one-caller (grepped): `MachineScalarIndex` = 21 matches in 8 files; `byte_transfer_qualifications_hold|scalar_array_qualifications_hold|indexed_byte_store_goal` = 14 matches in 5 files (3 in this file, 3 in `S/lib.rs`, others in wire `foundation_validation.rs` 3, optimization-unit-semantics `unit_validation.rs` 4, source-to-psi stage 06 `places.rs` 1).
8. Stated limits: `:1001-1002` "Unknown or cyclic sources add nothing."; `:1394-1395` "sparse or hostile IDs cannot size an arena".
9. Fail-open: `byte_literal_qualifications_hold_in` (:1299) and `scalar_array_qualifications_hold` (:1334) return `true` for an empty qualification list: traced, an empty list claims no predicate, so nothing wrong is accepted. `byte_predicate_requirement` (:39) returns `Truth` when `established` implies every required predicate: correct. `shared_entry_byte_predicates` (:1230) adds a predicate from a `requires` clause of the machine contract; trusts that contract (traced: the entry contract is the caller's obligation). No accept-on-unknown found.

## S/bytes/byte_sequence_predicates/tests.rs (1-423)
Test module for the above. Tests: whole-window rule with negatives (nonzero start, foreign measurement, wrong width, duplicate zero producer, shortened end; :28), boolean literal typing (:139), `MachineScalarIndex` forwarding (:193), memoisation `visits == [1; 4]` (:305), arithmetic failure/cycle/unknown giving None (:315), sparse/duplicate ids (:370), mixed boolean incoming values (:399).

## S/bytes/descriptor_predicate_tests.rs (1-397)
Test-only (`:1-3` "Raw source-graph queries, not an admitted terminal product"). A four-block fixture (`fixture`, :78) checks `incoming_byte_sequence_argument_predicates`: facts need every arrival to pass the same entry view, are refused after changed contents (`payload_invalidations`, `unresolved_contents_writes`), after forgotten window geometry, and for a mutable-borrow entry. One test (:274) checks `payload_history` drops byte membership after a branch write.

## S/bytes/fixed_arrays.rs (1-306)
1. Purpose: derives byte predicates of a literal-length u8 array from its complete default clauses.
2. Shape: `argument_predicates` (:15), `referenced_array_predicates` (:64), `consequences` (:119), `byte_array_length` (:50). Own `#[cfg(test)]` (:227).
3. In/out: module, machine, `StructuralArgument`; clauses from `crate::structural_qualifications::DefaultRequirements` (:35); output `ByteSequencePredicateSet`.
4. Decides: AsciiOnly needs a bound `<=127` on every index (`:217`), NoNul needs `>=1` on every index (`:221`), `NonEmpty` if length > 0 (`:214`); completeness = merged rows count equals length and indexes contiguous (`:209-213`). Keyed on subject place `PlaceId(1)` (:120) and `FixedIndex` path.
5. Cases by shape: clause forms understood: `Conjunction`, `LessOrEqual`, `Equal` with a `ByteSequencePredicate` or an 8-bit `IntegerField` bound; `Disjunction`/`Implication` hit `_ => continue` (:165) and the tests (:267-277) confirm they establish nothing.
7. Unused: `pub(super)`; callers in `byte_sequence_predicates.rs:393,:435,:1095,:1115`.
8. Stated limits: `:197-198` "Storage follows the supplied clause roster, never the largest received array extent."
9. Fail-open: unrecognised clauses are skipped (lose facts, never add). None seen.

## S/bytes/operation_bounds.rs (1-90)
1. Purpose: safety goal of byte/element read, write and subslice operations (`index < length`; `start <= end <= length`).
2. Shape: `operation_safety_goal` (:10).
3. In/out: `Operation` plus `ValueTypes`; returns `(ObligationId, Proposition)` or `None`.
4. Decides on operation kind (`:32-87`); operands must have type u64 (`:26`).
5. Cases by shape: `ByteSequenceRead`, `StructuralByteSequenceFieldRead`, `ByteSequenceWrite`, `ElementViewRead` share one goal; `ByteSequenceSubslice`, `ElementViewSubslice` another; all others `Ok(None)` (`:87`).
7. Unused: grep `operation_safety_goal` = 5 matches in 5 files (this file, `S/lib.rs`, wire `obligation_evidence.rs`, stage 06 `places.rs`, `places/byte_views.rs`).
8. Stated limits: `:7-9` "Collection bounds do not establish their own source, access or measurement correspondence".
9. Fail-open: `_ => return Ok(None)` (:87): an operation kind not listed carries no goal from this function. Whether another owner covers a new indexed kind was not traced: open.

## S/call_projection.rs (1-122)
1. Purpose: translates a structural path (fields, cases, indexes, referents) from one instantiation of a type to a matched formal type whose field ids differ.
2. Shape: `translate` (:9).
3. In/out: type table, source and destination type ids, path; returns the translated path or `None`.
4. Decides: fields matched by `declaration_identity` and `relevance`, unique match required (`:39-46`); cases by `declaration_identity` (`:65-71`); referents need equal access and refuse `WriteOnlyBorrow` (`:110`). Identities, not names.
5. Cases by shape: segments `Field`, `Case`, `FixedIndex`, `Referent`; else `None` (`:118`).
7. Unused: grep `call_projection::translate` = 3 matches (stage 05 `places_contents.rs` 1, stage 06 `places.rs` 2).
9. Fail-open: when `source == destination` and no case payload is pending, the rest of the path is copied unchanged (`:21-24`): traced, identical types need no translation.

## S/callable_envelopes.rs (1-429)
1. Purpose: validates declared callable operational envelopes (service reach, invocation ceiling, may-suspend, may-block, termination premises) and checks a selected machine stays inside the required envelope.
2. Shape: `validate_callable_operational_refinements` (:136), `validate_callable_envelope` (:278), `validate_callable_bounds` (:296), `callable_envelope_refines` (:322), `callable_operational_refines` (:357), private `validate_bounds` (:33), `CallableOperationalView` (:341); one inline test (:399).
3. In/out: `TerminalModule` catalogs; `Result<(), &'static str>` (error text only).
4. Decides: subset tests on `invocations`, `suspends`, `blocks` and termination premises (`:361-390`); strict ordering of service and invocation lists (`:54-65`); formal namespace by position (`:41-53`); invocation of a scalar formal must be `ScalarType::Callable` (`:94-98`). Keyed on service ids and parameter positions.
7. Unused: grep for `hosted_primitive_requirement|validate_descriptor_refinement_evidence|validate_callable_operational_refinements|validate_closed_reach_applications|validate_closed_conformances` = 27 matches in 14 files.
9. Fail-open: `validate_callable_bounds` takes `service_reach: None` as an explicitly open ceiling (doc `:295` "without closing an explicitly open service ceiling"): stated. `callable_operational_refines` checks termination only when the required side is `Terminates` (`:374`): a selected machine with any termination class passes against a `NotGuaranteed` requirement. Traced as intended.

## S/callable_values.rs (1-1338)
1. Purpose: computes which constructed code values (machine descriptors) a callable-typed scalar or storage location may hold, following SSA joins, stored payloads and returned values to a fixed point.
2. Shape: `CallableValueQuery` (:48), `CallableDefinition` (:77), `CallableValueBody` (:103), `CallableValueProgram` (:275), private `Solver` (:1030), `resolve_callable_value` (:1218), `resolve_reached_value` (:1260), `resolve_callable_value_at_invocation` (:1297); methods `sources` (:660), `incoming` (:712), `stored_sources` (:758), `input_actual` (:393), `validate_actuals` (:538), `validate_occurrence` (:347), `structural_actual_matches` (:497).
3. In/out: `TerminalCallableCatalog`, structural types, a descriptor graph (`Vec<DescriptorMachine>`) and per-machine bodies; output `CallableValuePossibilities` (descriptors, inputs, unresolved ceilings).
4. Decides: join of all sources; a bottom value becomes the contract ceiling (`:1241-1248`); a storage query below an earlier shorter-path query returns the ceiling (`:1069-1077`); opaque or ambiguous write sources give `Source::Ceiling` (`:889,:907,:957`). Keyed on machine id, descriptor position, location (root place + path), contract id.
5. Cases by shape: `stored_sources` fast path accepted only when `unchanged_stored_projection_sources_in` returns exactly one source (`:809-838`), otherwise predecessor equations. `structural_actual_matches` accepts equality, or same-access non-owned types equal on nine declaration fields plus `descriptor_reborrow_shapes_match` (`:502-535`).
6. Same job elsewhere: the outer reached-inputs fixed point in `callable_values/reachable.rs:402-443` repeats the loop shape of `:1230-1252` (read).
7. Unused: grep `resolve_callable_value_at_invocation` = 1 match, its definition (`:1297`); no caller in `source-rust`.
8. Stated limits: `:3-4` "never searches for functions with an eligible signature or calling plan".
9. Fail-open: unknowns widen to ceilings (more targets), not narrower. `CallableDefinition::Unsupported` errors (`:697`). None seen that accepts an unknown as safe.

## S/callable_values/reachable.rs (1-455)
1. Purpose: from entry roots, finds reached invocation sites, the machines they select, and binds formal code inputs to caller values (outer fixed point).
2. Shape: `reachable_callable_invocations` (:338), `reachable_callable_invocations_with_roots` (:345), `CallableInvocationRoot` (:7), `ReachedSolver` (:56), `checked_operation` (:18).
3. In/out: `CallableValueProgram`, root machine ids, bound registrar roots; output `ReachableCallableInvocations`.
4. Decides: a site is reached iff its predecessors reach the machine entry (`:141-147`); a cleanup edge also needs `includes_edge` (`:155-163`); a bound root must name exactly one boundary registrar occurrence (`:377-382`).
5. Cases by shape: a recursive structural projection input whose shorter suffix path is already present is cut to the contract ceiling (`:93-105`).
7. Unused: re-exports at `callable_values.rs:18`; `reachable_callable_invocations_with_roots|boundary_call_reach|reached_boundary_calls` = 27 matches in 11 files.
9. Fail-open: final pass adds the contract ceiling to unresolved targets (`:444-453`): widening. None seen.

## S/callable_values/terminal.rs (1-571)
1. Purpose: projects a `TerminalModule` into a `CallableValueProgram` (bodies, invocations, cleanup calls) and reports reached boundaries.
2. Shape: `terminal_callable_values` (:152), `terminal_callable_values_with_error_context` (:159), `boundary_call_reach` (:42), `reached_boundary_calls` (:33), `project_callable_invocation` (:457), `project_owned_cleanup_invocation` (:502), `stored_callable_definition` (:554), private `invocation` (:385), `CallableValuePreparationError` (:89).
4. Decides: a scalar definition is `Constructed` for `SelectMachine`, `Invocation` if a primary invocation sits at the position, `Opaque` for `BoundaryCall`, else a stored read or `Unsupported` (`:242-250`).
5. Cases by shape: invocation kinds `Call`, `CallUnit`, `CallStructuralScalar`, `CallStructuralWithScalarArguments`, `CallStructural`, `CallMachine`, `CallDynamic`, `BoundaryCall` (`:385-454`); terminators `InvokeNoNormal`, `ExternalCompletion` (`:259-286`).
8. Stated limits: error text `:65` "unresolved callable arrivals need their complete boundary demand representation" is a refusal when a reached unresolved ceiling has invocations.
9. Fail-open: none seen.

## S/callable_values/tests.rs (1-905)
Test module over the production projection: entry calls binding two selections through one formal (:253), read cut after an overlapping write (:435), external callable input keeps an unresolved ceiling (:524), ungrounded recursion becomes a ceiling (:549), stale contract and missing SSA are errors (:571), body service summaries (:615-688, call `crate::body_services::body_service_reaches`), descriptor reborrow authority (:691), reborrows keep declaration requirements (:770), missing boundary context (:836, :859). Test-only.

## S/calls.rs (1-304)
1. Purpose: module root of call judgments. It also holds `const_array_argument_matches` (:16), `nominal_argument_matches` (:29) and private `dependent_argument_matches` (:107), which decide whether an actual structural type supplies a formal type, including array-length parameters substituted from scalar actuals.
4. Decides: types walked with a pending stack; a callee `ArrayExtent::Parameter` must correspond to the caller's value (`:270-292`) or to a constant via `constant_runtime_index` (`:289`); nominal matching compares record storage, case and field identities (`:202-255`). Keyed on identity indexes and extents, not names, except `nominal_argument_matches` requires non-empty application identity text (`:62-66`).
5. Cases by shape: `Record`, `Sum`, `Mixed`, `Reference`, `FixedArray`; anything else false (`:257-269`).
7. Unused: grep `const_array_argument_matches|nominal_argument_matches` = 9 matches in 6 files.
9. Fail-open: `:131-139` accepts an identical pair whose referent has no declaration if it is `abstract_opaque` (`continue`): traced, only identical ids pass.

## S/calls/call_composition.rs (1-343), bounded_byte_owner.rs (1-101), fixed_byte_view.rs (1-310)
1. Purpose: a six-row schema table for call operations; helpers deciding when a byte buffer or fixed array can be lent to a byte-view or bounded-owned formal.
2. Shape: `CallCompositionSemanticRow::ALL` (call_composition.rs:268, built by macro `call_composition_rows!` :263 with rows `Call`, `CallUnit`, `CallStructuralScalar`, `CallStructural`, `CallStructuralWithScalarArguments`, `BoundaryCall` at :306-311), `validate_call_composition_semantic_rows` (:321), `boundary_buffer_capacity` (:24), `shared_boundary_buffer_capacity` (:39), `stored_view_field_presentation` (:54), `bounded_byte_owner_capacity_in` (bounded_byte_owner.rs:31), `fixed_byte_array_window` (fixed_byte_view.rs:102), `fixed_collection_storage_window` (:172), `fixed_byte_array_loan` (:269), `borrowed_referent_presentation` (:31).
4. Decides: each row must equal `canonical_schema(tag)` (:328). Windows need `start <= end <= length` (fixed_byte_view.rs:192); an erased empty array lends no backing (`:160-168`); a sub-window keeps only `AsciiOnly` and `NoNul` (`:294-305`); an owned buffer lends to an equal-capacity bounded owned formal only if predicates imply the required set and (non-shared) the required set implies the original (bounded_byte_owner.rs:93-98).
5. Cases by shape: `readable_collection_loan` accepts mutating/reading-borrow pairs or shared-shared only (`fixed_byte_view.rs:252-261`); `fixed_element_array_extent` accepts only `Field` path segments (`:214-218`).
7. Unused: grep `boundary_buffer_capacity|shared_boundary_buffer_capacity|stored_view_field_presentation` = 16 matches in 6 files. Schema table: `call_composition_semantic_row` (:335) callers counted in the same grep family not run separately.
9. Fail-open: none seen.

## S/calls/callable_refinement.rs (1-847)
1. Purpose: builds the ordered refinement goals (input, normal output, guarded outputs, crash causes) that a selected machine must prove to refine a required callable contract; validates the certificates for them.
2. Shape: `contract_refinement_goals` (:322), `callable_contract_refinement_goals` (:278), `descriptor_refinement_goals` (:705), `validate_descriptor_refinement_evidence` (:734, calls `proof_admission::verify_obligation` at :757), `validate_callable_evidence_contracts_in` (:768), `validate_callable_result_refinements` (:187), `CallableContractView` (:169).
4. Decides: goals `required_inputs => selected_inputs`; `required_inputs ∧ selected_outputs => required_outputs`; per guard; crash goal per selected cause `required_inputs ∧ selected guards => required guards` (`:576-646`). Evidence lanes paired by `same_evidence_meaning` (:103) on subjects, declaration, interface.
5. Cases by shape: result kinds Unit/Scalar/Structural must match (`:336-349`).
9. Fail-open: crash goals are produced only when `required.crash_policy == Closed` (`:608`); `callable_contract_refinement_goals` fixes both views to `Closed` (`:305,:314`). Other call sites of `contract_refinement_goals` with a different policy were not traced: open.

## S/calls/closed_conformances.rs (1-1233)
1. Purpose: validates closed conformance applications (schemas, rows, realization callables, static applications), authenticates the hosted `ProcessExit` primitive declaration, and checks each selected public call against its application.
2. Shape: `hosted_primitive_requirement` (:115), `validate_boundary_callable` (:257), `validate_closed_conformances` (:719), `dispatch_application` (:589), `dispatch_applications` (:641), `definition_types` (:455), `identity_package` (:664), `static_application_order_key` (:1195), `hosted_process_exit_identities` (:19).
4. Decides: exact string equality of canonical identity text against expected identities built at `:19-110` from the literal names "ProcessExitPrimitive", "ProcessExit", "ProcessExit::exit_process" (`:34-36`) and digests `core_revision::PROCESS_EXIT*`; one long `||` chain at `:145-242`. This is source-spelling-keyed (identity strings).
5. Cases by shape: only `HostedOperation::ProcessExit` (`:133`); schema kinds `Open`, `Nongeneric`, `Finite` (`:753-803`); schema definition bytes decoded with domain tag `omega.requirement-schema.v5\0` (:459).
7. Counted in the 27-match grep above.
8. Stated limits: `:52-54` "an identity string proves neither fact", so physical i32 and emptiness are checked separately (`:198-206`, `:248-253`).
9. Fail-open: `hosted_primitive_requirement` returns `Ok(None)` for a boundary without declaration or not `HostedPrimitive` (`:123,:131`): traced, "not this class". `validate_boundary_callable` returns `Ok(())` when `declaration` is `None` (`:261`).

## S/calls/closed_reach.rs (1-515)
Validates closed-reach applications: telescope kinds (:3), commitment recomputed from template and arguments (`validate_machine_application_arguments`, :34), operational contract commitment and may-suspend/block (`validate_operational_contract`, :81), service rows with parent closure (:161), selected callee equals retained contract (`:368-390`), `crashes none` binder (`:350`). Entry `validate_closed_reach_applications` (:133). Keyed on identities and 32-byte commitments. Fail-open: none seen (mismatches are `Err`). Counted in the 27-match grep.

## S/calls/evidence_calls.rs (1-807), evidence_calls/inputs.rs (1-201), evidence_calls/terminal_inputs.rs (1-201)
Purpose: checks every runtime call's evidence arguments against declared Requires lanes and their source (entry scope, guarded outcome, or earlier output), with snapshot-value correspondence. `validate_runtime_evidence_inputs_in` (evidence_calls.rs:30) returns `Ok(())` early when there are no Requires lanes and no runtime evidence calls (`:41-51`). `terminal_inputs::validate` (terminal_inputs.rs:5) builds `RuntimeEvidenceInputs` (inputs.rs:168) from a module. Decides: closed arguments compared by string equality of public argument text (`:203`); scalar snapshots equal if entry equal or value equal (`:206-220`); guarded source needs the selected result case at the use (`:633-642`) and exact use counts (`:643-665`). Fail-open: none seen; the early `Ok(())` accepts only modules with no evidence lanes and no snapshot-consuming calls (traced from the condition).

## S/calls/invocation_inputs.rs (1-43), S/calls/layout_target.rs (1-151)
`validate_empty_erased_invocation_inputs` (invocation_inputs.rs:4) refuses when the target has Requires lanes or erased formals; grep = 7 matches in 4 files. `snapshot_fields` (layout_target.rs:20) checks the sealed core `LayoutTargetFacts` boundary shape; keyed on identity text "toolchain::LayoutTarget" (`:93`) and "toolchain::LayoutTargetFacts" (`:113`) plus core layout digest (`:32`). Fail-open: none seen.

## S/content_transformations.rs (1-428)
1. Purpose: validates a `TransformContent` operation: owned disjoint operands, claim equations, memberships.
2. Shape: `validate_shape` (:90), `validate_contract` (:237), `validate_parent_memberships` (:177), `validate_memberships` (:371), `structural_path` (:21), `claim_term` (:54), `place_type` (:76).
4. Decides: operands must be `Owned` and pairwise non-overlapping (`:262-273`); output carrier equals source carrier (`:275-279`); rosters strictly canonical (`:254-261`); input claims and output claims disjoint, every parent covered (`:361-367`); result memberships must equal inherited plus derived (`:420`).
7. Unused: grep `validate_quotient_catalog|validate_shape\(|content_transformations::validate_contract|validate_parent_memberships` = 50 matches in 19 files (mixed; `validate_shape(` matches other names).
9. Fail-open: none seen.

## S/content_transformations/ancestry.rs (1-357), ancestry/arrivals.rs (1-144), ancestry/retirement.rs (1-622)
1. Purpose: authenticates the claimed lineage of a content transformation: each ancestry step must replace a known frontier of claims, producers must dominate the consumer, and call steps must match the callee's returned-claim origins. `retirement.rs` decides whether an input claim a callee receives is fully disposed of on every normal path.
2. Shape: `validate_frontier` (ancestry.rs:41), `validate_producers` (:84), `authenticate_step` (:130), `call_input_origins` (:185), `local_roots` (:264), `OriginTraversal` (:13, work budget 65,536 at :26, depth 128 at :292); `arrivals::roots` (arrivals.rs:66); `retirement::call_input_is_retired` (retirement.rs:59), `entry_claim_is_retired` (:435), `advance_owned_frontier` (:123), `apply_nominal_disposals` (:298).
4. Decides: producer position must precede and be dominated (`:111-117`, uses `control_graph::dominators`); a returned claim's roots must be identical across every normal return (`:252-257`); retirement says false if the claim is returned or the callee has no grounded normal return (error) (retirement.rs:516-526, :612-614).
5. Cases by shape: `advance_owned_frontier` handles `TransformContent`, direct callee calls, and "else" which errors on opaque/dynamic custody transfers (retirement.rs:263-272) and on an owned `StoreStructuralField` of a claimed value (`:273-282`). Terminators handled: `Return*`, `Jump`, `Conditional`, `StructuralCase`; `Crash|EliminateEmpty|ExternalCompletion|InvokeNoNormal` do nothing (`:606-609`).
8. Stated limits: error texts at retirement.rs:269-281 say opaque/dynamic custody transfer and borrowed-storage exit are not authenticated nominal retirement.
9. Fail-open: retirement.rs:606-609 treats crash, empty-elimination, external completion and no-normal invoke terminators as retiring nothing and continuing (no failure); then `normal` must still be set by some return (`:612`). Traced: a claim leaked only on a crash path is not reported. Whether crash-path leaks are covered by another checker was not traced: open.

## S/content_transformations/boundary_correspondence.rs (1-500)
1. Purpose: checks boundary-body content custody: public conservation laws substituted into the derivation, authority (declared law, provider candidate, domain authorization), and the bijection between owned address rosters of inputs and outputs.
2. Shape: `validate_relation` (:13), `validate_authority` (:110), `validate_backing` (:253), `owned_addresses` (:314).
4. Decides: equation equality after substitution; every substitution used (`:101`); input law anchored to an entry claim of a positional parameter (`:157-190`); output domain must `authorizes_boundary_requirement` (`:237-247`).
5. Cases by shape: backing storage must be a closed owned shape (`Record`, `Sum`, `Mixed`, `FixedArray`, scalar, atomic cell); `Opaque`, `TypeParameter`, `Reference`, `ByteSequence`, `ElementView` refuse (`:475-481`).
9. Fail-open: `validate_relation`, `validate_authority`, `validate_backing` return `Ok(())` for non-boundary custody (`:21,:115,:260-262`): ordinary custody is checked elsewhere (traced to `validate_contract` at content_transformations.rs:286-293).

## S/content_transformations/domain_preservation.rs (1-589), domain_preservation/tests.rs (1-175)
1. Purpose: decides whether a write to a path preserves a domain's membership predicate and content projection, by computing the storage paths the predicate reads and testing overlap.
2. Shape: `predicate_dependencies` (:125), `term_dependencies` (:14), `domain_predicate_is_closed` (:244), `domain_predicate_observes_no_storage` (:252), `domain_write_is_disjoint` (:339), `store_preserves_qualifications` (:415), `store_preserves_domain_content` (:441), `scalar_field_store_preserves_domain_content` (:465), `reference_preserves_ancestor_qualifications` (:484), `scalar_field_store_preserves_content` (:506), `store_preserves_content` (:525).
4. Decides: a predicate is closed only if every scalar term is a literal or a field/extent read of the subject place at the current version (depth cap 256 at :22,:138); writes through `Referent` or `RepresentationView` are never disjoint (`:345-352`).
5. Cases by shape: scalar term kinds enumerated at `:25-110`; unlisted terms return `None` (`:110`); propositions enumerated at `:141-228`, else `None`.
7. Unused: grep of the seven preservation names = 39 matches in 9 files.
9. Fail-open: `domain_content_write_is_disjoint` returns `true` when the domain has no content projection (`:396-398`): nothing to preserve. `reference_preserves_ancestor_qualifications` returns `true` for `SharedBorrow` or empty path (`:493`). `store_preserves_content` returns `true` for claims whose input path does not overlap the write (`:557-559`). All traced as structural. Tests: extent predicate closure (:12), case membership closure (:83).

## S/content_transformations/quotients.rs (1-155) and quotients/{opacity.rs (1-220), operation.rs (1-231), origins.rs (1-129), representations.rs (1-220)}
1. Purpose: validates the quotient catalog: unique formation rows, identity agreement, adaptation licenses (`Mint`, `Input`, `Result`), call correspondence, and that quotient values are only transported whole.
2. Shape: `validate_quotient_catalog` (quotients.rs:103), `quotient_copy_adaptation` (:27), `validate_quotient_opacity` (opacity.rs:102), `validate_operation` (operation.rs:3), `whole_origin` (origins.rs:20), `scalar_origin` (:78), `validate_quotient_representations` (representations.rs:6), `validate_quotient_representation_identities` (:75), `validate_carrier` (:104), `same_carrier_shape` (:172).
4. Decides: opacity — an operation producing a quotient must be a copy, move, case-copy or whole call (`opacity.rs:115-127`); observers of a quotient must be transport kinds (`:185-199`); case terminators may not observe one (`:209-215`). `origins.rs` follows copies and block parameters to a unique origin or None (`:28-31,:50-52`).
5. Cases by shape: quotient carrier must have visible owned storage; `TypeParameter`, `Opaque`, `AtomicCell`, `Reference`, `ElementView`, borrowed/bounded byte views refuse (representations.rs:136-146).
9. Fail-open: `validate_quotient_opacity` returns `Ok(())` immediately when the catalog has no representations (`opacity.rs:105-107`); traced: `validate_quotient_catalog` first runs `validate_quotient_representations` which refuses a declaration with `quotient_carrier` but no formation row (representations.rs:10-20), so a quotient type cannot slip in without a row.

## S/continuation.rs (907) and continuation/{coordinates.rs (459), declarations.rs (807), frontier.rs (424), predicates.rs (316), returned_views.rs (237), returns.rs (207)}
1. Purpose: proves that a parked entry (a suspended stack) can be switched to another admitted program: everything executed before the cut must be identical up to renumbering of identity-table coordinates.
2. Shape: `CheckedPrefix::check` (continuation.rs:40), `check_stack` (:53), `machine` (:172), `edges` (:280), `operation` (:430), `paths_from` (:699); `ReachedTypes` (declarations.rs:15), `structural_type` (declarations.rs:47), `types_match` (:208), `signature` (:663), `boundary` (:764); `MemberPairs` (coordinates.rs:23); `suspension` (frontier.rs:97); `same_predicate` (predicates.rs:17); `machine_result` (returns.rs:11); `ReachedInvocations` (returned_views.rs:63).
3. In/out: two `TerminalModule`s (old, new) and a stack of calls; output `CheckedPrefix` (a proof object that `accepts(old,new)` by pointer equality, :162) or `Err(&str)`.
4. Decides: the two machine graphs are walked in lock step; each visited block pair must be unique on both sides (`:200`); operations compared after normalising paths; control transfers compared by cloning the new terminator, rewriting it to the old, and requiring structural equality (`:327-426`). Identity numbers are translated via identity text (`field_identity`, declarations.rs:34-42, `index_of(text)`), i.e. keyed on identity strings. Type match walks both type graphs requiring a one-to-one correspondence (`:216-222`).
5. Cases by shape: `operation` (:467-541) accepts only a listed set of kinds (calls, arithmetic, element views, establishes, stores, moves, reads, literals) or any kind whose `operation_semantic_row` is a goal-free scalar leaf; anything else: "no checked correspondence for an operation before the preparation" (`:540`). `edges` accepts `Jump`, `Conditional`, the three returns, `StructuralCase` (`:328-421`); others: "no checked correspondence for the reached control transfer" (`:422`). Calls with `static_reach_binding` or `required_contract` are refused (`:439-445`).
6. Same job elsewhere: `continuation/predicates.rs:46-58` and `calls/callable_refinement.rs:60-86` both walk a `Proposition` and translate it; different keys (identity translation vs atom remap). Suspected only.
7. Unused: grep `CheckedPrefix` outside `continuation/` = 78 matches in 15 files (44 in `tests/checked_continuation.rs`; consumers in build evaluation, source-to-psi `compile_build/continuation.rs`, interpreter `embedding/continuation.rs`).
8. Stated limits: OWNER_CODE_CONCERNS.md:81 (crate-map) names `declarations.rs:208` `types_match`; `declarations.rs:263-264` refuses `nominal_cleanup` types outright; `:279-282,:284,:384-389` refuse callable-typed fields and storage; `preserves_services` lets only a parked root gain services (`:493-494`, comment "Only a parked root may gain inferred effects").
9. Fail-open: `scalar_set` returns `true` for the zero id (declarations.rs:101); `identity` returns true when both identities are absent (`:23-25`); `structural_domain` rejects domains with `access_policy` or `content_projection` (`:190-193`). `blocks_reaching` limits the old frame comparison to blocks reaching the call (`continuation.rs:185-194`): paths that never reach the call are not compared (stated at `:182-184`: "have contributed no state"). Traced to the comment; no wrong input found.

## S/control_graph.rs (1-418), control_graph/dominance.rs (1-176)
1. Purpose: reachability, successor and forwarded-binding queries, normal-completion possibility, cyclic components, dominators over `TerminalMachine` blocks, and a standalone dominance-forest builder.
2. Shape: `successors` (:26), `forwarded_bindings` (:44), `normal_completion_possible(_in)` (:121,:216), `cyclic_components` (:250), `validate_control_graph` (:299), `reverse_postorder` (:328), `DominatorTree` (:344), `dominators` (:367), `control_cycle_identity` (:389), `dominance_forest` (dominance.rs:30).
4. Decides: normal completion is true if some `Return*` or `PendingNormalCompletion` block is reachable, following `InvokeNoNormal` callees (`:194-212`); `Crash`, `EliminateEmpty`, `ExternalCompletion` stop (`:204-206`). Cycle identity is a SHA-256-derived id of machine id plus edge triples (`:400-413`).
7. Unused: `dominance_forest` = 16 matches in 3 files (this crate 14, stage 06 `graph.rs` 2). `normal_completion_possible` = 10 matches in 6 files.
9. Fail-open: `dominators` (:367) calls `.expect("the validated machine graph is reachable...")` (:385); it does not itself validate the graph, so an unvalidated machine with an unreachable block panics rather than refuses (traced to the signature only; callers' validation not traced: open). `PendingNormalCompletion` counts as "can return" (`:197-200`, stated: "An open tail can never prove a no-normal callee").

## S/descriptors.rs (1-14)
Module root declaring `replay`, `descriptor_lifetimes`, `membership_presentations`, `primitive_store_memberships`, `selected_memberships`, `descriptor_results` (:7-14). No logic.

## S/descriptors/descriptor_lifetimes.rs (1-862), descriptor_lifetimes/{arrivals.rs (116), locations.rs (434), residences.rs (162), resource_tests.rs (378)}
1. Purpose: data-flow over descriptor locations (a place + path naming a borrowed descriptor). Computes where a borrow is live, which names still authorize it, which carriers survive on any arrival, and which edge chose which loan at a join.
2. Shape: `DescriptorFlowStep` (descriptor_lifetimes.rs:31), `DescriptorFlowEdge` (:112), `descriptor_entry_lifetime` (:458), `descriptor_location_lifetime` (:489), `descriptor_successor_lifetime` (:514), `DescriptorLifetime` (:446: `live`, `demanded`, `authorized`, `surviving`), `joined_loan_authority` (:633), `transported_locations` (:725), `transport_authority` (:810); `DescriptorContentsStep/Query`, `DescriptorFlowSource`, `DescriptorFlowBinding` (arrivals.rs:7,:45,:51,:83); `descriptor_location` (locations.rs:36), `resolved_path` (:309), `view_element_descriptor_prefix` (:206), `fixed_runtime_index_alternatives` (:132); `descriptor_residence_live_positions` (residences.rs:23).
3. In/out: a `BTreeMap<DescriptorPosition, DescriptorFlowStep>` graph, a birth position and a location; output sets of positions and names.
4. Decides: `authorized` is a must-analysis (intersection at joins, started from the may-set to keep unchanged borrows at loops, `:800-806`) while `surviving` is the may-set (`:803-805`); a definition site not "fresh" yields empty authorization (`:552-556`); a loan choice at a join requires exactly one reaching candidate loan per arrival and refuses cycles without a known header loan (`:657-661`, :698-701). Keyed on position and location (root place + path); path steps use declaration ids.
5. Cases by shape: `fixed_element_paths` enumerates at most 4096 elements (`locations.rs:167`) "if ... interval.end - interval.start > 4096 { return None }"; `resolved_path` handles `RepresentationView`, `Referent`, `FixedIndex`, `Case`, `Field` (:324-396), else `None`. `DescriptorContentsStep::may_match` treats an `Element` step as possibly matching any `FixedIndex` (arrivals.rs:18-35).
6. Same job elsewhere: `locations.rs:402` private `declaration` (unique-row type lookup); other private lookups with the same shape at `S/inhabitation.rs:561`, `S/membership_facts.rs:638`, `S/structural/owned_structure/complete_claims.rs:68`, `S/structural_qualifications.rs:362` (grep `fn (structural_declaration|declaration)\(`; not read). `exactly_one` is the shared helper: 81 matches in 31 files under `S/src` (grepped); this unit uses it at `locations.rs` (5) and `domain_preservation.rs` (2) while `callable_refinement.rs:92-98`, `evidence_calls.rs:378-400`, `inputs.rs:90-109` hand-write the same filter/next/next pattern.
7. Unused: `descriptor_residence_live_positions|DescriptorResidenceRules` = 12 matches in 6 files; `residences.rs` is `pub(crate)` module with 3 matches.
8. Stated limits: none by keyword.
9. Fail-open: `descriptor_location` returns `case_fields.is_none().then_some(result)` (locations.rs:129): a path ending inside an unselected case payload is refused. `joined_loan_authority` returns `None` (refuse) for cycles and ambiguity. In `location_live_positions` (`:370-372`) a position with no step is skipped (`continue`), i.e. treated as not observing the name; traced: graph keys come from the same map. None seen that accepts an unknown. `resource_tests.rs` (:23-378) tests loop retention, edge-selected parents, cleanup order, swapped carriers, and one-branch survival.

## Coverage
Read in full: every file in `units/unit_045.tsv`, 51 files, 21,651 lines (line counts from the tsv; each range 1..last line read, in chunks of up to 2,000 lines). Skimmed: none. Not read: none.

## Five things a maintainer most needs to know about this unit
1. `bytes/byte_sequence_predicates.rs` (1,812 lines) holds two different jobs: byte-predicate resolution (`:16-1257`) and the scalar constant index used by wire validation, optimization-unit-semantics and others (`:1372-1809`, `MachineScalarIndex` has 21 grep matches in 8 files).
2. `callable_values.rs:1297` `resolve_callable_value_at_invocation` has no caller in `source-rust` (grep = 1 match, the definition), yet it carries about 40 lines of substitution logic.
3. Judgments across `calls/` and `continuation/` compare identity text and 32-byte commitments, not numbers: `closed_conformances.rs:19-110` builds expected identity strings for `ProcessExit`; `continuation/declarations.rs:34-42` translates ids through identity text; `layout_target.rs:93,:113` match "toolchain::..." strings.
4. `content_transformations/ancestry/retirement.rs:606-609` lets crash, empty-elimination, external-completion and no-normal terminators end a path without disposing a claim; only the requirement of at least one grounded normal return (`:612`) remains. Whether crash-path claim leaks are checked elsewhere was not traced.
5. `control_graph.rs:367` `dominators` panics via `expect` on an unreachable block, and `continuation.rs` (`CheckedPrefix`, 78 matches in 15 files incl. interpreter and build evaluation) refuses any operation kind outside its fixed list (`:467-541`), so adding a new operation kind to Psi needs an edit there to stay switchable.

## What you are unsure about
- Whether the other callers of `contract_refinement_goals` pass a non-`Closed` crash policy (only the two internal calls at `callable_refinement.rs:305,:314` were read).
- Whether `dominators` is only called on validated machines.
- Whether other owners cover `operation_safety_goal`'s `Ok(None)` kinds, and crash-path claim leaks (see item 4 above).
- The overlap between `bytes/byte_sequence_predicates.rs:898-921` and `control_graph::forwarded_bindings` is suspected from signatures, not compared line by line; the duplicate `declaration` helpers listed under `descriptor_lifetimes` were found by grep and not read.
- Callers of `call_composition_semantic_row` and `validate_callable_envelope` were not grepped.
