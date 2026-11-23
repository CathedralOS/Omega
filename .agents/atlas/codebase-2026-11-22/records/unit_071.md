# Record: unit_071 (area 05-source-representations)

Checkout `4f3c255d0559`. Paths below are relative to
`source-rust/subsystems/compiler/source-to-psi/representations/` (written `R/`). All counts come from
`grep -rEh --include=*.rs "<pattern>" source-rust tools | wc -l` run inside the worker-2 checkout; the `_recycling/` archive was excluded where noted.
Grepped files lists are of `source-rust` and `tools` only.

Unit has 40 files, 21,005 lines. Section numbers are the row numbers of the unit file (rows 13 to 22 are one group). All 40 rows were read in full;
row 36 (`callable_applications.rs`) is recorded after row 40 because it was read last.

---------------------------------------------------------------------------------------------------

## syntax-trees: `inspection/` (snapshot code)

These five files turn a `SyntaxTrees` forest into a serde-serializable tree of plain `*Snapshot` structs. They are read-only views, not a pipeline stage product.
Users of the snapshot JSON API outside the crate (`grep -rEl "snapshot_json_pretty|snapshot_json\b|to_json_pretty"`): 9 files total
including parser tests/benchmarks (`pipeline/01_tokens-to-syntax-trees/tests/unit/*.rs`, `benchmarks/parser_timing.rs`, `benchmarks/streaming.rs`),
`driver/parse_source_batch/parsed_files.rs`, `typed-trees/storage/forest.rs`, `typed-trees/proofs/evidence_and_inspection.rs` (the last two are other `to_json_pretty` users, inferred, not read).

### 1. `syntax-trees/inspection/expressions_and_types.rs` (877 lines, read)
1. Purpose: snapshot types and builders for identifiers, type references, static arguments, expressions and match patterns.
2. Shape: `IdentifierSnapshot` (`:12`; `text`, `source_id`, `start`, `end`, `source_backed`), `StaticArgumentSnapshot` (`:22`, untagged enum of Type/Path/Application/Const/EvidenceProjection),
   `TypeReferenceSnapshot` (`:43`, 11 variants incl. `Missing`), `ExpressionSnapshot` (`:165`, 24 variants), `MatchPatternSnapshot` (`:118`), `MatchFieldSnapshot` (`:142`).
   Entry fns: `snapshot_type_reference_handle` (`:288`), `snapshot_expression_handle` (`:409`), `snapshot_static_argument` (`:659`), `snapshot_identifier` (`:765`), `snapshot_match_fields` (`:800`), `snapshot_match_pattern` (`:829`).
3. Inputs/outputs: reads `SyntaxTrees` tables (`type_references`, `expressions`, `statements`, `items`); returns owned snapshot values. No mutation.
4. Decides: a handle that is not valid becomes `TypeReferenceSnapshot::Missing` (`:292-294`) or an empty-path `Name` expression (`:413-418`). Pattern kind: a destructure with a valid `subject_binding` is a `Binding` (`:840`),
   otherwise `Destructure`. Policy operators are chosen by the pair (operator, arithmetic domain) (`:716-739`), keyed on the enum values. Atomic ordering is rendered with `format!("{:?}", atomic.ordering)` (`:476`),
   i.e. a Debug string, not an enum.
5. Cases by shape: `snapshot_policy_operator` lists exactly 6 operators x 3 domains (add, subtract, multiply, divide, modulo, shift left/right with Wrapping/Saturating/Trapping), `_ => return None` (`:738`); None falls back to the plain operator name (`:480-481`).
   `snapshot_binary_operator` (`:742`) lists 18 operators. Cast `form` is one of 8 fixed strings (`:492-504`).
6. Same job elsewhere: statement/item snapshotting is in sections 2, 4 (same file family). `display_expression_name` (a text renderer of expressions) is called at `:842`; it is defined outside this unit (suspected a second text-rendering of expressions, not read).
7. Unused: all `pub(crate)` fns are called from sibling snapshot files (`items.rs`, `statements.rs`, `mathematical.rs`).
8. Stated limits: none.
9. Fail-open: `snapshot_expression_handle` on an invalid handle returns an empty `Name` (`:413`). This only affects the snapshot text, not an accept/reject decision. Class: traced (read the callers inside the unit; snapshot is output only). No acceptance path.

### 2. `syntax-trees/inspection/items.rs` (1,037 lines, read)
1. Purpose: snapshot structs and builders for items (machine, data, trait, conformance, domain, const, measure, operator, proposition, use/file import, capability, platform, type alias), type parameters, contracts, proof facts, data members.
2. Shape: `ItemSnapshot` (`:24`, 15 variants), `snapshot_item` (`:381`), `snapshot_operator` (`:731`), `snapshot_type_parameter` (`:752`), `snapshot_capability_contract` (`:880`), `snapshot_data_member` (`:933`), `snapshot_external_binding` (`:1020`).
3. Inputs/outputs: reads `syntax_trees.items` and the expression/type tables; produces snapshots.
4. Decides: keyed on enum variants of the syntax node (match arms). `supply` is rendered from `DataSupplyMode` (`:465-468`); `kind` of a type parameter from `TypeParameterKind` (`:757-818`); contract crash cause from `CrashCause` (`:894-897`) to the strings `"Trap"`/`"Abort"`.
5. Cases by shape: type parameter kinds map to 8 fixed strings including `"machine"` for three different `MachineParameterContract` cases (`Structural`, `Nominal`, `None` all give `"machine"`, `:787-805`).
   `Item::Platform` exists in `ItemSnapshot` (`:157`) but `snapshot_item` has no `Item::Platform` arm in the part read (`:381-729`); the match is exhaustive, so `Item` has no such variant (inferred; `Item` enum is defined outside the unit, not read).
6. Same job elsewhere: `ConformanceMember::Machine/Synthesized/TraitDefault` each rebuild an `Item::Machine(machine.clone())` and call `snapshot_item` (`:414-439`), cloning the whole machine node per member.
7. Unused: none checked individually; all `pub(crate)` fns are used by `statements.rs`/`mathematical.rs`.
8. Stated limits: none.
9. Fail-open: none seen. Line `:24` has formatting `pub enum ItemSnapshot {    Capability {` (extra spaces); not behavior.

### 3. `syntax-trees/inspection/mathematical.rs` (115 lines, read)
1. Purpose: snapshot of a mathematical definition (the root-level `MathematicalDefinition` list).
2. Shape: `MathematicalDefinitionSnapshot` (`:14`), `MathematicalTypeSnapshot` (`:39`; Ordinary/Arrow/Application), `snapshot_definition` (`:54`, `pub(super)`).
3. I/O: reads `trees.items.mathematical_*`; produces snapshots.
4. Decides: match on `MathematicalDefinitionBody` (`Definition`/`Assumption`, `:76-85`) and `MathematicalTypeNode` (`:90`).
5. Cases by shape: none beyond the enums. 6/7: one caller (`snapshot.rs:62`). 8: none. 9: none seen.

### 4. `syntax-trees/inspection/snapshot.rs` (119 lines, read)
1. Purpose: names the root `SyntaxTreesSnapshot` and attaches the four sibling snapshot modules with `#[path]` (`:105-115`); implements `SyntaxTrees::snapshot` and `PhaseSnapshot for SyntaxTrees` (`:97`).
2. Shape: `SyntaxTreesSnapshot` (`:36`; `source_id`, `root_items`, `mathematical_definitions`, `constant_extent_probes`), `ConstantExtentProbeSnapshot` (`:30`), `SyntaxTrees::snapshot` (`:56`), `snapshot_json(_pretty)` (`:88`, `:92`).
3. I/O: reads `roots` and tables. Only root items, root mathematical definitions and constant-extent probes are written out; the other table rows are reachable only through them.
4. Decides: nothing.
5. Cases: root lists only (a row not reachable from a root is not in the snapshot; compare `PartialEq` at `storage/equality.rs:24`, which does compare detached rows).
6. Same job elsewhere: the equality in section 9 also walks the same tables.
7. `PhaseSnapshot` implementers/users: `grep -rEh "PhaseSnapshot"` was not run; `artifacts::PhaseSnapshot` is imported at `:26` (not counted).
8/9: none.

