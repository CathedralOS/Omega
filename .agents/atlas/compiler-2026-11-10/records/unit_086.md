# Unit 86 notes

Files covered (all under `omega-rust/psi/compilation/representations/typed-trees/src/`):
1. `evidence_and_inspection.rs` (1-5602)
2. `lib.rs` (1-1871)
3. `operator_declarations.rs` (1-3900)
4. `product_and_trait_declarations.rs` (1-4130)
5. `references.rs` (1-3710)

Paths below are relative to that `src/` folder unless they start with `omega-rust`.

---------------------------------------------------------------------------
## 2. lib.rs (1871 lines)

**Purpose.** Crate root of `typed-trees`. It defines the `TypedTrees` program container (roots + arena tables + side tables), its frozen wrapper `FrozenTypedTrees`, and a set of small inline modules: boundary-value recipe building, direct-call declaration collection, authored-target lookup, build provider requirement scope, canonical const materialisation, empty-run-release authentication, authored-selection exclusion set, symbol rebinding pairs. Header comments (lib.rs:7-38) say it is produced by Psi 03, frozen by Psi 04, consumed by Psi 05/06/08, and give the file layout.

**Shape.**
- `TypedTrees` lib.rs:1348-1393: ~50 fields: `identity: ProgramIdentity`, `product_declaration_frontier: Option<Arc<TypedTrees>>`, `excluded_authored_selection_occurrences`, `roots: TypedTreeRoots`, `tables: TypedTreeTables`, `symbols: SymbolTable`, service reach tables, plan-laid layouts/target layouts/schema fields, access view/lifecycle plans, wire plans, `machine_specializations`, captured machine apps/states, trait/data implementation applications, `dynamic_slots`, `target_machine_families`, `evidence_forwardings`, `proof_output_calls`, `declaration_indexes`, `ranking_expression_custody`, `pending_const_range_endpoints: HashSet<ExpressionHandle>` (lib.rs:1385, a `HashSet`), constant recipe tables, callable identity interner + memo, `type_identities`.
- `TypedTreeTables` lib.rs:1447-1500: ~50 `Arena<...>` columns (const declarations, `DataDefinitionTable`, type aliases, data members/payload fields, proof facts, propositions, mathematical defs, machines, states, traits, conformances, signatures, `expression_table`, `statement_table`, `type_reference_table`, wire schemas/members, product operand applications etc.). `TypedTrees` derefs to `TypedTreeTables` (lib.rs:1792-1807); `DerefMut` calls `invalidate_state_declaration_index()` on every mutable deref (lib.rs:1804).
- `TypedTreeRoots` lib.rs:1503-1518: 14 `HandleSpan` roots; `with_roots` lib.rs:1521 fills 5 and defaults the rest (`type_aliases: HandleSpan::empty()`, others `default()`).
- `ProgramIdentity` lib.rs:1326-1345: counter-allocated u64; `PartialEq` always returns `true` (lib.rs:1341-1345) and its `Record` impl writes nothing and reads a fresh `default()` id (lib.rs:1831-1843). So two programs always compare equal on identity, and a deserialised program gets a new id.
- Macros: `typed_tree_table!` lib.rs:564, `declaration_columns!` 585, `span_columns!` 613 (generators for span accessors/appenders).
- `FrozenTypedTrees(TypedTrees, OnceLock<Arc<PreparedBoundarySignatures>>)` lib.rs:940; `From<TypedTrees>` calls `seal_declaration_indexes` (lib.rs:954-958); `into_inner` calls `unseal_declaration_indexes` (948-950); `boundary_signatures()` lazily prepares (943). Eq compares only the program (970-976). Two unit tests lib.rs:1006-1027.
- `boundary_values` module lib.rs:40-422: `BoundaryValuePreparation` (62) builds `BoundaryValueRecipes` for a type; entry `value_type` (101), `data` (124); private `type_recipe` (133), `data_recipe` (209), `record` (254), `record_geometry` (312). `foreign_reference_uses_descriptor` (389) and `substitute_boundary_type_reference` (404).
- `source_review` module lib.rs:423-466: plain record types `SourceTypeApplication`, `SourceRecordCorrespondence`, `SourceFieldCorrespondence`, `SourceLayoutPlanCoordinate`, `SourceLayoutRealizationCoordinate`.
- `direct_calls::collect` lib.rs:480-554 -> `DirectCallDeclarations<SymbolHandle>`.
- `authored_targets::authored_machine_target` lib.rs:645-693; `declared_outside_product_scope` 695.
- `build_selections::build_provider_requirement_scope` lib.rs:710-750.
- `canonical_values::materialize` lib.rs:783-841, `record` 843-901, `integer_value` 903-918, `TypedTrees::materialize_canonical_const_value` 921.
- `run_custody::EmptyRunReleaseDeclaration::authenticate` lib.rs:1054-1141.
- `selection_exclusions::AuthoredSelectionExclusions` lib.rs:1149-1182 (sorted+deduped Box slice + fingerprint); `symbol_rebinding::SymbolRebinding` lib.rs:1247 (two slices `execution`, `authored`; `Deref` yields the execution pairs, lib.rs:1289-1295).
- Free fns: `canonical_const_declaration_value` 1680, `integer_const_declaration_extent` 1705, `static_const_argument_spelling` 1714, `static_const_value_identity` 1748, private `canonical_static_const_spelling` 1759, `static_const_identity_from_spelling` 1770, `remapped` 1809, `escape` 1820.
- Methods on `TypedTrees` lib.rs:1547-1671: `product_declarations` (1548), `machine_specialization` (1552), `checks_authored_selection` (1561), `symbol_toolchain_relative_path` (1578), `authored_service_reach_rows_for` (1590), `with_roots` (1599), `ranking_expression_custody_for` (1651), `snapshot*` (1660-1670).
- Re-exports at lib.rs:1029-1034, 1144, 1297-1318, 1845-1870 (including `pub mod typed_trees { pub use crate::*; }` lib.rs:1869).

**Inputs/outputs.** Reads `TypedTrees` tables and `symbols` for lookups; the boundary-values code produces `BoundaryValueRecipes` (from `language_semantics::boundary_values`), `direct_calls::collect` produces `DirectCallDeclarations`; `materialize` mutates the expression table of a `&mut TypedTrees` and returns an `ExpressionNode`.

**How it decides.**
- `BoundaryValuePreparation::value_type` memoises recipes by normalised `TypeIdentityId` (lib.rs:107-121, linear `Vec` find, not a map); the substitution of boundary type parameters is keyed on symbol OR name: `(binding.parameter_symbol.is_valid() && == symbol) || binding.parameter_name == name.as_str()` (lib.rs:416-419) -> a spelled-name match can substitute even when the symbols differ.
- `type_recipe` branches on type-reference node variants (lib.rs:152-206): primitive -> Integer/Float by `scalar_byte_size`; `Constrained` -> recurse into base; `Reference` -> `descriptor` from `foreign_reference_uses_descriptor` (slice/dyn trait only); `Slice`/`DynamicTrait` -> descriptor reference; literal-length `FixedArray`; `Named` -> `data_recipe`; `Generic` -> closed data definition or Rejected; non-literal array, `ConstExpression`, `Unit` -> Rejected. Float is chosen only when the primitive is `F32|F64` (lib.rs:141).
- `data_recipe` for `BoundaryOpaque` supply mode looks up a representation by `representation.opaque == symbol` (lib.rs:227-231) and uses its position as `selection_ordinal`; none selected -> Rejected (233-236). Recursion is detected by `visiting` stack (222, 261).
- `record_geometry` rejects when plan-laid offsets count differs from runtime field count (lib.rs:324-333); when it meets a non-field member it pushes a Rejected field and `break`s (lib.rs:335-345; message "case data ... is not yet classifiable as a boundary value; pass it by reference"). Stored integer width must be non-zero multiple of 8 and <= 64 bits (358-361).
- `authored_machine_target` (lib.rs:645-693) is keyed on machine symbol identity: unique exact machine row, unique family member keyed `member.machine == symbol`, specialization origin chain (`instance == symbol` -> `template`), then `machine.target` if declared outside product scope (`dependency_scope != Product`, lib.rs:695-700). Loop bound = specializations + machines + 1 (649); exceeding returns an Err "contain a cycle".
- `EmptyRunReleaseDeclaration::authenticate` (lib.rs:1054-1141): decides a machine is "the empty-run release" by SOURCE: toolchain origin, no package identity, path relative to package root equals `run.omg`, the file bytes equal an `include_bytes!` of `source/library/core/run.omg` (lib.rs:1041-1044, 1078), and normalised hermetic identity string equals `"toolchain::Run::retire_empty"` (1079-1083). Any mismatch returns `Ok(None)` (not an error); once identity matches, any envelope change (body present, supply mode != `AdmissionClaim`, lifetimes, spelling, suspends, blocks, target, authored target, family interface, trait conformances, invokes, installed reach) returns `Err` (lib.rs:1088-1116), exactly one state and one non-const, non-mutable, non-erased parameter (1117-1135).
- `checks_authored_selection` (lib.rs:1561-1576): a selection is checked unless excluded by occurrence id; when a `product_declaration_frontier` exists it is checked only if its source file is Toolchain-origin or `DependencyScope::Build`, or has no source file (`is_none_or`).
- `canonical_values::materialize` (lib.rs:783-841) returns `None` for Float; Integer requires `type_name == primitive.name()` (792); Array requires `type_name == display_type_reference(declared_type)` (809, display string compare); Record/Variant require `definition.name == type_name` (858) and field names equal in order (889).
- `static_const_argument_spelling` (lib.rs:1714-1746) refuses when the argument has a type reference, application or evidence projection; otherwise literal, symbol-backed const, or single-segment path parsed via boolean spelling or `CanonicalConstValue::from_atom`.
- `remapped` (lib.rs:1809-1818) linear find over pairs; unmapped symbol is returned unchanged.

