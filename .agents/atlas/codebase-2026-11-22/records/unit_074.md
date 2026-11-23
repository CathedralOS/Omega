# Record: unit_074 (05-source-representations, crate `typed-trees`)

Checkout `4f3c255d0559`. All paths are relative to
`source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/` (written `TT/`). Every file in the unit
was read in full (see Coverage). Pipeline position: this crate is the typed forest that stage 03 builds and stages 04
to 07 read (crate map section "typed-trees"). Files here are inline `pub mod` blocks inside a few large files, so
`file:line` cites the block as well.

Count commands used (Grep tool, count mode, path `source-rust/`): `\.closed_boolean\(`, `names_parameter\(`,
`constant_integer_value\(`, `closed_literal\(`, `project_source_declarations\(`, `retains_input_storage\(`,
`resolve_witness_expressions|resolve_machine_witness_expressions|witness_expression_text`, `proof_only`,
`resolve_machine_witness_subjects|ranking::resolve`, `PreparedSymbolRebinding`, `record_missing_name|missing_name_uses`,
`DeferredRequest::[A-Za-z]+` (excluding deferred_refusals.rs), `OMEGA_STRUCT_TRACE`, `BoundaryTypeBinding`,
`SourceCallCorrespondence|SourceRecordCorrespondence|SourceServiceCorrespondence`,
`begin_declaration_output|AuthoredSelectionExclusions`, `declared_callable_with|...|is_true_crash_route`,
`byte_predicate_call\(|byte_observation_declaration\(|primitive_byte_slice_domain\(`.

---

## 1. TT/expressions/values.rs (6816 lines)

**Purpose.** The typed expression table (`ExpressionTable`) and everything that reads or edits it: integer folding,
display names, deep copy between tables, symbol remapping, conversion from the owned `Expression` tree, value-capture
(match subject) bookkeeping, subtree walkers, and a check that a later table still holds its input rows. Header
comment at `values.rs:1-5`.

**Shape (inline modules, in file order).**
- `captures` (`:7`): `TypedTrees::saved_value_source` (`:10`) finds the expression a saved subject stands for.
- `const_integer` (`:49`): `classify` (`:59`, open/closed integer), `closed_literal` (`:172`, fold `lit op lit`),
  private `fold`/`apply`/`closed_value` (`:99`, `:136`, `:199`).
- `expression` (`:320`), containing: `display` (`:409`), `shape` (`:1018`), `static_members` (`:1044`), `table_copies`
  (`:1256`; `CopyGraph` `:1272`, `copy_graph` `:1432`, `copy_subtree_with_occurrences` `:1922`, `remap_symbols_in_inner`
  `:1652`, `copy_call_operands_from` `:1835`, `replace_with_existing` `:1578`), `tree_conversion` (`:2927`,
  `insert_tree` `:3020`), then the types: `ExpressionTable` (`:3319`, 20 arenas incl. `completed_values`,
  `static_machine_applications`, `build_selection_requests`), `ExpressionNode` (`:4861`, 23 variants including `Block`
  and `SavedSubject`), `TableCallExpression` (`:5083`, many `Option<...Request>` fields), `StaticMachineArgument` (`:5213`),
  `TableNamePath` (`:5278`), owned `NamePath` (`:5349`), `BinaryOperator` (`:5534`), match types
  `TableMatchExpression`/`MatchPattern`/`TableDestructurePattern` (`:5694`, `:5751`, `:5760`),
  `CompletedValueCapture`/`ValueCaptureOwner` (`:5711`, `:5719`), and the owned-tree mirror (`Expression` `:3244`).
- `pattern_components` (`:5842`): `TypedTrees::pattern_component` (`:5856`).
- `subtrees` (`:5961`): `SubtreeEdges`, `subtree_any` (`:6102`), `operand_children` (`:6167`), `direct_blocks` (`:6259`),
  `body_statement_spans` (`:6290`), `statement_blocks` (`:6323`).
- `retains_input_rows` (`:6347`); test modules at `:242`, `:1309`, `:2598`, `:4993`, `:6371`, `:6570`.

**Inputs/outputs.** Reads `TypedTrees` (symbols, statement table, local data) and `arena` tables. Mutates only the
`ExpressionTable` it is called on; copy functions append rows and return `ExpressionCopy { root, correspondence }` (`:1282`).

**How it decides (keys).**
- `classify` name leaf: if the symbol is invalid it parses the SPELLING with `spelling.parse::<i128>()` (`:74-78`);
  otherwise asks the caller closure `is_binder(symbol, spelling)` (`:79`).
- `closed_value` folds only `Add/Subtract/Multiply` and only when `binary.policy == Exact` (`:215-219`); two different
  landing carriers refuse (`:224`); a value outside the carrier refuses (`:230-236`).
- `constant_integer_value` (`:4816`) folds `+ - * / %` with `i64` checked ops and sees through `Borrow` (`:4822`); it does
  NOT look at `binary.policy`, unlike `closed_value`.
- `names_parameter` (`:4485`): accepts when symbol equals OR `name.as_str() == parameter.name.as_str()` (`:4499-4501`),
  so a spelling match alone accepts.
- `authored_length_receiver` (`:4340`): a `MemberSelection::Unresolved` member counts as `length` by authored spelling
  (`:4355-4360`); a selected `CollectionMeasure` does not use spelling (`:4352`). Test at `:5031` pins that.
- `expressions_structurally_equal` (`:4650`): Names compare symbols plus identifier TEXT (`identifier_texts_equal`
  `:5519`); Calls compare `target.as_str()` text (`:4756`) and require no runtime call; Binary compares operator and
  children only, not `policy` (`:4735-4739`); Match destructure patterns compare by whole-struct `==` (`:4680`); any
  variant not listed (Cast, Block, Atomic, ArrayLiteral, StructLiteral, MachineValue, TypeExpression, ZeroValue) returns false (`:4760`).
- `static_arguments_equal_inner` (`:4516`): symbol, path, literal, projection equal; type via caller closure.
- `pattern_component` Query (`:5873`): follows Name->Local initial value, Borrow target, Cast value, and Match arms;
  a cycle returns `None` through a `visited` map (`:5935-5939`); all other node kinds return `Some(Fields)` (`:5914`).
