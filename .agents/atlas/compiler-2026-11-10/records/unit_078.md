# Unit 78 notes

Files covered (all under `omega-rust/psi/compilation/representations/`):
1. `checked-trees/src/facts.rs` (3632 lines)
2. `checked-trees/src/flow.rs` (2327)
3. `checked-trees/src/flow_ownership_and_roots.rs` (2104)
4. `checked-trees/src/flow_terminal.rs` (2665)
5. `checked-trees/src/flow_terminal_units.rs` (2634)
6. `checked-trees/src/lib.rs` (801)
7. `checked-trees/src/operators_and_proof.rs` (2826)
8. `checked-trees/src/providers_and_trust.rs` (897)
9. `checked-trees/src/termination_and_cleanup_facts.rs` (906)
10. `checked-trees/src/values_and_views.rs` (2803)
11. `lowered-psi/src/lib.rs` (266)
12. `symbol-resolved-trees/examples/diagnostic_name_allocations.rs` (50)

Everything below was "read" (every line, in order) unless marked "grepped" or "inferred". Paths in citations are relative to the crate `src/` (or the crate root for lowered-psi / the example) unless a full path is given. Most of this unit is data declarations (structs, enums, arenas). The code that makes judgments is a minority, and I list it explicitly.

---------------------------------------------------------------------
## 6. checked-trees/src/lib.rs (801 lines)

**Purpose.** Crate root of `checked_trees`: it declares the module layout, re-exports about 500 names, defines `CheckedTrees` (typed program + `CheckFacts`), the generic `KeyedRows` index and the `shifted_rows` helpers, and one test that forbids string identity fields.

**Shape.**
- `CheckedTrees { typed: FrozenTypedTrees, facts: CheckFacts }` at lib.rs:689-693. `Deref` to `TypedTrees` (lib.rs:780). Helper methods: `body_crash_summary` (703), `landed_integer_constant` (729), `consistent_terminal_origin` (745), `authored_selection_occurrences` (769).
- `callback_declarations` module (32-95): `CheckedTrees::callback_declaration` (39) and private `callback_registration_declaration` (57).
- `keyed_rows` module (174-423): trait `DurableRows` (179), impls for `Arena<T>` (201) and `PagedArenaSet<T>` (223), `KeyedRows<Key,Row>` (258) with `with_groups` (263), `seal` (309), `with_rows` (317).
- `shifted_rows` module (426-466): `shiftable` (429), `shift` (437), `shift_span` (450), `append_rows` (457).
- `statement` module (467-495): re-exports plus two local enums `TransitionGuard` (479) and `TransitionTarget` (485).
- `source_guards` test module (98-151) under `#[cfg(test)]`.

**Inputs/outputs.** `KeyedRows::with_groups` reads an arena through `DurableRows` (revision + optional append epoch) and a `key_of` closure and fills a cache inside an `OwnedIndex`; the cache is rebuilt when `arena.revision()` changes, extended in place when only rows were appended (lib.rs:276-288 compares `(rewrite, len)` epochs), else rebuilt (289-294). `shiftable` returns the number of existing rows only when both arenas have an append epoch and the same `fresh_generation` (429-435).

**How it decides.**
- `callback_registration_declaration` (57-93) chooses `CheckedSource` vs `External(identity)` by symbol lookups: `trait_machine_signature(operation)` and `owner.is_boundary` (63-67), `machine_by_own_or_state_symbol` and `supplies_checked_body()` (70-73), then `machine_parameter_signature` or a non-boundary requirement trait (80-85). If none match it returns `Err("nominal machine use lost its exact registration declaration: ...")` using `display_path` only in the message (87-90).
- `body_crash_summary` (703-727): only answers when exactly one machine body and exactly one contract plan have `symbol == machine` (keyed on symbol identity); otherwise `None`.
- `consistent_terminal_origin` (745-767): follows `NestedExpression{parent}` chains, returns `None` on a cycle (`parents.contains`, 751) or if the parent's value rows disagree on origin (761).
- `source_guards::checked_trees_carry_no_string_identity_fields` (131-150): greps source text for the literal substrings `identity: String`, `identity: Option<String>`, `identity: Vec<String>`, `identities: Vec<String>`, `identities: HashSet<String>`, `identities: BTreeSet<String>`; allowlist at lines 104-129 (six entries: `facts.rs registrar_application_identity: String`, `flow_terminal.rs requirement_identity/native_identity/native_policy_identity/native_semantic_identity`, `operators_and_proof.rs schema_identity`). The guard is spelling-based: fields that carry identity text under other names are not caught (see below).

**Cases by shape.** None as an explicit list. `shifted_rows::shift` (437-448) only shifts valid handles and panics with `expect("appended rows exceed the handle range")` on overflow.

**Same job elsewhere.**
- Per-key grouping caches: `KeyedRows` (this file) is re-implemented by hand in `operators_and_proof.rs:956-998` (`ExpressionUseIndex` inside `OwnedIndex`), `values_and_views.rs:2146-2174` (`SealedExpressionStateIndex`), `flow.rs:417-566` (`FlowDescriptorObservationTable.by_context`), `values_and_views.rs:1029` (`schedules_by_value_slot: Vec<Vec<Handle<..>>>`), `facts.rs:1300-1303` (`CheckedDeclarationDescriptions.machine_index`), `facts.rs:3417-3480` (`StateLocalFacts.spans`). Grepped: `KeyedRows` has 56 occurrences in 9 files (`grep KeyedRows omega-rust`, count mode); outside checked-trees it is used only in `05_typed-trees-to-checked-trees/src/checks_content.rs` (1) and `execution_terminal_unit_state_graph.rs` (5).

**Unused / one-caller.** `shifted_rows::{shiftable,shift,shift_span,append_rows}` are `pub(crate)` and only used inside `facts.rs` (9 hits) and `termination_and_cleanup_facts.rs` (9 hits) (grep `shifted_rows::|shiftable\(`, count). The only external callers of the two functions built on them are three sites in `05_typed-trees-to-checked-trees/src/checks_multiplicity_permission_events.rs:597,598,608`.
Comment-says-shared check: `KeyedRows` carries no "single/canonical" comment.

**Stated limits.** None in this file.

**Fail-open risks.**
- `KeyedRows::eq` always returns `true` (361-365) and `WalkGuard::eq` too (flow_ownership_and_roots.rs:113). Two `CheckedTrees` that differ only in cached groups compare equal. This affects equality, not acceptance. Traced: the cache is derived data.
- `KeyedRows::with_rows` returns an empty slice for a missing key (328): callers that treat "no rows" as "nothing to check" accept. Open (depends on callers).
- Otherwise none seen.

---------------------------------------------------------------------
## 11. lowered-psi/src/lib.rs (266 lines)

**Purpose.** Defines `LoweredPsi`, the unsealed program carrier between Psi 06, 07 and 08, plus "source custody" side tables that let later stages map Terminal Psi call sites and comparison operands back to checked source.

**Shape.**
- `LoweredPsi` (233-248): `semantic_module: TerminalModule`, `source_records`, `source_boundary_declarations`, `source_machine_declarations`, `source_proof_declarations`, `build_provider_requirements`, `retained_roots`, `debug_map`, `source_call_occurrences`, `selected_comparison_occurrences`, `state_blocks`. `into_parts` (251-265) returns `(module, debug_map, LoweredSourceCustody)` and drops the rest.
- `source_custody` module (23-209): `LoweredStatementCallCoordinate` (29), `LoweredSourceCallOrigin` (36; `Statement` or `Declaration{state, application, call, observation_block, observation}`), `LoweredSourceCallOccurrence` (195), `LoweredSelectedComparisonOccurrence` (98), `LoweredSelectedComparisonOperandOrder {Authored, Swapped}` (108), `LoweredSourceFrontierBindings` (160), `LoweredStateBlock` (204).
- `LoweredBuildProviderRequirement` (219), `LoweredSourceCustody` (228, one field).

**Inputs/outputs.** Pure data plus three methods; no tables read.

**How it decides.**
- `matches_statement` (70-78): equality on `(state symbol, statement_index, call_ordinal)`; `Declaration` origins never match.
- `matches_checked_origin` (80-94): first `checked.consistent_terminal_origin(origin)`; then requires the occurrence to be `Statement` origin and the origin to be `StateStatement{state_symbol, statement_index}` with the same state and statement index. Call ordinal is not compared; `Declaration` origins return `false`.
- `terminal_operand_position` (114-135): `Authored` maps ordinal to itself if `< operand_count`; `Swapped` is defined only for `operand_count == 2 && ordinal < 2`, giving `1 - ordinal`.

**Cases by shape.** `Swapped` handles exactly two operands (line 128): "operand_count == 2 && authored_ordinal < 2". All other counts return `None`.

**Same job elsewhere.** The `(state, statement_index, call_ordinal)` coordinate appears as `FlowCallFact{statement_index,call_ordinal}` (flow.rs, control module 248-250), `CheckedUnitCallCoordinate` (flow_terminal.rs:1247), `ContractCallFact` (operators_and_proof.rs:1729) and `SuspensionCrossingCarryFact` (facts.rs:2730). Grepped but only partly checked: same triple spelled in at least five structs.

**Unused / one-caller (grepped, `omega-rust`).**
- `matches_statement`: 14 call sites, all in `06_checked-trees-to-lowered-psi/src` (retention.rs x5, emission.rs, proofs.rs x2, attached_unit.rs, unit_*.rs x3, operation_emission.rs).
- `matches_checked_origin`: 3 call sites (`08.../result_policy_verification.rs:297,345`; `06.../machine_lowering.rs:213`).
- `terminal_operand_position`: 1 call site (`06.../retention.rs:2943`).
- `LoweredSourceCustody`: consumed by `08.../result_policy_verification.rs:280`; `08.../src/lib.rs:112` calls `lowered.into_parts()` and binds the custody to `_`.
- `visit_value_ids` (170-191): grep found no call in `omega-rust/psi` pipelines apart from `terminal-interpreter/src/boundary_effects.rs:1906`, which is a different type's method (`predicate.visit_value_ids`); I did not verify that this method has any caller. Suspected unused.

**Stated limits.** The doc comment (9-16) says the sidecars are "producer-private" and discarded "at the `PSITERM\0` wire boundary".

**Fail-open risks.** None seen. `matches_checked_origin` returns `false` on any mismatch.

---------------------------------------------------------------------
## 12. symbol-resolved-trees/examples/diagnostic_name_allocations.rs (50 lines)

