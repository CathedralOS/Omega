# Unit 79 notes

Checkout: `<checkout>/` (main at 457adf4a11f). All paths below are relative to it. All line numbers are 1-based as returned by the Read tool.

Files covered (all under `omega-rust/psi/compilation/representations/`):
1. `symbol-resolved-trees/src/control_flow_and_calls.rs` (1567)
2. `symbol-resolved-trees/src/declarations_and_machines.rs` (676)
3. `symbol-resolved-trees/src/lib.rs` (1435)
4. `symbol-resolved-trees/src/storage_and_indexes.rs` (646)
5. `symbol-resolved-trees/src/type_system_and_names.rs` (1118)
6. `symbol-resolved-trees/src/values.rs` (3323)
7. `symbol-resolved-trees/tests/authored_declaration_selection_ledger.rs` (70)
8. `syntax-trees/derive/src/lib.rs` (85)
9. `syntax-trees/src/control_flow_and_names.rs` (966)
10. `syntax-trees/src/copies.rs` (1918)
11. `syntax-trees/src/declarations.rs` (1294)
12. `syntax-trees/src/inspection.rs` (2695)
13. `syntax-trees/src/lib.rs` (1947)
14. `syntax-trees/src/values_and_types.rs` (2248)
15. `terminal-psi/codec/src/block_wire.rs` (1818)

Short names used below: SRT = symbol-resolved-trees crate (`omega-rust/psi/compilation/representations/symbol-resolved-trees/src/`), ST = syntax-trees crate (`.../syntax-trees/src/`). Grep commands were run with the Grep tool over `omega-rust` in the worker-23 checkout.

---------------------------------------------------------------------
## 1. SRT `control_flow_and_calls.rs`

**Purpose.** Data definitions for the symbol-resolved form of callable signatures, machines, states, statements and the "evidence" declarations (mathematical definitions, propositions). It also holds `StatementTable`, the table form of statements, and the code that turns the tree form of a statement (`Statement`) into the table form (`StatementNode`). Every record carries `SymbolHandle`s next to `DiagnosticName`s.

**Shape.**
- `calls::signature` (7-104): `StateSignature` (30) with `StateSignatureStorage` (37; type_parameters, conformance_bounds, parameters, return_type, service_reach_row, contracts, `terminates_guarantee`...), `StateParameter` (67), `SignatureContract` (77) with `SignatureContractKind` (86: Requires, Ensures, EnsuresForResultCase, Crashes{cause}), `CrashCause` (100: Trap/Abort), `AuthoredServiceReachRow/Target` (16-27).
- `control_flow::machine` (108-329): `Machine` (127, ~30 fields incl. `target`, `authored_target`, `target_family`, `body_status`, `termination_plan`, `compiler_selection_partition`), `MachineStorage` (187), `GenericConformanceBound` (212), `OwnedData` (224), `TraitConformance` (232), `MachineBodyStatus` {Deferred, Complete} (120). `Machine::symbol_spelling` (164) returns `target_symbol_name` if `target.is_some()` else `name`.
- `control_flow::state` (330-374): `State` (339), `StateStorage` (346; `statements: HandleSpan<Statement>` and `statement_nodes: HandleSpan<StatementNode>` side by side).
- `control_flow::statement` (375-1465): tree statements `Statement` enum (402: Assembly, RootBinding, AssemblyFact, Assignment, Call, ProofOutputBindingStatement, Expression, LocalData, Transition); table statements `StatementNode` (1160, same nine kinds with `Table*` payloads); `StatementTable` (612) with arenas for statements, transition targets, transition patterns, outcome proof selectors, expression handle spans, name path members.
  - `StatementTable::insert_tree` (847) / `insert_tree_from_source` (876): converts one `Statement` into a `StatementNode`, transplanting expressions/types through `StatementTransplant` (1115; `copy_expression_handles` flag picks "copy subtree" vs "reuse handle").
  - `validate_authored_selection_rebase` (747) and `apply_authored_selection_rebase` (800): remap `authored_call_selection` / `authored_static_argument_selections` occurrence ids of Call statements and Named transition targets after a ledger rebase.
- `evidence::mathematical` (1468-1523): `MathematicalDefinition`, `MathematicalType` {Ordinary, Arrow, Application}, `MathematicalBody` {Assumption, Definition}.
- `evidence::proposition` (1524-1566): `PropositionDefinition`, `PropositionBinderKind` {Type, Const, Machine}, `PropositionBody` {Primitive, Witness, Transparent}.
- Test helpers `test_program_with_guarded_transition`, `test_machine`, `test_state` (245-373) and two tests (1325, 1391).

**Inputs/outputs.** Pure data plus `insert_tree*` which reads a tree `Statement`, three source arenas (child type references, constraints, statement path members), an `ExpressionSource`, and writes into the `StatementTable`, an `ExpressionTable` and a `TypeReferenceTable`.

**How it decides.** No semantic judgments except: `Machine::symbol_spelling` (164) keys on `target.is_some()`. `validate_authored_selection_rebase` (747-798) decides per statement by arena position vs a frontier: `index < statement_frontier` => `rebase.retain_base(occurrence)`, otherwise `rebase.rebase_appended(occurrence)`; a `None` result is returned as `Err(store)`. `apply_...` (800-845) uses `handle.arena_index() > statement_frontier` (1-based handle index) for the same split (read; the two spellings agree only if arena indexes are 1-based, see the test at 1376 which reads `Handle::from_arena_index(1)` as the first row — inferred consistent).

**Cases by shape.** `insert_tree_from_source` matches all nine `Statement` variants (883-1030), no wildcard. Rebase code only looks at `StatementNode::Call` and `TransitionTargetNode::Named` (761, 780); other kinds are skipped.

**Same job elsewhere.**
- The "validate then apply a selection-ledger rebase over a store split at a frontier" is written four times in SRT: statements/targets (this file 747/800), tree statements (`lib.rs` 382/414), proof facts (`lib.rs` 429/465), expressions (`values.rs` 2775/2805). Same split rule each time (`index < frontier` => `retain_base`, else `rebase_appended`).
- Two parallel statement representations in one crate (`Statement` at 402 and `StatementNode` at 1160), converted by `insert_tree_from_source` (876). The ST crate has the table form only (`syntax-trees/src/control_flow_and_names.rs:324`).
- `TableOutcomeProofSelector` (1257) and `OutcomeProofSelector` (469) have identical fields.

**Unused / one-caller.** `StatementTable::insert_tree` (847): grepped `insert_tree\b` definitions — three crates have a function of this name (SRT 532 types table, SRT 847 statements, typed-trees values.rs:2603); I did not count callers per receiver type (suspected few; not traced). `Machine::symbol_spelling` (164): `\.symbol_spelling\(` has 1 call in all of omega-rust (`validation/src/declarations_core.rs:2558`, not read); typed-trees has its own `symbol_spelling` (`typed-trees/src/calls_and_control_flow.rs:1817`). So the SRT version has 0 or 1 callers (inferred 0; receiver type of the single call not checked).

**Stated limits.** None in comments. `MachineBodyStatus::Deferred` (122) is a state, no comment.

**Fail-open risks.** none seen. (`validate_authored_selection_rebase` fails closed: `None` => `Err`.)

---------------------------------------------------------------------
## 2. SRT `declarations_and_machines.rs`

**Purpose.** Records for constants, data, domains, measures, operators, traits/conformances, wire schemas, type aliases (module `declarations`), and `MachineDeclarations`, a wrapper around `OrderedRootArena<Machine>` that adds a symbol-to-position lookup.

**Shape.** `ConstDeclaration` (12), `DataDefinition`+`DataDefinitionStorage` (31/39), `QuotientDefinition` (57), `DataMember` {Field, Variant} (113), `Binder<K>` (125), `TypeParameter = Binder<TypeParameterKind>` (132) with kinds Type/Implementation/Const/Value/Machine/Proposition, `MachineParameterContract` (154: RequirementIdentity, Structural, AuthoredNominal, Nominal) and `MachineParameterContractView` (196), `DomainDefinition` (248), `ProofFact` {Expression, Membership} (282), `MeasureDefinition` (313), `OperatorDefinition` (330), `TraitDefinition`/`Conformance`/`ConformanceRow` (352/380/394) with `ConformanceRowSource` {Inline, Reference, TraitDefault, Synthesized} (409), `WireSchema` (439), `TypeAliasDefinition` (489). `MachineDeclarations` (508): `arena: OrderedRootArena<Machine>` + `positions: OwnedIndex<Vec<(SymbolHandle, usize)>, ()>`; `by_symbol` (553), `unique_by_symbol` (558), private `symbol_position` (564).