- `retain_build_selection_inputs` / `build_selection_inputs` (`:4006`, `:4068`) refuse stale handles, wrong request,
  wrong phase (strings like "Build input association names another request or generation", `:4049`).
- `retains_input_storage` (`:4576`) destructures `Self` exhaustively (`:4612-4618`) so a new arena is a compile error.

**Cases by shape.**
- `closed_literal` doc (`:147-171`): "Only what needs no machine and no declaration is folded: integer literals under the
  bare `+`, `-` and `*`." Division excluded.
- `atomic_once_operands` (`:6004`): accepts only `AtomicOrderingPlan::CompareExchangeOnce`, exactly two call args
  (`[expected, replacement]` pattern `:6028`), member name equal to `custody.observation.single_attempt_member_name()`
  (`:6033`, string compare), no machine/evidence args.
- `match_pattern_dependencies` returns at most two roots, `[head, condition]` (`:5514`); `match_pattern_operands` one.
- `copy_subtree_with_occurrences` `Block` arm copies only the tail; "a statement copy ... gives each copied block
  statements of its own" (`:1960-1962`). `remap_symbols_in_inner` `Block` arm likewise remaps only the tail (`:1688`).
- `insert_tree` (`:3020`) for `Expression::Match` handles only `Value` and `Wildcard` patterns (`:3026-3031`); an
  `Expression::Call` converts with `static_requirement_dispatch: None`, `machine_arguments: Box::default()`, etc. (`:3118-3125`).

**Same job elsewhere.**
- Integer folding twice in this file: `const_integer::fold/apply` (`:99-145`, i128, folds `/` and `%`) and
  `constant_integer_value` (`:4816`, i64). A third fold: `closed_value` (`:199`).
- Display rendering twice: owned-tree `Expression::display_name` (`:459`) and table `ExpressionNode::write_display_name_with`
  (`:540`); a third renderer for proof labels in `TT/proofs/evidence_and_inspection.rs:1785` (`render_expression`).
- Variant-by-variant walks over `ExpressionNode`: `copy_subtree_with_occurrences` (`:1959`), `remap_symbols_in_inner`
  (`:1686`), `operand_children` (`:6174`), `insert_tree` (`:3021`), snapshot `expression_snapshot` (evidence file `:4849`).
- Copy of static arguments written three ways: `copy_static_argument_members` (`:1064`), `copy_static_machine_application`
  (`:1484`), `snapshot_static_machine_bindings` (`:3704`).

**Unused / one-caller (greps).**
- `StaticMachineArgument::closed_boolean` (`:5235`): 1 call site, `callables/callable_applications.rs` (grep `\.closed_boolean\(`).
- `names_parameter`: 6 hits in 2 files, of which 2 are in values.rs (definition and a test); 4 in `expressions/access_views_and_commitments.rs`.
- `constant_integer_value`: 7 hits in 3 files (values.rs 4 incl. recursion and definition, access_views_and_commitments.rs 1, type_records/type_system.rs 2).
- `closed_literal`: 8 hits in 5 files; real callers outside values.rs: `callables/callable_applications.rs` (2),
  `type_records/type_identity.rs` (1), `pipeline/04.../passes/specialization.rs` (1).
- `retains_input_storage`: 8 hits; 1 production caller `driver/compile_time_invocations/constants.rs`, the rest in values.rs (definition + tests).
- `copy_graph` is documented in the module header as the cross-table copy; callers not counted (suspected several).
- `retain_every_field` (`:1305`) is the only `retain` closure ever passed in this file (every `copy_*` call site passes `&retain_every_field`; read).
  The `RetainField` generic parameter appears on 10 functions; none passes another closure inside the file (read).

**Stated limits (quotes).** "Older expression tables have no exact input association. They must be rebuilt" (`:365`);
`TableBinaryExpression.policy` doc: "A non-Exact raw recipe needs a complete selected operation identity ... refuses"
(`:4916-4918`); "Division is left out" (`:164`).

**Fail-open risks.**
- `names_parameter` accepts on spelling equality alone (`:4501`), even when `parameter.symbol` is valid and `path.symbol`
  differs. A name resolving to a different declaration but spelled like a parameter reaches it. Classification: open
  (callers in `access_views_and_commitments.rs` not read).
- `expressions_structurally_equal` ignores `policy` on Binary (`:4735-4739`): `a + b` and `a +% b` (if both reach it) compare equal. open.
- `pattern_component` returns `Some(Fields)` for any node kind not Name/Borrow/Cast/Match (`:5914`) and for non-Local names (`:5900`).
  Meaning "not a case view". open.
- `saved_value_source` / `saved_subject_source` return `None` (refuse) on any inconsistency; fail-closed. none seen there.
- `closed_boolean` (`:5235`) accepts the bare word `true`/`false` when no symbol, type, application, literal or projection is set. Stated:
  "Both are keywords, so no declaration answers them" (`:5232-5234`). A user declaration named `true` would carry a symbol and be refused.
- `retain_build_selection_inputs` contains a no-op `let _ = self.expressions.get_mut(...)` (`:4028-4030`), apparently to bump arena revision (inferred; no comment).

**Data-oriented ratchet.** `pattern_components` uses `std::collections::BTreeMap<(u32,u32), Option<PatternComponent>>` (`:5853`, `:5870`),
with comment "Full coordinates retain generations without allocating global ID gaps" (`:5868`).

---

## 2. TT/proofs/contract_structure.rs (276 lines)

**Purpose.** Normalizes a callable's declared contract (parameters, result type, `requires`/`ensures`/`crashes` clauses)
into a comparable `DeclaredCallableContract` using callbacks the caller supplies.

**Shape.** `DeclaredContractFact` (`:17`), `DeclaredContractKind` (`:29`), `DeclaredContractClause` (`:37`),
`DeclaredCallableContract` (`:51`), `ContractIdentityContext` (`:58`), `generic_binders` (`:65`), `callable_binders` (`:75`),
`declared_callable_with` (`:90`), `declared_contracts_with` (`:130`), `project_declared_fact_with` (`:208`), `is_true_crash_route` (`:267`).