**Cases by shape.**
- `record_geometry`: non-field member (variant case data) rejected, quote lib.rs:338 "case data `{name}` is not yet classifiable as a boundary value; pass it by reference".
- `type_recipe` rejects unit, const expression, non-literal-length arrays (lib.rs:196-205).
- `static_const_argument_spelling`: only `[name]` single-segment paths (lib.rs:1739).
- `canonical_values::materialize`: Floats never materialise (lib.rs:789).
- `authored_machine_target` handles only the four lookup routes above.

**Same job elsewhere.**
- Spelled-name type-parameter substitution `substitute_boundary_type_reference` (lib.rs:404) parallels the symbol-based substitution done by `callable_applications::CallableSubstitution` (exported lib.rs:930-933; suspected, not read in this unit).
- `static_const_identity_from_spelling` (lib.rs:1770) builds `named(canonical-const(type(..),encoding(..)))` strings via `escape` (lib.rs:1820); a likely sibling of identity-text builders in `type_identity.rs` (suspected, not read).
- `EmptyRunReleaseDeclaration::authenticate` and `SymbolRebinding` are a single mechanism each (see below).

**Unused / one-caller / shared.**
- `EmptyRunReleaseDeclaration`: grep `EmptyRunReleaseDeclaration` over `omega-rust` = 7 occurrences in 4 files (3 in lib.rs; 1 validation/calls.rs; 1 checks_content.rs; 2 execution_terminal_unit_control.rs).
- `BoundaryValuePreparation`: 4 occurrences (2 lib.rs; `omega/compilation/backend/layout/src/boundary_values.rs` 1; `omega/compilation/realization/provider-planning/src/program_entry.rs` 1).
- `authored_machine_target(`: 7 occurrences (1 lib.rs definition; 6 in omega packages/provider-planning).
- `build_provider_requirement_scope|materialize_canonical_const_value|integer_const_declaration_extent|static_const_value_identity|static_const_argument_spelling`: 24 occurrences across 14 files (grepped as one alternation; counts per function not split).
- `direct_calls::collect|foreign_reference_uses_descriptor`: 5 occurrences in 4 files (2 in lib.rs).
- Documented-as-shared: `BoundaryValuePreparation` has 2 users outside lib.rs (layout, provider-planning).

**Stated limits.** lib.rs:338 "not yet classifiable as a boundary value; pass it by reference" (case data by value at a boundary). lib.rs:1385-1386 `pending_const_range_endpoints` is a `HashSet` (data-oriented rule says dense Vec, noted only as fact).

**Fail-open risks.**
- `substitute_boundary_type_reference` name match (lib.rs:416-419): a `Named` type whose symbol differs but whose spelling equals a binding's `parameter_name` is replaced. Wrong program: a same-named unrelated type is substituted by the binding's actual. Status: open (did not trace callers; two callers outside lib.rs only via BoundaryValuePreparation).
- `EmptyRunReleaseDeclaration::authenticate` returns `Ok(None)` for non-matching machines (lib.rs:1058-1086): that means "not the empty-run release", which callers then treat as an ordinary machine; fail-closed in the sense that it grants nothing extra. Status: stated by shape (identity by bytes+path).
- `checks_authored_selection` `is_none_or` (lib.rs:1572): a selection whose span has no source file is treated as checked when a product frontier exists. Status: open.
- `ProgramIdentity::eq` always true (lib.rs:1341): equality of `TypedTrees` (derived PartialEq, lib.rs:1347) ignores identity; this is a design of equality, not acceptance. Status: open.
- Otherwise none seen (lookups return `None`/`Err`/`Rejected`).

---------------------------------------------------------------------------
## 5. references.rs (3710 lines)

**Purpose.** Walks a `TypedTrees` program and reports which symbols, expressions, statements, type references, proof facts, static-call applications and authored selections each declaration refers to. It is one visitor trait (`TypedReferences`) with many collectors, plus a pass that stamps lexical owner and declaration onto call static-argument applications. Header comment references.rs:1-4.

**Shape.**
- Trait `TypedReferences: Extend<SymbolHandle>` references.rs:1006-1052: default-method hooks (`declaration`, `expression`, `type_reference`, `statement`, `call_arguments`, `enter_lexical_scope`, ... ), each returning `true` ("descend") by default; `derived_declaration_references` (1017) defaults `true` and gates derived (not authored) edges.
- Walkers (free fns): `visit_program` 1709, `visit_declarations` 2295, `visit_machine` 3316 / `visit_machine_with_sidecars` 3320, `visit_signature` 2743, `visit_type_reference` 2794, `visit_expression` 2898, `visit_statement` 3162, `visit_call_expression` 3074, `visit_static_argument` 2067, `visit_static_machine_bindings(_inner)` 2084/2092, `visit_closed_conformance(_inner)` 2169/2177, `visit_callable_symbol` 1985, `visit_declaration_dependencies` 2226, `visit_data` 2614, `visit_trait` 2702, `visit_const` 2583, `visit_alias` 2567, `visit_type_parameters` 3265, `visit_mathematical_type` 3521, `visit_constant_evaluation_entries` 3504, `visit_product_operand_inputs` 1720, `visit_proof_fact` 2508.
- Collectors: `ReferenceCollector` 1066 (private; HashSets of symbols/callables/descriptor symbols/expressions/statements/types/facts), result `ProgramReferences` 1054; `TypeOccurrences` 1287 (scoped (owner, handle) tables `seen_types`/`seen_expressions`/`seen_mathematical_types`: `Vec<Vec<(owner,handle)>>` indexed by arena index, 1299-1314); `ProofFactCollector` 1928; `ProgramOperandOwners` 1219; `ProgramApplicationOccurrences` 1267; `AuthoredTypeOccurrences` 569 (in `source_occurrences`).
- `DeclarationSelection` 1426: sorted `Vec<SymbolHandle>` plus an `excluded` flag (complement representation: `all()` = excluded with empty list); `contains/intersect/excluding/include/from_symbols`.
- `MachineSidecarIndex` 1651: sorted (symbol,row index) vectors for `machine_specializations` and `authored_service_reach_rows`, built at 1675, queried with `partition_point` (1660-1672).
- `ReferenceClosure` 1839: incremental closure with `visited_declarations`.
- `StaticArgumentCall` 934 (Expression or Statement call) with accessors; `StaticArgumentRole` 928 {ValueDependency, DeclarationDescription}; `StaticArgumentClassifier` fn type 1004.
- `source_occurrences::call_applications` references.rs:34-376: `TypedTrees::retain_static_argument_contexts` 39 (program-wide pass), `retain_call_static_argument_context` 159 (single call), private `retain_static_operand_sources` 241, `retain_application_operand_sources` 307; helpers `direct_declaration` 356, `implementation_namespace` 363, `has_region_telescope` 370.
- `authored_type_occurrences` 387 (type occurrences for a set of owners, using lexical ancestors and "description" headers), `collect_authored_program_type_occurrences` 381, `lexical_description` 491, `consistent_header` 551.
- Tests: `authored_type_occurrence_tests` 606-919, `conformance_occurrence_tests` 3550-3710.

**Inputs/outputs.** Reads the whole `TypedTrees` (all tables). Outputs: `ProgramReferences`, `Vec<TypeOccurrence>`, `DeclarationSelection`, owner tables, proof-fact lists. The only mutating code is `retain_*` (mutates `expression_table` static machine applications, type-reference table owners, `statement_table` call nodes).