### 5. `syntax-trees/inspection/statements.rs` (630 lines, read)
1. Purpose: snapshot structs and builders for states, signatures, statements, dispatch arms, assembly, transition targets.
2. Shape: `StatementSnapshot` (`:76`, 11 variants), `StateSignatureSnapshot` (`:32`), `snapshot_statement` (`:318`), `snapshot_state_node` (`:221`), the `state_signature_snapshot!` macro (`:236`) used twice (`:281`, `:288`: `snapshot_state_signature` and `snapshot_state_signature_node` have identical bodies, a duplicated function pair), `snapshot_transition_target` (`:597`).
3. I/O: reads `syntax_trees.statements`, `items`, `expressions`.
4. Decides: `StatementNode` variant; `TransitionExit` -> `"Trap"`/`"Abort"`/None (`:554-558`). `LocalData.role` is `format!("{:?}", value.role)` (`:493`), a Debug string.
5. Cases: assembly `scratch` slots drop invalid expression handles (`:391`), so an unused scratch slot disappears from the snapshot and positions are not preserved. `default_body.unwrap_or_default()` (`:269`, `:270`) treats a missing default body as empty.
6. Same job elsewhere: see 2 above (the macro pair).
7. Unused: `snapshot_state_signature_node` and `snapshot_state_signature` both have callers (items.rs `:713`, `:721`, `expressions_and_types.rs:344`).
8. Stated limits: none. 9. Fail-open: none seen (output only).

---------------------------------------------------------------------------------------------------

## syntax-trees: crate root, statements, storage, text, types

### 6. `syntax-trees/lib.rs` (4 lines, read)
`#![forbid(unsafe_code)]`; `pub mod syntax_trees; pub use syntax_trees::*;`. Nothing else.

### 7. `syntax-trees/statements/assembly.rs` (224 lines, read)
1. Purpose: row types for inline assembly: authored form (`AuthoredAssembly`) and the lowered control-flow form (`AssemblyControlFlow`, `AssemblyBlock`, `AssemblyExit`).
2. Shape: `AuthoredAssembly` (`:16`; `requires`, `ensures`, `clobbers: Option<Box<[Identifier]>>`, `blocks`), `AuthoredAssemblyBlock` (`:69`), `AssemblyInstruction` (`:83`; `mnemonic: Identifier`, `operands`, `source_span`), `AssemblyOperands` (`:96`), `AssemblyControlFlow` (`:110`; `target_requirement`, `primitives`, `scratch: [ExpressionHandle; ASM_SCRATCH_SLOT_COUNT]`, `blocks`), `AssemblyBlock` (`:136`), `AssemblyExit` (`:152`; Fallthrough/Jump/ZeroFlag/EnclosingTransition), `AssemblyTable` via `paged_roster!` (`:160`).
   `AuthoredAssembly::visit_operands` (`:25`).
3. I/O: storage only; `visit_operands` calls a closure on each expression handle.
4. Decides: nothing, except the match in `visit_operands` (`:38-56`) over `AssemblyOperands` and `TransitionTargetNode`.
5. Cases: the instruction mnemonic is kept as an `Identifier` (source spelling), i.e. an assembly instruction is identified by its spelling at this stage (`:84`).
6. Same job elsewhere: `TableDispatch::visit_operands` (`statements/control_flow.rs:431`) does the same visit for dispatch.
7. Unused: `AuthoredAssembly::visit_operands`: `grep -rEh "visit_operands"` count in source-rust/tools: see section 8 note (not separately counted; suspected callers in pipelines 01/02).
8. Stated limits: `// The parser preserves instruction spelling, operand expressions and labeled blocks.` (`:7`).
9. Fail-open: none seen.

### 8. `syntax-trees/statements/control_flow.rs` (595 lines, read)
1. Purpose: statement rows, transitions, dispatch arms, transition targets, and the `StatementTable`.
2. Shape: `StatementNode` (`:233`, 11 variants), `TableDispatch` (`:406`), `DispatchArm` (`:478`), `DispatchPattern` (`:423`), `TableTransition` (`:514`; `patterns`, `target`, `continuation`, `guard`, `proof_selectors`, `exit`, `ends_dispatch`), `TransitionTargetNode` (`:580`; Named/Value/Terminal), `TransitionExit` (`:558`), `TableLocalData` (`:363`), `StatementTable` (`:30` via `paged_roster!`, 8 arenas + nested `assembly`).
3. I/O: storage plus `TableDispatch::visit_operands` (`:431`), `TableCall::call` (`:342`), `TableTransition::targets` (`:526`), `DispatchArm::proof_selectors` (`:496`).
4. Decides: `DispatchArm::proof_selectors` returns a selector span only when the arm has exactly one pattern (`:497-500`: `[pattern] => ...`, other lengths -> empty). `TableCall::call` panics with `unreachable!("a call statement holds a call expression")` if the expression is not a call (`:349`).
5. Cases by shape: "Only a single exact result-case pattern can carry proof selectors." (`:495`).
6. Same job elsewhere: `visit_operands` for assembly (section 7).
7. Unused: no per-function grep run.
8. Stated limits: the quote at `:495` above. `Default for StatementNode` is an expression with an invalid handle (`:307-311`).
9. Fail-open: none seen. `transition_pattern_mut` and `replace_statement` are mutating entry points on a table the brief calls "unresolved syntax" (`:83`, `:191`); who calls them is not traced.

### 9. `syntax-trees/storage/equality.rs` (143 lines, read)
1. Purpose: structural syntax equality (`SyntaxEquivalent`) with the two forests' text owners, and `PartialEq for SyntaxTrees`.
2. Shape: trait `SyntaxEquivalent` (`:5`); `impl PartialEq for SyntaxTrees` (`:24`); macros `indirect!` (`:78`), `arena_equality!` (`:89`), `plain!` (`:97`, applied at `:102`, `:128`, `:133` to 26 leaf types).
3. I/O: compares `source_id`, `roots`, and the `items`, `expressions`, `statements`, `type_references` tables with each side's text owner. `text` is only used to resolve names.
4. Decides: identifiers compare by spelling only: `self.as_str(left_text) == other.as_str(right_text)` (`:21`), so generated and source-backed names with equal text are equal, and spans are ignored for identifiers (test `tests/unit/forest.rs:1255-1258` states this). `Handle<T>` and `HandleSpan<T>` compare by index and generation (`:48-57`), not by pointee.
5. Cases: `plain!` list is closed: a new field type without `SyntaxEquivalent` fails to compile (inferred from the macro).
6. Same job elsewhere: `PartialEq for TokenStream` (`tokens/token_stream.rs:188`) uses spelling and spans.
7. Count: `PartialEq for SyntaxTrees` is one definition (`grep -rEh "PartialEq for SyntaxTrees" ... | wc -l` = 1).
8. Stated limits: none. 9. Fail-open: handle comparison by index means two forests whose rows were laid out differently compare unequal; the opposite (accepting different content) is not seen. Class: traced within the file.