**Inputs/outputs.** `symbol_position` builds, on first use after a change, a sorted `Vec<(symbol, position)>` (568-576) and binary-searches by `(arena_index, generation)` (580). `DerefMut` (536) clears the cached positions on every mutable access.

**How it decides.**
- Lookup is keyed on the exact `SymbolHandle` (index and generation): `partition_point` then `candidate == symbol` (580-586).
- `by_symbol` returns the first machine in arena order with that symbol (stable sort, first of equal keys); `unique_by_symbol` returns it only if no second row has the same symbol (558-562, 589-591). Test at 615 pins "keeps first duplicate".
- `DataDefinition::shape_kind_from_members` (84): Empty/Record/Enum/Mixed from presence of fields and variants.
- `ConformanceRowSource::owns_realization` (417): Inline, TraitDefault, Synthesized own; Reference does not.

**Cases by shape.** `shape_kind_from_members` is a closed 2x2 match (95-100).

**Same job elsewhere.** `DataShapeKind` + `shape_kind_from_members` exist also in typed-trees (`typed-trees/src/declarations.rs:520`, `:499`). `MachineParameterContractView` exists also in typed-trees (`declarations.rs:654`). `data_definition_by_symbol` / `domain_definition_by_symbol` have same-named functions in typed-trees `declaration_tables_and_index.rs:1842`, validation, pipeline 03, provider-planning (grep `fn (domain_definition_by_symbol|data_definition_by_symbol)`: 10 definitions across crates).

**Unused / one-caller.**
- `MachineDeclarations::unique_by_symbol`: 2 non-test call sites (`pipeline/03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs:1616`, `pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols.rs:3420`) + 3 in its own test (command: Grep `unique_by_symbol|...`).
- `DataDefinition::shape_kind_from_members` (84): 22 call sites of this name across omega-rust, but the typed-trees type has the same name; receiver type not resolved (inferred most use the typed-trees one; unknown for the SRT one).

**Stated limits.** None.

**Fail-open risks.** none seen. `by_symbol` returning the first of duplicates is stated in the test name (`selection_keeps_first_duplicate_and_exact_symbol_generation`, 615), and a separate `unique_by_symbol` exists for callers that need uniqueness.

---------------------------------------------------------------------
## 3. SRT `lib.rs`

**Purpose.** Root of the crate: the container `SymbolResolvedTrees` (roots + table storage + symbol table + service-reach tables + `resolution_indexes`), the authored-declaration-selection ledger access, and the "extension rebase" that shifts occurrence ids in every store after the ledger is merged with a destination ledger.

**Shape.**
- `SymbolResolvedTrees` (92): `roots: SymbolResolvedRoots`, `tables: SymbolResolvedTableStorage`, `symbols`, `service_reaches`, `service_reach_rows`, `authored_service_reach_rows`, `semantic_domains`, `external_bindings`, `evidence_forwardings`, `resolution_indexes`. `Deref` to roots (931).
- `SymbolResolvedRoots` (132): twelve `OrderedRootArena`s (+ `MachineDeclarations` for machines).
- `SymbolResolvedTableStorage` (185) -> `SymbolResolvedDeclarationStorage` (193, ~28 `Arena`s), `SymbolResolvedBodyStorage` (234: `expressions`, `statements`), `SymbolResolvedTypeStorage` (226); private `authored_declaration_selections` (189).
- `AuthoredSelectionExtensionFrontier` (106, six lengths), `AuthoredSelectionOccurrenceStore` (116), `AuthoredSelectionExtensionRebaseError` (125).
- `rebase_authored_selection_extension` (287): ledger `replace_prefix_and_rebase_suffix` first, then `prepare_occurrence_rebase` (321; validates every store) and only then mutates (307-317). On error it gives back `self` untouched (292-300, 302-305).
- `rebuild_tables` (843) and `append_completed_body_tables` (851): rebuild `bodies.expressions`, `bodies.statements`, `types.references` by calling `tables::SymbolResolvedTreeTables::from_*`.
- Many one-line span accessors (518-841) and ledger passthroughs (522-642).

**Inputs/outputs.** Reads the arenas listed above; writes the three rebuilt tables and the occurrence ids inside expressions, tree statements, table statements, transition targets, proof-fact memberships.

**How it decides.** Rebase validation is keyed on arena position relative to the recorded frontier (`index < frontier` => base row must map via `retain_base`; else `rebase_appended`) in `validate_tree_statement_rebase` (382-412) and `validate_proof_fact_rebase` (429-463); anything that maps to `None` is `OccurrenceOutsideOwnerFrontier(store)`. Lookups by symbol (`domain_definition_by_symbol` 264, `data_definition_by_symbol` 270, `trait_definition` 148, `const_declaration` 154/768, `const_declaration_at_source_span` 775) are linear `.iter().find` on `symbol` (or on `symbols.symbol_source_span(...) == Some(span)`, 779). `machine_parameter_contract_view` (716): RequirementIdentity and AuthoredNominal give `None`; Nominal looks up the trait by symbol and then the requirement by symbol in `trait_machine_signatures`, `?`-returning `None` if missing (731-735).

**Cases by shape.** `tree_statement_occurrences` (867-884): "two slots" array — only `Statement::Call` (1 occurrence) and `Statement::Transition` (target and continuation, if `Named`); every other `Statement` kind yields none (`_ => {}`, 881). `rebase_tree_statement_occurrences` (886-922) is the matching mutator.

**Same job elsewhere.** See file 1: the four-fold validate/apply rebase pattern. `const_declaration` is written twice: `SymbolResolvedRoots::const_declaration` (154) and `SymbolResolvedTrees::const_declaration` (768) which re-implements the `iter().find` instead of delegating (while `trait_definition` 764 does delegate). `ResolvedIndexes` (file 4) is a second lookup mechanism next to `MachineDeclarations` (file 2) and these linear finds.

**Unused / one-caller (greps).**
- `rebase_authored_selection_extension`: 2 non-test call sites, both in `pipeline/02_syntax-trees-to-symbol-resolved-trees/src/resolution.rs` (487, 915). `authored_selection_extension_frontier`: 3 non-test sites in the same file (349, 2145, 2453).
- `rebuild_tables`: 1 non-test call (`resolution.rs:2602`) + tests; `append_completed_body_tables`: 1 call (`resolution.rs:2601`).
- `set_proof_fact_source_span` (673): non-test callers in SRT are `values.rs:474` and `pipeline/02.../lowering.rs:795`; same-named functions exist in typed-trees and ST.
- `domain_definition_by_symbol` / `data_definition_by_symbol` SRT versions: grep for the names gives 251 occurrences over 97 files (shared names with typed-trees); per-receiver count not separable.

**Stated limits.** The header comment (lines 26-33) lists the layout as `declarations.rs` and `type_system.rs`, but the files are `declarations_and_machines.rs` and `type_system_and_names.rs` (stale name, read).

**Fail-open risks.**
- `set_proof_fact_source_span` (673-683) uses `Vec::resize(index + 1, None)`, which shrinks the vector when `index + 1 < len` and would drop later spans. Traced: both non-test callers (`values.rs:474`, `lowering.rs:795`) call it immediately after appending the fact, so the handle is the last one and the call extends or keeps the vector. Not reachable with a bad input from these callers; a future caller with an earlier handle would silently erase spans (traced to two callers).
- `proof_fact_source_span` (665) returns `None` for a missing slot (`.get(..).copied().flatten()`), which is read as "no span", not as acceptance.
- `machine_parameter_contract_view` returns `None` for `AuthoredNominal` (725): callers treat as "no view". Not traced (open).

---------------------------------------------------------------------
## 4. SRT `storage_and_indexes.rs`

**Purpose.** (a) `lookup_indexes::ResolvedIndexes`: cached position lookups (generic-origin index, data definition position, local-statement position, parameter position). (b) `storage::tables::SymbolResolvedTreeTables`: rebuilds the body tables (statement nodes, expressions, type references) from the tree-form arenas.

**Shape.**
- `GenericOriginIndex` (17), `PrefixPositions` (25), `PrefixPositionMaps` (32), `ResolvedIndexes` (73). `PrefixPositionMaps::position` (37-69) keeps a map keyed by the **raw address of the slice start** (`rows.as_ptr() as usize`, 44) and a watermark plus three sampled rows (first, middle, last) to decide whether the cached entry is still valid (46-56). `ResolvedIndexes::with_generic_origin_index` (123), `data_definition_position` (140, keyed by `OrderedRootArena::revision()`), `local_statement_position` (160), `parameter_position` (179).
- `ResolvedIndexes` trait impls: `PartialEq` always `true` (104-108), `Record::write` writes nothing and `read` returns default (214-221), `Clone` clones the mutex contents (80-96).
- `SymbolResolvedTreeTables::from_symbol_resolved_trees_with_state_spans` (267), `from_body_completion` (280), `append_declaration_tables` (307-452), `normalize_statements` (454).