**Purpose.** A standalone example binary that counts heap allocations (custom `GlobalAlloc`, lines 11-28) for three ways of building a diagnostic name and asserts the counts: the old `Arc<str>` path costs 2, `DiagnosticName::from_str` costs 1, `DiagnosticName::from(&str)` costs 1 (43-45).
**Shape.** `CountAllocations` global allocator (11-28), `main` (29-50). Uses `symbol_resolved_trees::name::DiagnosticName` (9).
**How it decides.** Hard-coded `assert_eq!` on counts 2/1/1 and string equality (46).
**Same job elsewhere.** None seen. `DiagnosticName::from_str` and `From<&str>` exist at `symbol-resolved-trees/src/type_system_and_names.rs:1042,1101` (grepped; the example is their only direct allocation test I found).
**Unused.** Example only; no callers by construction.
**Stated limits.** Line 2: "A single-thread process excludes allocations by the Rust test harness."
**Fail-open.** None seen.

---------------------------------------------------------------------
## 8. checked-trees/src/providers_and_trust.rs (897 lines)

**Purpose.** Turns checked requirements and conformances into the provider declaration/instance records that the Omega-side provider planner consumes, wraps checked trees in a lazily-computed `PreparedProviderInventory`, and builds the roster of trust declarations (admission-claim machines with their template and contract commitments).

**Shape.**
- `provider_declarations` (8-354): `ProviderDeclarationRequest` (35), `ProviderDeclarationRow` (28), `ProviderApplicationView {None, PhysicalLeaf, External}` (15), `ProviderDeclarationOrigin {Authored, BuildNative, Toolchain}` (22). `CheckedTrees::provider_declarations` (42-91), private `provider_reach` (93-151), `provider_external_applications` (153-281), free fn `closed_provider_leaf_applications` (284-331), `requirement_reach` (333-353).
- `provider_instances` (356-504): `inherited_instances` (368), `generic_instances` (428), `closed_requirements` (472), private `satisfied_requirement` (484).
- `provider_inventory::PreparedProviderInventory` (506-678): fields `checked: Arc<CheckedTrees>`, `OnceLock` caches `uses`, `origins`, `direct_calls`, `trust`, plus `services`. `require_owner` (624-631) checks `ptr::eq`.
- `trust_declarations::collect` (687-745) plus three tests.

**Inputs/outputs.** Reads `typed` (machines, trait definitions, closed requirement applications, specializations) and `facts` (`provider_closed_requirements`, `contract_plans`, `accepted_template_identities`, `service_reaches`). Produces `language_semantics` declaration structs (`ProviderDeclarations`, `ProviderInstanceApplication`, `ClosedProviderInstance`, `TrustDeclarations`).

**How it decides.**
- Matching a requirement row to closed applications is done by comparing identity TEXT: `trait_requirement_callable_identity_text(owner, signature) == row.requirement_identity` at 73, 164-166, 299, 378. This is a string compare of a rendered identity.
- `provider_reach` (93-151): `requirement_reach(...)` must exist and its identity text must equal `row.requirement_identity` (104); if the requirement's reach is not installation-bound it returns `Ok(None)` (107); only `Authored` origin has a realization (110-116).
- `provider_external_applications` (153-281): origin-dependent: Authored needs `typed.machine(symbol)`; BuildNative requires `requirement == row.requirement`, no named implementation, and a closed boundary trait without type or lifetime parameters (177-195); Toolchain returns `Err("selected Import has no retained source owner")` (196-198). For each closed application matching `template` and scope it requires `application.local_arguments.len() == required.len()` and the realization's type parameters to match in count and `std::mem::discriminant(&kind)` (261-275).
- `inherited_instances` (368-426) groups `provider_closed_requirements` rows by template, closed requirement, receiving application and `selecting_declaration` (381-389); requires one `declaration_application` across the group (407-412), no open type telescope and same package identity (393-396).
- `generic_instances` (428-470) keys on `machine_specializations` where `receipt.template == provider.symbol` and an operator realization names `template.symbol` (436-437); rejects instances that remain generic, differ in package, or whose closed requirement equals the template (456-460).
- `trust_declarations::collect` (687-745): machines with `supply_mode == AdmissionClaim`, excluding those that are fresh specialization instances of another template carrying an `accepted_template_commitment` (692-703). Template and contract evidence are looked up by cardinality: 0 -> `Missing`, 2+ -> `Duplicate` (711-720, 727-734). Duplicates are deferred, not resolved by taking the first.

**Cases by shape.** `ProviderApplicationView` has three cases; PhysicalLeaf uses only `closed_provider_leaf_applications`, External only `provider_external_applications` (56-80). `requirement_reach` handles only trait-requirement symbols or `TopLevelRequirement` machines (337-347).

**Same job elsewhere.** `closed_requirements` (472) and `satisfied_requirement` (484) re-derive "which closed requirement instance does this specialization satisfy" in a different way from `inherited_instances` (381-389). Both read the same closed-requirement tables.

**Unused / one-caller (grepped).**
- `provider_instances::inherited_instances`, `generic_instances`, `closed_requirements`: one external caller each, all in `omega/compilation/realization/provider-planning/src/provider_instances.rs:203,245,265`.
- `closed_provider_leaf_applications`: 2 uses (in-file 58; `provider-planning/.../provider_instances.rs:177`).
- `CheckedTrees::provider_declarations`: 1 caller (`provider-planning/src/provider_planning/selected_declarations.rs:57`).
- `PreparedProviderInventory`: 19 occurrences in 10 files (count mode), consumers include `psi-compiler/terminal-production/src/lib.rs` (4) and `provider-planning/.../provider_instances.rs` (3).
- Test-only: `trust_declarations::tests` (748-896).

**Stated limits.** Error strings only, e.g. "selected provider specialization instance `{}` remains generic" (444), "closed provider realization has an open telescope or a different package" (394).

**Fail-open risks.**
- 107: `if !bounded { return Ok(None) }`. An installation-reach requirement not flagged bounded yields no reach declaration. Traced: the flag is `service_reach_is_installation_bound` from the requirement (338-342), so a requirement mis-flagged upstream would silently drop its reach declaration. Stated: none. Open.
- 60: `ProviderApplicationView::None` leaves applications empty `Ok(Vec::new())`. By design (no application view).
- 161-163: `provider_external_applications` returns `Ok(Vec::new())` when `trait_machine_signature(row.requirement)` is `None`; a row whose requirement is not a trait requirement gets zero external applications without an error. Open.
- 262-269: `realization.is_none_or(...)`: with no realization (native origin) the telescope check is skipped. Stated: the BuildNative branch separately requires a closed boundary declaration (186-193).

---------------------------------------------------------------------
## 9. checked-trees/src/termination_and_cleanup_facts.rs (906 lines)

**Purpose.** Two fact families: contextual cleanup (what to dispose of at storage-window and state exits, and which parts are excluded) and termination ranking (rank coordinates, arrival edges, recursive components).

**Shape.**
- `contextual_cleanup` (7-451): `ContextualCleanupFacts` (33) with arenas `dispositions`, `storage_establishments`, `selected_dispositions`, `applications`, `exclusions`, `segments`, `window_invocations`, `entry_requirements`, and indexes `state_index: KeyedRows<(machine,state), Disposition>`, `selected_roster: OwnedIndex<Vec<Handle>>`. Rows: `ContextualCleanupDisposition` (324), `ContextualStorageEstablishment` (305), `StorageWindowInvocation` (314), `ContextualCleanupExcludedRegion` (26) with `ContextualCleanupExclusionCause {Unknown, Consumed, Unestablished, InactiveCase}` (17), `ContextualCleanupEntryRequirement` (354, with `predicate()` 377 and `direct_boolean_subject()` 391).
  Methods: `append_shifted` (47-189), `replaces_transferred_storage` (191-220), `is_selected_disposition` (235-261), state-keyed iterators (263-301).
- `termination` (453-906): `application_formation` (458-512), `arrival_correspondence` (514-825: `CheckedRankArrivalPlan` with 29 arenas at 754-783, `CheckedRuntimeRankComponent`, `CheckedSourceNaturalRank*`), `TerminationFacts` (828), `CheckedProofRecursiveComponent` etc. (834-890), `MachineTerminationFact` (902).

**Inputs/outputs.** Data tables; `append_shifted` consumes another `ContextualCleanupFacts` and appends rows with handle/span shifting (see below).

**How it decides.**
- `replaces_transferred_storage` (191-220): matches establishments on `(machine, state, source, root)` then compares the destination segment slice to `path`; returns `false` if the segment span is stale (206-209) or any matching establishment has `prior_value_transferred == false` (213-215); `true` only if at least one matched.
- `is_selected_disposition` (235-261): membership by `(arena_index, generation)` in a sorted, deduplicated roster rebuilt when `selected_dispositions.revision()` changes.
- `ContextualCleanupEntryRequirement::predicate` (377-389): `consequence` if `antecedent == Constant(true)`, else `Or(Not(antecedent), consequence)`. Default is `consequence = Constant(false)`, `antecedent = Constant(true)` (363-373).
- `direct_boolean_subject` (391-408): accepts only a consequence of the shape `StructuralParameterField` or `Not(StructuralParameterField)`.
- `judgment_for_body` / `for_body` (786-812): unique-row lookup on `(machine, body span)`; duplicates -> `None`.
- `TerminationFacts::for_machine` (893-898): first match by machine symbol.

**Cases by shape.** `CheckedRankOperand` is `Coordinate | Expression | Literal(u64)` (637-650); `CheckedRuntimeRankInput` is `Value | Distance` (663-670); `CheckedRankObservation` is `Value | Length` (631-634). `direct_boolean_subject` handles the single shape named above.

**Same job elsewhere.** `append_shifted` (47) and `ContentProjectionFacts::append_walk_rows` (facts.rs:749) both merge a worker's rows into a main table with `shifted_rows::*`; each hand-lists every field of every row type. Rank coordinates appear in three families: `CheckedRankCoordinate*` here, `CheckedNaturalRankMeasure` (flow_terminal_units.rs:175), and `CheckedRuntimeRank*` (here, 672-721): "suspected" overlap; I did not compare them.