**Decides.** Binder labels are positional strings `format!("$G{position}")` and `format!("$V{position}")` (`:71`, `:84`).
Crash clauses are bucketed by cause (`:168-175`); a bucket is "unconditional" (fact `None`) when the clause has no facts or
any fact is the literal `true` (`:177-178`, `:267-276`); otherwise facts are sorted and deduped (`:195-196`).
`project_declared_fact_with` returns `None` when any callback returns `None` (`?` at `:221`, `:224`, `:231`, `:248`, `:254`, `:261`).

**Cases by shape.** `Proposition` binder arguments: a const literal uses `literal.value_bignum()` (`:239`), else symbol identity with
optional `.member` (`:241-246`). `ProofFact` has exactly three variants handled (`:216-263`).

**Callers (grep).** The five public names total 8 hits in 2 files: this file (6) and `callables/runtime_callable_identity.rs` (2).

**Stated limits.** Header: "Callers supply the normalization callbacks because a type application must retain its outer substitutions" (`:3-4`).

**Fail-open.** An empty-fact `crashes` clause becomes an unconditional crash clause (`:177`, `:193`); that widens the declared ceiling
rather than narrowing it. Wrong input: a `crashes` contract whose facts were all lost earlier. Classification: open (no comment at the site).

---

## 3. TT/proofs/deferred_refusals.rs (526 lines)

**Purpose.** Rows that say "a pass could not serve this declaration", stored on the declaration; the reader that demands the
declaration raises the row. Header (`:1-20`): refusing at typing would stop every program for a library construct nobody uses.

**Shape.** `DeferredRequest` (`:44`, 24 variants, closed enum; `is_violation` `:138` true for 5: `UndeclaredCallName`, `UndeclaredValueName`,
`CaseCalledAsMachine`, `CallResultDomainMismatch`, `InvalidStaticApplication`), `DeferredRequest::construct` (`:151`, message text per variant),
`DeferredRefusal` (`:228`: `declaration`, `request`, `source_span`, `detail`, private `missing_names`, `unresolved_call_path`),
`TypedTrees::record_deferred_refusal` (`:252`), `record_deferred_refusal_with_detail` (`:263`), `record_missing_name` (`:274`),
`missing_name_uses` (`:293`), private `retain_refusal` (`:307`), `deferred_refusal` (`:342`), `deferred_refusals_of` (`:349`),
`refusal_declaration` (`:365`). Tests `:380-526`.

**Decides.** Rows are keyed by `(declaration.arena_index, declaration.generation, request)` in a sorted `Vec` with binary search (`:315-322`);
the first row recorded for a key wins (`:323`). The declaration is the parent Machine when the owner is a State whose parent is a Machine (`:370-373`).
`deferred_refusal` returns the first violation, else the first row (`:342-346`).

**Cases by shape.** `refusal_declaration` handles only State-under-Machine; everything else is its own declaration (`:376`).

**Same job elsewhere.** `remove_declaration_rows` in `storage/declaration_projection.rs:406-408` drops rows whose owner left.

**Callers (grep).** `DeferredRequest::<Variant>` outside this file: 6 hits in 3 files (`driver/compile_build/waits/tests.rs` 1,
`pipeline/03.../typing_queries.rs` 1, deferred_refusals.rs 4 excluded by glob; the count shown includes 4 here because the glob did not exclude
tests inside the file path match). Counted with `DeferredRequest::[A-Za-z]+`, glob `!**/deferred_refusals.rs`: the tool printed this file anyway, so the
external number is 2 files, 2 hits. `record_missing_name|missing_name_uses`: 2 hits (this file only in the listing), no other callers found.

**Stated limits.** "Most rows are requests no pass serves yet" (`:26`); every non-violation variant's `construct()` text says "no pass ... yet"
(e.g. `:154`, `:165`, `:168`, `:171`).

**Fail-open.** The design makes ABSENCE of a row mean "served" (`:15-20`): a pass that fails to call `record_deferred_refusal` leaves the
declaration accepted. Wrong input: a construct a pass cannot serve and silently skips. Classification: open (callers not traced; the comment states why
rows exist, not why absence is safe). `record_missing_name` sets `unresolved_call_path` only when `request == UndeclaredCallName` and the span matches (`:283`).

---

## 4. TT/proofs/mod.rs (5 lines)

Declares `contract_structure`, `deferred_refusals` (`pub`) and `evidence_and_inspection` (`pub(crate)`). Nothing else.

---

## 5. TT/proofs/evidence_and_inspection.rs (6192 lines)

**Purpose.** Two top-level modules. `evidence` (`:7-2184`): byte-predicate recognition, mathematical definition rows, proof-only data classification,
proposition rows and their normalization to identity labels, ranking-witness lookup. `inspection` (`:2186-6192`): `TypedTreesSnapshot`, a serde JSON
mirror of the whole typed forest for phase inspection.

**Shape.**
- `evidence::byte_predicates` (`:8`): `byte_predicate_call` (`:17`), `byte_observation_declaration` (`:111`), `domain_byte_predicate` (`:130`),
  `byte_slice_domain_predicate` (`:140`), `byte_domain_predicate` (`:151`), `whole_byte_observation_subject` (`:235`),
  `primitive_byte_slice_domain` (`:260`), `literal_bytes` (`:289`).
- `evidence::mathematical` (`:317`): `MathematicalDefinition`, `MathematicalParameter`, `MathematicalType` (Ordinary/Arrow/Application), `MathematicalBody` (Assumption/Definition).
- `evidence::proof_only` (`:373`): `ProofOnlyClassification` (`:390`, `HashMap<u32, ProofOnlyReason>`), `classify` (`:463`), `reaches` (`:588`).
- `evidence::proposition` (`:648`): `PropositionDefinition` (`:749`), `PropositionBody` (Primitive/Witness/Transparent `:778`),
  `normalize_*` family (`try_normalize_proposition_application` `:1520`, `try_normalize_nominal_proposition_application` `:1305`),
  `constructor_values::render` (`:663`), `render_expression` (`:1785`), `normalized_evidence_requirements` (`:920`), `FreeInputMentions` (`:1031`).