**Inputs/outputs.** `append_declaration_tables` walks data definitions, domains, propositions, traits, machines (those at or after the frontier, plus any listed in `completed_machines`, 410-429), operators, measures and inserts every type reference they own into `TypeReferenceTable`; for machines it also normalises each state's `statements` into `statement_nodes` (577-594) and assembly-block instructions into `statement_nodes` (349-360).

**How it decides.**
- Cache validity for prefix positions: slice start address + `watermark <= length` + equality of 3 sampled values (46-56). Sample for statements is a hash of discriminant plus local symbol index (112-120); for parameters it is `symbol.arena_index()` (190). A row changed in the middle (not index 0, mid, last) with the same address would not be noticed.
- Generic-origin index validity: sum of three revisions with `wrapping_add` (202-208).
- Which machines get re-normalised: `*ordinal >= frontier.machines.len() || completed.binary_search(...)` (420-427), keyed on `(arena_index, generation)` of the symbol.

**Cases by shape.** `append_declaration_tables` handles exactly: assembly blocks, data definitions, domain definitions (target type + operators), propositions (const binders, parameters, witness), traits (machine signatures), machines (declared type equations, owned data, states), operators, measures. It does **not** visit `const_declarations`, `type_aliases`, `mathematical_definitions`, `conformances`, `wire_schemas`, `data` `where_facts`/`cleanup_selection` or `satisfies` (read; absence of code, not a comment).

**Same job elsewhere.** `ResolvedIndexes::data_definition_position` (140) is a revision-keyed cache of the same thing `MachineDeclarations::symbol_position` (file 2) does for machines, and `SymbolResolvedTrees::data_definition_by_symbol` (lib.rs 270) does linearly.

**Unused / one-caller (greps).** `local_statement_position`: 1 call (`pipeline/02.../symbols_expression_paths.rs:253`). `parameter_position(parameters, symbol)` (ResolvedIndexes): 1 call (`symbols_expression_paths.rs:245`); other `parameter_position` hits are unrelated methods. `data_definition_position`: 2 calls (`symbols_expression_paths.rs:288`, `symbols.rs:4078`). `with_generic_origin_index` on `SymbolResolvedTrees`: not counted separately. `resolution_indexes` is reset to default in 3 places (`pipeline/03.../lowerer.rs:636`, `pipeline/02.../resolution.rs:222`, `preparation.rs:126`). Command: Grep `resolution_indexes|\.local_statement_position\(|\.parameter_position\(|\.data_definition_position\(` excluding typed-trees.

**Stated limits.** none (no comments in the file beyond the header).

**Fail-open risks.**
- `PrefixPositionMaps::position` cache (37-69): "open". A stale hit would return a position for a symbol in a different slice that happens to start at the same address and have the same first/middle/last samples. Wrong program: lookup returns an old position, caller then indexes the new slice (callers `symbols_expression_paths.rs:245/253`, not traced).
- `ResolvedIndexes: PartialEq` returns `true` (104-108): two `SymbolResolvedTrees` that differ only in cached indexes compare equal; stated by absence of fields in `Debug` (100) only ("open" but harmless for a cache).
- Position values are stored as `u32` via `as u32` (63, 152): truncation above 4G rows (open, practically unreachable).

---------------------------------------------------------------------
## 5. SRT `type_system_and_names.rs`

**Purpose.** (a) symbol-resolved type reference trees (`TypeReference` as a Rust tree with `Handle`s into a child arena, and `TypeReferenceNode` as the flat table form), constraints, fixed array lengths, primitive types; (b) `DiagnosticName`, a name that carries text and a source span.

**Shape.**
- `TypeReference` enum (166-193): Reference, Constrained, FixedArray, Slice, Generic, ConstExpression, DynamicTrait, Named, Unit, AliasUse. Wrapper structs with `*Storage` and `deref_to_storage!` (195-318). `FixedArrayLength` {Literal, Expression, ConstParameter, ConstCall} (253).
- `TypeReferenceTable` (321): node arena, handle-span arena, constraint arena, and `supplied_operand_owners: Arena<SuppliedOperandOwner>` (324). `insert_tree_from_source` (549-700) copies a tree type into node form; `insert_tree` (532).
- `PrimitiveType` (879) with `from_name(&str)` (912), `name` (930), `accepts_integer_literal` (947), `accepts_range_constraint` (951), `accepts_finite_constraint` (968); `TypeReference::primitive_type` (895).
- `display_name` (18-83): text such as `<type>[2 constraints]`, `[<type>; N]`, `Name<N arguments>`.
- `DiagnosticName` (985): `text: DiagnosticNameText {Missing, Static, Generated(Arc<str>)}` + `source_span`; `PartialEq` compares text only (1068-1072); `is_source_backed` = span non-empty (1037).

**Inputs/outputs.** `insert_tree_from_source` reads the child arena of types, the constraint arena and an `ExpressionSource`; writes nodes, spans, and `supplied_operand_owners` rows (only when owner or lexical owner is valid, 690-698). `ConstExpression` is copied through `source_expressions.copy_into` (630-634).

**How it decides.**
- `PrimitiveType::from_name` and `TypeReference::primitive_type` key on the **spelling** of a `Named` reference: `"bool"`, `"f32"`, ..., `"addr"` (912-927, 899). `AliasUse` is looked through (900). The `symbol` of the `Named` is ignored.
- `accepts_integer_literal` = `accepts_range_constraint && !accepts_finite_constraint` (947-949): true for i8..i64, u8..u64, addr; false for bool, f32, f64.
- `supplied_operand_owner` / `operand_lexical_owner` (363-378): linear scan of `supplied_operand_owners` by `reference` handle, default if none.

**Cases by shape.** `primitive_type` lists primitives by name (twelve names). `display_name` for `TypeReference::Slice` ignores its content (`let _ = slice`, 32).

**Same job elsewhere.** `PrimitiveType` is defined again in typed-trees (`typed-trees/src/type_system.rs:4591`). `DiagnosticName` here vs `Identifier` in ST (file 9) vs typed-trees' `Identifier` (`typed-trees/src/values.rs:540` uses `Identifier`): three name carriers (suspected; typed-trees not read). `TypeReferenceTable::insert_tree_from_source` mirrors typed-trees (`insert_tree` hits at `typed-trees/src/values.rs:2603` is for expressions, not types — not compared).

**Unused / one-caller.** `PrimitiveType::from_name` / `.primitive_type()`: 103 occurrences in 40 files (names shared with typed-trees); per-receiver not separable. Two of the 103 are the definitions in this file (`type_system_and_names.rs`: count 2).

**Stated limits.** none.

**Fail-open risks.**
- `primitive_type` (895-908): a `Named` type named `"u8"` is classified primitive by spelling even if its `symbol` resolves to a user type called `u8`. "open": depends on whether name resolution allows shadowing primitives (not traced).
- `supplied_operand_owner` returns default (invalid) handle when no row exists (370): means "no owner"; callers' treatment not traced (open).

---------------------------------------------------------------------
## 6. SRT `values.rs`

**Purpose.** The symbol-resolved expression table (`ExpressionTable`, `ExpressionNode` and the `Table*` payload structs), per-expression side data (source span, materialization source, authored exposure, compiler partition, authored-selection occurrence ids), expression display, and three copy mechanisms: subtree copy across or within tables (`table_copies`), copy-with-symbol-substitution of expressions/proof facts (`symbol_copies`), and static-argument copy/remap (`static_members`).