### 10. `syntax-trees/storage/paged_tables.rs` (70 lines, read)
`paged_roster!` macro (`:1`): generates a table struct from `arena` and `roster` field lists, derives `SyntaxEquivalent`/`Record`, and generates `append_checkpoint`, `rewind_appended`, `share_pages`, `new_with_generation`, `adopt`, `begin_page`, `relocate_page` for it (`:21-65`). `relocate_page` returns `Result<(), &'static str>`.
Users: 5 tables in this unit (`AssemblyTable`, `StatementTable`, `TypePreparation`, `TypeReferenceTable`, plus items/expressions tables outside the unit, not read).
Same job elsewhere: none. Stated limits: none. Fail-open: none seen.

### 11. `syntax-trees/storage/relocation.rs` (119 lines, read)
1. Purpose: moves a standalone page's coordinates (generation and source id) to the admitted ones.
2. Shape: `PageRelocation` (`:8`: `from_generation`, `generation`, `from_source`, `source`), trait `SyntaxRelocate` (`:15`), impls for handles, spans, `SourceId`, containers, and a `coordinate_free!` list of 30 types (`:88-118`).
3. Decides: `Handle::relocate` rewrites a handle only if its generation equals `from_generation` (`:21`); a `SourceId` only if it equals `from_source` (`:35`). Anything else is left unchanged.
4. Fail-open: a handle from a different generation is silently left as is (`:21`, `:35`). A foreign handle inside a page is not refused here. Class: open (not traced beyond `syntax_trees.rs:202`, which refuses a forest with more than one page).
5. Stated limits: none here (see `syntax_trees.rs:198-201`).

### 12. `syntax-trees/syntax_trees.rs` (367 lines, read)
1. Purpose: the `SyntaxTrees` forest: roots, tables, checkpoints and merging.
2. Shape: `SyntaxTrees` (`:24`; `source_id`, `roots: SyntaxTreeRoots`, `tables: SyntaxTreeTables`), `SyntaxTreeTables` (`:31`; `text`, `items`, `expressions`, `statements`, `type_references`), `SyntaxTreeRoots` (`:40`; `items`, `constant_extent_probes`, `mathematical_definitions`).
   Fns: `append_checkpoint`/`rewind_appended` (`:57`, `:69`), `extend_from` (`:163`), `adopt_page` (`:183`), `begin_page` (`:194`), private `relocate_unit` (`:202`), `const_expression_contains_name` (`:112`), `push_root_item` (`:106`). Module wiring by `#[path]` (`:314-367`).
3. I/O: `extend_from` copies each root item, mathematical definition and constant-extent probe from another forest (`copy_item_from`, `copy_mathematical_definition`, `copy_expression_from`), then remaps supplied operand owners (`:179-180`).
4. Decides: `relocate_unit` refuses unless `page_count() == 1` (`:207-209`). `const_expression_contains_name` is true for `Name`, recurses into `Binary` and `Unary`, and returns false for every other expression kind (`:113-121`).
5. Cases by shape: `const_expression_contains_name` handles three shapes; calls, member, index, cast and all else count as "no name" (`_ => false`, `:120`).
6. Same job elsewhere: `extend_from` and `copy_item_from` (outside the unit, `copying/`) share the copy engine.
7. Unused: `const_expression_contains_name` appears in 3 files (`grep -rEl`): this file, `.../02_syntax-trees-to-symbol-resolved-trees/declarations/generic_data/const_evaluation/domains.rs`, `.../facts.rs`; total 6 matches. `relocate_unit` is private and a grep for `relocate_unit` was not run: its caller was not located in this unit (suspected `files/pages.rs`, not read).
8. Stated limits: doc comment at `:198-201`: "A forest with more than one page, or whose rows were rewritten after parsing, is refused." Only the page-count half is checked in code (`:207`); no check for "rewritten after parsing" is visible in `relocate_unit`.
9. Fail-open: `const_expression_contains_name` `_ => false` (`:120`): an expression such as a call that contains a name is reported as not containing a name. What this decides at the callers (the two files above) was not read. Class: open.

### 13-22. Test files under `syntax-trees/tests/` (group, 12 files, read)
- `tests/identifier_paths.rs` (22): `Identifier::from_path` joins `outer` and `inner` into `outer::inner` and keeps the full span (`:6-21`).
- `tests/page_roots.rs` (31): `adopt_page` preserves constant-extent probes; `extend_from` snapshot equals source snapshot.
- `tests/unit/expression_tests.rs` (191): copy of saved-subject aliases, match arm order, call acknowledgement copy-on-write (`share_pages` then `set_call_acknowledgement`, revision numbers), and a rejected acknowledgement leaving the shared page untouched (`:175-191`).
- `tests/unit/float_spelling_tests.rs` (48): float spelling equality ignores owner and provenance, uses exact text; foreign owner panics with "does not belong to this forest".
- `tests/unit/forest.rs` (1,373): copy and `extend_from` tests: shared case subject kept shared and isolated per copy (`:74`, `:119`), page adoption (`:176`), const initializer normalization surviving record encode/decode, clone and copy (`:259`), semantic NaN survives copy (`:416`), normalized const argument expression copy (`:463`), const argument origins deep copy (`:522`), duplicate normalization panics (`:660`), nested generic origins (`:684`), generic instance origin (`:766`), visibility preserved (`:817`), root payload handles (`:878`), keyword spans (`:963`), call arguments and nested spans (`:1007`, `:1077`), text-owner lifetime (`:1183`), contextual equality (`:1226`), constant destination debt (`:1265`), const result materialization (`:1330`).
  Fixture helper `single_state_machine` (`:220`) lists every `Machine` field by name, so adding a field to `Machine` breaks it.
- `tests/unit/snapshot_tests.rs` (198): 3 tests: data snapshot shape, variant payload erased field, numbered data field relevance.
- `tests/unit/statement_tests.rs` (81): transition payload handles, record encode/decode of acknowledgements.
- `tests/unit/text_owner_storage_tests.rs` (48): restored owners keep shared pages without collision; conflicting page identity refused on decode (`:37-48`).
- `tests/unit/text_owner_tests.rs` (23): adopt moves ownership; unrelated forest cannot resolve a page (panic).
- `tests/unit/types_tests.rs` (14): `named_nodes_from(watermark)` excludes earlier nodes.
- Test count: 28 `#[test]` attributes in the group (counted by reading; not grepped).
Fail-open: none (tests). Note: `forest.rs:1255-1258` documents that generated and source-backed names with equal spelling compare equal.

### 23. `syntax-trees/text/identifiers.rs` (163 lines, read)
1. Purpose: `Identifier`: a name with text and a source span.
2. Shape: `Identifier` (`:8`: `text: IdentifierText` (Missing | Owned(Box<str>) | SourceBacked{source handle}), `source_span`), `from_path` (`:33`), `new` (`:52`), `generated` (`:59`), `source_backed` (`:66`), `as_str` (`:74`), `is_source_backed` (`:101`), `trailing_span` (`:89`), `is_self_receiver` (`:97`), `JoinedSyntaxPath` for `[Identifier]` (`:141`, separator `"::"`).
3. Decides: `is_source_backed` is true when the span is non-empty (`start != end`, `:102`), regardless of the text variant: `Identifier::new("x", nonempty_span)` reports source-backed although its text is owned (inferred from `:52-57` and `:101-103`). `IdentifierSnapshot.source_backed` takes this value (`expressions_and_types.rs:775`). `is_self_receiver` defers to `language_core::is_self_receiver(self.as_str(..))`, a string compare (`:98`).
4. Cases: `from_path` expects a nonempty slice and panics otherwise (`:36`, `:40`); asserts same source id in debug only (`:42`).
5. Fail-open: `trailing_span` leaves the span unchanged when `end < length` (`:91`); no acceptance effect. None seen otherwise.
6. Stated limits: comment `:23-24`: handles "name process-wide text generations, not the syntax page, so only the identifier's source span moves" (relocation).

