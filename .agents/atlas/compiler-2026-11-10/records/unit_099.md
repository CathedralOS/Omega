# Unit 99 notes

Files covered (all under `omega-rust/psi/compilation/validation/src/`, checkout `...\worktree\worker-23`, main `457adf4a11f`):

1. `lib.rs` (2132 lines)
2. `frozen_program_and_queries.rs` (4579 lines)
3. `float_and_mathematical_contracts.rs` (3690 lines)
4. `expression_types_and_fields.rs` (8764 lines)

All four were read in full, in order. Counts below come from Grep (count or `-o` content mode, uncapped, `head_limit 0`) over `omega-rust` unless stated.

---

## 1. `lib.rs`

**Purpose.** Crate root of the validation crate. It declares the flat module list, re-exports ~hundreds of names so call sites outside keep short paths (`declarations`, `machine_calls`, `proof_contracts`, `value_custody` facade modules, lib.rs:72-190), and also holds three small inline modules with real code: `compiler_evaluation`, `product_entries`, `native_locators` (plus `value_custody::pattern_components`).

**Shape.**
- Header comments lib.rs:1-38 describe four domain families and where the crate sits (Psi 03 -> 04 -> 05).
- `value_custody::pattern_components::report_delivery` (lib.rs:164): returns true (and pushes an error) when a value's `pattern_component` is missing, differs from `expected`, or is `Case` while the destination is not a reference type.
- `compiler_evaluation` (lib.rs:192-550): `checked_declaration` (:201), `symbol_custody` (:285), `selection_requests` (:300), `linear_carrier_violation` (:347), `checked_dependencies` (:414), `pending_condition_owners` (:510).
- `product_entries` (lib.rs:552-953): `root_bindings::collect_root_bindings` (:567), `RootBinding` (:710), `validate_source_compatibility` (:716), `ArrivalRequirementParameterType`/`ArrivalRequirementContract` (:817-829), `core_arrival_requirement_contract` (:831), `arrival_requirement_contract` (:871), `visible_entry_parameters` (:940).
- `native_locators::vocabulary` (lib.rs:955-1430): `NativeLocatorVocabulary` (:968), `vocabulary` (:973), `application_widths` (:1009), `unused_coordinate_widths` (:1051), `exact_vocabulary` (:1101), `binding_application` (:1135), `exact_width_parameters` (:1166), `validate_dll_import_shape` (:1199), `validate_binding_shape` (:1266), `require_fixed_bytes` (:1343), `binding_widths` (:1367).
- lib.rs:1437-2132 are only `pub use` re-exports, plus one generic helper `classify_type_references` (:2072, builds a dense `Vec<R>` indexed by type-reference arena index).

**Inputs/outputs.**
- `checked_declaration` reads `CheckedTrees.facts` (`service_reaches`, `operational`, `termination.rank_arrivals`), the typed machine and selection rows; produces `CompilerEvaluationDeclaration` or `Err(String)`.
- `collect_root_bindings` reads executed root bindings plus typed machines/symbols; returns `Vec<RootBinding>` or diagnostics.
- `validate_source_compatibility` reads a `ProductEntryDeclaration` and a slot contract; returns `Result<(), Vec<Diagnostic>>`.
- `native_locators` reads typed data definitions of the compiler-owned `ForeignBinding`/`DllImport` and returns three u64 widths / unused-coordinate masks.

**How it decides (keys).**
- `checked_declaration` picks the entry state by **string compare**: state name equals the last `::` segment of the machine name, else first state (lib.rs:236-247). `entry_requires` is true if machine or state has a `Requires` contract, or no entry state exists (`is_none_or`, :266).
- `local_terminates` is true if layout target facts exist OR (checked body AND `local_terminates` AND no diagnostic) (:261-264).
- `static_application` refusal keyed on `structural_type_equations_pending` and non-empty `type_parameters` (:268-278).
- `linear_carrier_violation` keyed on `Multiplicity::Linear` of attached data, owned data, state parameters, results, locals (:347-412); messages carry display spellings.
- `collect_root_bindings` AuthoredPath branch matches candidates by `machine.name.as_str() == declaration_path` **or** `display_path(..., "::") == declaration_path`, then requires same product package instance / dependency package identity and `source_reference_can_describe_product_symbol` (lib.rs:603-645); if any candidate matches by display path exactly the list is narrowed to those (:646-652). Duplicate slot -> error (:688).
- `core_arrival_requirement_contract` finds the owner trait by **name** `definition.name.as_str() == owner` among traits whose source file is a toolchain, Product-scope file whose path relative to package root is exactly `extent.omg` (lib.rs:840-857). `arrival_requirement_contract` walks inherited trait requirements and requires exactly one boundary trait named `owner` and exactly one machine signature named `method` (:881-917).
- `native_locators::vocabulary` identifies `ForeignBinding` and `DllImport` by data **name**, public, CheckedShape, non-instance, and source file path ending in `core/external_binding.omg` with Toolchain origin (lib.rs:977-990, 1113-1126, 1151-1164). Shapes are then pinned by names: case names `PeByName/PeByOrdinal/ElfVersioned/MachODylibSymbol` and field names (:1205-1210), parameter names `ObjectLength/SymbolLength/VersionLength` (:1177), `ForeignBinding` cases `DllImport{import}` and `Syscall{number: u64}` (:1286, :1332).
- `binding_widths` reads widths from the **spelling** of a Named type reference whose symbol is invalid and whose name is all ASCII digits, then parses u64 (lib.rs:1407-1426).

**Cases by shape.**
- `native_locators`: `unused_coordinate_widths` hard-codes per-case masks `[[false,false,true],[false,true,true],[false;3],[false,false,true]]` for the four `DllImport` cases and `[[false;3],[true;3]]` for the two `ForeignBinding` cases (lib.rs:1064-1070, 1098). Exactly 3 width parameters, exactly 4 DllImport cases, exactly 2 ForeignBinding cases (:1171, :1211, :1272-1280).
- `validate_source_compatibility`: matches `ProgramEntryVisibleParameters::None` (no visible params) and `ImageAndInitialStorage` (exactly 2) then `_ => {}` (lib.rs:754-774). Only the `ImageAndInitialStorage` slot also checks exact type identity against the arrival requirement (:776-809), with labels `["image","initial storage"][index]` (:792).
- `validate_dll_import_shape` branch `_ => unreachable!()` at lib.rs:1260 after length check at :1211.

**Same job elsewhere.**
- Selecting a product declaration by (possibly `alias::path`) name within a package is done twice in this unit: `root_bindings::collect_root_bindings` (lib.rs:601-685, own inline candidate filter and its own "not a product declaration visible from this build source's package" messages) and `queries::product_queries::ProductScope` + `select_declaration` + `describe_entry` (frozen_program_and_queries.rs:3512-3741, messages "not ... visible from this build occurrence's package"). Same filter conditions (dependency target lookup, `same_product_package_instance`, `source_reference_can_describe_product_symbol`), separately written. Read both.
- "compiler-owned declaration is the one in toolchain file X" is verified three ways: `native_locators` (source path `core/external_binding.omg`, lib.rs:1113-1126), `float_projection_bindings` (`normalized_hermetic_symbol_identity == "toolchain::..."` plus sealed source path, float_and_mathematical_contracts.rs:712-728, 1191-1214), `core_arrival_requirement_contract` (`extent.omg`, lib.rs:844-847). Constant counts: `FLOAT_*_CORE_SOURCE|core/external_binding.omg|extent.omg` found 31 occurrences in 11 files (Grep count).