**Unused / one-caller (grepped).** `append_shifted`: 1 external caller (`05.../checks_multiplicity_permission_events.rs:598`). `ContextualCleanupApplication|selected_dispositions`: 19 occurrences across 5 files (count mode), 14 in this file, 2 in `published_facts.rs`, 1 in `checks_multiplicity_cleanup_dispositions.rs`. `ContextualCleanupFacts::append_shifted` ignores `state_index` and `selected_roster` of `other` (destructured as `_`, 58-59); they are caches rebuilt from revisions.

**Stated limits.** None as comments.

**Fail-open risks.**
- `append_shifted` leaves `ContextualCleanupDisposition.path` unshifted (99-100, 104-120: `path` is copied) while shifting `exclusions`, `definite_exclusions`, `inactive_cases` into the cleanup facts' own `segments` arena (83-86, 116-118). Inferred: `path` indexes a different (flow ownership) segment arena. Not verified; open.
- `append_rows(&mut self.segments, segments, |row| row)` (167) and `entry_requirements` rows copied raw (186-187): they hold `SymbolHandle`/expression handles that do not need a shift. Traced by reading the row types (353-361).
- `replaces_transferred_storage` returns `false` on a stale span rather than error (206-209): fails closed.
- `is_selected_disposition` uses `revision()` of `selected_dispositions` only; changes to `dispositions` alone are covered because handles are compared with generation. Test at 415-449 covers it.

---------------------------------------------------------------------
## 1. checked-trees/src/facts.rs (3632 lines)

**Purpose.** Defines `CheckFacts`, the bag of every per-program fact table produced by Psi 05, and about 25 sibling modules each holding one fact family (content projection, declaration descriptions, nominal machine uses, provider closed requirements, carry facts, etc.). It also holds the closed scalar predicate evaluator front end.

**Shape (main items).**
- `CheckFacts` (22-79): 52 fields; accessors at 107-208: `authored_selection_occurrences` (119), `byte_write_is_proven` (134), `call_requires_decided` (140), `assignment_index_is_proven` (153), `assignment_index_coordinate` (171), `selector_coordinate` (191), `scalar_index_is_proven` (201). The proven-index tables are sorted Vecs searched with `binary_search_by_key` on `handle_position`.
- `scalar_predicates` (218-354): `closed_boolean_value` (228), `has_builtin_operators` (244), `evaluate` (270), `evaluate_for_atoms` (285), `source_node` (300), `predicate_operator` (341).
- `blocking` (356-379), `checked_crash_operator_site` (380-482), `content` (483-1190: `ContentProjectionFacts` 491 with 13 tables; `append_walk_rows` 749; `descendant_frontier` 716/971; `publishes_formation_membership` 1073; `establishes_completed_membership` 1144; `for_semantic_domain` 1176).
- `declaration_descriptions` (1192-1450), `proof_applications` (1451-1699, type aliases over `semantic_vocabulary::proof_applications` plus 230 lines of tests), `mutation` (1711), `qualifications` (1753-1946: `link_value_formations` 1829), `service_reaches` (1947), `suspensions` (2019), `synchronous_invocations` (2043), `calls` (2072), `quotient_operations` (2092), `dynamic_conformances` (2111), `installed_interrupts` (2152), `nominal_machine_uses` (2170-2415), `provider_closed_requirements` (2417), `provider_instance_publication` (2475), `requirement_call_specializations` (2582), `accepted_template_identities` (2658), `carry` (2695-2972: `canonical_suspension_crossing_id` 2858), `content_classes` (2973), `domains` (3016), `index_compatibility` (3071), `placement` (3104), `reference_custody` (3121), `resolved_types` (3188), `specialization_identities` (3251), `executable_bindings` (3290), `state_locals` (3402), `stored_integer_writes` (3486).

**Inputs/outputs.** Mostly plain data. Code that computes: scalar predicate evaluation (via `numerics::scalar_predicates::evaluate_nodes`), the content frontier walk, `link_value_formations`, `canonical_suspension_crossing_id`, `NominalMachineUseFacts::try_with_uses`, `RequirementCallSpecializationFacts::try_with_specializations`.

**How it decides.**
- `has_builtin_operators` (244-268): returns `false` if any operator use for this expression has a status other than `BuiltinFallback` (249-254); otherwise recurses only through `Binary`, `Unary`, `Borrow` and returns `true` for every other node kind (266).
- `source_node` (300-339): invalid expression -> `None`; static boolean -> value; integer/boolean literal -> value; `observe_boolean` hook; `Borrow` -> `Forward`; unary `LogicalNot` -> `Not`; binary operators `And, Or, Equal, NotEqual, Less, LessOrEqual, Greater, GreaterOrEqual` (341-353) -> `Binary`; anything else `Leaf`.
- `ContentProjectionFacts::descendant_frontier` (971-1069): frontier members are matched by `same` = equal `claim_identity`, `domain`, `semantic_domain`, `projection_machine` (980-988); errors on a non-unique entry roster (996-1004), on a transformation that consumes a subset of the frontier (1031-1040), and on duplicated outputs (1056-1058). Transformations are visited in arena order and stop at the `before` body conservation (1007-1014); a transformation applies when `statement_index < statement` or equal with same `evaluation` (1015-1023).
- `append_walk_rows` (749-967): refuses (returns `false`) unless every arena passes `shiftable`, and unless every `body_plan_index < body_conservation_plans.len()` (790-796).
- `publishes_formation_membership` (1073-1142) requires exactly one matching transformation whose completed value, followed by the output's place segments through constructed children, reaches the operand at the given `(state, statement, expression)`; `at_path` returns `None` on 0 or 2+ matching children (1099-1100).
- `link_value_formations` (1829-1916): builds decisions from `value_formations` plus `vacuous_uses`, drops those already published by a transformation (1850), sorts and dedups by coordinate tuple, groups them, and writes a span into each structural value keyed on `(machine,state,statement,expression, structural_value-or-invalid)`; `structural_value` is only the node's own handle for `ZeroedStructural | ZeroedScalarArray` (1893-1902).
- `canonical_suspension_crossing_id` (2858-2880): FNV-1a hash over symbol identity TEXT of machine, state, target (`symbol_identity` 2882-2918: `selector-slot:{interface text}:{requirement text}`, or `display_path(symbol,"::")` when the symbol has a module, else the legacy machine/state name `crossing_declaration_name` 2920-2930), statement and call ordinals, carry policy bytes and each live value's `normalized_type_identity_text`. Result is `max(1)`.
- `NominalMachineUseFacts::try_with_uses` (2321-2402): rejects empty envelope commitments (2326-2338), requires callback receipts to validate and to match the selected machine, entry and commitment (2339-2355), requires refinement fingerprints and commitments to equal the envelope identities (2356-2377), dedups by `(site, static_machine_ordinal)` and rejects conflicting duplicates (2378-2394); assigns `application_index`.
- `RequirementCallSpecializationFacts::try_with_specializations` (2616-2643): rejects specializations with no machine selections (2621-2625); dedup by `(site, registration_operation)`, conflicting duplicate -> `Err`.
- `CheckedDeclarationDescriptions::description` (1379-1396): uses the sorted `machine_index` only if `indexed_at == (revision, len)` (1383), else linear scan. `for_machine` (1435-1444) returns `Err` for `Missing`/`Unavailable`.
- `ContentClassFacts::classes` (2999-3005) and `ReferenceCustodyFacts::custody` (3171-3177): index rows by `TypeReferenceHandle.arena_index()`, with `.get(i).or_else(|| rows.first()).unwrap_or_default()`.
- `ResolvedTypeFacts::{place,result,carrier}` (3222-3248): index by handle with a check that the row's stored handle equals the query; mismatch -> invalid handle.
- `CheckedExecutableBindings::expression` (3385-3390): same equality check; missing -> `Invalid`.
- `StateLocalFacts::locals` (3440-3447): index by `state.arena_index()` only (generation is ignored) into `spans`; unknown -> empty. `named` (3467-3478) returns a local only if unique and the symbol is valid.
- `CheckedCrashOperatorSite::location` (400-420): `unreachable!("operator crash site must have StateStatement origin")` for any other origin.

**Cases by shape.** `CheckedUnitStructuralFieldType`-style lists are not here. In this file: `predicate_operator` maps 8 binary operators and refuses the rest (341-353). `SuspensionCrossingLoanSource` has 5 cases and `SuspensionCrossingLoanBacking` 5 (2755-2813). `CheckedExpressionSource` (3298-3310): `Invalid | Value | Saved | ProviderReadiness`.

**Same job elsewhere.**
- "machine -> fact row" linear lookups over `Vec<XFact>` repeated verbatim: `BlockingFacts::for_machine` (366), `MutationFacts::for_machine` (1736), `QualificationFacts::for_machine` (1936), `ServiceReachFacts::for_machine` (1968), `SuspensionFacts::for_machine` (2029), `SynchronousInvocationFacts::for_machine` (2053), `CheckedAcceptedTemplateIdentities::for_machine` (2683), `CheckedSpecializationIdentities::for_instance` (3272); also `TerminationFacts::for_machine` (termination_and_cleanup_facts.rs:893), `CheckedTerminalDebugPlans::for_machine` (flow_terminal.rs:2544), `CheckedPartialAffineUnitCleanupPlans::for_machine` (flow_terminal.rs:289), `CheckedNominalAffineUnitCleanupPlans::for_machine` (flow_terminal.rs:311), `CheckedUnitEffectPlans::for_machine/composed_for_machine` (flow_terminal_units.rs:1564,1568). About 14 copies of `iter().find(|x| x.machine == machine)`.
- `symbol_key` (facts.rs:1447), `handle_position` (operators_and_proof.rs:1400), `ProvenAssignmentIndex::key` (90), `OperatorSiteLocation::cmp` (452) each spell `(arena_index, generation)` ordering; `flow.rs:441` `key(context)` uses `(generation, arena_index)` in the opposite order.
- `ContentClassFacts` (2973) and `ReferenceCustodyFacts` (3121) are two copies of "dense Vec indexed by type reference with first-row fallback".
- `scalar_predicates::has_builtin_operators` (244) and `CheckedOperatorUseFact::resolved_through_builtin_fallback` (operators_and_proof.rs:477) both decide "is this use the builtin operator" with different status rules (see fail-open).