### 24. `syntax-trees/text/owner.rs` (168 lines, read)
1. Purpose: owns the retained source text pages that source-backed identifiers point at.
2. Shape: global `NEXT_SOURCE_GENERATION: AtomicU32` (`:7`), `RetainedSource` (`:10`), `SyntaxTextOwner` (`:20`, a sorted `Vec<RetainedSource>`), trait `SyntaxTextContext` (`:15`), `retain` (`:25`), `retain_from` (`:35`), `adopt` (`:46`), record read/write (`:116-160`).
3. Decides: a handle must have `arena_index == 1` (assert, `:61-65`) and its generation must be found, else panic "does not belong to this forest" (`:74`). On decode, generations must be strictly increasing (`:147`), zero refused (`:102`), and generations are remapped to fresh process-wide ones (`:106`).
4. Cases: generation counter is a process-global; exhaustion panics (`:30`).
5. Fail-open: none seen. Lookups of an unknown handle panic, not accept.
6. Same job elsewhere: `Handle::generation` relocation in `storage/relocation.rs` (section 11) is keyed the same way.

### 25. `syntax-trees/types/preparation.rs` (543 lines, read)
1. Purpose: side tables on `TypeReferenceTable` that record what declaration preparation did to a type occurrence (generic application origin, const argument normalization, alias application origin, supplied operand owner, pending const argument, alias header reference), keyed by type reference handle.
2. Shape: `TypePreparation` (`:12`, 8 arenas), `GenericApplicationOrigin`, `ConstArgumentOrigin` (`:82`), `ConstArgumentNormalization` (`:113`), `TypeAliasApplicationOrigin` (`:35`), `SuppliedOperandOwner` (`:52`), and ~30 methods on `TypeReferenceTable` (`:137-543`).
3. I/O: reads and writes the preparation arenas; `replace_type_occurrence` (`:506`) moves all five handle-keyed row families from `source` to `destination` using the `transfer!` macro (`:512`).
4. Decides: every lookup is a linear scan of an arena keyed on a `TypeReferenceHandle` (e.g. `is_alias_header_reference` `:144-149`, `supplied_operand_owner` `:181-193`, `const_argument_normalization` `:316-321`, `generic_application_origin` `:412-418`). No map or dense index (compare AGENTS.md data-oriented rule). `retain_*` functions assert uniqueness (`:162`, `:341-342`, `:379`, `:399`, `:425-426`).
5. Cases: `alias_use_at` (`:465`) accepts only when exactly one alias application row matches the (source, declaration) pair; two matches or none return `None` (`:471-478`), and only `Named` or `Generic` application heads are rewritten (`:482-486`).
6. Same job elsewhere: `transfer!` (`:512`) and the 5-row lists at `:537-541` are the occurrence-identity list; `generic_nodes` (`:435`) re-lists three of the same families.
7. Unused: `grep -rEh "const_expression_nodes|const_parameter_array_nodes_from|named_nodes_from|domain_constraints\(|\.generic_nodes\("` gives 28 matches in 18 files (parser tests, pipelines 02 and 03); `alias_header_reference` appears in 4 files, 11 matches.
8. Stated limits: `supplied operand lost its declaration during syntax copying` (`:189`).
9. Fail-open: `supplied_operand_owner` returns `Ok(ItemHandle::invalid())` when no row exists (`:193`): "no row" and "no owner" look the same to a caller. `remap_supplied_operand_owners` sets a row to the default (invalid) declaration when its source declaration is not in the `declarations` list (`:276-280`). `generic_application_origin` returns an invalid handle (default) when absent (`:416-417`). Class: open for the callers (pipeline 02 and 03, not read).

### 26. `syntax-trees/types/references.rs` (370 lines, read)
1. Purpose: type reference rows (`TypeReferenceNode`, 10 variants) and the `TypeReferenceTable`.
2. Shape: `TypeReferenceTable` (`:15`; arenas `type_references`, `type_reference_handles`, `constraints`, nested `preparation`), `TypeReferenceNode` (`:243`; Reference/Constrained/FixedArray/Slice/Generic/StaticSelection/Callable/ConstExpression/Named/Unit), `FixedArrayLength` (`:293`; Literal/ConstParameter/ConstCall/Expression), `TypeConstraintNode` (`:342`), `DomainConstraint` (`:357`).
3. Decides: `FixedArrayLength::same_value` compares `ConstParameter`/`ConstCall` by spelling (`:317-319`) and `Expression` by handle equality (`:320`); different variants are unequal (`:321`). `display_name` renders `ConstCall` as `format!("{}()", name)` and any `Expression` as `"<constant expression>"` (`:329-330`).
4. Cases: `const_parameter_array_nodes_from` finds only `FixedArray` whose length is `ConstParameter` (`:72-75`).
5. Same job elsewhere: `FixedArrayLength` snapshot (section 1 `:372-388`).
6. Fail-open: `same_value` `Expression` equality by handle (`:320`) says two distinct but identical constant expressions differ (conservative). None seen otherwise.

---------------------------------------------------------------------------------------------------

## tokens

### 27. `tokens/files/lexed.rs` (20 lines, read)
`LexedSource` (`:8`; `source_id`, `path: PathBuf`, `origin: Option<SourceOrigin>`, `tokens: TokenStream<'static>`, `scope: DependencyScope`) and `LexedSources` (`:17`; `Arena<LexedSource>`, `batch: HandleSpan<LexedSource>`). Data only.

### 28. `tokens/lexemes/keywords.rs` (58 lines, read)
Macro `define_keywords!` (`:4`) generates `KeywordKind` with `from_lexeme` (string match) and `lexeme` (`:12-25`). 28 keywords listed (`:29-58`), plus `Unknown` default. Users of `KeywordKind::from_lexeme|ordered_lexemes|matching_prefix`: `pipeline/00_source-files-to-tokens/lexer.rs` plus the tokens crate (6 matches in 3 files). Keyword set is a closed list: a word not in it is an identifier (inferred, lexer not read). Stated: "One keyword vocabulary generates classification and diagnostic spellings." (`:1`).

### 29. `tokens/lexemes/kinds.rs` (62 lines, read)
`TokenKind` (`:11`), `CommentKind`, `NumericLiteralKind`, `IntegerLiteralKind` (`base`, `empty_digits`, `has_suffix`), `FloatLiteralKind`, `NumericBase`. Data only. Fail-open: `Default` is `Identifier` for `TokenKind` and `Unknown` for keyword/punctuation (`:12`).

### 30. `tokens/lexemes/punctuation.rs` (220 lines, read)
`PunctuationKind` (`:4`, 76 variants with `Unknown` default), `define_punctuation!` (`:80`) generating `ordered_lexemes`, `matching_prefix` (first-byte dispatch then the first listed spelling that `starts_with`), `lexeme`. Cases by shape: policy operators for `+ - * / %` and `<<` with suffix `%`, `|`, `!` (`:57-74`), listed before the plain spelling for longest match; `>>` has no token (comment `:49-56`). Note `%%` is the wrapping spelling for `%` (`:138`). `matching_prefix` returns the first match in list order, so ordering is the longest-match rule (`:1`). Test `tokens/tests/punctuation.rs:4` checks every listed spelling matches itself.

### 31. `tokens/lexemes/token.rs` (62 lines, read)
`Token` (`:9`: `kind`, `decoded: Handle<DecodedRange>`, `span`) with predicate accessors. `is_trivia` is whitespace or comment (`:59`).