**Shape.**
- `ExpressionTable` (2407): `nodes: ExpressionNodeStorage` (2413: `expressions` arena + parallel `Vec`s `source_spans`, `materialization_sources`, `authored_expression_exposures`, `compiler_selection_partitions`, `authored_selection_occurrences`) and `spans: ExpressionSpanStorage` (2424: expression handles, name path members/symbols, struct fields, match arms/fields/subjects, static members, proof selectors, occurrence ids). `insert` (2629) pushes one entry in every parallel vec.
- `ExpressionNode` (3030, 22 kinds: Match, SavedSubject, ArrayLiteral, Atomic, Binary, Boolean, Cast, Call, Float, UndeterminedNaN, Indexed, Integer, Membership, Member, Borrow, Name, Range, StructLiteral, String, Unary, TypeExpression, ZeroValue). `TableCallExpression` (3132), `StaticMachineArgument` (3146), `StaticSymbolApplication` (3156), `TableNamePath` (3169), match types (3255-3322).
- `display` module (8-320): `ExpressionNode::display_name` (19), `display_name_path` (166), operator names (287-319).
- `static_members` (321-398): `copy_static_argument_from(_self)` (326/334), `remap_static_argument_symbols` (368).
- `symbol_copies` (399-1135): `SymbolResolvedTrees::copy_expression_with_symbols` (412), `copy_proof_fact_with_symbols` (426), private `SymbolCopy` (441) with `expression`, `expression_payloads` (498), `static_argument` (669), `type_reference` (695), `type_origins` (726), `type_span` (752), `type_payload` (790), `constraint` (856).
- `table_copies` (1136-1640): `ExpressionSource {Own, Table}` (1159), `ExpressionTable::copy_from` (1188), `copy_from_self` (1196), `copy_graph` (1208-1452), `copy_transition_pattern` (1504), `copy_match_subject` (1521), `copy_destructure_pattern` (1553).
- Authored-selection rebase on expressions: `prepare_authored_selection_rebase` (2775) and `apply_authored_selection_rebase` (2805).
- Many span helpers (2456-3014), tests (1643-2375) including `EXPECTED_COPY_ROWS` (2300) that pins the exact rows appended by a copy.

**Inputs/outputs.** `copy_graph` reads a source table (itself or another), writes new rows to `self` in child-first order and returns the new root; it also copies span, materialization source, authored exposure, compiler partition and occurrence ids (1224-1230, 1433-1450). `SymbolCopy` mutates the program in place: after `copy_from_self` it rewrites symbols and type handles inside the new copy only.

**How it decides.**
- Memoisation: `CopyGraph.expressions` is a sparse `Vec` indexed by the source handle's `arena_index`, entry valid when `original == expression` (1217-1222, 1441-1446); same for match-subject captures (1530-1549). The memo entry is written after the node is built (1446) and the `materialization_source` copy is done after the memo write (1447-1450).
- `SavedSubject` (1261-1277): if its capture has no copy yet **and** the source is `Own`, the node is inserted as is, sharing the original capture; otherwise the capture and pattern head are copied.
- `SymbolCopy::symbol` (479-487): linear `find` over `substitutions`; invalid symbols pass through unchanged.
- `SymbolCopy::type_reference` (695-724): dedup by linear search of `types` for the source handle (699-703); `type_origins` (726-750) linearly scans all `generic_application_origins` per copied type.
- `expression_payloads` visits each copied node once, guarded by `visited_payloads` (503-510); it recurses into `materialization_source` (511-515).
- Rebase (2775-2803): for each expression `index < frontier` => `retain_base`, else `rebase_appended`; `None` => `Err(())` (2795, 2797).
- `set_authored_expression_exposure` (2689) and `set_compiler_selection_partition` (2711) `assert_eq!` when a different value is already set (panic, not error).
- `match_pattern_dependencies` (2510-2539): for `Destructure` it returns `head` and `pattern.condition`, and `fields` is set to `HandleSpan::empty()` (2519), so the `Equal(value)` expressions of the fields are never listed.

**Cases by shape.**
- `copy_graph` matches all 22 node kinds, no wildcard (1231-1432).
- `expression_payloads` matches all 22 kinds explicitly (517-654).
- `display_name` matches all kinds (20-106). `StructLiteral` displays only the type name (101); `TypeExpression` displays `"type"` (104); `ZeroValue` displays `"zero_value<type>()"` (105).
- `copy_graph` copies type handles by value for `Cast.target_type` (1309), `Cast.semantic_domain_arguments` (1313), `StructLiteral.application_type` (1410), `TypeExpression`/`ZeroValue` (1427, 1430), `Call.requirement_application` (1338), destructure `application_type` (1575): these are not re-copied, so the copy shares type handles with the source table. `SymbolCopy` re-copies them for the in-program case (573-576, 628, 647, 530) but not `requirement_application` (not remapped in the Call arm 578-588; read).
- `SymbolCopy` cast arm rewrites `target_type`, `semantic_domain_arguments`, `semantic_domain_symbol` (571-577) but not `target_label` / `semantic_domain` names (name spans copied by `copy_name_path_members` only).

**Same job elsewhere.**
- Duplicated across three crates (grep `struct ExpressionTable|struct StaticMachineArgument`): ST `values_and_types.rs:1440/1877`, SRT `values.rs:2407/3146`, typed-trees `values.rs:2861/4441`.
- `copy_static_argument_from`, `copy_static_argument_from_self`, `remap_static_argument_symbols`: SRT `values.rs:326/334/368` and typed-trees `values.rs:956/964/998`. SRT copies are used only inside `copy_graph` (1326/1329); the SRT `remap_static_argument_symbols` has **no call site outside itself** (only recursive calls at 381 and 390); all external callers found (`pipeline/04.../body_cloning_and_rewriting.rs:1211-1222`, `typed-trees/src/...`) are on the typed-trees table (inferred from the file locations; receiver types not read).
- `display_destructure_pattern`, `BinaryOperator::display_name`, `UnaryOperator::display_name`, call display: also in ST `values_and_types.rs:1040, 1211, 1236, 1134`. `display_name_path` also in typed-trees `values.rs:540` (different name type).
- `match_pattern_dependencies`: three copies — SRT `values.rs:2510`, ST `values_and_types.rs:1503`, typed-trees `values.rs:3062`. The ST version includes the field `Equal` values and `pattern.guard` (1512); the SRT and typed-trees versions use `HandleSpan::empty()` for fields and `pattern.condition`. Grep `match_pattern_dependencies` shows 28 lines; 23 are call sites in other crates (pipeline 02, typed-trees, validation, build-time-evaluation); the receiver type per call was not resolved.
- Rebase validate/apply: 4th copy of the pattern in this file (2775/2805).
- Symbol substitution is done twice: `SymbolCopy` (this file) and typed-trees' own `remap_*` machinery (typed-trees not read).

**Unused / one-caller (greps).**
- `copy_expression_with_symbols` (412): 0 callers outside this file's tests (tests at 998, 1102, 1130); grep `copy_expression_with_symbols|copy_proof_fact_with_symbols` over omega-rust.
- `copy_proof_fact_with_symbols` (426): 1 non-test caller (`pipeline/02.../symbols.rs:4625`).
- `remap_static_argument_symbols` (368): 0 external SRT callers (see above).
- `ExpressionTable::copy_from` (1188): not counted (name shared with typed-trees).

**Stated limits.** `expect("...")` messages state invariants, e.g. "qualified static member belongs to its source expression table" (358). `StoredAuthoredSelectionOccurrenceId::occurrence` expects "expression occurrence spans cannot contain the arena dummy sentinel" (2402).

**Fail-open risks.**
- `match_pattern_dependencies` (2519): "open". A consumer that uses this to discover sub-expressions of a match arm (e.g. to queue them for evaluation or lowering) would miss `Equal` field values of destructure patterns. 23 call sites in other crates; I did not check which table type each uses.
- `copy_graph` shares type handles with the source table (1309-1338, 1410, 1427-1430, 1575): "open"; if the destination table's `child_type_references` is not the same arena the handle would point at a wrong or invalid type (callers `insert_tree_from_source` 886-1030 always pass the same tree; not traced further).
- `SymbolCopy::symbol` (479-487): a symbol not listed in `substitutions` is returned unchanged; "stated" by the test `rebases_nested_static_arguments_simultaneously_and_preserves_absence` (1069) and by the invalid-symbol early return (480-482).
- `SymbolCopy` does not remap `Call.requirement_application` (578-588) nor `authored_domain_selection` of copied proof facts (459-469, the occurrence id is cloned unchanged): "open".
- `static_member` returns `None` for an invalid handle (2463-2471); display prints `<qualified member requires owning table>` (279) rather than failing.

---------------------------------------------------------------------
## 7. SRT `tests/authored_declaration_selection_ledger.rs`

Integration test `authored_declaration_selection_ledger_runtime_canary` (10-70). Records one resolved and one late-bound selection in `AuthoredDeclarationSelections`, checks `ordinal()` 0 and 1, `get`, that a record with `SymbolHandle::invalid()` returns `Err(InvalidSelectedSymbol)` and leaves the ledger equal to its clone (41-51), and that `SymbolResolvedTrees::rebuild_tables()` keeps the ledger (62-69). Reads `language_semantics` types through SRT re-exports only. No decisions, no fail-open.

---------------------------------------------------------------------
## 8. ST `derive/src/lib.rs`

**Purpose.** Two proc-macro derives that implement `crate::SyntaxEquivalent` (structural equality across two `SyntaxTextOwner`s).

