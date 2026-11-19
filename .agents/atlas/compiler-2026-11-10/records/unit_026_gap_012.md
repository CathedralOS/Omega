Second pass for unit 26

Ranges covered (all in one file, paths relative to `<checkout>/`):

- `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs` lines 11751-17750 (6000 lines, read in full).

Short prefix: `UV` = that file. All line numbers below are in `UV` unless another file is named.

Where the range starts and ends (found by a grep of `mod` headers and by reading `UV:11735-11750`):
- Starts inside `operation_contracts` (`UV:9825`) -> `scalar_types` (`UV:11668`) -> `element_views` (`UV:11748`). Lines 11751-11753 are the tail of `element_views`' `use` list. The `match` that closes at 11745 belongs to `scalar_types::byte_views` (`UV:11681`), before my range.
- Ends inside `structural_catalog::type_declarations::validate_structural_fields` (fn at `UV:17709`), in the middle of a `match (&field.field_type, field.relevance)` arm (`UV:17740-17750`).
- Between those: rest of `scalar_types`, `service_calls` (12391), `structural_access` (12860, with nested `descriptor_access` 12886), `structural_cases` (14243), `values` (14309), the tail of `operation_contracts` (14447-14511), `references` (14512-15507, nested `access_paths` 14533), `runtime_evidence` (15508-15766), `services` (15767-16146), and `structural_catalog` (16147-...) with nested `catalog` (16153), `content_projection` (16495), `function_catalog` (16621), `paths` (17065), `provider_specialization` (17351), `storage_projection` (17379), `type_declarations` (17704).

Relation to the first pass (`notes/unit_026.md`): consistent with it. It said the `StructuralTypes` impl for the `BTreeMap` is at `UV:17341` (confirmed, `UV:17341`), `optimized_catalog_graph` is called at `UV:12842` (confirmed), and `operation_contracts` starts at 9825 (confirmed). No contradictions found.

Identity vs spelling: almost every judgment in this range is keyed on ids (`PlaceId`, `StructuralTypeId`, `MachineId`, `ServiceId`, `StructuralCaseId`, `BlockId`, `OperationId`, positions). Two exceptions keyed on text spelling are in section 8 (content projections found by name; content-claim path segments compared by identity text).

---

## 1. `scalar_types` tail: `element_views` and `operation_scalar_types_match` (UV:11751-12390)

**Purpose.** The per-operation "do the scalar value types fit this operation" check. `element_views::types_match` handles the element-view family; `operation_scalar_types_match` is the big dispatcher for every operation.

**Shape.**
- `element_views::types_match(function, operation, definitions) -> bool` (`UV:11754`, `pub(super)`).
- `operation_scalar_types_match(unit, function, operation, definitions, functions, boundary_machines) -> bool` (`UV:11811`, `pub(crate)`), one `match operation` from 11848 to 12388.

**Inputs/outputs.** Reads a `BTreeMap<ValueId, ValueDefinition>` of scalar types, the function's `structural_places`/`parameters`/`result`, the unit's `dynamic_dispatch`, `structural_types`, the callee `functions` map and `boundary_machines`. Returns `bool`; no mutation.

**How it decides.**
- Keyed on value ids looked up in `definitions` (`scalar` closure, 11822) and on callee `MachineId` / `BoundaryMachineId` lookups (`UV:12246, 12277, 12296, 12317, 12332, 12344, 12358`).
- First two gates for every operation: any structural path containing a `RepresentationView` segment returns `false` (`UV:11833-11840`); every runtime-index selector must be an integer of at most 64 bits (`UV:11841-11847`).
- `EstablishElementView` / `ElementViewSubslice` also require `structural_operation_result_contract(...)` to be `Some` (`UV:11759-11763`).
- `Call` accepts only a callee with no structural parameters, no entry-claim declarations, and no `Parameter`/`Result`/`ProviderAttachment` places (`UV:12246-12256`); then result type and per-argument scalar types are compared by position (`UV:12257-12268`).
- Dynamic `CallStructural` (`UV:11939-11979`): selector must be unsigned fixed 64-bit; arguments must equal the interface slot's `scalar_parameters` by position; result kind must match `signature.result`.
- `ExternalCompletion` / `BoundaryCall` (`UV:12339-12387`): boundary completion kind must match (`External(kind)` / `NormalReturn`), result kinds must match; structural boundary results compare type, multiplicity, qualifications, projected qualifications and require `claims.is_empty()` (`UV:12373-12378`).

**Cases by shape.**
- Fixed numeric windows with no comment: `ActivateTaskRegister` `entries` in `2..=8192` (`UV:11871`), `ReloadSegmentDescriptors` `entries` in `1..=8192` (`UV:11886`). Selector `u16`, result `u8` (`UV:11872-11874`, `11887-11889`).
- `ReadTaskRegister`/`ReadSegmentSelector` result must be unsigned fixed 16-bit (`UV:11891-11895`). `StorageAddress` result must be an address integer (`UV:11857-11860`). `EstablishActivationClaim`/`SplitActivationClaim` `committed` value must be unsigned fixed 64-bit (`UV:11854`).
- `IntegerEqual`/`IntegerLess*`: allowed when both integer types are equal, or neither is an address type (`UV:12116`).
- `IntegerArithmetic`: exact add and exact subtract go through `admits_exact_add_operands` / `admits_exact_subtract_operands` (`UV:12172-12182`); everything else needs `binary(left,right,scalar_type)` and, if an obligation is attached, a fixed carrier (`UV:12184-12185`).
- The match has no wildcard arm at the top level; every `AbstractOperation` variant is named (compile-time exhaustive). Inner wildcard arms exist only for result-kind pairs (`UV:11917`, `12380`) and `element_views::types_match` (`_ => false`, `UV:11807`).

**Same job elsewhere (in my unit).**
- The "zip arguments against callee parameters and compare scalar types" block is written five times: `Call` (`UV:12262-12268`), `CallStructural` Unit (`12282-12287`), Scalar (`12298-12303`), Structural (`12319-12324`), `InvokeNoNormal` (`12333-12337`). Same job as `direct_call_arguments_match` territory in `service_calls`, but that one checks structural arguments.
- The dynamic-interface slot lookup `unit.dynamic_dispatch.interface(i).and_then(|i| usize::try_from(slot).ok().and_then(|s| i.slots.get(s)))` is written at `UV:11950-11957` and again at `UV:12493-12500`.
- "unsigned fixed 64-bit integer" test is repeated inline at `UV:11854`, `11929-11931`, `11934-11936`, `11961-11963`; the `unsigned(ty,bits)` closure is written twice (`UV:11867`, `11882`).
- The "u64" scalar helper in `element_views` (`UV:11765-11768`) repeats the same test again; the earlier reader also noted an unsigned-8 repeat at `descriptor_results.rs:2648` (not re-checked).

**Unused or one-caller code (grep `-w` across `omega-rust`, `*.rs`).**
- `operation_scalar_types_match`: 1 call site (`UV:10443`) plus its import (`UV:10411`) and definition (`UV:11811`).
- `element_views::types_match`: called once, `UV:11991` (read).

**Stated limits.** None in this stretch (no comments).

