# Record: unit_070 (area 05-source-representations)

Checkout `4f3c255d0559`. Paths below are relative to `source-rust/subsystems/compiler/source-to-psi/representations/`
(written `R/`), with `SRT` = `R/symbol-resolved-trees`, `SYN` = `R/syntax-trees`.
All 42 ranges were read in full. Counts come from Grep (count or content mode) over `source-rust/`, command named inline.

## SRT/nodes/values.rs (1-2615)

1. Purpose: the resolved (post-name-binding) expression table. It stores every expression node kind as dense rows plus side
   vectors (source span, unresolved-name outcome, materialization source, selection exposure, compiler partition, selection
   occurrences), and supplies display, traversal, and a checked rebase of authored-selection occurrence ids.
2. Shape: `ExpressionTable { nodes: ExpressionNodeStorage, spans: ExpressionSpanStorage }` (values.rs:1316); `ExpressionNode` enum with 23 variants
   (:2165); `Table*Expression` row structs (:2192-2290); `StaticMachineArgument` (:2306, hand-written `Record` impl with marker `ARGSEL2` at :2324-2359);
   `TableNamePath` (:2375, includes `reserved_result_of`); `MatchPattern`/`TableDestructurePattern`/`TableMatchField`/`MatchFieldAction` (:2528-2567).
   Entry points: `ExpressionTable::insert` (:1539), `visit_children` (:2035), `direct_blocks` (:2013), `match_pattern_dependencies` (:1430),
   `prepare_authored_selection_rebase`/`apply_authored_selection_rebase` (:1775, :1831), `prepare_static_argument_rebase`/`apply_...` (:1705, :1763),
   `copy_static_argument_*` and `remap_static_argument_symbols` (:351-441), `display_name` (:19).
3. Inputs/outputs: mutated by resolution (stage 02) through the `insert*`/`set_*` methods; read by typing. Rebase takes a
   `AuthoredDeclarationSelectionSuffixRebase` closure/value and returns a prepared plan, then mutates (two-phase: validate then apply).
4. Decisions: `visit_children` walks operands by node variant (:2040-2100); a `Block` visits only `tail` (comment :2086, statements walked elsewhere).
   `match_pattern_dependencies` visits `head` and `condition` of a destructure pattern, not its retained `fields` or `guard` (:1436-1441), because resolution folds
   them into `condition` (comment :1434). `set_authored_expression_exposure` / `set_compiler_selection_partition` assert a slot is not set to a different value
   (:1626, :1648). Rebase refuses (`Err(())`) an occurrence outside the owner frontier (:1722, :1795) and a cyclic static member (:1737-1739).
   `display_name` keys on source spelling: `DiagnosticName` strings, `format!`s (:19-130). `unresolved_name` returns `Default` for an invalid handle (:1594).
5. Cases by shape: `display_name` for `StructLiteral` prints only `type_name` (:124); `Name` prints `<missing applied declaration>` for a dangling `static_application` (:109);
   `TypeExpression` prints a constant "type" (:127). `static_argument_for_actual`-style fixed list is not here (see SYN bodies.rs).
6. Same job elsewhere: `ExpressionNode`, `BinaryOperator`, `UnaryOperator`, `Table*Expression`, `display_name` and `visit`-style traversal are re-declared per representation.
   Grepped `enum BinaryOperator|enum UnaryOperator`: 3 definitions (here :2430/:2470; `SYN/expressions/values.rs:578/:602`; `R/typed-trees/expressions/values.rs:5534/:5658`).
   `BinaryOperator::display_name` is written three times (here :311, `SYN/expressions/display.rs:326`, typed-trees `values.rs:982`) with the same operator strings
   (read here and in SYN; typed-trees suspected, not read). `ExpressionNode::display_name` here (:19) and `SYN/expressions/display.rs:10` are the same match
   on different tables (read both). `direct_blocks` exists here (:2013) and in typed-trees `expressions/values.rs:6259` (grepped, suspected same job).
   `copy_pattern_fields`/`copy_transition_pattern` here (via table_copies.rs) have typed-trees twins `copy_pattern_fields_from`/`copy_transition_pattern_from`
   (`typed-trees/expressions/values.rs:2410, :2332`; grepped, not read).
7. Unused/one-caller: `direct_blocks` here: 3 call sites outside the definition (`nodes/state_bodies.rs:94`, `:99`, `pipeline/02.../names/statements.rs:204`; grep `direct_blocks`).
   `reserve_name_path_members`/`set_*_at_offset` are used by tests in this file; external use not grepped. The comment "We represent ... static member projections" (:1-4)
   makes no single-owner claim.
8. Stated limits: `StoredAuthoredSelectionOccurrenceId` "cannot contain the arena dummy sentinel" (:1311). Block comment: "A block's statements stay with their body" lives in symbol_copies.rs.
9. Fail-open: `materialization_source` returns invalid (no source) when the index is out of range via `.get(..).unwrap_or_default()` (:1567-1573); a reader treating invalid as
   "no distinct authored computation" would use the materialized value as authored. Open (no comment); invalid input = handle index beyond the side vector.
   `prepare_authored_selection_rebase` skips nodes that are not `Call` or an applied `Name` (`_ => continue`, :1812), so static arguments held elsewhere (e.g. in type references) are not rebased here (traced: the same rebase for statements is in selections.rs).
   Otherwise none seen.
Tests (:447-961, :2570-2615): copy-order test with a literal expected row dump (`EXPECTED_COPY_ROWS` :1210), copy preserves exposure/partition/occurrences, record round-trip of `StaticMachineArgument` that flips byte 8 to prove the marker is checked.

## SRT/nodes/values/expression/symbol_copies.rs (1-1183)

1. Purpose: copy resolved expressions, proof facts, type-reference spans and whole callable signatures inside one `SymbolResolvedTrees`, replacing symbol identities
   per a `(from, to)` substitution list.
2. Shape: public methods on `SymbolResolvedTrees`: `copy_expression_with_symbols` (:16), `copy_proof_fact_with_symbols` (:33), `copy_type_arguments` (:53), `copy_callable_signature` (:85).
   Private `SymbolCopy { program, substitutions, type_arguments, types, callables, lifetime_owners, visited_payloads }` (:149) with `signature`, `expression`, `expression_payloads`,
   `type_reference`, `type_payload`, `callable_contract`, `receiving_application`.
3. I/O: reads and appends to `program.declarations_mut()` arenas (`data_type_parameters`, `proof_facts`, `child_type_references`, `signature_*`), the expression table (via `copy_from_self`),
   `tables.types.{constraints, operand_lifetime_owners, generic_application_origins}`, `tables.declarations.runtime_callable_contracts`, `authored_service_reach_rows`.
4. Decisions: symbol replacement is a linear `find` over the substitution slice keyed on the exact `SymbolHandle` (:343-351); an invalid source stays unchanged. `type_payload` replaces a `Named` reference
   whose symbol equals a formal binder by the supplied `TypeReference` clone (:671-678). `copy_callable_signature` seeds substitutions with the signature symbol, then every descendant symbol of that signature
   (breadth walk over `child_handles`, :92-122), then caller `selections` (minus the signature symbol itself), then `type_arguments` that are `Named` (:123-135).
5. Cases by shape: `copy_type_arguments`/`copy_callable_signature` only convert a type argument into a symbol substitution when the actual is `TypeReference::Named` (:58-67, :129-135);
   other forms are replaced only through the `type_arguments` payload route. `expression_payloads` handles each `ExpressionNode` variant explicitly (:381-537).
   Inside a `Match` destructure arm it rewrites `application_type`, `head`, `guard`, and field `field_symbol`/`Equal` values (:395-417); it does NOT visit `pattern.condition`,
   `pattern.subject_binding`, `pattern.proof_selectors`, or the match's `capture` (read; `condition` is the combined condition table_copies.rs copies at :475). For `SavedSubject` it does nothing (:429).
   `Block` visits only `tail` (comment :426: "A block's statements stay with their body").
6. Same job elsewhere: syntax-level copying with substitution is `SYN/copying/items.rs` + `bodies.rs` (`SyntaxCopy`), a parallel implementation on the syntax tables. Expression-graph copying is also
   `table_copies.rs` (same crate). typed-trees has a third copy family (`copy_*_from`, suspected).