**Shape.** `derive_syntax_equivalent` (22): for a struct, one match arm comparing each field with `crate::SyntaxEquivalent::same_syntax(left_i, right_i, left_text, right_text)` joined by `&&`; for an enum one arm per variant, plus a `_ => false` fallback (64). Unions rejected (58-61). `derive_plain_syntax_equivalent` (76): `self == other`, ignoring both text owners.

**How it decides.** Fields without a `SyntaxEquivalent` impl fail to compile (the generated call needs the trait). Enum variants of different kind are unequal (`_ => false`).

**Cases by shape.** Structs, enums with named/unnamed/unit variants. No generics bounds are added beyond `split_for_impl`.

**Unused / one-caller.** Used by ST only; grep `SyntaxEquivalent` outside `syntax-trees/**` gives 3 files (`pipeline/01.../tests.rs`, `pipeline/02.../preparation_machine_equations.rs`, `lowering_machine.rs`; count 1 each).

**Stated limits.** The header says deriving guarantees "every struct field and enum variant ... participates" (7-11). `PlainSyntaxEquivalent` types (e.g. `TableMatchArm`, `TableCastExpression`) use `==`, which compares handles and spans, not text.

**Fail-open risks.** none seen. Note: for an enum with exactly one variant the generated `_ => false` arm is unreachable (compiles with a warning only; not checked).

---------------------------------------------------------------------
## 9. ST `control_flow_and_names.rs`

**Purpose.** Syntax-level statements, inline-assembly graphs, `Identifier` (text, owned or backed by retained source), and `SyntaxTextOwner` (the page store that lets identifiers borrow from source text).

**Shape.**
- `control_flow::assembly` (8-126): `AssemblyControlFlow` (23), `AssemblyBlock` (44), `AssemblyExit` (55), `AssemblyTable` via `paged_roster!` (63).
- `control_flow::statement` (127-569): `StatementTable` (149, `paged_roster!` with statements, expression handles, identifier path members, transition targets/patterns, outcome proof selectors, nested assembly roster), `StatementNode` (324: same nine kinds as SRT but identifier-only), `TableCall` (399), `TableTransition` (448), `TransitionTargetNode` (498).
- `names::identifier` (573-720): `Identifier` (581) = `IdentifierText {Missing, Owned(Box<str>), SourceBacked{source: Handle<RetainedSource>}}` + `source_span`; `as_str(context)` (619); `trailing_span` (637); `JoinedSyntaxPath` for `[Identifier]` (689-719).
- `names::text_owner` (722-965): `RetainedSource{generation, text: Arc<str>}` (732), `SyntaxTextOwner{sources: Vec<RetainedSource>}` (742) with `retain` (747), `retain_from` (757), `adopt` (768), `SyntaxTextContext::source_text` (780); `Record` impls (846-909).

**Inputs/outputs.** `retain` draws a generation from a process-wide `AtomicU32` (729, 748-752) and returns `Handle::from_parts(1, generation)` (754). `retain_from`/`adopt` insert foreign pages sorted by generation, skipping generations already present (757-777). `source_text` asserts `arena_index == 1` (783), checks the last page first, then binary-searches (788-797), and panics with "retained syntax source does not belong to this forest" if absent (796).

**How it decides.** Page identity = generation number only (a second page with the same generation is dropped, 759-764). `Identifier::as_str` for source-backed slices the page by the identifier's own span (627-628). Storage decode rejects zero generation (851), non-increasing generation order (896-901) and remaps generations through `define_generation` (855).

**Cases by shape.** `Identifier` has three text forms. `TransitionTargetNode` has Named/Value/Terminal.

**Same job elsewhere.** `Identifier` (ST) vs `DiagnosticName` (SRT file 5): both = text + span, one resolves through a text context, the other owns/shares `Arc<str>`. `StatementTable`/`StatementNode` here vs SRT (file 1) and typed-trees (not read).

**Unused / one-caller.** `Identifier::trailing_span` (637): not grepped (suspected rare). `SyntaxTextOwner::retain_from` used by `SyntaxCopy::new` (copies.rs:1052) and tests.

**Stated limits.** none.

**Fail-open risks.**
- `retain_from`/`adopt` keep the page already stored when generations collide (759-764): "stated" by the test `duplicate_page_identity_cannot_change_its_retained_text` (949), which pins rejection at decode, not at `retain_from`. A different text under the same generation in memory would silently keep the first (open; generations are process-unique so collisions come only from decode, which is checked at 896-901).
- Missing identifier text renders as `""` (624); `Identifier` is not `PartialEq` — equality goes through `SyntaxEquivalent` comparing text only (lib.rs:53).

---------------------------------------------------------------------
## 10. ST `copies.rs`

**Purpose.** Copy items, bodies, types and expressions from one `SyntaxTrees` into another, keeping shared-expression identity inside one copy transaction and making retained source pages resolvable in the destination.

**Shape.**
- `SyntaxCopy<'destination>` (1027): destination `&mut SyntaxTrees` plus `expression_copies`, `type_substitutions`, `expression_substitutions`, `supplied_type_copies`, `retain_alias_header_role`, `lifetime_substitutions`, `scoped_lifetimes`, `array_length_substitutions`, `match_subject_copies`. `SyntaxCopy::new` (1051) calls `destination.text.retain_from(&other.text)`.
- `SyntaxTrees` entry points (1082-1198): `copy_item_from`, `copy_expression_from`, `copy_proof_fact_from`, `copy_state_signature_from`, `copy_state_signature_in_lifetime_scope`, `copy_static_machine_argument`, `transfer_static_machine_arguments`, `copy_mathematical_definition`, `instantiate_type_reference_from` (1161), `copy_type_reference_handle`.
- `body_copies` (7-1004): `copy_statement_node` (208), `copy_type_reference_handle` (395-589), `copy_expression_handle` (706-880), `copy_destructure_pattern` (664), `copy_match_subject` (632), spans, `copy_mapped_span` (973).
- `item_copies` (1006-1918): `copy_item` (1311-1504, all 14 `Item` variants), `copy_machine` (1625), `copy_trait_definition` (1705), `copy_type_parameter_span` (1744), contracts, data members, `copy_proof_fact` (1220).

**Inputs/outputs.** Reads `other: &SyntaxTrees`, writes destination tables; returns new handles. Source-text pages are retained in the destination first.

**How it decides.**
- Expression memo: sparse `Vec` indexed by source `arena_index`, valid when `source == handle` (723-728, 872-878); `expression_substitutions` (linear find by handle) wins over the memo (715-721).
- Type substitution: if the source handle is in `type_substitutions`, the actual replaces it; if the source has alias-application origins a fresh occurrence is made and origins are copied (404-432).
- Lifetime renaming: `copied_lifetime` (1201-1218) matches by `(source_span, spelling)` then by spelling only in `scoped_lifetimes` — string compare on `name.as_str(other)`.
- Fixed-array `ConstParameter(name)` replaced by an `Expression(actual)` when `(span == name.source_span() && spelling == name.as_str(other))` (467-474).
- Const items (1368-1418): `qualification` is kept only if both `destination` and `initializer` still equal the source's (1409-1416), else `None`; `pending_qualification` is rewritten to the new handles only if all three fields match (1395-1398), else cloned unchanged (1405).
- `retain_alias_header_role` (520-524) toggles copying the alias-header marker.

**Cases by shape.** `copy_statement_node` handles all nine kinds (213-362). `copy_item` handles Capability, Conformance, Const, Data, TypeAlias, Domain, Measure, Module, Operator, Package, Proposition, Use, Machine, Trait. `copy_type_parameter_span` handles six `TypeParameterKind`s. `copy_destructure_pattern` (664-704) has no `condition` field (ST patterns have `guard` only).

**Same job elsewhere.** The SRT `table_copies::copy_graph` (SRT values.rs:1208) and typed-trees' `copy_*` do the same walk for other table types; within ST the expression walk here (730-867) and `inspection.rs` snapshot walk (410-578) and `values_and_types.rs` display walk (921-1037) are three exhaustive matches over `ExpressionNode`.

**Unused / one-caller (grep over omega-rust excluding `syntax-trees/src/**`).**
- `instantiate_type_reference_from`: 2 callers (`pipeline/02.../preparation.rs:4574, 4842`).
- `copy_state_signature_in_lifetime_scope`: 1 (`preparation.rs:3147`).
- `copy_item_from`: 2 non-test (`build-time-evaluation/src/layouts.rs:2665`, `pipeline/02.../preparation_generic_data.rs:3740`) + 3 in tests.
- `copy_proof_fact_from`: 3 (`typed-trees/src/calls_and_control_flow.rs:2600`, `preparation_generic_data.rs:2138, 3619`).
- `transfer_static_machine_arguments`, `copy_state_signature_from`, `SyntaxTrees::copy_static_machine_argument`: 0 call sites outside `syntax-trees/src` (grep pattern covered all three; none listed).