**Fail-open risks.**
- `UV:11927` `EstablishAccessReference { .. } => true`, `UV:11896` `GlobalDescriptorTable { .. } => true`, `UV:11856` `ReleaseActivationClaim | MergeActivationClaim => true`, `UV:12225` `Jump => true`, `UV:12085`/`12106` float/boolean constants `=> true`, and the long list at `UV:12055-12072` (`PrimitiveScalarRead`, `EstablishScalarArray`, `EstablishStructuralLocal`, `ReleaseReference`, `PortWrite`, `StructuralCase`, `ReturnUnit`, `MoveStructuralValue`, `TransformContent`, `EstablishRecord`, ...). These accept without checking any scalar operand type. Which wrong program would get through depends on whether those operations carry scalar values whose types are checked elsewhere (for example `PortWrite`). I did not read the operation definitions: **open**.
- `UV:12086-12088` `FloatResultPolicy` delegates to `policy.accepts_operand_types`; not read: **open**.
- Lookup failures (`functions.get(..)`, `boundary_machines.get(..)`, interface slot) all give `false`: fail-closed.

---

## 2. `service_calls` (UV:12391-12859)

**Purpose.** Two checks on call-like operations: (a) the caller publishes every service the callee can reach; (b) the structural arguments, claim transfers and result of a structural call fit the callee's declared signature.

**Shape.**
- `operation_service_contract_matches` (`UV:12419`, `pub(crate)`).
- `operation_structural_call_contract_matches` (`UV:12457`, `pub(crate)`, one `match` 12470-12666).
- Private helpers: `direct_call_arguments_match` (`UV:12670`), `structural_call_contract_surface_matches` (`UV:12696`), `reference_call_matches` (`UV:12718`), `descriptor_call_origins_match` (`UV:12828`).

**Inputs/outputs.** Reads function/boundary/service maps, `call_accesses: &CallAccesses`, the unit's dynamic-dispatch catalog; returns `bool`. Delegates to `structural_arguments_match` (`structural_access`), `validate_internal_claim_transfers`, `boundary_requirements_match`, `boundary_completion_matches`, and several `payloadless_cases`/`records`/`scalar_arrays` helpers (all outside my range).

**How it decides.**
- Service check keyed on `ServiceId` containment: callee (or boundary) `published_service_ceiling` must be a subset of `caller.published_service_ceiling` (`UV:12427-12447`); `PortWrite` needs the service to exist and be in the caller ceiling (`UV:12448-12451`).
- Structural call check keyed on callee machine id via `callee.machine()` and `functions.get(..)`; a callee not found gives `false` (`is_some_and`).
- `InvokeNoNormal` requires `callee.verified_contract` to exist, `requirement_obligations.len() == contract.requires.len()` and equal crash routes (`UV:12472`). `structural_call_contract_surface_matches` (`UV:12709-12715`) differs: a callee with no verified contract is accepted if `requirement_obligations` is empty. So a missing contract fails for `InvokeNoNormal` and is allowed (with empty obligations) for structural calls.
- Dynamic `CallStructural` with a structural result requires `selected_evidence.is_empty()`, no `qualification_establishments`, no borrowed storage in the result type, and `signature.matches_operation_result` (`UV:12516-12529`). The direct structural-call arm instead calls `payloadless_selected_evidence_surface_matches` (`UV:12624`), so selected evidence is allowed on direct calls and refused on dynamic ones. No comment says why.
- `descriptor_call_origins_match` (`UV:12828`): first asks `call_accesses.has_descriptor_call_origins(machine, result)`; if that returns `Some(matched)` returns it; otherwise clones all type declarations into a `Vec` and builds a whole-unit `optimized_catalog_graph` (`UV:12841-12850`) for this one call, then `reconstruct_call_origins(...).is_some()`. Graph build failure gives `false`.
- It is only consulted when the result is `Unrestricted` and the type is a shared descriptor (`UV:12627-12640`).

**Cases by shape.**
- `reference_call_matches` is a closed list of 17 named conditions in an array `checks` of `(&str, bool)` (`UV:12749-12823`); the labels are discarded: `checks.iter().all(|(_, ok)| *ok)` (`UV:12825`). It requires `Affine` result multiplicity, empty `claims`, empty `claim_transfers`, empty callee entry claims, and `StructuralProjectionPolicy::Unit` argument matching (`UV:12795-12821`).
- Wildcard `_ => true` at `UV:12665` (structural-call match) and `UV:12452` (service match).

**Same job elsewhere.**
- `reference_call_matches` result checks (`sig_type`, `sig_mult`, `sig_qual`, `sig_pqual`, `affine`) overlap with `validate_function_references` result-signature checks in `references` (`UV:15325-15354`) and with `catalogs` dynamic-slot checks (earlier notes `UV:1403-1478`, not re-read).
- `descriptor_call_origins_match` builds `optimized_catalog_graph` per call; the same graph builder is called at `UV:15567` (as `optimized_graph`) and, per the earlier notes, `UV:1214-1226` and `current_ownership.rs` (`163, 2865`). Suspected: the same whole-unit graph is rebuilt in several phases; the `call_accesses` cache at `UV:12837` is the only reuse seen here.

**Unused or one-caller code.**
- `operation_service_contract_matches`: 1 call site (`UV:10481`). `operation_structural_call_contract_matches`: 1 call site (`UV:10461`). Both from `node_contracts` (`UV:10410-10502`, not my range).
- `direct_call_arguments_match`: 2 call sites in this module (`UV:12575`, `12604`).

**Stated limits.** None.

**Fail-open risks.**
- `UV:12437-12442` `unit.call_machine_references(operation).all(...)`: `.all` is true on an empty iterator. If `call_machine_references` yields nothing for a `CallStructural` (for example a dynamic call whose interface has no table rows) no published-ceiling check runs. I did not read `call_machine_references` (defined `optimization-unit/src/lib.rs:225`): **open**.
- `ExternalCompletion`, `Jump`, `StructuralCase`, and every other operation fall to `_ => true` (`UV:12452`) in the service check; only `Call`, `InvokeNoNormal`, `CallStructural`, `BoundaryCall`, `PortWrite` are examined. Whether other operations can reach services is not stated: **open**.
- `UV:12665` `_ => true` for all non-listed operations in the structural-call check: they carry no callee, so no signature to compare (inferred).
- Missing callee, boundary, or interface slot: `false` (fail-closed).

---

## 3. `structural_access` (UV:12860-14242)

**Purpose.** The core "may this place, at this path, with this access, be handed to this formal parameter" decision, plus the "what do we know about a place" lookup (`StructuralSourceContract`) used across the crate.