- `evidence::ranking` (`:2063`): `RankingExpressionCustody` (`:2071`), `resolve_machine_witness_subjects` (`:2079`), `resolve_witness_expressions` (`:2100`),
  `witness_expression_text` (`:2133`).
- `inspection::snapshot` (`:2187`): `declaration_snapshots`, `machine_snapshots`, `statement_and_expression_snapshots`, `type_snapshots`, then
  `TypedTreesSnapshot::from_typed_trees` (`:5782`), `to_json`/`to_json_pretty` (`:6085`, `:6089`), `TypedTableSnapshot` row counts (`:6132`).

**Inputs/outputs.** Reads `TypedTrees` only (`&TypedTrees`); produces `Option`/`String`/snapshot structs; mutates nothing (read).

**How it decides (keys).**
- `byte_predicate_call` (`:17-107`): accepts a call only if EVERY one of ~14 request fields is default/None (`:37-54`), the selected target is a machine
  holding a `CompilerPrimitive::ByteObservation` (`:111-128`, catalog custody, not spelling), `authored_call_binding` selects and original are equal (`:63-67`),
  and exactly one argument (`:99-102`).
- `domain_byte_predicate_fact` (`:196`): domain with no alias, no index arguments, `Unrestricted` issuance, one `ProofFact::Expression` whose call is a byte predicate over a shared whole-view borrow (`:221-224`).
- `proof_only::classify` (`:463`): data is proof-only if it holds `Int` (builtin symbol identity `:465-467`), is recursive (`reaches` `:533`), contains a proof-only field type, or has a quotient carrier mentioning one; keys are `symbol.arena_index()` only, dropping generation (`:396`, `:400`, `:492`, `:521`).
- `normalize_proposition_application_inner` (`:1560`): refuses cycles via `visiting` (`:1568`); a `PropositionParameter` symbol found in `data_type_parameters` yields a fact-only label (`:1572-1602`); transparent boolean bodies render a label (`:1659-1668`); other mismatches return `Ok(None)`.
- `render_expression` (`:1785`): `Name` renders by symbol substitution, then by FIRST-member spelling in `name_substitutions` (`:1889-1895`), then as a variant constructor when the member symbols form a parent chain ending in a `Variant` (`:1899-1922`), else by joined spelling (`:1923`). `Block` is an error (`:1800`).
- `witness_expression_text` (`:2133`) renders Integer/Name/Member/`a - b` to a string; any other node becomes the literal string `"value"` (`:2157`). `resolve_witness_expressions` (`:2100`) finds expressions whose rendered TEXT equals the witness text (`:2115`, `:2125`), preferring ones rooted at a parameter of the state (`expression_belongs_to_state` `:2161`), falling back to ANY expression anywhere in the table with that text (`:2120-2128`); text `"value"` returns `None` (`:2108`).
- `FreeInputMentions` (`:1031`) decides whether a type mentions a free binder by walking references with scoped binder lists, memoized on `(handle, Vec<SymbolHandle>)` in `std::collections::HashSet` (`:1037-1038`).

**Cases by shape.**
- `byte_carrier_domain_predicate` (`:164`): element type must be `Slice` or (for `SliceOrFixedArray`) `FixedArray`, whose element is a `Named` type whose builtin atom is `U8` (`:173-190`).
- `whole_byte_observation_subject` (`:235`): comment "A shared loan of an entire range observes the same bytes as its backing. Partial ranges remain distinct subjects" (`:232-234`); full range means `start`/`end` invalid and `!end_inclusive` (`:253`).
- `literal_bytes` (`:289`): `String` or `ArrayLiteral` of integers fitting `u8` only.
- `snapshot` module: each snapshot function is a closed match over node kinds (e.g. `expression_snapshot` `:4849`, 23 arms; `type_reference_snapshot` `:5349`, 9 arms; `authored_declaration_selection_intrinsic_name` `:5623`, 33 intrinsic arms).

**Same job elsewhere.**
- `render_expression` (`:1785`) duplicates the display grammar of `values.rs:540` (two renderers of `ExpressionNode`).
- `normalize_nominal_proposition_application_inner` (`:1328`) and `normalize_proposition_application_inner` (`:1560`) are near-copies (same binder/parameter count guards, same Primitive/Witness/Transparent split); the first returns an application identity, the second a formula label.
- `statement_snapshot`/`expression_snapshot` (`:4610`, `:4839`) repeat the per-variant enumeration also in `values.rs`.
- `witness_expression_text` (`:2133`) is a fourth expression-to-text renderer.

**Unused / one-caller (greps).**
- `ranking::resolve_witness_expressions | resolve_machine_witness_expressions | witness_expression_text`: 10 hits, all in this file (grep).
- `resolve_machine_witness_subjects|ranking::resolve`: 2 hits: this file and `pipeline/05.../read/rankings.rs` (1 external call site).
- `proof_only` : 8 hits: 7 here, 1 in `TT/exports.rs`; no caller of `classify` outside this file found (grep `proof_only`). The module is exported but I found no user, so it is either unused or reached only by path through `exports.rs` (open).
- `byte_predicate_call|byte_observation_declaration|primitive_byte_slice_domain`: 23 hits; external: `pipeline/03.../typing_queries.rs` 13, `pipeline/05.../read/byte_observations.rs` 4, `read/predicates.rs` 1.

**Stated limits.** `OMEGA_STRUCT_TRACE` debug printing is read from the process environment inside `classify` (`:501`, `:574`); grep shows 2 hits, both here. "Display helpers accept total callbacks. Discard their complete output when a child cannot be normalized" (`:1763-1764`).

