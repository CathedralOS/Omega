# Front half: source text to sealed portable program — boundary facts

Checkout `.agents/worktree/worker-23` (HEAD `a336a9f17c0`; atlas records cite `457adf4a11f`).
Read-only. Nothing built or run.

Marks: **[opened]** = I read the code at that line in this checkout. **[record]** = carried from an
atlas digest or record (records are right about 82% of tested claims; "unused"/"same job" least
reliable). "Suspected", "not traced", "inferred" are kept as the record wrote them.

Paths: `PC/` = `omega-rust/psi/compilation/psi-compiler/src/`, `TPR/` = `.../psi-compiler/terminal-production/src/`,
`S0N/` = `.../pipeline/0N_*/src/`, `BTE/` = `.../build-time-evaluation/src/`, `CODEC/` =
`.../representations/terminal-psi/codec/src/`, `TP/` = `.../representations/terminal-psi/src/`,
`LP/` = `.../representations/lowered-psi/src/`, `TT/` = `.../representations/typed-trees/src/`,
`PRF/` = `.../proof/src/`, `ADM/` = `.../proof/admission/src/`.

## The production driver (where the boundaries actually are)

Production does not call the stage crates' own `parse_sources` / `lex_sources` entries (those are
test-only, area_07 "Unused" [record, digest checked by grep]). The real sequence is in the Psi facade:

1. Lex + parse per source page: `PC/lib.rs:335-346` calls `Lexer::new(..).tokenize_reusing` then
   `tokens_to_syntax_trees::parse(syntax, source_id, &tokens)` [opened].
2. Build-time evaluation on syntax, then resolution, then typing: `PC/source_compilation.rs:795-856`
   (`evaluate_pre_resolution` → `resolve_deferred` → `lower_symbol_resolved_trees_to_seeded_base`),
   then a body-completion loop `:858-871` and `complete_authored` `:1046-1083` which runs
   `pre_check.evaluate_or_defer_recorded(typing.typed_mut())` `:1069` [opened].
3. Check: `PC/source_checking.rs:676-818` (`CheckRequest::check`): `prepare_source_completion` `:691`,
   `evaluate_fixed_layout_applications` `:734`, whole-program reuse match `:741-769`, else
   `prepare_typed_trees` (Psi 04) `:776` then `lower_typed_trees_reusing` (Psi 05) `:798` [opened].
4. Terminal production: `TPR/lib.rs:431-520`: `lower_machine` (06) `:444`, `run_psi_optimization`
   (07) `:476`, `finalize_terminal_program` (08) `:489` [opened].

---

## B0. Psi 00 tokens → Psi 01 syntax trees

1. **Handed over.** `TokenStream<'source>` from `Lexer::tokenize_reusing` (`S00/lexer.rs:382-396`,
   returns `Ok(tokens)` `:389`) [opened]; consumed by `parse(syntax_trees, source_id, tokens)`
   (`S01/parser.rs:227-231`) [opened]. Tokens carry kind + span; string literals are stored decoded;
   whitespace and comment tokens are included [record, area_07:29].
2. **Trusted / re-checked.** The parser trusts token kinds and spans. It re-checks one thing the lexer
   only flagged: malformed numbers (`0x`, `1e`) lex successfully with flags and are rejected in stage 01
   at `S01/tokens_and_storage.rs:250` and `:290` [record; digest writer checked].
3. **Check on the form.** None at runtime. Test-only: `PC/lib.rs:400-454` compares the fused path's
   errors and page snapshots against `lex_sources` + `parse_source_pages_from` [opened].
4. **Independence.** The test compares two drivers over the same lexer and parser functions — a call
   into the producer's own code, not an independent derivation [opened, `PC/lib.rs:407, 421`].
5. **Identity.** Token kind enum + byte spans; keywords by exact spelling, which is inherent to lexing
   (`tokens/token_kind.rs:83`) [record].
6. **Lost.** Only the first lexical error per source is reported (`S00/lex_sources.rs:22-25`) and the
   fused facade stops a page at the first lexical error (`PC/lib.rs:339-343`, `:301-305`) [opened for
   the facade; record for S00].

## B1. Psi 01 syntax trees → Psi 02 symbol-resolved trees

1. **Handed over.** `SyntaxTrees` pages + `ParsedRootItems` (`S01/parser.rs:227-256`) [opened] —
   gathered as `ParsedSources` (`PC/lib.rs:318-324`) [opened]; consumed via
   `resolve_deferred(ResolutionRequest{syntax, sources, top_level_bindings, retained_base})`
   (`S02/resolution.rs:2091-2125`, returns `DeferredResolvedFrontier`) [opened], called at
   `PC/source_compilation.rs:828-835` [opened]. Before that, build-time evaluation has already
   rewritten the syntax (B-BTE1).