**How it decides.**
- Visit pruning is keyed on handle identity: `ReferenceCollector::expression/type_reference/proof_fact` return false when the handle was already inserted (references.rs:1154-1166); `TypeOccurrences` dedups on (owner symbol, handle) (1299-1314). So the same expression reached under two different owners is reported twice for `TypeOccurrences` and once for `ReferenceCollector`.
- `ReferenceCollector::declaration` skips declarations not in `selection` (1147-1153); `DeclarationSelection::contains` keys on `(arena_index, generation)` of the symbol (1445-1452).
- `call_arguments` in `ReferenceCollector`: when a classifier fn pointer says `DeclarationDescription`, the call's static arguments are collected as descriptor symbols and NOT visited as references (1128-1143); with no classifier it returns true (everything is a normal reference).
- `visit_callable_symbol` (1985-2051) adds derived edges only if `derived_declaration_references()`: access lifecycle plans where `callable == symbol` (linear scan of `access_lifecycle_plans`, 1995-2005) and `closed_requirement_application(symbol)`.
- `retain_static_argument_contexts` (39-150) picks statement calls to stamp using a four-way OR: authored machine application valid, non-empty `machine_arguments`, `has_region_telescope(target)`, `implementation_namespace(receiver)`, or (no static parameter, no dispatch, and the target resolves to a direct declaration) (references.rs:96-103).
- `retain_static_operand_sources` (241-305) marks an application `complete=false` when it has an evidence projection or a member, or a conformance operand whose type/lifetime parameter counts differ from the supplied application (282-290); arguments with no symbol, no literal and no canonical const spelling also make it incomplete (297-302). It returns `Result<bool,&str>` and callers ignore the bool (`?;` at 111, 168, 322, 340-343).
- `retain_call_static_argument_context` returns `Ok(())` without doing anything if the expression is not a call (164-166) or if no arguments / region telescope / implementation namespace / direct declaration apply (207-213).
- `authored_type_occurrences` (387-481): resolves each owner to a lexical "description" by walking symbol parents (`program.symbols.get(current).parent`, 403) and matching `SymbolKind` Machine/Data/Trait/TypeAlias/Const (510-548); `consistent_header` (551-567) accepts the current declaration, else the first copy only if all other copies agree (`PartialEq::eq` for machines/data/traits/aliases; a field-wise comparison for const at 538-544); disagreement returns `Err(())` -> owner skipped silently (`ambiguous` -> `continue`, 420-422). Result filtered to `requested` owners (479).
- `visit_declaration_dependencies` (2226-2293) selects data/trait implementation applications by `lexical_owner == owner` or by `static_machine_application(supplying_application).lexical_owner == owner`; captured machine applications by `conformance_application == selected.application` (struct equality) and lexical owner.
- `collect_expression_source_owners` (1763-1837) keys on the expression's `SourceSpan` being contained in a given list of spans (1804-1806): a source-span match, not identity.
- `visit_statement` / `visit_call_expression` list explicit field families (quotient operation, installed interrupt operation, private layout operation, native code layout, layout query); a new call-side field not listed is not visited (read, 3117-3143).

**Cases by shape.** `visit_type_parameters` (3265-3314): `Machine{contract}` -> Structural/Nominal/`RequirementIdentity` (None, nothing visited, 3295); `Type` -> nothing (3310). `visit_expression` match (2927-3013) is exhaustive over `ExpressionNode`. `visit_statement` match (3173-3262): `Assembly`/`Expression` visit nothing beyond the roots (3174, 3261); `Assignment` only visits `receiving_type` when derived (3222-3226).

**Same job elsewhere.**
- Statement-call and expression-call static-argument stamping are two copies: `retain_static_argument_contexts` statement branch (references.rs:87-148) vs `retain_call_static_argument_context` (159-239). They duplicate the "set `lexical_owner`, `binding_declaration`, retain application sources, write back `authored_machine_application`/`machine_arguments`" sequence (read). The expression variant additionally checks `!call.requirement_application.is_valid()` in the declaration test (173) which the statement variant lacks (112-113).
- The installed-interrupt visiting block appears twice, in `visit_call_expression` (3123-3134) and `visit_statement` (3200-3211).
- The "captured machine application whose `conformance_application == selected.application` and owner matches" loop appears twice in `visit_declaration_dependencies` (2245-2257 and 2276-2291).
- `ProgramReferences`/`TypeOccurrences` machine/sidecar paths: `visit_machine_with_sidecars` has a sidecar branch and a non-sidecar linear-scan branch for specializations (3390-3402) and service reach rows (3454-3468); both do the same job.
- Suspected (not read): `has_region_telescope`/`direct_declaration` resemble machine lookups in `declaration_tables_and_index.rs`.

**Unused / one-caller / shared (all grepped over `omega-rust`).**
- `ReferenceClosure`: 4 occurrences = 3 in references.rs + 1 in `build-time-evaluation/src/syntax_probes.rs` (one outside user).
- `collect_expression_source_owners|collect_declaration_symbol_dependencies|collect_program_operand_owners|collect_program_conformance_occurrences|program_scoped_proof_facts|program_proof_facts|collect_machine_references|collect_signature_references`: 39 occurrences in 17 files (9 inside references.rs).
- `retain_static_argument_contexts|retain_call_static_argument_context`: 9 occurrences in 5 files (3 in references.rs, 1 in `product_and_trait_declarations.rs`, 3 in 04 `preparation.rs`, 1 in 03 `lowerer.rs`, 1 in 04 `assembly_and_operators.rs`). The doc comment at 156-158 says the program-wide pass "runs once" and other stages "use this for that call" (stated).
- `collect_program_references_with_roles|collect_selected_program_references|collect_program_references(|collect_selected_program_type_occurrences|collect_program_type_occurrences|collect_type_symbol_dependencies|collect_expression_symbol_dependencies|visit_product_operand_inputs|authored_type_occurrences`: 72 occurrences in 28 files.
- Not individually counted: `collect_program_declarations` (internal `DeclarationSelection::from_program`), `ProgramApplicationOccurrences` (pub(crate)).

**Stated limits.** None with "unsupported/remains" wording found. Comment 246-247 "An unresolved child defers its enclosing application until this same context pass runs after ordinary resolution." describes `complete=false` deferral.

**Fail-open risks.**
- `retain_call_static_argument_context` returns `Ok(())` silently for a non-call expression (164-166): an expression that was meant to be a call but is not leaves no application stamped. Traced: caller passes expression handle; effect is "no stamping", so a later "judged like the same call written by hand" step (doc 156-158) would not apply. Status: open (callers in 04 stage not read).
- `retain_static_operand_sources` returns bool `complete` that callers drop (references.rs:111, 168, 226-230 via `retain_application_operand_sources`, 322, 340-343): an application flagged incomplete is still written back. Status: open (I did not trace whether the flag is meant to be consumed; no caller uses it in this file).
- `authored_type_occurrences` silently omits an owner whose headers disagree (420-422), so type occurrences for that owner are absent rather than erroring. The unit test at 890-898 asserts this on purpose ("conflicting" lifetime parameter). Status: stated by test.
- `visit_call_expression`/`visit_statement` do not visit unlisted fields; a collector that depends on completeness (e.g. dead-code or dependency closure) would miss references through such a field. Status: open.
- Otherwise none seen; the walkers only collect.

---------------------------------------------------------------------------
## 3. operator_declarations.rs (3900 lines)

**Purpose.** Defines the typed `OperatorDefinition` record and all the logic that decides which operator declaration a use site refers to: spelling lookup (`+`, `-`, comparisons, `[]`, ranges), named operator calls, operand-type matching with type/const parameter binding, trait-requirement operators (bound, scoped, selected), satisfied-operator resolution for machine conformances, primitive float/integer operator rules, and closed operator realization applications with a canonical byte encoding. Header comment operator_declarations.rs:1-5. Everything lives in one `pub mod operator { ... }` (7-3900), re-exported through `crate::declarations::operator` (lib.rs:1849 `operator`).