**Fail-open risks.**
- `witness_expression_text` maps unknown node kinds to `"value"` (`:2157`); `resolve_witness_expressions` rejects that exact text (`:2108`), but two different unsupported expressions both render `"value"` and are not distinguished earlier. Reaches it: a ranking witness over an expression other than int/name/member/subtraction. Classification: traced to the guard at `:2108`; fail-closed there.
- Fallback lookup in `resolve_witness_expressions` (`:2120-2128`) takes the first expression ANYWHERE with equal text when none belongs to the state: a witness `n` can resolve to an unrelated local `n` in another machine. Classification: open (string-keyed, text identity).
- `proof_only::classify` keys by `arena_index` without generation (`:396`): a stale-generation symbol with the same slot would match. open.
- `domain_byte_predicate_fact` returns `None` (refuse) on every mismatch; fail-closed.
- `render_expression` falls back to joined path spelling for any unresolved name (`:1923`): two different declarations with the same spelled path produce the same proof label. open.

**Data-oriented ratchet.** std `HashMap<u32, ProofOnlyReason>` (`:378`, `:391`, `:468`, `:519`) and `HashMap<u32, usize>` (`:468`) in `proof_only`; std `HashSet` in `FreeInputMentions` (`:1037-1038`).

---

## 6. TT/storage/arena_tables.rs (129 lines)

**Purpose.** The row storage for the typed forest. `TypedTreeTables` (`:6`, ~50 fields: `RootedArena`s for data, domains, machines (`MachineDeclarationTable`), traits, conformances, contracts, signatures, plus `expression_table`, `statement_table`, `type_reference_table`, `wire_schemas`) and `TypedTreeRoots` (`:80`, 15 `HandleSpan`s naming which rows are roots). `TypedTreeRoots::with_roots` (`:103`) sets five roots and defaults the rest. `begin_declaration_output` (`:73`) finishes the output pages of the expression and statement tables.
**Decides.** Nothing. `partial_build_can_return: bool` and `partial_build_entry` (`:83-85`) are state fields only.
**Callers (grep).** `begin_declaration_output|AuthoredSelectionExclusions`: 30 hits in 9 files (production caller `pipeline/03.../lowerer.rs` 13).

## 7. TT/storage/forest.rs (229 lines)

**Purpose.** Constructor, clone and snapshot for `TypedTrees`. `Clone` (`:6`) clones all fields via a macro list `clone_with_symbols` (`:34-83`, 41 named fields). `with_replaced_symbols` (`:16`) moves a new symbol table in without copying the old one when the `Arc` is unique. `with_roots` (`:134`) builds an empty forest; `core_vocabulary` (`:105`) resolves a specialization to its template first; `checks_authored_selection` (`:115`) is false for excluded occurrences; `ranking_expression_custody_for` (`:185`) is a linear scan by machine symbol; `snapshot` (`:194`).
**Decides.** `core_vocabulary` keys on `specialization.template` identity (`:109-111`).
**Cases by shape.** The `clone_storage!` macro destructures `Self` exhaustively (`:37`) so a new field fails to compile unless listed.
**Fail-open.** `pending_const_range_endpoints` initialised to a std `HashSet` (`:176`); ratchet note only. none seen otherwise.

## 8. TT/storage/frozen.rs (147 lines)

**Purpose.** `FrozenTypedTrees` (`:35`, a `TypedTrees` with sealed declaration indexes plus a lazily computed `PreparedBoundarySignatures` in `OnceLock`),
and `ProductSourceIdentity` (`:8`, a `Weak<FrozenTypedTrees>` used as identity; equality is `Weak::ptr_eq`, `:15`).
**Decides.** `From<TypedTrees>` seals indexes (`:51`); `into_inner` unseals (`:44`). Record `read` runs `validate_recovered_callback_contracts` and refuses with its string (`:99-100`). `PartialEq` compares the inner program only, not the cached census (`:65-69`).
**Stated.** Doc: "This is an association, never a checking verdict or a content identity" (`:6`).

## 9. TT/storage/mod.rs (13 lines)

Declares storage submodules; `continuation_storage_tests` is `#[cfg(test)]` (`:11-12`); re-exports `TypedTreeTables`, `TypedTreeRoots` (`:13`).

## 10. TT/storage/selection_exclusions.rs (80 lines)

**Purpose.** `AuthoredSelectionExclusions` (`:4`): a sorted, deduped `Box<[AuthoredDeclarationSelectionOccurrenceId]>`; `contains` by binary search (`:19`). Keyed on occurrence id. Callers: 7 hits (counted together with `begin_declaration_output`, see above). One test (`:38`).

## 11. TT/storage/source_review.rs (98 lines)

**Purpose.** Records tying emitted lowered output back to typed source: `SourceNode` (`:9`, Expression or Statement handle), `SourceCallCorrespondence` (`:16`), `SourceTypeScope`/`SourceTypeApplication` (`:33`, `:40`), `SourceServiceCorrespondence` (`:50`), `SourceRecordCorrespondence` (`:57`, incl. `storage_identity: String` `:67`), `SourceRecordCleanup`, `SourceFieldCorrespondence`, `SourceLayoutPlanCoordinate`, `SourceLayoutRealizationCoordinate` (`:96`, `target: String`). Data only. Comment: "Typed occurrence identity belongs to the frozen source kept beside a publication. Spans are diagnostic positions, never this association" (`:6-7`).
**Callers (grep).** 28 hits in 13 files, including pipeline 05 `read/source_records.rs` (5), pipeline 07 files, `lowered-source-publication/correspondence.rs` (2).
**Keys of note.** `target: String` (`:96`) and `storage_identity: String` (`:67`) are string identities (display/spelled), not ids.

## 12. TT/storage/symbol_rebinding.rs (188 lines)

**Purpose.** Old-symbol to new-symbol mapping used when copying declarations. `SymbolRebinding` (`:87`) has an `execution` and an `authored` pair list; `PreparedSymbolRebinding` (`:28`) indexes the pairs with a `HandleKeyMap`. `Pairs::lookup` (`:69`): borrowed lists do a linear first-match scan (`:71-73`), indexed lists a map lookup (`:74`); both give first insertion wins (`IndexedPairs::insert` returns the previous and keeps it, `:13-20`). `execution()` and `authored()` pick which list serves both slots (`:104`, `:111`).
**Callers (grep).** `PreparedSymbolRebinding`: 12 hits in 5 files (pipeline 04 `specialization.rs` 5, `callable_applications.rs` 1, `exports.rs` 1, values.rs 2).
**Same job elsewhere.** Two lookup strategies for one meaning (`Borrowed` linear vs `Indexed`), `values.rs:1091-1115` uses `remapped(...)`.
**Fail-open.** An unmapped symbol maps to itself (test `:181-182`: `remapped(external, view) == external`). Wrong input: a copied expression referencing a symbol not in the pair list keeps the ORIGINAL symbol silently. Stated in the test as intended; classification: stated (test comment-free, assertion), consequence untraced.