2. **Trusted / re-checked.** Stage 02 trusts the tree shape; it runs its own "preparation" passes on
   syntax with a throwaway symbol table (`ConstantSelection`, `S02/preparation.rs:57`) and loops a
   provisional resolver until no new root items appear (`S02/resolution.rs:2097-2121`) [opened].
   Parser rewrites done in stage 01 (`move x` → `x`, compound assignment, hoisting, `asm` expansion)
   reach stage 02 already applied and are not re-checked [record, area_07:31].
3. **Check on the form.** No verifier of syntax trees found. Syntax trees have a JSON snapshot used by
   tests (`ST/inspection.rs:2668`) [record].
4. **Independence.** n/a (no check).
5. **Identity.** Names are `Identifier` text; `Identifier` equality is text only (`ST/lib.rs:53`;
   test comment "name equality retains its existing spelling-only contract" `ST/lib.rs:1498`)
   [record]. Item/expression handles are arena handles with page generations
   (`PC/lib.rs:284`, `:452-453`) [opened].
6. **Lost.** `move` keyword leaves no node (`S01/expressions.rs:668-669, 737`; pinned by a test)
   [record, digest checked]. A Boolean-literal arm set covering 2^n combinations gets its last guard
   replaced with `Always` (`S01/bodies_and_transitions.rs:1068-1107`) [record, digest checked].
   `Capability` and `Package` items produce nothing in stage 02 lowering (`S02/lowering.rs:2057`);
   an `a = b` where either bare name spells an evidence term becomes a forwarding row and the statement
   is dropped (`S02/lowering_statement.rs:1113-1140, 1537-1541`) [record, digest checked].
   `ST/lib.rs:1789-1811`: `constant_extent_probes` not carried by `extend_from`/`adopt_page`; the one
   production caller re-copies them [record, "traced"].

## B-BTE1. Build-time evaluation → syntax trees (before resolution)

1. **Handed over.** `evaluate_pre_resolution(BuildTimeEvaluationRequest{syntax_trees, source_context})`
   (`BTE/lib.rs:201-215`) returns `PreResolutionEvaluation`, split into rewritten syntax + a
   `PreCheckEvaluation` (`PC/source_compilation.rs:795-808`) [opened]. It runs alias normalisation,
   const initializers, const generic expressions and qualification proving on syntax
   (`BTE/lib.rs:208-215`) [opened]. It runs stages 02–04 on private probe programs [record,
   area_07:20].
2. **Trusted / re-checked.** Stage 02 receives the substituted syntax as ordinary authored syntax.
   The evaluator itself compares what a probe used against the author's selection ("custody")
   [record, area_07:23]; the custody comparison is skipped when no expected origins are supplied,
   which data-argument probes never get (`BTE/const_generic_expressions.rs:6684-6691`, `:5952-5955`)
   [record, "traced"].
3. **Check.** Receipt charge checks inside the producer (`BTE/evaluation_accounting.rs:160-184,
   408-445`) [record]; `EvaluationAccounting::finish()` at `BTE/lib.rs:615-618` [opened]. A
   non-recording accounting builds and validates no receipt (`BTE/evaluation_accounting.rs:499-501,
   525-527`; production use "not traced") [record].
4. **Independence.** Same crate checks its own receipts: not independent [record + opened call site].
5. **Identity.** A folded number is written back as `TypeReferenceNode::Named{symbol: invalid,
   name: generated(spelling)}` (`BTE/const_evaluation.rs:855-858`) [opened] and read back later by
   parsing the name (sites listed area_07:70) [record]. An invalid-symbol `Named` counts as closed
   (`BTE/const_evaluation.rs:305-309`) [opened].
6. **Lost.** A captured native binding whose requirement source file is not loaded is skipped
   silently (`BTE/product.rs:242-253`); a failed range-endpoint round is left unfolded with no
   diagnostic (`BTE/const_evaluation.rs:4933, 5019-5034`); open layout policies are never evaluated
   (`BTE/layouts.rs:5338-5348`) [record].

## B2. Psi 02 symbol-resolved trees → Psi 03 typed trees

1. **Handed over.** `DeferredResolvedFrontier` (`S02/resolution.rs:2091-2125`); stage 03 entry
   `lower_symbol_resolved_trees_to_seeded_base(frontier.shared_trees())`
   (`S03/lowerer.rs:744-753`, returns `SeededTypingBase{resolved, typed}`), called at
   `PC/source_compilation.rs:849-855`; bodies complete later through
   `complete_machine_bodies` (`S02/resolution.rs:311`) → `typing.complete_bodies`
   (`S03/lowerer.rs:657`) in the loop `PC/source_compilation.rs:858-871` [opened].