**Shape.**
- `descriptor_access::backing_access` (`UV:12889`), private to the module.
- `enum StructuralProjectionPolicy { Unit, Projected, Boundary }` (`UV:12941`).
- `struct StructuralSourceContract<'a> { caller, place, structural_type, multiplicity, access, qualifications, projected_qualifications, additional_input_qualifications, operation_result }` (`UV:12948`) with accessors (`UV:12960-13180`): `backing_access`, `claimed_backing_access`, `payload_contract`, `has_only_element_memberships`, `declared_byte_predicates`, `is_unqualified`, `has_result_evidence`, `carries_qualification`, `memberships_disjoint_from_write`, `restorable_qualifications`, `supplies_qualifications`, `reference_ancestor_qualifications_preserved`, `reference_memberships`, `selected_qualifications`, `borrowed_qualifications_match`.
- `structural_arguments_match` (`UV:13182`) -> `structural_arguments_match_at_invocation` (`UV:13205-13824`, about 620 lines in one function).
- `is_material_write_only_type` (`UV:13826`), `owned_bounded_owner_loan` (`UV:13872`), `structural_source_contract` (`UV:13928`), `trivial_affine_local_contract` (`UV:13984`), `structural_operation_result_contract` (`UV:14008`), `view_access` (`UV:14036`), `structural_access_can_supply` (`UV:14204`), `structural_access_is_exclusive` (`UV:14221`), `is_nonempty_field_path` (`UV:14228`), `is_partial_affine_path` (`UV:14235`).

**Inputs/outputs.** `structural_arguments_match_at_invocation` takes the caller function, the invocation site, the call's `StructuralArgument`s (place, path, access), the callee's formal parameters (any type convertible to `StructuralCallFormal`), type and domain maps, and a projection policy; returns `bool`. It builds a synthetic `StructuralParameterDeclaration` per formal (`UV:13222-13233`) and reads the caller's nodes to find byte-literal declarations (`UV:13244-13256`).

**How it decides.**
- Place identity: `argument.place` -> `structural_source_contract(caller, place)`; no contract gives `false` (`UV:13267-13269`).
- Per argument it computes about twenty boolean "presentation" flags (`static_borrowed_path`, `unrestricted_mutable_subloan`, `unrestricted_shared_subloan`, `view_loan`, `buffer_presentation`, `shared_buffer_presentation`, `view_field_presentation`, `fixed_array_presentation`, `bounded_owner_presentation`, `affine_owner_loan`, `completed_result_loan`, `copied_byte_predicates`, `byte_content_qualifications`, ...; `UV:13294-13772`) and then combines them into `actual_multiplicity` (`UV:13702-13731`), which must equal the formal's multiplicity (`UV:13773`).
- Access: `argument.access == parameter.access` (`UV:13638`), and the source must be able to supply it via `structural_access_can_supply`, with one exception: a `MutableBorrow` source may supply an `Owned` argument when the path is non-empty (`UV:13639-13642`).
- Qualifications: each formal qualification must be carried by the source at the argument path (`UV:13778-13788`), unless a byte-predicate or bounded-owner presentation exemption applies.
- Final pairwise loop (`UV:13801-13822`): two arguments on the same `place` with overlapping paths (`structural_paths_may_overlap`) are rejected if either access is exclusive (`MutableBorrow`/`WriteOnlyBorrow`), or if either is `Owned` and the source is an owned `Affine` place.
- `structural_source_contract` lookup order (`UV:13933-13981`): arrival parameter, then the first node whose structural-result place equals the place, then a trivial affine local (`UV:13984`), then a byte-sequence literal (fixed as `Unrestricted`, `SharedBorrow`, `UV:13969-13979`). The node search uses `.find(...)` and does not check that the producer is unique; `descriptor_access::backing_access` does check uniqueness (`UV:12913-12917`).
- `view_access` (`UV:14036-14202`): walks from a view result back to its backing place through `EstablishElementView`/subslice sources, bounded by `for _ in 0..caller.structural_places.len()` and returning `None` when the loop ends (`UV:14201`). `Unrestricted` view result means `SharedBorrow`, `Affine` means `MutableBorrow`, `Linear` gives `None`.

**Cases by shape.**
- Under `StructuralProjectionPolicy::Unit` the argument path must be one of: empty, a "static borrowed path", an all-`Field` path, `[FixedIndex]`, `[FixedIndex, FixedIndex]`, or an owned partial-affine path (`UV:13423-13446`). `Projected` and `Boundary` accept any path shape (`UV:13447-13448`). No comment explains the asymmetry.
- `static_borrowed_path` allows only specific (source access, argument access) pairs (`UV:13389-13409`) and path segments `Field` (non-`ABSENT`), `FixedIndex`, `Case`, `RuntimeIndex`; it rejects `AllElements`, `Referent`, `RepresentationView`, `FixedRange` (`UV:13410-13421`).
- A path containing a `Case` segment is rejected when the argument access is `Owned` or the policy is `Boundary` (`UV:13238-13243`).
- `is_material_write_only_type` (`UV:13826-13870`) accepts only primitive scalars, atomic cells, records whose live fields are scalar/atomic/IEEE-float/structural, and fixed arrays of such; byte sequences, refined/bounded scalars, erased fields, references, element views, sums and mixed shapes return `false`.
- `structural_access_can_supply` (`UV:14204-14219`): `Owned` supplies anything; `SharedBorrow` only `SharedBorrow`; `MutableBorrow` supplies `SharedBorrow`/`MutableBorrow`/`WriteOnlyBorrow`; `WriteOnlyBorrow` only itself.

**Same job elsewhere.**
- `structural_access_can_supply` (`UV:14204`) and `references::access_covers` (`UV:14940-14950`) have the same truth table (`Owned` covers all; `MutableBorrow` covers `SharedBorrow` and `WriteOnlyBorrow`; otherwise equality). The earlier notes cite a third copy, `descriptor_sources::covers` at `UV:5124-5129` (not re-read by me).
- `is_nonempty_field_path` (`UV:14228`) and the all-`Field` test in `formal_origin` (`UV:15133-15140`) are near-duplicates (the latter also admits `FixedIndex` and `RepresentationView`).
- `trivial_affine_local_contract` (`UV:13984`) and the byte-literal arm inside `structural_source_contract` (`UV:13958-13981`) are two more hand-built `StructuralSourceContract` literals beside `structural_operation_result_contract` (`UV:14008`).