### 32. `tokens/lib.rs` (4 lines, read)
`#![forbid(unsafe_code)]`, `pub mod token_stream; pub use token_stream::*;`.

### 33-34. `tokens/tests/punctuation.rs` (15) and `tokens/tests/storage.rs` (103) (read)
Storage tests: recycled scratch does not overwrite escaped literals (`:5`), literal bytes shared and stable (`:27`), `retain_source` owns one source reference (`:47`), decoded ranges survive clone (`:65`), unrelated equal-length source refused by panic (`:85`), stream equality uses spelling and spans, not ownership (`:93`).

### 35. `tokens/token_stream.rs` (218 lines, read)
1. Purpose: compact token records with their source text and decoded literal bytes for one file.
2. Shape: `TokenStream<'source>` (`:16`; `source: SourceBacking` Borrowed|Retained, `tokens: Vec<Token>`, `decoded_ranges: Arena<DecodedRange>`, `decoded_bytes: Option<Arc<Vec<u8>>>`), `TokenScratch` (`:24`), `with_capacity` (`:58`), `with_scratch` (`:67`), `recycle` (`:78`), `push_source` (`:100`), `push_decoded` (`:112`), `retain_source` (`:130`), `spelling` (`:163`), `literal_bytes` (`:168`).
3. Decides: `push_source` asserts the kind is not `StringLiteral` (`:101-104`). `retain_source` asserts the supplied `Arc<str>` is the same pointer as the borrowed text (`std::ptr::eq`, `:131-134`). `literal_bytes` returns `None` for a token without a decoded range (`:169`). `PartialEq` compares source text, length, kinds, spans and literal bytes (`:188-197`).
4. Users: `TokenScratch|with_scratch`: 17 matches in 8 files (lexer, batch driver, `source-syntax/lib.rs`, tests).
5. Fail-open: none seen. `recycle` drops the decoded byte buffer when it is still shared (`Arc::get_mut` fails, `:87-91`) instead of clearing shared bytes.

---------------------------------------------------------------------------------------------------

## typed-trees `callables/` (small files)

### 37. `typed-trees/callables/callable_copies.rs` (939 lines, read)
1. Purpose: copies or rebinds symbols in a callable type (runtime callable contract, with its signature, parameters, contracts and facts) between `TypedTreeTables`.
2. Shape: `TypeCopy` (`:19`, copy source -> target; `contracts: Vec<(src, dst)>` map), `TypeRemap` (`:28`, rewrites symbols in place via `SymbolRebinding`; `contracts: Vec<handle>` visited set), trait `TypePayloadTransform` (`:480`) with impls for both, free fns `static_argument_types` (`:569`), `application_types` (`:595`), `expression_type_payloads` (`:655`). Public entry: `copy_type_reference_from` (`:39`), `copy_type_reference_with_expressions` (`:47`), `remap_type_reference_symbols_in` (`:56`), `copy_runtime_callable_binding_from` (`:64`), `remap_runtime_callable_binding_symbols_in` (`:77`).
3. I/O: reads `source.runtime_callable_contracts`, `data_type_parameters`, `state_parameters`, `signature_invokes`, `signature_contracts`, `proof_facts`, expression tables; inserts copies into `target`, resets `resolved_source` to `None` on copies and remaps (`:125`, `:330`).
4. Decides: keyed on handle identity: visited sets are `Vec` scans (`:108-111`, `:323`, `:426`, `:502`). `expression_type_payloads` matches `ExpressionNode` with every variant listed and no wildcard (`:661-736`).
5. Cases by shape: copies handle three proof-fact kinds (`Expression`, `Membership`, `Proposition`) (`:250-282`), four `MachineParameterContract` kinds (`:148-169`), five `AuthoredInvocationTarget` kinds (`:384-393`). Asserts that spans have the stated length (`:137-141`, `:188-192`, `:202-206`, `:239-243`).
6. Same job elsewhere: `TypeCopy` and `TypeRemap` duplicate the same signature walk (`signature` `:130` vs `:334`, `parameters` `:234` vs `:413`, `fact` `:250` vs `:425`); the `TypePayloadTransform` trait (`:480`) is the shared part for expression payloads only.
7. Unused: no per-fn grep.
8. Stated limits: none.
9. Fail-open: `TypeRemap::signature` `AppliedService`, `Unresolved`, `Undeclared` invocation targets are left unchanged (`:391-393`), correct only if they hold no symbol of the rebound set; not traced. Class: open. Panics via `expect` on missing static member/build request (`:581-583`, `:674`).
   Inline tests (`:762-939`): invocation remap refuses a bad tail before changing the prefix; provider readiness request copied and rebound once.

### 38. `typed-trees/callables/callable_lifetimes.rs` (76 lines, read)
1. Purpose: resolves a signature's own lifetime telescope under the declaring owner and the preceding applications.
2. Shape: `CallableLifetimeBinding` (`:9`, pub: `parameter`, `argument` identities), `CallableLifetimeFrame` (`:15`), `CallableLifetimeApplication` (`:21`), `resolve_callable_lifetime_bindings` (`:30`).
3. Decides: the receiving parameter must have a lifetime identity under `declaration` whose declaration equals the lifetime owner (`:45`); repeated parameter refused (`:47`); argument identity found under `argument_owner`, then rewritten by the latest preceding application that binds it (`:56-69`).
4. Unused: `grep -rEn "resolve_callable_lifetime_bindings"` in `source-rust`/`tools` finds only its definition (1 match, this file). `CallableLifetimeFrame` and `CallableLifetimeApplication` are matched only in this file. The module is `pub mod callable_lifetimes` (`callables/mod.rs:3`) and re-exported in `exports.rs:8`, but no caller exists (grepped). A second, private `CallableLifetimeBinding` struct is defined in `callable_applications.rs:8403`.
5. Fail-open: none seen. Stated limits: "This is source formation metadata." (`:2`).