7. Unused/one-caller: `copy_expression_with_symbols`: 0 call sites outside this file (grep `copy_callable_signature|copy_type_arguments|copy_proof_fact_with_symbols|copy_expression_with_symbols`
   excluding this file; only its unit tests call it). `copy_proof_fact_with_symbols`: 1 (`pipeline/02.../names/target_machine_families.rs:195`). `copy_type_arguments`: 3
   (`constants/receiving_applications.rs:532`, `:566`, `selections/callable_inference.rs:199`). `copy_callable_signature`: 1 (`selections/callable_inference.rs:428`).
8. Stated limits: comment at :226-228 "Runtime rows can share their formal namespace while their supplying scope changes" (service-reach row dedup via `contains`, quadratic Vec scan).
9. Fail-open: `type_reference` memoizes through `self.types.iter().find` (:588); an unmatched substitution leaves the symbol as is (:350, `map_or(source, ..)`), so a symbol the caller forgot to list is silently kept rather than refused
   (traced: callers pass lists built from child handles, but nothing checks completeness). The missed `condition`/`subject_binding`/`proof_selectors` rewrite in `Match` destructure arms means a copied pattern keeps pointing at the
   original expressions for those fields (copy_from_self at :357 already deep-copied expression handles, so `condition` is a fresh copy whose inner symbols are not substituted: wrong program would be a copied destructure
   guard-combined condition still naming the source symbols). Open (no comment; not traced to a caller that supplies a destructure pattern with a substituted symbol).
Tests (:891-1183): membership copy keeps provenance span, copy leaves originals unchanged, nested static argument rebase (note the substitution list includes `(invalid, original)` and expects it ignored).

## SRT/nodes/values/expression/table_copies.rs (1-547)

1. Purpose: copy an expression graph from another `ExpressionTable` (or the same one) into this one, keeping shared children shared within the copy.
2. Shape: `ExpressionSource<'source> { Own, Table(&ExpressionTable) }` (:23); `ExpressionTable::copy_from` (:52), `copy_from_self` (:60), `copy_graph` (:72), `copy_pattern_fields` (:404),
   `copy_transition_pattern` (:413), private `CopyGraph { expressions, subjects }` (:13, position-indexed `Vec<(old, new)>`).
3. I/O: reads node + side vectors of the source; appends rows to this table; copies spans, `unresolved_name`, `materialization_source`, selection occurrences, exposure, partition (:88-95, :330-348).
4. Decisions: memo hit is `copies.expressions.get(position)` with `original == expression` (:82-86), keyed on arena index and generation. A `SavedSubject` whose capture is not yet copied within the own table is inserted unchanged (:143-145).
   Static arguments are copied through `copy_static_argument_from[_self]` (:203-207, :272-278).
5. Cases by shape: explicit arm per `ExpressionNode` variant; `Cast` copies `target_type` and `semantic_domain_arguments` as handles (not re-copied, :186,:190); `StructLiteral.application_type` is the same handle (:307); `Block` keeps `statements`/`statement_nodes` and copies only `tail` (comment :129-130 "the copy names the same ones").
6. Same job elsewhere: `SYN/copying/bodies.rs:892` `copy_expression_handle` is the syntax-tree twin (read); symbol_copies.rs `SymbolCopy::expression` calls this and then post-patches (read). typed-trees `copy_*_from` (suspected).
7. Unused/one-caller: `ExpressionSource::copy_into`: 3 external call sites (`nodes/control_flow_and_calls.rs:1131`, `:1367`, `nodes/type_system_and_names.rs:793`; grep `\.copy_into\(` filtered). `copy_pattern_fields`: 1 (`control_flow_and_calls.rs:1188`). `copy_transition_pattern`: 1 (`:1240`).
8. Stated limits: comment :129-130 (block statements are not copied).
9. Fail-open: none seen; a `Block` copy reusing `statements` is stated (:129-130) and the copied block shares the original statement span.

## SRT/roots.rs (1-58)

1. Purpose: ordered top-level declaration lists of the resolved forest.
2. Shape: `SymbolResolvedRoots` (:8) with 12 `OrderedRootArena` fields plus `machines: MachineDeclarations`; `trait_definition` (:24), `const_declaration` (:30), `with_roots` (:36).
3. I/O: plain storage. 4. Decides: `trait_definition`/`const_declaration` are linear `iter().find` on `symbol ==` (:25-27, :31-34): keyed on symbol identity.
7. Unused: `SymbolResolvedRoots::const_declaration`: also duplicated as `SymbolResolvedTrees::const_declaration` (`tables/views.rs:222`) with the same body (read); `trait_definition` is forwarded by `views.rs:218`.
   Grep `\.with_roots\(` not run for this type (the `SymbolResolvedTrees::with_roots` is in symbol_resolved_trees.rs:109).
9. Fail-open: `with_roots` leaves `const_declarations`, `measures`, `propositions`, `conformances`, `wire_schemas`, `type_aliases` empty (:44-55); a caller that supplies roots this way gets a forest missing those declarations. Open (no comment; callers not grepped).

## SRT/selections.rs (1-481)

1. Purpose: checked rebase of authored declaration-selection occurrence ids across every store in the resolved forest when a successor forest extends a base (extension join). Also thin wrappers that record/finalize ledger rows.
2. Shape: `AuthoredSelectionExtensionFrontier` (:23, six lengths), `AuthoredSelectionOccurrenceStore` (:33), `AuthoredSelectionExtensionRebaseError` (:42), `SymbolResolvedTrees::rebase_authored_selection_extension` (:60, consumes self, returns `(Self, error)` on failure),
   `prepare_occurrence_rebase` (:116), `validate/apply_tree_statement_rebase`, `validate/apply_proof_fact_rebase`, `tree_statement_occurrences` (:426), `rebase_tree_statement_occurrences` (:445), and 8 `record_*/finalize_*` wrappers (:320-423).
3. I/O: reads/mutates expression table, tree statements (`state_statements`), statement table, transition targets, proof facts, the ledger (`tables.authored_declaration_selections`).
4. Decides: per element, "index < frontier ? retain_base : rebase_appended" (:182-186, :227-231, :279-283); `None` is refused as `OccurrenceOutsideOwnerFrontier`. All validation happens before mutation (stated :115-209, mutate at :85-112).
5. Cases by shape: tree statements: only `Statement::Call` and `Statement::Transition` carry occurrences (`tree_statement_occurrences` :426-442, two slots: target and continuation; "`_ => {}`"). Proof facts: only `ProofFact::Membership.authored_domain_selection` (:273-278).
   Statement table and transition targets are delegated to `statement_table()` methods in another file (not in unit).
6. Same job elsewhere: expression-level rebase is in values.rs:1775-1863 (same unit). Statement-table rebase is in `nodes/control_flow_and_calls.rs` (not in unit).
7. Unused: wrappers are called from stage 02 (`pipeline/02.../selections/authored_selections.rs`, count not grepped).
8. Stated limits: none.
9. Fail-open: stores not listed in the frontier (e.g. type-reference tables, static arguments inside `TypeReference::StaticSelection`, `ConstExpression` bodies) are not rebased. If an occurrence were held there it would keep its old number. Open (not traced).
   `apply_*` functions use `expect(... validated before mutation)`, so a validation hole would panic, not accept.

## SRT/symbol-table/source_inputs.rs (1-1071)

1. Purpose: the captured source map and its selection state: which files are in the current compilation, which files are rejoined to others, which package dependency aliases a requester may use, and visibility rules between a reference span and a declaration span.
2. Shape: `SourceMap { files, selection: SourceSelection, source_rejoins, product_dependencies (BTreeMap), query_dependencies }` (:26); `SourceSelection::{All, Explicit(Vec<bool>)}` (:35); `SourceCatalogue { backing: SourceMap }` (:42) with `fork`, `begin_selection`, `admit_checked_instance` (:71), `admit_source_occurrence` (:141); trait `InvocationSourceConsent` (:15).
   Key `SourceMap` methods: `validate_logical_filenames` (:208), `retain_source_rejoin` (:235), `reference_can_see_declaration` (:542), `reference_can_see_stratum` (:567), `captured_reference_can_see_stratum` (:600), `same_package` (:519), `same_checked_instance` (:526), `selection_view` (:641), `bind_anonymous_owner` (:345).