**Unused / one-caller (grepped, `omega-rust`).**
- `canonical_suspension_crossing_id`: 2 callers (`06.../retention.rs:5058`, `06.../projections.rs:1060`).
- `has_builtin_operators`: 42 occurrences in 5 files (36 in `05.../checks_contracts*.rs`, 6 here). `closed_boolean_value`: callers in `checks_contracts.rs:3378`, `checks_contracts_calls.rs:110`.
- `ContentClassFacts::classes`: 1 external caller (`06.../shared.rs:360`); built by `validation/src/value_custody_core.rs:3714`. `ReferenceCustodyFacts::custody`: 1 external caller (`06.../shared.rs:1473`); built by `validation/src/reference_result_custody.rs:8964`.
- `append_walk_rows`: 2 call sites (`checks_multiplicity_permission_events.rs:597,608`).
- `CheckedBoundaryAdapterDispatch`, `CheckedFactCallProjection` and two flow_terminal types: 52 occurrences across 13 files for the combined pattern (grep `CheckedBoundaryAdapterDispatch|CheckedUnitNominalAffineCleanupRequirementPlan|CheckedReferenceResultSourcePlan|CheckedFactCallProjection`), so they are used (19 in `validation/reference_result_custody.rs`).
- `CheckFacts::{byte_write_is_proven, call_requires_decided, assignment_index_is_proven, scalar_index_is_proven}`: the combined grep returned 17 occurrences in 7 files (including the 6 definitions here), so at least some have outside callers; per-function counts not taken.
- String-identity claim: the doc/guard in lib.rs says checked trees "carry identity handles, not text identities". Fields in this file that still carry text identity (not matched by the guard's literal patterns): `receiving_application: Result<String,String>` (2304), `canonical_requirement_overload: String` (2307), `registrar_declaring_application: String`, `registrar_arguments: Vec<String>`, `registrar_lifetimes: Vec<String>` (2161-2163), `CheckedProviderReceivingDescription{declaration_application: String, application: String}` (2549-2553), `CheckedCallableHeader.source_target: Option<String>` (1204), `CheckedSynchronousBoundaryTarget{declaration: String, application: Option<String>}` (1232-1236), `family_tuple: Box<[String]>` (2089, 2136), `IndexCompatibilityFact{name, actual_label, expected_label}` (3087-3095). Read, not counted by command.

**Stated limits.** `CheckedCallableHeader::default` sets `compiler_evaluation: Err("checked compiler-evaluation facts are absent")` and `synchronous_invocations: Err("checked synchronous invocation facts are absent")` (1223-1226). Comment on `ContentBodyConservationFact.plan_index` (586-589): "This source may belong to a terminal caller, so its parameter positions are not the completed body's coordinates."

**Fail-open risks.**
1. `has_builtin_operators` (266, `_ => true`): any expression kind other than Binary/Unary/Borrow (call, indexed, match, etc.) is reported builtin without inspecting children. Wrong program: an expression like `f(a + b)` where `a + b` resolves to a user operator would be called "builtin" at the `Call` node. Traced partway: `source_node` (336) turns the same kinds into `Leaf`, so `evaluate` does not fold through them. The 42 callers were not traced. Status: partly traced, open for callers.
2. `ContentClassFacts::classes` (2999-3004) and `ReferenceCustodyFacts::custody` (3171-3177): a handle past the table end, or an invalid handle (index out of range), returns row 0 (the first type reference's class) or an all-false/all-default row, instead of an error. Wrong program: a type reference interned after the table was built would be classified as the first type's contents. Only 1 consumer each (`06.../shared.rs:360,1473`). Open (I did not trace table construction).
3. `StateLocalFacts::locals` (3440-3447): keyed by `arena_index` alone, ignores generation, and `with_states` (3424-3438) overwrites `spans[index]` if two states share an arena index. Inferred, not traced.
4. `CheckFacts::byte_write_is_proven` etc. rely on Vecs being sorted (134-207); if a producer leaves them unsorted, `binary_search` returns `Err` -> "not proven" (fails closed). Traced by reading.
5. `CheckedCrashOperatorSite::location` panics instead of failing (413); not an accept path.

---------------------------------------------------------------------
## 2. checked-trees/src/flow.rs (2327 lines)

**Purpose.** Declares per-state control-flow facts (`FlowStateFact`, `FlowCallFact`, `FlowStatementFact`, `FlowOperandFact` and retention/referent structures), the storage-snapshot chain, the `FlowStateTable` with keyed lookups, checked store plans, and query helpers; also re-exports the ownership and terminal plan names.

**Shape.**
- `borrow_lifetimes` (9-37), `boundaries` (38-50), `call_origins::CheckedCallOrigin {Statement, Declaration}` (51-156) with `declaration()` validator (64-132), `contexts` (157-225: `FlowConstraintKind` with 7 variants; two `#[macro_export]` pattern macros `borrow_resource_pattern!` and `nonloan_constraint_pattern!`), `control` (226-415: `FlowCallFact` 248 with 40 fields, `FlowStatementFact` 332, `FlowStateOutgoingFact` 355, `FlowExitFact` 369, `FlowStateFact` 394), `descriptor_observations` (417-665), `invalidations` (666-697), `operand` (698-1088: `FlowOperandFact` 910, `FlowOperandRetention` 752, `FlowOperandCompletion` 965, `FlowOwnedCaseSelection` 1010, `FlowOperandChild` 1033), `storage` (1089-1122: `FlowStorageSnapshot`, `FlowStorageOperation {Unknown, Writes, Store, PreserveBinding, Join}`), `semantic_dependencies` (1125-1161), `state_rows::FlowStateTable` (1162-1672), `store_plans` (1675-1892: `CheckedStorePlan`, `CheckedStoreLeaf` with 10 variants 1770-1805, `CheckedStoreValue` 1808), `queries` (1895-2215).

**Inputs/outputs.** Data plus lookup methods on `FlowFacts`, `FlowFactsRef`, `FlowControlFacts`.

**How it decides.**
- `CheckedCallOrigin::declaration` (64-132): returns `Err(&'static str)` unless the application and call handles are valid, application machine/state equal the caller's, the machine body span is non-empty and valid, bindings are non-empty or an input-home receipt exists, the call list contains this call, the call has a valid expression, authored target, target machine, target state and `target_contract.is_some()` (123-130). Fail-closed.
- `FlowStateTable` groupings: `machine_rows`, `state_rows`, `call_rows` keyed by machine symbol, state symbol and `calls.start().generation()` (1256-1262). `update_row` clears caches only when the grouping key changed (1270-1284); `DerefMut` always clears them (1287-1295); `revision` is a `OnceLock` (1178).
- `call_owners` (1298-1324): finds states owning a call by generation key then confirms `state.calls.handles().nth(offset) == Some(call)`.
- Lookups: `state_by_symbol` returns the FIRST row (1339-1346); `unique_state_by_symbol` returns only if exactly one row (1348-1358); `state` first match (1360-1367); `unique_state` (1369-1382).
- `state_call_entry_constraints` (2146-2167): exact call by `(statement, call_ordinal, target_symbol, receiver_symbol)`; if absent, the statement's `entry_constraints`; if absent, the state's.
- `state_call_prior_invalidations` (2186-2212): invalidations strictly before the call, ordered by `(statement_index, call_ordinal)`.
- `CheckedStorePlan::window_place` (1715-1746): only `StructuralStore` or `OwnedRebind` leaves; the root parameter is looked up by `position` in `structural_parameters` (1727-1731).

**Cases by shape.**
- `installed_value` (1705-1713): "(StructuralStore | OwnedRebind, Place)" only; all other leaf/value pairs return `None`.
- `CheckedStoreDestination::field` (1750-1759): only a path ending in `Field(..)`.
- `FlowCallFact::argument_operands` (291-296): skips operand 0 iff `authored_receiver` is valid.

**Same job elsewhere.**
- `FlowControlFacts::call_at` / `call_position` (flow_ownership_and_roots.rs:390-415) and `FlowFactsRef::state_call` (flow.rs:2126-2144) are three ways to find a call in a state by `(statement, ordinal)`, with different keys (`state_call` also compares target and receiver symbol).
- `FlowStateTable` + `KeyedRows` triple vs `FlowOwnershipFacts.permissions_by_state` etc. (flow_ownership_and_roots.rs:144-148): same pattern.
- Two meanings of "no normal completion" live here as `FlowStateOutgoingFact.cannot_return_normally` (361-365) and in flow_terminal_units (see file 5).

**Unused / one-caller (grepped).** `FlowStateTable` and `unique_state*`/`state_by_symbol`/`machine_states`: the combined pattern count was not usable (my pattern also matched unrelated `.state_by_symbol(` in typed-trees); not reported. `CheckedCallOrigin` is used from `values_and_views.rs:567` and from lowered-psi (`LoweredSourceCallOrigin::Declaration` holds the same two handles, lowered-psi lib.rs:38-44).

**Stated limits.** Comment 1006-1008: "They introduce no transfer or disposal; cleanup still meets the actual branch outcomes." Comment 1022-1024: "Only a total case pattern's failure excludes that case."

**Fail-open risks.**
- `state_by_symbol` (1339-1346), used by `call_at`/`call_position` (flow_ownership_and_roots.rs:396,408), takes the first of duplicate state rows. The test `unique_state_symbol_keeps_duplicates_and_generations_distinct` (1396) shows duplicates are expected to exist. Wrong program: a state with two flow rows would have its call found in the first row only. Open (callers not traced).
- `state_call_entry_constraints` (2162-2166): missing call falls back to wider scopes. Open.
- `FlowFactsRef::state_statement` / `state_call` return `None` for unknown; consumers decide.
- `CheckedCallOrigin::is_valid` (134-141) only checks handles are valid, not rows.

---------------------------------------------------------------------
## 3. checked-trees/src/flow_ownership_and_roots.rs (2104 lines)

**Purpose.** (a) Root structs for flow facts: `FlowContextFacts`, `FlowInvalidationFacts`, `FlowBorrowLifetimeFacts`, `FlowOwnershipFacts`, `FlowBoundaryFacts`, `FlowControlFacts`, `FlowFacts`, with a macro that gives each paged-arena struct share/reset/page-transfer methods. (b) Ownership/permission facts. (c) `computation_custody`: counting how many "call transfers" of a locally produced structural value happen along each path.

**Shape.**
- `roots` (6-663): `WalkGuard` (72-119), `FlowOwnershipFacts` (122-239, 21 arenas + 3 KeyedRows), `FlowControlFacts` (253-458, 36 arenas), `FlowFacts` (461-509, 6 flow roots + 8 terminal plan tables), macro `paged_flow_roots!` (511-567) applied at 569-662.
- `ownership` (665-2104): `computation_custody` (670-1735), `FlowPermissionEvaluation` (1783), `FlowPermissionEventFact` (1792), `FlowOwnedSelectionReceipt` (1824), claim outcome and claim join types (1985-2080), instruction disposition/effect receipts (2083-2103); methods `state_exit_cleanup_roots` (1890), `owned_selection_for_expression` (1928), `owned_selection_at` (1950), `owned_selection_at_evaluation` (1961).

**Inputs/outputs.** `CallTransfers` (768-1608) reads `CheckedValueFacts` (scalar computations, structural values), a call-schedule closure, `CheckedGuardedExitPlans`; produces `usize` counts or lists of `(destination, count)`.

**How it decides.**
- `WalkGuard` (72-111): records whether a worker walk touched a state outside its owned list; read at `checks_multiplicity_permission_events.rs:740` (grepped). Notes happen inside `with_state_permissions`, `with_state_owned_selections`, `with_state_selection_place_bindings` (194, 208, 222) and one extra call site `checks_multiplicity.rs:6729`.
- `state_exit_cleanup_roots` (1890-1926): collects `StateExit` + `AffineDrop` events for a state; returns `None` if the same `(root, segments, evaluation)` occurs twice (1908-1910) or if two events at the same root/path differ in more than `evaluation` (1912-1920).
- `owned_selection_for_expression` / `_at_evaluation` (1928-1981): unique row by `(state, statement_ordinal, expression | evaluation)`; 0 or 2+ matches -> `None`.
- `CallTransfers::argument_transfer` (1023-1041): counts 1 only when access is `Owned`, path empty, `type_identity` equals the result's, and the source is the same structural result binding, the same local symbol, or the same expression (matched by handle identity, 1027-1039).
- Branch merges take the MINIMUM over alternatives: `Select` uses `min` of the two arms (1327-1331); `Dispatch` uses `alternatives.min().unwrap_or(0)` (1137, 1339, 1465); `scalar_exits` takes the min over exit paths (912); `completion` takes the min over arms (1241-1254).
- `source_is_retained` (1008-1021) requires the structural value's root to be found under its expression home (`root_for_value_home`), else the argument contributes 0.
- `constructed` (1069-1153) refuses to re-enter a `Place` node's production (comment 1088-1090: "following production again turns that identity edge into infinite execution").
- `current_result_binding` (694-731): follows `StructuralLocal` sources backwards through earlier `OwnedRebind` stores; stops with `None` on anything but a whole-value store.

**Cases by shape.** `CallTransfers::computation` (1257-1395) is an exhaustive match over 17 `CheckedScalarComputationKind` variants; `structural` (1397-1508) over 20 `CheckedStructuralValueKind` variants; `operation` (1529-1607) over `CheckedUnitEffectOperationPlan`. `ConditionalReturnArm` match at 830-864 lists each arm kind; `Crash` and `Unit` count 0 (863).

**Same job elsewhere.**
- The recursive walk over `CheckedStructuralValueKind` here (1397-1508) has siblings in `values_and_views.rs:1239-1338` (`completion_with`), `values_and_views.rs:1342-1396` (`value_dependencies`), and `facts.rs:1073-1142` (content path walk). Three exhaustive-ish matches over the same enum, each with `_ =>` or enumerations written separately. The `record_or_structural_case_pattern!` macro (values_and_views.rs:1737) exists to shorten some of them.
- `FlowOwnershipFacts::with_state_permissions` etc. repeat the `KeyedRows` + `walk_guard.note` pattern three times (189-229).

**Unused / one-caller (grepped).** `WalkGuard`: `for_states` 1 caller (`checks_multiplicity_permission_events.rs:688`), `crossed` 1 (740), `note` 3 outside (`checks_multiplicity.rs:6729`, `flow_ownership.rs:244,298`). `FlowFacts` terminal tables are consumed by 06 (grepped via `CheckedUnitPlan::for_machine`).
`definite_call_transfers` has 5 definitions here (`CheckedUnitEffectOperationPlan` 742, `CheckedCallScalarArgument` 755, `CheckedScalarReturnPlan` 795, `CheckedConditionalReturnArm` 817, `CheckedUnitCompletionPlan` 1611); I did not grep their callers.

**Stated limits.** 1088-1090 (quoted above). 1006-1008 in flow.rs. Comment 266-267: "The shared index follows the operand arena's exact mutation revision."

**Fail-open risks.**
- `scalar_exits` (905-914): `.and_then(|paths| ... .min()).unwrap_or(0)`: when no exit path yields a count (empty paths or the `Guarded` roster span is stale, 951 `?`), the transfer count is 0. A count of 0 means "no definite transfer counted". Direction: undercounts. Whether an undercount is the safe direction depends on the caller. Open.
- `computation` for `RetainedConstant | Value` returns 0 (1392-1393); `CheckedScalarCaseCall`-like nodes with an invalid handle would panic in `plans.nodes.get(root)` rather than return (1262). Not an accept path.
- `call_argument_evaluation_order` (379-388): an empty order span means "authored order". Stated: none. Open.
- `roots_for_...` and `retained_roots` (316-328) return `None` if `stored_projection` is non-empty: fail-closed.

---------------------------------------------------------------------
## 4. checked-trees/src/flow_terminal.rs (2665 lines)

**Purpose.** Declares checked Terminal lowering plans that Psi 06 reads: affine cleanup control, atomic access, boundary machine plans, calling signatures, dynamic slots, place loans, result bindings, scalar state terminators, structural arguments, structural control successors, structural return and type plans, terminal machine selections and debug plans.

**Shape (main items).**
- `affine_cleanup_plans` (8-531): `CheckedCleanupControlPlans` (21; 14 arenas), `CheckedCleanupPartitionPlan/Node/Arm/Step/Leaf/Ref` (43-123), `CheckedNormalCleanupAction {Discard, InvokeNominal, RetireHome}` (390), `CheckedNormalExitCleanups` (481), `CheckedNormalResultCompletion {NoNormal, Value, Dispatch}` (500); methods `operation_actions` (134), `normal_actions` (164), `result_actions` (176-213, an iterator with an explicit stack), `value_completion` (215-229).
- `atomic_plans` (532-625): `CheckedAtomicEvent` 6 variants (551-581), `CheckedAtomicAccessPlan` (612).
- `boundary_machine_plans` (626-767): `CheckedBoundaryMachinePlan` (726; 22 fields), `callable_entry` (752).
- `calling_signatures` (768-816), `opaque_representations` (817-824), `dynamic_slot_plans` (830-860), `place_loan_plans` (861-885), `result_binding_plans` (886-1007: `CheckedByteSequenceValue` 6 variants 973-1000), `scalar_state_plans` (1008-1183: `CheckedScalarStateTerminator` 1044, `CheckedScalarBranchDestination` 1064, `returning_paths` 1129-1181), `structural_argument_plans` (1184-1683: `CheckedUnitStructuralArgumentSourcePlan` with 12 variants 1274-1350, `CheckedUnitStructuralArgumentPlan` 1353 with path/selector helper methods 1376-1611), `structural_control_plans` (1684-1911: `CheckedStructuralControlSuccessorPlan` 1744), `structural_return` (1912-1988), `structural_type_plans` (1989-2515), `terminal_selections` (2516-2569).

**Inputs/outputs.** Data; helpers compute path/selector positions and fixed-shape predicates.

**How it decides.**
- `CheckedGuardedExitPlans::returning_exits` (1096-1125): Err unless the roster span is valid and non-empty and every destination is `Return{is_continuation:false}`; `ScalarReturnPath`/`returning_paths` (1129-1181) accepts only `Return`, `Conditional`, `Guarded` terminators ("scalar completion has no normal return destination") and only `Return` destinations ("scalar completion has a nonreturning destination").
- Index bookkeeping on structural arguments: `call_selector_positions` (1384-1410), `projection_evaluations_match` (1433-1466), `selector_computation` (1489-1512) each count index-type path segments (`FixedIndex | ComputedIndex | RuntimeIndex`) and treat the first `index_count - projection_evaluations.len()` as "inherited". `projection_evaluations_match` requires, for each non-inherited index, `FixedIndex` or a `ComputedIndex` equal to the evaluation handle (1458-1463).
- `CheckedUnitStructuralTypePlan` equality (2044-2058) compares `identity`, `descriptor_presentation`, native identity strings, flags, `shape`, `scalar_restriction` and IGNORES `source_declaration` and `source_application`. `CheckedUnitStructuralFieldPlan` equality (2131-2143) ignores `type_reference` and `source_declaration` (test at 2486-2513 asserts that `receiving_type_identity`, not source allocation, controls equality).
- `cleanup_type_identity` (2165-2178): structural field type -> its identity; borrowed byte sequence -> `byte_sequence_type_identity`; else default.
- `CheckedStructuralAccess::borrow_access_kind` (2307-2316): `Owned -> None`.

**Cases by shape (fixed lists).**
- `is_bounded_nominal_cleanup_record` (2412-2441): a record qualifies only if every field is non-erased and `Scalar(Bool|I8|I16|I32|I64|U8|U16|U32|U64|Addr)`; floats, bounded integers, byte sequences, structural fields, atomics are rejected. This is the quoted list at 2418-2429.
- `is_partial_affine_field_type` (2443-2473): `Structural{valid identity}`, `BoundedInteger`, `RefinedScalar`, owned bounded `ByteSequence`/`PredicateByteSequence`, or scalar of `Bool|I8..I64|U8..U64|Addr|F32|F64` (2458-2471). `Atomic`, `AtomicBoundedInteger`, borrowed byte sequences, `Erased` are refused. The two scalar lists differ: floats are in the second and not the first.
- `CheckedUnitStructuralTypeShape` has 9 shapes (2079-2108).
- `CheckedTerminalSignatureEligibility {Eligible, FreeUnitEffect, Unsupported}` (2532-2536).

**Same job elsewhere.**
- The scalar primitive list `Bool|I8|I16|I32|I64|U8|U16|U32|U64|Addr` is written out twice here (2418-2429 and 2458-2471) and is a third separate list from `complement_ceiling` in flow_terminal_units.rs:163-171 (U8..U64 only).
- `CheckedUnitPartialAffineDiscardPlan::same_effect` (259) and `CheckedUnitNominalAffineCleanupPlan::same_effect` (347) are field-by-field copies of `PartialEq` minus `partition_leaf`.
- `CheckedUnitStructuralArgumentSourcePlan` variants `StructuralLocal`, `StructuralResult`, `StructuralExpression` are identified separately by `CallTransfers::argument_transfer` (flow_ownership_and_roots.rs:1027-1039), `current_result_binding` (694) and `structural_storage_root` (1556-1568): the same "which place is this argument" decision made in three places.

**Unused / one-caller (grepped).** `CheckedTerminalSignatureEligibility::*` referenced at `06.../machine_lowering.rs:1516,1529,1808`, `scalar_lowering.rs:1069`, `attached_unit.rs:1310` and `05.../execution.rs:1367-1376`; the pair `Eligible | FreeUnitEffect` is re-spelled at three of these (`scalar_lowering.rs:1069-1070`, `attached_unit.rs:1310-1311`, here via `admits_signature`). `CheckedUnitPlanGap`/`Unsupported` usage: 20 occurrences in 7 files (grep, count). `CheckedUnitNominalAffineCleanupRequirementPlan` (361): this struct does not derive `Record`; counted within the 52-occurrence grep above, not individually.

**Stated limits.**
- Quoted comments: "A selected scalar result completes its admitted transaction before the private result join" (492-494); `CheckedNormalResultCompletion::NoNormal`: "The evaluated prefix remains executable but has no normal disposal edge" (501); struct-argument source variants carry their own contracts: "this identity never authorizes evaluating the expression again" (1297-1298); "This source is always the exact whole local" (1311-1312).
- `CheckedTerminalSignatureEligibility::Unsupported` (2535).

**Fail-open risks.**
- `CheckedUnitStructuralTypePlan::eq` ignoring `source_declaration` (2044): `CheckedUnitEffectPlans::adopt_structural_types` (flow_terminal_units.rs:1520-1543) treats two plans with the same `identity` and equal-under-this-`eq` as a non-conflict. Wrong program: two plans identical in all compared fields but built from different declarations merge. Stated: none; the test (2486) shows the intent for fields. Open.
- `CheckedUnitStructuralArgumentPlan::call_selector_positions` (1384-1410) treats `authored` as "`index && ordinal >= inherited && !projection_evaluations.is_empty()`"; if `projection_evaluations` is empty, FixedIndex segments are never reported as selectors while `ComputedIndex` and some `RuntimeIndex` always are. By design for the arrangement but not documented. Open.
- No `true`-on-missing accept paths found otherwise.

---------------------------------------------------------------------
## 5. checked-trees/src/flow_terminal_units.rs (2634 lines)

**Purpose.** Declares and implements the unit-machine Terminal plans: composed control machines (states, terminators, completion trees), call plans with an admission table, effect operations, and `CheckedUnitPlan` selection among three plan kinds. Also gap diagnostics for unlowerable machines.

**Shape.**
- `composed_unit_control_plans` (8-1017): `CheckedComposedUnitControlMachinePlan` (27), `CheckedComposedUnitControlStatePlan` (207), `CheckedComposedUnitControlTerminatorPlan` (228, 14 variants), `CheckedConditionalReturnArm` (763, 8 variants), `CheckedUnitCompletionExpression` (633), natural-rank types (118-197), guarded jump and closed-sum plans (920-1016). Methods: `has_normal_continuation` (287-375), `abandons_custody` (377-428), `all_paths_crash` (430-445), `successors`/`successors_mut` (447-520), `return_arms` (522-554), `live_mask` (601-620), `state_position` (68).
- `unit_call_plans` (1019-1440): `CheckedUnitCallPlan` with private fields and the validating constructor `new(parts) -> Option<Self>` (1113-1279).
- `unit_effect_plans` (1442-2558): `CheckedUnitEffectPlans` (1479), `CheckedUnitPlanGap` (1502), `CheckedUnitEffectOperationPlan` (1669, 17 variants), `CheckedUnitLocalBindingPlan` (1747, 7 variants), `CheckedOperandSequence` (1881), `CheckedStructuralValueCall` (1945), `CheckedUnitEffectMachinePlan` (2448), `CheckedUnitCallableEntry` (2474), three macros (2534-2557).
- `unit_plans::CheckedUnitPlan` (2560-2634).

**Inputs/outputs.** Data plus predicates/queries over plan trees; `gap_diagnostic` (1581-1665) builds a `Diagnostic` from a gap chain.

**How it decides.**
- `CheckedUnitCallPlan::new` (1113-1279): a call plan is created only for these `(completion, route, result, transfers)` combinations (quote 1127-1267 as `match completion`):
  - `External(_)`: `(Boundary, Unit, Abandonment)` only.
  - `NormalReturn`: `DescriptorTable` with `Unit|Scalar` result and `Claims(empty)` whose result type matches and whose operation passes per-op checks (`ActivateTask`: `U8` result, `entries` in `2..=8192`, 3 structural args, 1 scalar arg; `ReloadSegments`: `U8`, `1..=8192`, 2 structural, 1 scalar; `ReadTask`/`ReadSegment`: `U16`, no args) (1138-1172); `Module` with `NoNormal`+`Claims` iff `contract_commitment.is_some()`; `Module` + `Unit` + `Claims` iff commitment `None`; `Module` + `Scalar` + `Claims` iff `Some`; `Module` + `Structural` + `StructuralCustody` iff `Some`; `ActivationStorage` `Establish`/`Split` (bound `Some`) and `BorrowBytes`/`Merge` (bound `None`) with `Structural`+`StructuralCustody`, `Release` (`None`) with `Unit`+`Claims`; `Boundary` with `Unit|Scalar`+`CompletionReceipts` or `Structural`+`StructuralCustody`; `Dynamic` with `Unit|Scalar`+`Claims` or `Structural`+`StructuralCustody`; everything else `false` (1265).
- `has_normal_continuation` of the terminator (287-375): false if any operation lacks normal continuation (292-297); per-variant rules; `Guarded` validates that `return_values` is parallel to the arms plus fallback and each arm destination is `Return` with matching ordinal and continuation flag (308-335), else `None`; final `_ => true` covers `ReturnUnit | ReturnScalar | ReturnStructural` (373).
- `abandons_custody` (377-428): any external-completion call makes it true (382); `Crash` true; compound variants require ALL arms to abandon.
- `has_external_completion` (910-917): looks only at `Call` operations whose `completion()` is `External`.
- `CheckedUnitEffectOperationPlan::has_normal_continuation` (2124-2130): `EvaluateStructuralValue` false; `Call` by its plan; everything else true.
- `CheckedStructuralValueCall::with_operands` (1979-2003): rejects operands lacking normal continuation, requires the operation be a call, and accepts if it has a structural result OR (result `NoNormal` and no completions) (1985-1994).
- `CheckedUnitPlan::for_machine` (2577-2591): precedence NominalAffineCleanup, then PartialAffineCleanup, then ComposedControl; first non-empty wins. `admits_signature` (2601-2607) ignores `self`; `is_duplicated_in` (2609-2632) counts rows for the plan's own kind only.
- `live_mask` (601-620): BFS from state 0 using `successor_position`; `None` if any successor target cannot be positioned.
- `gap_diagnostic` (1581-1665): follows `Callee`/`BoundaryTarget` chains while a gap exists for the target and no cycle; maps `CoverageForm` to a noun; `UnselectedOpaqueRepresentation` becomes `Diagnostic::error`, all other gaps `Diagnostic::incomplete`.

**Cases by shape.** The match in `CheckedUnitCallPlan::new` is the main one (above). `CheckedNaturalRankFunction::complement_ceiling` (163-171): only `Complement` with `U8|U16|U32|U64`. `CheckedUnitPlanGapReason` has 4 reasons (1512-1517).

**Same job elsewhere.**
- "does this have a normal continuation" implemented separately at: `CheckedComposedUnitControlTerminatorPlan::has_normal_continuation` (287), `CheckedConditionalReturnArm::has_normal_continuation` (802), `CheckedUnitEffectOperationPlan::has_normal_continuation` (2124), `CheckedUnitCallPlan::has_normal_continuation` (1311), `CheckedStructuralValueCall::has_normal_continuation` (2019), `CheckedStructuralControlSuccessorPlan::has_normal_continuation` (flow_terminal.rs:1816), plus the stored bool `CheckedUnitCompletionExpression.normal_continuation` (636), plus `CheckedStructuralValuePlans::completion_with` (values_and_views.rs:1231, `NoNormal`). Seven places.
- `Eligible | FreeUnitEffect` (2604-2605) is spelled again at `06.../scalar_lowering.rs:1069`, `06.../attached_unit.rs:1310` (grepped).

**Unused / one-caller (grepped).**
- `CheckedUnitPlan::admits_signature`: 1 caller (`06.../machine_lowering.rs:1578`). `CheckedUnitPlan::is_duplicated_in`: 1 caller (`machine_lowering.rs:1575`).
- `CheckedUnitCallPlan::new`: 10 call sites in 2 files (`05.../execution_terminal_unit_calls.rs` 8, `execution_terminal_unit_control.rs` 2); the constructor is the only way to build one outside tests since `route`, `completion`, `result`, `transfers` are private (1102-1109) but `CheckedUnitCallParts` is public and `into_parts` (1281) lets a caller edit parts and rebuild (used at `execution_terminal_unit_control.rs:4608,4664`, grepped).
- `CheckedUnitEffectPlans::gap_diagnostic`: not grepped.

**Stated limits.**
- "this compiler does not lower {construct} of it yet ({detail})" (1612): the user-visible text of `CheckedUnitPlanGapReason::Unsupported`.
- `CheckedGuardedReturnValue` doc (75-76): "The producer joins the exact outgoing target; lowering consumes this list."
- "Mutually exclusive calls may reuse private result ordinals." (865-866).

**Fail-open risks.**
- 373 `_ => true` in `has_normal_continuation`: `ReturnUnit`, `ReturnScalar`, `ReturnStructural` report a normal continuation without examining the completion. Intended (they return). Stated: no comment. Traced by reading: these variants have no sub-plans that can stop.
- 2124-2130 `_ => true` for `Store`, `Complete`, `Establish*`, `ReleaseReference` etc.: the operation is assumed to continue. Wrong program: none identified, since only `Call` and `EvaluateStructuralValue` can stop per the variant list. Traced by reading.
- 972, 1005 `return_arm.statement_ordinal().unwrap_or(0)`: a return arm that retains no statement ordinal is reported as ordinal 0 (e.g. `Crash`-less `Structural(op)` where `source_statement_index()` is `None`). Open.
- `CheckedUnitCallPlan::new` returns `None` for unlisted combinations (fail-closed).
- `successor_position` (55-66): with `target_occurrence = Some(index)` it requires `states[index].state == target_state`; with `None` it takes the first state with that symbol (68-73). A state symbol appearing twice resolves to the first. Open.

---------------------------------------------------------------------
## 7. checked-trees/src/operators_and_proof.rs (2826 lines)

**Purpose.** (a) `operators`: checked operator uses and candidates, boundary application demands, float/integer comparison selection, crash contracts, and the `CheckedOperatorFacts` table with its lookups. (b) `proof`: contract proof facts, float-meaning projections and semantic applications, lemmas, mathematical declarations, proof obligations, proposition vocabulary and the `ProofFacts` root with keyed lookups.

**Shape.**
- `operators` (7-1403): `application_operands` (11-203), `comparisons` (204-331: `selected_float_comparison` 238, `selected_integer_comparison` 271, `occurrence_carries_operation` 310), `selected_float_comparison_execution` (333-418), `CheckedOperatorUseFact` (454), `CheckedOperatorResolutionStatus {Missing, Resolved, Ambiguous, DomainPending, BuiltinFallback, Inadmissible}` (443), `CheckedOperatorCandidateFact` (696), `CheckedOperatorFacts` (937), `ExpressionUseIndex` (956-998), lookups 1054-1398, `handle_position` (1400).
- `proof` (1405-2826): `contracts` (1406-1759), `float_meaning` (1760-2053), `lemmas` (2054-2104), `mathematical` (2105-2238), `obligations` (2239-2308), `propositions` (2309-2445), `roots::ProofFacts` (2446-2778; 35 fields, 4 KeyedRows).

**Inputs/outputs.** Read `TypedTrees` expression tables, operator declarations and signature contracts; produce selections (`(IeeeFloatComparisonOperation, PrimitiveType)` etc.).

**How it decides.**
- `boundary_application_operands` (28-141): requires exactly one resolved spelled use XOR exactly one named use for `(expression, origin)` (59-66); operator must be boundary (69); parameter count must equal operand count and no parameter may be const (80-84); operand primitive type from the declared type, else from the unique type argument bound to the same binder symbol (117-134).
- `selected_float_comparison` / `selected_integer_comparison` (238-307): operator must be a resolved boundary binary operator (`resolved_boundary_binary_operator` 211-236), spelling equals the declaration's spelling, and `occurrence_carries_operation` (310-331) accepts only an `Expression` occurrence that is a `Binary` with that operator, or `MatchEquality` with `Equal`. Integer: parameter types equal primitive, not `Bool|F32|F64`, returns `Bool`.
- `CheckedSelectedFloatComparisonExecution::validated_primitive` (386-416): re-derives the selection and requires exactly one boundary application at the use site with no arguments and no symbolic application.
- `resolved_through_builtin_fallback` (477-486): spelling equal, no selected operator symbol, `candidate_count == 0` and status `Missing` OR `BuiltinFallback`.
- `resolution_issues` (1326-1343): reports `Missing | Ambiguous | Inadmissible` only (not `DomainPending`, not `BuiltinFallback`).
- `expression_use` (1176-1191): first `Expression`-occurrence use for the expression, via the index built per revision.
- `selected_candidate` (1345-1355): the candidate whose `operator_symbol == selected_operator_symbol`.
- `resolved_contract_uses` (1378-1390): requires exactly one candidate (`let [candidate] = ...`) with `contract_count > 0`.
- `declared_contract_fact` (2570-2642): finds the unique contract of the right kind whose `facts` contains this fact (2597-2599 `Err`-like `return None` on a second hit), computes the lane position (counting only bound contracts, 2602-2604), requires exactly one recorded `ContractProofFact` for `(owner, kind, fact)`, and, for bound contracts, that the evidence term's owner, kind, lane position, name, proposition and interface equal the normalized expectation and that an interface exists (2617-2637); an unbound contract must have no evidence term (2638).
- `contract_call_at` (2644-2662) returns the LAST row for the key (`rows.last()`); `unique_contract_call_at` (2664-2685) requires exactly one.
- `integer_assignment_at` (2753-2776): unique match else invalid handle.
- `CheckedFloatSemanticApplication::validate` (1917-1966) and `CheckedFloatMeaningProjection::validate` (1989-2043): check contract identity against the catalog row, result kind, operand count/kinds, `Meaning(value)` operand ids strictly less than the result id, format agreement, source provenance non-invalid, source format equals the operation's.

**Cases by shape.**
- `selected_integer_comparison` (282-290): six spellings only (`Equal, NotEqual, Less, LessEqual, Greater, GreaterEqual`). `selected_float_comparison` (255-263): six `BinaryOperator`s.
- `CheckedUnit`-style lists: `CheckedFloatProjectionSource` 9 variants (1859-1869); `ProofLemmaKind` 5 (2059-2066); `QuantifiedBoundFact {Zero, Literal(i64), Length}` (2083-2088).
- `UseSite::operands` (506-551): Binary -> `[left,right]`; Indexed -> `[collection, index]` or `[collection, start, end]` for a range index (518-527); `MatchEquality` -> `[subject, pattern]` only for `MatchPattern::Value` and spelling `Equal`; anything else `None`.
- `CheckedProofValueDeclaration` supports one value type (`CheckedProofOnlyValueType::FloatMeaning`, 1791-1793).

**Same job elsewhere.**
- `ExpressionUseIndex` (956-998) duplicates `KeyedRows` (lib.rs:258).
- Sorted-Vec + `binary_search_by_key(handle_position)` lookups (`formation_is_decided` 1086, `is_trap_free` 1094) duplicate the sorted-roster pattern in `facts.rs:134-207`.
- `ProofFacts` has four unique-vs-last style lookups (2644-2749) where `contract_call_at` is "last row wins" and the others are unique; mixed conventions.
- Evidence/proposition identity is carried as `String` in `CheckedEvidenceTerm.name/evidence_type`, `CheckedPropositionApplication.arguments: Vec<String>`, `CheckedEvidenceRequirementIdentity.declaring_trait_arguments: Vec<String>` (2372), `OutcomeSpecificArmRowFact.captured_argument_identities: Option<Vec<Option<String>>>` (1506) and `instantiated_label: Option<String>`: text that stands in for identities; not caught by the lib.rs guard patterns. `schema_identity: String` (581) is the allowlisted one.

**Unused / one-caller (grepped).**
- `contract_call_at`: 6 callers (`checks_contracts.rs:7062`, `checks_content.rs:2336`, `checks_contracts_calls.rs:260,981`, `flow_call_phases.rs:2289`, `flow_calls.rs:57`); `unique_contract_call_at`: 1 caller (`checked-trees/src/contract_and_obligation_facts.rs:2856`).
- `resolved_through_builtin_fallback`: 2 callers (`05.../execution_terminal_unit_control.rs:5547`, `05.../published_facts.rs:6016`). `resolution_issues`: 1 caller (`05.../checks_operators.rs:2841`).
- `CheckedOperatorFacts::selected_float_comparison`/`selected_integer_comparison`: not grepped.

**Stated limits.** `CheckedFloatProjectionSource::TransitionalInput` (1860) is named "transitional" with no comment. Doc comment at 2196-2197: "A formed sealed Id statement, retained separately from evidence of truth." and 2490: "Typed Id formation supplies no identity witness or executable Boolean."

**Fail-open risks.**
- `resolved_through_builtin_fallback` (477-486) accepts status `Missing` with zero candidates as "builtin fallback". Wrong program: a use of `+` on a type that has no operator declared at all is status `Missing`; two callers (`execution_terminal_unit_control.rs:5547` for `Index`, `published_facts.rs:6016` for `Equal`) then treat it as the builtin. Contrasts with `scalar_predicates::has_builtin_operators` (facts.rs:249-254), which treats `Missing` as NOT builtin, and with `resolution_issues` (1334) which lists `Missing` as an error. Whether the type checker already rejected such programs is not traced. Open.
- `resolution_issues` omits `DomainPending` (1331-1337). If any use stays `DomainPending` it is not reported here. Open (not traced who handles it).
- `contract_call_at` returning the last row of duplicates (2659). Open.
- `declared_contract_fact` fails closed (returns `None`) on any mismatch.
- `CheckedFloatMeaningProjection::validate` `_ => {}` (2037) lets `TransitionalInput`, `ExactBinary32Literal`, `ExactBinary64Literal` sources through without provenance checks. Stated: none.

---------------------------------------------------------------------
## 10. checked-trees/src/values_and_views.rs (2803 lines)

**Purpose.** (a) `values`: checked value facts, scalar computation plans, structural value plans, completion plans, proof terms, scalar expression plans keyed by `(state, statement, role)`. (b) `views`: borrowed read-only/walk views (`CheckFactsRef`, `FlowFactsRef`, etc.) over the fact tables.

**Shape.**
- `values::computations` (25-712): `CheckedScalarComputationPlans` (136) with `roots`, `nodes`, `operands`, `dispatch_arms`, three KeyedRows; `CheckedScalarComputationKind` (498; 16 variants); `CheckedScalarCallee` (613; Body, Boundary, DescriptorTable, Dynamic); lookups `root_at` (211), `roots_for_statement_role` (232), `roots_for_machine` (302), `machine_root_handles` (315), `projection_selectors_at` (166).
- `proof_terms` (721-846): `CheckedProofTerm` (766; 7 variants), `CheckedProofTerms::term_at` (828).
- `completion_expressions` (851-1001): `CheckedCompletionPlans` (863), `CheckedCompletionRoot` with `refusal` (870), `CheckedCompletionRefusalReason` (888; 7 reasons).
- `structural_values` (1007-1875): `CheckedStructuralValuePlans` (1022), `CheckedStructuralValueKind` (1099; 22 variants), `completion_with` (1231-1340), `value_dependencies` (1342-1396), `call_schedule` (1398-1417), `retain_calls_for_root` (1419-1444), root lookups (1491-1733), `completed_dependency_from` (1605-1675).
- `guard_complement::exact_complement` (1893-2007).
- `CheckedValueOrigin` (2014), `CheckedValueFact` (2067), `CheckedValueFacts` (2076), `CheckedScalarExpressionPlans` (2166) with `bound_expression_at` (2221) and `expression_at` (2272), `CheckedScalarExpressionRole` (2312; 33 variants), `CheckedSubsliceSite` (2413), qualification contracts (2488-2520).
- `views` (2523-2803): `CheckFactsRef`/`CheckFactsWalk`, `FlowFactsRef`/`FlowFactsWalk`, `QualificationFactsRef/Walk`, `ContentProjectionFactsRef/Walk`; `view()` and `walk()` constructors.

**Inputs/outputs.** Lookup methods read keyed arenas; `completion_with` takes a `prepared_root` closure; `completed_dependency_from` takes a closure mapping an expression to `CheckedExpressionSource`.

**How it decides.**
- Root lookups require uniqueness: `root_at` (211-230), `projection_selectors_at` (166-191), `unique_state_root` (1491-1504), `term_at` (828-844), `bound_expression_at` (2221-2270, requires one binding AND one expression); `root_at` for completions (951-977) requires the key to occur exactly once in a sorted roster (no next equal key).
- `expression_at` (2272-2300) returns the FIRST matching expression (no uniqueness), through the sealed index only if `sealed_len == expressions.len()` (2279).
- `completed_dependency_from` (1605-1675): follows `Saved` captures; rejects cycles via `active_captures` (1632-1635, 1669-1671); rejects an `Invalid` source (1613-1618, 1626-1631); a root must be unique among `(state, expression, statement?)` with no `captured_copy` (1638-1658); returns it only if the node's expression equals the lookup expression (1660-1662).
- `root_for_value_home` (1707-1721): root and node must agree on expression, machine, state, statement ordinal and type reference.
- `completion_with` (1231-1340): `Call` node -> `NoNormal` iff its single scheduled call lacks normal continuation (1261-1272); `Dispatch` -> `NoNormal` if the subject stops or ALL arms stop (1273-1295); `Record/StructuralCase/QuotientCall/FixedArray/Projection/ViewSubslice/QuotientAdaptation/Reference` combine children (1296-1326); `_ => Normal` (1327).
- `exact_complement` (1899-1974): two Boolean expressions are complements when they are structurally equal with opposite polarity, or a pair of IEEE `Equal`/`NotEqual` comparisons over equal operands, or two `StructuralCaseMembership` of the same subject with different cases that exactly cover a declared case list of length 2 (1951-1971). `polarity` (1984-2006) strips `Not` and `== Constant`.
- `retain_calls_for_root` asserts no existing schedule for the value (1430-1436).

**Cases by shape.**
- `exact_complement`: only the two comparison pairs and case-membership with exactly two declared cases (quote 1967: `cases.len() == 2`).
- `CheckedElementViewRegion` 3 forms (1077-1084); `CheckedScalarCallee` 4.
- `CheckedValueOrigin` 4 origins; `CheckedValueStatementRole` 8.
- `CheckedCompletionRefusalReason::phase` text per reason (899-913).

**Same job elsewhere.**
- `roots_for_state`/`roots_for_statement`/`root_records_for_state`/`roots_for_statement_role` in `CheckedScalarComputationPlans` (232-300) and `CheckedStructuralValuePlans` (1515-1552) are parallel copies of the same keyed lookups over two root types.
- `CheckedScalarExpressionPlans::bound_expression_at` (2221) and `expression_at` (2272) each duplicate the "sealed index or scan" branch (2241-2258 and 2278-2299).
- `completion_with` (1231) / `value_dependencies` (1342) / `CallTransfers::structural` (flow_ownership_and_roots.rs:1397) are three walks of the same enum.
- `CheckedCompletionRoot::coordinate` 7-tuple (989-999) and the `ProvenAssignmentIndex::key` 4-tuple are two spellings of the same keyed-by-symbols idea.

**Unused / one-caller (grepped).**
- `completion` (1223): callers `05.../execution_terminal_unit_control.rs:6438`, `execution_terminal_unit_control_statement_sequence.rs:2260`; `completion_with`: `execution_terminal_unit_control.rs:6147,6403`.
- `views::*` types: re-exported in lib.rs:498-501; not individually grepped.
- `SealedExpressionStateIndex` implements `compiler_storage::Record` as a no-op (2436-2448): written as nothing and read as default, so it is dropped on serialization (inferred from reading).

**Stated limits.** Doc on `CheckedCompletionRefusal` (879-880): "This is diagnostic data, not a partial executable plan or permission to omit a selected call." Module doc 852-854: "Unit creates no scalar slot or structural home." Comment 2084-2085: "Proof-only erased actuals ... own no runtime operand rows."

**Fail-open risks.**
1. `completion_with` line 1255-1256: `if active.contains(&value) { return Some(Normal); }`. A cyclic value graph reports `Normal` instead of an error. Wrong program: a structural value whose subtree contains itself and a stopping call would be reported as completing normally. Likely unreachable given that value graphs are built acyclic; not traced.
2. Lines 1261-1272: a `Call` node with NO scheduled call (`call` is `None`) yields `Normal` (`call.is_some_and(...) ... else Normal`). Wrong program: a call that stops (no normal return) but whose schedule row is missing is treated as normal. Callers at `execution_terminal_unit_control.rs:6147,6403,6438` and `..._statement_sequence.rs:2260` are the consumers; not traced. Open.
3. Line 1327 `_ => Normal` for leaf kinds (Scalar, Place, ByteLiteralView, etc.) assumes leaves cannot stop. Stated: no comment.
4. `expression_at` (2272-2300) first-match with no duplicate check, unlike its sibling `bound_expression_at`. Open.
5. `CheckedValueFacts::scalar_type_refinement_predicates` (2116-2128): first matching refinement; unknown type -> `None`; fail-closed vs "no predicates" depends on caller.
6. `completed_expression_dependency` (1568-1591) maps non-`SavedSubject` expressions to `Value` without checking validity beyond `expression_is_valid` (1576): fine.

---------------------------------------------------------------------
## Coverage

- Read in full: all 12 files, 21,911 lines in total.
  - checked-trees/src/facts.rs 1-3632 (3632)
  - flow.rs 1-2327 (2327)
  - flow_ownership_and_roots.rs 1-2104 (2104)
  - flow_terminal.rs 1-2665 (2665)
  - flow_terminal_units.rs 1-2634 (2634)
  - lib.rs 1-801 (801)
  - operators_and_proof.rs 1-2826 (2826)
  - providers_and_trust.rs 1-897 (897)
  - termination_and_cleanup_facts.rs 1-906 (906)
  - values_and_views.rs 1-2803 (2803)
  - lowered-psi/src/lib.rs 1-266 (266)
  - symbol-resolved-trees/examples/diagnostic_name_allocations.rs 1-50 (50)
- Skimmed: none.
- Not read: none.
- Test modules (about 1,300 lines inside the above) were read but I recorded only what they assert when it bears on behavior.
- Commands run for counts: Grep (count and content mode) over `omega-rust` for `KeyedRows`, `append_shifted|append_walk_rows`, `admits_signature|is_duplicated_in`, `has_builtin_operators`, `contract_call_at|unique_contract_call_at`, `content_classes.classes|reference_custody.custody`, `canonical_suspension_crossing_id`, `provider_instances::|inherited_instances(|generic_instances(|closed_requirements(`, `WalkGuard|walk_guard`, `CheckedTerminalSignatureEligibility::`, `CheckedUnitCallPlan::new(`, `resolved_through_builtin_fallback|resolution_issues(`, `matches_statement|matches_checked_origin|terminal_operand_position|LoweredSourceCustody`.

## Five things a maintainer most needs to know about this unit

1. This crate is almost entirely passive data. The only code that makes judgments is: the admission table `CheckedUnitCallPlan::new` (flow_terminal_units.rs:1113-1279), the three `has_normal_continuation`-style walks (flow_terminal_units.rs:287-375, 802-810, 2124-2130; values_and_views.rs:1231-1340), `CallTransfers` (flow_ownership_and_roots.rs:768-1608), the content-claim frontier (facts.rs:971-1069), `declared_contract_fact` (operators_and_proof.rs:2570-2642), and the provider/trust assemblers (providers_and_trust.rs). Everything else is lookup.
2. "Normal continuation" (does control return from this?) has seven separate implementations and one stored boolean (listed under file 5). The structural-value version (`completion_with`) defaults a missing scheduled call and a cycle to `Normal` (values_and_views.rs:1255-1256, 1267-1271).
3. Lookups are inconsistent about duplicates: some require uniqueness and return `None` on two matches (`unique_*`, `root_at`, `term_at`, `bound_expression_at`, `declared_contract_fact`), others silently take the first (`state_by_symbol` flow.rs:1339, `expression_at` values_and_views.rs:2272, `for_machine` x14, `successor_position` fallback flow_terminal_units.rs:72) or the last (`contract_call_at` operators_and_proof.rs:2659).
4. "Operator is builtin" is decided by two different rules: `has_builtin_operators` (facts.rs:244-268) treats only status `BuiltinFallback` as builtin, while `resolved_through_builtin_fallback` (operators_and_proof.rs:477-486) also accepts `Missing` with zero candidates; `resolution_issues` (1326-1343) lists `Missing` as an error and omits `DomainPending`.
5. Identity-as-text is still present despite the guard in lib.rs:98-151: requirement and provider matching is done by comparing rendered identity strings (providers_and_trust.rs:73,164,299,378), `canonical_suspension_crossing_id` hashes display paths (facts.rs:2882-2930), and many `String` fields carry identity under names the guard does not match. The guard only greps six literal spellings.

## What I am unsure about

- Whether any of the fail-open spots (values_and_views.rs:1255-1271; facts.rs:2999-3004 and 3171-3177; operators_and_proof.rs:477-486; flow.rs:1339-1346) is reachable with a bad input: I did not trace the producers in `05_typed-trees-to-checked-trees` or `validation`.
- Whether `ContextualCleanupDisposition.path` (termination_and_cleanup_facts.rs:99-100, left unshifted in `append_shifted`) indexes the flow ownership `segments` arena rather than the cleanup facts' own `segments`; I inferred this from the content-facts comment at facts.rs:744-748 but did not check the producer.
- `visit_value_ids` in lowered-psi (lib.rs:170): I found no caller in a quick grep; the only hit is a different type's method. Not confirmed unused.
- Counts for `byte_write_is_proven`, `call_requires_decided`, `assignment_index_is_proven`, `scalar_index_is_proven`, `selector_coordinate` are only a combined total (17 occurrences across 7 files, including definitions), not per function.
- `CheckedUnitPlan::admits_signature` ignores `self`; I do not know whether that is intended or a placeholder for per-plan rules.
- The `contract_and_obligation_facts.rs` and `admissibility_and_borrow.rs` modules named in lib.rs are outside this unit and were not read.