## 13. TT/storage/continuation_storage_tests.rs (422 lines)

Test-only module (`storage/mod.rs:11-12`). Five tests (`:14`, `:118`, `:249`, `:379`, `:410`) pin that a continuation (`with_replaced_symbols`) shares unchanged rows (pointer equality via `std::ptr::eq`) and detaches only changed rows; they cover deferred refusals, authored service reach rows, applied domain applications, access policy invocations, state parameters, proof facts, contracts, machine states, signatures, applied callable signatures, data type parameters/members/payload fields/applications. No production code.

## 14. TT/storage/declaration_projection.rs (477 lines)

**Purpose.** Narrow a typed program to the declarations of its selected sources, plus derived ones and whatever they reference; drop side-table rows whose owner left. Header: "A declaration of another source leaves the program instead of travelling on unjudged" (`:6-14`).
**Shape.** `SymbolMembership` (`:26`, per-slot generation vector), `project_source_declarations` (`:87`, public), private `project_declarations` (`:194`, fixpoint loop `:206-285`), `remove_machine_rows` (`:352`), `remove_declaration_rows` (`:395`).
**Decides.** A symbol is a "selected root" when not derived and its provenance span is absent or its source is selected (`sources.source_is_selected`, `:124`); derived = in `instances` or has `generated_from`, or any ancestor does (`:106-115`); a derived or span-less symbol is an admitted reference (`:127`). References pull in parents while `retain_reference` holds (`:247-252`). Authored selections written in a departed source and not reached by a kept declaration join the exclusion list (`:139-163`). Retained product applications leave only when owner, supplier and declaration are all removed (`:447-469`, `:465-467`).
**Cases by shape.** Root-table projection lists 12 named root tables in a macro (`:225-236`) plus machines (`:237`); a root table not in that list is not projected (read).
**Callers (grep).** `project_source_declarations(`: 2 hits: `pipeline/03.../lowerer.rs` (1 caller) and the definition.
**Same job elsewhere.** `retain_live_trait_implementation_applications` in `declaration_tables_and_index.rs:2148` removes other rows by the same `owner_removed` ancestor walk (`:2166-2174` vs `:397-405` here): two copies of the ancestor-walk closure.
**Fail-open.** `retain_reference`/`selected_root` treat an INVALID symbol as retained (`:152-153`, comment `:148-149`: "An invalid handle has no provenance, so it counts as both a selected root and an admitted reference"). Stated. `departed_selections` skips selections with an empty span (`span.start != span.end`, `:144`): a selection with an empty span is never excluded. open.

## 15. TT/storage/declaration_tables_and_index.rs (5972 lines)

**Purpose.** Declaration storage tables with cached lookup indexes keyed by symbol, plus query methods on `TypedTrees`. Header (`:1-7`) lists the modules.

**Shape (inline modules).**
- `data_definitions` (`:9`): `DataDefinitionIndex` (`:192`, by symbol and by name), `DataDefinitionTable` (`:233`), `insert` (`:269`, clears index unless a pure append), `seal/unseal` (`:296`, `:300`).
- `declaration_index` (`:306`): `operator_spellings` (`:314`, rows keyed by `OperatorSpelling`), `relationships` (`:424`: `SymbolRows<Position>` `:433` sorted rows + optional dense offsets, `DENSE_OFFSETS_PER_ROW = 4` `:430`; `DataMemberIndex` `:531`), `requirement_applications` (`:741`), `CallTargetIndex` (`:1365`), `IsolationVerdicts` (`:1374`, four std-keyed verdict maps), `DomainCandidateIndex` (`:1401`), `DeclarationIndexes` (`:1407`, 9 `OwnedIndex` caches), `seal_declaration_indexes` (`:1434`), lookups `machine_by_symbol` (`:1501`), `state_position_by_symbol` (`:1539`), `data_type_parameter_by_symbol` (`:1672`), `data_definition_by_symbol` (`:1717`), `domain_position_by_symbol` (`:1763`), `with_call_target_index` (`:1782`), `build_call_target_index` (`:1802`).
- `declaration_tables` (`:1869`): scope/ownership queries (`declaration_scope_parent` `:1908`, `enclosing_source_conformance` `:1939`, `owned_conformance_realization` `:1956`), conformance sites (`closed_conformance_application_sites_matching` `:2054`), `retain_live_trait_implementation_applications` (`:2148`), `lifetime_declaration_owner` (`:2281`), table accessors via `typed_tree_table!` (`:2348`), authored-selection ledger pass-throughs (`:2359-2527`), data application views (`:2529-2593`), column accessors (`:2601-2645`), `proof_fact_source_span` (`:2742`).
- `machine_declarations` (`:3006`): `MachineDeclarationTable` (`:3053`) with root index and `retained_declaration` (`:3113`).
- `machine_parameter_signatures` (`:3136`): batched `MachineParameterSignatureQueries` (`:3142`).
- `spelled_declarations` (`:3560`): `SpelledDeclarationTable<T>` with a fresh `Arc<()>` identity on every `deref_mut` (`:3605-3609`).
- `table_reads` (`:3700`), `trait_and_machine_tables` (`:3762`: `implementation_value_identity` `:3800`, `machine_parameter_signature_in_with_scope_parents` `:4268`, `retained_static_machine_contract` `:4309`, `machine_holding_state` `:4357`, `immutable_local_initializer` `:4410`), `type_parameters` (`:4726`), `type_reference_queries` (`:4941`: `type_application_value_indices` `:4970`, `reference_descriptor_word_count` `:5042`, `static_binding_matches_argument` `:5359`, `same_static_machine_operand` `:5514`, `type_multiplicity` `:5563`).
- Large `#[cfg(test)]` blocks: `:17-189`, `:659-738`, `:802-1341`, `:2901-3003`, `:3200-3557`, `:3648-3697`, `:4436-4723`, `:4789-4938`, `:5092-5237`, `:5660-5971` (about 2,350 of the 5,972 lines).