**Shape (submodules of `operator`).**
- `OperatorDefinition` 3870-3883: `is_public`, `is_boundary`, `symbol`, `name: HandleSpan<Identifier>` (path members), `lifetime_parameters`, `type_parameters`, `parameters`, `return_type`, `contracts`, `spelling: Option<OperatorSpelling>`, `token_count`, `home_domain`. `OperatorConstBinding {symbol, value: CanonicalConstIdentity}` 3864. `SpelledOperator {operator, domain}` 3886. `declaration_by_symbol` 3891 (linear `find` over `all_operators()` chain `machine_token_bindings()`).
- `applications` 8-606: `ClosedOperatorApplicationArgument` {Type, Const} 24; `SymbolicOperatorTypeApplicationArgument` 37; `ClosedOperatorRealizationApplication` 44; `closed_operator_application_from_bindings` 49; `closed_operator_application_for_operands` 121; `closed_indexed_operator_application_for_operands` 129; private `closed_application_for_operands` 143; `symbolic_operator_type_application_for_operands` 251; `closed_operator_realization_application` 327; `canonical_closed_operator_realization_bytes` 470 (builds a length-prefixed, sorted byte encoding; `operator_symbol_identity` 588 uses package digest hex or "unmanaged::"); `closed_boundary_application_type` 406 / `closed_boundary_application_nominal` 452.
- `indexing` 607-775: `builtin_index_read_is_spelled` 618, `resolve_indexed_spelling_for_operands` 640, `shared_collection_elements` 667, `borrowed_collection_match` 704, `unconstrained_type_reference` 760.
- `named_calls` 776-1161: `resolve_named_call` 781, `named_expression_call_candidates` 848, `named_statement_call_candidates` 1005, `selected_named_*_call_has_no_runtime_receiver` 919/1086, `expression_static_receiver_segments` 952, `statement_static_receiver_segments` 1123.
- `operand_signatures` 1162-1277: `operator_operand_signature` 1169, `TypeParameterNormalizer` 1241 (assigns `$0,$1..` by first occurrence).
- `primitive_float` 1278-1388: `primitive_float_binary_semantics` 1307, `is_sealed_toolchain_float_operator` 1356, `is_default_primitive_operator` 1378.
- `primitive_integer` 1389-1442: `builtin_integer_result_carrier` 1393, `integer_peer_operand_carrier` 1428.
- `satisfied_operators` 1443-1685: `resolve_satisfied_{boundary,checked}_operator_for_conformance` 1452/1460, `resolve_specialized_checked_operator_application` 1507, `satisfied_operator_candidates` 1542, `operator_realization_static_binders` 1597, `boundary_operator_requirement_identity` 1660.
- `spellings` 1686-2894 (incl. test module `token_roster_tests` 2351-2893): `resolve_spelling` 1700, `spelled_candidates` 1709, `resolve_spelling_for_operands/_operand_types` 1816/1834, `has_builtin_spelled_expression_meaning` 1847, `operand_bindings_with_indexed_collection` 1978, `bindings_satisfy_known_multiplicities` 2053, `closed_multiplicity_carrier` 2076, `declared_domain_constraints(_match)` 2259/2276, `declared_domain_constraint_matches` 2309, `OperandType` {Reference, Primitive, Unknown} 1800.
- `trait_operators` 2895-3419: `trait_operator_matches_application` 2910, `trait_operator_operand_bindings` 2942, `bound_trait_operator_meanings` 3019, `trait_scope_operator_meanings/_exists` 3098/3123, `visit_trait_operator_meanings` 3141, `selected_trait_operator_meanings/_exists` 3253/3291, `trait_operator_operand_signature` 3328, `abstract_requirement_may_have_operator_bound` 3361.
- `type_matching` 3420-3809: `type_reference_matches` 3429 (calls policy with `allow_name_fallback = true`), `type_reference_matches_with_policy` 3450, `closed_const_identity_from_type_reference` 3713, `nominal_type_identity_matches` 3745, `expected_bindable_symbol` 3770, `expected_type_parameter` 3793, `fixed_array_lengths_match` 3677.

**Inputs/outputs.** Reads type reference table, operator/machine/trait/domain tables, `operator_spelling_rows` index (spellings.rs:1717, from declaration index), symbol table (visibility via `source_reference_may_select_symbol` / `source_reference_can_see_symbol`), type identity text. Outputs are `Vec<SpelledOperator>`, bindings (`OperatorOperandBindings`), closed application argument lists, canonical bytes, bool answers. Nothing here mutates the program.

**How it decides.**
- Spelling lookup (1709-1761): candidates come from the per-spelling row index; kept if the use site "may select" the symbol OR (can see it AND it is a default primitive float operator, 1743-1754); then filtered by receiver type (`operator_matches_receiver` 2331) or operand types.
- Operand matching (1978-2051): by position; `Unknown` operand always matches (1997); `Primitive` operand matches if the expected parameter type is not a primitive or is the same primitive (1998-2003) (so non-primitive expected params accept any primitive operand); `Reference` operands go through `type_reference_matches` plus `declared_domain_constraints_match` on both the original and the read-adjusted types, and `bindings_satisfy_known_multiplicities`.
- `type_reference_matches_with_policy` (3450-3636): binds expected type parameters (3490-3514), const parameters (3477-3489), structural cases for references (access equal, lifetime ignored: `lifetime: _` 3524/3529), constrained (strips constraint on either side, 3535-3548), arrays, slices, named, generic; other shapes -> `false` (3634). A `Named` actual vs `Generic` expected goes through the data definition's `generic_instance` (3576-3585).
- Name fallback: `nominal_type_identity_matches` with `allow_name_fallback=true` accepts `actual_name == expected_name` even when symbols differ (3752-3755); `find_type_parameter` matches a parameter by `parameter.name == *name` (3673). The plain `type_reference_matches` always passes `true` (3446); the `_with_policy(.., false)` callers are the "closed application" functions (applications 188-196, 280-288, 382-390; trait_operators 2977-2986). With `false`, symbols must both be valid and equal, or both invalid with equal primitive names (3756-3767).
- Named calls (781-812): if `target_symbol` valid, direct lookup in operators; else candidates are filtered on last path member `== target_name` and prefix equal to receiver segments (string compare, 1046-1070) and arity (1072-1084); all candidates must be "the same authored declaration" = identical provenance span, non-empty span, same file path (814-832) else `None`.
- Static receiver namespace test (952-1003, 1123-1160): a receiver is a static namespace if its symbol kind is Module/BuiltinType/Data/Domain/Machine/Trait, or, when its symbol is invalid, if some operator's path prefix equals the receiver name segments (string compare).
- `primitive_float_binary_semantics` (1307-1354) is keyed on spelled path `["Float", "add"|"subtract"|...]` (1311-1329) plus primitive F32/F64 on both operands and result type. `is_sealed_toolchain_float_operator` (1356-1376) is keyed on `normalized_hermetic_symbol_identity == "toolchain::{ns}::{name}"` AND toolchain relative path equal to `numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE`. `is_default_primitive_operator` (1378-1387) adds `is_boundary && is_public && !home_domain.is_valid()`.
- Satisfied operator (1468-1505): candidate filter by `operator_path_matches` = exactly `[owner, member]` with `owner == conformance.name` and `member == requirement` string compare (1649-1658), parameter const/mut flags and type identity under positional binders `$n` (1597-1647); when `requirement_symbol` is valid pick by symbol (1483-1487), otherwise by source-scoped lookup of path string `"{name}::{requirement}"` (1488-1501), requiring a unique result.
- `resolve_specialized_checked_operator_application` (1507-1540): exactly one specialization row for the machine, exactly one retained row matching `(namespace, requirement)` path, and a fresh reconstruction equal to the retained row; else `None`.
- `abstract_requirement_may_have_operator_bound` (3361-3418): returns `true` (conservative "may") for several unresolved cases: parent is a trait symbol with no definition (3371-3372), signature/requirement count mismatch (3375-3379, 3392-3395), unknown trait in the walk (3387-3389).
- `builtin_integer_result_carrier` (1393-1426): fixed shapes (see below).

**Cases by shape.**
- `closed_application_for_operands` (143-249): refuses lifetime-parameterized operators, any operator type parameter that is not `Type` or `Const` (153-161: Machine/Implementation/Proposition/Value-less kinds -> `None`, 243-245), requires operand count equal to parameter count. `Value` kind is accepted at 231 but rejected at the early check (153-159 lists only Type|Const), so the `Value` arm in 230-231 is not reachable from the early check (read; inferred).
- `symbolic_operator_type_application_for_operands` (251-325): only all-`Type` operator parameters, no const bindings (296), actual operands must all be known (`is_some_and`, 279).
- `closed_operator_application_from_bindings` (49-119): no lifetime parameters, binding count must equal parameter count (57).
- `closed_boundary_application_type` (406-450): Reference without lifetime, literal-length arrays, slices, Generic without lifetime args, Named that is primitive/const atom/integer text or `BuiltinType|Data` symbol, Unit; refuses Constrained, ConstExpression, DynamicTrait, non-literal arrays (417-424).
- `builtin_integer_result_carrier` (1393-1426): comment-free fixed table: `Addr` combos only `Add(Addr,U64)`, `Add(U64,Addr)`, `Sub(Addr,U64)` -> Addr, `Sub(Addr,Addr)` -> U64; shifts give left type; Add/Sub/Mul/Div/Mod/And/Or/Xor require equal operand types; everything else `None`.
- `is_default_primitive_operator`/`primitive_float_binary_semantics`: only ten named Float requirement names (1317-1328) for F32/F64.
- `closed_multiplicity_carrier` (2076-2165): depth limit 64 (2081), only Unit, constrained, lifetime-free reference, literal-length array, slice, closed data / generic / dyn trait with no lifetime params.
- `shared_collection_elements` (667-702) only handles expected `&[T]` (shared slice reference, 673-688) against actual array/slice (optionally under one reference); `borrowed_collection_match` (704-758) only shared or mutable references.

**Same job elsewhere.**
- Expression and statement named-call resolution are two parallel copies: `named_expression_call_candidates` 848-893 vs `named_statement_call_candidates` 1005-1044 (statement variant omits `machine_token_bindings` in its `operators()` chain at 1029-1031 and 1010-1014, expression variant includes it at 856, 880); `expression_static_receiver_segments` 952-1003 vs `statement_static_receiver_segments` 1123-1160; `selected_named_expression_call_has_no_runtime_receiver` 919 vs `selected_named_statement_call_has_no_runtime_receiver` 1086 (read).
- `operator_operand_signature` (1169-1198) and `trait_operator_operand_signature` (3328-3359) are the same normalize-then-join routine on different parameter sources.
- `closed_operator_application_for_operands` (121) vs `closed_indexed_operator_application_for_operands` (129) share `closed_application_for_operands`.
- Binder-name construction `format!("${ordinal}")` appears in `TypeParameterNormalizer::bindings` (1273) and twice in `operator_realization_static_binders` (1610, 1615).
- Static-receiver namespace SymbolKind list (Module|BuiltinType|Data|Domain|Machine|Trait) is repeated at 968-978 and 1128-1137.
- `type_reference_matches_with_policy` is a second type-equality-with-binding relation beside the canonical type identity comparison in `type_identity.rs` (used by satisfied operators at 1577-1583); suspected overlap, not read here.
- Suspected: the Float name-keyed rule (1311-1329) is mirrored in Psi 05 `values_scalar.rs` / `execution_scalar_computations.rs` (grep `primitive_float_binary_semantics` shows users there; not read).