2. **Trusted / re-checked.** Stage 03 copies the whole symbol table from stage 02
   (`symbols: symbol_resolved_trees.symbols.clone()`, `S03/lowerer.rs:1630`) and trusts every stamped
   symbol [opened]. It re-checks language rules on the resolved trees: equality operands, Equatable
   conformances, case-dispatch exhaustiveness (`S03/lowerer.rs:1611-1620`, `:1625`) [opened]. For an
   extension it checks pairing with the retained base (`CrossPairedResolvedBase`,
   `RetainedResolvedRootsChanged`, `AuthoredSelectionPrefixChanged`, `S03/lowerer.rs:761-773`)
   [opened].
3. **Check on the form.** No verifier of resolution (that each symbol is the right one) found. The
   pairing checks above run in default compilation on the extension path only [opened].
4. **Independence.** The pairing checks compare equality/prefix of the two handed-over tables: an
   identity/prefix comparison, not a second derivation [opened].
5. **Identity.** `SymbolHandle = Handle<Symbol>` (arena index + generation)
   (`psi/foundation/symbols/src/symbol.rs:294`) [opened]. Stage 02 still decides about 155 places on
   spelling (area_07:63) [record].
6. **Lost.** Ambiguity errors dropped at `S02/symbols_expressions.rs:314-319, 554` and re-raised only
   for expressions with authored exposure [record, "traced for those; open for the rest"]. If the child
   stream is shorter than the roster a declaration silently gets an invalid symbol
   (`S02/symbols_symbol_table.rs:1097, 1124, 1142, 1148`) [record]. Stage 03: a `requires x in D` whose
   domain symbol is still invalid becomes no progress premise, because premises are extracted at
   `S03/lowerer.rs:1854` before membership selections settle at `:1870` [record, order checked by
   digest writer]; exhaustiveness returns `Ok(())` for any run containing a transition with a
   continuation (`S03/equality_and_exhaustiveness.rs:932-934`) [record, digest checked].

## B3. Psi 03 typed trees → Psi 04 (typed → typed)

1. **Handed over.** `TypedTrees` (≈50 fields, `TT/lib.rs:1348`) [record] out of `SeededTypingBase`
   (`CompletedSource::finish` → `PreparedSource{typed}`, `PC/source_compilation.rs:1122-1127`)
   [opened]; consumed by `prepare_typed_trees(program, &preparation)`
   (`S04/preparation.rs:1941-1944`), called at `PC/source_checking.rs:776` [opened].
2. **Trusted / re-checked.** Stage 04 rejects any machine still `Deferred`
   (`S04/preparation.rs:1947-1953`) and requires evaluated constants unless pending inputs are
   admitted (`:1957-1959`) [opened]. It calls validation functions that also *rewrite* the tree
   (`resolve_dynamic_call_targets` `:1967, 2025`; `settle_value_destinations` →
   `resolve_named_result_overloads`, `land_float_literal_destinations` `:2058-2063`) [opened].
3. **Check.** None on 03's output beyond those preconditions. `validate_primitive_meanings` runs twice
   on the same program (`S04/assembly_and_operators.rs:613`, `S04/preparation.rs:1946`) [record
   grep; second site opened].
