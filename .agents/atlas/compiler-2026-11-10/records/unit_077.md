# Unit 77 notes

Files covered (all paths under `omega-rust/`, checkout worker-23):
1. psi/compilation/proof/src/obligations.rs (1-4107)
2. psi/compilation/psi-compiler/src/lib.rs (1-570)
3. psi/compilation/psi-compiler/src/source_checking.rs (1-884)
4. psi/compilation/psi-compiler/src/source_compilation.rs (1-1254)
5. psi/compilation/psi-compiler/terminal-production/src/build_bindings.rs (1-983)
6. psi/compilation/psi-compiler/terminal-production/src/custody.rs (1-221)
7. psi/compilation/psi-compiler/terminal-production/src/lib.rs (1-717)
8. psi/compilation/representations/checked-trees/facts/src/contexts_and_places.rs (1-2210)
9. psi/compilation/representations/checked-trees/facts/src/evidence.rs (1-1163)
10. psi/compilation/representations/checked-trees/facts/src/lib.rs (1-1928)
11. psi/compilation/representations/checked-trees/facts/src/predicates.rs (1-866)
12. psi/compilation/representations/checked-trees/src/admissibility_and_borrow.rs (1-2181)
13. psi/compilation/representations/checked-trees/src/contract_and_obligation_facts.rs (1-3802)

(Entries are appended per file below.)

---

## 1. proof/src/obligations.rs

**Purpose.** Walks a typed program (`CheckingInput`) and lists every "bounded value" proof obligation (a value that must fit a constrained type), computes the constraints known about each expression (integer/float ranges, named facts), and gives each obligation a canonical string key (used as a cache fingerprint). The file is a single module tree: `collection` (10), `constraints` (1332), `identity` (1834), `plan` (3105), `ranges` (3655), re-exports at 3872-3886, and a `range_tests` module (3888).

**Shape.**
- `collection::build_proof_plan` (obligations.rs:42): iterates machines -> owned data, states -> parameters, return type, statements. Emits `ProofObligation` variants: BoundedValue for owned data/state params/state return (54, 73, 89), BoundedInitializer for data initial values (61), BoundedStateReturn (100, 1020), BoundedAssignment (115, 324), BoundedCallArgument (130, 809), BoundedInitializer for locals/constructor fields/array elements (147, 863, 898, 938), GuardedTransition for `when` guards (159), BoundedTransitionArgument (170, 547).
- `estimated_proof_obligation_capacity` (196): a pre-sizing estimate only (arena capacity).
- `walk_bounded_type` (243): follows `Reference`, `FixedArray`, `Slice`, `Generic` args; stops at `Constrained` (calls back); ignores `DynamicTrait/Named/ConstExpression/Unit`.
- `ensures_witness_bounds_at` (464): scans prior statements, keeps `(place, bound)` witnesses from callee `ensures` clauses (`<=`/`<` against literal only, 535-543); retires witnesses overlapping any written path (`retire_written_witnesses` 452, uses `validation::frame_paths_overlap`).
- `incoming_state_guard` (1167): unique guard on every transition into a state; returns None if any incoming transition is unguarded or guards differ.
- `expressions_equivalent_for_precondition` (1255): structural expression equality (Name by path member list, Call by target/symbol/args, Member, Binary, literals); anything else false.
- `constraints::expression_constraints` (1363): per-ExpressionNode constraint derivation; `derived_binary_constraints` (1670); `collect_constraints` (1554, from a type reference); `primitive_constraints` (1605) with a `match name.as_str()` over `"u8"|"u16"|"u32"|"u64"` only (1607-1613); `augment_constraints_with_named_facts` (1789).
- `identity::proof_obligation_key` (1894) -> `Option<ProofObligationKey>`; `KEY_SCHEMA = 2` (1858). Builds a string of nested `tag(a,b)` with `escape` (2756). Returns None when `unavailable` cell set (capture problems, 2354-2364, 2386).
- `plan`: `ProofPlan` (3119) with `obligations`, `type_constraints`, `assignment_value_ranges` arenas; `ProofConstraint` (3201: Named, IntegerRange, FloatRange, ArithmeticDomain); `ConstraintBuffer` (3260, 4 inline + overflow vec); obligation structs (3508-3600); `FloatRange` (3612).
- `ranges`: `integer_range_from_constraint_iter` (3706): intersects all IntegerRange constraints, then lifts minimum to 0/1 for named `non_negative`/`positive`; `float_binary_range` (3782) only for Add/Sub/Mul/Div on finite operands; `constrained_type_reference` (3849).

**Inputs/outputs.** Input: `&dyn validation::CheckingInput` (expression/statement/type tables, symbols). Output: `ProofPlan` (arenas of obligations + constraint spans). Mutates only the plan.

**How it decides.**
- Whether an assignment/argument/return produces an obligation is keyed on whether the declared type is `Constrained` after peeling `Reference` (`constrained_type_reference`, 3849-3868). Non-constrained targets produce none (411, 831, 1105).
- Initializers: `collect_receiving_initializer_obligation` returns early unless `program.primitive_type_reference(reference)` is Some (918-920) -> only primitive-carried constrained types get initializer obligations.
- Transition arguments/state returns: guards are only attached when the arguments are call-free (`expression_contains_call_node`, 571-573, 1057); otherwise guard replaced with `Always` and refuted guards dropped (615-624, 1064-1073).
- Case guard narrowing: `case_guard_proven_variant` (679) accepts `receiver == Variant` (path equal via `expressions_equivalent_for_precondition`) possibly under `&&` or `== true`; payload field found by `field.symbol == member_symbol || field.name == *member_name` (724-725): name compare is a fallback even when symbols are valid.
- Identity: symbols by `normalized_hermetic_symbol_identity`, falling back to `display_path` (2320-2323); name paths with unresolved symbols fall back to spelled members (2326-2346); member by symbol else name (2488-2492); `format!("{:?}", ...)` of `node.ordering`, `result_custody`, `form`, `access`, `operational_acknowledgement`, quotient `kind`/`role` are inside the key (2437-2438, 2467, 2503, 2609, 2651, 2659). Statement index is NOT in BoundedAssignment key (test at 2904) but IS in owner keys for StateLocal/ConstructorField/ArrayElement (2132, 2147, 2174).
- Float ranges key by exact `to_bits` (2243-2245).

**Cases by shape.**
- `primitive_constraints` (1607-1613): only unsigned `u8/u16/u32/u64` get a built-in range; signed types and others get none by this path (they may be handled via type constraints elsewhere). Read.
- `float_format` (1489): only F32/F64.
- `collect_witness_conjunct` (535-539): only `LessOrEqual` and `Less`; others `continue`.
- `binary_value_operands` (416): only when value is a top-level `Binary`.
- `collect_array_element_obligations` (938): handles borrow, match arms, array literal; anything else returns (947-975).
- `incoming_state_guard`: gives up (None) on first unguarded incoming transition (1193-1195) or differing guards (1197-1206).
- `table_transition_target_state_and_arguments` (1216): only `TransitionTargetNode::Named`; `Value` targets None.

**Same job elsewhere.** `expressions_equivalent_for_precondition` (1255) is a structural expression equality; `identity::expression_identity` (2348) is a second full-expression walker producing strings. `ConstraintBuffer` range derivation (`derived_binary_constraints`) uses `validation::integer_binary_range` (re-exported 3780) - integer fold lives in `validation`, float fold local (3782). Grep: `expressions_equivalent_for_precondition` appears 11 times, all in this file (Grep count over omega-rust). `declared_integer_range`, `build_proof_plan`, `proof_obligation_key(`, `ensures_witness_bounds_at`, `KEY_SCHEMA`: 37 occurrences total across 5 files (obligations.rs 27, proof/lib.rs 1, derivation_store.rs 3, checker.rs 5, 05_typed-trees-to-checked-trees/checking.rs 1) (Grep count of the five names).

**Unused/one-caller.** Not individually grepped beyond the above combined count; `estimated_proof_obligation_capacity` is called only from `ProofPlan::new` (3128) (read). `callable_parameters` (1325) is a one-line wrapper over `state_parameters(..).iter()` (read; 2 uses at 576 only).

**Stated limits.** None as comments. The test comment "constraint order is retained, not normalized away" (2977).

**Fail-open risks.**
- `ensures_witness_bounds_at` (464-515): when `call_frames` is `None` (CallFrameResolver::new returns Option, 44/122), `may_write_paths` yields None -> `retire_written_witnesses(.., None)` clears all witnesses (453-455) = fail-closed. When `call_write_origins` is None, `continue` (487-491): no witness added = fail-closed. Traced by reading.
- `incoming_state_guard` None -> `state_guard` None -> assignment obligation without incoming guard (384-389); fewer assumed facts = fail-closed direction. Read.
- `collect_bounded_value_obligation` etc. silently skip (`return`) when a type is not constrained (411) - intended (nothing to prove). `expression_type_reference` None at 344 returns with no obligation and no diagnostic: an assignment whose target type cannot be resolved gets no obligation. Status: open (not traced to callers).
- `ConstraintBuffer` for `Match`/`SavedSubject` is empty (1370) = no facts assumed = fail-closed.
- `proof_obligation_key` returns None when capture state is unavailable; callers (checker.rs, derivation_store.rs) not read by me - open.
- `from_node` (3222) drops constraints that don't convert (`Domain` with unknown name -> None, 3229) via `filter_map` at 3195: an unrecognized domain constraint name silently disappears from the obligation's constraint list, so the obligation checks less. Status: open.