3. I/O: consumes `SourceFile` rows from `source_admission`; produces `&SourceFile` handles; query routes read `product_dependencies`/`query_dependencies`.
4. Decides (all on identity fields, not display names): exact-occurrence dedup keyed on (path, source text, package_root, package_identity, origin, stratum, dependency_scope) (:93-101, :319-327); "same package" = equal `PackageKeyIdentity` or, for unowned files, equal origin and package_root (:666-672);
   "same checked instance" additionally requires equal `dependency_scope` (:526-540). Stratum visibility: `Base` always visible; `CurrentActivationExtension` only to a reference in that stratum; `DependencyActivationExtension` visible unless a Base reference in the same package (:588-597).
   Logical filename uniqueness keyed on a tuple (package identity, (root, origin) if unowned, scope, name) in a `HashSet` (:209-224). `query_dependencies` applies only to `Origin::User` files whose root equals the recorded root (:432-433).
5. Cases by shape: `SourceCatalogue::admit_checked_instance` treats three shapes: current-activation-extension or unowned User source (always added new, :114-125), other (dedup/select existing, :127-136). Toolchain-owned `<build-prelude>` files are skipped in filename checks by `origin.is_toolchain() && path == package_root` (:213).
6. Same job elsewhere: package comparison `same_package_files` (:666) is re-stated in `SymbolTable::same_source_package` (table.rs:2177) and `Self::same_checked_instance` (:526) uses a near-identical match arm; `ImportedProductMember::matches` (table.rs:2612) re-checks product scope.
   Uses `BTreeMap` for `product_dependencies` keyed on `PackageKeyIdentity` (a non-dense-id key; `tools/ratchets/dense_id_maps.txt` coverage not checked).
7. Unused/one-caller (greps in `source-rust/`, excluding this file): `SourceMap::selection_view`: 0 call sites (grep `selection_view\(` in `subsystems`, only the definition). `SourceCatalogue::fork`: no obvious caller (grep `\.fork\(\)`: only `build/inputs/source/assembly/mod.rs:127`, a different type, suspected). `begin_selection`: 7 sites (build/inputs).
   `admit_checked_instance`: 4 sites (`frontier.rs:654`, `generated.rs:97`, `generated_tests.rs:41`, plus one more in build). `admit_source_occurrence`: 2 non-test (`frontier.rs:374`) + tests in `bindings.rs`. `bind_anonymous_owner`: 2 non-test (`anonymous_owner.rs:65`, `frontier.rs:346`).
   `retain_query_dependencies`: 1 (`product_queries.rs:465`). `same_product_dependency_scope`: 1 (`driver/resolve_and_type/activation.rs:65`). `validate_logical_filenames`: 1 (`pipeline/02.../resolver.rs:96`). `InvocationSourceConsent`: implemented by `AnonymousSourceCapture` (`build/inputs/source/anonymous_owner.rs:71`) and test captures; consumed in driver and pipeline 07 (about 20 hits).
8. Stated limits: none.
9. Fail-open: (a) `reference_can_see_declaration` returns `true` when `reference.span.start == reference.span.end` (source-free reference, :554-556) and when either `file_at` is `None` (:557-562). Stated by the test `source_free_reference_remains_permissive_with_an_extension_declaration` (:841-860) and `table.rs:2170-2176`.
   Traced: a source-free span means generated/focused-test references; a real authored reference always has nonzero span. (b) `SourceSelection::includes` on `Explicit` returns `false` for an out-of-range id (:190), fail-closed. (c) `rejoined_source_id` returns `None` on mismatch (fail-closed). (d) `text_at` returns `""` for an unknown id (:659-663), stated by a test (:833-839).
Tests (:674-1071): logical filename collision, query alias scope, strata visibility, package-identity reconciliation, identity framing of toolchain sources (a literal sha256 hex at :975).

## SRT/symbol-table/source_symbols.rs (1-90)

1. Purpose: module root for the symbol table, plus three custody gates for bundled declarations and a Build-prelude predicate. 2. Shape: re-exports (:8-19); `sealed_intrinsic_at` (:27), `core_vocabulary_at` (:42), `core_source_at` (:56), `toolchain_site` (:63), `is_build_protocol` (:79).
4. Decides: `toolchain_site` returns `None` unless `file.origin.is_toolchain()` and `file.package_identity.is_none()`, and the path strips `package_root` (:67-73); the catalogue then matches package-relative path, declaration path and exact source bytes (comment :20-26). `is_build_protocol` compares `symbols.name(declaration) != name` (string) and requires origin `Toolchain` and path exactly `<build-prelude>` (:79-89): keyed on source spelling of the name plus file path string.
7. `core_source_at`: 1 external call (table.rs:1915). `is_build_protocol`: 6 call sites (`pipeline/03.../call_and_product_selections.rs:48, 196, 3621`; `pipeline/05.../read/calls.rs:1365`; `selection_questions.rs:690`; `pipeline/07.../build_companions.rs:106`; grep `is_build_protocol\(`).
9. Fail-open: none seen (all gates return `None`/`false` when the check cannot be made).

## SRT/symbol-table/source_symbols/builtin.rs (1-169)

1. Purpose: list the compiler-installed root symbols (builtin types, builtin functions, fixed `core::*` declarations, `u64::Bound` member) in the exact order their root slots take. 2. Shape: `builtin_type_symbols` (:8), `builtin_function_symbols` (:12), `fixed_core_declaration_symbols` (:17), `builtin_type_member_symbols` (:26), `is_builtin_type_spelling` (:5).
4. Decides: `builtin_type_member_symbols` yields a member only when `builtin_type == BuiltinTypeMember::AddressBound.owner()` (:29): one hardcoded member. `is_builtin_type_spelling` forwards to `BuiltinTypeAtom` (string test, covers an open-ended `u<N>`/`i<N>` family, test :159-168).
5. Cases by shape: exactly one builtin member (`Bound`) and it is hardcoded. Tests (:36-169) pin ordinals: `FloatMaximum` 0, `AsmHlt` 3, `ContentOld` 33, `IntegerEmbed` 71, `AsmWriteTpidrEl1` 99, `AsmAarch64FullFence` 102 (:88-102); test asserts the retired `entry` builtin spelling is absent (:112-117).
9. None seen.

## SRT/symbol-table/source_symbols/retained_declaration.rs (1-250)