4. **Independence.** n/a.
5. **Identity.** `SymbolHandle`, `TypeReferenceHandle`, and `TypeIdentityId(u32)`
   (`TT/type_identity.rs:2995`) interned from the `String` that `normalize_type_reference` returns
   (`:2308-2312`); text recoverable by `type_identity_text` (`:7636-7638`) [opened]. So two types are
   the same id iff their rendered text is equal [opened for the text-returning normaliser and the
   text table; "same iff equal text" is the record's reading, area_12:78].
6. **Lost.** Stage 04 "most 'cannot do this' outcomes return `None`/`Ok(None)`/`continue` and leave the
   program as authored with no diagnostic" (unit 045, area_07:367, 36 sites) [record]. Examples: a
   `requires` type predicate blocks a clone only on `Some(false)` (`S04/body_cloning_and_rewriting.rs:744-746`);
   dynamic-slot demand collection error → `Ok(())` (`S04/monomorphization.rs:2667-2669`) [record,
   digest checked].

## B-BTE2. Build-time evaluation → typed trees (beside 03/04)

1. **Handed over.** In-place mutation of `TypedTrees`: `evaluate_or_defer_recorded(&mut TypedTrees)`
   (`BTE/lib.rs:597-613`, called `PC/source_compilation.rs:1069`) and
   `evaluate_fixed_layout_applications(&mut program, scopes, authority)` (`BTE/layouts.rs:6095`,
   called `PC/source_checking.rs:734-738`); after 04, `retain_access_lifecycle_plans`
   (`PC/source_checking.rs:777-781`) [opened].
2. **Trusted.** Stage 04/05 receive installed geometry, folded values and generated access-view records
   as ordinary typed data [opened call sites]. `evaluate_or_defer` refuses a restored pre-check that
   has not rejoined authority (`BTE/lib.rs:620-625`) [opened]. `validate_plan`
   (`BTE/layouts.rs:4333-4749`) validates a returned layout plan inside the producer [record].
3–4. As B-BTE1: producer-internal checks only.
5. **Identity.** Same invalid-symbol `Named` text for values; generated `Access<{policy},{schema}>`
   records later found by name (`BTE/layouts.rs:4190-4196`, `:2184-2219`) [record].
6. **Lost.** `BTE/layouts.rs:3955, 4011, 4023`: with no source map, any user type named `Access` with
   two nominal arguments is treated as the access template [record, "not traced"].

## B4. Psi 04 → Psi 05 checked trees

1. **Handed over.** `(FrozenTypedTrees, TypedPreparation)` from `prepare_typed_trees`
   (`S04/preparation.rs:2046-2055`); `TypedPreparation` holds `admits_pending_inputs`,
   `deferred_layout_applications`, `mathematics: CheckedMathematics` (computed at `:2045` by
   `proof::mathematical_declarations::check_mathematical_declarations`), opaque receipts and
   selections [opened]. Consumed by `lower_typed_trees_reusing(program, preparation, ...)`
   (`S05/checking.rs:938-985`), which calls `preparation.into_checking_judgments()` (`:956`) and
   `check_program` (`:1012`) [opened].
2. **Trusted / re-checked.** Stage 05 takes the specialised program and the mathematics judgment as
   given (`S05/checking.rs:1016, 1038`) [opened]; it re-runs the language checks (B5) but does not
   check that stage 04's cloning/specialisation preserved the authored program. Reuse: an equal prior
   input is taken without preparing or checking (`PC/source_checking.rs:741-769`; equality is
   `self.preparation_admission == *admission && ... && *self.preparation_input == *program`,
   `S05/checking.rs:849-869`); request-scoped authority still runs (`admit_request`, `:991-1000`)
   [opened].
3. **Check.** None on 04's output as such.
4. **Independence.** The reuse test is whole-value equality of the typed program: an equality check,
   not a derivation [opened]. Record: the typed-trees cache-key sum leaves out five tables
   (`DTI:1581-1609`) [record, open]; `ProgramIdentity` always compares equal (`TT/lib.rs:1341`)
   [record, open].
5. **Identity.** As B3. `CheckedMathematics` tables keyed `HashMap<SymbolHandle, ..>`
   (`md:11863-11927`) [record].
6. **Lost.** Product pruning is an explicit list of about 40 machine-keyed tables; a new table not on
   the list would be left unpruned (`PROD:659-920`) [record, "inferred"].

## B5. Psi 05 → validation and proof (side calls inside stage 05)

1. **Handed over.** `validation::CheckingContext::new(&typed_program)` with roots, pending-geometry
   flag, opaque selections and `mathematics.prepared_view()` (`S05/checking.rs:1032-1038`), used as
   `&dyn validation::CheckingInput` [opened]. Calls: `validate_selected_attached_method_bounds` `:1039`;
   `proof::obligations::build_proof_plan` `:1046`; `validation::prepare_specialized_program` and
   `proof::checker::check_proof_plan_with_decisions` (`:413-424`); `judge_contract_entailment`
   `:1094`; `admit_checked_quotient_requests` `:1102`; `prepare_specialization_contract_identities`
   `:1130`; `validate_runtime_proof_citations` `:1158`; `validate_reserved_cleanup_selections`,
   `validate_declaration_visibility` `:1232-1233`; `validate_boundary_calls` `:1394` [opened].
   Returns: `PreparedTypedProgram{deferred_entailment, frozen_dependency_bodies, integer_assignments,
   constant_recipe_obligations, proof_plan, operational, service_reaches, validation_facts,
   deferred_overflow}` (`S05/checking.rs:383-397`); `ProofDecisions{integer_assignments,
   constant_recipe_obligations}` (`PRF/checker.rs:6463-6466`); `ContractEntailment{proven_machine_contracts,
   dependency_body_judgments}` (`S05/checking.rs:1091-1097`) [opened].
2. **Trusted.** Stage 05 trusts every validation verdict and every proof decision; it consumes
   `validation_facts.expression_intervals` and `integer_range_obligations` directly
   (`S05/checking.rs:1061-1066`) [opened]. Dependency-body judgments may be reused across consumers
   when "correspondence still matches" (`S05/checking.rs:820-830`) [opened comment]; the reuse key
   hashes the source text of statements and contract facts (`pv:1216-1276`) [record].
3. **Check.** The only independent check on a validation/proof result is the admission kernel, and
   only on the integer-range certificate route: `BoundedValueCertificate::verify` →
   `verify_obligation(.., EvidenceRoute::CertificateDerived(..))` (`PRF/checker.rs:898-907`); routes in
   `ADM/lib.rs:94-161` [opened]. It runs in default compilation, inside the proof call. When no
   certificate is produced (`Uncovered`) the interval path and
   `validation::occurrence_integer_expression_fits_range` decide and the kernel never sees it
   (`PRF/checker.rs:373-397`) [opened]. Contract automation in validation has no kernel check where
   it accepts without evidence (`omega-rust/pipeline.md:101`) [opened doc].
4. **Independence.** The admission crate checks a certificate rule by rule and does no proof search of
   its own (area_11:15-19; design doc :132-137) — a separate derivation in a separate crate [record +
   doc]. Its elaborator, when it cannot build a derivation for a rule instance, declares that instance
   an opaque assumption (`ADM/bounded_denotation.rs:8640-8652`) [record, reader-reported].
   `checks_multiplicity_linear_validation.rs` both produces the permission events and replays them
   (`:2643`, `:4193`) [record].
5. **Identity.** Validation keys place facts by path strings (`ad:22110-22135`, `calls:16026-16043`),
   Psi 05 range facts by `display_name` labels (85 uses in `checks_ranges.rs`), proof obligations by a
   string key (`PRF/obligations.rs`) [record]. The proof derivation cache is keyed by
   `proof_obligation_key` (`PRF/checker.rs:2925-2945`) [opened].
6. **Lost.** Certificate objects: see answer (b). Records list 263 + 247 spelling-keyed and 143
   silent-drop sites in validation (README, area_13, area_14) [record].

## B6. Psi 05 checked trees → Psi 06 lowered Psi

1. **Handed over.** `CheckedTrees { typed, facts }` built by `CheckedTrees::with_roots(typed_program,
   facts)` (`S05/checking.rs:1392`), wrapped in `Arc` (`:969`) [opened]; `CheckFacts` has 52 fields
   (`ct/facts.rs:22-79`) [record]. Consumer `lower_machine(checked, machine, exact_toolchain_sources,
   roots, publication, external_declarations)` (`S06/machine_lowering.rs:1673-1680`), called at
   `TPR/lib.rs:444-451` [opened].
2. **Trusted / re-checked.** Stage 06 states it resolves everything "directly from Psi 05's frozen fact
   tables" and fails closed where no checked plan exists (`S06/lib.rs:10-16`) [opened]. Two entry
   refusals (`S06/machine_lowering.rs:1681-1696`) [opened]. It joins checked facts to lowered
   operations by `(state, statement index, call ordinal)`, usually demanding exactly one match
   (area_10:88-94) [record]. On its own output it runs
   `terminal_semantics::validate_runtime_evidence_inputs` (`S06/machine_lowering.rs:1800-1804`;
   defined `terminal-psi/semantics/src/calls.rs:1945`) [opened].
3. **Check on CheckedTrees.** Corpus only: `OMEGA_CORPUS_CHECKED_INTERPRETER=1` runs "the independent
   checked-tree source oracle instead of Terminal execution" (`omega/compilation/compiler/tests/corpus_runner.rs:80-84`)
   [opened]; opt-in. Test-only lint: `checked_trees_carry_no_string_identity_fields` greps six field
   spellings (`ct/lib.rs:131-150`) [record].
4. **Independence.** The checked-tree interpreter executes the same checked trees with a separate engine
   (not read; whether it shares stage 05/06 code was not checked). The lint is a text scan. The
   06 runtime-evidence check is in the shared semantics crate, not in stage 06 [opened location].
5. **Identity.** `SymbolHandle` and `(statement, ordinal)` coordinates for most lookups; duplicates
   handled three ways (unique / first / last) (area_11:60) [record]. Crash predicate identity is
   canonical bytes of spelled names (`c&o:2353-2400`) [record].
6. **Lost.** Opened by me: a closed `Boolean(_)` or `Integer(_)` contract clause becomes
   `Proposition::Truth` and its value is dropped (`S06/scalar_lowering.rs:1761-1763`); source-checked
   guarantees are filtered out of the projected contract (`:1783-1792`). Record (digest-checked):
   ensures rows dropped when the structural result is unavailable (`attached_unit.rs:541, 547`);
   authored `requires` lowered only if every clause is `Some` (`:6290`); no contract plan → no crash
   routes (`unit.rs:98-100`); last dispatch arm taken without testing its pattern
   (`unit_structural_values.rs:3593-3634`). Planners in 05: a builder's `None` means "no plan row",
   recorded as a plan gap or not at all (area_09:25) [record]. 56 silent-drop sites in 06 [record].

## B7. Psi 06 lowered Psi → Psi 07 lowered Psi

1. **Handed over.** `LoweredPsi` (`LP/lib.rs:233-248`): `semantic_module: TerminalModule`,
   `source_records`, source-boundary/machine/proof declaration pairs keyed by `SymbolHandle`,
   `retained_roots`, `debug_map: Option<TerminalDebugMap>`, `source_call_occurrences`,
   `selected_comparison_occurrences`, `state_blocks` [opened]. Before 07, terminal-production captures
   calling signatures, `unoptimized_behavior`, `lowered_module_provenance`, and takes
   `source_records` out (`TPR/lib.rs:454-473`) [opened]. Consumer `run_psi_optimization(lowered,
   selections)` (`S07/psi_optimization.rs:18-21`) [opened].
2. **Trusted.** 07 trusts the module; it computes `terminal_psi_identity` of input and output
   (`:22`, `:55-59`) [opened].
3. **Check.** In default compilation: only the fingerprint record (`PsiOptimizationExecutionRecord::new`,
   `:60-62`) [opened]. Corpus: a `rollback.txt` fixture compiles twice and reports whether the two
   semantic fingerprints differ (`module:differs`/`module:same`) (`corpus_runner.rs:86-99`) [opened].
4. **Independence.** Hash only [opened].
5. **Identity.** Terminal dense ids (`ValueId`, `PlaceId`, `OperationId`, `MachineId`, `BlockId`)
   [record, area_12:38]; identity equality by text per owning table (`TP/identity.rs:22-39`) [record].
   Side tables by `SymbolHandle` + `LoweredStatementCallCoordinate{state, statement_index,
   call_ordinal}` (`LP/lib.rs:29-33`) [opened].
6. **Lost.** Four selections (`ProofCheckElision`, `StateSpecialization`,
   `RepresentationSpecialization`, `LoopInvariantScalarMotion`) do nothing and are still recorded in
   the selection list (`S07/psi_optimization.rs:25-31, 48-51, 60-62`) [opened]. Machines with a ranked
   cycle are skipped by passes (`07/control_flow_and_retention.rs:20`) [record].

## B8. Psi 07 → Psi 08 Terminal Psi publication

1. **Handed over.** `PsiOptimizationStageResult{lowered, execution}` (`S07/psi_optimization.rs:75-78`);
   consumer `finalize_terminal_program(optimized, publication)` (`S08/lib.rs:104-133`), returns
   `TerminalProgramProduct::{Portable(CanonicalTerminalArtifact), Invocation(InvocationTerminalCode)}`
   [opened].
2. **Trusted / re-checked.** 08 runs `terminal_semantics::activation_storage::validate_module(&module)`
   (`S08/lib.rs:113-117`; defined `semantics/src/storage.rs:537`) [opened]. Portable:
   `CanonicalTerminalArtifact::from_parts` → `encode_module` (`CODEC/canonical_artifact.rs:71-77`),
   which runs only `scalar_qualification_wire::validate` and `validate_canonical_order`
   (`CODEC/semantic_module.rs:4064-4083`) [opened]. Invocation: `InvocationTerminalCode::new` checks only
   an invocation scope and computes identity (`CODEC/canonical_artifact.rs:374-386`) [opened].
3. **Check on the form.** The structural foundation validator, identity-formation validation and
   dynamic descriptor admission run only in `decode_module` (`CODEC/semantic_module.rs:4093-4120`)
   [opened] — i.e. when bytes are decoded: `CanonicalTerminalArtifact::from_bytes`
   (`CODEC/canonical_artifact.rs:166-183`, which re-encodes and byte-compares) [opened]. Same-invocation
   consumers borrow the produced artifact without serialising or decoding (`omega-rust/pipeline.md:201-206`)
   [opened doc]. In Omega, the only non-test `decode_module` call is the loose-bytes entrance
   `omega/.../00_terminal-psi-to-abstract-operations/src/artifact_admission.rs:1083` [opened grep];
   compiler tests call `from_bytes` on produced artifacts (e.g. `compiler/tests/access_view_calls.rs:156`,
   `support/behavior_admission_controls.rs:17`) [opened grep]. `verify_identity_formations`,
   `verify_module_mathematical_actuals`, `verify_indexed_theorem_roots` have callers only in
   `omega/compilation/compiler/tests/support/` [record; my grep found them only in tests/support files];
   `verify_mathematical_proof_actual` has none [record, rechecked by digest]. `verify_boundary_result_policies`
   (`S08/result_policy_verification.rs:277`) is exported and not called [record, digest checked].
   The `--validator`/`--every-validator` selection in `wiki/spec/build/command_line.md:104-131` has no
   hit for "validator" in `omega/application` or `omega/compilation/compiler/src` [opened grep].
4. **Independence.** The foundation validator is a separate 11,882-line derivation in the codec
   (`CODEC/foundation_validation.rs`, 618 `malformed(` sites) checking structural well-formedness, not
   meaning [record]. `decode_module` "checks only self-consistency of identity formations; comparison
   with source roots is in verifiers that have no production caller" [record, "traced"]. Activation
   storage validation is in the shared semantics crate [opened location].
5. **Identity.** Canonical `PSITERM\0` bytes, `TerminalPsiIdentity{vocabulary_marker,
   program_fingerprint}` (`CODEC/canonical_artifact.rs:85-91`) [opened]; canonical order keys built
   from identity text with `unwrap_or_default()` (`CODEC/semantic_module.rs:313-328, 424-436, 516-536`)
   [record].
6. **Lost.** `LoweredSourceCustody` is discarded: `let (module, debug_map, _) = lowered.into_parts()`
   (`S08/lib.rs:112`; `LP/lib.rs:251-265`) [opened]. The LP header says publication "discards them at
   the `PSITERM\0` wire boundary" (`LP/lib.rs:9-16`) [opened]. Installation section is always `None`
   (`CODEC/canonical_artifact.rs:95`) [record].

---

## (a) Boundaries whose handed-over form the receiver takes on faith, with no check anywhere

"No check anywhere" here means: no verifier, validator, round trip or oracle on that form in default
compilation, the corpus/test harness or a foreign-package boundary, that I found or that the records
name. End-to-end corpus outcomes (pass/fail rows, `--both`) exercise all stages together and are not
counted as a check on one boundary.

- **B1 syntax → resolution**: no verifier of syntax trees; parser rewrites (`move` erasure, guard →
  `Always`) arrive unchecked [record].
- **B2 resolution → typing**: symbol table copied whole (`S03/lowerer.rs:1630`) [opened]; no check that
  a stamped symbol is the right one. Only the extension pairing checks (`:761-773`) [opened].
- **B3 typing → preparation** and **B-BTE2 evaluation write-back into typed trees**: no verifier;
  folded values arrive as invalid-symbol `Named` text (`BTE/const_evaluation.rs:855-858`) [opened].
- **B4 preparation → checking**: no check that specialisation preserved the program;
  `CheckedMathematics` is taken as given [opened].
- **B5** validation verdicts that are not integer-range certificates (contract entailment, ownership,
  termination, the `Uncovered` interval fallback `PRF/checker.rs:373-397`) [opened]; pipeline.md:101
  says this automation remains trusted [opened doc].
- **B7 lowered → optimised**: fingerprint only (`S07/psi_optimization.rs:60-62`) [opened].
- **B6** has only an opt-in corpus oracle (checked-tree interpreter); **B8** has `decode_module` checks
  that run on decode (tests, loose-bytes ingress), not on the same-invocation artifact; activation
  storage is checked in default compilation (`S08/lib.rs:113`) [opened].

## (b) Where proof-obligation evidence is created and where it stops being carried

- **Integer range obligations.** Created in the proof crate during stage 05: `BoundedValueCertificate
  {context, obligation, assumptions, envelope: CertificateEnvelope}` (`PRF/checker.rs:891-896`),
  verified immediately by the admission kernel (`:898-907`, `ADM/lib.rs:111-141`) [opened]. It
  **stops at the proof → stage 05 return**: `certificate_verdict_measured` keeps the accepted fact in
  measurements and the certificate in a per-run derivation cache (`PRF/checker.rs:934-962`), and
  `ProofDecisions` carries only `CheckedIntegerAssignment{obligation, range}` and recipe obligations
  (`PRF/checker.rs:6458-6466`) [opened]. Nothing in `PreparedTypedProgram` (`S05/checking.rs:383-397`)
  holds a certificate [opened].
- **Mathematical declarations, identity formations, indexed theorems.** Created in stage 04
  (`check_mathematical_declarations`, `S04/preparation.rs:2045`) [opened]; carried in
  `TypedPreparation` to stage 05, copied into `facts.proof` with term, type and assumption closure
  (`S05/checking.rs:1279-1391`) [opened]; stage 06 re-encodes mathematical certificates into the
  Terminal module (`S06/scalar_lowering.rs:1321, 1392, 1901-1917, 1968-1984`) [opened grep lines]; they
  cross B8 into the artifact. Decode re-checks identity formations for self-consistency
  (`CODEC/semantic_module.rs:4111-4114`) [opened]; the source-root comparison verifiers are test-only
  [record].
- **Contract automation (requires/ensures, ownership, etc.).** No evidence object is created; the
  result is a verdict. Stage 06 carries "source-checked" as a flag: `SourceCheckedGuarantee` values are
  refused as runtime requirements (`S06/scalar_lowering.rs:1767-1770`) and filtered from projected
  guarantees (`:1783-1792`) [opened]; obligations on decided arrival edges are marked source-checked
  with no certificate (`unit_composed_control.rs:10967-10988`, "stated") [record]. Terminal's
  `EvidenceRoute` has a `SourceChecked` variant (`TP/artifacts.rs:371-377`) and the admission kernel
  rejects it outside an operation (`ADM/lib.rs:159-161`) [opened]. Downstream (Omega 00, outside my
  scope) accepts operation obligation references with no evidence (design doc :226-232, author-opened)
  [doc].

## (c) Last stage with source positions; how later stages point back

- Tokens and syntax carry spans (`ST/lib.rs:7`) [opened header line]; the symbol table carries
  provenance spans (`symbol_provenance_source_span`, used `PC/source_compilation.rs:917, 1246`)
  [opened]; typed trees keep that table (`S03/lowerer.rs:1630`) [opened]; `CheckedTrees` contains the
  whole typed program (`S05/checking.rs:1392`) [opened]. **Stage 06 is the last stage that reads source
  positions**: `build_debug_map` from the stage 05 plan `facts.flow.terminal_debug`
  (`S06/machine_lowering.rs:1806-1825`, `:648-832`) [opened].
- `TerminalModule` itself carries no `SourceSpan`: in `terminal-psi/src`, `SourceSpan` appears only in
  `artifacts.rs` and `lib.rs` side records (my grep; `terminal_module.rs` has none) [opened grep].
- Pointers back:
  - `TerminalDebugMap{semantic, files: Vec<DebugSourceFile{id, origin, byte_len, digest, path}>,
    sites: Vec<DebugSite{subject: DebugSubject(Machine|Block|Operation|Edge|Value|Contract|Obligation|Place|Claim),
    span: DebugSourceSpan{file, start, end}}>}` (`TP/artifacts.rs:115-155`) [opened], encoded as the
    artifact's optional debug section (`CODEC/canonical_artifact.rs:79-84`) [opened]. Built only when
    the selection is `Eligible` and the plan is `FromCheckedPlan`, otherwise `None`
    (`S06/machine_lowering.rs:1807-1825`) [opened]. Correspondence is positional with fallback: block
    index → source state of the same index, else the last state; operation index → `operation_spans`,
    else the state span (`:703-733`) [opened].
  - `PreparedNominalRoot{.., source_machine_name, span}` and `TerminalRootRequest{.., span}`
    (`TP/lib.rs:82-87, 150-154`) [opened]; `TerminalMachineProvenance{machines: MachineId →
    (String, Option<SourceSpan>), callers}` (`TP/artifacts.rs:163-167`) [opened].
  - Beside the artifact, not in it: `ProducedTerminalProgram{.., provenance, source_records}`
    (`TPR/lib.rs:471-473, 511-519`) [opened]. Source custody (`LoweredSourceCustody`) is dropped at 08
    (`S08/lib.rs:112`) [opened].
  - Front-end diagnostics before typing print `path:line:col` from byte offsets (`PC/lib.rs:370-379`)
    [opened].

## Could not establish

- Whether the checked-tree interpreter (corpus oracle) shares code with stage 05/06; not read.
- Whether any production path decodes a same-invocation artifact (I read pipeline.md's statement and
  grepped `decode_module`; I did not trace every Omega consumer).
- Whether `debug_map` positional fallback ever misattributes a span in practice.
- Stage 04's `selection_and_reflection.rs` and `preparation.rs` were outside area_07's records
  (area_07:532); I read `prepare_typed_trees` only.
- Records' "open" accepting paths were not traced to wrongly accepted programs by anyone.

## Claims I opened myself (at least twelve)

`PC/lib.rs:284-379, 400-454`; `PC/source_compilation.rs:795-1083, 1122-1173`;
`PC/source_checking.rs:676-818`; `S00/lexer.rs:382-396`; `S01/parser.rs:227-256`;
`S02/resolution.rs:2091-2125, 300-330`; `S03/lowerer.rs:744-773, 1611-1631`;
`S04/preparation.rs:1941-2056`; `S05/checking.rs:376-455, 740-1408`; `PRF/checker.rs:318-397,
884-962, 6458-6470`; `ADM/lib.rs:88-165`; `BTE/lib.rs:195-215, 590-640`;
`BTE/const_evaluation.rs:300-314, 848-860`; `TT/type_identity.rs:2295-2312, 2993-3001, 7629-7638`;
`S06/lib.rs:10-16`; `S06/machine_lowering.rs:690-745, 1673-1696, 1795-1830`;
`S06/scalar_lowering.rs:1755-1792`; `S07/psi_optimization.rs:1-80`; `S08/lib.rs:1-141`;
`LP/lib.rs:1-266`; `CODEC/canonical_artifact.rs:60-120, 150-190, 365-400`;
`CODEC/semantic_module.rs:4040-4125`; `TP/artifacts.rs:100-155, 340-420`; `TP/lib.rs:58-177`;
`corpus_runner.rs:30-99`; greps for `decode_module`, `from_bytes`, `verify_*`, `validator`,
`SourceSpan`, `SourceChecked`.