**Unused / one-caller.**
- `collect_root_bindings|validate_source_compatibility|core_arrival_requirement_contract|arrival_requirement_contract|visible_entry_parameters`: 20 occurrences in 7 files; outside this crate: psi-compiler/source_compilation.rs 1, provider-planning program_entry.rs 3, program_entry/source_contract.rs 3, program_entry/discovery.rs 2, compiler/product_boundary.rs 1 (Grep count).
- `native_locators::*|application_widths|unused_coordinate_widths|binding_widths`: 12 occurrences in 4 files; outside: terminal-production/build_bindings.rs 1, 04 constructed_results.rs 2, provider-planning binding_evaluation.rs 4.
- `compiler_evaluation::(checked_declaration|checked_dependencies|symbol_custody|linear_carrier_violation)`: 3 occurrences in 2 files (build-time-evaluation/machine_execution.rs 1, 05 published_facts.rs 2). `symbol_custody` and `linear_carrier_violation` are `pub` but the grep with `compiler_evaluation::` prefix found only 3 total, so call sites of these two via other paths were not separately counted (suspected 0 to few outside).
- Header says `CheckingContext`/`CheckingInput` give "every later reader the same immutable program" (lib.rs:3-4); not counted (users run to hundreds of files; I did not run a count).

**Stated limits.** lib.rs:7-8 "Successful validation is a source judgment, not a portable proof certificate." lib.rs:27-28 "Checking only reads typed tables: preparation may rewrite them, checking may not". lib.rs:1233-1235 etc. use "drifted" errors for the compiler-owned vocabulary.

**Fail-open risks.**
- `report_delivery` (lib.rs:172-174): `!value.is_valid()` returns false (no error) -> an invalid value handle is silently accepted. Open (not traced).
- `validate_source_compatibility` `_ => {}` (lib.rs:773): any `ProgramEntryVisibleParameters` variant other than the two named gets no parameter-count check. I did not read the enum, so whether other variants exist is not known. Open.
- `visible_entry_parameters` (lib.rs:940-951) hides any parameter that has an access-view plan, so such parameters never count against "exposes no arrival parameters". Open.
- `checked_dependencies` (lib.rs:421-444, 493): a recursive component that is incomplete is silently skipped (not reported). That drops evidence rather than accepting; consumer behavior not traced. Open.
- `unused_coordinate_widths` returns `None` on any failure (lib.rs:1056-1098) rather than an error; caller meaning not traced. Open.

---

## 2. `frozen_program_and_queries.rs`

**Purpose.** Two inline modules. `frozen_program` defines the immutable checking input trait and the memo tables (`CheckingInput`, `CheckingContext`, `CheckingProjection`, `CheckingMemos`, `CheckingQuery`). `queries` holds shared read-only helpers: `execution_projection` (carving sub-programs out of `TypedTrees`), `expression_traversal`, `layout_checking`, `product_queries`, `substitutions`, `type_applications`, `type_regions`.

### 2a. `frozen_program` (frozen_program_and_queries.rs:8-1664)

**Shape.**
- `trait CheckingInput` (:567), sealed (:556-565), impls for `TypedTrees` (:693), `FrozenTypedTrees` (:723), `CheckedTrees` (:741), `CheckingContext` (:974), `CheckingProjection` (:1087), `LayoutCheckingContext` (layout_checking :3250), `CheckingQuery` (:428). `Deref for dyn CheckingInput` to `TypedTrees` (:667); `PartialEq for dyn CheckingInput` compares roots, layout target, deferral flag and the whole typed program (:676-683).
- `CheckingContext` (:771): fields incl. `memos: Arc<CheckingMemos>`, `call_frame_tables: OnceLock<..>`, `retained_borrows`, `finalized_selections`, `prepared_effects`, `storage_graph`, `expression_intervals`, `prepared_mathematics`, `opaque_selections`. Builder methods `with_*` and `clone_with_*` (:797-954); most reset or clone the memo table (`reset_context_sensitive_memos` :789).
- `CheckingProjection` (:1051): lazily clones the typed program (`program_mut` :1064-1072, `typed_program().clone()`); once projected, `memo_source()` returns None (:1074) so memos, effect plans, storage graph, intervals, call-frame tables and mathematics all disappear (:1152-1189).
- `CheckingMemos` (:1198-1439): about 55 memo slots (counted by reading; not grepped): `OnceLock<Arc<..>>` for whole-program tables and `Mutex<SymbolKeyMap<..>>`/`RwLock` for keyed answers. Methods `matches_effect_plans` (:1442), `retain_typed_tree_memos` (:1455), `query_scratch` (:1486), `clone_typed_tree_memos` (:1493).
- `CheckingQuery` (:338) with `QueryKind::{Numeric, OperandFits, CompletedCases, RecastFormation}` (:322): an "active query" ancestry used as a cycle cut. Constructors `recast_formation/numeric_observation/operand_fit_population/completed_case_population` return `None` if the same query is already active (:356-395). Counters `cut_reads` and `met_enclosing_cut` (:342-343, :409-424).
- `CutGuard` (:1572), `readable_memos` (:1590), `frozen_memo` (:1600), `once_lock_rows` (:711), `SelectedCarrierVerdict` (:1562), tests (:29-319, :1615-1663).

**Inputs/outputs.** Reads `TypedTrees` and optional checked facts; mutates only memo slots (interior mutability). `CheckingProjection::program_mut` is the only path producing a modified `TypedTrees` here.

**How it decides.**
- Cycle cut is keyed on the (machine, state), machine, or expression **handle** of the active query (:429-479); depth limit 64 for recast formation (:599, :1097).
- `CutGuard::memos` returns shared memos only when `cut_reads() == 0`, else the query-local memos (:1581-1587): answers computed after a cycle cut was hit are not published to the shared table.
- Memo reuse across context rebuilds is keyed on **value equality** of effect plans (`matches_effect_plans` compares `operational` and `service_reaches` by `==`, :1442-1453) and selection equality (:848).

**Cases by shape.** None as a fixed program-shape list; `QueryKind` is a closed 4-variant enum.

**Same job elsewhere.**
- Memo table design: sorted `Vec<(handle, OnceLock<T>)>` with binary search appears in `once_lock_rows` (:711), `DeclaredTypeDomainQueries` and `OwnedFieldDomainQueries` (expression_types_and_fields.rs:7536, 8183), all in this unit.
- `CheckingMemos` holds one std `HashMap<String, Option<TypeReferenceHandle>>` (`effective_result_references`, :1344-1349) while all other keyed memos use `symbols::SymbolKeyMap`. Its users: contract_entailment.rs:38867 and :38887 (2 lines, Grep).

**Unused / one-caller.**
- `name_path_last_member` (frozen:2792): 1 occurrence total (definition). 0 call sites (Grep count across `omega-rust`).
- `expressions_before_occurrence` (:2486): callers calls.rs:13101, 05 checks.rs:1624 and :2048.
- `inert_value_tree_with_operations` (:2405): 04 preparation.rs:510 and :530, 05 checks_current_scalar_values.rs:3450 and :6385.
- `expression_has_inert_prefix` (:2432): places.rs:6419, 05 checks_contracts_exits.rs:7731 and :8451.
- `visit_post_order` (:2698): arithmetic_domains.rs:26880, proof/obligations.rs:40 and :750, 05 checks.rs (2 lines), checks_contracts_prover.rs (2), flow_context.rs:1131, 04 constructed_results.rs (3), data_and_product_operands.rs (2), 04 lib.rs (2).
- `projection_climb` (:2744): reference_result_custody.rs:7147, program_validation.rs:1783, access_views_and_borrows.rs:1797 (3 outside).
- `single_member_argument_symbol` (:2773): domains_and_predicates.rs:7040, :11321.
- `push_children_reversed` (:2815): calls.rs:17788, :17929.
- `collect_type_expressions` (:3003): proof_contracts_core.rs:763, program_validation.rs:2718, 04 monomorphization.rs:2227, :2314, 04 selection_and_reflection.rs (3 lines).
- `type_applications::DeclarationScopes`, `type_regions` items: 52 occurrences in 8 files (traits.rs 6, reference_result_custody.rs 10, calls.rs 11, borrow_and_reflection.rs 7, contract_entailment.rs 2, representation-planning/representation_selection.rs 2, lib.rs 1, frozen 13).
- `layout_checking_targets|pending_layout_geometry|LayoutCheckingContext|layout_family_is_complete`: 34 occurrences in 7 files (arithmetic_domains.rs 6, recasts.rs 11, literals_and_struct_literals.rs 2, value_custody_core.rs 1, build-time-evaluation/const_evaluation.rs 2, lib.rs 2, frozen 10).
- `project_build_program|project_source_program|project_selected_program|descriptor_selection_occurrences|retain_machine_roster|product_request_operand_occurrences`: 19 occurrences in 8 files; outside: omega build-settings admission/checking.rs 4, 05 shared.rs 2, 05 products.rs 1, 03 lowerer.rs 1, build-time-evaluation lib.rs 1 and const_initializers.rs 1, declarations_core.rs 1.