### 39. `typed-trees/callables/callable_signature_shapes.rs` (1,946 lines, read)
1. Purpose: compares callable signatures (declaration matching, callback correspondence, call-site lookup of the selected requirement or operand) before bodies exist.
2. Shape: `realizes_signature` (`:24`, `pub(crate)`), `realization_static_binders` (`:77`), `BoundCallableShape` (`:141`), `CallableDeclaration` (`:166`; Signature | Machine), `CallableSignatureShape` (`:734`), `compare_callable_signature_shapes` (`:744`), `CallbackSignatureIdentity` (`:787`), `TypedTrees::compare_callback_signature_types` (`:829`), `selected_implementation_operand` (`:242`), `source_call_requirement_signature` (`:305`), `source_conformance_call_member` (`:385`), `source_call_selected_operand` (`:486`), `source_call_record_operand` (`:604`), `source_call_declaration` (`:688`), `SelectedRecordCallOperand` (`:14`). Tests: `invocation_lifetime_tests` (`:986`), `callback_signature_tests` (`:1320`, 12 tests).
3. I/O: reads `TypedTrees` (machines, states, specializations, static applications, conformance rows, trait requirement signatures, symbols); returns signatures, operands, or `Err(&'static str / String)`.
4. Decides: keyed on type identity values: `realizes_signature` compares `type_identity` of each parameter type with binders named `"$0"`, `"$1"`, ... generated by `format!("${ordinal}")` (`:89`, `:94`), position labels, not user names; `compare_callback_signature_types` compares identity strings (`:915-925`) and region identities, with binders `"$T{position}"` (`:884`). `required_regions.len() != supplied_regions.len()` -> Err (`:949`). `source_call_declaration` decides between requirement and machine by `dispatch || trait_machine_signature(target).is_some()` (`:714`).
5. Cases by shape: `realizes_signature` looks only at the first machine state (`program.machine_states(machine).first()`, `:31`). `realization_static_binders` accepts only Type/Type (not demanding stronger properties) and Const/Const with equal bounds and identity; any other pair returns `None` (`:99-131`). `CallableDeclaration::shape` requires exactly one state for machines (`[state]`, `:193`). `source_call_selected_operand` has four operand sources (requirement application, named receiver, lexical map capture, namespace-only) (`:529-580`). `source_call_record_operand` returns `Ok(None)` unless the application has a record source receiver (`:628-630`). Callback comparison refuses a mode difference with only `ParameterMode` results (`:863-875`).
6. Same job elsewhere: `compare_callable_signature_shapes` (`:744`) and `compare_callback_signature_types` (`:829`) both produce `Vec<CallableShapeMismatch>`; the first takes a caller closure `types_match`, the second computes identity text. `realizes_signature` (`:24`) is a third comparison.
7. Unused: `grep -rEh "compare_callable_signature_shapes"` = 3 matches in 2 files (this file, `pipeline/05.../callable_formations.rs`); `realizes_signature\(` = 4 matches in 3 files (here, `calls_and_control_flow.rs`, `operator_declarations.rs`); `compare_callback_signature_types` = 4 matches in 3 files (here, `type_identity.rs`, pipeline 04 `passes/specialization/callable_binder.rs`).
8. Stated limits: "Neither lowers a type to compare it." (`:3`); "this comparison establishes no contract refinement." (`:23`). Error text at `:951`: "no complete region correspondence".
9. Fail-open: `lexical_conformance_call` returns `Ok(None)` for many "not this shape" cases (`:340`, `:352`, `:354`, `:357`, `:361`) which `source_conformance_call_member` then treats as "no member" (`:405`) and `source_call_selected_operand` turns into an Err ("selected call lost its exact named requirement row", `:526`); so a None does not accept there. `source_call_record_operand` `Ok(None)` at `:620` and `:630` means "not a record call": callers (not read) then proceed without record binding. Class: open for those two sites.
   In `compare_callback_signature_types` the loop `bindings[ordinal].get_or_insert(required)` keeps the first binding seen for each bound region (`:961-965`); a later, different required region for the same bound region is ignored, and the final comparison (`:977-982`) is stated to be the authority (comment `:926-930`). Class: stated.

### 40. `typed-trees/callables/callable_values.rs` (277 lines, read)
1. Purpose: runtime callable contract rows (`RuntimeCallableContract`), machine values, the scalar value type that may be a primitive or a callable.
2. Shape: `RuntimeCallableContract` (`:13`; `signature`, `lexical_owner`, `resolved_source: Option<(u32,u32)>`), `MachineValue` (`:22`), `RuntimeCallBinding` (`:31`), `RuntimeCallExpression` (`:37`), `RuntimeCallableLowering` (`:75`; `Vec<handle>` indexed by source index), `ScalarValueType` (`:119`), `declared_callable_contract` (`:195`), `declared_callable_expression_type_reference` (`:218`).
3. Decides: `ScalarValueType::same_type` compares callables by handle, then by `runtime_callable_contract_identity` equality (`:159-163`); a stale handle is an Err (`:154-158`). `RuntimeCallableLowering::retain` refuses a second contract for the same source index (`:102-104`). `declared_callable_expression_type_reference` returns `Ok(None)` for any expression kind other than `MachineValue` and a `Call` with a `runtime_call` (`:250-251`, `:270`) and refuses a call that retains static call metadata (`:253-262`).
4. Unused: `RuntimeCallableLowering|from_contracts(` appears in 3 files (`pipeline/03.../runtime_callables.rs`, `callable_values.rs`, `storage/arena_tables.rs`); 4 matches.
5. Stated limits: "The declaration coordinate is a lowering memo, not a semantic identity." (`:3`). "Source coordinates accelerate lowering; the pointed-to row remains authoritative." (`:73`).
6. Fail-open: `declared_callable_expression_type_reference` `Ok(None)` (`:250`, `:270`) means "another source query owns this expression" (stated in the doc comment `:215-217`). Class: stated.

---------------------------------------------------------------------------------------------------

## typed-trees `callables/callable_applications.rs`

### 36. `typed-trees/callables/callable_applications.rs` (9,432 lines, read in full)
(Row 36 of the unit file; recorded here because it was read last.)

1. Purpose: builds the typed copies of a callable's declaration for one concrete application. `CallableApplication` (`:8367`) substitutes the type, const, machine and lifetime bindings of one declaration into copied types, expressions, proof facts and signatures. The same file stores the results (`AppliedCallableTable`, `:6698`), forms "closed requirement instances", prepares bodyless-call contracts, finds a named conformance member for a call, and turns static operands to and from type references.
2. Shape (module and line anchors, from `grep` of top-level items and reading):
   - top-level `impl TypedTrees` (`:11-547`): `prepare_callable_scalar_argument` (`:15`), `callable_scalar_value` (`:137`), `static_record_schema` (`:161`), `canonical_callable_selection_value` (`:183`), `canonical_callable_atom` (`:221`), `canonical_static_callable_value` (`:251`), `canonical_aggregate_argument` (`:286`), `canonical_operation_record_value` (`:327`), `prepare_callable_operand_application` (`:429`).
   - `call_targets` (`:549-984`): `AppliedCallTarget` (`:846`), `selected_call_target` (`:856`), `apply_call_target` (`:951`), `retain_selected_call_application` (`:556`), `retain_prepared_call_application` (`:715`).
   - `closed_operands` (`:985-1241`): `OperandClosure` visitor and `require_*_operands` (`:1078-1099`, private `require_application_operand_bindings` `:1120`).
   - `conformance_scopes` (`:1242`), `domain_scopes` (`:1298-1674`: `domain_identity` `:1304`, `retain_domain_application` `:1359`, `retain_domain_arguments` `:1389`, 4 tests).
   - `machine_selections` (`:1675-2025`): `applied_machine_callable_at` (`:1737`), `applied_machine_callable_origin` (`:1809`), `same_registered_machine_operation` (`:1849`), `applied_machine_callables` (`:1863`).
   - `namespaces` (`:2026-2566`): `namespace_symbols` (`:2034`), `named_conformance_call_member` (`:2195`), `named_conformance_capture_key` (`:2307`), `prepare_named_conformance_application` (`:2338`), `named_conformance_member(_at)` (`:2460`, `:2475`).
   - `payloads` (`:2570-3487`): `CallableApplication::type_reference` (`:2576`), `type_payload` (`:2654-3027`), `type_arguments`, `expression` (`:3047`), `apply_expression_rows` (`:3134`), `apply_call_application` (`:3328`), `apply_static_machine_application` (`:3356`), `static_arguments` (`:3425`).
   - `declaration_invocations` (`:3488-4145`): `prepare_bodyless_callable_application` (`:3506`), `prepare_selected_requirement_application` (`:3691`), `bodyless_callable_signature` (`:3918`), `bodyless_application_matches` (`:3963`), `bodyless_declaration` (`:4033`), `bodyless_machine_signature` (`:4058`).
   - `requirement_instances` (`:4147-5242`): `RequirementLocalArgument` (`:4156`), `ClosedRequirementApplication` (`:4214`), `require_closed_requirement_call_application` (`:4458`), `instantiate_requirement_application` (`:4634`).
   - `selected_operands` (`:5243-5932`): `declaration_operand_from_expression` (`:5251`), `static_operand_region_supplier` (`:5290`), `type_argument_names_value` (`:5348`), `static_operand_from_type_reference` (`:5381`), `static_const_literal_from_type_reference` (`:5482`), `static_operand_type_reference_in_scope` (`:5527`), `selected_value_expression` (`:5558`), `selected_argument_reference` (`:5662`).
   - `signatures` (`:5934-6641`): `CallableApplication::signature` (`:6107`), `parameter_kind` (`:6294`), `facts` (`:6403`).
   - `table` (`:6642-7366`): `AppliedCallableTable` (arena plus linked per-invocation row lists, `:6698`), `insert`/`replace`/`free` (`:6825`, `:6831`, `:6845`), `callback_rows`/`declaration_rows`/`machine_rows` (`:6968-6988`).
   - `trait_scopes` (`:7367-7890`): `applied_callable_signature` (`:7387`), `prepare_conformance_requirement_headers` (`:7422`), `prepare_applied_callable_signatures(_reporting_change)` (`:7493`, `:7500`), `materialize_trait_application(_arguments)` (`:7804`, `:7817`).
   - then `AppliedCallableSignature` (`:8242`), `AppliedCallableOwner` (`:8262`; 7 variants Absent, Realization, Trait, RequirementInvocation, RequirementInstance, MachineSelection, StaticCallback), `CallableApplication` fields (`:8367-8401`, 17 fields), private `CallableLifetimeBinding` (`:8403`), `impl CallableApplication` (`:8423-8798`: `runtime_contract` `:8446`, `invocation_region` `:8537`, `scoped_lifetime` `:8579`, `symbol` `:8677`, `nominal_contract` `:8687`, `nominal` `:8750`), and public `TypedTrees` entry points (`:8800-9261`: `apply_refinement_postconditions`, `apply_callable_result_type`, `apply_callable_type(_in_scope)`, `instantiate_invocation_signature` `:8975`, `instantiate_data_member_type` `:9038`, `copy_callable_contract` `:9111`, `apply_copied_callable_interface` `:9133`, `instantiate_nominal_machine_parameter_contracts` `:9222`).
   - Tests: 9 `#[test]` blocks inside submodules plus `invocation_owner_walk_tests` (`:8070`) and `canonical_record_argument_tests` (`:9263`). (Counted by reading, not grepped.)