**Unused / one-caller / shared (grepped over `omega-rust`).**
- `closed_indexed_operator_application_for_operands|symbolic_operator_type_application_for_operands|closed_operator_application_from_bindings|canonical_closed_operator_realization_bytes`: 19 occurrences in 7 files (8 inside this file incl. re-export and definitions).
- `resolve_named_call(|resolve_spelling_for_operands(|operator_operand_signature|trait_operator_operand_signature|boundary_operator_requirement_identity|is_sealed_toolchain_float_operator|builtin_integer_result_carrier|integer_peer_operand_carrier`: 91 occurrences in 27 files.
- `selected_named_{statement,expression}_call_has_no_runtime_receiver|resolve_named_expression_call|resolve_specialized_checked_operator_application|resolve_satisfied_{checked,boundary}_operator_for_conformance`: 83 occurrences in 30 files.
- `allow_name_fallback|type_reference_matches(`: 27 occurrences, all in this file (name fallback is confined here).
- Not individually counted: `resolve_named_call` alone (alternation above), `has_builtin_spelled_expression_meaning`, `scalar_operand_read_type`, `declared_domain_constraints`.

**Stated limits.** No comment says "unsupported"; limits are expressed as `None`/`false`. Test module comment string at 2654 "use-site queries share only raw rows" and 2799 "another row at the old position cannot become that owner" describe cache/index behaviour in tests only.

**Fail-open risks.**
- `type_reference_matches` (3429-3448) always sets `allow_name_fallback = true`: two `Named` types with different symbols but the same spelling are treated as matching (3752-3755), and a type parameter is found by name when `symbol` differs (3673). Wrong program: a user type named like an expected operator parameter type, from another declaration, would be accepted as a spelled operator operand (matches via `operand_bindings_with_indexed_collection` 2020, `operator_matches_receiver` 2339). Status: open (not traced which callers pass in same-named distinct data types; the strict variant is used only in the closed-application functions).
- `OperandType::Primitive(p)` matches any non-primitive expected parameter (`is_none_or`, 1998-2003). Wrong program: a primitive operand against an operator parameter of data type could be reported as matching. Status: open (callers outside unit not traced; `OperandType::Primitive` constructed outside this file only where grep shows users; not counted).
- `abstract_requirement_may_have_operator_bound` returns `true` on unresolved structure (3371-3372, 3377-3378, 3387-3395), which then suppresses the "builtin spelled meaning" decision in `has_builtin_spelled_expression_meaning` (1910-1914, it requires `!`this). So a malformed trait table makes the function say "an operator bound may exist" and refuses the builtin meaning (fail-closed direction for that check). Status: traced within unit.
- `declared_domain_constraints_match` returns `true` when the expected type has no domain constraints (2265-2267): intended; a constrained actual against an unconstrained expected passes. Status: stated by shape.
- `declared_domain_constraint_matches` (2309-2329): when both have valid semantic ids they compare ids only (2314-2316) and ignore symbol/arguments. Status: open.
- `resolve_named_call` with a valid `target_symbol` returns `operator_definition_by_symbol` with no visibility or arity check (789-791). Status: open.
- Otherwise none seen.

---------------------------------------------------------------------------
## 4. product_and_trait_declarations.rs (4130 lines)

**Purpose.** Two declaration families. (a) `product_operand` (lines 7-1580): records of designated "Product" operands (the declaration paths a Build selection names, e.g. `select_provider<Console, NativeConsole>`), the static environments captured for Build helpers, and helpers that resolve those operand paths to declaration symbols. (b) `trait_definition` (1582-4130): the typed `TraitDefinition`/`Conformance` records plus `AppliedTraitScope`, the structure that carries a trait's type/lifetime arguments through nested applications (refinement, bounds, enclosing data), and the logic to resolve requirement signatures, identities and refinement views against it. Header comment lines 1-5.