### 2b. `queries::execution_projection` (:1667-2396)

**Purpose.** Copy a `TypedTrees` and keep only a chosen subset of declarations, so a Build program, a source-selected program, or a declaration-selected program can be checked alone.

**Shape.** `project_build_program` (:1683), `project_source_program` (:1770), `project_selected_program` (:1908), `project_declarations` (:1929, macro `project_roots!` :1944), `instantiated_declarations` (:1829), `is_derived_declaration` (:1852), `build_descriptor_declarations` (:1867), `descriptor_selection_occurrences` (:2079), `retain_machine_roster` (:2095), `apply_declaration_sidecar_pruning` (:2145), `product_request_operand_occurrences` (:2212), `materialized_product_operand_occurrences` (:2268).

**How it decides.**
- Build context = file origin `Toolchain` or dependency scope `Build`, read from `symbol_provenance_source_span` -> `source_file` (:1690-1699, :1716-1723). Keyed on the file where the symbol was declared, not on a symbol identity.
- A symbol is "derived" if it, or any ancestor, is a generic instance/specialization instance or has `generated_from` valid (:1852-1865).
- `project_declarations` iterates to a fixed point: select roots, collect their references, add referenced symbols (walking parents) that `retain_reference` allows into `synthesized`, repeat until no growth (:1941-2023). Typed type aliases referenced as descriptors are also added (:1994-1997).
- Product operand occurrences are matched by **source span containment** (`span.span.start >= operand_span.span.start && end <=`, :2240-2245) and by equality of rejoined `SourceSpan` (:2250) for captured expressions.

**Cases by shape.** `project_roots!` is invoked for a fixed list of 12 root tables (const_declarations, data_definitions, type_aliases, domain_definitions, measures, operators, machine_token_bindings, propositions, mathematical_definitions, traits, conformances, wire_schemas, :1962-1973) plus machines separately (:1974-1980). Any new root table not on this list is copied unfiltered (the `program` starts as a full clone, :1936).

**Fail-open.** `retain_reference` for Build projection returns true for symbols with `symbol_provenance_source_span(..).is_none()` (:1751-1754), i.e. symbols with no provenance are always retained. Stated: none. Open.
`project_selected_program` returns an unfiltered clone when `selection.is_all()` (:1912). Traced by name: "all" means no filter intended.
`descriptor_selection_occurrences` removes only occurrences that are "materialized" (:2090-2091); others are kept in the descriptor set. Open.

### 2c. `queries::expression_traversal` (:2397-3226)

**Purpose.** Walk expression trees: child enumeration (`children` :2590, `executable_children` :2654 which differs only for `Atomic`), pre/post order visits, collectors for expressions under statements, transition targets, proof facts and type references.

**Shape/decisions.** `children` is an exhaustive match on `ExpressionNode` (:2595-2651); `TypeExpression|SavedSubject|literal pattern` have no children (:2648-2650). `inert_value_tree_with_operations` (:2405): inert node kinds fixed list `Name|Integer|Boolean|Float|String|StructLiteral|ArrayLiteral|Borrow|Member|Match`, everything else must pass `allow_operation` (:2415-2429). `expression_has_inert_prefix` (:2432): depth cap 64 returns false (:2453); any `Call|Atomic` before the endpoint marks an effect (:2461). `expressions_before_occurrence` returns `Some` only when the endpoint occurs exactly once (:2587).

**Same job elsewhere.** `collect_expression_nodes` (:2841) vs `visit_post_order` (:2698) vs `visit_pre_order` (:2721) all do tree walks with a `visited`/`seen` set; three distinct implementations of "visit each expression once" here (`Vec::contains` in the two visits, `HashSet` in collect). Also `typed_trees::SubtreeEdges`/`program.subtree_any` used in `executable_subtree_any` (:2675) is a fourth walker (suspected in typed-trees; not read).

**Fail-open.** `visit_post_order` / `visit_pre_order` skip invalid expressions silently (:2710, :2729). `expression_has_inert_prefix`: `visit` returns true for an invalid expression (:2450-2452) so an invalid sub-expression counts as inert. Open.

### 2d. `queries::layout_checking` (:3228-3501)

**Purpose.** Decide which target layouts a set of type references must be checked against; wrap a `CheckingInput` with a fixed `layout_target`.

**Shape.** `LayoutCheckingContext` (:3239), `layout_family_is_complete` (:3330), `layout_checking_targets` (:3354), `discover_layout_checking_targets` (:3367), `pending_layout_geometry` (:3445).

**How it decides.** Per type reference: data definitions with `PlanLaidRealizations::TargetInputs` contribute a list of target names (spelled strings from `row.target`), intersected across types (:3413-3426). With a selected `layout_target` the answer is `[Some(target)]` only if alternatives equal `[None]` or contain it (:3359-3363). Errors collapse to `Err(())`.
`layout_family_is_complete`: for `TargetInputs`, row count equals input count and each row has same target facts, same machine, `row.target == row.target_facts.profile` (**string compare**), valid facts, and no duplicate target (:3341-3351).

**Fail-open.** `layout_family_is_complete` returns `true` when `layout.realizations` is not `TargetInputs` (:3334-3336). No comment; enum variants not read. Open.
`LayoutCheckingContext::checking_memos`/`shared_memos` return `None` (:3295-3305): no memoization under a layout context; not an accept path.

### 2e. `queries::product_queries` (:3502-3742)

**Purpose.** Resolve a build-time named product declaration (`describe_schema`, `describe_provider`, `describe_entry`) into a retained declaration handle.

**Decisions.** `ProductScope::contains` requires (path match by `display_path` if the name was qualified else by leaf `name`) AND (dependency-package match plus `is_public`, else same product package instance) AND `source_reference_can_describe_product_symbol` (:3536-3567). `describe_entry` tests each visible machine with `validate_source_compatibility` against the slot; exactly one compatible machine required else "ambiguous"/"incompatible" (:3696-3731). Messages distinguish "inaccessible" from "unknown" by comparing leaf/canonical names (:3637-3638, :3711-3715).

**Same job.** See 1 above (duplicate of `collect_root_bindings` selection).

### 2f. `queries::substitutions`, `type_applications`, `type_regions` (:3743-4578)

- `substitutions::actual_for` (:3746): last-wins lookup over a `(SymbolHandle, T)` slice.
- `type_applications::DeclarationScopes` (:3765): a scope stack of binding lists; `head` (:3801), `type_identity` (:3835), `is_strict_argument` (:3851), `definition` (:3900). `substituted_head` (:3966) loops at most `bindings.len()+1` times. `unique_data_definition` (:3989) = "exactly one `DataDefinition` with this symbol" by linear scan. `push_generic_application_bindings` (:4004) only accepts data definitions whose type parameters are all `Type` kind and count-equal (:4013-4018). `DeclarationScopes::definition` also refuses unless all parameters are `Type` and arguments are valid and lifetime counts equal (:3932-3944).
- `type_regions` (:4033-4578): `TypeRegionScope` trait (:4039); `retained_lifetime_identity` (:4139); `type_reference_regions_match` (:4188) uses `CallableRefinement` and requires exactly one resulting application; `type_reference_region_applications` (:4227) walks Reference/FixedArray/Slice/Generic/DynamicTrait pairs; `domain_region_applications` (:4468); `type_structure` (:4548).

**Cases by shape.** `type_reference_region_applications` handles pairs (Reference,Reference), (FixedArray,FixedArray)|(Slice,Slice), (Generic,Generic), (DynamicTrait,DynamicTrait) and `_ => true` (:4458).