3. Inputs and outputs: reads `TypedTrees` tables (type references, expressions, static machine applications, symbols, machines, conformance rows, trait signatures, domain definitions, const declarations) and the `CallableApplication` substitution lists (`types`, `static_arguments`, `symbols`, `lifetimes`, `source_binders`). Mutates `TypedTrees`: inserts new type reference nodes, expression copies, generated symbols (`insert_generated_root_from`, `:6383`, `:4810`), applied signature rows, static machine application rows. Returns `Result<_, &'static str>` almost everywhere; a few return `String` or `diagnostics::Diagnostic` (`:1364`).
4. How it decides (keys):
   - By handle or symbol identity: substitution lookups are `.iter().rev().find(|(binder, _)| *binder == symbol)` over `Vec`s (e.g. `:2667-2682`, `:3181-3193`, `:8678-8684`), a linear scan keyed on `SymbolHandle`.
   - By type-identity values: `applied_type_identity` / `applied_index_identity` equality decides whether two operands match (`:1697-1735`, `:1775-1807`, `:4388-4417`, `:4618-4625`).
   - By spelling: (a) `expression` rebinds an unresolved single-member `Name` by comparing its text to `source_binders` names (`:3072-3080`); (b) `static_const_literal_from_type_reference` parses the spelling of a `Named` node with an invalid symbol as an integer, accepting `0b`, `0o`, `0x` prefixes and a leading `-` (`:5499-5514`); (c) `selected_argument_reference` and `prepare_callable_scalar_argument` read a `Named`/path name as a canonical const atom with `CanonicalConstValue::from_atom` (`:87`, `:104`, `:5686-5689`); (d) lifetimes are found by name: `lifetime_parameter_identity(program, owner, argument.as_str())` (`:680-684`, `:2875-2879`, `:8948`) and `binding.parameter == *lifetime` on `Identifier` equality (`:2874`, `:8628`, `:8673`); (e) generated names are built by `format!`: `"@invocation-region-{ordinal}"` (`:8557`) and `"$callable"` (`:8511`).
   - By symbol kind: closed-operand check treats `TypeParameter`, `MachineParameter`, `ConformanceParameter`, `PropositionParameter`, `PropositionMachineParameter`, `Local`, `Parameter` as "open" (`:1178-1187`); `type_argument_names_value` lists `Const`, `Parameter`, `Local`, `Machine`, `MachineParameter`, `ConformanceParameter` as value-like (`:5371-5376`).
5. Cases by shape:
   - `prepare_callable_scalar_argument`: only integer carriers (`landed_landing_fixed`) and `Bool` (`:48`, `:64`, `:132`); else Err.
   - `prepare_callable_operand_application`: "a callable application requires its complete admitted tuple and premises" (`:465`): no member, equal arity, default bounds, no conformance bounds or type equations when applied (`:454-466`); parameter kinds `Type` and `Const`/`Value` only, anything else Err (`:516-520`).
   - `prepare_bodyless_callable_application`: every type parameter must be `Type` with default bounds, no conformance bounds (`:3545-3552`); error text "bodyless call needs its static value, evidence or eligibility judgment".
   - `prepare_selected_requirement_application`: all binders must be `Const` with equal primitive carriers; no lifetime parameters (`:3758-3764`, `:3781-3796`); returns `Ok(())` immediately when the requirement has no type parameters (`:3741-3743`).
   - `prepare_conformance_requirement_headers`: skips conformances with type or lifetime parameters (`:7425-7429`) and requirements whose trait is not `is_boundary`, or that have their own type or lifetime parameters (`:7441-7449`).
   - `named_conformance_call_member`: eligible receiver type telescope is `Type` parameters with default bounds and no lifetimes (`:2229-2237`).
   - `apply_expression_rows` (`:3134`) lists every `ExpressionNode` variant with no wildcard (`:3160-3317`); the comment above says: "Keep this exhaustive over node kinds so a new node cannot silently retain a template's type rows." (`:3132-3133`).
   - `apply_copied_callable_interface` handles statements by listing `LocalData` and `Call` and ignoring all others with `_ => {}` (`:9189-9214`).
   - `AppliedCallableTable` keys rows by invocation handle and purpose `MachineSelection | Requirement(lexical_owner) | StaticCallback` (`:6656-6680`); other owner variants (`Trait`, `RequirementInstance`, `Realization`, `Absent`) are not indexed (`:6678`).
6. Same job elsewhere:
   - The type-copy walk in `callable_copies.rs` (`TypeCopy`, `:19`) and this file's `CallableApplication::type_payload` (`:2654`) both rebuild type reference nodes; the second also handles lifetimes and domains. Suspected overlap; `callable_copies.rs` copies between tables, this file within one table.
   - Three comparers of lists of `StaticMachineBinding`: `same_selected_bindings` (`:1691`), `requirement_application_bindings_match` (`:4344`), and `bodyless_application_matches` (`:3963`, compares identity of arguments).
   - The same `DeclaredStaticOperandIdentityRequest { binders: &[], lifetime_binders: &[], exact_toolchain_sources: &[] }` literal is written out three times (`:3592`, `:3832`, `:4018`).
   - Two types named `CallableLifetimeBinding`: private one at `:8403` and the public one in `callable_lifetimes.rs:9`.
   - `bodyless_declaration` (`:4033`) and `bodyless_machine_signature` (`:4058`) share `bodyless_machine_signature_for` (`:4078`).