**Shape.**
- `product_operand::input_selections` 8-184: `ProductArgumentStep` {Invalid, Argument(u32), Member} 19, `ProductTypeSelectionSlot` {Declaration, ArrayExtent} 27, `ProductInputSelectionTarget` 33, `ProductInputSelection` 44 with `selected_symbol` 51 (walks a recorded argument path through `StaticMachineArgument`s, or checks a type reference is a retained input via a `TypedReferences` visitor `Inputs` 97-129).
- `applications` 190-313: `ProductApplicationDemand` {FormationOnly, Selected} 201, `ProductOperandApplication` 208 (operand, optional `Arc<ProductStaticEnvironment>`, owner, role, declaration, parameters, demand, argument, input selections, `RetainedTraitScope`, supplying application, optional closed implementation); `product_operand_application_identity` 225, `prepared_product_operand` 263 (linear `find_map` over `product_operand_applications`), `retain_dependency_product_application` 281, `retain_product_operand_application` 296 (dedups by `(operand, environment, owner, role)` equality).
- `occurrences` 316-399: `ProductSelectionOccurrence` 327, `collect_product_selection_occurrences` 334 (visitor `Census` 358 over each machine/state's statement expression roots; takes calls whose `build_selection_request` is `BuildSelectionRequest::Declaration`).
- `static_environment` 402-1218: `capture_static_environment` 411 (builds `ProductStaticEnvironment` for a Build helper machine from its unique `MachineSpecialization`; builds an identity byte string tagged `omega-source-static-environment-v1\0` at 524), `capture_closed_operand` 579, private `Capture` 601 with `operand` 645, `ty` 767, `constraint` 913, `expression_selections` 939, `expression` 975.
- Free fns 1221-1579: `is_declaration_operand` 1243, `resolve_product_operand` 1266, `resolve_product_path` 1278, `product_candidates` 1304, `ProductEntryDeclaration` 1369 (`from_declared_owner` 1386, `from_machine` 1432, `erased_requires_inputs` 1420), `resolve_build_request_author` 1457, `native_binding_formation` 1475 (SHA-256 over relative path, source bytes, callable identity, operand path, span), `same_product_operand_source` 1537, `same_product_request_source` 1564.
- `trait_definition::applications` 1583-4026: `AppliedTraitScope` 3574 {`declaration`, `substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>`, `substitution_scope_starts`, `argument_owner`, `region_scopes`}; constructors `new` 3721, `from_type_reference` 3583, `from_conformance_application` 3629, `from_conformance` 3664, `from_machine_conformance` 3691, `abstract_scope` 3759; traversal `visit` 3974 / `visit_parents_first` 3978 / `visit_inherited` 3986; `AppliedTypeReference` 3478 with `project_type_reference` 3539; identity methods (regions module 1593-2017, e.g. `portable_application_identity` 1714, `application_identity` 1959, `with_identity_scopes` 1911); `under_invocation` 1633; `keeps_declared_signatures` 1697.
- `enclosing` 2057-2260: `project_enclosing_data_type_reference` 2066, `with_enclosing_data_identity_scopes` 2117, `data_application` 2182, `append_data_application` 2233.
- `evidence` 2261-2608: `GenericBoundRequirement` 2267, `evidence_requirement_declaration` 2273, `requirement_callable_parameters` 2325, `machine_callable_parameters` 2353, `callable_conformance_bounds` 2374, `declaration_conformance_bounds` 2391, `callable_conformance_bound` 2412, `conformance_target_requirement` 2434, `evidence_bound_requirement` 2464, `collect_requirement_namespace` 2585.
- `refinement_views` 2610-2777: `refinement_prefixes` 2620, `callable_contract_key` 2650, `callable_refinement_clauses` 2671, `apply_callable_refinements` 2700 (mutates a `StateSignature`: postconditions, suspends/blocks, termination guarantee, service reach rows).
- `retention` 2778-3035: `TraitApplicationEnvironment` 2790, `RetainedTraitScope` 2801 (Copy handle into `trait_application_frames`), `RetainedTraitApplicationFrame` 2808, `retain` 2918, `RetainedTraitScope::scope` 2995, `visit_references` 2973.
- `source_fields` 3036-3304: `contract_application_roots` 3049 (walks type occurrences through data fields to find `dyn Trait` roots; `SourceFields` 3098, `field_inventory` 3229).
- `TypedTrees` methods 3308-3467: `lexical_conformance_scope` 3309, `machine_conformance_requirement_scope` 3340, `applied_trait_roots` 3375, `applied_requirement_scope` 3442.
- Records 4039-4130: `TraitDefinition` 4040, `TraitRefinementClause` 4055, `TraitRefinementReach` {Inherited, Concrete(row), IndependentBounded(rows)} 4063, `Conformance` 4077, `ConformanceRow` 4091, `ConformanceRowSource` {Inline, Reference, TraitDefault, Synthesized} 4103 (`owns_realization` = all but `Reference`, 4111), `TraitRequirement` 4117, `DynamicSignatureIneligibility` {BoundaryRequirement, RequirementLocalGenerics} 4126.

**Inputs/outputs.** Reads machine specializations, static machine applications and bindings, symbol provenance spans (for identity), the expression/type reference tables, the authored declaration selection ledger, trait/data/conformance tables. Mostly produces owned/borrowed structs. Mutating pieces: `retain_*` product application functions (2 in `applications`), `AppliedTraitScope`'s `TraitApplicationEnvironment::retain` (writes `callable_substitutions`, `callable_lifetime_substitutions`, `trait_application_frames`), `apply_callable_refinements` (mutates a signature and interns service reach rows in `self.service_reach_rows`).

**How it decides.**
- Product operand classification: `is_declaration_operand` (1243-1250): a "bare path" operand = no application, no literal, no evidence projection, non-empty path, and either no type reference or a retained type reference that is a `Named` with symbol equal to `argument.symbol` (1252-1264). The block comment 1227-1242 explains the retained type reference is a record of source scope.
- Path resolution is by source spelling: `resolve_product_operand` joins `argument.path.joined_path()` (1275) and `find_build_subject_declaration_from_source(path, occurrence, candidates, token_bound_machines)` (1296) over a candidate list per `ProductOperandKind` (1304-1366): BoundarySubject = boundary traits + `TopLevelRequirement`-supply machines + boundary operators; Provider = data + conformances; Requirement = requirement states of boundary traits; Data; Representation = conformances; `WireCompatibilityFact` -> `None` (1309).
- `resolve_build_request_author` (1457-1473): identifies the machine by equality of source span (`symbol_source_span(machine.symbol) == Some(author)`), requiring exactly one.
- `same_product_operand_source` (1537-1562): equal path string and equal rejoined source span, plus structural equality of the applications after remapping spans (1554-1558).
- `prepared_product_operand` (263-279): keyed on operand, environment value (`as_deref()` equality), owner symbol and role.
- `capture_static_environment` (411): `Ok(None)` if no specialization for the machine (419-421); `Err` for multiple specializations (422), invalid binding application, `binding_declaration != template` (433), missing source spans. Skips `RuntimeValue` bindings with `continue` (472).
- `Capture::ty` (767-912): rejects any type with transparent alias applications ("cannot yet retain transparent alias application obligations", 778); detects cycles with `active_types`; a `Named` type with invalid symbol whose spelling is a canonical const atom becomes a `CanonicalConst` (791-805); a `Named` data type with `generic_instance` is captured as its generic origin (806-816).
- `Capture::expression` (975-1216): supports Integer, Boolean, Float, String, Name, StructLiteral, ArrayLiteral, UndeterminedNaN, ZeroValue, TypeExpression, Unary (BitwiseNot/LogicalNot only), Binary (all listed ops), Member, Indexed, Cast, Call; anything else returns `Err("Build specialization has a non-static or unmaterialized selected value")` (1197-1200). A call with `native_code_layout` is rejected (1134).
- `AppliedTraitScope::parameter_application` (1863-1878) finds the LAST substitution whose binder symbol equals the parameter (`.rev().find`), then the frame index by `partition_point` on `substitution_scope_starts`. `project_type_reference` (3539-3571) repeatedly replaces a `Named` reference by the last matching substitution (symbol equality) until none matches (loop is unbounded; no cycle guard, read).
- `AppliedTraitScope::new` (3721-3757): refuses when argument counts of type or lifetime parameters differ from the trait's (3727-3730).
- `same_application` (3860-3866): equal declaring symbol AND both `application_identity` are `Some` and equal; if either identity is `None` it is `false`.
- `declaring_scope` (3914-3932): walks the inherited scopes; if two visited scopes of the same trait declaration are not the same application it returns `None`.
- `visit_inherited` (3986-4024): cycle guard via `active` stack of declaration symbols (3993); parent traits that fail to resolve or build a scope are skipped with `continue` (4003-4018).
- `evidence_bound_requirement` (2464-2582): requires receiver be a `ConformanceParameter` symbol that has an authored bound; target must be a `State` symbol; either a prepared closed application or a child of the receiver; ambiguity across reachable declarations -> `Err`; zero matches -> `Err` (2576-2581).
- `apply_callable_refinements` (2700-2775): termination guarantee from clause replaces signature's when clause is `Terminates` and (premises empty OR signature has `NoGuarantee`) (2725-2735); `suspends`/`blocks` are AND-ed only when the clause carries keyword spans (2713-2724).
- `source_fields::visit` (3108-3227): uses `seen: Vec<BTreeSet<..>>` keyed on (owner, reference, enclosing frames) plus an `active` ancestry check by data declaration symbol and frame depth (3197-3205) for recursion. References and slices are not entered (3168-3170).

**Cases by shape.**
- `product_operand_application_identity` (225-261): only roles `BoundarySubject | Provider` have an identity; otherwise `Err("Product application has no nominal declaration role")` (231-236); no formals and no lifetimes -> `Ok(None)` (241-243).
- `Capture::expression`: fixed list of node kinds above; "unary" admits two operators only (1068-1073).
- `ProductInputSelection::selected_symbol` type slot (140-175): only `Declaration` slot on Named/Generic/DynamicTrait and `ArrayExtent` on `FixedArray{ConstParameter | ConstCall}`; everything else `Err("... no longer names its retained type declaration slot")` (170-174).
- `product_candidates` (1308-1364): six `ProductOperandKind` variants.
- `ProductEntryDeclaration::from_declared_owner` (1386-1414): requires span lengths of contracts/states/facts to equal their recorded counts or returns `None`.
- `AppliedTraitScope::from_type_reference` (3583-3627): handles Reference, Constrained, Named, Generic, DynamicTrait; other nodes -> `None`.
- `native_binding_formation` (1498): exactly one operand ("native formation lost its exact requirement").

**Same job elsewhere.**
- `evidence_requirement_declaration` (2273) and `evidence_bound_requirement` (2464) both resolve a requirement state against a binder's bound; `collect_requirement_namespace` (2585) enumerates the same refine/requires closure that `visit_inherited` (3986-4024) walks and `AppliedTraitScope::visit` reuses; `abstract_requirement_may_have_operator_bound` in operator_declarations.rs:3361-3418 walks the same trait-parent closure a third time (read, both).
- Frame-pushing blocks in `under_invocation` (1646-1650 and 1681-1685) are two copies of "splice substitutions, shift starts, insert 0, insert region".
- The scope-start bookkeeping `partition_point(|start| *start <= ordinal)` appears at 1874, 3498, 3560; a variant using `.iter().rev().find(|start| **start <= ordinal)` at 3843-3848, 3895-3901 (read; they should agree but are four spellings).
- `AppliedTraitScope::parameter_argument` (3828) vs `AppliedTypeReference::parameter_argument` (3485) and `parameter_application` (1863): three lookups of "last substitution for a binder".
- `capture_static_environment` identity framing (524-575) is a hand-rolled length-prefix encoding similar to `canonical_closed_operator_realization_bytes` (operator_declarations.rs:470-586) and `encode_application_text` (602). Suspected separate encoders, not compared.
- `native_binding_formation` framing (1508-1516) is a third length-prefixed digest writer.
- `Inputs` visitor (97-129), `Census` (358), `FieldTypes` (3272) are three more one-off `TypedReferences` implementors (see references.rs notes).

**Unused / one-caller / shared (grepped).**
- `AppliedTraitScope` (documented as the carrier for trait applications): 226 occurrences across 28 files (35 in this file).
- `capture_static_environment|capture_closed_operand|native_binding_formation|resolve_build_request_author|same_product_operand_source|same_product_request_source|is_declaration_operand|resolve_product_operand|resolve_product_path`: 58 occurrences across 11 files (13 in this file).
- `ProductEntryDeclaration|collect_product_selection_occurrences|DynamicSignatureIneligibility|applied_trait_roots(|ConformanceRowSource`: 99 occurrences across 24 files.
- `DynamicSignatureIneligibility` (own grep, content mode): 6 uses outside the definition at traits.rs:5440/5806/5809 (validation) and calls_and_control_flow.rs:159/221/223/231; defined at 4126.
- Not individually counted: `retain_dependency_product_application`, `prepared_product_operand`, `keeps_declared_signatures`, `callable_contract_key`.

**Stated limits.** 778 "cannot yet retain transparent alias application obligations" (Build helper static types with alias applications are refused). 1135 "an executed static layout actual must retain its materialized value before Product capture". 1199 "non-static or unmaterialized selected value". 1652-1653 comment: "Untyped bindings remain unresolved. They cannot close a demanded trait operand".

**Fail-open risks.**
- `capture_static_environment` returns `Ok(None)` when the machine has no specialization (419-421): the caller then proceeds with no static environment. Wrong program: a Build helper that should be specialized but whose receipt is missing would run without captured actuals. Status: open (callers in build-time-evaluation not read).
- `Capture` skips `RuntimeValue` bindings silently (472). Status: open.
- `ProductInputSelection::selected_symbol` requires the retained type to be in the argument's visited inputs (132-136); fail-closed (Err).
- `project_type_reference` loop (3548-3564) has no cycle detection: two substitutions that name each other would loop forever (not accept). Status: inferred, not traced.
- `AppliedTraitScope::visit_inherited` silently skips unresolved or unbuildable parent traits (4003-4018) so `visit`, `declaring_scope` and `evidence_bound_requirement` can see a smaller closure than the declaration states; for `evidence_bound_requirement` that yields `Err` (2576), for `visit_*` consumers (e.g. operator meanings at operator_declarations.rs:3158) that means fewer meanings found. Status: open.
- `apply_callable_refinements` with `TraitRefinementReach::Inherited` does `continue` (2737) which skips the service reach row update entirely for that clause (intended per the enum name; no comment). Status: open.
- `is_declaration_operand` accepts a type reference alongside a path only if it is `Named` with the same symbol (1260-1263); otherwise false. Fail-closed.
- Otherwise none seen.

---------------------------------------------------------------------------
## 1. evidence_and_inspection.rs (5602 lines)

**Purpose.** Two public modules. `evidence` (lines 7-1975) holds typed proof support: byte-sequence predicate recognition, mathematical definition records, the "proof-only" data classification, proposition records with a label-based normalizer, and ranking-witness expression resolution. `inspection` (1977-5602) holds `ExplicitStateCallees` (a callee collector), the `snapshot` module (serde-serialisable debug/golden snapshot of a whole `TypedTrees`) and `diagnostic_expression_name`. Header comment 1-5. Re-exported by lib.rs:772 (`pub use evidence_and_inspection::{evidence, inspection}`) and lib.rs:1852/1853.

**Shape.**
- `evidence::byte_predicates` 8-117: `byte_predicate_call` 16, `domain_byte_predicate` 32, `type_reference_domain_predicates` 53, `literal_bytes` 90.
- `evidence::mathematical` 118-173: `MathematicalDefinition` 127 (symbol, name, binders, parameters, `result: MathematicalTypeHandle`, `body: MathematicalBody` {Assumption (default), Definition(expr)}), `MathematicalParameter` 138, `MathematicalType` {Ordinary, Arrow, Application} 148. Data only, no logic.
- `evidence::proof_only` 174-647: `ProofOnlyReason` {Integer, Quotient, Recursive, Contains} 183; `ProofOnlyClassification {reasons: HashMap<u32, ProofOnlyReason>}` 191 with `is_proof_only` 196, `reason` 200, `describe` 204, `proof_only_mention` 225, `contract_term_carrier` 233, `is_proof_machine` 363, `is_proof_signature` 405; `classify(program)` 463 (builds containment edges between data definitions, marks Integer, integer-containing, recursive (via `reaches` 588), then fixpoint over containment and quotient carriers 538-572).
- `evidence::proposition` 648-1842: records `PropositionDefinition` 751, `PropositionBinder` 762, `PropositionBody` {Primitive, Witness, Transparent} 780, `PropositionFormula` 792, `PropositionApplication` 798, `PropositionBinderArgument` 806; normalized forms `NormalizedPropositionFormula` 828, `PropositionEvidenceClassification` 839, `NormalizedEvidenceInterfaceIdentity` 1194; `TypedTrees` methods `normalize_nominal_proposition_application` 1267, `normalize_proposition_application` 1456, `normalized_identity_label` 1483, `render_proof_expression` 1406, `proposition_labels` 1435, `render_proof_constructor_value` 1391; private `render_expression` 1693 (full ExpressionNode match), `constructor_values::render` 663, `normalize_evidence_interface` 847, `normalized_evidence_requirements` 919, `type_reference_mentions_any` 997, `expression_mentions_any` 1053.
- `evidence::ranking` 1843-1974: `RankingExpressionCustody` 1851, `resolve_machine_witness_subjects` 1858, `resolve_machine_witness_view_arguments` 1870, `resolve_witness_expressions` 1891, `witness_expression_text` 1924.
- `inspection::callees` 1978-2101: `ExplicitStateCallees` 1985 with `collect` 1991; used by `direct_calls::collect` (lib.rs:497-507).
- `inspection::snapshot` 2102-5562: sub-modules `declaration_snapshots` 2103, `machine_snapshots` 3003, `statement_and_expression_snapshots` 3650, `type_snapshots` 4448, then the top-level `TypedTreesSnapshot` 4873 (roots, tables counts, selections, external bindings, evidence forwardings, proof output calls, service reaches, specializations, ranking custody, pending const range endpoints) built by `from_typed_trees` 5133, `to_json`/`to_json_pretty` 5455/5459, `TypedTableSnapshot` 5502 (53 `*_count` fields), helper `to_strings` 5559. About 60 `#[derive(Serialize)]` snapshot structs/enums (mechanical mirror of table rows).
- `TypedTrees::diagnostic_expression_name` 5565-5600.

**Inputs/outputs.** Reads typed tables (data definitions, domains, proof facts, expression and type tables, symbols, machine termination plans). Outputs: predicates/identity strings/labels, a `ProofOnlyClassification` object, resolved `ExpressionHandle` lists, and serialisable snapshots. No mutation of the program.

**How it decides.**
- Byte predicates keyed on SPELLING: `byte_predicate_call` accepts a call with no `target_symbol` and no receiver whose `call.target.as_str()` parses via `ByteSequencePredicate::from_name`, with exactly one argument (evidence_and_inspection.rs:21-29). `domain_byte_predicate` requires the domain to have exactly one `ProofFact::Expression` fact of that call form on the domain subject and no alias, index arguments, establishment routes or index parameters (36-50).
- Proof-only classification is keyed on `symbol.arena_index()` (u32 only, generation ignored): `reasons: HashMap<u32,..>` (192, 197, 201, 330, 468, 492, 521, 526). Edges = data definitions mentioned inline by non-erased field types (Named, Generic base + args, Constrained base, FixedArray element; not Reference, Slice, DynamicTrait) 607-646. `Int` builtin type symbol seeds `Integer` (465-467, 520-522). Recursive if a definition reaches itself (532-536). Fixpoint adds `Contains` if any edge targets a proof-only definition (544-557) and `Quotient` if the carrier mentions a proof-only type (558-567).
- `is_proof_machine` (363-403) and `is_proof_signature` (405-428): true when some non-erased parameter/return/local type mentions a proof-only type, but return `false` immediately if that parameter is mutable or borrows proof-only data (394-397, 422-424).
- `contract_term_carrier`/`closed_contract_carrier` (233-317): admits a Named data type only if supply mode `CheckedShape`, no quotient, no type parameters, shape Empty/Record/Enum/Mixed, no plan-laid layout, no access view plan, no wire schema with the same NAME (`schema.name == definition.name`, 293-296), and all fields recursively admitted (primitive, proof-only mention or another closed carrier); arrays only with literal length.
- Proposition normalization is keyed on rendered label strings: `canonical_application_label(name, binder_labels, argument_labels)` (1670-1681) and identity strings (`proposition:fact:{label}`, `proposition:witness:{evidence}:{label}`, `boolean:{label}` 1223-1235). Binder identities come from `display_path` of type/machine/proposition parameter symbols (1092-1110). Transparent propositions expand by substitution of rendered argument text (1627-1668) with a `visiting` stack to stop cycles (1301, 1507). A `Transparent { BooleanExpression }` is normalised as `Boolean{label}` in `normalize_proposition_application_inner` (1588-1598) but yields `None` in `normalize_nominal_proposition_application_inner` (1363-1365).
- `normalize_evidence_interface` (847-917): if the evidence type is not a Named/Generic reference to a trait symbol, returns the label with no interface (868-874); if any trait argument mentions a binder that lacks an exact identity, interface `None` (875-884).
- `render_expression` Name case (1779-1820): substitution by symbol (`path.symbol` or `head_symbol`) or by first-segment NAME (1787-1793); a chain of symbols that are parent/child and end in a `Variant` renders as a constructor label (1797-1818); else the `::`-joined spelled path.
- Ranking: `resolve_machine_witness_subjects` prefers `ranking_expression_custody_for(machine)` if its length equals the witness length, else returns `None` when lengths differ (1863-1866); otherwise resolves each rendered string by searching ALL expressions for ones whose `witness_expression_text == expected` that belong to the root state (root name/member/cast chain resolves to a state parameter, 1952-1973), and if none belongs falls back to ANY expression anywhere with the same text (1911-1919); the text `"value"` always yields `None` (1899-1901).
- `ExplicitStateCallees` (1978-2100): collects `call.target_symbol` of statement calls, expression calls (via `subtree_any` with edges `atomic_results` and `pattern_dependencies` added to `SubtreeEdges::VALUES`, 2085-2099) and named transition targets (`path.symbol`, 2067); dedups by symbol equality (2007).
- `diagnostic_expression_name` (5565-5600): follows chains of single-assignment immutable locals (not `Local`-kind authored symbols with a source span) up to `expression_count` steps.

**Cases by shape.**
- `witness_expression_text` (1924-1950): only Integer literal, Name path (joined with "."), Member chain, and `Subtract` binary; everything else is the string `"value"`.
- `literal_bytes` (90-116): String literal or array literal of integer literals that each fit `u8`; otherwise `None`.
- `ranked_expression_root` (1963-1973): Name, Member, Cast only.
- `expression_mentions_any` (1053-1090): Name, Binary, Unary, Borrow examined; literals false; "ArrayLiteral | Match | SavedSubject | Atomic | Cast | Call | Indexed | Member | Range | StructLiteral | TypeExpression | ZeroValue" return `!symbols.is_empty()` (1077-1088), i.e. "assume mentioned if any unresolved binder exists".
- `type_reference_mentions_any` (997-1051): fixed list of type node kinds.
- Snapshot `StatementSnapshot::Call` (4023-4046) does not carry the `private_layout_slot` that the expression variant carries (4230-4233); snapshot type references drop lifetimes (4658 `lifetime: _`) and symbol identity (`Named { name }` 4759).

**Same job elsewhere.**
- Three separate "walk the trait requires/refines closure" routines: `normalized_evidence_requirements::collect` (924-977), `collect_requirement_namespace` (product_and_trait_declarations.rs:2585-2607), `AppliedTraitScope::visit_inherited` (product_and_trait_declarations.rs:3986-4024), plus `abstract_requirement_may_have_operator_bound` (operator_declarations.rs:3361-3418) (read all four).
- Expression text rendering: `render_expression` (1693-1841) vs `crate::expression::display_call_label`/`display_name` it delegates to; and `witness_expression_text` (1924) is a second, narrower renderer used for ranking witnesses (both read; the delegated functions are in `values.rs`, not read).
- "mentions a symbol" walkers: `type_reference_mentions_any` 997 and `expression_mentions_any` 1053 parallel the `TypedReferences` visitors in references.rs (suspected overlap; `references::collect_type_symbol_dependencies` references.rs:1182 returns a symbol set that could answer the same question; not compared).
- `ExplicitStateCallees` (1985) vs `references::ReferenceCollector.callable_uses` (references.rs:1057, filled by `visit_callable_symbol` 1985): two collectors of called symbols; the callees one omits static dispatch and machine-argument callables (read).
- `snapshot` `OperatorDefinitionSnapshot` (2158) lists fields of `OperatorDefinition` (operator_declarations.rs:3870) except `home_domain`.
- `proof_only::classify` builds `index_by_symbol: HashMap<u32, usize>` (468) beside `data_definition_by_symbol` lookups elsewhere in the crate (suspected duplicate lookup).

**Unused / one-caller / shared (grepped over `omega-rust`).**
- `proof_only::classify|ProofOnlyClassification`: 76 occurrences in 14 files (5 in this file).
- `resolve_witness_expressions|resolve_machine_witness_|witness_expression_text`: 39 occurrences in 8 files (13 in this file; 15 in `05 .../checks_termination.rs`).
- `normalize_nominal_proposition_application|normalize_proposition_application|normalized_identity_label|render_proof_expression|render_proof_constructor_value|diagnostic_expression_name`: 103 occurrences in 19 files (15 here).
- `byte_predicate_call|domain_byte_predicate|type_reference_domain_predicates|literal_bytes(|ExplicitStateCallees|snapshot_json`: 104 occurrences in 43 files (8 here; the `literal_bytes(` pattern also matches unrelated names, count is an upper bound).
- `OMEGA_STRUCT_TRACE`: 5 occurrences in 3 files (2 here at 501 and 574, validation `trace_flags.rs`, `contract_entailment.rs`).
- Not separately counted: `is_proof_machine`, `is_proof_signature`, `contract_term_carrier`, `TypedTreesSnapshot` consumers (the `snapshot*` names are common).

**Stated limits.** None with "unsupported/remains" wording. Inline error string text at 5359-5361 (`expect("service reach table fits u32")`).

**Fail-open risks.**
- `resolve_witness_expressions` fallback (1911-1919): if no expression in the root state matches the rendered text, ANY expression in the whole program with that text is returned. Wrong program: a ranking witness `n - 1` that is not written in the machine's own body could be bound to an unrelated expression elsewhere that spells the same, so the termination check would be applied to the wrong expression. Status: open (callers in `checks_termination.rs` not read).
- `render_proof_constructor_value` (1391-1404) returns `"invalid-constructor:{type_name}"` instead of failing when the constructor cannot be uniquely rendered; two unrelated invalid constructors of the same-named type render identically, and these strings serve as identity labels. Status: open.
- `proof_only` classification keyed on `arena_index()` only (192-201, 468): two symbols with equal index and different generation would be conflated. Status: open (not traced whether generations other than 1 occur).
- `expression_mentions_any` conservative `!symbols.is_empty()` (1088) leads to interface `None` (fail-closed for identity), stated by shape only. Status: traced within unit.
- `is_proof_machine`/`is_proof_signature` return `false` for mutable/borrowing proof-only parameters (394-397, 422-424): classification "not a proof machine" means treated as a runtime machine. Status: open.
- `byte_predicate_call` matches on call spelling when `target_symbol` is invalid (21-24): a user call to a name that equals a predicate name but unresolved would be read as the predicate. Status: open (depends on whether unresolved calls reach here).
- `fact_snapshots_for_span(...).remove(0)` at 4009-4013 panics rather than accepts. Not a fail-open.
- Otherwise none seen.

---------------------------------------------------------------------------
## Coverage

All five files read in full, in order, by Read in chunks (no part skimmed or skipped):
- `omega-rust/psi/compilation/representations/typed-trees/src/evidence_and_inspection.rs` lines 1-5602 (5602 lines), read in full.
- `.../src/lib.rs` lines 1-1871 (1871), read in full.
- `.../src/operator_declarations.rs` lines 1-3900 (3900), read in full.
- `.../src/product_and_trait_declarations.rs` lines 1-4130 (4130), read in full.
- `.../src/references.rs` lines 1-3710 (3710), read in full.
Total 19213 lines read in full, 0 skimmed, 0 not read. Counts of call sites come from Grep count mode over `omega-rust` (some patterns are alternations, so per-name counts are not split; noted at each site).

## Five things a maintainer most needs to know about this unit

1. `references.rs` is a single visitor trait (`TypedReferences`, references.rs:1006) with about a dozen one-off implementors (`ReferenceCollector` 1066, `TypeOccurrences` 1287, `ProofFactCollector` 1928, plus ones in product_and_trait_declarations.rs:97, 358, 3272 and evidence/lib). The walkers list fields by hand (call fields 3117-3143), so a new field on a call node is silently not visited until added there, and statement and expression call handling are two parallel copies (3074-3147 vs 3175-3218; 87-148 vs 159-239).
2. `operator_declarations.rs` has two matching regimes: `type_reference_matches` always allows matching by spelled name when symbols differ (3446, 3752-3755, 3673) while the "closed application" functions use `allow_name_fallback=false` (symbols required). Float operator recognition is by spelled path `Float::add...` plus a toolchain file path (1311-1329, 1356-1376).
3. Expression and statement named-call operator resolution are duplicated (operator_declarations.rs:848-893 vs 1005-1044; 952-1003 vs 1123-1160), and the statement copy omits `machine_token_bindings`.
4. `AppliedTraitScope` (product_and_trait_declarations.rs:3574) is the carrier for trait applications (226 grep hits in 28 files). It stores flat `substitutions` plus `substitution_scope_starts` and parallel `region_scopes`; the same "last substitution for a binder + partition_point on starts" lookup is written in at least four spellings (1874, 3498, 3560, 3843).
5. `lib.rs` is both the program container (`TypedTrees`, ~50 fields; `DerefMut` invalidates a declaration index on every mutable deref, 1801-1806) and a bag of inline modules; `EmptyRunReleaseDeclaration::authenticate` (1054) identifies a "toolchain run release" machine by comparing file BYTES to an `include_bytes!` copy of `source/library/core/run.omg` plus a normalised identity string (1041-1083).

## What you are unsure about

- Whether `retain_static_operand_sources`'s returned `complete` flag (references.rs:241-305) is meant to be consumed; no call site in this file uses it (all use `?;`). I did not read the stage 04 callers.
- Whether `OperandType::Primitive` operands (operator_declarations.rs:1998-2003) can reach matching against non-primitive expected parameters in practice; I did not read the callers that construct `OperandType::Primitive`.
- Whether the name-fallback matching (operator_declarations.rs:3752-3755) can accept two distinct same-named data types in real programs; callers of `type_reference_matches` live in this file (27 hits, all here) but the operand types they receive were not traced.
- Whether the ranking-witness global fallback (evidence_and_inspection.rs:1911-1919) is reachable with a wrong expression; callers in `checks_termination.rs` were not read.
- `project_type_reference` (product_and_trait_declarations.rs:3539-3571) loop termination for cyclic substitutions is inferred from the code, not traced.
- Statement `retain_static_argument_contexts` four-way OR (references.rs:96-103) vs the expression variant's declaration test (172-176): I recorded the difference (`requirement_application` check present only in the expression variant) but did not trace whether it is intentional.
- Counts for alternation greps are per-pattern-set, not per name.