**Same job elsewhere.**
- `unique_data_definition` (frozen:3989) is the same job as `places::unique_data_definition_by_symbol` (places.rs:8659) and `typed_trees` `unique_data_definition_by_symbol` (declaration_tables_and_index.rs:1850) (Grep `fn unique_data_definition...`), and the inline copy in `selected_value_type` (expression_types_and_fields.rs:4428-4437). Four sites; the typed-trees and places ones were not read (suspected identical).
- `substituted_head` (frozen:3966) vs `reference_values::substituted_reference` (expression_types_and_fields.rs:4021) vs `substituted_required_reference` (:4567): three "chase named type-parameter substitutions" loops with slightly different bounds/return shapes (substituted_head returns the last reference unchanged on exhaustion; substituted_reference returns `None` on exhaustion, :4040).

**Fail-open.**
- `type_reference_region_applications` `_ => true` (frozen:4458): when the two resolved nodes are not one of the four paired shapes (e.g. Named vs Reference, Generic vs Named), no region constraint is produced and the match succeeds. No comment at the site. Which wrong program passes depends on whether the caller separately checks type shape; not traced. Open.
- `domain_region_applications` (:4537-4539): under `CallableRefinement`, a required domain obligation with no corresponding actual domain is skipped (`continue`). Looks like intended refinement semantics; no comment. Open.
- `retained_lifetime_identity` (:4147-4149, :4165-4167): no retained regions or an invalid declaration yields `Ok(None)`, meaning no region constraint at that slot. Open.
- `CheckingProjection` after `program_mut()` drops all `has_active_*` answers to false except recast depth (:1097-1119; test :159-165 asserts this). Effect is loss of the cycle guard, not acceptance. Suspected unbounded recursion risk if a cycle passes through a projection; not traced.
- `CheckingMemos::clone_typed_tree_memos` copies mutex-guarded maps with `.lock().map(|g| g.clone()).unwrap_or_default()` (:1504-1555): a poisoned lock silently becomes an empty memo (no acceptance).

---

## 3. `float_and_mathematical_contracts.rs`

**Purpose.** Two inline pub modules: `float_projection_bindings` / `float_projection_invocations` recognize the compiler's sealed toolchain float semantic operations (projection to `FloatMeaning`, conversions, results, predicates) inside operator contracts; `mathematical_applications` is source custody for dependent mathematical definitions (identity, fixed-core vocabulary, prepared view, scalar/float predicates, logical bodies, result types).

### 3a. `float_projection_bindings` (:7-1338)

**Shape.**
- `conversion_contracts` (:8-323): `numeric_float_conversion_contract` (:25), `float_format_conversion_contract` (:17), `selected_numeric_float_conversion_contract` (:33), `integer_float_conversion_bounds` (:135), `scan_conversion_equalities` (:166), `projection` (:212), `float_integer_conversion_contract` (:228), `float_integer_conversion_bounds` (:300).
- `result_contracts` (:324-612): `float_result_semantic_contract` (:331), `float_saturation_divisor_parameter` (:435), `semantic_term_uses_only_inputs` (:475, work budget 1024), `float_boolean_semantic_contract` (:534), `float_scalar_semantic_contract` (:543).
- `semantic_operations` (:618-869): `exact_toolchain_float_semantic_application` (:629), `float_semantic_application_for_parameters` (:657), `typed_value_kind` (:756), `exact_toolchain_float_semantic_contract` (:799), `validate_float_semantic_operation_bindings` (:844).
- `semantic_values` (:870-1178): `closed_float_meaning_equality` (:888), `closed_meaning` (:920), `closed_float_semantic_integer_operand` (:1003), `exact_toolchain_float_format_const` (:1057), `struct_literal_matches_decoded_const` (:1142).
- Module-level (:1185-1337): `has_exact_toolchain_float_projection_owner` (:1191), `is_exact_toolchain_float_meaning_type` (:1216), `exact_toolchain_float_projection_contract` (:1253), `validate_float_projection_operator_bindings` (:1284).

**Inputs/outputs.** Reads operator definitions and their `ensures` equalities (`ensures_expression_facts`), expression table, hermetic symbol identity, toolchain relative paths. Returns the primitive types / semantic catalog row (`FloatSemanticOperation`) or `None`; `validate_*` push diagnostics for malformed canonical declarations.

**How it decides (keys: spelling and identity strings).**
- An operator is a "float semantic operation" if its path is `[namespace, name]` with `FloatSemanticOperation::namespace_matches`, `normalized_hermetic_symbol_identity == "toolchain::{FLOAT_SEMANTICS_NAMESPACE}::{name}"` (**format! string compare**, :717-722) and its symbol is declared in sealed source `FLOAT_PROJECTION_CORE_SOURCE` (:723-727). Shape checks: public, not boundary, no spelling, no lifetime/type params, no contracts, params not const/mut (:818-831).
- `typed_value_kind` maps primitives bool/i8..u64 to catalog kinds, rejects F32/F64/Addr/Selector (:760-775), then recognizes `FloatMeaning`, `FloatFormat`, `FloatClass` by `name == spelled && identity == "toolchain::Name" && source path && data definition exists by name` (:730-754, :1216-1242).
- `exact_toolchain_float_format_const` recognizes `BINARY32/BINARY64` by hermetic identity string (`"toolchain::FloatFormat::BINARY32"`, :1061-1090) for Names, and by matching a StructLiteral against the decoded evaluated const value for literals (:1092-1137).
- Conversion contracts require a **specific ensures shape**: an `==` between the result placeholder and a named call to a catalog operation; result must be owned by this operator (`reserved_result_owner(...).0 == operator.symbol`) and the call argument must be the single parameter by symbol (conversion :126-131, :288-291).
- `float_scalar_semantic_contract` / `float_result_semantic_contract` accept the first matching `==` found in the ensures list that uses only inputs (budget 1024 nodes, :421, :597).

**Cases by shape.**
- `selected_numeric_float_conversion_contract`: one parameter, source f32/f64 or an integer carrier, target f32/f64 (:42-52); for float source requires the catalog row name `"convert"` (:103); for integer source requires kernel `FromInteger` (:119).
- `float_integer_conversion_contract`: source f32/f64; row kernel `ToIntegerExact | ToIntegerSaturating` (:268-271).
- `float_saturation_divisor_parameter`: row name in `"divide"|"divide_toward_zero"|"divide_toward_positive"|"divide_toward_negative"` (:445-451).
- `typed_value_kind` `match primitive` is exhaustive over the 12 listed primitives (:761-775).
- `closed_projection_source`: pairs `(Meaning32, UndeterminedNaN F32)`, `(Meaning64, UndeterminedNaN F64)`, `(Meaning32, Float)`, `(Meaning64, Float)`; `_ => None` (:1036-1054).

**Same job elsewhere.**
- "Float format is binary32/binary64 from f32/f64 primitive" `match` is repeated at :92-96, :147-151, :402-406, :1261-1266, :1300-1305.
- Equality-of-result scanning loop `for (result, x) in [(left, right), (right, left)]` is repeated in `scan_conversion_equalities` (:188), `float_result_semantic_contract` (:349), `float_scalar_semantic_contract` (:575): same scan, three copies.
- `has_exact_toolchain_float_semantics_owner` (:712) vs `has_exact_toolchain_float_projection_owner` (:1191): same identity-string + sealed-path check parameterised differently.

**Unused / one-caller (Grep content `-o`).** All public float functions have outside users: `float_format_conversion_contract` proof/obligations.rs:1403 (also lib.rs re-export); `float_integer_conversion_bounds` proof/obligations.rs:1424, arithmetic_domains.rs:6949; `integer_float_conversion_bounds` arithmetic_domains.rs:9081; `numeric_float_conversion_contract` machine_calls_core.rs:8947, calls.rs:40144, arithmetic_domains.rs:8865, :9076; `float_integer_conversion_contract` machine_calls_core.rs:8954, contract_entailment.rs:22020, calls.rs:3571; `float_result_semantic_contract` machine_calls_core.rs:5382, calls.rs:40143, 05 checks.rs:9845, checks_operators.rs:814; `float_boolean_semantic_contract` 05 checks_operators.rs:538 (1 outside); `float_saturation_divisor_parameter` 08 result_policy_verification.rs:419, 06 operation_emission.rs:633; `float_static_call_resolution` 05 published.rs:3269 (1 outside); `exact_toolchain_float_projection_primitive` machine_calls_core.rs:5373, declarations_core.rs:113, contract_entailment.rs:77859; `collect_float_meaning_projection_invocations` program_validation.rs:4613; `validate_float_projection_operator_bindings` program_validation.rs:325.