**Inputs/outputs.** Reads `TypedTrees` tables and symbols; builds caches in `OwnedIndex` cells keyed by arena revision stamps and root rosters (`CallTargetScope` `:1354`, `DataMemberScope` `:523`). Seal step precomputes all indexes (`:1434-1463`).

**How it decides (keys).**
- Machine lookup `machine_position_by_symbol` (`:1479`): FIRST row in the ROOT roster with that symbol (index `.first`). `machine_declaration_for_lexical_scope` (`:1506`): root first, else a retained row ONLY IF all rows with that symbol are equal (`RetainedDeclarationIndex::build` sets `selected = None` when two rows differ, `:3035-3037`).
- `machine_by_own_or_state_symbol` (`:1595`): minimum of direct position and containing-state position (`:1605-1608`).
- `data_type_parameter_by_symbol` returns the LAST row (`:1687`); `first_data_type_parameter_by_symbol` the first (`:1697`): two answers for one symbol; test `:1068` pins "parameter occurrences select before testing binder kind".
- `unique_declared_field_by_symbol` (`:616`) requires exactly one row `[position]` (`:626-628`) and a `Field` symbol kind.
- `domain_position_by_symbol` (`:1763`) first match by symbol.
- `const_declaration_by_symbol`, `conformance_by_symbol` (`:3785`), `proposition_definition_by_symbol` (`:2763`), `operator_definition_by_symbol` (`:2813`), `machine_token_binding_by_symbol` (`:2839`): linear `iter().find` by symbol equality (no index) (read).
- `expression_const_declaration` (`:2442`): requires selection span equal to the expression span (`:2456`), errors if two selected constants differ (`:2467-2468`).
- `implementation_value_identity` (`:3800`): two implementations are the same value when all rows are `Reference` source, argument shapes are equal, and all binders unused; only for closed argument shapes (`:3890`). `Shape::Primitive(String)` compares primitives by NAME (`:3838`, `:3821`).
- `type_multiplicity` (`:5563`): for `Named`/`Generic` falls back to searching `data_definitions` BY NAME when no symbol matches (`:5604-5608`, `:5623-5627`), default `Affine` (`:5610`, `:5629`).
- `reference_descriptor_word_count` (`:5042`): Slice 2 words; FixedArray/Reference/Callable/Unit 1; named builtin (except `Slice`) 1; data with a unique definition or a machine with a root row 1; else `None`.
- `static_binding_matches_argument` (`:5359`) compares by canonical identity strings via `static_const_argument_identity` (`:5303`) and `applied_index_identity`; falls back to `normalized_type_identity_text(reference)` (`:5354`).
- `machine_holding_state` (`:4357`): prefers the machine at the parent's root position; else the first other machine unless the symbol is not allowed to name a stored state (`may_name_stored_state` `:4385`: kind `State` or `Unknown` and parent not a `Trait`).

**Cases by shape.**
- `declaration_type_parameters` (`:4203`): handles symbol kinds Data, TypeAlias, Domain, Trait, Conformance, Machine/State, Operator; `_ => None` (`:4253`).
- `reference_descriptor_word_count` and `type_application_value_indices` (`:4970`) match a fixed list of `TypeReferenceNode` variants and ignore the rest (`_ => {}` `:5028`).
- `densify` only when the offset table is at most 4 times the row count (`:469`).

**Same job elsewhere.**
- Duplicate "first / unique / all positions" query triples over `SymbolRows`: data definitions (`:1710-1760`), machines (`:1479-1504`), states (`:1539-1642`), type parameters (`:1672-1699`), domains (`:1763`), call targets (`:1782`); each has its own scope tuple and cache.
- Linear symbol scans repeat in `const_declaration_by_symbol` (`:2415`), `conformance_by_symbol` (`:3785`), `proposition_definition_by_symbol` (`:2763`), `operator_definition_by_symbol` (`:2813`), `machine_token_binding_by_symbol` (`:2839`), `data_field` (`:2674`), `state_parameter_by_symbol` (`:4400`): none use an index while neighbours do.
- `data_definition_by_name_str` (`:2595`) and `realized_machine_named` (`:4103`) look up by NAME string with a linear scan; `DataDefinitionIndex.by_name` (`:194`) offers the indexed form.
- Ancestor-walk closure for "owner removed": `:2166-2174` here and `declaration_projection.rs:397-405`.

**Unused / one-caller.** Not individually grepped (too many); the `pub` set is large. `data_definition_by_name_str`, `realized_machine_named`, `implementation_value_head`, `application_lifetime_is_relevant` are suspected low-use (not grepped).

**Stated limits.** "A partial field relation cannot prove uniqueness; malformed storage refuses the complete query family" (`:561-562`); `DeclarationIndexes::eq` always returns true (`:1419-1423`, no comment), so two forests differing only in cached indexes compare equal; "Statement and expression storage is absent, so body edits keep this signature-only index" (`:1352-1353`).