(Details above are from full read of 1-4107.)

---

## 2. psi-compiler/src/lib.rs (1-570)

**Purpose.** Facade crate root. Contains three inline modules (`portable_productions` 32, `source_parsing` 235 with inner `batches` 236) plus re-exports (538-570) of `source_checking`, `source_compilation`, and the whole public surface of `psi-terminal-production`.

**Shape.**
- `PortableProductions` (lib.rs:88): `Mutex<Vec<(ProductionKey, Arc<OnceLock<Production>>)>>`. `produce` (103) finds an existing slot by linear scan using `ProductionKey::matches` (77): `Arc::ptr_eq` on the CheckedTrees program, equal entry symbol, equal machine and boundary root lists, equal exact-toolchain `(SourceId, [u8;32])` list, equal `PsiOptimizationSelections`. First caller runs the closure inside `OnceLock::get_or_init` (125).
- `ProductionKey::new` (61): only `TerminalMachineSelection::Symbol` and `::Declarations` are shareable; any other variant (i.e. `Name`) returns None so `produce` runs uncached (62-66, 108-110). Test `named_inspection_is_never_shared` (213).
- `batches::parse` (249): splits loaded sources into pages (one per source when `independent_units`, else up to `parse_worker_count*4`, 256-266), parses pages in parallel with rayon (294-297), lexical errors take priority over parse errors (301-310), checks page directory overflow (269-279). `parse_page` (327): lex then parse each source; after the first parse error stops parsing but keeps lexing (345) so a later lex error still wins.
- `SourceInput::append` (477) / `into_units` (493); `load_source_files` (464); `parse_text` (523) for snippets.

**Inputs/outputs.** Loaded sources in; `SyntaxTrees` pages + `ParsedSources` out. `PortableProductions` stores `Arc<PortableTerminalProgram>` or diagnostics.

**How it decides.** Cache reuse of terminal production is keyed on program pointer identity (`Arc::ptr_eq`, 78) plus the value comparisons listed above (not on contents of the program).

**Cases by shape.** `ProductionKey::new` `_ => return None` (65) refuses all selections except Symbol/Declarations.

**Same job elsewhere.** Linear-scan "find prior input equal to this one" caches also appear in `source_checking.rs:866-874` (`ProductSelections::select`, `Arc::ptr_eq` + roots) and `source_compilation.rs:305-307` (`AuthoredCompletions::find`). Read.

**Unused/one-caller.** Grep (count mode) for `PortableProductions|library_slice_totals|whole_program_reuse_hits|retained_layout_match_hits|retained_count(` across omega-rust: 32 occurrences in 9 files (lib.rs 11, source_checking.rs 6, source_compilation.rs 1, omega/compilation/{compiler/src/lib.rs 2, compiler/tests/retained_layout_match.rs 4, compiler/tests/corpus_runner.rs 1, products/src/terminal_publication.rs 3, products/src/prepared_source.rs 2}, omega/build-system/orchestration/tests/shared_compilation.rs 2). Not split per name.

**Stated limits.** None.

**Fail-open risks.** `ProductionKey::matches` does not compare the `PortableProductionInputs` contents beyond listed fields; two requests with the same Arc'd program, same roots but different `TerminalProductionRequest` fields not in the key (for example `external_declarations`, `provider_inventory`, `publication`, which are fields of `TerminalProductionRequest` in terminal-production/lib.rs:402-411) would share a production. Whether callers can vary those under one key: open (callers are in omega/ crates, not read).

---

## 3. psi-compiler/src/source_checking.rs (1-884)

**Purpose.** Runs Psi 05 checking for a whole program under fresh admission, reusing a prior retained judgment when the preparation input matches, and computes a digest key for a "library slice" (all toolchain and non-root package sources) only for bookkeeping counters.

**Shape.**
- `constant_entries::retain_evaluation_entries` (19): copies a constant's `evaluation_entry` machine into the program if absent; errors if entry invalid (27-35).
- `library_slice::LibrarySliceRequest::key` (100): sorts slice sources (`SliceSource` 216: package digest, package-relative path with `\\`->`/`, scope, stratum, content digest), then hashes completion admission, layout scopes, representation selections in slice, asm authority, wire demands. Used only to count distinct/computed slices (`record_computed` 81, `library_slice_totals` 70, global statics 67-68).
- `retained_check::LayoutEvaluationInput` (411): clone of the whole `TypedTrees` + scopes + recorded authority; `matches` (439) compares scopes, authority, and full program equality (`self.program == *program`, 448).
- `CheckRequest::check` (676): admit representations (678), compute slice key (679), prepare source completion (691), build preparation/checking requests (695-705), look for prior layout-evaluation match (710-717), evaluate fixed layout applications unless matched (733-740), find prior judgment via `judges_preparation_input` (743), then either `reuse_retained_check` (757) or `prepare_typed_trees` + `lower_typed_trees_reusing` (775-806). Returns `CheckedSource` (660).
- `CheckedSource::select_product` (822), `ProductSelections::select` (861, cache keyed `Arc::ptr_eq` input + equal roots, 866-870).

**Inputs/outputs.** `CheckRequest{program, admission, previous, retain, library_slice}`; output `CheckedSource{program Arc<CheckedTrees>, retained, reused}`.

**How it decides.**
- Reuse of prior check: by `judges_preparation_input(&program, &admission, external_roots)` (744-748) (implemented outside this unit), or layout-evaluation match + `judges_preparation_admission` (710-717).
- Library slice membership (207-212): source `origin == Toolchain` OR package identity is Some and != root package. A file with `package_identity == None` and `origin != Toolchain` is not in the slice (test 383-386).
- Digest uses Debug text: `format!("{authority:?}")` (190) and `format!("{demand:?}")` (197, sorted at 199) and `portable_provider_selection` uses `{:?}` of `application_identity` and `composition_mode` (299, 335).

**Cases by shape.** None as lists.

**Same job elsewhere.** `ProductSelections::select` (861) duplicates the pointer-equality "find prior" pattern of `PortableProductions::produce` (lib.rs:116) and `AuthoredCompletions::find` (source_compilation.rs:305).

**Unused/one-caller.** `whole_program_reuse_hits`, `retained_layout_match_hits`, `library_slice_totals`: see combined grep under entry 2 (counts are not per name). `LibrarySliceKey` only feeds `record_computed` (771-773) (read): the key has no consumer other than the counters in this file.

**Stated limits.** None as comments. Test comment-like names: "a predicate without a comparison scope is never captured" (561).

**Fail-open risks.**
- `LayoutEvaluationInput::capture` returns None if authority capture fails (435 `.ok()?`) -> no reuse, fail-closed (traced by test 561-570).
- `check` line 741-751: a prior judgment is accepted if `judges_preparation_input` says so; correctness of that predicate lives in typed_trees_to_checked_trees (not read): open.
- `matches` compares `self.program == *program` after layout evaluation not yet run on the new program (the `evaluated` search at 710 happens before `evaluate_fixed_layout_applications`) - by design as the retained input is the pre-evaluation program (captured at 724 before 734). Read; traced.

---

## 4. psi-compiler/src/source_compilation.rs (1-1254)

**Purpose.** Sequences resolver and typing across Build execution: prepares a `SourceContinuation` (resolved + seeded typed base, deferred authored bodies, pending pre-check evaluations), completes authored bodies into a `CompletedSource`, extends it with generated units, and admits Product policy (providers, representations, native bindings, root grants, declarations, behavior exclusions).

**Shape.**
- `SourceCompilationRequest::prepare` (766): builds `SourceContext` (770), tries `activation::select_retained_source` (777) to reuse a retained frontend, evaluates pre-resolution Build code (795), resolves (fresh or extension) and types, loops completing Build/Toolchain machine bodies (`build_bodies` 1237: bodies whose source file origin is Toolchain or dependency_scope Build) (838-871).
- `activation` mod (16-278): `SourceContext::compatibility` (50) returns `SourceReuseMiss` (125: BuildHelperSelection, LookupContributors, Imports, ProductAliases, DeclarationAuthority). `select_retained_source` (172) picks candidate retained frontends sorted by number of selected activation files descending (204), completes selected bodies, rejects any whose references point at unselected sources (238-252), rejoins authority on pending pre-checks (258-265).
- `authored_completion::AuthoredCompletions` (289): `Vec<Completion>` find by `same_completion_input` (307) and push on retain (336); entries with incomplete usage accounting are not retained (326).
- `SourceContinuation::complete_authored` (1046), `materialize_authored_bodies` (1023), `CompletedSource::extend` (1129), `finish` (1122).
- `product_policy` (381): `admit_product_policy` (429) calls six collectors. `provider_requests` (494): `admit_product_provider_requests` (509) first prepares formation-only operands for declared requests and selected operands for executed events, then collects selections.
- `storage` (627): `Record` impls; `SourceContext` write captures the authority via `RecordedDeclarationAuthority::capture` (643); read gives `authority: None, recorded_authority: Some` (659-660).
- `SourceFrontends` (1191): `Arc<Mutex<Vec<Arc<SourceCompletionInput>>>>` with `retain`, `fork`, `replace_with`, `snapshots`.