**Stated limits.** Doc comment on `copy_state_signature_in_lifetime_scope` (1111-1112): "The requirement's own telescope shadows the enclosing trait telescope."

**Fail-open risks.**
- `pending_qualification` cloned with source handles when fields differ (1405): "stated" by test `constant_destination_debt_is_remapped_without_settling_or_refreshing_stale_inputs` (lib.rs:1506), which asserts the stale obligation is kept (`assert_eq!(pending, stale)`, lib.rs:1564). The kept row refers to handles of the other table (the test asserts `pending.initializer != copied.value`).
- `qualification` dropped (becomes `None`) when handles differ (1409-1416): fail-closed-ish, stated by the same test (`assert!(copied.qualification.is_none())`).
- Unmatched type/lifetime/array-length substitutions fall back to the original name or a plain copy (474, 1217): "traced" for lifetimes only to the `unwrap_or_else(|| name.clone())`.

---------------------------------------------------------------------
## 11. ST `declarations.rs`

**Purpose.** Syntax-level item records and their paged storage.

**Shape.** `Item` enum (611-626): Capability, Conformance, Const, Data, Domain, Measure, Module, Operator, Package, Proposition, Use, Machine, Trait, TypeAlias. Records: `DataDefinition` (119), `Machine` (339, ~30 fields incl. `nested_machines: HandleSpan<Item>`), `State` (377), `StateSignature` (409), `TraitDefinition` (386), `ConformanceMember` {Machine, TraitDefault, Synthesized} (164), `TypeParameterKind` (207), `MachineParameterContract` (232: RequirementIdentity, Structural, Nominal), `ProofFact` (545), `ConstDefinition` (661) with `normalization`, `qualification`, `pending_qualification` (668-670), `ExternalBinding` {Syscall, CompilerIntrinsic, VtableSlot, VtableField, TableFunction} (727). Storage: `ItemTable` (787), `StateStorage` (796), `DeclarationStorage` (810) through `paged_roster!`. `MachineNode` (1270) and `TraitNode` (1287) are reduced projections.

**Inputs/outputs.** `ItemTable::insert_machine` (1203) and `insert_trait_definition` (1221) build `MachineNode` / `TraitNode` rows in `state_storage`. `append_proof_fact` (1173) appends a parallel `Option<SourceSpan>` row so handles line up (debug-asserted 1179-1180).

**How it decides.** `requirement_facts` (982-991) keeps contracts with `kind == Requires`. `proof_fact_source_span_handle` (1241) reuses the proof-fact handle indices for the span arena.

**Cases by shape.** `Item::as_data / as_type_alias / as_domain / as_machine` (629-655) — one variant each.

**Same job elsewhere.** The tree form here and the SRT records (files 1-2) describe the same declarations twice (ST handle-based, SRT symbol-resolved). `SyntaxTrees::insert_machine/insert_trait_definition` (lib.rs:1842-1848) feed `MachineNode`/`TraitNode`.

**Unused / one-caller.** `ItemTable::insert_machine` / `insert_trait_definition`: each called once from `SyntaxTrees::insert_item` (lib.rs:1823-1824) (read). The produced `MachineHandle`/`TraitHandle` is discarded there (lib.rs:1842-1848 return `()`); readers `machine()` (922) and `trait_definition()` (926) — grep not run (suspected 0 external callers; not verified).

**Stated limits.** none.

**Fail-open risks.** none seen. Defaults: `ExternalBinding::default()` is `Syscall{number: 0}` (735-739), `Item::default()` is `Use` (741) (open; used for `Default`-derived containers).

---------------------------------------------------------------------
## 12. ST `inspection.rs`

**Purpose.** Convert `SyntaxTrees` to a serde `SyntaxTreesSnapshot` (JSON) for phase inspection; `PhaseSnapshot` impl at 2688.

**Shape.** Snapshot types per kind (`ExpressionSnapshot` 160, `TypeReferenceSnapshot` 53, `ItemSnapshot` 807, `StatementSnapshot` 1958, `StaticArgumentSnapshot` 32, ...). Functions: `snapshot_expression_handle` (402), `snapshot_type_reference_handle` (271), `snapshot_item` (1168), `snapshot_statement` (2180), `snapshot_state_signature` (2133) and `snapshot_state_signature_node` (2140) (same macro `state_signature_snapshot!` 2084), `SyntaxTrees::snapshot` (2668).

**Inputs/outputs.** Reads `SyntaxTrees` tables; produces nested owned snapshot values with text resolved and spans split into `source_id/start/end`.

**How it decides.** Invalid expression handle => `ExpressionSnapshot::Name{path: []}` (406-408), invalid type handle => `TypeReferenceSnapshot::Missing` (275-277). Static argument precedence: type ref, const literal, evidence projection, application, else path (617-653). `LocalData.role` uses `format!("{:?}")` (2328); `Atomic.ordering` uses `format!("{:?}")` (453).

**Cases by shape.** All matches are exhaustive over their enum. Statement snapshot omits `TableTransition.proof_selectors`, `ends_dispatch`, `source_span` (2338-2377) and Named target `source_span` (2400-2417). `ItemSnapshot::Machine` has no `generic_data_template`, `default_body_trait`, `target_family_interface` (1410-1490), `ItemSnapshot::Trait` has no `refines`/`refinement_clauses` (1491-1521), `Const` has no `normalization`/`qualification`/`pending_target_observation` (1232-1241), `TypeAlias` has no `authored_expansion` (1295-1309), `Data`/`Domain` have no `cleanup_selection` (read; compared with the field lists in `declarations.rs`).

**Same job elsewhere.** The `Match.capture` and `SavedSubject.capture` fields are emitted as `(arena_index, generation)` (413-416, 433), i.e. raw arena handle values, although the header says snapshots avoid exposing raw arena handles (2-3). `snapshot_state_signature` and `_node` are the same function twice (2133-2145): `_node` has 1 caller (1515), `snapshot_state_signature` 2 callers (1584, 1684).

**Unused / one-caller.** `snapshot_state_signature_node`: 1 caller (command: Grep in syntax-trees).

**Stated limits.** none.

**Fail-open risks.** Snapshot is read-only inspection; an invalid expression renders as an empty name (406-408), indistinguishable from a real empty path (open; for display only).

---------------------------------------------------------------------
## 13. ST `lib.rs`

**Purpose.** Container `SyntaxTrees`, structural equality (`SyntaxEquivalent`), the `paged_roster!` macro that builds paged tables with checkpoint/rewind/adopt, `ParsedSource(s)`, and the root-level operations (`extend_from`, `adopt_page`, checkpoint/rewind).

**Shape.**
- `SyntaxTrees{source_id, roots, tables}` (1628), `SyntaxTreeTables{text, items, expressions, statements, type_references}` (1635), `SyntaxTreeRoots{items, constant_extent_probes, mathematical_definitions}` (1644).
- `equality` module (31-192): trait `SyntaxEquivalent` (37), impls for `Identifier` (compares text only, 53), `Handle`/`HandleSpan` (by `==`, 80-89), `Vec`, `Option`, `Box`/`Arc`, tuples, arenas (`equal_by`, 135-142), a `plain!` list (143-191: bool, usize, u8..u64, i64, String, `SourceSpan`, `Span`, `SourceId`, `SourceBytes`, `OperatorSpelling`, `FloatFormat`, `IntegerLiteral`, several `language_core` enums, `ArithmeticDomain`). `PartialEq for SyntaxTrees` (56-77): compares `source_id`, `roots`, and the four tables via `same_syntax`; does **not** compare the text owner directly (only via name resolution).
- `paged_roster!` (202-258): generates table struct + `append_checkpoint`, `rewind_appended`, `share_pages`, `new_with_generation`, `adopt`, `begin_page`.
- `extend_from` (1789-1803), `adopt_page` (1805-1811), `begin_page` (1813), `append_checkpoint`/`rewind_appended` (1661-1685), `const_expression_contains_name` (1716-1726), `constant_initializer_owns_selection` (1767-1787), `insert_item` (1821-1840).
- ~1300 lines of tests (307-1613).

**Inputs/outputs.** `extend_from(other)`: for every root item and root mathematical definition of `other`, `copy_item_from` into self and push; then `remap_supplied_operand_owners(frontier, &declarations)` (1790-1802). `adopt_page` concatenates roots and adopts paged tables without remapping (1805-1811; test 486 pins "without remapping roots").