**Fail-open risks.**
- `DeclarationIndexes: PartialEq` returns `true` for any pair (`:1419-1423`). Wrong input: two `TypedTrees` whose index caches differ still compare equal on that field; the index is derived, so only a stale cache could matter. Classification: traced (cache keys include revision stamps `:1354`, `:523`), no wrong program reaches it.
- `LifetimeTemplateCandidates::Ambiguous` returns an INVALID owner (`:2337`) rather than picking one; fail-closed.
- `type_multiplicity` default `Affine` when no definition found (`:5610`, `:5629`); the stricter outcome, fail-closed. A same-name data definition found by name (`:5604-5608`) can supply a different declaration's multiplicity: open (name-keyed fallback).
- `retain_live_trait_implementation_applications` keeps an application when `selected.source_scope.scope(self)` is `None` (`:2220-2222`, `return true`). Wrong input: an application whose scope can no longer be rebuilt is KEPT instead of dropped. Classification: open (no comment at the site).
- `retain_live_trait_implementation_applications` keeps a data implementation application when its supplying application's exact conformance application is held by a captured machine whose instance is still a root machine (`:2152-2188`); test `:2906` pins it. stated by test name.
- `closed_conformance_application_sites_matching` treats occurrences absent from `live` as dead (`:2099-2103`): fail-closed relative to a missing live list.
- `static_binding_matches_argument` returns `false` on unmatched kind pairs (`:5503`); fail-closed.
- `domain_position_by_symbol`, `machine_by_symbol` return first match on duplicates; duplicates are not refused except in `unique_*` variants (`:1739`, `:1627`). open.

**Data-oriented ratchet.** `use arena::HandleKeyMap as HashMap;` aliases a handle-keyed map under the name `HashMap` (`:15`, `:1343`); `by_name: HashMap<Box<str>, Vec<u32>>` (`:194`) and `DomainCandidateIndex` keyed by `Box<str>` (`:1402-1403`) and `IsolationVerdicts` keyed by `(TypeReferenceHandle, bool, Vec<...>)` (`:1375-1397`) are string/tuple-keyed. std `HashMap<_, Vec<_>, arena::BuildIndexHasher>` at `:2153`.

## 16. TT/table_macros.rs (86 lines)

Three declarative macros: `typed_tree_table!` (`:1`, generates `push_*` and a root-span accessor returning `arena::RootRows`), `declaration_columns!` (`:21`, per-node span accessors, three arms: `rows[...]`, `&[...]`, `append`), `span_columns!` (`:61`, `reserve_*`/`set_*`/`push_*` over an arena). Used in `declaration_tables_and_index.rs:2348`, `:2601`, `:2632`, `:3771`, `:4002` and `values.rs:4248` (read). Count not grepped.

## 17. TT/type_records/boundary_values.rs (7 lines)

`BoundaryTypeBinding { parameter_symbol, actual }` (`:3-6`). 7 hits in 2 files (grep `BoundaryTypeBinding`): `definitions/service_declarations.rs` 6, this file 1.

---

## Coverage

Read in full (no skimming):
- `expressions/values.rs` lines 1-6816 (6816)
- `proofs/contract_structure.rs` 1-276 (276)
- `proofs/deferred_refusals.rs` 1-526 (526)
- `proofs/evidence_and_inspection.rs` 1-6192 (6192; lines 2186-6192 are serde snapshot types and mapping, read at lower attention but every line viewed)
- `proofs/mod.rs` 1-5 (5)
- `storage/arena_tables.rs` 1-129, `continuation_storage_tests.rs` 1-422, `declaration_projection.rs` 1-477,
  `declaration_tables_and_index.rs` 1-5972, `forest.rs` 1-229, `frozen.rs` 1-147, `mod.rs` 1-13,
  `selection_exclusions.rs` 1-80, `source_review.rs` 1-98, `symbol_rebinding.rs` 1-188
- `table_macros.rs` 1-86, `type_records/boundary_values.rs` 1-7

Total read in full: 21,684 lines (the sum of the unit's `line count` column: 6816+276+526+6192+5+129+422+477+5972+229+147+13+80+98+188+86+7).
Skimmed: none. Not read: none.

## Five things a maintainer most needs to know about this unit

1. `values.rs` is one 6.8k-line file holding about 15 inline modules; the same 23-variant `ExpressionNode` is walked by hand in copy (`:1959`), remap (`:1686`), operand children (`:6174`), tree conversion (`:3021`), display (`:540`), proof-label rendering (evidence `:1785`) and snapshot (evidence `:4849`). Adding a node kind touches all of them; only `retains_input_storage` and `clone_storage!` force it by compile error.
2. Several decisions are keyed on spelling: `names_parameter` accepts a same-spelled name (`values.rs:4501`), `expressions_structurally_equal` compares call/name text (`:4756`, `:5519`), ranking witnesses are resolved by rendered text with a table-wide fallback (evidence `:2115-2128`, `:2157`), `implementation_value_identity` compares primitive shapes by name (`declaration_tables_and_index.rs:3838`), and `type_multiplicity` falls back to a by-name data search (`:5604-5608`).
3. Deferred refusals (`deferred_refusals.rs`) make "no row" mean "served": 24 request kinds, 5 of them violations (`:138-147`), 19 of them "no pass serves this yet". Whether a program is refused depends on a reader demanding the declaration and on every pass having recorded its refusal.
4. `declaration_tables_and_index.rs` has about 2,350 lines of tests and a cache layer (nine `OwnedIndex` caches, sealed by `seal_declaration_indexes` `:1434`); lookups differ in which duplicate they return (first, last, unique, or "equal-or-none" for retained machines `:3035-3037`), so a caller must pick the right one.
5. Projection (`declaration_projection.rs`) removes side-table rows by symbol ancestry and keeps invalid handles (`:152-153`), and keeps trait-implementation applications whose scope cannot be rebuilt (`declaration_tables_and_index.rs:2220-2222`).

## What I am unsure about

- Whether `names_parameter`'s spelling arm (`values.rs:4501`) can mis-accept in practice: I did not read its 4 callers in `expressions/access_views_and_commitments.rs`.
- `proof_only::classify` appears to have no caller outside its file (`proof_only` grep: 7 here, 1 in `exports.rs`); I did not read `exports.rs` to see how it is re-exported or whether a path-qualified call uses another spelling.
- The `DeferredRequest` external-use count: the Grep glob exclusion did not remove deferred_refusals.rs from the listing, so only the clear external sites (`typing_queries.rs` 1, `waits/tests.rs` 1) are firm; other `DeferredRequest` variants may be referenced through a different import path (not grepped by name).
- I did not grep individual callers of most `pub` lookups in `declaration_tables_and_index.rs`; "suspected low-use" items there are guesses from names.
- Counts for `copy_graph` callers and macro users in `table_macros.rs` were not made.