7. Unused or one-caller (grep run in the worker-2 checkout, excluding `_recycling/` and this file; `grep -rEn --include=*.rs <name> source-rust tools | grep -v _recycling | grep -v callable_applications.rs | wc -l`):
   `prepare_callable_scalar_argument` 8; `callable_scalar_value` 3; `static_record_schema` 5; `canonical_aggregate_argument` 2; `canonical_operation_record_value` 5 (3 in a test file); `canonical_static_callable_value` 2 (1 in a test file); `prepare_bodyless_callable_application` 1; `prepare_selected_requirement_application` 1; `instantiate_requirement_application` 3; `prepare_conformance_requirement_headers` 1; `retain_live_trait_callable_applications` 1; `remove_trait_declaration_applications` 1; `apply_copied_callable_interface` 2; `instantiate_invocation_signature` 4; `instantiate_data_member_type` 3; `copy_callable_contract` 2; `instantiate_nominal_machine_parameter_contracts` 1; `CallableApplication::for_type_parameters` 1; `named_conformance_capture_key` 6; `prepare_named_conformance_application` 4.
   `canonical_callable_selection_value_in_context`: 0 outside this file (called only from `canonical_operation_record_value_in_context` `:407`; that function's name was not grepped).
   Comment `:180` says "The one canonical callable atom producer." `canonical_callable_selection_value` has 2 matches in `source-rust`/`tools`, both in this file (definition and the call at `:276`); `canonical_callable_atom` is called from two sites in the file (`:204`, `:218`).
8. Stated limits (quotes under 20 words):
   - "recursive nominal callable application is not yet represented by cycle-aware applied telescopes" (`:8779`).
   - "compound nominal callable proposition operands are not yet represented" (`:6504`).
   - "has an unrepresented static type operand" (`:5711`).
   - `closed requirement call has no runtime value argument admission receipt` (`:4543`).
   - "closed requirement call has an unrepresented static application feature" (`:4503`).
9. Fail-open risks:
   - `require_application_operand_bindings` filters `symbol.is_valid()` before the open-operand check (`:1152`), so an operand symbol that is invalid (unresolved) is not reported as open. Later steps (`:1192-1237`) can still refuse it. Class: open.
   - `CallableApplication::symbol` returns the original symbol when no substitution exists (`:8684`), and `lifetime` returns the original name (`:8674`); `scoped_lifetime` returns an invalid `region` (`Default::default()`) when there is no trait application and no binding (`:8636-8642`). An unbound lifetime or symbol therefore flows into the copy unchanged. Class: open (callers in pipelines 03/04 not traced).
   - `apply_copied_callable_interface` statement walk ignores every statement kind except `LocalData` and `Call` (`:9213`). Whether other statement kinds hold type or symbol payload not covered by the expression rows (`rows` from `body.first_expression..`, `:9177-9182`) was not traced. Class: open.
   - `prepare_bodyless_callable_application` returns `Ok(false)` when `require_owned_application_operands` fails (`:3581-3588`); comment: "A declared outer binder is still open. Its generated callers prepare their own exact closed applications." Class: stated.
   - `declared_*`-style optional lookups return `Ok(None)` for non-matching shapes in `named_conformance_call_member` (`:2208-2224`), `selected_call_target` (`:861-865`, `:888-894`, `:911-917`), `source_call_record_operand` (read in section 39). A caller that treats `None` as "no obligation" would skip the selection check. Class: open for the callers outside this unit.
   - `applied_machine_callable_origin`: an invocation that has a `registration_operation` and no `callable_contract_source` is returned as its own origin (`:1821-1823`) with no further check. Class: open.
   - `callable_application_owner_is_live` returns true for `Trait`, `RequirementInstance`, `MachineSelection`, `Absent` (`:8003`) without checking an owner machine. Class: open.
   - `CallableApplication::expression` rebinds only single-member names (`:3072`); multi-member unresolved names pass unchanged. Class: open.
   - `apply_expression_rows` skips a `Name` whose head is a `ConformanceParameter` with `continue` (`:3174-3179`) after only the static-member application step. Class: open.

---------------------------------------------------------------------------------------------------

## Coverage
Read in full, in order (line counts from the unit file):
- all 40 ranges: rows 1 to 40, 21,005 lines (877, 1037, 115, 119, 630, 4, 224, 595, 143, 70, 119, 367, 22, 31, 191, 48, 1373, 198, 81, 48, 23, 14, 163, 168, 543, 370, 20, 58, 62, 220, 62, 4, 15, 103, 218, 9432, 939, 76, 1946, 277).
- Skimmed: none. Not read: none.
- Inside `callable_applications.rs` the read was one pass in chunks (1-1300, 1300-2600, 2600-4100, 4100-5600, 5600-7100, 7100-8600, 8600-9432). The test modules inside that file (`:1420-1673`, `:1879-2024`, `:4848-5241`, `:5742-5931`, `:6540-6639`, `:7012-7365`, `:8070-8239`, `:9263-9432`) were read but are summarized, not itemized.
Grep scope: `source-rust` and `tools` of the worker-2 checkout, `_recycling/` excluded where stated. One early set of greps was run by mistake against the primary checkout (`C:/Development/Omega/source-rust`); all counts in this file come from the later run in `.agents/worktree/worker-2`.

## Five things a maintainer most needs to know about this unit
1. `typed-trees/callables/callable_applications.rs` (9,432 lines) is a logic file, not a row type: it substitutes bindings into types, expressions, facts and signatures, forms closed requirement instances, and prepares bodyless-call contracts. It has about 15 nested modules with their own `impl TypedTrees`/`impl CallableApplication` blocks (`:549`, `:985`, `:1242`, `:1298`, `:1675`, `:2026`, `:2570`, `:3488`, `:4147`, `:5243`, `:5934`, `:6642`, `:7367`).
2. Several judgments rest on spelling, not identity: `source_binders` name match (`:3072-3080`), integer-literal parsing of a `Named` node's text (`:5499-5514`), lifetime lookups by name (`:680`, `:2875`, `:8948`), and canonical const atoms read from path names (`:104`, `:5686`).
3. `callable_lifetimes.rs` (`resolve_callable_lifetime_bindings`) has no caller anywhere in `source-rust`/`tools` (grep: 1 match, its own definition), and defines a pub `CallableLifetimeBinding` while `callable_applications.rs:8403` has a different private type with the same name.
4. The syntax-tree snapshot code (`inspection/`, 2,780 lines) is output only. It prints some enums with Debug (`format!("{:?}")` at `expressions_and_types.rs:476` and `statements.rs:493`), drops invalid scratch handles (`statements.rs:391`), and maps an invalid expression handle to an empty-path `Name` (`expressions_and_types.rs:413`).
5. Tables in `syntax-trees/types/preparation.rs` are linear-scanned arenas keyed by `TypeReferenceHandle` (e.g. `:144-149`, `:181-193`, `:316-321`); `supplied_operand_owner` returns `Ok(invalid)` when no row exists (`:193`), the same value as "owner lost" is not (that case is `Err`, `:189`), so absence and an invalid owner are distinguishable only by the `Result` arm.

## What I am unsure about
- Whether `ItemSnapshot::Platform` (`items.rs:157`) is reachable: `snapshot_item` has no `Item::Platform` arm in the 15 arms read (`:381-729`), so either `Item` has no `Platform` variant or the match is non-exhaustive; the `Item` enum is outside the unit and was not read.
- Callers of `relocate_unit` (`syntax_trees.rs:202`) were not located; the doc comment says a forest "whose rows were rewritten after parsing, is refused" but only the page-count check is visible (`:207`).
- Whether the open fail-open items in section 36 are reachable; I did not trace callers in pipelines 03 to 05 (not read).
- The `PhaseSnapshot` user count was not grepped.
- `visit_operands` call-site counts (sections 7 and 8) were not grepped.
- Test counts for the 12-file test group in `syntax-trees/tests/` were counted by eye, not by command.