**How it decides.**
- `const_expression_contains_name` (1716-1726): `Name` => true; `Binary` recurses on both sides; `Unary` recurses on the operand; every other kind (`Call`, `Cast`, `Member`, `Indexed`, `Match`, ...) => `false`.
- `constant_initializer_owns_selection(reference)` (1767-1787): scans all root `Const` items and matches the span against `normalization.selections[*].reference`, `builtin_operators`, `call_selections[*].0` — keyed on source span.
- `rewind_appended` truncates root vectors to the checkpointed lengths (1678-1684).

**Cases by shape.** `const_expression_contains_name` handles exactly Name/Binary/Unary (comment-free). `insert_item` (1821-1840) matches Machine and Trait specially.

**Same job elsewhere.** `adopt_page` vs `extend_from`: the first moves handles as is, the second re-copies items; `extend_from` copies `roots.items` and `roots.mathematical_definitions` but **not** `roots.constant_extent_probes`; `adopt_page` (1805-1809) also does not move `constant_extent_probes`.

**Unused / one-caller (greps).**
- `constant_extent_probes` (1646): written outside ST only at `psi-compiler/src/source_compilation.rs:158-167` (retain, then re-copy each probe with `copy_expression_from` right after `extend_from`, 164-168) and a test (`pipeline/01.../tests.rs:2741`); read at `pipeline/02.../lowering.rs:1201` and `build-time-evaluation/src/const_generic_expressions.rs:6111`. So the non-copy in `extend_from` is compensated by that caller (traced).
- `extend_from`: non-test callers `psi-compiler/src/source_compilation.rs:164`, `pipeline/02.../preparation.rs:5260, 5296` (the last two not checked whether test code), `pipeline/01.../expressions.rs:2203, 2253` (appear to be tests, not checked). `adopt_page`: `psi-compiler/src/lib.rs:488`, `build-time-evaluation/src/lib.rs:329`.
- `const_expression_contains_name`: 2 callers (`pipeline/02.../preparation_const_evaluation.rs:1504, 3239`).

**Stated limits.** The header says `SyntaxEquivalent` "compares forests structurally through both text owners so Build continuation sharing observes every retained row" (14-16). Test `contextual_syntax_equality_includes_detached_nodes_and_logical_name_text` (1467-1503) records that name equality is spelling-only ("name equality retains its existing spelling-only contract", 1498).

**Fail-open risks.**
- `const_expression_contains_name` returns `false` for any kind other than Name/Binary/Unary (1724): "open". Wrong program: a const initializer like `f(NAME)` or `NAME as u8` reports "contains no name" to its two callers (`preparation_const_evaluation.rs:1504, 3239`, not read).
- `PartialEq for SyntaxTrees` ignores `tables.text` identity by design (1498 test comment, "stated").

---------------------------------------------------------------------
## 14. ST `values_and_types.rs`

**Purpose.** Syntax-level type references (`TypeReferenceNode`, constraints, fixed array length) and the side tables that track their provenance (alias applications, generic application origins, const-argument normalizations, supplied-operand owners, pending const arguments), plus syntax expressions (`ExpressionNode`, `Table*` payloads), their display, and `FloatSpelling`.

**Shape.**
- `TypeReferenceTable` (paged roster, 18-32, 11 arenas); `TypeReferenceNode` (771: Reference, Constrained, FixedArray, Slice, Generic, ConstExpression, DynamicTrait, Named, Unit); `FixedArrayLength` {Literal, ConstParameter, ConstCall, Expression} (813); `ConstArgumentNormalization` (109); `TypeAliasApplicationOrigin` (43).
- Side-table methods (132-760): `retain_alias_header_reference` (141), `retain_type_alias_application` (162), `supplied_operand_owner` (177), `retain_supplied_operand_owner` (195), `retain_unresolved_supplied_operand_owner` (230), `remap_supplied_operand_owners` (253), `retain_pending_const_argument(_uses)` (274/300), `retain_const_argument_normalization` (338), `retain_const_argument_expression` (366), `..._materialized_expression` (386), `retain_generic_application_origin` (426), `generic_nodes` (491), `alias_use_at` (646), `replace_type_occurrence` (683), `fresh_type_occurrence` (637).
- `values::expression`: display (911-1244), `ExpressionTable` (1440, 9 arenas incl. parallel `source_spans`), `ExpressionNode` (1703, 22 kinds), `TableCallExpression` (1864), `StaticMachineArgument` (1877), match types (1968-2072). `set_call_acknowledgement` (1668).
- `values::float_spelling::FloatSpelling` (2080): Owned / SourceBacked / Materialized.

**Inputs/outputs.** Side tables are `Arena`s of rows scanned linearly by handle (e.g. `is_alias_header_reference` 147-151, `const_argument_normalization` 320-329, `generic_application_origin` 414-424, `type_alias_applications` 153-160, `supplied_operand_owner` 177-193).

**How it decides.**
- `supplied_operand_owner` (177-193): `Ok(declaration)` if a row with a valid declaration; `Err("supplied operand lost its declaration during syntax copying")` if the row exists with an invalid declaration; `Ok(invalid)` if no row (192).
- `retain_supplied_operand_owner` (195-217): a second, different declaration for the same operand marks the row invalid (207-209).
- `remap_supplied_operand_owners` (253-272): rows past the frontier get their declaration looked up in `declarations` (linear); not found => `unwrap_or_default()` i.e. invalid (270).
- `retain_*_normalization/_origin` use `assert!` (346-347, 382, 402, 431-432): duplicates panic (test at lib.rs:941).
- `alias_use_at` (646-681): requires exactly one application row with the given `declaration` span; `None` if zero or more than one (656-659) or if the node is not Named/Generic (663-666).
- `generic_nodes` (491-511): Generic nodes that are not alias headers, not the `application` of a generic-origin row and not the `application` of an alias-application row.
- `retain_pending_const_argument_uses` (300-318): finds every Generic or Constrained-domain node whose argument list contains the argument (full scan).
- `replace_type_occurrence` (683-719): moves rows of five side tables from `source` to `destination` (frees existing destination rows); it does **not** move `alias_header_references`.
- `FloatSpelling::same_syntax` (2146-2164): equal text and both-or-neither `Materialized` with equal literal.
- `match_pattern_dependencies` (1503-1530): head, field `Equal` values, and `guard`.

**Cases by shape.** `ExpressionNode::display_name` exhaustive (922-1037). `StaticMachineArgument::display_name` (1172): `<structural type>` for any valid type handle.

**Same job elsewhere.** Display code duplicates SRT `values.rs:8-320` and typed-trees; `ExpressionTable`, `StaticMachineArgument`, `match_pattern_dependencies`, static-member insert/get/get_mut (1454-1479) duplicate SRT `values.rs:2456-2482`. `generic_application_origin(s)` rows exist in both ST (127) and SRT (`GenericApplicationOrigin` at SRT type_system_and_names.rs:160).

**Unused / one-caller (greps).** `retain_alias_header_reference`: callers `copies.rs:523` and `pipeline/02.../preparation.rs:4921` (2). `copy_static_argument_*`: not here.

**Stated limits.** The error string at 188 and the panics at 346/382/402/431 are the only stated limits.

**Fail-open risks.**
- `supplied_operand_owner` returns `Ok(ItemHandle::invalid())` when no row exists (192): means "no owner", distinct from the `Err` "lost" case; callers (e.g. `copies.rs:529-536`) copy `Err` as an "unresolved" row. Traced for the copy path only.
- `remap_supplied_operand_owners` turns an unmapped declaration into "lost" (270) — fail-closed (becomes `Err` on read).
- `FixedArrayLength::same_value` (833-847): `Expression(a)` vs `Expression(b)` compares **handles**, not the expressions they stand for (844); unequal handles with equal expressions compare different (fail-closed direction).

---------------------------------------------------------------------
## 15. terminal-psi codec `block_wire.rs`

**Purpose.** Canonical byte encoding/decoding of Terminal Psi blocks: block header, operations (all `OperationKind`s), operation results, and terminators.

