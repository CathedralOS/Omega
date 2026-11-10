# Unit 59 notes

Files covered by the unit file (all under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`):
1. `execution_terminal_unit_structural_scalar_store.rs` (1-3826) — read in full
2. `execution_terminal_unit_types.rs` (1-5353) — read in full
3. `flow.rs` (1-11937) — NOT READ (see Coverage)

Abbreviations: SSS = structural_scalar_store.rs, TYPES = execution_terminal_unit_types.rs.

---

## 1. SSS — execution_terminal_unit_structural_scalar_store.rs (3826 lines, read in full)

### Purpose
Plans the checked "Store" operation for one authored assignment statement `place = value` whose destination is parameter or local storage. It resolves a destination (root, write authority, carrier path), then lets the leaf's declared type pick which `CheckedStoreLeaf` kind the store gets, and pairs it with an already-checked value source (header comment 1-39). Whole-record assignments and primitive-array literals are explicitly not handled here (they stay in `control::statement_sequence`, 27-30, 18-19).

### Shape (file:line)
- Top-level types: `Occurrence` (2921, fields program/facts/machine/state/statement_index/assignment/source/trace), `Site` (2938, an Occurrence plus `&Destination`).
- Inline modules: `byte_stores` (65-650), `descriptor_store` (652-828), `destination` (829-1765), `element_stores` (1766-1891), `primitive` (1892-2360), `selectors` (2361-2558), `value` (2559-2755), `view_stores` (2756-2913).
- `destination`: `StoreRoots` (856: structural / scalar / storage slices), `Root` enum Parameter|Local (863), `Destination` (900: root, place, dynamic_index, recast_view, loan_type, selectors, root_type, root_owner), `Carrier` (931), `StorageProjection` (937), `resolve` (956), methods `exclusive_authority` (1209), `leaf_carrier` (1269), `storage_projection` (1283), `walk` (1306).
- `value`: `AssignmentSource` enum Authored | StructuralValue{binding_ordinal,result_type} | CallResult{position,primitive_type} (2580-2596); `assignment_value` (2602), `authored_value` (2656), `admits` (2704), `runtime_index` (2730), `byte_value` (2750).
- Entry functions: `build_structural_scalar_field_store_sequence_traced` (3017), `build_structural_call_result_field_store` (3097), `build_completed_byte_value_store` (3133), `scalar_capture_destination` (2949), `plan_assignment` (3275, the dispatcher), `single_parameter_store` (3796), `runtime_index_expression` (3181).
- Leaf families emitted: `ByteElement` (byte_stores::byte_write 96), `ByteContents` (whole_store 158, field_store 237), `ByteDescriptor` (descriptor_store 727), `ViewElement` (element_stores::store 1781), `Primitive` (primitive::store_at 1961), `RepresentationView` (view_stores::plan 2792), `ScalarField` (3446), `SumField` (sum_field_store 3716; fed by `case_field_store` 3500 and `case_literal_field_store` 3602).

### Inputs / outputs
Reads: `CheckingInput` program tables (expression/statement/type-reference tables), `CheckFacts` (`facts.values.scalar_computations`, `facts.values.scalar_expressions`, `facts.values.structural_values`, `facts.proof.integer_assignment_at`, `facts.borrow`, `facts.operators`), the parameter plans (`StoreRoots`). Produces `Vec<CheckedUnitEffectOperationPlan>` (always `new_store(...)`), or `None`. `ShapeCollector::add_stored_type` is mutated by `scalar_capture_destination` (3008).

### How it decides
- Dispatch order in `plan_assignment` (3275-3471): (1) refuse if a reference value is installed into a whole root (3286-3297); (2) resolve destination; (3) element-view path -> `element_stores` (3321-3332); (4) slice/bounded-byte root with dynamic index -> `byte_write` (3333-3355); (5) whole bounded-owned byte root -> `whole_store` (3356-3361); (6) `exclusive_authority` check (3362); (7) `recast_view` -> `view_stores` (3363); (8) primitive leaf -> `primitive::store_at` (3367); (9) field-keyed: byte field, shared view field (descriptor), primitive field, sum field (3370-3470).
- Keyed on identity/place, not names: roots matched by `parameter.symbol == root_symbol` (1033), fields by `SymbolHandle` (`exact_relevant_field` 1529), cases by `variant` symbol. Domain checks use `domain_by_symbol`/semantic ids. Not spelling-keyed, except `PrimitiveType` equality.
- Value source is selected in exactly three places ("computation root, bound pure expression, call result" 2561-2570). A computation present but not matching is "a custody mismatch, not permission to fall back" (2651-2654, enforced 2669-2674).
- Float stores: `admits` (2704) only allows IEEE literal / parameter / local / structural field/indexed read as pure values; a computation or call result is always admitted for F32/F64 (2720-2721).
- Destination write authority: `exclusive_authority` (1209) demands every non-erased parameter has exactly one plan row (1243-1249), `!parameter.is_const` (1250), and plan access equal to the declared reference shell access (1253-1261). Comment 1204-1208: a second binding-mutability test "would reject writes through references held inside an owned parameter".

### Cases by shape (fixed lists that refuse the rest)
- `store_width` (2904-2912): only Bool/U8/I8=1, U16/I16=2, U32/I32=4, U64/I64=8; anything else (floats, usize etc.) returns None for representation-view stores.
- `view_stores::plan` (2792): leaf must be a primitive Field, a `FixedIndex`, or `Index` of a bounded-owned `u8` field/`[u8; N]`; a `Computation` store value is refused (2849); comment 2784-2791: borrowed view "stays refused".
- `byte_value_plan` (378): string literal, builtin `+` concatenation, or a place (whole or field); anything else returns None; depth cap 128 (391).
- `case_field_store` (3500): only an unrestricted payload-free enum field overwritten by a whole parameter (or member below one) of the same named type, access Owned/SharedBorrow, no qualifications. Comment 3493-3498: "call results, payload sums, records and affine carriers keep declining."
- `case_literal_field_store` (3602): only `closed_sum`, no omitted fields (3626), every payload scalar primitive (3654).
- `walk` (1306-1458): path segments only Field/Case/FixedIndex/Index; array carriers only `FixedArray` and `Slice`; reference crossings only exclusive (1438); a shared slice view is not walked into.
- `TargetSelectors::resolve` (2401-2495): only fixed-array or slice carriers (2427-2434), only integer literal or retained runtime selectors (2436-2452).
- Primitive leaf: `scalar_expression_needs_no_bindings` (2302-2329) and `boolean_expression_needs_no_bindings` (2331-2359) list which scalar/boolean node kinds are accepted (parameters, locals, literals, ImplementationSelection, binary, widen, RepresentationView, boolean not/and/or/comparisons); StorageRead, ErasedParameter, ConstantApplication, StructuralSequenceLength and structural reads return false.
- `primitive_leaf_type_reference` (2157): peels only ArithmeticDomain-only `Constrained` shells and non-shared references.

### Same job elsewhere
- Scalar-binding exactness: `scalar_custody_is_exact` (2226) and `scalar_store_value_is_exact` (2267) are two variants of one check (both call `scalar_binding_symbols_exact` 2203 then re-lower via `lower_unit_scalar_argument`); the second tolerates a binding that cannot re-lower, the first does not (2281-2290 vs 2245-2254).
- `descriptor_store::descriptor_store` (727) and `byte_stores::byte_value_plan` (378) both resolve a byte operand; the former delegates to `byte_operands::plan` (787), the latter implements its own place walk (suspected duplicate of `byte_operands`; not read).
- Place -> carrier path walking: `destination::Destination::walk` (1306) vs `byte_value_plan` (552-563, own loop mapping `PlaceSegment` -> `Segment`) vs `ShapeCollector`/`projected_argument_path` (`calls`, used at 3539; not read).
- Root resolution has three sibling constructors: parameter plan (1031-1091), `local_destination` via storage rows (1036-1050), `exclusive_reference_local_root` (1717), `owned_local_root` (1129).

### Unused / one-caller
- Grep (count mode) for `build_structural_scalar_field_store_sequence_traced|build_structural_call_result_field_store|build_completed_byte_value_store|scalar_capture_destination|byte_value_plan|scalar_custody_is_exact|primitive_local_before|is_shared_view_field` across `omega-rust`: 37 occurrences in 4 files (SSS 18, control_statement_sequence 8, calls 7, execution_terminal_unit 4). Per-name caller counts not separated; none appears to have zero outside users except those that are intra-file.
- `plain_record` (1476, imported in `use destination::{..., plain_record}` at 58): used at 3461 in this file. `crosses_reference` (1516) used at 3425. `exact_relevant_field` (1529) has `pub(super)` visibility; I did not grep external users.
- No claim here is stated "single/canonical" except `Occurrence` doc (2915-2919: "one owner carries it instead") — a local struct, used throughout this file.

### Stated limits
- "What remains refused is a custody or proof condition, not an arrangement" (36-38).
- Whole record assignment "stays with ordinary structural construction ... This planner does not decompose constructors" (27-30).
- Views wider than leaf: "a borrowed view's live extent has no declared footprint bound ... so it stays refused" (2786-2791).
- Selectors: "The planner proves no bound for a runtime element: Terminal re-proves `index < extent`" (2370-2371).
- Floating operation in a pure expression "has no selected provider, so it is refused" (2698-2699).

### Fail-open risks
- `destination::resolve` swallows a failed `receiver_aliases::aliases` with `unwrap_or_default()` (1006-1007) and keeps the unresolved `place` on `resolve` failure `unwrap_or(place)` (1008-1009): treats an unresolved alias as "no alias". "open": I did not read `receiver_aliases`; a write through an alias that fails to resolve is planned against the authored place.
- `retained` (2529-2557): a selector with neither a bound expression nor a computation root returns `None`, fail-closed. none seen.
- `build_structural_scalar_field_store_sequence_traced` (3061-3088): when `plan_assignment` returns None for an assignment whose value is a Call or has a structural value root, the assignment is skipped (`continue`, 3084-3086) rather than failing; the skipped statement gets no store from this builder. "stated" by code shape (the call-result and completed-value builders, 3097/3133, are the other lanes), but whether those lanes always pick it up is "open" (not traced).
- Statements where `validation::atomic_assignment_carrier(...)` is Some or `assignment_type(...)` is Some are skipped (3051-3056); other owners are assumed to handle them. "open".
- `primitive::store_at` for `RefinedScalar` leaves returns `receiving_range = None` (2011), i.e. no numeric receipt; comment 1898-1899 says "general scalar predicates use the same completed source-check cut without manufacturing a numeric receipt". "stated".

---

## 2. TYPES — execution_terminal_unit_types.rs (5353 lines, read in full)

### Purpose
Home of the structural-type collector (`ShapeCollector`) that turns type references into the `CheckedUnitStructuralTypePlan` catalog (types, domains, opaque representations, cleanup arenas) for a program, plus helpers for scalar field restrictions, type identities, parameter rosters, signature-contract domain requirements, borrowed-view classification, affine return discards and layout-query subjects (header 1-3).

### Shape (file:line) — inline modules and their main items
- `call_qualifications` (21-113): `retain_call_qualification_domains` (31).
- `declared_result` (115-249): `declared_result_plan` (132), `declared_result_type` (149).
- `domain_predicates` (251-441): `retain_domain_predicates` (260); nested `complete` (339) and `value_predicate` (384).
- `scalar_fields` (443-707): `field_requires_place_observation` (465), `supports_primitive_storage` (507), `stated_field_interval` (527), `retain_scalar_field` (558).
- `affine_discards` (711-974): `return_unit_affine_discards` (727), `projected_type` (920).
- `boundary_domains` (975-1183): `BoundaryDomainRequirements` (996), `boundary_domain_requirements` (1002).
- `identities` (1184-1446): `terminal_field_identity` (1192), `terminal_field_declaration` (1205), `structural_access_for_type_reference` (1242), `byte_sequence_type_identity` (1258), `base_type_identity` (1317), `lent_storage_identity` (1350), `substitution_for` (1366), `substituted_formal_type` (1389), `closed_data_identity` (1410), `scalar_type` (1426).
- `layout_observations` (1447-1600): `retain_layout_observation_types` (1461) with a `TypedReferences` visitor `QuerySubjects` (1502).
- `parameters` (1602-1841): `state_flow` (1617), `parameter_is_root` (1640), `machine_binders` (1644), `strips_erased_parameter` (1666), `parameter_storage_plans` (1680), `erased_scalar_parameter_plans` (1720), `erased_proof_parameter_plans` (1757), `dense_structural_parameter_index` (1799), `dense_scalar_parameter_index` (1826).
- `shape_collector` (1842-4338): sub-module `forks` (1871-2517): `ConsultedMap` (1913), `ConsultedSet` (2144), `RowMarks` (2172), `ShapeEdits` (2182), `CompletedRequests` (2208), `ChangeLog` (2300), `CopyOnWrite` (2491), and `ShapeCollector::{prepare_forks 2361, fork 2366, row_marks 2379, edits_since 2405, apply 2460}`. Then `ShapeCollector` struct (2574; fields program, types: TypeCatalog, domains, opaque_representations, cleanup_control, completed_requests, in_progress, domains_in_progress, consulted_domains, byte_view_elements, unselected_opaque, canonical_carriers), `carrier_index` (2622), methods `add_domain` (2705), `add_domain_application` (2737), `add_selected_domain` (2788), `add_domain_definition` (2811), `add_attached_data` (2901), `add_accessor_attachment` (2931), `add_reference_type` (3015), `add_view_type` (3033), `add_reference_shape` (3096), `add_reference_backing` (3147), `retain_reference_contract_types` (3168), `add_calling_parameter` (3211), `add_stored_type` (3266), `add_logical_type` (3325), `add_type` (3347), `collect_type` (3372), `add_data_shape` (3722), `collect_data_shape` (3741), `structural_field_plan` (4102), `retain_transitive` (4233), `authored_field_storage` (4317).
- `signature_contracts` (4339-5250): `checked_machine_signature_has_plan` (4357), `checked_state_contracts_have_plan` (4409), `checked_structural_signature_has_plan` (4428), `signature_progress_requirement` (4490), `parameter_qualifications` (4518), `qualification_carriers` (4538), `fold_boundary_result_domains` (4622), `normal_mutable_parameter_position` (4697), `register_authored_result_domains` (4723), `fold_parameter_entry_domains` (4786), `retain_parameter_arrival_domains` (4880), `body_structural_qualifications` (5047), `projected_parameter_qualifications` (5161), `projected_result_qualifications` (5182), `type_domain_semantic_id` (5199).
- `views` (5252-5346): `element_view_shell` (5271), `borrowed_slice_view_element` (5282), `borrowed_slice_view` (5295), `borrowed_slice_result` (5309), `slice_view_carrier` (5320), `borrowed_named_view` (5337), re-export `byte_sequence_carrier` (5345).

### Inputs / outputs
Reads the typed program through `validation::CheckingInput` (type-reference table, data definitions, domain definitions, `semantic_domains`, `access_view_plans`, `plan_laid_layouts`, `opaque_representation_selections`), `CheckFacts` (`facts.flow.ownership`, `facts.semantic`, `facts.qualifications`, `facts.contract_plans`, `facts.proof`). Mutates the `ShapeCollector` catalogs; returns plan structs (`CheckedControlResultPlan`, `CheckedUnitStructuralParameterPlan` fields, `BoundaryDomainRequirements`, discard rows).

### How it decides
- Type shape registration is keyed on `TypeIdentityId` (from `structural_type_identity`), one shape per identity; a different plan for a known identity is refused (`register_type` 2692-2703; same in 2992-2999, 3482 returns an existing row without comparing).
- `add_type` memoizes on exact `(reference, binders, substitutions)` request (2566-2571, 3354-3361) and then looks at `self.types.contains_key(&identity)`.
- `collect_type` branch order (3372-3691): reference shell -> byte-sequence carrier -> borrowed slice view -> primitive -> fixed array -> closed data -> named/generic data lookup. Primitive vs data decided by `PrimitiveType::from_name(name.as_str())` on the type reference name (3638) and in `scalar_type` (1437) — a spelling-keyed primitive test.
- `retain_scalar_field` (558): walks `Constrained` shells, intersects domain-derived integer intervals (`exact_declared_domain_carrier_interval`), ANDs scalar domain predicates, rejects any `Named` constraint (621-623), rejects `has_non_exact_domain` combined with bounds/predicate (658-660), atomic handling (638-657).
- `return_unit_affine_discards` (727): matches owned structural parameters; keys off `selected_sources` by symbol, `parameter_vacancies`, event fields `access == Owned && multiplicity == Affine && claim_identity == Unknown && provenance == Unknown && !obligation_live && segments empty` (856-868).
- Boundary domain requirements (1002): keyed by `membership.semantic_domain` (id), `domain_symbol` and the source parameter position found by `single_segment_name_symbol` (1063).
- Domain-plan lookup in `retain_domain_predicates` (266-282): finds the definition with `semantic_id == plan.domain`; if two match or it has an alias, `continue` leaving the plan's `predicate` as it was.

### Cases by shape
- `declared_result_type` (149-248): accepts Unit, primitive (if `scalar_result_contract_is_represented`), or structural when one of `view_result`, `represented_copy`, `owned_bytes`, `claim_bearing`, reference record, BorrowFree or OwnedOrSharedView holds; otherwise None (197-220). A non-view reference result is refused (197).
- `ShapeCollector::collect_type` fixed-array length: Literal, ConstParameter via substitution, Expression, ConstCall recipe (3551-3566).
- `collect_data_shape` accepts only `DataSupplyMode::CheckedShape` with `shape_kind_from_members` in Empty/Record/Enum/Mixed (3866-3877; same at 3659-3669). `LayoutTarget` core layout declarations are refused (3779-3785).
- `retain_scalar_field` atomic: "`predicate.is_some() || has_non_exact_domain` -> None" (651-653).
- `store_width`-style fixed list: not in this file. `qualification_carriers` rejects constraint kinds other than ArithmeticDomain/Domain (4561-4574).
- `erased_scalar_parameter_plans` (1720): erased scalar formals must have an `erased_scalar_observation_type`; erased `const`/mutable bindings refuse the plan (1666-1671).

### Same job elsewhere
- `retain_domain_predicates` `complete` (339) and `value_predicate` (384) both walk `required_memberships` parent plans with identical lookup (match by `domain` and `carrier_type_identity`, refuse if ambiguous): 355-361 vs 403-410.
- Shape-struct literal `CheckedUnitStructuralTypePlan { descriptor_presentation: Default::default(), source_declaration..., native_* None ... }` is spelled out 9 times (2974, 3116, 3433, 3460, 3524, 3598, 3789, 3941, 4063), counted by reading; a constructor helper does not exist in this file.
- `byte_sequence_carrier` classification is repeated in `qualification_carriers` (4544-4551, 4578-4581), SSS (byte_stores, 127, 198, 3379) and `collect_type` (3408).
- Domain-lookup by `domain_definitions().iter().find(semantic_id == ...)` appears in `add_domain` (2722-2727) and `retain_domain_predicates` (266-276, which also chains `applied_domain_applications`) — the latter also considers applications, the former does not.
- Scalar `[erased]` plans: `erased_scalar_parameter_plans` (1720) and `erased_proof_parameter_plans` (1757) are the same filter split by `contract_term_carrier || is_source_evidence_carrier`.
- Dense parameter indexing: `dense_structural_parameter_index` (1799) and `dense_scalar_parameter_index` (1826) vs SSS `runtime_index_expression` `Parameter` arm (3219-3229, own dense walk via `occupies_scalar_position`).

### Unused / one-caller
- Count-mode grep over `omega-rust` for `retain_call_qualification_domains|retain_domain_predicates|retain_layout_observation_types|return_unit_affine_discards|boundary_domain_requirements|fold_parameter_entry_domains|retain_parameter_arrival_domains|body_structural_qualifications|checked_machine_signature_has_plan|checked_state_contracts_have_plan|fold_boundary_result_domains|register_authored_result_domains|add_affine_array_construction_type|carrier_index\(|prepare_forks`: 51 occurrences: TYPES 23 (definitions+re-exports), execution_terminal_unit.rs 5, state_graph 2, scalar_computations 6, control 11, calls 4. Every name has at least one non-TYPES use unless noted below; per-name counts not broken out.
- `add_affine_array_construction_type` (3693): body is `self.add_type(type_reference, binders, &[])`, a pure alias. Its count is included in the grep above; not separated, so I cannot say how many callers (open).
- `ConsultedMap|ConsultedSet|ChangeLog` count-mode grep: 18 occurrences in 2 files (TYPES 16, execution_terminal_unit.rs 2). So the fork machinery is used by one outside file.
- Comment 3686-3690 / 2613-2619 call `carrier_index` a single index serving "every collector over that same program"; outside-file users: `carrier_index(` occurs in the grep set above; not separately counted.

### Stated limits
- `add_logical_type`/`stores_reference_shape` loop bounded by `type_reference_count()` (3334).
- Comment 1861-1863: descriptor disposal judgment "never permits ordinary resource-free cleanup".
- `unselected_opaque` (2598-2604): collector records the opaque datum "the build selects no representation for"; `add_calling_parameter` only tolerates that when the formal is borrowed (3236).
- `retain_scalar_field` doc on `supports_primitive_storage` (500-505): "A domain kept only in custody (such as Finite) is not an unrestricted store".

### Fail-open risks
- `retain_domain_predicates` (277-281): if the definition is not found, ambiguous, or aliased, `continue` leaves `plan.predicate`/`required_memberships` as constructed (`predicate: None` from `add_domain_definition` 2857 unless provided). Later (373-382) `complete == None` clears them. "traced" partly: a missing definition leaves `complete_predicate_theory` as set by `add_domain_definition` (2851-2853); `complete(..)` returns `Some(false)` when `required_memberships` is None (348-350), so it is marked incomplete, not complete.
- `retain_domain_predicates` membership arm (308-312): a membership proof fact whose domain is in `required_memberships` lowers to `Constant(true)`. This makes the child predicate true on the assumption the parent plan (conjoined later by `value_predicate`) supplies the parent predicate; "stated" implicitly (parent conjunction 401-424).
- `declared_result_type`: `view_result` path uses `.ok()?` so failure refuses. none seen.
- `fold_parameter_entry_domains` (4786-4872): several `continue`s skip a requires-membership silently — missing root symbol (4820-4823), parameter not in structural roster (4830-4833), and when `!routed && !vacuous` (4845-4847: "scalar membership predicates are handled elsewhere"). The skipped membership is not published on the parameter plan. "open": whether the scalar lane always catches it.
- `boundary_domain_requirements` (1093): a scalar membership with no provenance and not vacuous is `continue`d (skipped). "open".
- `fold_boundary_result_domains` (4653-4659): `let Some(domain) = ...domain_definition(...) else { continue }` silently drops an ensures membership whose domain definition is missing. "open".
- `register_authored_result_domains` (4760-4762): `continue` when the domain definition is missing. "open".
- `retain_parameter_arrival_domains` (4932-4958): `continue` when the fact's place does not canonicalize, the root is not a parameter, a primitive carrier (4965-4967), or the membership is already declared. Fail-closed for unsupported exact path (error at 4964).
- `return_unit_affine_discards` (878-886): an Affine exit-cleanup event whose root is neither a structural parameter, an erased formal nor an admitted local returns None (refuse). none seen.
- `retain_call_qualification_domains` (90-97): skips facts whose place type or carrier cannot be resolved or whose carrier is primitive; returns nothing and the domain simply is not retained. Qualification is "catalog retention", not evidence ("it introduces no membership", 25-27). "stated".

---

## 3. flow.rs (11937 lines) — lines 1-1200 read; lines 1201-11937 NOT READ
Only the first 1200 lines were read. Nothing is claimed about 1201-11937 (that range holds the rest of `normal_returns` and the other inline modules/children the header lists: state_values, entry_origins, statements, calls, exits, mutation, transfers, reach, etc.; inferred from header text at flow.rs:6-31 only).

### Purpose (read, lines 1-31 header)
Flow-facts builder: for each state/statement/call/exit of every machine it records the semantic contexts and constraints active there plus invalidation, borrow-lifetime, boundary-edge and control rows (flow.rs:1-4). Single entrance `builder::build_flow_facts` (flow.rs:291), called by `crate::published::facts::build_check_facts` (header 6-8).

### Shape (read)
- `assignment_observations` (37-162): `AssignmentObservations` (68) per-sweep scratch of prepared contexts; `FlowBuildContext::scalar_value_at_place` (147). It re-prepares a context only when the stored handle differs (103); cleared by `discard_output` (stated 47-50).
- `atomic_effects` (163-234, `pub(crate)`): `append` (179) adds an `InstructionEffect::AtomicReleaseStore` fact; `release_store` (218) is keyed on `atomic_assignment_carrier` with operation `Store(Publish | GlobalOrder)` and `place_is_atomic_storage` (225-232).
- `builder` (235-899): `FlowInputs` (280), `MACHINE_PAGE_BASE = 2` (277), `build_flow_facts` (291), `build_groups` (354, rayon `par_iter` per machine group), `build_split_group` (422), `build_group` (506), `converge` (532), `sweep` (600), `machine_groups` (697), `MachineGroup` (887), `replay_guard_holds` (893).
- `completed_values` (900-907): re-exports four functions from `validation::completed_storage` (`predecessors`, `predecessors_in_control`, `predecessors_in_flow`, `terminal_alternatives`).
- `context` module declared via `#[path = "flow_context.rs"]` (908-909; file not in this unit).
- `normal_returns` (910-...): `ReturnedPredicate` (927), `NormalReturnPredicates` (957; `contains` 969, `proves` 984, `replace` 1031), `family_members` (1060), `has_exclusive_parameters` (1080), `dependents` (1091), `refresh` (1124; read to line 1200, then cut off).

### How it decides (read)
- Fixed point: `converge` (532-591) loops up to `pass_limit = 2 + sum over states of (1 + 2*parameter count)` (541-550), extended by `new_state_field_input_height` (569) and the return-predicate height (574). If the bound is exhausted, confirmation fails, or a replay diverges it returns None and `build_group` discards output, sets all inputs to `unknown_inputs`, and does one complete `sweep` (513-524). The unknown-input fallback is the stated safety fallback (flow.rs:11-13).
- Grouping keyed on machine roster position and symbols: lexical parent (717-737), calls found in `borrow.states` whose callee is not a plain public entry (755-766), `target_machine_families` membership by symbol (769-783), exclusive-parameter return flow (751-754, 794-798). Independent parts are groups members never called from another part (846-874).
- `normal_returns::proves` (984-1029) requires every family member to hold the same predicate for the same argument index and `type_reference` (1017); a different telescope supplies no guarantee (stated 981-982).
- `refresh` (1124): publishes only after a complete sweep; missing proof or unknown origin removes the row (stated 1120-1123; code read to 1200 only).

### Cases by shape (read)
- `build_split_group` (422) proceeds only if `group.independent_parts.len() >= 2` (368) and replay of recorded deliveries matches exactly; otherwise it returns None and the serial build is used (491-495).
- `release_store` (218): only `Publish` and `GlobalOrder` store orderings count as release stores (226-230).

### Fail-open risks (flow.rs 1-1200 only)
- `normal_returns::refresh` (1141-1149): machines with no body, no exits or no exclusive entry parameter are skipped (no row published) — fail-closed since a missing row means "nothing proved" (`proves` returns false when `contains` fails, 984-1029). none seen in the lines read.
- `machine_groups` (742-748): a borrow-call row whose caller or callee symbol does not resolve is `continue`d, so no grouping edge is added; a missing edge could place dependent machines in separate groups. "open" (not traced; the later `ReplayGuard` check, 893-898, covers only replayed parts).

### Unused / one-caller
- Not measured (no greps run for flow.rs).

---

## Coverage
- Read in full: SSS lines 1-3826 (3826 lines); TYPES lines 1-5353 (5353 lines); flow.rs lines 1-1200 (1200 lines). Total read: 10379 lines.
- Skimmed: none.
- Not read: `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs` lines 1201-11937 (10737 lines). Reason: the unit (21116 lines) exceeded the reading capacity available to this reader.

## Five things a maintainer most needs to know about this unit (files 1 and 2 only)
1. SSS `plan_assignment` (SSS:3275-3471) is an ordered dispatcher: the order of the checks decides which `CheckedStoreLeaf` a store gets, and several arms return `None` without trying later arms (e.g. SSS:3354 byte write, 3331 element store, 3365 representation view). Reordering changes accepted programs.
2. Value sources are chosen in exactly three places (computation root, bound pure expression, call result), and a computation that exists but mismatches deliberately blocks the fallback (SSS:2651-2681, 2097-2120). Float stores are narrower than integer stores (SSS:2704-2723).
3. `build_structural_scalar_field_store_sequence_traced` silently skips (restores the trace and `continue`s) any assignment that fails planning but has a Call value or a structural value root (SSS:3075-3086); other builders (call-result 3097, completed byte value 3133) must pick those up.
4. `ShapeCollector` (TYPES:2574) registers exactly one shape per `TypeIdentityId`, memoizes completed requests per type-reference arena slot (TYPES:2208-2296), and is forked per checked body with consulted/written-row tracking and an ordered replay/`admits` merge (TYPES:1871-2517). Any new catalog field must be added to `ShapeEdits`/`edits_since`/`apply` or fork replay will drop it.
5. Domain handling is split between three entry points with different lookup rules: `add_domain` (by constraint on the carrier, TYPES:2705), `add_domain_application` (typed application with arguments, 2737), and `add_selected_domain` (declaration chosen by a contract, 2788); `retain_domain_predicates` (260) later fills predicates from `proof_facts` and closes memberships transitively (339-440).

## What you are unsure about
- Whether `receiver_aliases::resolve` failing (SSS:1008) can reach a write with a wrong place; not traced.
- Caller counts per exported name (my greps were combined counts, not per-name).
- Whether `byte_value_plan`'s own place walk (SSS:552-563) duplicates `calls::projected_argument_path`; I did not read `calls`.
- Everything about `flow.rs`.