1. Purpose: transfer a selected declaration between independently allocated source contexts as an inert anchor, and rejoin it by exact source occurrence. Also a span-sorted candidate index. 2. Shape: `SourceSpanCandidates::at_span` (:35), `retain_declaration` (:67), `rejoin_declaration` (:93).
3. I/O: `retain_declaration` reads a symbol's span and source file, returns `source_admission::RetainedDeclaration`. `rejoin_declaration` takes candidate symbols and returns one handle or `&'static str` error.
4. Decides: candidate matches iff kind equals, span equals, and `source.continues_capture_of(declaration.source())` (:100-114); more than one distinct match is refused as ambiguous (:115-117); zero is refused (:120-122). `SourceSpanCandidates` rebuilds when the table address, node/name revision or length changes (key at :38-44): keyed on identity of the table (pointer), arena revision and length.
7. `SourceSpanCandidates::at_span`: 4 external call sites (`driver/compile_build/waits/demand.rs:756`, `driver/compile_product/provider_declarations/retained.rs:189, 231`, plus construction at `demand.rs:750`, `retained.rs:77, 182`); `retain_declaration`/`rejoin_declaration` called by the driver (not counted).
8. Comment "GUARDED" at :19-22.
9. Fail-open: the cache key uses `std::ptr::from_ref(symbols) as usize`; a different table at the same address with equal revision and lengths would reuse stale rows (traced: key also includes arena revision and lengths, so only an exact-length twin allocated at the same address; open as not traced to callers).

## SRT/symbol-table/source_symbols/storage.rs (1-336)

1. Purpose: paged, shareable symbol row storage with copy-on-write of one 128-row page. 2. Shape: `SymbolRows<T>` (:12, `RetainedArena<T>` plus `pending`), `PAGE_ROWS = 128` (:9), `HierarchyStorage` impl (:107), `SymbolHierarchy` alias (:154).
4. Decides: on decode, refuses a page over 128 rows and non-dense/multi-generation rows (:92-97); `contains_span` validates a child range by its two endpoints only, relying on "Symbol rows are never retired" (:124-139).
8. Stated limit: the endpoint-only validation (:124-126). 9. Fail-open: `contains_span` returning true when first and last child are valid but a middle row is not; stated safe because decode checks density (:95-97) and rows are never retired (comment :124-126): stated.
Tests (:156-336): fork shares pages, copy cost is one page, codec refuses stale children/cycles.

## SRT/symbol-table/source_symbols/table.rs (1-4512)

1. Purpose: the symbol table itself: a hierarchy arena plus the whole name-lookup system (lexical, top-level, module-qualified, import-exposed, domain-qualified), generated-extension checks, builtin slot classification, and provenance/package queries. Builder, extension, and iterator types live at the end.
2. Shape: nested modules `child_batches` (:26), `indices` (:359), `modules` (:528), `source_queries` (:1818), `tests` (:2341). Main types: `SymbolTable` (:2463), `SymbolTableBuilder` (:2508), `SymbolTableExtension` (:4440),
   `SourceScopedTopLevelBinding` (:2524, targets `Declaration | Module | FileAlias | ProductFileAlias`, producers `Loader | AuthoredImport` :2531-2543), `SymbolLookup { NotFound, Unique, Ambiguous }` (:2654), `UnresolvedName` (:2666), `ImportedProductMember` (:2606), trait `SymbolTableAppender` (:2808).
   Entry points: `lookup_top_level_candidates` (:3700), `find_module_qualified_candidates` (:1483), `select_namespace_candidates` (:1348), `domain_name_reaches` (:969), `source_reference_can_see_symbol` (:3921), `symbol_source_is_selected` (:3951), `is_generated_extension_of` (:2844), `begin_source_activation` (:3110), `find_declaration_from_source` (:2066), builtin slot classifiers (:4029-4184).
3. I/O: reads `SourceMap` (`Arc`), bindings, `bundled_declarations` catalog results via `source_symbols.rs`. Caches (four `OnceLock` index caches) are ignored by equality and cleared by every mutation (:436-476).
4. Decides (the key judgments):
   - Visibility: `source_reference_can_see_symbol` requires `inherited_symbol_is_selected` and `SourceMap::reference_can_see_declaration` (:3921-3944); modules are visible if any contributing source is visible.
   - Top-level lookup precedence (:3700-3834): source-backed reference first filtered by selection; `|alias|::name` file-alias path (:3717-3743); module-qualified roster (:3744); candidates by name/leaf filtered on kind, caller predicate and visibility (:3757-3768);
     `binds_declaration(name)` loader binding wins (:3777-3789); namespace context -> `select_namespace_candidates` (:3791-3793); otherwise fall to a preference chain (same source, then same checked instance and unseparated stratum, then unseparated stratum, then `candidates.first()`) (:3805-3833).
   - `select_namespace_candidates` (:1348-1432): local-in-module declarations beat imports except domains; imports match by display path equal to the logical import path or, for unmoduled exact-source imports, by name; then ambient toolchain symbols.
   - Identity vs spelling: builtin types/functions/fixed-core declarations are classified by exact root slot ordinal, "Names are deliberately irrelevant" (:4036-4041, `root_slot_ordinal` :4281). Integer carriers `uN`/`iN` are recognized by decimal spelling of a root-parented, non-generated `BuiltinType` symbol (:4142-4159) (a spelling-based decision, with the guard that `roster_ordinal` is `None`).
   - `domain_name_reaches` (:969-1042) mixes display-path string equality, `same_semantic_name` (exact or leaf equality, :1776-1780), and `rsplit_once("::")` of the authored spelling.
   - Generated-extension proof: `is_generated_extension_of` compares prefix rows by value, and each appended symbol must have a valid earlier origin, no authored span, a unique place among parent's children (:2844-2945).
5. Cases by shape: `find_child_by_name` / `find_child_by_name_and_kind` refuse (return `None`) more than one same-named child only when the parent is a `Module` (:3378, :3400); for non-module parents the first match wins. `find_top_level_declaration_or_operator_family_from_source` returns a family representative only when `Operator` kind and more than one candidate (:3860-3870).
   `builtin_type_member` hardcodes the single member `AddressBound` (:4109-4113). `find_declaration_from_source` treats only "same Operator kind or token-bound Machine kind with identical display path, package identity and source package" as a family (:2132-2143).
6. Same job elsewhere: `same_source_package`/`same_checked_instance` re-stated in source_inputs.rs (see above). `lookup_top_level_candidates` unqualified fallthrough re-implements the "first of same source" preference also seen in `find_declaration_from_source` (:2123-2145).
   Two builder paths (`SymbolTableBuilder::insert_children` :2741 and `SymbolTableExtension::insert_children` :4462) plus `SymbolTable::append_children_from` (:2989) create child rows with near-identical `Symbol { parent, declaration_owner: invalid, children: empty, kind, name, generated_from }` literals (also :1684-1692, :3189, :3241).
7. Unused/one-caller (grep; excludes `symbol-table/`): `SymbolTableAppender`: 27 occurrences, 22 in `pipeline/02.../names/symbol_table.rs`, 1 in `declarations/declaration_selection.rs`, plus here (grep count). `find_descendant_by_path`: 2 external (`pipeline/02.../names/targets.rs:841`, `pipeline/03.../equality_and_exhaustiveness.rs:2799`); `resolve_child_path`/`path_from_members`/`path_member_arena`: 0 external call sites (grep pattern filtered `!**/symbol-table/**`: no hits, only the table's own tests).
   `is_generated_extension_of`: 1 production call (`continuations.rs:38`) plus tests. `find_top_level_declaration_or_operator_family_from_source`, `lookup_signature_free_top_level_from_source_matching`, `source_import_exposes_domain`, `carrier_qualified_domain_reaches`, `domain_name_reaches`, `install_integer_families`, `universe_sort_for_symbol`, `rejoin_product_occurrence` together: 53 occurrences in 28 files outside `symbol-table/` (grep count; per-name counts not taken).
   `SymbolTable::new` and `SymbolTable::{rename}`: not grepped. `JoinedPath` (:1734) claims to be "shared by the path member rosters": users not grepped.
8. Stated limits: "Source-free lowering is used by focused representation tests; those trees model one package and retain the historical all-local behavior." (:2174-2175). "Source-free focused trees ... explicit fallback" (:2235-2238). Comment at :3496-3501: "neither may silently select its first declaration" for synthetic references with signature-free selection (yet `selection == Ordinary` with a non-source-backed reference does `take(1)` at :3774).
9. Fail-open risks:
   - `same_source_package` returns `true` when `self.sources` is `None` (`is_none_or`, :2177-2181): stated (:2174-2176). Wrong program: a source-free table treats all declarations as one package. Traced: only focused tests build tables without sources? (not traced to every constructor; `SymbolTableBuilder::new` and `Default` produce `None`).
   - `same_product_package_instance` returns `true` without sources (:2195-2197): same stated reason (open for production callers).
   - `same_symbol_source_package` returns `true` without sources (:2212-2214) but `false` when either symbol lacks provenance (:2215-2220): asymmetric.
   - `source_reference_can_see_symbol` returns `true` when `sources` is `None` (:3926-3928) or the symbol has no provenance span (:3933-3935); `symbol_source_is_selected` likewise (:3952-3954, :3962-3964); `inherited_symbol_is_selected` returns `true` beyond the vector (:3972, `unwrap_or(true)`). Stated by `source_reference_can_see_symbol` doc (:3917-3919: "Source-free consumers retain their existing behavior").
   - `symbol_is_in_dependency_scope` returns `true` when provenance is missing (`is_none_or`, :1838-1840): stated (:1829-1830 "Missing provenance retains the existing unscoped declaration behavior").
   - `lookup_top_level_candidates` for a reference without a source span and `selection == Ordinary` picks the first candidate (`take(1)`, :3774); last arm `candidates.first()` (:3831) picks the first candidate in declaration order when no source-position test matches. Open (no comment at the site that says why; behavior reached by references with source ids that match nothing).
   - `find_declaration_from_source` `(None, None)` arm returns authorized when `!self.has_source_metadata()` (:2115). Stated implicitly (source-free).
   - `is_ambient_toolchain_symbol` treats a symbol with no provenance span as ambient (`None => ...is_none()`, :2229) so any source-free symbol is visible to every lookup by name (:1426-1430). Comment :2224-2225 says "source-free builtin symbols are ambient". Stated.
   - `validate_source_module_import` returns `Ok` for an exact-source import whose declaration source declares no module (`!module.is_valid() { continue }`, :774-777): accepts without checking that the path names anything. Open (comment :739-740 says "Exact-file inputs retain their existing file admission").
   - `source_binding_is_selected` with `sources == None` returns true for every non-logical import (:931-934): source-free tables accept imports.
   Candidate-truncation: `logical_import_targets` returns an empty list when more than one candidate exists and any is not `Machine`/`Operator` (:902-911): refuses (fail-closed).
Tests (inline modules `child_batches::tests` :188-357, `tests` :2341-2455) and the separate tests.rs below.

## SRT/symbol-table/source_symbols/tests.rs (1-2721)

Test-only file (included from `source_symbols.rs:6-7`). Modules: `generated_extension` (:6-119) pins `is_generated_extension_of` accept/reject cases (renamed row, appended authored child, existing child range changed, `Unknown` kind, rebound loader binding);
`hierarchy` (:120-521; appended lexical children order, extension, generated files importing other generated files, declaration ownership isolation);
`module_namespaces` (:522-1734; the largest: logical imports, overload retention, domain exposure by exact qualifier or exposing import, carrier-qualified domains, unmoduled exact imports and checked-instance independence of order, build alias vs product instance, warmed index cache vs rename);
`package_scope_separation` (:1735-1828); `paths` (:1829-1878); `provenance` (:1879-2254; generated children keep origins; product identity excludes Build instance; Build subject reaches public toolchain declaration but ordinary product selection does not, :2178-2218; family selection needs token membership);
`source_scoped_bindings` (:2255-2402; build-prelude custody paths including `<build-prelude>/Build/mod.omg` true and `nested/mod.omg` false, :2265-2297); fixtures (:2404-2479); `closed_contract_scope` (:2481-2635); `integer_types` (:2637-2721; `u13` installed once, declared `u7` is not the carrier).
Tests assert behavior by exact symbol handles; module-qualified lookup tests build tables with `SourceMap`. Noted: tests use `SourceMap::from_files`, `add_with_metadata`, `add_checked_instance` which are the only production-adjacent constructors for these paths.

## SRT/symbol-table/source_tables.rs (1-408) and source_tables_tests.rs (1-163)

1. Purpose: interning tables for external bindings, semantic domains, service reach rows and service reach definitions. 2. Shape: `ExternalBindingTable` (:12, linear `position` dedup), `SemanticDomainTable` (:44, seeded with `Wrapping`, `Saturating`, `Trapping` ids 1-3, :48-58, constants :70-72), `ServiceReachRowTable` (:105; rows `Concrete(Vec)` or `AbstractBounded(row id)`), `ServiceReachTable<A>` (:202; `definitions` plus `derived` with high-bit ids `1<<31`, :253).
4. Decides: interning by structural equality via linear scan (`intern`: :18-24, :75, :130-136, :145-149, :229-233): O(n) per intern (the dense-id rule appears to be kept for ids but lookup is a scan). `ServiceReachRowTable::services` on an unknown id returns `&[]` (:155-163), and on `AbstractBounded` returns the bound's services. `set_canonical_id` panics on a missing id (`expect`, :281). `extend_closure` follows parents recursively (:350-361) with a `contains` guard.
5. Cases by shape: `SemanticDomainTable::default` hardcodes three domain names (:51-54). `ServiceReachRowTable::intern` creates the empty row on first call so `EMPTY_ROW` is id 1 (:127-129).
7. `try_extend_arguments`: 1 call (`pipeline/03.../lowerer.rs:1995`). `id_for_name`: 1 non-test call (`pipeline/02.../selections/service_reaches.rs:745`), `identities()` and `SemanticDomainTable::lookup` not separately counted (grep alternative matched 84 occurrences in 42 files, includes unrelated names).
9. Fail-open: `services()` returning `&[]` for an unknown row id (:162) means a stale/unknown row id reads as "no services" (open; empty reach is the most permissive reach set for a reader that treats fewer services as less authority, but if used as "reaches nothing needed", it passes). `set_parents` on a missing id does nothing (:344-347). Open (not traced).
Tests (`source_tables_tests.rs`, `retained_side_record_tests` :367-408): structural identity of bindings, empty row id equals `EMPTY_ROW`, abstract rows intern per bound, domain table idempotence, successors share row payloads by pointer.

## SRT/symbol-table/symbol_table.rs (1-7)

Crate root: `#![forbid(unsafe_code)]`, declares `source_inputs`, `source_symbols`, `source_tables` (:5-7). Nothing else.

## SRT/symbol_resolved_trees.rs (1-194)

1. Purpose: crate root. 2. Shape: `SymbolResolvedTrees` (:69) with `roots`, `tables`, `symbols`, `service_reaches`, `service_reach_rows`, `authored_service_reach_rows`, `semantic_domains`, `external_bindings`, `evidence_forwardings`, `generic_binding_overlaps`, `pending_token_shape_comparisons`, `resolution_indexes`; `SymbolResolvedTableStorage` (:101; `authored_declaration_selections` is private); `GenericBindingOverlap` (:87), `PendingTokenShapeComparison` (:95); `with_roots` (:109). `Deref<Target = SymbolResolvedRoots>` (:131).
   `pub use crate as source_resolved` (:11) is the import name consumers use.
3. I/O: aggregates the other files. 4. Decides: nothing.
7. `source_resolved` alias: consumers import through it (count not taken). Comment at :85 "Typing decides whether this visible generic's bound denotes a value or a type"; :93 "Dependent operand shapes must be compared after their types are lowered": both are pending-work lists carried to typing (stated limits, :85-98).
9. None seen.

## SRT/tables/lookup_indexes.rs (1-218)

1. Purpose: cached lookup indexes tied to backing storage revision. 2. Shape: `GenericOriginIndex` (:12), `PrefixPositions` / `PrefixPositionMaps` (:22-33), `ResolvedIndexes` (:70, `OwnedIndex<...>` plus two `Mutex<PrefixPositionMaps>`), `data_definition_position` (:137), `local_statement_position` (:157), `parameter_position` (:176), `SymbolResolvedTrees::with_generic_origin_index` (:194).
4. Decides: freshness of a prefix map is judged by slice start pointer `rows.as_ptr() as usize`, a length watermark, and three sampled rows (first, middle, last) compared by `statement_sample` (hash of the discriminant and local symbol index, :109-117) or the parameter symbol index (:187). Revision for generic origin index = sum with `wrapping_add` of three arena revisions (:199-205).
   Uses `HandleKeyMap` aliased as `HashMap` (:9) and `OwnedIndex`.
6. Suspected-same-job: other caches in `table.rs` `index_cache!` (table.rs:436-476) use `OnceLock` plus manual clear instead of revision keys.
9. Fail-open: the freshness check is a sample (3 rows), so a slice at the same address with the same watermark that has had rows in the middle replaced but the three sampled rows unchanged would serve stale positions. Open (no comment; mutation patterns not traced). `ResolvedIndexes` `PartialEq` always returns `true` (:101-105) and serialization writes nothing (:211-218): stated by design (indexes are derived).
   Panics: `samples` closure indexes `rows[0]` when `entry.watermark` is nonzero but `rows` could be empty only if `entry.watermark <= length` fails; guarded.

## SRT/tables/mod.rs (1-458)

1. Purpose: publish normalized body/type tables from the resolver's declaration storage, either from scratch (`rebuild_tables`, :411) or for completed machines (`append_completed_body_tables`, :419).
2. Shape: `SymbolResolvedTreeTables { bodies, types }` (:20); `Transplant` (:37); `append_declaration_tables` (:84-253); `normalize_statements` (:255); `insert_*` helpers.
4. Decides: it walks only the roots after a `frontier` (`skip(frontier.X.len())`, :167-252) for data, domains, propositions, traits, operators, measures; machines are visited when ordinal >= frontier or present in `completed_machines` (binary search on `(arena_index, generation)`, :215-230). Block-valued expressions publish `statement_nodes` when `statement_nodes.is_empty() && !statements.is_empty()` (:142-165).
5. Cases by shape: handles only these root kinds: assembly blocks, block values, data definitions, domains (target type and operators), propositions (const binders, parameters, witness evidence), trait machine signatures, machines (type equations, owned data, states), operators, measures. Conformances, constants, wire schemas, type aliases, mathematical definitions are not walked (:167-252, matching `SymbolResolvedRoots` fields at roots.rs:8-21). Read.
7. `rebuild_tables`: 3 non-test callers (`pipeline/02.../resolver.rs:250`, `selections/authored_selections.rs:2210`, `:2414`; grep `rebuild_tables\(`). `append_completed_body_tables`: 1 (`resolver.rs:249`).
8. Stated limits: none. 9. Fail-open: the roots listed in item 5 are not published into `types.references`; if typing reads type references for conformance or alias roots it finds none (open; consumer not traced). `from_body_completion` leaves previously published tables in place via `mem::take` of `statements` and `references` and starts a fresh `expressions` (:66-74): an assumption that earlier expressions need no republish.
Test (:435-458): publishes one machine and checks counts.

## SRT/tables/storage.rs (1-55)

Three storage structs: `SymbolResolvedDeclarationStorage` (:9, 33 `Arena` fields), `SymbolResolvedTypeStorage` (:44), `SymbolResolvedBodyStorage` (:52). Pure data; `proof_fact_source_spans: Vec<Option<SourceSpan>>` (:17) is a side vector indexed by proof fact handle.

## SRT/tables/views.rs (1-316)

1. Purpose: span-to-slice accessors over declaration storage on `SymbolResolvedTrees`. 2. Shape: `expression_table` (:23), `declarations` (:54), `data_members`... ~40 accessors, `machine_parameter_contract_view` (:166), `missing_type_names` (:29), `const_declaration_at_source_span` (:229).
4. Decides: `domain_definition_by_symbol`/`data_definition_by_symbol`/`const_declaration` are linear `iter().find` on symbol equality (:62-72, :222-227); `machine_parameter_contract_view` returns `None` for `Inferred`, `RequirementIdentity` and `AuthoredNominal` (:171-176) and for a `Nominal` whose trait or requirement is not found (`?`, :182-186).
   `const_declaration_at_source_span` compares `symbol_source_span == Some(span)` (:233).
6. `SymbolResolvedTrees::const_declaration` (:222) duplicates `SymbolResolvedRoots::const_declaration` (roots.rs:30), and `trait_definition` (:218) forwards to `roots.rs:24`.
7. `data_definition_by_symbol`: `ResolvedIndexes::data_definition_position` (lookup_indexes.rs:137) is the indexed twin; callers of the linear version not grepped.
9. `set_proof_fact_source_span` resizes the side vector with `None` and overwrites (:120-130): fail-open for out-of-order writes only in that it silently grows. None else seen. `proof_fact_source_span` returns `None` for a missing slot (:112-118).

## SRT/tests/authored_declaration_selection_ledger.rs (1-70) and SRT/tests/resolved_records/mod.rs (1-462)

Test-only. Ledger canary: ordinals 0 and 1, invalid symbol is refused with `InvalidSelectedSymbol` and leaves rows unchanged (:41-51), `rebuild_tables` keeps the ledger (:62-69). `resolved_records/mod.rs` (included from `symbol_resolved_trees.rs:193-194` as `mod tests`): intrinsic finalization state checks (:1-58), independent arenas for ranking views and signature invokes (:100-123), and five extension-rebase tests: shifting every extension-owned store (:178-308), failure returns the original trees intact (:310-341), late failures in `Statements`/`TransitionTargets`/`ProofFacts` leave earlier stores untouched (:343-427), appended clones of base occurrences keep their base ids (:429-462). They define the contract that `rebase_authored_selection_extension` is atomic.

## SYN/copying/bodies.rs (1-1216)

1. Purpose: `SyntaxCopy` methods that copy statements, states, signatures, type references, expressions and match structures from one `SyntaxTrees` into another (or same), substituting types, expressions, lifetimes, array lengths and lexically scoped supplied arguments.
2. Shape: macro `copied_state_signature` (:24-67); `impl SyntaxCopy`: `static_argument_for_actual` (:72), `remap_static_machine_argument` (:132), `copy_state_handle_span` (:170), `copy_state_signature` (:211), `copy_statement_node` (:302), `copy_type_reference_handle` (:543), `copy_capture` (:804), `copy_destructure_pattern` (:866), `copy_expression_handle` (:892), `copy_mapped_span` (:1192), `copy_span` (:1209).
4. Decides: memo hit keyed on `expression_copies[arena_index]` with `(source, copied)` equality (:909-914). Substitution priority for a type handle: explicit `type_substitutions` (by handle) then `scoped_arguments` match by `binder == name.as_str(other)` for a `Named` reference (:552-566): keyed on SPELLING of a type name. Array-length const parameter substitution compares `span == name.source_span() && spelling == name.as_str(other)` (:628-629) or binder string (:634). When the replaced actual has no alias origins and no lexical match it is reused directly (:572-575), otherwise a fresh occurrence is made.
   `remap_static_machine_argument`: a bare single-name path is substituted when its spelling equals a scoped binder (:137-150).
5. Cases by shape: `static_argument_for_actual` handles `Named`, `Generic`, `StaticSelection`, `ConstExpression` (only `Integer` literal or `Name` expression), and everything else becomes an opaque `type_reference` (:77-119, `_ => reference()`). `FixedArrayLength::ConstParameter` substitution: parses the actual `Named` text as a decimal literal via `value.as_str(..).parse()`, else keeps an `Expression` (:633-641): a string parse of a type name.
6. Same job elsewhere: `SRT/.../table_copies.rs` copy_graph (read), symbol_copies.rs (read). The `Block` copy re-inserts statements through `block_statements` (:949-968); the resolved copy shares statements (table_copies.rs:129-137).
7. Unused: none grepped individually; callers are `copying/items.rs`.
8. Stated limits: comment `A declaration operand keeps its name and telescope...` (:70-71).
9. Fail-open: `copy_type_reference_handle` returns an invalid handle for an invalid input (:548-550). The `lexical` substitution only fires for `Named` references in a non-empty `scoped_arguments` (:556-564); a binder spelled the same as an unrelated local type name inside the copied scope would be substituted (that is by design, `with_binders` removes shadowed binders, items.rs:675-709). `ExpressionNode::Name` copy remaps only `application`, not `members` (`copy_expression_identifier_span`, :1044-1057): a name path spelling is never substituted. Open (no comment).
   `copy_capture` uses a memo keyed by arena index (:813-817); `copy_destructure_pattern` calls `copy_expression_handle` for `subject_binding` after other fields (:882): order not a risk.

## SYN/copying/items.rs (1-1007)

1. Purpose: copy items (machines, data, domains, constants, conformances, traits, propositions, measures, aliases, capabilities, use/package) between `SyntaxTrees`, including the public entry points for the driver and resolver.
2. Shape: `SuppliedSyntaxArgument { binder: String, ty }` (:23); `SyntaxCopy` (:28) with seven substitution tables; `SyntaxTrees::copy_item_from` (:90), `copy_expression_from` (:94), `copy_statements_from` (:111), `copy_proof_fact_from` (:126), `copy_state_signature_from` (:134), `copy_type_arguments_with_bindings` (:142), `copy_state_signature_with_arguments` (:156), `instantiate_type_reference_from` (:185), `copy_type_reference_handle` (:219); `copy_item` (:335), `with_binders` (:675), `copy_machine_fields` (:732).
4. Decides: `with_binders` drops any scoped argument or lifetime whose binder SPELLING (`p.name.as_str(other) == argument.binder`) is redeclared by the inner parameters (:690-703): spelling-keyed shadowing. `copied_lifetime` matches a lifetime substitution by `(source span, spelling)` then by spelling alone (:234-245). Const preparation copying (:412-459): a pending/qualified obligation is retargeted only if its `declaration`, `destination` and `initializer` still equal the source constant's (`obligation.declaration == constant.name.source_span() && destination == constant.type_reference && initializer == constant.value()`, :436-439, :451-452), else it is kept unchanged (`obligation.clone()`, :446) or dropped for `qualification` (`.and_then(... then_some)`, :450-457).
5. Cases by shape: `copy_item` matches all 13 `Item` variants (:336-552). Conformance members copy via a manual span-building loop (:372-387) rather than `copy_mapped_span`. Lifetime arguments of generic conformance bounds are substituted only for `Machine` (:771) and state signatures (bodies.rs macro :37-41); data, alias, trait and domain bounds `.clone()` them (:477, :587, :818).
6. Same job elsewhere: `copy_mapped_span`/`copy_span` (bodies.rs:1192-1215) vs the hand-written loop (:372-387); SRT copies (symbol_copies.rs, table_copies.rs).
7. Unused/one-caller (grep): `copy_type_arguments_with_bindings`: 1 external (`pipeline/02.../declarations/trait_defaults.rs:506`). Others not counted; `copy_expression_handles_to_statement_table` and `expression_identifier_path` are in statement_paths.rs.
9. Fail-open: obligation not retargeted is kept as-is (:446): a copied constant keeps a destination obligation pointing at the original constant's handles; open (not traced). `satisfies` clause copy moves `trait_name`, `requirement`, `alias`, `via` fields without remap (:750-761): by design they are spellings.

## SYN/copying/statement_paths.rs (1-44)

`SyntaxTrees::copy_expression_handles_to_statement_table` (:4) appends each argument handle to the statement table's handle arena and returns a span; `expression_identifier_path` (:18) returns the identifier path of a `Name` without application or a `Member` chain, else `None`, and an invalid handle gives `Some(Vec::new())` (:23-25). Callers (grep): `copy_expression_handles_to_statement_table` 1 (`pipeline/01.../transitions/targets.rs:89`); `expression_identifier_path` 6 (`driver/compile_build/waits/mod.rs:366`, `targets.rs:82`, `pipeline/02.../lowering/statement.rs:183`, `package-manager/.../dependency_rows.rs:275`, `direct_build_call.rs:23`, self-recursion `:37`). Judgments: identifier SPELLING path (callers compare with `owner.as_str(syntax) == BUILD_TYPE` etc.).
Fail-open: an invalid receiver returns `Some(empty)`, which callers using `matches!(.., Some([owner]))` treat as no match: safe.

## SYN/declarations/{capabilities,conformances,constant_preparation,constants,data,domains,items,machines,parameters,proofs,type_aliases}.rs

Group: parser-output node types in `syntax-trees`; the first nine are row/struct definitions with derives `SyntaxEquivalent`, `SyntaxRelocate`, `Record` and no logic. Line ranges read: capabilities 1-122, conformances 1-56, constant_preparation 1-93, constants 1-71, data 1-115, domains 1-60, items 1-655, machines 1-149, parameters 1-78, proofs 1-198, type_aliases 1-25.
- capabilities.rs: `OperatorDefinition` (:11), `CapabilityDefinition`, `CapabilityMember`, `CapabilityContract` with `CapabilityContractKind { Ensures, EnsuresForResultCase, Requires, Crashes{cause} }` (:104), `CrashCause { Trap, Abort }` (:119).
- conformances.rs: `ConformanceItem` (:11), `ConformanceMember { Machine, TraitDefault, Synthesized, Selection }` (:29-50); comment (:37-44) cites wiki spec `conformances.md` "Implementations are operation records".
- constant_preparation.rs: `ConstPreparation` (:13), `ConstQualification`, `ConstDestinationObligation`, `ConstInitializerCustody`, `ConstInitializerNormalization` (:85). The first three derive `PlainSyntaxEquivalent` so cross-store equality compares `ExpressionHandle`/`TypeReferenceHandle` values, not structure (:31-58).
- constants.rs: `ConstSyntax`, `ConstDefinition` (:31) with `value()` (:56) = `materialized_initializer.unwrap_or(initializer)`, `preparation()` returns a static `EMPTY` when `None` (:62-66).
- data.rs: `DataDefinition` (:10), `QuotientDefinition`, `DataMember { Field, Variant, Retired(u64) }` (:74), `DataVariant.retired_payload_identities` (:114).
- domains.rs: `DomainDefinition` (:10), `DomainAliasDefinition`. 
- items.rs: `Item` enum (13 variants, :35-49), `ExternalBinding` (:81; default `Syscall{number: 0}`), `UseItem { Namespace, File }` (:108), `ItemTable` via `paged_roster!` (:142), `StateStorage` (:151), `DeclarationStorage` (:163), accessors and `append_*` methods (:189-583). `StatementBody`, `statement_bodies`, `set_statement_body` (:410-438), `detach_statement_handles_since` (:482).
  `set_statement_body` for `TraitDefault` uses `.expect("selected trait default")` (:435). Defaults at :89-99 (`ExternalBinding::default` = syscall 0; `Item::default` = empty namespace `Use`).
- machines.rs: `Machine` (:11, 30 fields), `State`, `TraitDefinition`, `TraitRefinementClause`, `MachineBody`, `StateSignature` (:124); comment :41-46 and :144-148 on `crashes none` meaning ("the empty value" in `wiki/spec/language/effects.md`).
- parameters.rs: `TypeParameter`, `TypeParameterKind { Type, Implementation, Const, Value, Machine{contract: Option<..>}, Proposition{..} }` (:25), `MachineParameterContract { Inferred, RequirementIdentity, Structural, Nominal }` (:56); comment :57 "An authored `const F: machine`; exact typed uses must supply its type."
- proofs.rs: `PropositionDefinition`, `PropositionBody`, `MathematicalDefinition`, `MathematicalTypeNode`, `MeasureDefinition`, `ProofFact { Expression, Membership }` (:139), `ProofMembershipFact` (:160; derives `Record` and `SyntaxRelocate` only; `ProofFact` derives `PlainSyntaxEquivalent`, so equality is `==` on handles), `SatisfiesClause`, `GenericConformanceBound`.
- type_aliases.rs: `TypeAliasDefinition` (:13) with `asserted_multiplicity`.
Cases by shape: `TypeParameterKind::Machine { contract: Option<..> }` where `None` is a distinct state from `Inferred`; the resolved side (SRT) turns this into `MachineParameterContract::{Inferred, RequirementIdentity, Structural, AuthoredNominal, Nominal}` (read in `symbol_copies.rs:264-289`, `views.rs:166-192`).
Same job elsewhere: `ConstPreparation` copy logic is in `copying/items.rs:412-459`. `ConformanceMember::TraitDefault.requirement_ordinal: usize` identifies a requirement by position.
Fail-open: `ProofFact: PlainSyntaxEquivalent` and many expression row structs (`TableBinaryExpression`, `TableCastExpression`, `TableMatchExpression`, `TableMatchArm`, etc., `SYN/expressions/values.rs:312-430, :619-713`) derive `PlainSyntaxEquivalent`, so `same_syntax` on them is `self == other` on arena handles, not on the pointed-to nodes. The derive doc (derive/src/lib.rs:7-11) says structural equality for "continuation-sharing comparisons"; two structurally equal expressions in distinct trees compare equal only if their handles are equal. Traced for `expression` leaves only through `ExpressionNode::Binary` etc. (derive for `ExpressionNode` is `SyntaxEquivalent` which dispatches into these plain impls, values.rs:274-310). Consequence (inferred, not run): `same_syntax` on two expression nodes with distinct handle numbers returns `false` for a plain-derived variant even when contents match (fail-closed for equality, fail-open for "different").
Unused: `ExternalBinding::default` / `Item::default`: not grepped.

## SYN/derive/src/lib.rs (1-136)

Proc-macro crate. `SyntaxEquivalent` (:21): builds a `match (self, other)` with one arm per struct/enum variant, `true && same_syntax(left_i, right_i, left_text, right_text)` per field, and `_ => false` for enums (:46-69). `PlainSyntaxEquivalent` (:75): `self == other`, ignores both text owners (:82). `SyntaxRelocate` (:90): visits every field with `SyntaxRelocate::relocate(field, relocation)` (:108). Unions are refused with a compile error (:58-62, :120-124).
Judgment keyed on: derived over field lists, so a new field automatically participates (comment :7-11). Fail-open: none seen (a field whose type lacks the trait fails to compile).
Callers: derive is consumed only by `syntax-trees` (grep of `syntax_trees_derive::` not counted here).

## SYN/expressions/display.rs (1-359) and float_spelling.rs (1-119)

display.rs: `ExpressionNode::display_name(&self, table: &SyntaxTrees)` (:10-147); helpers for destructure pattern, binary, unary, cast, call, `StaticMachineArgument::display_path(table, opener)` (:287), `BinaryOperator::display_name` (:326), `UnaryOperator::display_name` (:351). Decides on SPELLING only (display strings). Same job elsewhere: `SRT/nodes/values.rs:19-345` (read, near-identical except identifiers come from `Identifier::as_str(table)` and syntax prints `..=` for inclusive ranges (:120), the resolved version prints `..` only, values.rs:114-123); the resolved `Range` display ignores `end_inclusive` (fail: a diagnostic string dropping `=`). Syntax `Self::Member` prints a position `.N` (:90-93), resolved prints `.member` only (values.rs:97-99).
float_spelling.rs: `FloatSpelling { text: Owned | SourceBacked(handle) | Materialized(FloatLiteral), source_span }` (:6-23); `same_syntax` compares text and, if either side is `Materialized`, requires both `Materialized` and equal (:73-85). `literal()` parses the spelling with `FloatLiteral::parse` unless materialized (:44-52). Test file `../tests/unit/float_spelling_tests.rs` is `#[path]`-included (:117-119), not in this unit.

## SYN/expressions/values.rs (1-797)

1. Purpose: syntax-level expression table and node types. 2. Shape: `ExpressionTable` via `paged_roster!` (:11-23); `ExpressionNode` (:281, 23 variants, `Name { members, application }` carries no symbol); row structs with `PlainSyntaxEquivalent` (:312-460) or `SyntaxEquivalent`; `StaticMachineArgument` (:495), `StaticSymbolApplication` (:510); match/pattern types (:619-790); `ExpressionTable::match_pattern_dependencies` (:75).
4. Decides: `match_pattern_dependencies` here includes `head`, every `Equal` field value, and `guard` (:79-103); the resolved version (SRT values.rs:1430-1443) visits `head` and `condition` instead, because resolution builds a combined `condition` the syntax tree does not have (`TableDestructurePattern` here lacks `condition`, :747-756, vs SRT :2537-2547). `set_call_acknowledgement` returns `false` for a non-call (:240-253). `copy_identifier_path_prefix` clamps `count` with `min(span.len())` (:197) (silent clamp).
5. Cases by shape: syntax destructure has no `condition` field; syntax `TableMemberExpression` has `access_source_span` (resolved does not).
6. Same job elsewhere: node types duplicate SRT values.rs (see above). `source_span_handle` conversion (:264-266) vs `source_span_index` in SRT (values.rs:2151-2156).
9. Fail-open: `copy_identifier_path_prefix` with `count` greater than the path length silently returns the whole path (:197). Open (callers not grepped).
`ProofMembershipFact` and `TableMatchArm` equality are handle-based (see declarations note above).

## SYN/files/pages.rs (1-47) and SYN/files/parsed.rs (1-77)

pages.rs: `ParsedSyntaxPages { pages: SealedPagedArena<SyntaxTrees>, sources: Vec<ParsedSyntaxSource> }` (:19); `syntax_for<T>(handle: Handle<T>)` maps any handle to its page by generation only: `Handle::from_parts(1, handle.generation())` (:33-35). `into_parts` flattens pages into a `Vec<SyntaxTrees>` and builds `ParsedSources` (:41-46).
parsed.rs: `ParsedSource` (:16, default `Arc::from("")`, scope `Product`), `ParsedSources`, `ParsedUnit { syntax, source, root_items, import_root_range }` (:50), `at_coordinates(source_id, generation)` (:59) which refuses generation 0 and relocates a clone via `SyntaxRelocate` (:66-71).
Judgment: page lookup keyed on handle generation (page identity), not source id (`syntax_for`, :33). Fail-open: `syntax_for` returns the page for ANY handle type whose generation matches a page, regardless of the handle's arena index (:34); a stale handle from another page with the same generation resolves (open; not traced to callers).

---

## Coverage

All 42 ranges in `units/unit_070.tsv` were read in full, in order, in chunks: 21,808 lines read in full; 0 lines skimmed; 0 lines not read.
(Sum of the TSV line counts, computed by hand: 2615 + 1183 + 547 + 58 + 481 + 1071 + 90 + 169 + 250 + 336 + 4512 + 2721 + 408 + 163 + 7 + 194 + 218 + 458 + 55 + 316 + 70 + 462 + 1216 + 1007 + 44 + 122 + 56 + 93 + 71 + 115 + 60 + 655 + 149 + 78 + 198 + 25 + 136 + 359 + 119 + 797 + 47 + 77 = 21,808.)
Test-heavy content that I read but recorded only by title and headline assertions: `source_symbols/tests.rs` (2,721), tests in `values.rs` (:447-961, 2570-2615), `table.rs` inline tests, `source_inputs.rs` tests (:674-1071). Callers not read: `control_flow_and_calls.rs`, `type_system_and_names.rs`, `state_bodies.rs` (SRT nodes outside the unit); typed-trees twins were only grepped.

## Five things a maintainer most needs to know about this unit

1. The symbol table's "source-free means permissive" rule is pervasive: with `sources: None` (or a symbol with no provenance span) `same_source_package`, `same_product_package_instance`, `source_reference_can_see_symbol`, `symbol_source_is_selected`, `source_binding_is_selected` all return true and an unsourced symbol counts as ambient (`table.rs:2177-2181, 2195-2197, 3926-3935, 3952-3964, 2226-2232`). Each is commented as intended for focused tests; whether a production path can build a table without a `SourceMap` was not traced.
2. Top-level lookup ends in positional fallbacks: a source-backed reference that matches no source-position rule gets `candidates.first()` (`table.rs:3831`), and a non-source-backed ordinary lookup takes the first candidate (`:3774`), though a comment at `:3496-3501` says neither may silently pick the first declaration for the signature-free path.
3. Three parallel expression-copy implementations exist in this unit alone: `SRT/.../table_copies.rs` (graph copy between resolved tables), `SRT/.../symbol_copies.rs` (copy plus symbol substitution, which skips `condition`, `subject_binding`, `proof_selectors` and the match `capture` in destructure arms), and `SYN/copying/bodies.rs` (syntax). typed-trees has a further `copy_*_from` family (grepped only). Same for node enums and display: `BinaryOperator`/`UnaryOperator` are defined in three representations with three `display_name` bodies.
4. Authored-selection rebase is two-phase (validate everything, then mutate) and atomic on failure, pinned by tests (`SRT/tests/resolved_records/mod.rs:178-462`); it covers expressions, tree statements, statement-table statements, transition targets, proof-fact memberships. It does not cover static arguments inside type references or constant expressions (not listed in the frontier, `selections.rs:23-30`).
5. Syntax equality is partly structural and partly by handle: `SyntaxEquivalent` derives walk fields, but any struct derived with `PlainSyntaxEquivalent` (many expression row types, `ProofFact`, `ConstQualification`, `PropositionBody`) compares arena handles by `==` (`derive/src/lib.rs:75-85`; `SYN/expressions/values.rs:312-460, 619-713`; `SYN/declarations/proofs.rs:19-32, 139`).

## What I am unsure about

- Whether any production caller builds a `SymbolTable` or `SourceMap` with `sources == None`, which decides if the "source-free permissive" paths are reachable outside tests.
- Whether the omitted fields in `SymbolCopy::expression_payloads` (`condition`, `subject_binding`, `proof_selectors`, `capture`) matter: callers of `copy_callable_signature` / `copy_proof_fact_with_symbols` were found (1 each) but I did not read what they copy.
- The coverage of `tools/ratchets/dense_id_maps.txt` for the `BTreeMap`/`HashSet`/`HashKeyMap` uses (`source_inputs.rs:8-9, 30-31, 209`; `lookup_indexes.rs:9`; `table.rs` index caches): not checked.
- `ExpressionTable::reserve_*`, `set_*_at_offset`, `push_*` call-site counts were not grepped; they may be single-use.
- `StatementTable` and `TypeReferenceTable` methods used by `tables/mod.rs` and `selections.rs` live outside this unit (not read).
- The per-name counts for the 8 `table.rs` public helpers in the combined grep (53 hits in 28 files) were not split by name.