**Shape.**
- `operation_tags` (394-550): ~100 `u8` constants; unassigned tags: `0, 48, 49, 51-55, 57, 61, 67, 71-74, 76, 80, 82, 87` plus, by reading the list, 88 and everything >= 138 (read).
- `encode_block` (1213) / `decode_block` (1252); `encode_operation` (1227) / `decode_operation` (1271): id, optional `static_reach_binding` (bool + u32), result tag 0/1/2 (Unit/Scalar/Structural), kind, optional `suspension_crossing`.
- `operation_wire!` macro (1299-1358) generates `encode_operation_kind` and `decode_operation_kind` from a table of `rows` (tag => variant {field: wire-kind}), `paired` rows (absent tag | present tag with optional obligation, 1633-1644) and hand-written `encode(...)`/`decode(...)` arms (1645-1817).
- Sub-modules: `atomic_operations` (7-288; `LOAD..COMPARE_EXCHANGE_ONCE` sub-tags 1-6, orderings 1-5, fence 1-3), `call_operations` (289-393; register access), `scalar_operations` (551-662; trapping integer sub-tags 1-8, float result policy tags 1 and 4), `storage_operations` (663-927), `terminator_wire` (928-1012; `wire_enum!` with tags 19 InvokeNoNormal, 17 ExternalCompletion, 14 Jump, 2 Return, 13 ReturnUnit, 18 ReturnStructural, 15 Conditional, 16 StructuralCase, 4 Crash; `CompletionKind` 1 => ProcessExit).

**Inputs/outputs.** `Writer`/`Reader` from `super::wire`; ids via `writer.id` / `reader.id("Name")`; encodes `terminal_psi::{Block, Operation, OperationKind, Terminator}`.

**How it decides.**
- Decoding is by tag byte; an unknown tag returns `CodecError::InvalidTag(<kind>, tag)` at each level (e.g. 1354, 161, 136, 262, 284, 621, 658, 1282).
- Encoding chooses between plain and "projected/restricted" tags by whether the path / obligation is empty/present (`encode_release_reference` 732-736, `encode_primitive_scalar_read` 765-772, `encode_element_view_read` 811-815, `encode_write_only_primitive_store` 855-872, `encode_atomic_access` 39-43); decode of the projected tag rejects an empty path (754, 781, 834, 880).
- `encode_register_access` (297-320) refuses unless `event.operation().is_admitted()` and `service.is_some() == requires_service_authority()`; decode rechecks agreement (335-339) and rejects descriptor-table operations (341-349).
- `encode_once_custody` / `decode_once_custody` (171-243): rejects non-canonical `AtomicCompareExchangeOnceResultCustody` both ways (175, 237).
- `encode_operation_kind` is an exhaustive match on `OperationKind` (no wildcard in the macro-generated match, 1314-1336).

**Cases by shape.** Fixed lists: trapping integer ops (8 sub-tags), float result policy (2: Trapping, Saturating), memory orderings (5), fence orderings (3), atomic events (6), read-modify-write ops (5), terminator kinds (9), completion kinds (1: `ProcessExit`, 999).

**Same job elsewhere.** Several parts of the same crate (`machine_wire`, `structural_signature_wire`, `wire_fields`, `contract_wire`, ...) share `wire_record!`/`wire_enum!`/`Field` machinery (imports 1201-1211); not read.

**Unused / one-caller.** `encode_float_result_policy` (626) and `encode_trapping_integer` (560) always return `Ok(())` though typed `Result` (read). Public API of the file: `encode_block` (1213), `decode_block` (1252) (callers not grepped).

**Stated limits.** Tests: `TAGS_WITHOUT_TABLE_ROWS` (1131-1133) lists tags without rows; the test `an_operation_tag_without_a_table_row_rejects_the_module` (1167) checks them plus 138..=255. Tag 88 is neither assigned in `operation_tags` nor in that list (read; the list ends `..., 80, 82, 87`).

**Fail-open risks.** none seen for accept paths; unknown tags reject. The untested gap at tag 88 (only tested via the `match` fallthrough at 1354, which would reject it) is not a risk (traced: no constant, so decode hits `tag => InvalidTag`).

---------------------------------------------------------------------
## Cross-file observations (this unit)

- Three crates keep parallel definitions of the same data model: `ExpressionTable`, `StaticMachineArgument`, `display_name_path`/display code, `PrimitiveType`, `DataShapeKind`, `MachineParameterContractView`, `match_pattern_dependencies` (ST, SRT, typed-trees). Greps: `enum PrimitiveType|enum DataShapeKind|struct DiagnosticName|enum MachineParameterContractView|enum TypeConstraint\b|struct StaticMachineArgument|struct ExpressionTable`.
- Lookup-by-symbol uses four mechanisms: linear `.iter().find` (SRT lib.rs 148-157, 264-273, 768-780), sorted vec + binary search (`MachineDeclarations`), revision-keyed hash map (`data_definition_position`), address+sample keyed prefix cache (`local_statement_position`, `parameter_position`).
- ST side tables in `TypeReferenceTable` are all scanned linearly by handle.

---------------------------------------------------------------------
## Coverage

All ranges read in full (21,806 lines in 15 files):
- SRT `control_flow_and_calls.rs` 1-1567 (1567)
- SRT `declarations_and_machines.rs` 1-676 (676)
- SRT `lib.rs` 1-1435 (1435)
- SRT `storage_and_indexes.rs` 1-646 (646)
- SRT `type_system_and_names.rs` 1-1118 (1118)
- SRT `values.rs` 1-3323 (3323)
- SRT `tests/authored_declaration_selection_ledger.rs` 1-70 (70)
- ST `derive/src/lib.rs` 1-85 (85)
- ST `control_flow_and_names.rs` 1-966 (966)
- ST `copies.rs` 1-1918 (1918)
- ST `declarations.rs` 1-1294 (1294)
- ST `inspection.rs` 1-2695 (2695)
- ST `lib.rs` 1-1947 (1947)
- ST `values_and_types.rs` 1-2248 (2248)
- terminal-psi codec `block_wire.rs` 1-1818 (1818)

Skimmed: none (test bodies and long `expect` lists were read but are described only at the level of what they pin). Not read: none.

Extra non-unit reads for tracing (not part of coverage): `typed-trees/src/values.rs` 3060-3090 and 3265-3273 region of `declaration_tables_and_index.rs` (via grep context), `psi-compiler/src/source_compilation.rs` 145-175, `pipeline/02.../lowering.rs` 770-805.

## Five things a maintainer most needs to know about this unit

1. The authored-declaration-selection rebase is implemented per store four times in SRT (statements+targets, tree statements, proof facts, expressions) with the same `index < frontier => retain_base else rebase_appended` split; the stores must all be listed in `AuthoredSelectionExtensionFrontier` (lib.rs:106) and `prepare_occurrence_rebase` (lib.rs:321) or a new store with occurrence ids is silently not rebased.
2. `ExpressionTable`, `StaticMachineArgument`, display code, `match_pattern_dependencies`, `PrimitiveType`, `DataShapeKind` exist in ST, SRT and typed-trees as separate copies; the SRT and typed-trees `match_pattern_dependencies` ignore destructure field `Equal` values and use `condition`, while the ST one lists them and uses `guard` (SRT values.rs:2510-2539 vs ST values_and_types.rs:1503-1530); 23 call sites in other crates.
3. Copy machinery has three flavours with different handle semantics: `copy_graph` (SRT values.rs:1208) shares type handles with the source table; `SymbolCopy` re-copies types and substitutes symbols but is reached in non-test code only via `copy_proof_fact_with_symbols` (1 caller, `symbols.rs:4625`); ST `SyntaxCopy` copies everything and records substitutions by handle/span+spelling.
4. `ResolvedIndexes` (SRT storage_and_indexes.rs:73) is a cache that always compares equal and serialises to nothing; its prefix-position caches are validated by slice address plus three sampled rows (44-56), and it is reset to default in 3 places (`lowerer.rs:636`, `resolution.rs:222`, `preparation.rs:126`).
5. Terminal Psi block wire tags are a fixed table (`operation_tags` 394-550) with unassigned gaps pinned by a test (1131-1133); tag 88 is unassigned and not in that test's list; encode is exhaustive over `OperationKind`, so a new variant must be added to the `operation_wire!` table or the build fails.

## What you are unsure about

- Whether the SRT `arena_index` is 1-based so that `validate_...` (`index < frontier` on `enumerate()` index) and `apply_...` (`handle.arena_index() > frontier`) agree; I inferred yes from the tests (control_flow_and_calls.rs:1376, lib.rs:472) but did not read `arena`.
- Receiver types of the 23 `match_pattern_dependencies` call sites in other crates, and which of them use the SRT table.
- Whether `PrimitiveType::from_name` in SRT (type_system_and_names.rs:912) has any caller (names are shared with typed-trees; 103 occurrences).
- Whether `MachineNode`/`TraitNode` (ST declarations.rs:1270, 1287) have any readers; I did not grep `machine(` / `trait_definition(` accessors.
- Whether `extend_from` calls in `pipeline/02.../preparation.rs:5260, 5296` and `pipeline/01.../expressions.rs:2203, 2253` are test-only.
- The exact semantics of `Arena::free`, `OrderedRootArena::revision`, `OwnedIndex`, `PagedArenaSet::equal_by` (not read; used in claims about caching and equality).