**Inputs/outputs.** In: syntax, `SourceMap`, top-level bindings, selection authority, retained frontends. Out: `SourceContinuation` (retained in `frontends`), then `CompletedSource`, `PreparedSource{accounting, typed}`.

**How it decides.**
- Retained frontend is reusable only if: its previous source map's storage is retained by the new map (184), at least one selected activation file exists (193), `compatibility` passes (196), no reference/selection symbol in the typed result originates outside the activation sources (238-252), and authority rejoin succeeds (258-265). Authority comparison is behavioural: for each (requester, owner) package pair it asks old vs new `allows_declaration_selection` (103-118). Build-helper identity compared as a string (57, `Cow<'static,str>` at 32).
- `same_input` (37): requires `recorded_authority.is_none()` on both, `Arc::ptr_eq` of declaration syntax, equal bindings, equal helper identity string, and Arc pointer equality of authority objects (43-47).
- Provider recipe ownership: provider formation author is found by matching the request source to occurrences and requiring a single lexical owner (`authored_origin`), 564-585.
- `has_provider_application` (618): `request.operation == BuildOperation::ProviderSelection` with any operand having an application.

**Cases by shape.** `build_bodies` (1237): "Toolchain or Build-scope" only; authored Product bodies are deferred. `complete_selected` (892) completes only machines whose source is in the selected set (911-920).

**Same job elsewhere.** `AuthoredCompletions::find`/`retain` (300-341) vs `ProductSelections::select` (source_checking.rs:861) vs `PortableProductions::produce` (lib.rs:103): three separate Arc-pointer/equality caches. `complete_authored_bodies` (1086) and `complete_selected` (892) and the `prepare` loops (838-871) are three copies of the "pick pending machines, resolve bodies, type bodies, repeat" loop (read).

**Unused/one-caller.** Grep (count mode) for `describe_schema(|describe_provider(|describe_entry(|rejoin_machine|materialize_authored_bodies|capture_package_native_bindings(|admit_captured_native_bindings` across omega-rust: 25 occurrences in 8 files; 9 in this file (definitions + `product_policy` text). External uses: terminal-interpreter embedding.rs 3, checked-interpreter build_machine.rs 3, validation frozen_program_and_queries.rs 3, omega products package_build.rs 1, compiler product_selections.rs 1, product_queries.rs 3, build_continuation.rs 2.

**Stated limits.** Error text only: "selected retained source still has a deferred authored body" (222).

**Fail-open risks.**
- `compatibility` loop at 74-120: for a file in `activation` with `package_identity == None` it does `continue` (86-88) skipping the product-alias and authority checks for that file. Which bad input: a retained frontend reused where an unpackaged source's declaration authority changed. Status: open (not traced).
- Line 99: `old_has_authority != current.authority.is_some()` only compares presence; per-pair comparison happens only `if let Some(new)` (102): when the new authority is None and old had none, no pair compare needed - fine. Traced by reading.
- `AuthoredCompletions::retain` (326) silently skips retention when usage is incomplete (fail-closed: less caching).

---

## 5. terminal-production/src/build_bindings.rs (1-983)

**Purpose.** After lowering, captures per-call "Build companion" records (bindings, queries, requests, filesystem protocol calls, native locators, native formations) that tie source-level Build calls to the emitted Terminal operations, and keys them to the artifact's custody identity so they can be handed out only for the matching artifact.

**Shape.**
- `native_formations::capture` (18): empty unless a declared request has `BuildOperation::NativeBinding` (23-28); root by `Declarations`/`Symbol`/`Name` (29-42, `Name` resolves only if exactly one machine has that name, 33-40); scans every statement's expressions for calls whose `build_selection` is a Declaration request with NativeBinding (55-63); duplicates must be equal else error (71-78); every declared native request must have an owner (86-95).
- `native_locator` (99): `TerminalBuildNativeLocator{structural_type, widths:[u64;3], shape}`; `TerminalBuildValueShape` {Scalar, Array, Sum}; `capture` (133) and `collect` (221) walk the emitted structural type.
- `TerminalBuildBindingCall` (323); `TerminalBuildBindings` (334) with `*_for(artifact)` getters that return Some only if `self.identity == artifact.custody_identity()` (343-368).
- `capture` (387): for each `lowered.source_call_occurrences` that is an expression call with valid `build_selection`: maps `call.target.as_str()` to `TerminalBuildBindingOperation` by spelling (418-431), finds the emitted boundary (432, `emitted_boundary` 507), looks up the emitted requirement application in `lowered.build_provider_requirements` by `(selection, source_state)` (481-491).
- `capture_declarations` (527): unique declaration requests over all expression-table calls (uses `Vec::contains`, 539).
- `build_protocol::capture` (652): generic over action enum; finds boundary traits flagged by `is_build_protocol(symbols, symbol, protocol)` (660-661) and maps each signature's `name.as_str()` via a caller closure; errors on unknown names (666-671); then matches `source_call_occurrences` by `target_symbol` (696).
- Wrappers: `build_filesystem` (546, protocol "BuildFilesystemOperations", 15 names), `build_queries` (861, "BuildProductOperations", 5 names), `build_requests` (908, "BuildRequestOperations", 7 names).
- `build_provider::BuildProviderProductionRequest::produce` (760): selects exactly one public conformance by package identity and `display_path(...) == declaration_path` (763-771), requires closed boundary implementation (788-795), produces a terminal program rooted at the conformance's realization machines (825-847).

**How it decides.** Keyed on source spelling in several places: `call.target.as_str()` with literals `"await_provider"`, `"prepare"`, `"bind"`, `"bind_description"`, `"bind_native"`, `"select"`, `"select_provider"`, `"exclude"` (418-425); protocol signature names (609-623, 886-890, 962-968); protocol identity by `is_build_protocol(..., "BuildFilesystemOperations")` string names (605, 884, 959); provider declaration chosen by `display_path(symbol,"::") == self.declaration_path` (769-770); machine root by `machine.name.as_str() == name` (36). Native locator: type identity equality of emitted storage text vs `normalized_type_identity_text(reference)` (201-203).

**Cases by shape (refuse the rest).**
- native locator `collect` (243-267): allowed shapes `PrimitiveScalar`, `FixedArray` with literal length, `Sum` whose fields are `Scalar` or `Structural`; everything else "native locator cannot retain references, views or opaque storage" (266); linear claims or quotient carrier refused (238); recursive shape refused (230); erased fields refused (256).
- `bind_native` locator: call must have exactly 3 arguments (148) and exactly one owned structural parameter (184-193).
- Provider production: closed implementation only (`type_parameters`/`lifetime_parameters` empty, 788-795).

**Same job elsewhere.** `build_filesystem::capture` (598), `build_queries::capture` (879), `build_requests::capture` (954) are three copies of "name -> enum via closure, then project tuple", all over `build_protocol::capture` (652). `capture` (387) re-implements a similar source_call_occurrences scan for binding calls (393-503). `emitted_boundary` (507) used by both (432, 707). `Vec<Arc<ProductDeclarationRequest>>` dedupe `capture_declarations` (527) vs scope dedupe at 71-81.

**Unused/one-caller.** `capture_native_formations` single caller terminal-production/lib.rs:440 (read). `ProducedBuildProvider::artifact/path/...` getters not grepped.

**Stated limits.** Error messages only.

**Fail-open risks.**
- `build_protocol::capture` `None => continue` (694), non-call statement `_ => continue` (682, 691), and unmatched target `else { continue }` (696-699): a Build protocol call occurrence that cannot be associated is silently dropped from the companion list. What wrong program gets through: a protocol call whose source site is missing yields no companion record (consumer then sees fewer calls). Status: open.
- `capture` (387) skips occurrences without a source site or valid selection (394-402) - same shape; open.
- `native_formations::capture` `Name` that matches 0 or >1 machines yields root None (33-41) rather than an error: open.

---

## 6. terminal-production/src/custody.rs (1-221)

**Purpose.** Holds the produced Terminal program (`ProducedTerminalProgram`) plus companions and per-application receipts; its `Portable` twin (`PortableTerminalProgram`) is `Arc`-shared. Implements storage `Record` for `ProducedTerminalProgram`.

**Shape.** `ProducedTerminalProgram` (31): `program`, `build_bindings`, `build_requests`, `build_filesystem`, `applications`, `provenance`, `source_records`. `write` (42) refuses if a companion's identity no longer matches the artifact (`companion!` macro 49-55); `read` (68) re-creates the companions with the freshly decoded artifact's view (101-105) and re-verifies each entry receipt via `rejoin_program_entry_receipt` (88) and each composition root by `artifact.semantic_module().machine(root.machine).is_none()` (96).

**How it decides.** Custody is keyed on `TerminalExecutionIdentity` equality (build_bindings.rs:347). On read, applications are validated against the decoded artifact; machine ids by identity.

**Fail-open risks.** On read, an application whose `receipt` is `Err`/`Ok(None)` or whose roots are `Err` are not checked (87, 94 `filter_map ok`): that is the stored negative judgment being preserved - stated by structure, not by comment. None seen otherwise.

**Unused/one-caller.** `into_parts`, `retain_code`, `into_shared_portable` not grepped.

---

## 7. terminal-production/src/lib.rs (1-717)

**Purpose.** Sequences Psi 06 -> 07 -> 08 (checked -> lowered -> optimized -> canonical Terminal artifact) in `TerminalProductionRequest::produce`, captures calling signatures before publication, defines observation stages, and attaches per-application entry receipts and composition roots.

**Shape.**
- `terminal_production::TerminalProductionRequest` (402) fields: `publication`, `checked`, `exact_toolchain_sources`, `external_declarations`, `provider_inventory`, `machine`, `roots`, `optimization_selections`. `produce` (431): captures declarations and native formations (438-441), `lower_machine` (444), calling-signature capture if `provider_inventory` is Some (454-464), `UnoptimizedMachineBehavior::capture` per machine (465-470), optimization (476), captures binding/request/filesystem/query calls on the optimized lowered form (479-486), `finalize_terminal_program` (489), attach each application (495-500).
- `calling_signatures::capture` (46) and `boundary_application` (183): boundary ABI application lookup; for an `ExternalRealization` machine collects closed-requirement ABI applications over its trait conformance edges (200-306) and demands exactly one distinct application (307-313).
- `observation::PsiCompilationStage` (320) with `OBSERVATION_LABELS` array indexed by `self as usize` (333-364); `UnmeasuredProduction` (376).
- `composition_roots::TerminalApplicationCustody` (610) `prepare` (617), `attach` (640); `project_root` (698).

**How it decides.** Policy callable lookup in `calling_signatures::capture` by symbol equality in `machines`/`boundaries` lists (114-134) must be unique (128); policy declaration by symbol in `proof_declarations`, unique (140-152). `boundary_application` matches provider/template symbols and portable application identity text equality (`declaration.declaring_application == outer`, 275).
- `TerminalApplicationCustody::prepare` (622): if neither entry identity nor composition is given the custody is empty and `attach` returns `receipt: Ok(None), roots: []` (650-655).

**Cases by shape.** `boundary_application` (183-314): external satisfier must have exactly one ABI application: `[application]`, `[]` -> Err, `_` -> competing Err (307-313). Declaration filter for implicit ABI: `operands.is_empty()` and every formal has a callback (276-281).

**Same job elsewhere.** The per-stage label table `OBSERVATION_LABELS` (333) is parallel with the `PsiCompilationStage` enum order and indexed by cast, i.e. the two must be kept in the same order by hand (read; inferred).

**Unused/one-caller.** `PsiCompilationObserver` is implemented by `UnmeasuredProduction` here; other implementors not grepped.

**Stated limits.** None.

**Fail-open risks.** `calling_signatures::capture` stores per-instance failures as `Err` values inside `CallingPolicyRequest.instances` (173-177) rather than failing production: a bad policy instance does not abort `produce`; it is carried as data. Consumer behaviour not read: open. `boundary_application` returns `None` (no signature entry) for any machine that is not `ExternalRealization` and has no `abi_for_boundary` (188-198): stated by structure only; open.

---

## 10. facts/src/lib.rs (1-1928)

**Purpose.** Root of the `facts` crate: defines `FactPlan`, the paged-arena store of places, place segments, facts, fact contexts (per program point), scalar/boolean predicates, integer ranges, qualification correspondences, and symbol fact sets; plus its query helpers. Lines 42-1121 are a `#[cfg(test)]` test module (read in full); production code is 1123-1928.

**Shape.**
- `FactPlan` (lib.rs:1135): 17 arenas (contract_member_projections, qualification_issuers, places, place_segments, facts: `FactTable`, scalar_values, boolean_predicates, boolean_predicate_bindings, integer_ranges, qualification_correspondences, domain_definition_facts, data_definition_facts, instantiated_expressions, refs, assigned_refs, contexts: `FactContexts`, symbol_sets). The macro `fact_plan_root_roster!` (31-40) lists 16 roots (everything but `contexts`) and `fact_plan_root_impls!` (1195-1251) generates `Clone`, `clone_from`, `with_capacity`, `share_pages`, `begin_page`, `take_additional_pages`, `adopt_additional_pages` over that list.
- `append_place_from_expression` (1428): Borrow -> target; Name -> root symbol (`head_symbol` else `symbol`), then one `Field` segment per later path member (symbol if valid else looked up by name, 1459-1466); Member -> receiver place + `effective_member_symbol` else name lookup (1473-1481); Indexed -> `FixedIndex` if the index is a constant integer fitting usize else `Index{expression}` (1487-1494); anything else -> `PlaceRoot::Expression`.
- `push_field_place_segment` (1507): inserts a `Case{variant}` segment before a `Field` when the field's parent symbol is a Variant (`payload_variant_for_field`).
- `append_context` (1532): builds the `assignments` index (AssignedValue/AssignedScalarValue/AssignedIntegerBounds facts with a Place), sorted by `root.group_key()` (1560); `assignments_with_root` (1569) binary searches it.
- `places_equal` (1819): equal root and equal segment slices. `places_match` (1827): see below. `first_owned_qualification_positions` (1746), `group_fact_refs_by_place` (1795), `fact_place_bucket_hash` (1720) via `places::PlaceBucketIndex`.
- `append_qualification_correspondence` (1341): linear scan for an equal row before append (1345-1351).
- `domain_implies` (1658): transitive closure over `domain_memberships_for_symbol` facts, with a visited vec; false when either symbol invalid (1673).
- `admitted_qualification_issuers` (1280): for every fact with `AdmittedReceipt` origin, requires a retained issuer record whose source and requirement symbols match (1287-1295), else returns an `Err(&'static str)`.
- `declaration_page_fork_totals` (1260), `note_declaration_page_fork` (1267): two global atomics counting shared vs copied declaration baselines.

**Inputs/outputs.** Reads `TypedTrees` (expression table, symbols, data/machine definitions, proof facts). Mutates only the plan.

**How it decides.**
- Place identity is two-level: `places_equal` is structural over (root handle, segment list) (1819-1825); `places_match` (1827-1868) additionally returns true when the two places have equal canonical LABEL strings, except when either root is `InvocationReferent` or `InvocationOutput` (1836-1844). The label is built by `write_canonical_place_label` (contexts_and_places.rs:1272) using `program.symbols.name(symbol)` (the bare name, contexts_and_places.rs:1337) for roots and fields, `display_name` for expression roots, `display_type_reference` for type roots. So two places whose roots/fields have the same bare name but different symbols produce equal labels. Read. This is the judgment "keyed on source spelling/display name". `places_match` external call sites: grep `\.places_match\(` = 11 occurrences: 6 outside facts (05_typed-trees-to-checked-trees: checks_contracts_calls.rs 1, checks_contracts_prover.rs 4, flow_calls.rs 1), 5 inside facts (4 in test at lib.rs:91-94, 1 in contexts_and_places.rs:1062).
- Member symbol resolution by name: `resolve_place_member_symbol` (contexts_and_places.rs:1543) and `effective_member_symbol` (1208) look fields up by `field.name.as_str() == member_name` (1584, 1594) and machine-owned data by `owned.name.as_str()` (1253).
- Qualification duplicate detection (1746-1793): same evidence, structurally equal place, and either equal FloatPredicate or equal (domain_symbol, semantic_domain) membership. Comment 1741-1745: "Only supported membership and float payloads suppress a new occurrence." Other payload kinds (CarryPermission, CarryOrigin etc.) are never treated as duplicates in this function (traced: `domain_membership()` is None for them, so `is_some_and` false).
- Labels: `boolean_fact_label` (1363) and `proposition_fact_label` (1388) return `display_name` text or the instantiated label; `proves_proposition_label` (contexts_and_places.rs:1007) compares to a required label string.

**Cases by shape.** `append_place_from_expression` `_ => append_expression_place` (1498): any expression not Borrow/Name/Member/Indexed becomes an opaque Expression-rooted place. `FixedIndex` only for constant integer index (1487-1491).

**Same job elsewhere.** Two "does fact place equal this place" notions: `fact_place_equals` (1709, structural, used by `proves_place_domain_membership` contexts_and_places.rs:1034) and `places_match` (label-capable, used by `proves_place_domain_membership_in_program` contexts_and_places.rs:1062). Both exist as separate public methods. Grep `\.proves_place_domain_membership_in_program\(|\.proves_place_domain_membership\(` = 3 occurrences (facts lib.rs 1 [test], flow_transfers.rs 1, checks_contracts_prover.rs 1); the two names cannot be separated by that count. Write-frame path strings (see entry 8) are a third place-identity notion (strings like `$P0.value`), used in obligations.rs (`frame_paths_overlap`).

**Unused/one-caller.** Grep (count mode) of `\.places_match\(|\.fact_place_equals\(|first_owned_qualification_positions|group_fact_refs_by_place|\.facts_for_symbol\(|\.place_label\(|proves_place_domain_membership_in_program|proves_place_domain_membership\(|proves_proposition_label` over omega-rust: 46 occurrences in 12 files (facts lib.rs 13, contexts_and_places.rs 5, validation/domains_and_predicates.rs 1, and 05_typed-trees-to-checked-trees: checks_contracts_calls 3, checks_operators 1, checks_contracts_prover 6, checks_contracts 3, flow_calls 2, flow 1, checks 2, flow_transfers 2, published 7). Counts are not per name.

**Stated limits.** None as "unsupported" comments.

**Fail-open risks.**
- `places_match` name-label equality (1859-1867) returns true for places with identical bare-name labels. What wrong program could pass: two distinct locals/fields with the same spelling (for example a field `value` of two different data types reached through `PlaceRoot::Expression` or `Symbol` roots) compare equal, so a domain-membership fact on one would satisfy a requirement on the other via `proves_place_domain_membership_in_program` (contexts_and_places.rs:1062). Reachability not traced beyond noting 6 external callers: open. (Symbol roots with distinct symbols that share a name are the case; a `Symbol(symbol)` root label is `symbols.name(symbol)` only, no path.)
- `unique ... first_owned_qualification_positions`: duplicate suppression requires full evidence equality so it errs toward keeping facts: traced by reading.
- `domain_implies_inner` returns false for invalid symbols (1673) = fail-closed.
- `append_place_from_expression` with an unresolvable member name yields `SymbolHandle::invalid()` field segment (1466, 1479): the place still exists with an invalid field; whether `places_equal` then equates two different unresolved members: two `Field{symbol: invalid}` segments are equal (derive PartialEq on `PlaceSegment`, contexts_and_places.rs:2064), so `a.x` and `a.y` with both unresolved produce equal segment lists. Status: open (not traced to callers); the test at lib.rs:926-936 asserts an invalid field segment for malformed inputs.

---

## 8. facts/src/contexts_and_places.rs (1-2210)

**Purpose.** Two inline modules: `contexts` (7-1185) holds `ProgramPoint`, `FactContext`, `FactContexts` (an indexed paged arena grouping contexts by program point) and `FactContextView` (query over one context); `places` (1187-2210) holds place roots/segments, symbol-to-type resolution for member lookup, canonical place labels, captured-view segment composition, write frames, and a hash-bucket index.

**Shape.**
- `ProgramPoint` (1086): Global, Definition, Machine, State, Statement, Call, CallRequires, CallEnsures, TransitionArm, Exit (each with symbol handles + statement index / call ordinal / transition target handle). `call_point_triple_pattern!` macro (1177).
- `FactContexts` (773): `contexts: Arena<FactContext>`, shared `baseline: Arc<ContextIndex>` (point -> group ordinal, members), local `groups`, `members`, `extensions`. `append` (912) -> `index_context` (895). `handles_at_point` (940), `group_at_point` (947), `handles_in_group` (956, filters freed handles via `is_valid`, 978). `share_pages` (832) seals local groups into the Arc baseline; `take_additional_pages` (860) / `adopt_additional_pages` (883) for parallel workers.
- `FactContextView` (992): `proves_proposition_label` (1007: string compare), `proves_place_domain_membership` (1019: requires `has_symbol_only_proof` for target and fact domain, `fact_place_equals`, `domain_implies`), `proves_place_domain_membership_in_program` (1040: same but `places_match`).
- `places::resolution` (1188): `payload_variant_for_field` (1196), `effective_member_symbol` (1208), `resolve_member_symbol_from_receiver` (1226), `write_canonical_place_label` (1272), `symbol_type_reference` (1390), `collection_element_type` (1498; bounded loop of 8, 1505), `resolve_place_member_symbol` (1543), `fact_place_type_reference` (1600).
- `view_coordinates` (1646): `compose_captured_view_segments` (1649), `project_captured_view_segments` (1664), `compose_boundary` (1696) composing OpaqueRange/FixedRange/FixedIndex/Index.
- `write_frame` (1769): `NormalizedWriteFrame{completeness: Complete|Opaque, paths: Vec<String>, compatibility_report_fingerprint}`; `complete` (1791) sorts, dedups, and drops any path that has an ancestor path as string prefix followed by `.` or `[` (1796-1802). `opaque()` is the `Default` (1784).
- `PlaceRoot` (1975: Unknown, Symbol, InvocationReferent{parameter,type_reference}, InvocationOutput(identity), Expression, TypeReference), `PlaceSegment` (2065: Field, Case, FixedIndex, FixedRange, OpaqueRange, Index), `Place` (2100), `FactPlace` (2106), `PlaceBucketIndex` (2131: sorted `(hash, position)` vec).

**Inputs/outputs.** Reads `TypedTrees` for symbol kinds/parents, state parameters/locals, data members, machine owned data. Produces labels, segments, groupings.

**How it decides.**
- Type of a symbol (`symbol_type_reference` 1390-1463) is keyed on `SymbolKind` of the symbol and its parent kind chain: Parameter/Local -> via parent state and grandparent machine; for Local also requires `program.symbols.name(symbol) == local.name.as_str()` (1414); Field -> parent Machine/Variant/Data.
- Member lookup by spelling: `resolve_member_symbol_from_receiver` (1226) first tries data-field type of the receiver by member NAME, then type symbol, then machine attached data / owned data by name (1253). `effective_member_symbol` (1208-1223) prefers the name-resolved symbol over the stored `member.member_symbol` (the stored symbol is the fallback at 1219).
- Write-frame ancestor dropping keyed on string prefix + separator char (1796-1799).
- `compose_boundary` (1696): returns `None` for unsupported pairs, and `compose_captured_view_segments` uses `?` on the `Option<Option<..>>`, so an uncomposable adjacent pair aborts the whole composition (returns None) (1653).

**Cases by shape.** `collection_element_type` (1498): only FixedArray/Slice, or a `Named` generic instance whose `generic_instance` origin peels to one; depth capped at 8 (1505). `fact_place_type_reference` (1600): roots Unknown/TypeReference/InvocationReferent/InvocationOutput return None; range segments return None (1634-1639). `symbol_type_reference` only Parameter, Local, and Field with parent Machine, Variant or Data (1398-1462).

**Same job elsewhere.** Place-label building appears here (1272) and `FactPlan::place_label` in lib.rs:1870 wraps it. `data_member_symbol_by_name` (1574) and `resolve_place_member_symbol` (1543) vs `resolve_member_symbol_from_receiver` (1226): two name-resolution paths for member symbols (read both; they differ in which roots they accept). `variant_payload_field_type_reference` in obligations.rs:711 is a third by-name payload-field lookup (suspected same job; read).

**Unused/one-caller.** `FactContexts::free` is used in tests; production use not grepped. `FactContexts::with_capacity` is pub(crate) (821).

**Stated limits.** Test ignore reasons only ("manual ... cost measurement", 563, 684).

**Fail-open risks.**
- `proves_place_domain_membership` / `_in_program` (1019, 1040): return false when `has_symbol_only_proof` is false (fail-closed).
- `FactContexts::handles_in_group` filters by `is_valid` so freed contexts vanish (978). `Handle::invalid()` group returns empty (961-973).
- `resolve_place_member_symbol` returning None leaves the invalid-symbol field (see entry 10 risk): open.
- `compose_boundary` (1700-1741): `(FixedRange, FixedIndex)` with `index >= end-start` returns None (out-of-range index refuses) = fail-closed; `(OpaqueRange, FixedIndex|Index)` composes to an `Index{invalid expression}` - i.e. loses the index value, which is a widening (more places overlap), not an accept.

---

## 9. facts/src/evidence.rs (1-1163)

**Purpose.** Defines `Fact` (place, point, origin, qualification evidence, payload) and its enums, `FactTable` (arena wrapper with a lazily built point index), `FloatPredicate`, `QualificationEvidence` provenance algebra, and small records for domain/data definition facts.

**Shape.**
- `FloatPredicate` (evidence.rs:21; inner module 14-171): `{format, comparison, bound_bits}`; `non_nan` (28), `comparison` (36; only Equal/Greater/GreaterOrEqual/Less/LessOrEqual with a non-NaN bound), `implies` (100), `common_consequence` (80: if one implies the other keep the weaker, else same-format `non_nan`, else None).
- `FactTable` (262): `arena`, `points: OwnedIndex<Points>`, `revision: OnceLock<u64>`; `at_point` (269), `rows_at_point` (281; falls back to a scan when handles do not round trip, 283-296). `DerefMut` clears the index (348-353).
- `FactOrigin` (690, 16 variants), `QualificationEvidence` (737: origin, source_symbol, requirement_symbol, issuer, provenance), `QualificationProvenance` (746: Unknown, Independent, InvocationInput, MixedInputs) with `common` (758), `FactPayload` (886: 26 variants), `QualificationPayloadIdentity` (1088), `QualificationCorrespondence` (1129), `Fact` (1141), `ProofObligationKind` (868: seven kinds mirroring obligations.rs `ProofObligation` variants).
- `FactPayload::match_pattern_comparison` (1040): reconstructs (subject, pattern, matched) from a MatchPattern payload; refuses (None) if the arm handle generation differs from the dispatch arms start (1060) or the pattern is not `MatchPattern::Value` (1070).

**How it decides.** Evidence merge: `common_consequence` (828) requires `same_authority` (equal origin, source symbol, requirement symbol, issuer) else None; provenance combine: equal -> same; Unknown wins over anything (763); Independent yields to the other (764); else MixedInputs (765). `available_alternative` (836) prefers an Independent-provenance proof. `is_independent_receipt` (849) = AdmittedReceipt origin and Independent provenance. `FloatPredicate::implies` (100-131): same format only; a required predicate with no bound (non_nan) is implied by any same-format predicate (105-107).

**Cases by shape.** `FloatPredicate::comparison` accepts five operators only (42-48). `FactPayload::default` is `BooleanExpression(invalid)` (1081).

**Same job elsewhere.** `ProofObligationKind` (868) is a payload-less mirror of `ProofObligation` variants in proof/obligations.rs:3353 (same seven names) (read both). `FactOrigin` vs `ProgramPoint` overlap in granularity (read).

**Unused/one-caller.** Not grepped.

**Stated limits.** None.

**Fail-open risks.** `FloatPredicate::implies` line 105-107: `required.bound()` None means non_nan, returns `true` for any same-format `self`, including a `self` that is itself `non_nan`; that is the intended meaning (a non-NaN predicate implies non-NaN); traced by reading, not a risk. `QualificationProvenance::common` with Unknown returns Unknown (fail-closed, test at 216-232).

---

## 11. facts/src/predicates.rs (1-866)

**Purpose.** Data definitions for canonical boolean predicates, scalar expressions, integer math terms, structural paths, and byte-sequence operands that checking stores in `FactPlan`; plus trapping-operation queries.

**Shape.** `BooleanPredicate` (19) with `role: CurrentObservation | EntryPathCondition | UnknownEntryPathCondition` (84-89) and `bindings` span; `CheckedScalarExpression` (315, 22 variants), `CheckedBooleanExpression` (601, 18 variants), `CheckedIntegerBinaryKind` (551, 31 kinds; `is_trapping` 586 lists seven Trapping* kinds), `CheckedIntegerMathExpression` (162, an arena of terms), `CheckedStorageRoot` (232), `CheckedStructuralPredicatePathSegment` (794), `CheckedRuntimeIndex` (816, manual `Ord` at 838 with `unreachable!` for impossible tag matches at 861), `u64_integer_literal` (394: only U8/U16/U32/U64), `byte_literal_equality_in_extent` (100).

**How it decides.** `contains_trapping_operation_with_certificates` (463, 688, integer-math 198): an IntegerBinary is trapping when `kind.is_trapping()` and not (`use_certificates && trap_free`) (475). Integer-math walk is capped at 4096 terms and returns `true` (assume trapping) when the cap is hit or a handle is invalid (203-207): fail-closed. `byte_literal_equality_in_extent` (100): builds `len == bytes.len()` AND per-byte reads for the first `maximum_length` bytes (121-125): bytes beyond `maximum_length` are not compared (length equality is still total).

**Cases by shape.** `u64_integer_literal` `_ => return None` for non-unsigned primitives (398-404).

**Fail-open risks.** `contains_trapping_operation` returns `false` for `StructuralParameterField`, `StorageRead`, `ByteSequenceEqual`, etc. (509-517, 709-719): stated by enumerated arms; those variants hold no arithmetic. `byte_literal_equality_in_extent` with `maximum_length < bytes.len()`: the predicate then constrains only the first `maximum_length` bytes while asserting full length: the predicate is weaker than "bytes equal", traced by reading; whether it is used as an assumption (accepting) or a requirement is open.

**Unused/one-caller.** Not grepped.

---

## 12. checked-trees/src/admissibility_and_borrow.rs (1-2181)

**Purpose.** Two inline modules. `admissibility` (7-1048): read-only "acceptance" views over `CheckFacts` for a state, statement, call, exit, or operator use, each reporting a seven-dimension `AcceptanceSummary` (borrow, proof, service_reach, suspension, blocking, boundaries, termination) built from evidence counts. `borrow` (1050-2181): plain data records for borrow checking results (incoming references, loans, call argument accesses, reborrow/direct/supplied-output resources, restoration/disposition events) in `BorrowFacts`, plus lookup helpers.

**Shape.**
- `AcceptanceSummary` (summary mod 791-936), `AcceptanceCheck` (check mod 677-754) with verdict `Accepted | Rejected | NotApplicable` and `rejected(..)` constructor (730), `AcceptanceDimension` via macro (755-790), trait `AcceptanceView` (942) with default methods over `summary()`.
- Wrapper structs `StateAcceptance`, `StatementAcceptance`, `CallAcceptance`, `ExitAcceptance`, `OperatorAcceptance`, enum `StateOperationAcceptance` (wrappers mod 978-1030). `CheckedTrees::state_acceptance` (411-426) looks the state up in `facts.flow.control.state(machine, state)`.
- `BorrowFacts` (1666): 27 arenas incl. `incoming_references`, `loans`, `calls`, `direct_loan_resources`, `reborrow_loan_resources`, `supplied_output_resources`, `reborrow_disposition_events`; `state_index: KeyedRows<SymbolHandle, StateBorrowFact>` keyed by state symbol.
- Key enums: `BorrowAccessKind` (1188: Read, Mutable, WriteOnly) with `direct_reborrow_effect` (1210), `BorrowLoanLineage` (1307: DirectRoot, Reborrow{parent_loan}, UnretainedDerived, SuppliedOutput, LifetimeDependency), `IncomingReferenceOrigin` (1078), `BorrowLoanOwnerSegment` (1641).
- Lookups: `output_origin_call` (1857) is a long cross-check that a supplied-output loan matches exactly one call's completed reference loans (checks at 1865-1929); `call_at` (1713) requires exactly one state row and exactly one call with that (statement_index, call_ordinal) (1721, 1729); `storage_loans` (1975) returns None if the loans of one owner/statement are not contiguous (1986-1988); `with_state_borrows` (2009) filters state rows by machine symbol when the state symbol index returns rows of other machines (2020-2035); `unique_state_borrow*` (2039, 2080).

**Inputs/outputs.** Reads `CheckFacts` flow/proof/semantic tables. Produces `AcceptanceSummary` values and borrowed slices; no mutation.

**How it decides.**
- Every `AcceptanceView::summary` implementation in this file calls `AcceptanceSummary::accepted(..)` with evidence counts only: Call (37-50), Exit (202-213), Operator (305-315), State (429-455), Statement (625-635); `StateOperationAcceptance` delegates (247-254). `accepted` builds checks with `AcceptanceCheck::accepted` for six dimensions and `not_applicable` for termination when its count is 0 (873-880). Read. No implementation constructs `rejected` and no verdict is computed from facts: the verdict is accepted whenever the record exists. Grep `AcceptanceCheck::rejected|AcceptanceSummary::with_checks|AcceptanceSummary::accepted|\.rejected_checks\(|\.rejected_check_count\(|\.is_accepted\(\)` over omega-rust: 8 matches, all in this file. Grep of `AcceptanceView|StateAcceptance|StatementAcceptance|CallAcceptance|ExitAcceptance|OperatorAcceptance|StateOperationAcceptance|AcceptanceSummary|AcceptanceDimension` over the whole worktree `*.rs`: 117 matches in 2 files: this file (113) and checked-trees/src/lib.rs (4). `state_acceptance(` over omega-rust: 1 match (its definition at line 412). So the acceptance views have zero users outside this crate.
- `StateOperationAcceptance::statement_index` (283-295) for an Operator whose origin is not `StateStatement` returns 0 (292).
- `BorrowAccessKind::direct_reborrow_effect` (1210-1224): Read child of Read parent -> SharedRelease, Read child of Mutable -> SharedFreeze, Mutable/WriteOnly children of Mutable, WriteOnly child of WriteOnly -> ExclusiveSuspension; Read parent with Mutable/WriteOnly child, WriteOnly parent with Read/Mutable child -> None.
- `handle_span_contains` (2000) compares generation then arena index range.

**Cases by shape.** `direct_reborrow_effect` fixed 3x3 table (above). `IncomingReferenceIdentity::place_root` (1106): Parameter/ParameterReferent -> Symbol root, Invocation/DescriptorLoad -> Expression root, Output -> InvocationOutput root.

**Same job elsewhere.** `unique_state_borrow_by_symbol` (2039) and `unique_state_borrow` (2080) and `first_state_borrow*` (2054, 2069) are four lookups over the same index with different uniqueness rules. The acceptance counts (`borrow_constraint_count` 139, `state_borrow_evidence_count` 340) duplicate per-dimension counting per level (call 37-50, exit 202-213, state 429-455). Write-frame/place identity: `BorrowFacts` stores `facts::PlaceSegment` spans (access_segments 1669), the same segment type as facts crate (entry 8).

**Unused/one-caller.** See grep numbers above: `state_acceptance` 1 match (definition only) => 0 call sites in omega-rust; acceptance wrappers have 0 outside users in the worktree `*.rs`. Not grepped: individual `BorrowFacts` helpers.

**Stated limits.** None.

**Fail-open risks.**
- Acceptance views (above): `is_accepted()` returns true for any existing record regardless of contents, since `summary()` never builds a rejection. What wrong program gets through: none by this module, because nothing outside the crate calls it (grep above); if a future/other consumer relied on `verdict()` to reject, it never would. Status: traced (grep), the module is unreferenced.
- `state_owns_loan` etc. return false/None on invalid handles: fail-closed.
- `StateOperationAcceptance::statement_index` default 0 (292): open, no callers.

---

## 13. checked-trees/src/contract_and_obligation_facts.rs (1-3802)

**Purpose.** Data and small logic for contract-related facts retained in `CheckedTrees`: obligations that can only be discharged once a build target is selected (constant predicates, constant recipes, integer ranges), canonical contract identity/commitment hashing, crash route buckets/sites/capsules, crash predicate expressions, per-entry resource envelope anchors, scalar contract plans, and `MachineContractPlans`.

**Shape.**
- `constant_predicate_obligations::ConstantPredicateObligation` (16): `retain` (49) walks the expression with a stack; returns `None` unless the expression is closed over Integer/Boolean/layout-query Call/const Name/builtin Binary/`!`/non-recast Cast and at least one node needs target closure (112-114); `_ => return None` at 109. `discharge_all` (173) evaluates all with `scalar_predicates::evaluate_nodes`, passing `&|_, _| true` for the operator-builtin check (189), errors for any result other than `Boolean(true)` (191-207). `capture_predicate` (220): iterative capture into an arena, cycle detection by `active` list (241).
- `constant_recipe_obligations::ConstantRecipeObligation` (399) with `ConstantRecipeRequirement` {ExceedsBound, AtLeast, AtMost, EqualRecipe} (391); `discharge_all` (462) re-evaluates every obligation each call (resets `discharged` at 466-468), collects diagnostics, marks all discharged only if none failed (524-531). Two `unreachable!` for EqualRecipe after an early `continue` (498, 507).
- `integer_range_obligations` (536): `IntegerRangeRecipe` (555, steps of `IntegerBoundStep`; ground compositions are evaluated eagerly to `Bounds`, 614-631), `IntegerRangeObligation` (648), `RangeRequirement::{Fixed, AddressModel}` (635), `discharge` (765) evaluates steps with `program.closed_integer_expression_value_in`, compares to limits; AddressModel limits are `[0, 2^bits-1]` from the supplied `IntegerRangeModel` only if `carrier.is_address()` (775-784).
- `contract_plans::contract_identity` (942-1252): `DeclaredContractFact/Kind/Clause/...` (952-1010), `contract_identity` (1029) hashes supply mode (1043-1050 numeric tags 1-6), external binding, sorted+deduped service names, invocation interface, suspension/blocking/crash interface, termination, and canonical declaration; `ContractHasher` feeds both an FNV-1a 64-bit "report" fingerprint and a domain digest commitment (1203-1252). Type identities enter as text via `type_key` (1148, `.expect("checked contract type must retain its owning identity")`).
- `crash_plans` (1380): `CrashRouteBucket::new` (1401: `Truth` alternative collapses the set; empty -> None), `CheckedBodyCrashSummary` (1436), `CrashPlan` (1491) with `with_checked_sites` (1727: returns None on same state+ordinal duplicates, on guard-covering bucket ids that are unknown or have a different cause, or on `PermissionClaimIdentity::Unknown` in frontier, 1740-1754), `with_checked_calls` (1784: None on duplicate location), `with_checked_operators` (1811: None unless each site has valid `selected_operator`, exactly one of `operator_use`/`named_use`, and `invocation` valid iff not named, 1827-1835), `uses_structural_proof_gated_arithmetic` (1616, nested helper fns), `with_trapping_sites` (1548, merges duplicate locations with OR on `may_trap`, 1556-1563).
- `crash_predicates` (1882): `CrashPredicateExpression` (1913, 17 variants incl. `Opaque(String)`, `Name(Vec<String>)`, `Member{member: String}`, `Call{target: String}`, `Parameter(u32)`), `substitute` (2025), `boolean_value` (2211, And/Or/Not of Booleans only), `write_canonical` (2280); `CrashPredicateIdentity` (2418, Arc of canonical bytes + optional expression + optional scalar form); equality/order/hash all by `canonical_bytes` (2501-2528). `CrashRouteBucketId` one-based u32 (2539).
- `crash_sites` (2560): `CrashSiteLocation`, `CrashCallSiteLocation`, `CheckedCrashSite` (2635), `CheckedCrashCallSite` (2717) with `executable_contract_identity` (2831), `CrashContractCapsule` (2729), `crash_frontier_claim_sort_key` (2954).
- `resource_envelopes` (3005): `CheckedResourceAxisAnchor`, `CheckedEntryResourceEnvelope`, `CheckedMachineResourceEnvelopes`, three derivation obligations (3014: TerminalAndTargetStackClosure, TerminalControlAndWorkSchedule, SelectedInstructionMachineStateFootprint), `validate` methods that replay report fingerprints (3197, 3300), `RealizedMachineContractEnvelope` (3422).
- `scalar_contracts` (3439): `ClosedScalarValueContractPlan` etc.
- `MachineContractPlans` (3583): `machines`, `crash_capsules`, `realized_envelopes`, index `machine_index` sorted by `(arena_index, generation)` (3658-3673) with an `indexed` flag; `for_machine` (3604) uses the index if built else a linear scan; `validate_resource_envelopes` (3719). `MachineContractPlan` (3783). `MachineContractCommitment` via `typed_trees::digest_commitment!` (3792).

**Inputs/outputs.** Mostly constructors and validators over symbol handles, fingerprints and commitments; discharge functions read `TypedTrees` and a target name string and mutate the obligation's `discharged*` fields.

**How it decides.**
- Obligation discharge compares exact integers/booleans from `TypedTrees` evaluation to retained requirements: constant predicates must equal `Boolean(true)` (191); recipes by BigInt comparison (493-496); integer ranges by containment (806).
- Resource envelope validation keyed on equality of machine/entry symbol handles, fingerprints and commitments, and rejects zero fingerprint/commitment (3102, 3198, 3301, 3730, 3735).
- Crash predicate identity is keyed on canonical bytes of a tree whose leaves include spelled strings: `Name(Vec<String>)` bytes are the member spellings joined by `.` (2353-2360), `Member` the member string (2361-2366), `Call` the target string (2387-2390), `Opaque` the display string (2396-2400), `Integer`/`Float` the literal text (2339-2352). Two names with the same spelling but different symbols have identical bytes. Read. Grep `CrashPredicateExpression::Opaque|CrashPredicateExpression::Name\(|::Opaque\(` over omega-rust: 54 matches in 9 files (validation/contract_entailment.rs 42, this file 3, terminal-interpreter 6, published_facts_crash_* 3).
- Contract identity text: service names/invocations sorted by string (1059-1071); membership clauses hash `value`/`domain` strings and type text (1176-1187).

**Cases by shape.**
- `ConstantPredicateObligation::retain` accepts only: Integer, Boolean, layout-query Call, const `Name`, builtin `Binary`, `LogicalNot`, non-recast `Cast` (74-109), `_ => return None`.
- `IntegerRangeRecipe::binary` (589): only Add, Subtract, Multiply; else None (594).
- `boolean_value` (2211): only Boolean leaves with `And`, `Or`, `LogicalNot`; anything else returns None (2242).
- `CheckedScalarExpression` proof-gated arithmetic check (1616-1717) handles divide/remainder with non-literal or zero/(-1) divisors and exact shifts; other kinds fall to `_ => false`.
- `crash_frontier_claim_sort_key`: non-`Established` identity -> all zeros key (2966).
- `u64::try_from(..).unwrap_or(u64::MAX)` saturations in sort key (2978 etc.) and in roster fingerprint length (3401).

**Same job elsewhere.** Two hash/fingerprint families: FNV-1a 64-bit "report fingerprint" (`semantic_vocabulary::Fnv1a`, 1204, 3414) and domain-digest "commitment" (1205); `ContractHasher` writes to both at once (1237-1240). `validate` of resource envelopes recomputes via `from_checked_contract` (3322-3333), a replay rather than a stored check. Obligation discharge appears three times with parallel shape (predicate 173, recipe 462, integer range 765) and a "retain/discharge/pending" API each (`is_pending` at 128, 458, 738) (read). Sorting/dedup helpers: `sorted_dedup` (2948) and ad hoc `sort(); dedup()` in `CrashRouteBucket::new` (1408), `CrashPlan::published_ceiling` (1506), `contract_identity` (1059-1071).

**Unused/one-caller.** Grep results (count mode, omega-rust): `validate_resource_envelopes` 3 matches (definition here 1, `05_typed-trees-to-checked-trees/src/published_facts.rs` 2). `CrashContractCapsule::new\(|CheckedCrashCallSite::new\(`: 0 matches, so the zero-commitment constructors at 2743 and 2882 have no qualified-path call sites (their `new_with_commitment` forms at 2758/2899 are what `new` forwards to, passing `MachineContractCommitment::from_digest([0;32])` at 2753 and 2894); a call through an alias or `Self::new` would not match. `ConstantPredicateObligation::retain|ConstantRecipeObligation::retain|IntegerRangeObligation::retain|\.discharge_all\(`: 14 matches in 9 files (build-time-evaluation/const_evaluation.rs 2, validation: arithmetic_domains 1, literals_and_struct_literals 1, contract_entailment 1; 06_checked-trees-to-lowered-psi/attached_unit.rs 1; 05_typed-trees-to-checked-trees: checks_contracts_exits 2, checks_contracts_calls 1, checks_contracts 3, checks_ranges 2); this count mixes names. `contract_plan_by_machine` (3775) not grepped.

**Stated limits.** `unreachable!("handled both-recipe relation above")` (498, 507); error strings only otherwise.

**Fail-open risks.**
- `ConstantPredicateObligation::retain` returns `None` for unsupported shapes (109) and when no node needs closure (112-114): the predicate is then not retained as a pending obligation. Whether the caller evaluates it immediately or drops it: callers in 05_typed-trees-to-checked-trees/checks_contracts*.rs and checks_ranges.rs (not read): open. What wrong program could get through: a constant predicate over an unsupported expression shape that depends on the target would not be re-checked per target.
- `discharge_all` for predicates passes `&|_, _| true` as the builtin check (189), so at discharge time every binary operator node is treated as builtin even though `retain` checked `builtin(current)` (87-89). Traced: `retain` already refused non-builtin binaries, so a retained predicate only contains builtin binaries; reachable only if nodes captured by `capture_predicate` (which uses `scalar_predicates::source_node` for non-Name expressions at 256, not the `builtin` closure) differ from those checked: open.
- `ConstantRecipeObligation::discharge_all` marks all obligations discharged iff the whole batch had no diagnostics (524-531): batch-level, not per-obligation: stated by structure.
- `IntegerRangeObligation::discharge` with `AddressModel` and a non-address carrier or `None` model gives `limits = None`, then errors "selected address model is not closed" only after steps evaluate OK (803-806): fail-closed.
- `CheckedCrashCallSite::executable_contract_identity` (2831-2853): when target machine/state match the call site's own, compares fingerprint and commitment; the zero-commitment constructor (2894) would make a site with commitment 0 fail that comparison against a real target (error), not pass. Traced by reading.
- `CrashPredicateIdentity` equality by canonical bytes only (2503): predicates that differ only in the symbol behind a same-spelled `Name`/`Member`/`Call` compare equal (see above). What wrong program could pass: crash-route guard matching treats two differently-resolved same-named parameters/fields as the same predicate; consumers in validation/contract_entailment.rs (42 matches) not read: open.
- `type_key` `.expect(..)` (1151) panics if a type identity is missing rather than failing closed with a diagnostic.

---

## Coverage

All 13 ranges in unit_077.tsv read in full, in order.

| File | Lines read | Lines not read |
| --- | --- | --- |
| proof/src/obligations.rs | 4107 (1-4107) | 0 |
| psi-compiler/src/lib.rs | 570 | 0 |
| psi-compiler/src/source_checking.rs | 884 | 0 |
| psi-compiler/src/source_compilation.rs | 1254 | 0 |
| terminal-production/src/build_bindings.rs | 983 | 0 |
| terminal-production/src/custody.rs | 221 | 0 |
| terminal-production/src/lib.rs | 717 | 0 |
| facts/src/contexts_and_places.rs | 2210 | 0 |
| facts/src/evidence.rs | 1163 | 0 |
| facts/src/lib.rs | 1928 | 0 |
| facts/src/predicates.rs | 866 | 0 |
| checked-trees/src/admissibility_and_borrow.rs | 2181 | 0 |
| checked-trees/src/contract_and_obligation_facts.rs | 3802 | 0 |
| Total | 20886 | 0 |

Large test-only regions were read but described only briefly: obligations.rs 2778-3103 and 3888-4107; facts/lib.rs 42-1121; contexts_and_places.rs 18-551 and 553-763 (test and measurement code); evidence.rs 134-170, 172-247, 395-645.

## Five things a maintainer most needs to know about this unit

1. Place identity has three independent notions and one of them is spelling-based: structural (`FactPlan::places_equal`, facts lib.rs:1819), label-based (`places_match`, lib.rs:1827, labels use bare `symbols.name`, contexts_and_places.rs:1337) with 6 callers in 05_typed-trees-to-checked-trees, and write-frame path strings compared by prefix (contexts_and_places.rs:1796). Member symbols are resolved by field NAME in several places (contexts_and_places.rs:1226, 1543, 1574; obligations.rs:724-725).
2. The `admissibility` module in admissibility_and_borrow.rs (7-1048) always returns "accepted": every `summary()` calls `AcceptanceSummary::accepted` (lines 38, 203, 306, 438, 626), and across the worktree its types have no users outside the checked-trees crate (117 matches in 2 files, 4 of them in lib.rs); `CheckedTrees::state_acceptance` has no callers in omega-rust.
3. Obligations keys (`proof_obligation_key`, obligations.rs:1894) are strings built from symbols' hermetic identity (falls back to display path, 2320-2323) and include `format!("{:?}", ..)` of several enums (2437, 2438, 2467, 2503, 2609, 2651); `BoundedAssignment` keys drop `statement_index` but owner keys keep it. `ProofConstraint::from_node` silently drops constraints that do not convert (obligations.rs:3195, 3229).
4. Crash predicate identity (contract_and_obligation_facts.rs:2418-2528) is canonical bytes where names, member names, call targets and `Opaque` display text are spelled strings (2353-2400), so symbol-distinct same-spelled names compare equal; `CrashContractCapsule::new` and `CheckedCrashCallSite::new` (zero-commitment forms) have 0 qualified-path callers.
5. `psi-compiler` has three separate "find a prior equal input" caches built on `Arc::ptr_eq` plus field equality (`PortableProductions` lib.rs:116, `ProductSelections` source_checking.rs:866, `AuthoredCompletions` source_compilation.rs:305), and three copies of the "complete pending machine bodies" loop (source_compilation.rs:838, 892, 1086). Terminal production companions are keyed to the artifact by `custody_identity()` equality, and Build call companions are chosen by spelled operation names (`"bind_native"` etc., build_bindings.rs:418-425, 609-623, 886-890, 962-968).

## What I am unsure about

- Whether `places_match`'s name-label equality can be reached with two different symbols of the same spelling (the 6 external callers in 05_typed-trees-to-checked-trees were not read).
- Callers of `proof_obligation_key` returning `None` (checker.rs, derivation_store.rs not read).
- Who consumes the `Err` instances stored in `CallingPolicyRequest` (terminal-production/lib.rs:173-177), and whether `TerminalProductionRequest` fields outside `ProductionKey` (lib.rs 61-85) can vary for one key.
- Whether callers of `ConstantPredicateObligation::retain` treat `None` as "evaluate now" or "ignore" (callers not read).
- Whether any consumer of `CrashPredicateIdentity` equality depends on symbol identity behind same-spelled names (validation/contract_entailment.rs not read).
- Counts given as combined greps (several names per pattern) cannot be split per name without re-running; I did not re-run them split.
- Line numbers cited inside the large test modules were taken from the read; all production-code line numbers were checked against the Read output.