**Stated limits.** Diagnostic text at :863 and :1314 states the exact canonical declaration form required ("one public, contract-free ordinary tokenless declaration ..."; "one private, contract-free ordinary tokenless operator ...").

**Fail-open risks.**
- `float_semantic_application` / `exact_toolchain_float_semantic_application` return `None` when 0 or more than 1 candidate (:649-652); `None` means "not a recognized operation", consumed as "no contract". Not an accept path by itself.
- `closed_meaning` evaluation: any operand shape not Format/Meaning/Integer -> `None` (:985).
- `semantic_term_uses_only_inputs` `Name` arm accepts any operator parameter by symbol (:496-498) and any `Integer/Float/Boolean` literal (:491-493): an arbitrary literal in the semantic term is accepted as an "input-only" term. Stated: none. Open.
- `float_boolean_semantic_contract`/`float_scalar_semantic_contract` accept the first matching equality and ignore later equalities that might conflict (:557-609). Open.

### 3b. `float_projection_invocations` (:1340-1577)

**Purpose.** In proof facts, find calls to float-meaning projection operators and `==` of two such projections; error when two compared projections disagree on operation or contract.

**Shape.** `ValidatedFloatMeaningProjectionInvocation` (:1354), `ValidatedFloatMeaningEqualityProposition` (:1364), `exact_projection_operation` (:1370), `requested_projection_operation` (:1392, keyed on the **spelling** of the receiver path's single segment as namespace and `call.target` as name), `resolve_projection_call` (:1403), `float_static_call_resolution` (:1411, requires receiver namespace spelled `"Float"`, :1419), `walk_expression` (:1435), `collect_float_meaning_projection_invocations` (:1542).

**How it decides.** Calls whose spelled `(namespace, target)` names a projection operation must resolve to exactly one exact canonical operator with the same operation, else error (:1481-1493). Equality of two projections requires equal operation and contract (:1463). Projections with mismatching operand type -> error (:1505-1513).

**Fail-open.** `walk_expression` returns Ok for non-projection calls and nodes (:1523, :1525). Equality where one side is not a projection is ignored (only both-projection equalities are recorded, :1451-1459). Stated: none. Open.
`expect(...)` at :1500 and `unreachable!` at :1503 are panics if earlier checks drift.

### 3c. `mathematical_applications` (:1579-3689)

**Shape (inline sub-modules).**
- `fixed_core_vocabulary` (:1587): `FixedCore {Level, Type, Strict, Squash, Identity}` (:1592), `FixedCoreVocabulary::resolve/classify` (:1642-1680), `FIXED_CORE_FALLBACK` spelling list for 4 of the 5 (:1683-1688).
- `identity` (:1690-1832): `identity_application_at` (:1712), `identity_application` (:1784), `is_identity_declaration` (:1773).
- `static_arguments` (:1838-1967): `mathematical_carrier_head`, `mathematical_binder_kind`, `source_generic_argument_plan` (:1930; allows omitting `Level` binders when counts match, :1944-1948).
- `expressions` (:1972-2255): `expression_nodes` (memoized :1984), `scoped_expression_nodes` (:2000), `definition_roots` (:2049), `erased_initializer_nodes` (:2089), `InitializerAccesss` (:2198).
- `logical_bodies` (:2259-2645): `is_indexed_logical_machine` (:2268, memoized fixpoint), `indexed_contract_is_proved` (:2311), `has_exact_theorem`, `logical_body`, `logical_body_contents` (:2390), `logical_expression` (:2574).
- `float_predicates` (:2647-2791): `float_predicates` (:2660) extracts `subject <op> literal` float bounds from a prepared term.
- `prepared` (:2792-3098): `PreparedMathematicalApplications` (:2803; 8 arenas + `safety_assumptions`), `PreparedConstantMeaning` (:2927, 16 variants), `PreparedMathematicalView` (:2963) with linear-scan lookups (`source_constructor`, `source_evidence`, `source_carrier`, `application`, `identity_value`, `constant`: each `iter().find_map`, :2970-3059), `application_spine` (:3062), `integer_embedding_meaning` (:3082).
- `scalar_predicates` (:3099-3325): `scalar_predicate` (:3111) converts a prepared term into `CheckedBooleanExpression`.
- Module level (:3337-3689): `is_mathematical_call/declaration` (:3345, :3353), `selected_definition` (:3361), `parameter_type_reference` (:3379), `operand_type_reference` (:3399), `scalar_observation_type_reference` (:3420), `subject_observation_type_reference` (:3431), `operand_type_reference_in_declaration` (:3448), `result_type_reference` (:3472), `argument_type_references` (:3488), `applied_result_reference` (:3516), `value_index_reference` (:3594), `ordinary_parameters` (:3628), `source_operations_have_same_meaning` (:3650), `is_source_evidence_carrier` (:3679).

**How it decides.**
- Identity is recognized by **symbol identity** against `symbols.fixed_core_declaration_symbol(Identity)` (:1773-1782) rather than spelling; but `FixedCoreVocabulary::classify` falls back to **spelling** `"core::Level"/"core::Type"/"core::Strict"/"core::Squash"` when the symbol is invalid (:1676-1680, :1683-1688). `Identity` has no spelling fallback.
- A machine is an "indexed logical machine" if it has an exact theorem and its body (and everything reachable through calls and transitions) passes `logical_body`; computed as a greatest fixpoint over candidates (:2282-2305).
- `logical_body_contents` fixed conditions: checked body, present, unit result, no attached data, no owned data, no `invokes`, no suspend/block, no service reach, contracts only Requires/Ensures; states: params non-mut non-const with a logical carrier; statements: Assignment/Assembly rejected, AssemblyFact accepted, Call/LocalData/Expression/Transition restricted (:2396-2534).
- `float_predicates::collect` accepts only `Squash`, non-Bool `Sigma`, or an application of a `FloatRelation` constant with one literal side and one capture variable (:2693-2788).
- Lookups by expression handle use linear scans through arenas (:2970-3059).

**Cases by shape.** `logical_expression` explicit refusals: `Atomic|Float|UndeterminedNaN|String|ZeroValue -> false`, `Indexed -> false`, Cast must be non-recast, logical carrier target, non-Trapping domain (:2587-2627). `scalar_predicate::predicate` handles `Squash`, `Id` over `Two` (bool) or integer carrier, `Sigma` (bool-case `Or` or `And`), and applications of `Negation`, `IntegerRelation`, `FloatRelation`; `_ => None` (:3140-3243). `comparison` only for fixed-integer primitive (:3300). `scalar` variable captures only fixed-integer or f32/f64 (:3257-3259). `float_predicates` supports only f32/f64 formats from `FloatFormat` (:2734).
`PreparedConstantMeaning::Opaque` is `#[default]` (:2928-2929).

**Same job elsewhere.**
- `float_predicates::collect` (:2685) and `scalar_predicates::predicate` (:3131) both walk the same prepared term shape (Squash, Sigma, FloatRelation application) and extract float comparisons with mirrored-operator logic; two implementations (float lit extraction at :2719-2765 vs :3265-3278).
- `operand_type_reference_in_declaration` (:3448) vs `expression_types_and_fields::declaration_expression_result_type_reference` (expression_types_and_fields.rs:4760) - the former is a thin wrapper over the latter.
- `ordinary_parameters`/`source_operations_have_same_meaning` get parameters from either a mathematical definition or a machine's first state (:3656-3668).

**Unused / one-caller (Grep count of the group: 148 occurrences in 27 files).** Per-file outside counts: proof/mathematical_declarations.rs 12, contract_entailment.rs 20, proof_contracts_core.rs 11, 05 checks.rs 11, values_scalar.rs 8, arithmetic_domains.rs 6, terminal-interpreter boundary_effects.rs 3, etc. I did not break the group down per function.

**Stated limits.** Header: "A mathematical definition owns a checked dependent telescope and transparent term; it is never an executable machine, a runtime result occurrence or a purity claim." (:1581-1584). `identity` module: "This view forms a statement; it grants neither a witness nor a value law." (:1693-1696).

**Fail-open risks.**
- `logical_body` (:2381-2383): `if stack.contains(&machine.symbol) { return true; }` - a cycle back to a machine already on the stack is treated as admitted. No comment. The fixpoint's `has_exact_theorem` requirement and whether termination is judged elsewhere were not traced. Open.
- `logical_expression` `_ => true` (:2633): ExpressionNode kinds not listed (Boolean, Integer, Name, Range, StructLiteral, ArrayLiteral, TypeExpression, Match, SavedSubject) are admitted, with children recursed (:2638-2643). Name is admitted without checking it is a logical parameter/local. Open.
- `erased_parameter` (:2181-2195): a missing target/state/parameter returns false => argument treated as runtime (non-erased), conservative direction.
- `erased_initializer_nodes` removes any expression also seen on a runtime path (:2173-2175): shared expressions are runtime. Conservative.
- `scoped_expression_nodes` returns early with empty set when no mathematical definition has expressions (:2016-2018); `expressions.retain(..executable_seen)` drops mathematical-definition expressions also reached from executable code (:2045). Intent: not stated.
- `PreparedConstantMeaning` default `Opaque` (:2928): an unresolved constant stays opaque, consumers treat as no meaning.

---

## 4. `expression_types_and_fields.rs`

**Purpose.** Two inline pub modules: `expression_types` computes the type reference of expressions (`expression_result_type_reference` and friends), decides whether an argument matches a required type/reference, and reports type/shape/class mismatches in operators, casts, matches and stores; `field_domains` answers questions about field/array domain constraints on declared types and byte-sequence domains.

### 4a. `expression_types` (:6-7496)

**Shape (inline sub-modules, in order).**
- `cast_validation` (:18-285): `validate_cast_types` (:27), `validate_quotient_mint` (:157), `report_number_to_bool_cast` (:236), `report_invalid_numeric_cast_source` (:257).
- `float_coercions` (:286-452): `validate_exact_float_coercion` (:295), `float_coercion_preserves_denotation` (:402).
- `float_operations` (:453-539): `validate_binary_float_formats` (:459), `validate_float_expression_formats` (:514).
- `match_dispatch` (:540-1504): `structural_patterns` (:551-1075: `structural_match_subject_type` :565, `structural_match_pattern_case` :585, `structural_match_pattern_is_total` :685, `validate_for_subject` :740, `structural_match_cases_are_covered` :982, `variant_is_impossible` :1014, `validate_transition_patterns` :1035), `match_subject_primitive_type` (:1082), `validate_match_coverage` (:1132), `validate_match_dispatch` (:1210), `is_scalar_value` (:1313), `check_compatible_values` (:1353), `borrowed_arrays_share_element_view` (:1472).
- `operator_validation` (:1505-2085): `validate_binary_operand_types` (:1512), `validate_builtin_comparison_operand_landing` (:1602), `expression_is_float_typed` (:1661), `report_float_bitwise_operator` (:1698), `report_cross_class_binary_operands` (:1733), `report_non_bool_logical_operands` (:1785), `report_invalid_text_operator` (:1816), `report_non_bool_logical_not` (:1858), `report_non_integer_bitwise_not` (:1879), `report_non_integer_index` (:1901), `report_undeclared_struct_operator` (:1993), `report_array_operator_operands` (:2049).
- `float_destinations` (:2088-2194): `report_mismatch` (:2096).
- `shape_validation` (:2195-2404): `value_shape_is_array` (:2205), `report_array_scalar_shape_mismatch` (:2232), `report_scalar_data_shape_mismatch` (:2272), `report_view_element_argument_mismatch` (:2373).
- `value_classification` (:2405-2755): `ValueClass {Boolean, Text, Numeric}` (:2412), `value_class` (:2450), `report_cross_class_store` (:2542), `concrete_data_type_symbol` (:2626), `report_data_type_conflict` (:2705).
- `call_carriers` (:2757-2921): `argument_matches_call_carrier` (:2761), `call_value_index_carriers_match` (:2828).
- `delivered_types` (:2922-3388): `DeliveredType` (:2933), `delivered_value_type` (:2938), `application_form` (:2986), `Carrier` (:3013), `delivered_carrier_conflict` (:3056), `carrier_conflict` (:3078), `static_implementation_operand_conflict` (:3243), `delivered_type_conflict` (:3290), `report_delivered_type_mismatch(_with_component)` (:3342, :3361).
- `reference_values` (:3390-4584): reference/array/slice correspondence (`reference_type_matches` :3404, `value_carrier_matches_reference` :3496, `fixed_array_binds_open_length` :3604, `owned_array_projects_to_slice` :3871, `owned_array_satisfies_constrained_referee` :3909, `constrained_referee_forgets_qualification` :4000), place access (`place_permits_reference_borrow` :4162, `place_storage_borrow_access` :4242, `ProjectionContext` :4265, `selected_value_type` :4324), `substituted_required_reference` (:4567).
- `result_type` (:4585-6329): `expression_result_type_reference` (:4600), `parameter_expression_result_type_reference(_in_scope)` (:4614, :4634), `ExpressionOwner` enum {State, Domain, Declaration, Telescope} (:4796), `result_type` (:4831, memoized), `join_result_type_references` (:5358), `same_result_type` (:5405), `binary_result` (:5567), `builtin_binary_result` (:5746), `arithmetic_carrier` (:6204), `qualified_builtin_carrier` (:6211), `cast_result_type_reference` (:6265), `operand_arithmetic_domain` (:6294), `selected_operator_result_type_reference` (:6312).
- `scalar_observations` (:6333-6541): `primitive_delivery_matches` (:6339), `builtin_scalar_operand_references` (:6398), `scalar_observation_meaning` (:6464), `readable_observation_type_reference` (:6475), `completed_observations_satisfy` (:6503).
- Module level (:6543-7496): re-exports; `argument_matches_type_reference_handle` (:6615), `completed_value_type_matches` (:6623), `completed_call_operand_matches` (:6634), `argument_matches_carrier` (:6672), `argument_matches_type_reference_using` (:6712-7182, the central matcher), `builtin_member_type_reference` (:7184), `named_value_type_reference` (:7195), `hinted_named_value_type_reference` (:7236), `state_symbol_declared_type` (:7351), `bounded_byte_buffer_capacity` (:7373), `validate_expression_type_handle` (:7383), `expression_type_name_handle` (:7418), `report_place_value_mismatches` (:7453).

**Inputs/outputs.** Reads `CheckingInput` tables: expression table, type reference table, data definitions, operator resolution (`typed_trees::operator::*`), memos. Produces `Option<TypeReferenceHandle>` / bool answers or pushes `Diagnostic`s. Memoizes `result_type` into `expression_result_types` / `parameter_result_types` (RwLock maps, :4842-4866, :5339-5354).

**How it decides.**
- Result type of an expression is computed by one recursive `result_type` over `ExpressionNode` (:4879-5330); it first returns a cached value, then const-declaration declared type (:4870), builtin member (:4873). Match arm results join by identical type, same domain shell, or same arithmetic carrier (:5358-5402). Call results are taken from, in order: address operand, proof embeddings, byte predicates, content conservation intrinsics, mathematical applications, retained constructed type, scoped requirement signature, resolved call result, selected operator (:4925-5070).
- Binary results: case-membership -> bool; spelled operators first try builtin meaning, else trait operator meanings (bound/selected), else resolve by spelling (:5567-5700); builtin table in `builtin_binary_result` (:5746-5985). Mathematical integer (`Int`) path precedes builtin (:5761-5792).
- Cast legality is keyed on the **spelled target name**: `program.named_type_reference(cast.target_type)`, `PrimitiveType::from_name(target.as_str())`; quotient targets found by `definition.name.as_str() == target_name.as_str()` and `definition.quotient.is_some()` (cast_validation :66, :91, :121-124, :129-139).
- `validate_cast_types` non-scalar target: accepted silently if the operand's stored structural identity equals the target's (:91-119); otherwise error "non-scalar `as` only erases qualifications from the same data carrier" (:129-138).
- Float coercion legality (`float_coercion_preserves_denotation` :402): int->float requires bounds in +-2^precision or a single value that round-trips (:410-431); float->float f32->f64 or same is true (:435); f64->f32 or float->int requires interval bounds (:441-450). Bounds come from `float_source_bounds`/`integer_source_bounds`.
- Match coverage: a wildcard arm, or both Boolean values, or `structural_match_cases_are_covered` (:1193-1203). Impossible variants dropped only when a `where` fact is literal integer 0 or literal false (:1022-1032). `admitted_match_cases` may narrow (:1008-1010).
- Value-class (Boolean/Text/Numeric) is derived from literal kind, operator kind, call return primitive, or declared primitive (:2450-2528): class decisions based on node kind and primitive name.
- `type_reference_is_text_carrier` / `is_byte_sequence_carrier`: reference-stripped FixedArray/Slice with `u8` element (:1768-1783, :8646-8663).
- `carrier_conflict` (:3078): compares carriers (`Parameter|Data|Implementation|Primitive`) by symbol identity; `rigid` = type parameter in scope of the owner (`parameter_is_in_scope` :3041); applications compared argument-wise, implementation const/proposition/machine operands via static operand identity (:3243-3288).
- `argument_matches_type_reference_using` (:6712): sequence = pattern-component check -> static type equality in context -> primitive delivery -> match arms all -> borrows -> recasts -> call result reference -> stored value reference -> named reference -> delivered carrier identity -> subslice -> final `match` on the required type node kind.

**Cases by shape.**
- `argument_matches_type_reference_using` final match (:6971-7181): `Reference` -> needs a delivered type or shared target recursion; `Constrained` -> peel; `FixedArray` -> accepts empty `String` for literal-0 `[u8; 0]` (:6994-6998), ArrayLiteral element-wise (:7000-7007), else exact identity or open-length binding; `Slice` -> accepts `Call|Indexed|Member|Name`, or a `String` when element is `u8` (:7030-7042); `Generic` -> accepts `Binary|Call|Cast|Indexed|Integer|Member|Name|StructLiteral|Unary` (:7043-7054); `DynamicTrait` -> accepts `Call|Cast|Indexed|Member|Name` (:7055-7062); `Named` primitive -> literals/landing/binary/call/cast/index/member/name/struct literal (:7106-7130); `Named` data -> StructLiteral must have a closed constructor, `Name` must be a variant of that data, Cast to primitive/Binary/Unary refused, else falls through (:7133-7159); `TypeParameter` + `Integer` -> true (:7161-7166); then the generic node-kind list (:7168-7178); `ConstExpression|Unit` -> false.
- `delivered_value_type` handles only `Call`, `Name`, `Member|Indexed`, `Atomic` (:2952-2973); `_ => return None`.
- `validate_binary_float_formats` fires only for `(F32,F64)|(F64,F32)` operand pairs with builtin spelled meaning (:493-511).
- `report_float_bitwise_operator` list: `BitwiseAnd|Or|Xor|ShiftLeft|ShiftRight|Modulo` (:1708-1716).
- `report_invalid_text_operator` list of 13 operators (:1826-1841).
- `binary_operator_spelling` supports only Add..GreaterEqual (9 spellings) (:1979-1989).
- `validate_match_dispatch` for non-structural subjects refuses `ArrayLiteral|StructLiteral|Range|Borrow` patterns with "not supported by value dispatch yet" (:1225-1235) and non-scalar subjects/patterns with "... are not supported yet" (:1237-1243).

**Same job elsewhere.**
- `type_reference_is_text_carrier` (:1768-1783) and `is_byte_sequence_carrier` (:8646-8663): identical body (carrier_beneath_borrows then FixedArray/Slice with u8 element). Two copies in this unit.
- `readable_type_reference` (:8026, field_domains) and `readable_observation_type_reference` (:6475): both peel Constrained and readable References; the first returns the last non-wrapper node, the second returns the last referee seen (with a loop bound of `type_reference_count()+1`). Two similar helpers.
- `value_shape_is_array` (:2205), `type_reference_is_collection` (outside), `collection_element` (:2319), `slice_view_element` (:2304), `field_domains::readable_slice_element` (:7719), `type_reference_fixed_array_capacity` (:8749): several element/capacity extractors over `FixedArray|Slice`.
- Selecting "unique data definition by symbol": see 2f (inline at :4428-4437).
- Variant/field lookup by name or symbol appears in `data_field_type_by_name` (:7915), `data_field_type_by_symbol_or_name` (:7936), `construction_field_type` (:7884), `exact_declared_field` (:7705), `structural_patterns::validate_for_subject` (:825-844), `hinted_named_value_type_reference` (:7261-7288).
- Attached-data field lookup by name: `attached_data_field_type` (:7842) resolves `machine.attached_data` via `data_definition_by_name_str`; sibling `attached_data_field_type_reference` exists in places.rs:8354 and calls.rs:41556 (grepped, not read; suspected same job).
- Binary-operand diagnostics (`validate_binary_operand_types` :1512) vs `float_operations::validate_float_expression_formats` (:514) both call `validate_binary_float_formats`.

**Unused / one-caller (Grep content, uncapped).**
- `attached_data_field_type` (:7842): 1 call site, `proof/src/lib.rs:612`.
- `data_field_type_reference_by_symbol_or_name` (:7970): 2 call sites (`proof/src/lib.rs:768`, 05 `checks_ranges.rs:10926`).
- `type_reference_fixed_array_capacity` (:8749): 1 call site (`contract_entailment.rs:24810`).
- `float_coercion_preserves_denotation` (:402): 1 outside call (`arithmetic_domains.rs:24973`).
- `completed_value_type_matches` (:6623): 2 (contract_entailment.rs:31597, :40751). `completed_call_operand_matches` (:6634): 1 (machine_calls_core.rs:8428).
- `validate_match_coverage` (:1132): 1 listed outside (contract_entailment.rs:41335; the grep matched the name, not verified as the same function).
- `domain_predicate_theory_is_complete` (:8390): 06 emission.rs:187, 05 execution_terminal_unit_types.rs:2548, :2852.
- `string_literal_grants_view_domains` (:8679): 05 execution_scalar_computations.rs:9095, execution_terminal_unit_state_graph.rs:6104.
- `domain_admits_zero_byte_carrier` (:8699): value_custody_core.rs:4102, structural_call_custody.rs:2830, proof_contracts_core.rs:7592, 05 published.rs:9632.
- `domain_is_concat_preserving` / `domain_is_subslice_preserving`: 05 flow_transfers.rs:132 / :156, checks_contracts_writes.rs:679, checks_contracts_prover.rs:46/:299, execution_terminal_unit_structural_scalar_store.rs:413.
- `domain_is_vacuous` (:8413): many callers (value_custody_core 3, recasts 1, domains_and_predicates 1, 05 multiple files).
- `expression_type_name_handle` (:7418): machine_calls_core.rs:7235/:7316, calls.rs:11/:4256/:4334/:40238/:40256.
- `report_place_value_mismatches` (:7453): program_validation.rs:2876, :3714. `validate_expression_type_handle` (:7383): program_validation.rs:3575, :3687.
- `argument_matches_type_reference_with_substitutions` (:6656): type_references_and_wire.rs:2099, proof_contracts_core.rs:6781, contract_entailment.rs:40750.

**Stated limits (quoted <20 words).**
- "arrays do not support `==` / `!=` yet" (:2070-2071).
- "match structural, range, and binding patterns are not supported by value dispatch yet" (:1233).
- "... are not supported yet" for structural/domain/case value patterns (:1241).
- Comment at :6725-6728 explains why static equalities are applied to the full delivered type including reference shells.
- Comment at :4976-4980 explains why runtime callers need the applied signature while trait-contract callers use the sibling's raw result.
- Comment at :8270-8271: applied schema keeps its fields even when an argument is generic.

**Fail-open risks.**
- `validate_exact_float_coercion` (:305-312, :354-356): returns `true` (no diagnostic) when the operand's result type is unknown (`None`), when either side is not a primitive, or when `float_coercion_preserves_denotation` returns `None` (non-float/int source, or a float source and a target that is neither integer-literal-accepting nor f32, :432-440). A cast whose operand type cannot be computed gets no float-denotation check. Stated: none. Open (not traced to callers; `validate_cast_types` calls it only when `state` is `Some`, :39-59).
- `validate_cast_types` (:36-38): `cast.form.is_recast()` returns immediately with no checks. Stated: none. Open (recasts validated elsewhere in `recasts.rs`, not read).
- `validate_binary_float_formats` (:484-492) and `float_destinations::report_mismatch` (:2117-2127): operand/source type unknown -> no error.
- `check_compatible_values` (:1444-1462): when neither side has a declared type nor a value class and neither is a struct literal, `data_mismatch` is false and the values are accepted as compatible. Open.
- `validate_match_dispatch` (:1270-1275): if all live arms have declared types and `common_static_type_reference` returns `Some`, returns before any compatibility check. Arms flagged by `static_type_context_is_unreachable` are dropped from checks (:1259-1262, :1297-1300). Open.
- `delivered_type_conflict` (:3290-3340) and `carrier_conflict` (:3078-3241): return "no conflict" for: unreachable static context (:3296), expression kinds not in `delivered_value_type` (:2972), failed substitution (:3099-3105), any `Reference` on either side (:3138-3141), unresolved carriers (:3144-3148), foreign (out-of-scope) type parameters (:3149-3153), and same-named declared types at `Stored` position (:3165-3167). The function reports conflict only, so an unknown is accepted. Stated: none. Traced partially: `report_delivered_type_mismatch` is a diagnostic reporter; whether another gate rejects those is not traced. Open.
- `argument_matches_type_reference_using` final arms (:7030-7062, :7168-7178): required `Slice`, `Generic`, `DynamicTrait`, and non-data/non-primitive `Named` types accept an argument by **expression node kind alone** (Call/Indexed/Member/Name/etc.) once earlier identity/delivered checks did not return. E.g. a `Call` whose `constructed_value_type_reference` is unset reaches `Generic` and is accepted. Stated: none. Open (earlier returns at :6882 and :6928-6951 only cover reference-typed call results and delivered types that exist).
- `named_value_type_reference` (:7195): `[_] = name_path_members else None` single-segment only; fine.
- `structural_match_cases_are_covered` (:993-995): `cases.contains(&SymbolHandle::invalid())` (a whole-record or wildcard-like case) -> covered. `variants.peek().is_none()` -> requires that invalid case. Traced by reading: wildcard arm does not reach here (handled by `wildcard` flag at :1162), the invalid symbol marks a total record pattern.
- `structural_match_pattern_is_total` (:685-704): `MatchPattern::Value(_) => true` - any value pattern counts as total in coverage; for structural subjects a value pattern head that resolves to a case covers that case (:613-632). Whether a `Value` pattern with a guard is possible is not traced. Open.
- `variant_is_impossible` (:1014): only literal `0` integer or literal `false` where-facts mark a variant impossible; no inference. Conservative.
- `field_domains::domain_requires_provenance` (:8362-8388): unknown domain symbol -> `true` (needs provenance), cycle -> `true`. Conservative.
- `domain_is_vacuous` (:8413-8437): invalid/cyclic/unknown domain -> `false` (not vacuous). Conservative.
- `domain_membership_implies` (:8608): requires `has_symbol_only_proof` for both, else false. Conservative.
- `cast_erases_membership` (:8517): `is_recast || !result_type.is_valid()` -> false.
- `string_literal_expression_grants_domain` (:8665): decided by evaluating the byte predicate on the literal bytes; no predicate -> false.

**Stated shape lists in `field_domains`.** `domain_admits_zero_byte_carrier` match on `ByteSequencePredicate::{ValidUtf8, AsciiOnly -> true, NoNul -> length == 0, NonEmpty -> length != 0}` (:8726-8730): four predicate kinds. `declared_owned_field_obligations_uncached` handles data definitions, fixed arrays with literal length (zero length returns, :8294-8296) and records every other non-primitive, non-unit carrier as "unresolved" (:8331-8338). Cycles (`ancestors.contains`) push `unresolved` (:8277-8280).

---

## Coverage

- Read in full: `lib.rs` 1-2132 (2132 lines); `frozen_program_and_queries.rs` 1-4579 (4579); `float_and_mathematical_contracts.rs` 1-3690 (3690); `expression_types_and_fields.rs` 1-8764 (8764). Total 19,165 lines read in full.
- Skimmed: none. Not read: none.
- Outside-unit sites named above (places.rs:8659, calls.rs:41556, places.rs:8354, typed-trees declaration_tables_and_index.rs:1850) were seen only as grep hits, not read.

## Five things a maintainer most needs to know about this unit

1. `lib.rs` is not only a facade: it contains three real implementations (`compiler_evaluation`, `product_entries`, `native_locators`) that identify compiler-owned declarations by **name and source-file path** (`core/external_binding.omg`, `extent.omg`, `toolchain::FloatFormat::...`), and pin shapes with hard-coded name lists (4 DllImport cases, 3 width params).
2. The `CheckingInput` family (`CheckingContext`, `CheckingProjection`, `CheckingQuery`, `LayoutCheckingContext`) carries a ~55-slot memo table; `CheckingProjection::program_mut` clones the whole typed program and then returns `None` for memos and `false` for all `has_active_*` cycle cuts (frozen:1064-1119), so anything checked through a projection runs uncached and without the cycle guard.
3. `expression_types::argument_matches_type_reference_using` (expression_types_and_fields.rs:6712-7182) is the central "does this argument fit this type" test; after its identity checks it falls back to accepting by expression **node kind** for `Slice`, `Generic`, `DynamicTrait` and plain `Named` targets (:7030-7062, :7168-7178).
4. The mismatch reporters (`carrier_conflict`, `report_cross_class_store`, `check_compatible_values`, `validate_exact_float_coercion`, `validate_binary_float_formats`) only emit errors when they can compute both sides; an unknown side silently yields no error (list under Fail-open in section 4).
5. Several jobs are written more than once: product-declaration selection (lib.rs:601-685 vs frozen:3512-3741), unique-data-definition lookup (4 sites), byte-sequence-carrier test (expression_types_and_fields.rs:1768 vs :8646), float-format `match` (5 sites), the "result `==` call" scan (3 sites), term-walk for float predicates (:2685 vs :3131).

## What I am unsure about

- Whether the node-kind acceptance arms in `argument_matches_type_reference_using` are backstopped by a later gate (I did not trace callers in `program_validation.rs`, `calls.rs`, `contract_entailment.rs`).
- Whether `PlanLaidRealizations` and `ProgramEntryVisibleParameters` have variants other than those named (enums not read), which decides if `layout_family_is_complete`'s early `true` and `validate_source_compatibility`'s `_ => {}` are reachable.
- Whether `logical_body`'s "cycle returns true" is sound; termination of logical machines is not judged in this unit.
- Per-function outside-call counts for `mathematical_applications` and `compiler_evaluation::symbol_custody|linear_carrier_violation` were grouped, not split.
- The count of memo slots in `CheckingMemos` (~55) was counted by eye while reading, not by command.
- The `CheckingProjection` unbounded-recursion concern (item in 2f) is inferred from the test at frozen:159-165 and code at :1097-1119; no caller traced.