**Unused or one-caller code (grep over the crate's `src`).**
- `structural_arguments_match_at_invocation`: 3 call sites (`UV:12473`, `13194`, definition `13205`); the first is an `InvokeNoNormal` arm, the second is `structural_arguments_match`.
- `owned_bounded_owner_loan`: 2 call sites (`UV:9164`, `13322`), plus the re-export at `UV:14449`.
- `structural_source_contract(`: 115 occurrences across 4 files (`current_ownership.rs` 45, `descriptor_results.rs` 2, `lib.rs` 6, `unit_validation.rs` 62), so it is the widely shared lookup.
- `structural_access_is_exclusive`, `is_partial_affine_path`, `is_nonempty_field_path`: `unit_validation.rs` uses are all in this module (`UV:13426, 13441, 13469, 13724, 13807-13808, 14240` are the ones I read). `current_ownership.rs` has 6 occurrences of the set `storage_projection|is_reference_projection|owned_bounded_owner_loan|structural_access_is_exclusive|is_partial_affine_path|is_nonempty_field_path` (count mode); I did not split them by name.

**Stated limits.** None; the only comments in this module are none (no comments inside `structural_access`).

**Fail-open risks.**
- `UV:13639-13642`: a `MutableBorrow` source may supply an `Owned` argument when `argument.path` is non-empty (moving out through a mutable borrow). Whether this is bounded is decided later: `actual_multiplicity` must equal the formal's, and for an `Affine` formal this path is accepted when `argument.access == Owned && source.access == MutableBorrow` (`UV:13722-13727`) without requiring `is_partial_affine_path`. A program moving an `Affine` field out of a mutable borrow would depend on `type_matches` (`UV:13609-13617`) and `path_shape_matches` (`UV:13447-13448` for `Projected`/`Boundary`) only. No comment; I did not trace callers: **open**.
- `UV:13447-13448` `Projected | Boundary => true`: path shape unrestricted for those policies. Other checks (type match, access) still run: **open** on whether they are sufficient.
- `UV:13244-13265` byte-literal qualification check only runs when the argument place is a byte-literal declaration found in the caller's nodes; otherwise skipped. Intended (inferred).
- `structural_source_contract` `.find(...)` taking the first producer (`UV:13951-13955`): if two operations produced the same place it would use the first. The place table duplicate check is on place declarations (`UV:16726`), not on producers, and I did not trace a producer-uniqueness check: **open**.
- Missing contract / type / lookup: `false` or `None` (fail-closed).

---

## 4. `structural_cases` and `values` (UV:14243-14446) and the tail of `operation_contracts` (UV:14447-14511)

**Purpose.** `structural_cases::validate` checks a `StructuralCase` terminator against the sum/mixed type; `values` checks value definitions, dominance and edge binding arities; `validate_values_and_bindings` runs them together with atomic coherence and per-node contract checks.

**Shape.**
- `structural_cases::validate` (`UV:14250`, `pub(super)`).
- `values::collect_value_definitions` (`UV:14316`), `validate_node_uses` (`UV:14335`), `validate_successor_bindings` (`UV:14398`).
- `validate_values_and_bindings` (`UV:14453`, `pub(crate)`). Re-exports at `UV:14447-14451`: `records::constructible`, `owned_bounded_owner_loan`, `structural_access_can_supply`, `structural_operation_result_contract`, `structural_source_contract`.

**How it decides.**
- `structural_cases::validate`: source place must have a contract and must not be `WriteOnlyBorrow` (`UV:14262-14265`); selected type at `path` must be a `Sum` or `Mixed` (`UV:14273-14277`); successors must be exactly one per declared case, in declaration order (`successor.case != case.id`, `UV:14281-14284`); each target block must exist and have matching counts of scalar and structural parameters (`UV:14285-14290`); each payload must name a case field by id, match parameter value and scalar type, and the field must not be erased (`UV:14291-14303`). The content of `structural_bindings` is only counted here, not compared (`UV:14287`).
- `validate_node_uses`: function parameters are always visible; block parameters and other-block node definitions need the defining block in the use block's dominator set (`UV:14356-14365`, `14382-14391`); a same-block definition needs a smaller node index (`UV:14367-14378`). A missing dominator row gives an error (`is_some_and`).
- `validate_successor_bindings`: for `StructuralCase` delegates; otherwise per edge, binding count equals target parameter count and each binding parameter, scalar type and source type match (`UV:14420-14443`). Structural bindings of ordinary edges are not examined in this function.

**Cases by shape.** `validate_successor_bindings` treats only `O::StructuralCase` specially (`UV:14405-14419`); every other terminator is treated as a plain edge list.

**Same job elsewhere.** Binding checks for ordinary edges also appear in `control_flow::validate_nodes_and_edges` and `affine_authority` (earlier notes `UV:5664-5719`, `UV:597-880`); I did not compare them line by line.

**Unused or one-caller code.**
- `validate_values_and_bindings`: 1 call site (`UV:9647`) plus import (`UV:2743`).
- `structural_cases::validate`: 1 call site (`UV:14411`) (read).

**Stated limits.** None.

**Fail-open risks.** None seen as accepts. Panics: `.expect("u32 fits usize")` (`UV:14371`), `.expect("successor validated")` (`UV:14421`): both abort rather than accept.

---

## 5. `references` (UV:14512-15507)

**Purpose.** Reference-typed values: path/type resolution through `Referent` segments, the identity types used by the ownership tracker, and the whole-function rules for functions that take, build or return references.

**Shape.**
- `access_paths` submodule (`UV:14533-14667`): `call_source_type` (14541), `access_referent_tail` (14574), `project_referent_tail` (14596), `access_path_type` (14646); re-exported at `UV:14668-14670`.
- Types: `ReferenceIdentity { place, path }` (14673), `ReferenceOriginIdentity { place, path: Vec<ReferenceOriginProjection>, extent }` (14679), `ReferenceParent` (14719), `ReferenceOrigin { Invocation, Primitive, IngressLeaf, Alternatives }` (14725), `LiveReference { identity, carrier, carrier_path, root, parent }` (14741), `FormalReferenceSource` (15154).
- Functions: `contains_reference` (14749), `leaf_paths` (14756), `referent` (14769), `reference_projection` (14785, `pub`), `record_field_destination` (14808), `leaf_referent` (14830), `leaf_type` (14846), `reference_access` (14928), `access_covers` (14940), `leaf_access_within` (14952), `projected_carrier_type` (14963), `is_reference_projection` (14975), `storage_projection` (14984), `reference_source_accessible` (15001), `reference_source_type` (15037), `direct_reference_parameter` (15083), `formal_origin` (15100), `formal_reference_source` (15160), `operation_structural_result` (15188), `validate_function_references` (15212), `validate_alternative_result_dependencies` (15438), `access_reference_source_compatible` (15495).

**Inputs/outputs.** Take the function, a `BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>` (or any `StructuralTypes`), a source `StructuralArgument` or path. The identity and `LiveReference` types are pure data consumed by `current_ownership.rs`.

**How it decides.**
- `project_referent_tail` (`UV:14596-14644`) walks the path; allowed segments are `Field`, `FixedIndex`, `RuntimeIndex`, `Referent` (others return `None`, `UV:14637`). At each `Referent` the reference type's access narrows the running authority: if `authority` covers the reference's access the new authority is the reference's access, else if the reference covers the authority the authority stays, else `None` (`UV:14627-14633`); `WriteOnlyBorrow` authority gives `None` (`UV:14624-14626`).
- `referent` (`UV:14769-14783`) returns a referent only for `Reference` types whose access is shared, mutable or write-only, and whose referent id is in the map.
- `reference_source_accessible` (`UV:15001-15035`): `Owned` access refused; `Linear` or result-evidence sources refused; accessibility via `terminal_semantics::reference_source_projection_accessible`; a path without `Referent` also requires the place not to be entry-claim declared.
- `formal_origin` (`UV:15100-15152`): a formal parameter yields `Primitive` (empty path, borrowed access), `IngressLeaf` (path ends in `Referent`, owned or shared parameter), or `IngressLeaf` for a direct reference parameter with an all-`Field`/`FixedIndex`/`RepresentationView` path.
- `validate_function_references` (`UV:15212-15436`), all failures are `catalog_mismatch(function)`:
  - Nodes with a structural result: `MoveStructuralValue`/`TransformContent` need `Owned` source and the static projection tip type to equal the result type (`UV:15231-15248`). A result containing a reference but not itself a reference may only come from `EstablishRecord`, `EstablishStructuralArray` or a structural `CallStructural`, with `Affine` multiplicity, no qualifications, no claims (`UV:15249-15264`). A reference-typed result may only come from `EstablishReference`, `EstablishAccessReference`, or a structural `CallStructural` (`UV:15265-15277`).
  - Parameters whose type contains a reference: multiplicity `Affine` if owned else `Unrestricted`, no qualifications, `constructible`, not entry-claim declared (`UV:15279-15297`). Block structural parameters: `Owned`, `Affine` or `Unrestricted`, no qualifications, constructible (`UV:15298-15313`).
  - Any edge structural binding whose source type contains a reference must be `Owned` access (`UV:15314-15324`).
  - Function result: if it does not contain a reference, `reference_sources` must be empty (`UV:15328-15334`); otherwise `Affine`, no qualifications (`UV:15350-15354`), and `leaf_paths(types, result_type, reference_sources.len())` must return exactly as many leaf paths as there are mappings (`UV:15355-15364`); each mapping path equals the leaf path, sources non-empty, strictly increasing (`UV:15381-15386`). Per source: `Invocation` origins need `Projected` extent, matching endpoint type and covered access (`UV:15389-15408`); `Parameter` origins go through `formal_reference_source`, must be `MutableBorrow`/`SharedBorrow`, must refer to a function parameter, and a (place,path) formal may appear twice only if both uses are shared descendants (`UV:15410-15432`).
  - If any mapping has more than one source, an `Invocation` source, a `WithinRegion` extent or an `Element` projection, `validate_alternative_result_dependencies` runs (`UV:15365-15376`): depth-first search over reference-returning structural calls (direct machine, or every row of every dispatch table for the interface); revisiting an active machine is an error (`UV:15457-15459`), so recursion through reference-returning calls is refused.

**Cases by shape.**
- `leaf_type` (`UV:14846-14926`) walks `Case`, `RepresentationView`, `FixedIndex` (literal-length arrays only, index in range), `Field`; any other segment (`RuntimeIndex`, `AllElements`, `FixedRange`, `Referent`) returns `None` (`UV:14902-14904`). Scalar fields resolve to the `PrimitiveScalar` declaration with the same scalar (`UV:14912-14921`); any other field type gives `None`.
- `operation_structural_result` (`UV:15188-15210`) lists 9 kinds of result-producing operations plus structural `BoundaryCall`.
- `access_reference_source_compatible` (`UV:15495-15506`): result must be a `Reference` whose referent equals the backing carrier and whose access is shared or mutable (not write-only).

**Same job elsewhere.**
- Three path walkers over `StructuralPathSegment` lists: `leaf_type` (`UV:14846`), `access_path_type` (`UV:14646`, adds `RuntimeIndex` on `FixedArray`/`ElementView`), and `structural_catalog::paths::resolve_path` (`UV:17178-17287`, adds `FixedRange` followed by `RepresentationView`, `FixedIndex`/`RuntimeIndex` on `ElementView`). They differ in which segments they accept.
- `leaf_paths`, `leaf_referent` and `reference_source_accessible` have same-named functions in other crates (grepped, not read): `pipeline/02_abstract-operations-to-target-operations/src/references.rs:373, 475` and `pipeline/03_target-operations-to-selected-instructions/src/legalization_reference_custody.rs:4236, 4329, 4399`. **Suspected** duplicates of the same job, not compared.
- `reference_projection` (`UV:14785`) has a differently shaped same-named function at `pipeline/03.../src/structural_inputs.rs:1711`, which is a different function (grep).

**Unused or one-caller code.**
- `reference_projection` (`pub`, re-exported `lib.rs:83`): 1 external call site, `omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_reference_custody.rs:4001` (`optimization_unit_semantics::reference_projection(`). Other grep hits for that name are unrelated functions/fields.
- `validate_function_references`: 1 call site (`UV:9619`).
- `record_field_destination`: 8 call sites (`UV:2017, 3010, 4202, 4647, 7405, 7619, 8717`, and `current_ownership.rs:5315`).
- `is_reference_projection`: only `current_ownership.rs` (import `:3788`, call `:6601`); inside `UV` it is defined at 14975 and used nowhere else in `UV` (grep over `UV`). `storage_projection` (`UV:14984`): imported at `current_ownership.rs:3790`; no use inside `UV` outside its definition. I did not count its `current_ownership.rs` call sites separately.
- `ReferenceOriginIdentity`, `ReferenceParent`, `LiveReference`, `ReferenceIdentity`, `ReferenceOrigin`: used by `current_ownership.rs` (imports `:25`, `:3786-3790`, `:3810-3813`); `current_ownership.rs` has 237 hits for the whole name set of this section (count mode); no uses elsewhere in the crate except `UV`.

**Stated limits.** None (no comments in this module).

**Fail-open risks.**
- `UV:15482-15484`: in `validate_alternative_result_dependencies`, a dispatch-table row that is `None` is skipped (not followed). A reference-returning call via a missing slot therefore adds no dependency edge. Reason not stated: **open** (inferred: an empty slot has no machine to analyze).
- `UV:15396` `unreachable!()` inside a `let ... else` assumes `StructuralReferenceOrigin` has only `Parameter` and `Invocation`; a new variant would panic rather than be rejected.
- `UV:15375`: alternative-result dependency check only runs for the conditions listed; for single-source `Parameter` mappings no recursion check is done. Intentional (inferred).
- Missing contract, missing type, missing parameter: `None`/error (fail-closed).

---

## 6. `runtime_evidence` (UV:15508-15766)

**Purpose.** Reconstructs, from the optimized unit, the inputs the shared evidence validator needs (call sites, lanes, value expressions, payload contracts) and hands them to `terminal_semantics::evidence_calls::validate_runtime_evidence_inputs_in`.

**Shape.** `validate` (`UV:15518`, `pub(super)`) wraps `validate_current` (`UV:15524`) and maps a `&'static str` reason to `InvalidRuntimeEvidence { reason }` (`UV:15521-15522`).

**Inputs/outputs.** Reads `unit.runtime_evidence` (catalog of declarations, lanes, calls, terms), `unit.functions`, and the descriptor graph; produces a list of `RuntimeEvidenceBody` and calls the validator at `UV:15754-15764`.

**How it decides.**
- With no catalog, passes only if no function has any evidence lane (`UV:15525-15534`).
- Catalog must belong to the unit's Terminal input (`catalog.psi() != unit.psi`, `UV:15535`).
- Each function's `evidence_contract_lanes` must equal the catalog's lanes for that machine (`UV:15549-15558`).
- Descriptor graph is built (`crate::descriptor_results::optimized_graph(unit)`) only when some catalog call has a runtime call and evidence arguments (`UV:15559-15571`); failure to build is an error.
- Call operations are collected only when `checks_calls` is true (`UV:15562-15565`, `15585`): `Call`, `CallStructural` with a machine callee, `InvokeNoNormal`.
- Each function needs exactly one checked declaration and a `verified_contract` (`UV:15574-15582`, `15743-15748`).

**Cases by shape.** In the call collector `_ => continue` (`UV:15674`) drops every other operation, including `BoundaryCall`; `CallStructural` with a non-machine (dynamic) callee is dropped by `let Some(callee) = callee.machine() else { continue }` (`UV:15623-15625`).

**Same job elsewhere.** The classification of `Call`/`CallStructural`/`InvokeNoNormal` into (site, callee, role, arguments, result) mirrors the projection `activation_storage::operation` does (earlier notes `UV:92-351`, not compared). `scalar_expression` (`descriptor_results.rs:2733` per earlier notes) is called at `UV:15712`.

**Unused or one-caller code.** `runtime_evidence::validate`: 1 call site (`UV:18267`).

**Stated limits.** None (only error strings).

**Fail-open risks.**
- `UV:15623-15625`: dynamic `CallStructural` operations are not given to the evidence validator. A dynamic call that passes evidence arguments would not be among the validated call positions. Catalog calls come from the catalog (`UV:15538-15543`), so whether a dynamic call can have catalog evidence is not traced: **open**.
- `UV:15541` ignores catalog calls whose caller is not in the unit; by design for pruned machines (inferred).
- `UV:15709-15715` values without a scalar expression are omitted (`filter_map`): the validator sees fewer expressions; whether a missing expression is fatal downstream is not read: **open**.

---

## 7. `services` (UV:15767-16146)

**Purpose.** Validates the service catalog (identities, parent closure, acyclicity), service ceilings, root service reach (derived from the entry machine), provider refinements, and the structural signature of boundary machines.

**Shape.** `index_service_catalog` (`UV:15777`), `valid_service_ceiling` (`UV:15869`), `validate_root_service_reach` (`UV:15885`), `derive_root_service_reach` (`UV:15935`), `refresh_root_service_reach` (`UV:16024`), `validate_provider_service_refinements` (`UV:16049`), `boundary_structural_signature_matches` (`UV:16074`), `structural_signature_matches` (`UV:16125`).

**How it decides.**
- Service catalog: duplicate id, `ABSENT` or duplicate identity, parents not self/not duplicate/known/strictly ascending, no cycle (recursive `visit`, `UV:15823-15843`), and parent closure (every parent's parents must be in the declaration's own parents, `UV:15850-15865`).
- `valid_service_ceiling`: unique, known, every service's parents also in the ceiling, ascending (`UV:15873-15882`).
- Root reach: declared `concrete` and each installation dependency must be a valid ceiling, dependency requirement identity not `ABSENT`, unique and ascending (`UV:15891-15919`); then recomputed with `derive_root_service_reach` and compared (`UV:15920-15931`).
- `derive_root_service_reach` (`UV:15935-16022`): DFS from `unit.entry` through `call_machine_references` and cleanup `InvokeNominal` machines; for `BoundaryCall`, if an installation dependency has the boundary's `identity` as its requirement identity, the boundary's `published_service_ceiling` must equal `fixed_service_reach` union dependency `upper_bound` (`UV:15979-15994`), `fixed_service_reach` is added and the dependency marked used; otherwise the boundary's whole published ceiling is added (`UV:15998`). `PortWrite` adds the service and its parents (`UV:16001-16006`).
- Provider refinement: realized ceiling equals the candidate function's published ceiling and is a subset of the boundary's (`UV:16061-16067`).
- Boundary signature: non-`Machine` kinds must have no attachment; `requires` and `normal_qualifications` strictly ascending; each refers to an in-range argument index and a domain whose carrier equals the type at the path; normal-qualification arguments must be `MutableBorrow` (`UV:16079-16122`).

**Cases by shape.** `derive_root_service_reach` handles `BoundaryCall` and `PortWrite` only; other operations contribute only through `call_machine_references`/cleanup (`UV:15972-16008`).

**Same job elsewhere.** `valid_service_ceiling` is used for boundaries (`UV:1192`), functions (`UV:9576`) and root reach (`UV:15891, 15903`). Published-ceiling subset logic also appears in `operation_service_contract_matches` (`UV:12427-12447`).

**Unused or one-caller code (grep, whole `omega-rust` `*.rs`).**
- `refresh_root_service_reach`: 1 call site (`UV:18223`, in `revalidate_rewrite_output`; I read only the grep line, not the function). It overwrites `unit.root_service_reach` with the derived value (`UV:16045`).
- `validate_provider_service_refinements`: 1 call site (`UV:1207`).
- `boundary_structural_signature_matches`: 1 call site (`UV:1197`).
- `valid_service_ceiling`: 4 call sites outside its definition (`UV:1192`, `9576`, `15891`, `15903`) plus import `UV:2744`.
- `structural_signature_matches`: 2 call sites (`UV:16084`, `16696`).

**Stated limits.** None.

**Fail-open risks.**
- `UV:15976`: a `BoundaryCall` whose boundary is not found returns `OperationServiceContractMismatch(OperationSite::new(function.machine, function.entry, 0))`, a placeholder site (entry block, node 0), not the real call position. Error, not accept.
- `UV:15955-15957`: a missing callee machine during the DFS returns `MissingEntryMachine(machine)`, a name for a different condition. Error, not accept.
- `UV:16024-16046`: `refresh_root_service_reach` replaces the declared reach with the derived one; if the following validation compares declared to derived (as `validate_root_service_reach` does at `UV:15920`), the comparison is trivially equal for rewrite output. I read neither `revalidate_rewrite_output` nor its call order; only the grep line `UV:18223`: **open**.
- `structural_signature_matches` and `boundary_structural_signature_matches` check `qualifications` but not `projected_qualifications` of parameters in these two functions (`UV:16135-16144`, `16084-16122`); the function path does it separately at `UV:16714-16719`. Boundary parameters' projected qualifications are not checked in these functions: **open**.

---

## 8. `structural_catalog` (UV:16147-17750)

### 8.1 `catalog` (UV:16153-16494)
**Purpose.** Index and validate the unit's structural types and domains.
- `index_structural_types` (`UV:16165`): validates quotient catalog and identities (`UV:16171-16181`); per declaration `validate_inline_carriers`, duplicate id, identity `ABSENT`/duplicate (`UV:16184-16207`); ids strictly ascending (`UV:16208-16214`) (the slice `StructuralTypes` impl at `lib.rs:256` binary-searches, so this ordering matters); referent table ids must not collide with types and identities are shared (`UV:16221-16243`); per-shape checks (`UV:16244-16280`): `AtomicCell` must be supported with no `scalar_restriction`; `ByteSequence` carriers listed three; `Record` and `Mixed` fields via `validate_structural_fields`; `Sum` cases via `validate_structural_cases`; `Reference`, `PrimitiveScalar`, `FixedArray`, `ElementView` have no check at this step. Then structural children must exist (`UV:16315-16319`); a `Reference` is admissible when its access is shared/mutable and the referent is in the referent table or a non-reference type, or when write-only and the referent is a `PrimitiveScalar`; `Owned` is `_ => false` (`UV:16320-16348`). An `ElementView` whose element contains a reference is rejected (`UV:16351-16362`).
- `index_structural_domains` (`UV:16366`): duplicate id; identity `ABSENT`/duplicate; one domain per `(semantic_domain, carrier)`; family byte predicate consistency (`UV:16383-16391`); ascending order (`UV:16399-16405`); carrier known (`UV:16406-16415`); required memberships well-formed and complete-theory closure (`UV:16417-16425`); establishment routes ascending with non-empty identity text (`UV:16426-16436`); predicate closed over the carrier (`UV:16444-16450`); `byte_predicate` domains must have no content projection, no required memberships, no routes and a `ByteSequence` carrier (`UV:16457-16465`); content projection valid (`UV:16472-16490`).
- **Cases by shape.** Error mislabeling: type failures that are not about identity (reference admissibility `UV:16341-16347`, element-view-with-reference `UV:16356-16360`, atomic cell unsupported `UV:16249-16254`) and all domain failures (`UV:16392-16470`) return `InvalidStructuralTypeIdentity` / `InvalidStructuralDomainIdentity`. Count of those two names in `UV`: 9 (count mode).
- **Fail-open.** None seen. `types[&declaration.carrier]` at `UV:16462` indexes directly; unknown carriers were rejected at `UV:16406-16415`, so it cannot panic on that path (traced).

### 8.2 `content_projection` (UV:16495-16620)
**Purpose.** Validates `StructuralContentProjection` expressions attached to domains.
- `validate_content_projection_scalar` (`UV:16501`, depth limit 256 at `UV:16508`), `validate_content_projection_expression` (`UV:16567`), `validate_structural_content_projection` (`UV:16588`).
- **How it decides.** `SubjectField`/`RuntimeScalarEmbedding` paths are lists of **strings**; each segment is resolved by `identities.index_of(segment)` against the identity table (`UV:16527`), i.e. keyed on source spelling, then the field is looked up by that identity; the last segment must be a scalar field, earlier ones structural (`UV:16533-16540`). `Natural` must be digits without leading zeros (`UV:16544-16548`). The projection's domain must equal the declaring domain, the report fingerprint must be non-zero, algebra kind must match the expression kind, and a recomputed fingerprint (`language_semantics::content::terminal_projection_report_fingerprint`) must equal the stored one (`UV:16605-16618`).
- **Unused or one-caller code.** `validate_structural_content_projection`: 1 call site (`UV:16476`, import `UV:16155`). `validate_content_projection_expression`: 1 call site outside itself (`UV:16609`). `validate_content_projection_scalar`: calls only inside this module (`UV:16550-16583`).
- **Fail-open.** None seen.

### 8.3 `function_catalog` (UV:16621-17064)
**Purpose.** Per-function structural catalog: places, parameters, claims, byte literals, trivial affine locals.
- `ValidatorStructuralRootKey` (7 variants, `UV:16641-16653`), `structural_root_key` (`UV:16655`), `validate_function_structural_catalog` (`UV:16683`) returning the byte-literal and trivial-affine-local `(place, declaration)` lists.
- **How it decides.** Parameter positions must equal their index; parameter places unique; types known; qualifications match (`UV:16705-16723`). Each `structural_places` row: unique id; `known_type` per kind (`UV:16729-16768`): `BlockParameter`/`Parameter`/`Result`/`OperationResult` are cross-checked against the function; `ByteSequenceLiteral { .. } => true` and `TrivialAffineLocal { .. } => true` are accepted here; `ProviderAttachment` requires `function.attachment == Some(attachment)`. A `Linear` parameter must be entry-claim declared (`UV:16781-16785`). Each structural-result node's place must be declared with the matching `OperationResult` kind (`UV:16820-16835`); `EstablishTrivialAffineLocal => None` (not checked here, `UV:16822`). Entry claims are numbered `1..` by `ClaimId::new(index+1)`, point at a non-`Unrestricted` parameter, resolve to a type, and must not overlap by prefix (`UV:16838-16862`); block entry claims are sorted and similar (`UV:16863-16891`); content entry claims are numbered, versioned `Entry`, rooted at a parameter, projections non-empty and sorted, and (when matched to a structural claim) path segments equal the structural path as **text**: `identities.text(*identity).unwrap_or_default()` compared with `ContentPlaceSegment::Field(String)` (`UV:16901-16947`); other segment kinds (`Case`, `AllElements`, ...) map to `None`, which makes the comparison fail. Each projection must be owned by exactly the domain with the same projection domain, with identical identity and algebra (`UV:16951-16971`). Byte literal ordinals must be contiguous from 0 and the type a `BorrowedView` byte sequence (`UV:16972-17016`); trivial affine local ordinals contiguous and the type an empty `Record` (`UV:17018-17061`).
- **Cases by shape.** Trivial affine local must be an empty record (`UV:17048-17051`); byte literal must be `BorrowedView` (`UV:17001-17006`).
- **Same job elsewhere.** `structural_signature_matches` (`UV:16125`) is called at `UV:16696` and re-checked per parameter below it (`UV:16705-16723`: position, uniqueness, type known, qualifications): the same four checks run twice for function parameters.
- **Unused or one-caller code.** `structural_root_key`: 1 call site (`UV:7178`, import `UV:6648`, re-export `UV:18179`). `ValidatorStructuralRootKey` used only here and via that call.
- **Fail-open.**
  - `UV:16763-16764` `ByteSequenceLiteral`/`TrivialAffineLocal => true`: traced to the ordinal/type checks at `UV:16972-17061` (read), which reject non-canonical ordinals and wrong types; but a literal or local place with no producing operation is not checked in this function: **open**.
  - `UV:16919-16920`: `identities.text(...).unwrap_or_default()` maps a missing identity to `""`; a claim segment `""` would equal it. Reachable only if the identity table lacks the index: **open**.

### 8.4 `paths` (UV:17065-17350)
**Purpose.** Path resolution and qualification-shape checks on structural types.
- `structural_qualifications_match` (`UV:17075`), `structural_projected_qualifications_match` (`UV:17088`), `resolve_structural_path` (`UV:17113`), `resolve_runtime_projection` (`UV:17127`), `resolve_leaf_copy_path` (`UV:17137`), `structural_projection_type` (`UV:17159`), private `resolve_path` (`UV:17178`), `byte_carrier_leaf_window` (`UV:17289`), `impl StructuralTypes for BTreeMap<..>` (`UV:17341`).
- **How it decides.** `resolve_path` is one walker parameterized by a field-type closure: `canonical_leaf_type` (`resolve_structural_path`), `leaf_copy_type(Some(expected))` (`resolve_leaf_copy_path`), or `owner_loans::loaned_field_type` (`structural_projection_type`). Rules: `FixedRange` only on literal-length arrays, `start <= end <= length`, and must be followed by a `RepresentationView` (`UV:17189-17208`); `Case` only on `Sum`/`Mixed`, only once, then only `Field` (`UV:17209-17228`); `RepresentationView` not through a `Reference` (`UV:17238-17239`); `FixedIndex` needs index below a literal length; `RuntimeIndex` on `FixedArray` or `ElementView`; `FixedIndex` on `ElementView` (`UV:17264-17282`). `byte_carrier_leaf_window` (`UV:17289-17339`) hard-codes a `u8` element type (`UV:17317-17323`), rejects any `Case` segment, and requires `index + layout.size <= length`.
- **Cases by shape.** `resolve_leaf_copy_path` rejects any `Case` segment (`UV:17143-17148`); `byte_carrier_leaf_window` accepts a path whose last segment is `FixedIndex` only (`UV:17301-17303`).
- **Same job elsewhere.** Three walkers (`leaf_type`, `access_path_type`, `resolve_path`), see section 5. The `BTreeMap` impl of `StructuralTypes` (`UV:17341`) and the slice impl (`lib.rs:254`) are two implementations of the same trait with different lookup (`self.get` vs binary search).
- **Unused or one-caller code.** `resolve_leaf_copy_path`: 1 call site (`UV:8329`); `byte_carrier_leaf_window`: 1 call site (`UV:8340`) (both re-exported at `UV:18181`).
- **Fail-open.** None seen; unresolved paths return `None`/`false`.

### 8.5 `provider_specialization` (UV:17351-17378)
`validate_provider_attachment_specialization` (`UV:17357`) returns `InvalidProviderAttachmentSpecialization(function.machine)` if the function has any `ProviderAttachment` place; the `_boundary_machines` and `_types` parameters are unused (leading underscore). So a function that the catalog check accepts for having a `ProviderAttachment` place (`UV:16765-16767`) is always rejected by this check. 1 call site (`UV:9587`); no comment explains.

### 8.6 `storage_projection` (UV:17379-17703)
**Purpose.** Classify what a path points at in storage: an endpoint type, a collection window, or a byte field.
- `StructuralStorageProjection { Endpoint(type), CollectionWindow { backing, offset, length }, ByteField { carrier, predicates } }` (`UV:17581`) with `matches` (`UV:17595`), `structural_storage_projection` (`UV:17628`), `structural_borrowed_storage_projection` (`UV:17651`), `byte_field` (`UV:17671`), `shared_view_declaration` (`UV:17575`).
- Tests inside the file (three `#[test]`s, `UV:17505`, `17535`, `17561`) cover byte-field naming, case payload byte field, and declared owner carrier.
- **How it decides.** Path ending in `FixedRange` gives `CollectionWindow` through `terminal_semantics::fixed_collection_storage_window` (`UV:17633-17643`); else `Endpoint` from `structural_projection_type`, else `byte_field` (a trailing `Field`, optionally under a `Case`, naming a live field with a byte-sequence carrier) (`UV:17644-17648`, `17671-17702`). `matches`: `Endpoint` vs `Endpoint` and window backing use `borrowed_referent_matches`; windows also need equal offset and length; `ByteField` needs exact equality; mixed kinds are `false` (`UV:17600-17624`).
- **Unused or one-caller code.** `shared_view_declaration`: 7 call sites (`UV:6113, 6127, 6153, 9532, 11319`; `current_ownership.rs:1561`), re-exported `UV:18173`. `structural_storage_projection`/`structural_borrowed_storage_projection`/`StructuralStorageProjection`: `lib.rs` has 3 hits; the repo-wide count of the three names across `omega-rust` `*.rs` is 85 hits in 12 files (count mode, includes `reference_projection` which was in the same pattern); I did not split by name.
- **Fail-open.** None seen.

### 8.7 `type_declarations` (UV:17704-17750, range ends mid-function)
`validate_structural_fields` (`UV:17709`): fields strictly ascending by id (`UV:17715-17722`); identity `ABSENT`/duplicate rejected (`UV:17725-17734`); the visible part of the field-type match starts at `UV:17740` with an `Atomic`/`AtomicBoundedInteger` arm that errors with `InvalidStructuralFieldIdentity` when `atomic_scalar_type()` is `None` (`UV:17741-17750`), again an identity error name for a non-identity problem. The rest of this function lies beyond line 17750 (outside this range, not read by me).

---

## Coverage

- Read in full: `unit_validation.rs` 11751-17750 (6000 lines).
- Read for context only (not counted): 11735-11750.
- Skimmed: none. Not read: none.
- Greps run (all with the Grep tool, `-w`-style word-boundary regex over `omega-rust` `*.rs` unless noted): call sites of `operation_scalar_types_match|operation_service_contract_matches|operation_structural_call_contract_matches|structural_arguments_match_at_invocation`; `refresh_root_service_reach|valid_service_ceiling|boundary_structural_signature_matches|validate_provider_service_refinements|validate_provider_attachment_specialization|byte_carrier_leaf_window|resolve_leaf_copy_path|shared_view_declaration|structural_root_key|ValidatorStructuralRootKey`; `leaf_paths|leaf_referent|leaf_access_within|projected_carrier_type|is_reference_projection|reference_source_accessible|formal_origin|formal_reference_source|access_reference_source_compatible|record_field_destination` (first 80 hits only for the repo-wide listing; used for the sites cited, not for counts); `reference_projection`; `validate_values_and_bindings|owned_bounded_owner_loan|validate_content_projection_*|validate_function_references|runtime_evidence::validate|fn call_machine_references`; count of `structural_source_contract(` in the crate `src`; count of `InvalidStructuralTypeIdentity|InvalidStructuralDomainIdentity` in `UV` (9).

## Five things a maintainer most needs to know about this range

1. `structural_arguments_match_at_invocation` (`UV:13205-13824`) is one function of about 620 lines that decides every structural call argument. It derives about twenty "presentation" booleans and combines them; the path-shape rule is strict only for `StructuralProjectionPolicy::Unit` and unrestricted for `Projected` and `Boundary` (`UV:13422-13449`).
2. `StructuralSourceContract` / `structural_source_contract` (`UV:12948`, `13928`) is the crate-wide "what is this place" lookup (115 occurrences of the call across `current_ownership.rs`, `descriptor_results.rs`, `lib.rs`, `unit_validation.rs`); its lookup order is arrival parameter, first producing node, trivial affine local, byte literal, and the node search does not check producer uniqueness.
3. `operation_scalar_types_match` has no wildcard arm, so a new operation variant must be added there; but about twenty variants are accepted unconditionally (`UV:11856, 11896, 11927, 12055-12072, 12085, 12106, 12225`), so scalar typing for those rests on other checks.
4. Several error names do not describe the failure: `InvalidStructuralTypeIdentity` / `InvalidStructuralDomainIdentity` are returned for reference admissibility, atomic cells and domain predicate problems (`UV:16249-16254, 16341-16360, 16392-16470`); `MissingEntryMachine` for any missing callee in root-reach DFS (`UV:15955`); `InvalidStructuralFieldIdentity` for an unsupported atomic field (`UV:17745-17750`).
5. `validate_provider_attachment_specialization` (`UV:17357`) rejects every function that has a `ProviderAttachment` place, while `function_catalog` accepts such places (`UV:16765`); and `refresh_root_service_reach` (`UV:16024`, one call at `UV:18223`) recomputes the declared root reach from the unit before it is compared.

## What I am unsure about

- Whether the unconditional `=> true` arms in `operation_scalar_types_match` are covered by checks elsewhere (I did not read `AbstractOperation`'s definitions or `node_contracts`, `UV:10410-10502`).
- Whether `call_machine_references` can return an empty set for a call (affects `UV:12437-12442`).
- The order in which `revalidate_rewrite_output` runs `refresh_root_service_reach` and validation (only the grep line `UV:18223` seen).
- Whether the same-named `leaf_paths`/`leaf_referent`/`reference_source_accessible` in `pipeline/02` and `pipeline/03` are real duplicates of the `references` module (only line numbers seen via grep).
- Per-name call counts of `storage_projection` and `is_reference_projection` inside `current_ownership.rs` (only a combined count of 6 for six names there).
- The grep listing for `leaf_paths|leaf_referent|...` was capped at 80 lines, so call-site counts for those names are not given.
