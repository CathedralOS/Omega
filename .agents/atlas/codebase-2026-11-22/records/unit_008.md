# Record: unit_008 (areas 09-native-stages-00-03 and 10-native-stages-04-10)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`.
Paths are relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/`. `S3` = `03_abstract-operations-to-abstract-operations/src/abstract_optimization/`. `S4` = `04_abstract-operations-to-target-operations/src/`.
All 20 ranges in the unit were read in full. Grep counts are from the Grep tool (count or content mode, uncapped unless noted).

Cross-file facts used below:
- Stage 03 is a set of rewrite rules. Each rule proposes `PsiRewriteCandidate`s from analysis products and a `PsiOptimizationUnit`; a pass manager (not in this unit) runs them. Selection is by `PsiOptimization` enum value through `PSI_PASS_CATALOG` (`rules.rs:223-257`).
- Stage 04 files in this unit are the plan-preparation half of the stage: they turn retained target rows into a `LegalizedOperationPlan` and also contain the independent replay of that derivation.

---

## S3/ranked_rewrites.rs (1284 lines)

1. Purpose. Implements the one "session step" pass, loop-invariant scalar motion: it moves loop-body operations that do not change per iteration into the loop's single preheader block, rebinding values, places and cleanup custody. It also holds the independent validator that recomputes the move from the input and compares it to the proposed candidate.
2. Shape. One big `pub(crate) mod loop_invariant_scalar_motion` (`:8`). Sub-modules: `apply` (`:31`, with `realize::realize` at `:53`, `operation_location` `:272`, `validated` `:346`, `reconstruct_custody` `:394`), `propose` (`:411`; `all` `:423`, `component_candidate` `:447`, `admit_member_node` `:542`, `component_plan` `:768`, structs `ComponentPlan` `:482`, `MemberAdmission` `:487`, `PlanEvidence` `:493`), `validate` (`:993`; `candidate` `:1001`, `reconstruct_provenance` `:1071`). Public wrappers `propose_/validate_/apply_loop_invariant_scalar_motion` at `:1159-1178`. `contract()` `:1195` (analysis in: `EffectSummaries`; invalidates `AnalysisKind::ALL`; safety class `StructuralIdentity`). Error enum `LoopInvariantScalarMotionError` `:1248`.
3. I/O. Reads a `VerifiedPsiOptimizationSession` (unit, cycle components, ranking certificates), effect summaries (`calls::unit_effect_summaries`, `validation.rs:585`). Produces `LoopInvariantScalarMotionCandidate`, then a new `PsiOptimizationUnit` with moved nodes, recomputed identity (`:268`), a `PsiTransformationLedger` (`:379`).
4. How it decides. Admission is keyed on node shape and identity: the first provenance entry must be `PsiProvenance::Operation(op)` (`:90`, `:831`), and the unique shared entry source of a cycle component (`shared_entry_source`, `validation.rs:1096`). `admit_member_node` (`:542-711`) is an `if/else if` chain over ~20 shapes, each delegating to `ops::admissible_*`/`calls::admissible_*` in `validation.rs`. A node is considered only if its block is in `guaranteed_executed_member_blocks` and the preheader's terminator has all successors inside the component (`:551`, `:789-793`). Storage-producing nodes also need `component_members_unobservable` (`:570-578`). Fixed point: the `loop` at `:824` re-scans members until no node is admitted (`:962`). Validation recomputes the plan and requires `*candidate == expected` (`:1050`).
5. Cases by shape. The chain at `:590-705` is a closed list: scalar leaf relocation, place read, byte read, field byte read, subslice, byte literal, primitive local, record, scalar array, scalar case, trivial affine local, scalar call, crash-continuation call, unit call, structural scalar call, structural call (each with crash-continuation variant), else generic scalar computation. Result classification `:870-938` is again a fixed list and returns `CandidateMismatch` for any other definition shape (`:935`, `:937`). `realize` handles crash-continuation only for `O::Call` and `CallStructural` with `AbstractCallTarget::Machine` (`:127-139`).
6. Same job elsewhere. The "scan the whole unit for the one node with provenance Operation(op)" pattern is written three times: `apply::realize` (`:89`), `apply::operation_location` (`:272-295`) and `validate::unique_source` (`:1132-1156`). `Unit.function(...)` lookups follow the same pattern as in `semantic_analyses.rs` (linear scans, below). `propose` and `validate` both build the plan with the same `component_plan` + `realize` + `LoopInvariantScalarMotionCandidate::new` sequence (`:453-479` and `:1022-1049`).
7. Unused/one-caller. `loop_invariant_scalar_motion|ranked_rewrites` appears in 5 files, 28 occurrences (grep count over `source-rust`); `pass_manager.rs` has 14. No pub function here looked unused.
8. Stated limits. None as "unsupported"; comment header says "unique-preheader" only (`:3`). Candidate budget refusal `CandidateBudgetExhausted` (`:439`).
9. Fail-open risks. `reconstruct_custody` (`:394-409`) runs the three countdown analyses only `if !session.ranking_certificates().certificates().is_empty()`; with no certificates it returns `Ok(())` without checking. Classification: open (no comment says why; I did not trace whether a unit with cycle components but no certificates can reach here). `relocated_affine` and `consumed` sets (`:203-234`) drive custody rewriting; a miss there changes cleanup rosters but later `from_transformed` validation (`:366`, `:1054`) re-checks the unit (read). Otherwise none seen.

## S3/rules.rs (1296 lines)

1. Purpose. Declares the pass-name constants, the nine-entry pass catalog, the ordered rule registry, the `PsiOptimizationRule` trait and the rule families that are small enough to live here: copy propagation (redundant block parameter), dead pure scalar elimination, case-membership and field-value specialization, state-argument specialization.
2. Shape. Pass names `:8-19`. `standard_rule_contract` `:21`. Macros `psi_rule_contract!` `:56`, `psi_rule_impl!` `:100`, `required_analysis!` `:114`. Modules: `accepted_obligations` `:122`, `catalog` `:163` (`PSI_PASS_CATALOG: [PsiPassCatalogEntry; 9]` `:223`, `BuiltInRuleRegistration` `:260`, `assemble_built_in_registry` `:274`), `node_elision` `:292`, `registry` `:483` (`RuleAnalysisView` `:500`, `RuleProposalError` `:534`, trait `PsiOptimizationRule` `:546`, `OrderedRuleRegistry` `:585`), `case_membership_specialization` `:648`, `copy_propagation` `:720`, `dead_scalar_elimination` `:892`, `field_value_specialization` `:1047`, `state_specialization` `:1129`. `PsiPassStep` `:1237`. `built_in_psi_steps_for_selections` `:1280`.
   Note: the crate map gives the catalog at `:216-250`; in this checkout it is `:223-257`.
3. I/O. Rules read `RuleAnalysisView` products (analyses requested by the rule's contract) and the `PsiOptimizationUnit`, and return candidate lists. `built_in_psi_steps_for_selections` reads `PsiOptimizationSelections` and returns pass steps.
4. How it decides. Pass selection is keyed on the `PsiOptimization` enum (`:1283-1295`), and an unlisted value returns `UnsupportedOptimization`. A rule's access to analyses is keyed on its declared `AnalysisSet`: `RuleAnalysisView::get` returns `None` unless the kind was requested (`:516-521`). Registry identity keys on `OptimizationRuleIdentity` bytes (such as `b"omega.psi-rule...v1"`); duplicates and mixed passes are refused (`:604-616`). Dead-scalar elimination keys on the liveness row and a "pure scalar baseline" effect row (`:374-382`). Redundant block parameter keys on all incoming bindings carrying the same argument and a dominance test (`:796-808`).
5. Cases by shape. Dead-scalar classification lists operations by type: `dead_scalar_literal_elimination::classify` `:929-952` (integer and boolean constants only); `dead_unconditionally_total_scalar_elimination::classify` `:992-1034` (boolean ops, comparisons, bitwise ops, widen, arithmetic with `arithmetic.obligation().is_none()`, wrapping shift). Elision requires a following node in the block (`:362`, `:442`). Specialization families skip any machine in `scc.cyclic_machines(unit)` (`:671`, `:1071`, `:1176`).
6. Same job elsewhere. The two `psi_rule_contract!` forms and `constant_fold_rules!` in `sccp.rs:8` do the same job (declare a rule type with a contract). `representation_specialization_contract` (`:40`) vs `standard_rule_contract` (`:21`): the first calls the second.
7. Unused/one-caller. `accepted_obligation_fact` (`rules.rs:129`) is used in `sccp.rs` (twice, `:357`, `:543`) and `rules.rs:416` (read). A different function of the same name is `optimization-unit/src/proof.rs:288` (method on the unit; this function calls it at `:150`). `BuiltInRuleRegistration::new` has one registration site per rule family. No other pub unused function noted.
8. Stated limits. `RuleRegistryError::UnsupportedOptimizationCombination` (`:574`) is declared; in this file it is never constructed (grep inside the file, read: only the enum arm). Not traced elsewhere.
9. Fail-open risks. `assemble_built_in_registry` uses `assert_eq!` (`:280`) so a bad ordinal panics rather than passes. `node_elision::propose` skips on missing liveness (`live.is_none_or(...)` -> continue, `:380-384`): an unknown means "keep the node", which is fail-closed. None seen that accept a wrong program.

## S3/sccp.rs (1235 lines)

1. Purpose. Sparse conditional constant propagation rules: constant folding of integer binary/unary/cast operations, Boolean folds, and range-vs-constant and range-vs-range comparisons. It also defines the registration order of the 39 SCCP rules.
2. Shape. Macro `constant_fold_rules!` `:8`. Modules `boolean_constants` `:35`, `integer_binary_constants` `:186` (22 rules `:205-228`), `integer_binary_shapes` `:367`, `integer_cast_constants` `:466`, `integer_unary_constants` `:547`, `range_against_constant` `:639`, `range_against_range` `:800`. Top level: `built_in_registrations` `:994` (ordinals 0-38), `constant_evaluation_contract` `:1038`, `propose_integer_constant_folds` `:1060`, `propose_boolean_evaluations` `:1116`, `range_comparison_contract` `:1160`, `integer_constant` `:1174`, `integer_value_type` `:1191`.
3. I/O. Reads `ScalarConstantAnalysis` (and `ValueRangeAnalysis`) from the analysis view; emits `PsiRewriteCandidate::new_integer_evaluation`/`new_boolean_evaluation` with a witness naming the facts used.
4. How it decides. Folding is keyed on the fact table: `integer_constant` (`:1174-1189`) linear-scans `constants.facts` for `(machine, value)` and needs `fact.identity` present. Range rules find the range fact by `(machine, value, scalar_type)` and `ranges.fact_applies_at` (`:741-746`, `:882-888`). Type agreement is checked by `integer_value_type` on both operands (`:155-158`, `:737-740`, `:877-880`). Evaluation uses `IntegerType` methods (`compare`, `exact_cast_value_to`, ...) and returns `None` when undefined, so no fold is proposed (`:159`, `:515`, `:621-627`).
5. Cases by shape. One rule per (policy, operator) pair for binary integer operations (`:205-228`). Safety class per rule is spelled in the macro table: Exact add/sub/mul/shift are `ProofCertified`; saturating divide/remainder and wrapping divide/remainder are `ProofCertified`; wrapping/saturating add/sub/mul are `ExactOperationSemantics`. Range rules all use `ProofCertified` through `range_comparison_contract` (`:1170`).
6. Same job elsewhere. `range_against_constant::contract` (`:656-664`) and `range_against_range::contract` (`:817-822`) take a `_safety` argument and ignore it: the safety class is always `ProofCertified`, although the macro table passes the identical `ProofCertified` anyway (read). The two `evaluate` functions (`:766`, `:912`) repeat the same min/max comparison logic.
7. Unused/one-caller. `literal_integer_constant` (`:1650` in semantic_analyses.rs) is used in `sccp.rs:736`. Not otherwise checked.
8. Stated limits. None.
9. Fail-open risks. `integer_constant` returns `None` for a boolean fact and for a fact with `identity: None` (`:1184`); the caller then proposes no fold (keeps). None seen.

## S3/semantic_analyses.rs (2371 lines)

1. Purpose. The analyses the rules read: effect summaries, ownership frontiers, place access summaries, place aliases, sparse conditional constants/executable edges, use-definition, value liveness, plus `semantically_reachable_blocks`.
2. Shape. Modules: `effect_summaries` `:11` (`EffectClass` `:24`, `NodeEffectSummary` `:34`, `effect_summaries` `:90`, `transitive_function_effects` `:139`, `operation_effect` `:317`), `ownership_frontiers` `:417`, `place_access_summaries` `:467` (`FunctionContext` `:615`, `PendingSurface` `:794`, `scan_operation` `:891`, `place_access_summaries` `:1286`), `place_aliases` `:1473`, `sparse_conditional_constants` `:1575` (`LatticeValue` `:1720`, `sparse_conditional_constants` `:1726`, `sccp_machine_snapshot` `:2051`), `use_definitions` `:2152`, `value_liveness` `:2225`, `semantically_reachable_blocks` `:2350`.
3. I/O. All take `&PsiOptimizationUnit` and return an analysis struct stamped with `revision: unit.identity`.
4. How it decides. Effect summary keys on the `AbstractOperation` variant (`operation_effect`, `:327-414`, an exhaustive `match` with no wildcard). A call with no callees, unresolved targets, or a callee not in the unit's machine set becomes May on all four effects (`:218-226`); a `CallStructural` to a `Dynamic` target is May (`:228-239`). Place-access analysis resolves a place through `roots`, `aliases` and `bindings` keyed on `PlaceId` and path (`:703-778`); cycles end as `External` (`:714-716`). SCCP uses a three-point lattice per `ValueId`: parameters start `Overdefined` (`:1785-1787`), block parameters `Unknown`, node definitions `Overdefined` unless a literal fact exists (`:1797-1829`). An edge is executable only if the condition is a known Boolean matching the arm (`:1864-1894`); `Conditional` with an integer constant, `Unknown` or missing value gives no successors (`:1891-1893`).
5. Cases by shape. `scan_operation` (`:891-1260`) has no wildcard arm; unlisted operations in the final group do nothing to access sets (`:1227-1258`, with the comment "Partition changes custody, without reading or writing the value's bytes." at `:1239`). `RegisterAccess`, `BoundaryCall`, `ExternalCompletion`, and a `CallStructural` with no machine callee set `unsummarized = true` (`:948-950`, `:1213-1226`, `:1193-1202`).
6. Same job elsewhere. Reachability is computed locally by `semantically_reachable_blocks` (`:2350`), 5 call sites, all in this file (`grep -c`: 5 in `semantic_analyses.rs`). The SCCP edge-state code appears twice with the same three-way rule: `:1946-1999` (edge facts) and `:2075-2089` (snapshot); both repeat "source reachable and condition is a constant Boolean -> inexecutable; source reachable and other conditional -> unknown; non-conditional -> inexecutable". Effect-class pure scalar list (`:328-342`) and `dead_unconditionally_total_scalar_elimination::classify` (`rules.rs:992`) list overlapping operations separately.
7. Unused/one-caller. `PlaceAliasesAnalysis` has two empty `impl` blocks (`:1511`, `:1513`). `place_aliases`/`ownership_frontiers` re-exported at `:2338-2339`; not traced for callers (suspected: pass manager or `analyses.rs`).
8. Stated limits. None as comments, except the `unsummarized` flag.
9. Fail-open risks. `transitive_function_effects` fixed point skips callees missing from `prior` (`:269-271`: `let Some(callee) = prior.get(&callee) else { continue; }`). Traced within the file: callees outside `machines` already forced May at `:220-226`, so a missing row does not weaken the summary. SCCP `merge` treats `LatticeValue::Unknown` incoming as no change (`:1735`). Stated by lattice semantics, not by a comment. Non-SCCP: `place_access_summaries` marks `ResolvedAccess::Internal` as no external access (`:848`); an alias with no entry and no referent step is "Internal" (`:764`). Open (no comment; a wrong program would be one where an access through an unrecorded alias is hidden).

## S3/specialization.rs (552 lines)

1. Purpose. Plans for three rewrites that need proof evidence: field-value specialization, case-membership specialization, and state-argument dispatch specialization. Header says every family excludes machines with cyclic components and derives provenance rows for independent validation (`:1-8`).
2. Shape. `field_value_specialization::propose::plan` `:17`; `representation_specialization::{accounting::provenance_rows `:49`, admission::{membership_evidence `:87`, admit_membership_node `:101`}, propose::plan `:154`}`; `state_specialization::{accounting::provenance_rows `:207`, admission::{dispatch_evidence `:285`, incoming_edges `:334`, admit_incoming_edge `:351`, delivered_argument `:438`, call_result_constant `:478`}, propose::plan `:517`}`.
3. I/O. Reads `PsiOptimizationUnit`, function, place or block, and (state) `ScalarConstantAnalysis`. Returns rewrite patch structs from `optimization_unit`.
4. How it decides. Case membership: the node must be `StructuralCaseMembership` on the declaration's own place id with a Boolean result (`:109-121`); the proven case comes from `established_case_at` or `resolved_sole_case` (`:122-131`); `outcome = proven_case == *case` (`:144`). State dispatch: block must not be the entry, must have no structural parameters, last node must be `Conditional`, and the condition is a block parameter directly or via `integer_comparison_condition` (`:293-313`). The parameter must have no existing constant fact (`:314-320`). Each incoming edge must be a `Jump` or `Conditional` arm with empty structural bindings and cleanup actions (`:364-399`). `plan` clears all edges if every incoming edge specialized (`:542-544`).
5. Cases by shape. Only `Jump`/`Conditional` predecessors (`:395` `_ => return None`). `call_result_constant` handles a callee with exactly one `O::Return` (`:492-501`). `delivered_argument` follows single-predecessor block parameter chains, bounded by block count (`:445`).
6. Same job elsewhere. `rules.rs:648-718`, `:1047-1125`, `:1129-1220` are the rule wrappers that call these planners. Provenance accounting appears in `rules.rs:433-481` (`node_elision_accounting`) too, in a different form.
7. Unused/one-caller. Each `plan` is called once, from its rule in `rules.rs` (grep not run; read).
8. Stated limits. Header comment on cyclic exclusion (`:5-6`).
9. Fail-open risks. `edges.clear()` when all incoming edges specialize (`:542-544`) means "do not rewrite" (keeps). None seen.

## S3/validation.rs (3151 lines)

1. Purpose. Admission and validation helpers: optimization context checks (`context`), the shape and safety predicates used by loop-invariant motion (`invariant_calls`, `invariant_operations`, `member_blocks`, `place_observations`, `relocation_rewrites`, `scalar_case_frontiers`), and the pre-physical manifest and plan projection sealing (`prephysical_manifest`, `projection`).
2. Shape. `context` `:12` (`validate_verified_psi_context` `:18`, `validate_transformed_psi_context` `:25`); `invariant_calls` `:55`; `invariant_operations` `:663`; `member_blocks` `:1071` (`guaranteed_executed_member_blocks` `:1105`, `resolve_member_parameters` `:1191`, `member_scalar_operand_substitution` `:1318`); `place_observations` `:1395`; `prephysical_manifest` `:2149` (`project_pre_physical_optimization_manifest` `:2266`); `projection` `:2319` (`seal_optimized_abstract_plan_projection` `:2329`, validator id `b"omega.validator.optimized-abstract-plan-projection.v33"` `:2346`); `relocation_rewrites` `:2395`; `scalar_case_frontiers` `:2923` (`rewrite_relocated_case_result_frontiers` `:2932`).
3. I/O. `context` calls `optimization_unit::validate_runtime_evidence_context` and `ranked_cycles::validate_exact_ranked_cycles` (`:38-44`). Others take `PsiOptimizationFunction`, `OptimizerCycleComponent`, `OptimizationNode`, sets of `ValueId`/`PlaceId`.
4. How it decides. Judgments are keyed on shape and identity: `call_shape_spine` (`:77-90`) requires provenance first = `Operation(psi_operation)`, `uses` equal to the argument list in order, and no successors. `scalar_call_callee_pure` (`:600-613`) is true only if the callee's function summary has observable, crash and suspension all `No` (and a missing summary is false, `is_some_and`). `component_members_unobservable` (`:623-661`) requires every member call target to be pure and every other node's effect row to be `No`; a missing row is false (`observable != Some(No)`, `:653`). `crash_routes_cover` (`:569-583`) compares `route.cause` and alternatives. `scalar_case_result_contained` (`:2468-2587`) is a per-operation `match` on the node; the final `operation =>` arm uses `member_place_references`, and an operation that function returns `false` for is accepted only if it is outside the component (`!member`, `:2541`). Manifest projection builds an initial unit from the input and counts statistics (`:2174-2197`).
5. Cases by shape. Quoted by function: invariant calls accept only `O::Call` and `O::CallStructural` with `AbstractCallTarget::Machine` (`:103-111`, `:142-150`) and structural results of multiplicity `Affine | Unrestricted` with no qualifications or claims (`:238-246`). `invariant_record` allows structural fields only if `access == Owned` and an empty path (`:906-912`). `node_preserves_place_observations` (`:1476-1532`) is an explicit allow list ending `_ => false`. `member_place_references` (`:1651-1730`) is an allow list ending `_ => return false`. `produced_place_root` lists 9 producer shapes (`:1761-1784`).
6. Same job elsewhere. The "root resolves to `place` if visible in the preheader, or is relocating with one producer, else try the representative" block appears four times: `invariant_calls.rs`-style at `:452-459`/`:468-475`/`:499-510` (`call_argument_root_landing`), `invariant_operations::invariant_record_admission` `:950-961`, `place_observations::invariant_observation_root` `:1876-1888`. `component_preserves_place_observations` and `exclusive_borrow_call_preserves_place_observations` repeat the access-mode loop that `borrow_call_evidence` (`:441-487`) does.
7. Unused/one-caller. `admissible_invariant_scalar_computation` is called only from `member_blocks::invariant_scalar_operand_substitution` (`:1312`) in this file (read). `build_verified_psi_optimization_unit` is called in `validation.rs:2174` (1 occurrence in this file; 8 across `source-rust`: `mod.rs` 2, `validation.rs` 1, `verified_unit.rs` 4, `lib.rs` 1). `ValidatedPrePhysicalOptimizationManifest` is `pub` (`:2298`) and re-exported at `:3145`.
8. Stated limits. `crash_routes_cover` and related functions return `None` when the callee has no `verified_contract` (`:386`, `:565`, `:2880`).
9. Fail-open risks. `scalar_case_result_contained` `operation =>` arm (`:2536-2543`): for an operation whose references cannot be listed, it returns `!member`; so a non-member operation of any unknown kind passes. Classification: open (no comment). Wrong program: an operation outside the component that observes the case result in a way `member_place_references` cannot list. Other accessor functions return `false` on `None`.

## S3/value_ranges.rs (106 lines)

1. Purpose. Builds value-range facts from scalar constants only ("Constant-backed ranges, independently reconstructed by their consumers", `:1`).
2. Shape. `constant_facts::collect` `:14`, `facts::new` `:56`, `ValueRangeAnalysis` `:84` with `fact_applies_at` `:89`, `value_ranges` `:102`.
3. I/O. Calls `scalar_constants(unit)` and keeps integer facts with an identity; each becomes a `[value, value]` range with support `ScalarConstant(identity)`.
4. How it decides. Keeps a fact only if it is an integer, has `identity`, its function exists, and the value's definition scalar type is an integer (`:17-31`). `fact_applies_at` requires the fact to be in `facts` and `validate_current_value_range_fact_at(...)` to be `Ok` (`:97-98`).
5. Cases by shape. Only singleton ranges from constants; no interval analysis.
6. Same job elsewhere. `integer_constant` in `sccp.rs:1174` reads the same constants table with different filtering.
7. Unused/one-caller. `value_ranges` re-exported in `semantic_analyses.rs:2343`; used for the analysis kind (not traced).
8. Stated limits. None.
9. Fail-open risks. None seen.

## S3/verified_unit.rs (872 lines)

1. Purpose. Builds the verified optimization unit: reconstruct the unit seed from the admitted plan, attach the module context, derive "accepted obligation" facts (with evidence-seal fingerprints), and attach them.
2. Shape. `VerifiedPsiOptimizationUnitBuildError` `:15`; `build_verified_psi_optimization_unit` `:84`; `product_obligation_census` `:114`; `source_checked_obligation_facts` `:189`; `sealed_fingerprint` `:446`; `VerifiedPsiOptimizationUnit` `:473` (`input()`, `unit()`, `into_parts()`); `#[cfg(test)]` helper `authorize_fixture` `:493` and `qualified_cleanup_obligation_receiving_tests` module `:505`.
3. I/O. Reads `VerifiedPsiOptimizationInput` (`plan()`, `context().module()`, `evidence_seals()`). Env var read: `std::env::var_os("OMEGA_PROOF_MEASUREMENTS")` at `:95`; when set, prints the census to stderr (`:96-99`).
4. How it decides. Fact coordinates are `(machine, owner, obligation)` de-duplicated in a `BTreeSet` (`:208`). For `EstablishElementView` with `WholeRepresentation` region it reconstructs the count goal from layout sizes (`:211-267`). A seal is used when `EvidenceSealValidator::fingerprint` finds one; otherwise the fingerprint is `[0; 32]` (`:465-467`). Unused seals fail the build (`:424-432`). Receiving restriction is derived for `WriteOnlyPrimitiveStore`, `EstablishPrimitiveLocal`, `AtomicEvent` (`:286-342`).
5. Cases by shape. `receiving` closure matches exactly three operation kinds, `_ => return None` (`:333`). Terminator owner rows: block-owned read path, edge-owned bindings, edge-owned unsealed cleanup (`:349-391`, comment `:349-354`).
6. Same job elsewhere. None in this unit.
7. Unused/one-caller. `product_obligation_census` has one caller (`:98`). Comment at `:105-113` says it answers a question in `wiki/drafts/designs/check_on_lowered_form.md`; one `// ponytail:` comment at `:149`. `OMEGA_PROOF_MEASUREMENTS` appears in `pass_manager.rs` (1), `verified_unit.rs` (1) and `wiki/drafts/reference/matching_logic.md` (1) (grep count).
8. Stated limits. `sealed_fingerprint` doc (`:436-445`): "What is still taken on faith: that the proof kernel accepted the certificate under those hypotheses". Also "zero when no seal names it, as for every module the current pipeline produces" (`:437`).
9. Fail-open risks. (a) `sealed_fingerprint` returns `[0; 32]` when no seal exists (`:465-467`) and the fact is still accepted. Stated (`:436-445`). Wrong program: a module with no seals gets accepted obligations with a zero fingerprint; the code says every module the current pipeline produces looks like this. (b) `unwrap`-style `?` in the `receiving` closure returns `None` on a missing restriction, so the scalar-receiving data is simply omitted (`:305`, `:321`, `:325-326`); classification open. (c) Tests (2) are `#[ignore]` (`:540`, `:624`) and need an env-supplied artifact, so this file has no running unit test (read).

## S3/lib.rs (11 lines)

Crate root: `#![forbid(unsafe_code)]`, re-exports `AbstractOptimizationError`, `ValidatedOptimizedAbstractPlan`, `optimize_abstract_operations`; under feature `test-support` also `build_verified_psi_optimization_unit` and its error (`:5-11`). Decides nothing. No fail-open.

---

## S4/descriptor_input.rs (439 lines)

1. Purpose. Decides whether a foreign call that reads bytes from a descriptor (file or stream read) may write only a scalar-prefix of its buffer, and computes that prefix length so lowering can bound the write ("`DescriptorInputGuard`").
2. Shape. `descriptor_input_guard_with_closed_scalars` `:23` (from a boundary declaration), `descriptor_input_guard_for_call_with_closed_scalars` `:62` (from a call's normalized arguments), `derive` `:125`, `selected_descriptor` `:191`, `scalar_prefix` `:204`, `scalar_prefix_byte_width` `:298`, `PrefixRecipe` `:317`, `coverage` `:324`, `merge` `:369`; one test `descriptor_scalar_geometry_keeps_exact_width_and_projection_identity` `:392`.
3. I/O. Reads the boundary declaration, the `NormalizedForeignCallBinding`, structural type declarations, a layout closure and a scalar-closing closure; returns `Result<Option<DescriptorInputGuard>, &'static str>`.
4. How it decides. `Ok(None)` means "not a descriptor read"; `Err` means "is one but invalid". Keys: the call's system-call kernel number or hosted/import locator mapped by `runtime_abi::descriptor_system_call`/`descriptor_import` (`:194-201`, a table lookup, not a name compare); `access == MutableBorrow` (`:139`); non-empty `opaque_regions()` (`:146`); exact scalar types `i32, u{32|64}` result `i{32|64}` by ABI (`:150-160`); entry plan validated against a `CallSignature` (`:171-182`). Prefix: the minimum byte offset of any opaque region, discriminator, or non-trivial field (`:210-268`), then contiguous coverage by intervals (`:281-292`).
5. Cases by shape. Declaration form requires exactly one structural parameter, `parameter_order == [Scalar, Structural, Scalar]`, `NormalReturn`, no content guarantees, result `Unit` or `Scalar` (`:32-50`). Call form requires one structural argument and exactly two scalar arguments at parameter indexes 0 and 2 (`:79-86`). Comment at top (`:2`): "may alter only an independently reconstructed scalar prefix". Prefix recipes accept only boolean, IEEE float and non-address whole-byte integers (`:307-314`).
6. Same job elsewhere. Both public functions call `derive`. The declaration form and call form are called from 4 sites (below).
7. Unused/one-caller. `descriptor_input_guard_with_closed_scalars`: 3 call sites (`legalization_descriptor_inputs.rs:144`, `references.rs:1570`, and definition); `descriptor_input_guard_for_call_with_closed_scalars`: 3 (`legalization_target.rs:3360`, `unit.rs:959`, definition). Grepped over `source-rust`.
8. Stated limits. `:296-297`: "The public classifier accepts independent raw layout metadata." (note on `scalar_prefix_byte_width`).
9. Fail-open risks. `Ok(None)` for every non-matching shape: the caller treats `None` as "no guard", so a descriptor-reading call that fails the shape test (for example a different parameter order) simply gets no guard. Classification: open; I did not trace the callers' handling of `None` at `unit.rs:959` / `legalization_target.rs:3360`. In `preserves_boundary` (`legalization_descriptor_inputs.rs:144-156`) `None` or `Err` both map to `false` (conservative).

## S4/emission_facts.rs (171 lines)

1. Purpose. Projects, from the optimized abstract program, the per-function facts that later emission still needs after the executable graph is dropped.
2. Shape. `project` `:10` returns `SemanticEmissionFacts`; `function` `:30` builds `EmissionFunctionFacts` (source occurrences, descriptor producers, primitive locals, scalar call results, RAM obligations).
3. I/O. Reads `OptimizedAbstractProgram` and `AdmittedLoweringFacts`; output is one `SemanticEmissionFacts`.
4. How it decides. One `match` per output list on the `AbstractOperation` variant (`:49-65`, `:72-107`, `:114-126`, `:127-145`, `:153-162`).
5. Cases by shape. Descriptors only for `ByteSequenceSubslice`, `ElementViewSubslice`/`EstablishElementView`, `EstablishByteSequenceLiteral` (`:72-107`). RAM obligations for `EstablishAccessReference`, `BoundaryCall`, `ExternalCompletion` (`:153-162`).
6. Same job elsewhere. Not in this unit.
7. Unused/one-caller. `project` has one caller, `target_lowering/mod.rs:379` per the crate map (not re-grepped).
8. Stated limits. None.
9. Fail-open risks. Unlisted operations contribute no row (`_ => None`); if a later stage needs a fact for an operation kind not listed here the fact is absent. Open (not traced).

## S4/lib.rs (72 lines)

Crate root. Declares `descriptor_input`, `emission_facts`, `preparation`, `target_lowering`; exports `LoweringError`, `OptimizedTargetLoweringRequest`, `lower_native_body` and a `replay` module (`:13-19`). `NativeBodyPlan` `:27` holds private fields (`selection`, `facts`, `private_references`) with a `compile_fail` doctest (`:22-26`). Accessors `semantic_facts`, `original_for_audit`, `release_original_for_audit`, `private_references`, `selections`, `into_selection`, `execution_plan`. No decisions.

## S4/preparation/callable_invocations.rs (97 lines)

1. Purpose. Computes, once per optimized unit, which callable values (function pointers, callbacks) can be invoked from the entry and from each other root, and seals that into `NativeExecutionFacts` for the target plan.
2. Shape. `PreparedCallableInvocations` `:9` (psi, optimization_unit, work_schedule, entry, `invocations`, `entry_invocations`); `new` `:22`; `for_target` `:61`; `callable_program_with_providers` `:91`.
3. I/O. Reads the unit and `NativeCallableBindings`; calls `terminal_semantics::callable_values::reachable_callable_invocations_with_roots` for the entry (`:29`) and for each other root (`:44`).
4. How it decides. `for_target` compares four identities (psi, unit identity, work schedule, entry) and refuses on any difference (`:68-76`).
5. Cases by shape. None.
6. Same job elsewhere. The same computation is written out by hand in `legalization.rs:827-861` (`verify_execution_plan`): call `callable_program_with_providers`, `reachable_callable_invocations_with_roots` for `[unit.entry]`, then each other root, then `NativeExecutionFacts::seal`. Grep for `reachable_callable_invocations_with_roots` outside `terminal-semantics`: this file (2), `legalization.rs` (2), plus definition in `psi/semantics/.../reachable.rs:342,345`. `PreparedCallableInvocations::new` call sites: `legalization.rs:513` (test-only fn), `sum_layout_selection.rs:497`, `target_lowering/mod.rs:440`.
7. Unused/one-caller. `callable_program_with_providers`: 2 uses (this file `:26`, `legalization.rs:828`).
8. Stated limits. None.
9. Fail-open risks. None seen.

## S4/preparation/installed_interrupts.rs (148 lines)

1. Purpose. Joins the target operations to the separately admitted interrupt-installation cohort, and in the replay path checks each interrupt operation against its source boundary call.
2. Shape. `PreparedInstallationSources` `:8` (rows of `BoundaryCall` operations keyed by `(machine, psi_operation)`); `authorize_target` `:39`; `validate_target` `:118`.
3. I/O. Reads `TargetOperationPlan`, `TerminalModule`, `AdmittedInstalledInterruptCohort`, and (validate) the `AbstractOperationPlan`. Returns `Ok(())` or `LegalizationError::InstalledInterruptAssociation`.
4. How it decides. Keyed on `(machine, psi_operation)` identity: `scopes.get(...)` must return exactly one scope (`:82-87`); scope boundary, arguments, validator and work callee must equal the target row's (`:88-99`); the module's boundary declaration must report `StaticInterruptOperation::Install` (`:100-110`). Cohort psi identity must equal target psi (`:48-53`).
5. Cases by shape. Only `StaticInterrupt` and `InstallStaticInterrupt` rows are checked; `_ => {}` (`:112`). `calls[0]`/`calls[1]` are indexed positionally as Validate and Work (`:90-94`).
6. Same job elsewhere. `interrupt_installation.rs` is a second check on the install rows (below); both read `PreparedInstallationSources`.
7. Unused/one-caller. `authorize_target` called from `legalization.rs:660` and `:735`... see `validate_target` `:124`; `PreparedInstallationSources` has 2 users (this file and `interrupt_installation.rs`) per grep.
8. Stated limits. Doc at `:37-38`: "Source-to-target operand preservation belongs to the independent audit."
9. Fail-open risks. `_ => {}` skips every other operation kind; stated by scope (only interrupt rows matter). In `validate_target`, the source check is `!sources.boundary_calls(...).iter().any(...)` and the `if let ... && !any` form means a row with no source boundary call fails (`:141-145`). None seen.

## S4/preparation/interrupt_installation.rs (47 lines)

1. Purpose. For each `InstallStaticInterrupt` target row, requires a matching source `BoundaryCall` with the same operation id, boundary, no scalar arguments and the same structural arguments.
2. Shape. One function `validate_target` `:4`.
3. I/O. `TargetOperationPlan` and `PreparedInstallationSources`; returns `LegalizationError::InstalledInterruptAssociation` with "installation changed its authenticated source operation" (`:42`).
4. How it decides. Identity equality on `(psi_operation, boundary)`, `scalars.is_empty()`, and `structural_arguments.as_slice() == arguments` (`:36-37`).
5. Cases by shape. Only `InstallStaticInterrupt` rows (`let ... else { continue }`, `:19-27`).
6. Same job elsewhere. `installed_interrupts.rs:135-146` does the equivalent check for `StaticInterrupt` rows; `installed_interrupts.rs:147` calls this.
7. Unused/one-caller. 1 caller (`installed_interrupts.rs:147`).
8. Stated limits. None. 9. Fail-open: none seen.

## S4/preparation/legalization.rs (1068 lines)

1. Purpose. Builds the `PreparedExecutionPlan` from retained target operations plus the optimized unit's facts (`prepare_execution_plan`), and provides the independent replay (`verify_execution_plan`, `verify_target_operation_plan_with_closed_module`) that rederives the plan another way and compares.
2. Shape. Modules `model` `:8` (`LegalizationError` `:34`, validator id `b"omega.terminal-target-legalization-independent-replay.v75"` `:13`), `obligations` `:205` (`retained_fact` `:239`, `matching_fact` `:250` binary search), `runtime_indices` `:278` (`retained_operands` `:291`, `operands` `:317`, `projection` `:364`, one test `retained_index_needs_only_target_row_and_exact_admitted_fact` `:399`). Functions: `PreparationEvidence` `:495`, `legalize_target_operations` `:503` (`cfg(test)`), `surviving_installations` `:536`, `entry_scalar_codecs` `:560`, `function_abis` `:603`, `prepare_execution_plan` `:651`, `verify_target_operation_plan_with_closed_module` `:703`, `audit_target_operation_plan_with_module` `:764`, `verify_execution_plan` `:804`, `derive_expected_functions` `:953`, `validate_call_descriptor_origins` `:1027`.
3. I/O. Reads the target plan, `PreparationEvidence` (unit identity, work schedule, accepted-obligation facts, ownership-frontier facts), boundary origin facts, callable invocations, interrupt cohort, `ModuleTypes`. Writes a sealed `PreparedExecutionPlan` (`:699`).
4. How it decides. Proof-fact lookup is by the exact key `(machine, owner, obligation)` via `binary_search_by_key` (`:259-262`); a missing fact returns `LegalizationError::custody()` (`:247`). Function ABIs are sorted by machine; two with one machine refuse (`:633-639`). Replay compares published plan to derived plan row by row and names the first difference by function (`:930-945`). Pairing of abstract/optimized function per target function requires exactly one of each (`:1007-1014`).
5. Cases by shape. `entry_scalar_codecs` handles only `BorrowedScalarReference` placements and requires a primitive-scalar referent (`:576-591`); others return `Some(0)` (`:577`). `runtime_indices::operands` accepts index scalar types that close to an integer of at most 64 bits (`:329`, "integer.bits() <= 64").
6. Same job elsewhere. `verify_execution_plan:827-861` repeats `PreparedCallableInvocations::new` (see `callable_invocations.rs` item 6). `prepare_execution_plan` calls `source::derive_target_function_rosters` and the replay calls `derive_expected_functions` -> `derive_source_function_rosters`: two derivations compared on purpose.
7. Unused/one-caller. `legalize_target_operations` and `audit_target_operation_plan` are `#[cfg(test)]`. `accepted_fact`/`facts`/`references_obligation` re-exported `:479`. `#[allow(unused_imports)]` on five re-exports (`:481-491`) hides unused imports. `prepare_execution_plan` is called from `target_lowering/mod.rs:448` (crate map).
8. Stated limits. Doc `:643-649`: "A fact or type the plan needs and cannot find refuses the program here". Doc `:793-803`: "Ordinary compilation never calls this; it runs on an explicit validator request and in tests." `:1026`: "Independent source correspondence, never called by ordinary construction."
9. Fail-open risks. `entry_scalar_codecs` returns `Ok(Some(0))` for non-reference parameters (`:577`) and `Ok(None)` (via `and_then`) when no codec exists; its doc says "Unsupported conversions stay explicit for internal-only calls" (`:558-559`). Stated. Replay-only checks are skipped in ordinary builds (stated, `:802-803`), so a defect in `prepare_execution_plan` is not caught by default; that follows the project's "verify at trust boundaries" rule rather than being a bug in this file.

## S4/preparation/legalization_control.rs (475 lines)

1. Purpose. Projects each target control block's terminator (return, jump, conditional, case, crash, external completion, invoke-no-normal) into a `LegalizedScalarTerminator`, attaching retained accepted-obligation facts to runtime indices and cleanup calls.
2. Shape. `home_source` `:19`, `bindings` `:36`, `successor` `:63`, `structural_arguments` `:84`, `project` `:111`.
3. I/O. Reads `TargetControlBlock`, structural parameter declarations, accepted-obligation facts, `ModuleTypes`; returns `LegalizedScalarTerminator`.
4. How it decides. Every proof lookup goes through `retained_operands` keyed on `(machine, owner = edge or block, obligation)` (`:52-57`, `:100-105`, `:298-303`). `StructuralCase` refuses (`custody()`) if any pointer slot is a scalar descriptor, if `byte_offset + layout.shape.byte_size` exceeds `region_byte_size` or overflows, or if `cases.len() != layout.cases.len()` (`:289-296`). Cleanup calls attach to the jump/conditional/case edge whose id equals `owner.edge()`; otherwise `custody()` (`:443-472`).
5. Cases by shape. Comment `:288`: "Scalar-descriptor chases require a different lowering." Returns with cleanup calls are wrapped as `ReturnCleanup` (`:440-442`). A cleanup call on a `Crash`, `ExternalCompletion` etc. falls to `_ => return Err(custody())` (`:470`). A cleanup call that carries `source` metadata is refused (`:404-408`).
6. Same job elsewhere. `legalization_instruction.rs` does the instruction half with the same `retained_operands` helper.
7. Unused/one-caller. `project` is `pub(crate)`; re-exported by `legalization_source.rs` (crate-map not re-grepped).
8. Stated limits. `:288` quoted above.
9. Fail-open risks. None seen; every absent fact is `custody()` error.

## S4/preparation/legalization_descriptor_inputs.rs (158 lines)

1. Purpose. Caches, per audit, the two descriptor catalogs (view and reference) that functions are checked against, and decides whether a boundary call preserves descriptor custody.
2. Shape. `DescriptorInputs` `:14` (`OnceCell` for `views`, `references`); `new` `:23`; `views` `:47`; `references` `:62`; `operation` `:78`; `preserves_boundary` `:96`.
3. I/O. Reads `TargetOperationPlan`, `NativePlan`, `NativeUnit`; calls `optimized_catalog_graph` and `optimized_reference_catalog_graph_with_boundary_preservation` from `optimization_unit_semantics::descriptor_results`.
4. How it decides. `preserves_boundary` is `false` unless: node is `BoundaryCall` with empty completion claim sources and receipts (`:113-115`); a `NormalizedForeignCall` target row exists for `(caller.machine, psi_operation)` (`:116-127`); the boundary declaration is unique and its scalar/structural argument counts and access modes equal the call's (`:128-143`); and `descriptor_input_guard_with_closed_scalars` returns `Ok(Some(_))` (`:144-156`).
5. Cases by shape. Only boundary calls that are descriptor-input-guarded foreign reads preserve custody.
6. Same job elsewhere. `descriptor_input.rs:62` does the call form of the same guard.
7. Unused/one-caller. `DescriptorInputs::new` one caller (`legalization.rs:993`, grep).
8. Stated limits. Header (`:11-13`) explains the view vs reference catalogs differ in boundary rules.
9. Fail-open risks. `operation` (`:78-94`) uses `.ok().map(...)`: if `selected_boundary_operation` errors, it returns `None`; the effect on the catalog graph is not traced. Open.

## S4/preparation/legalization_function_inputs.rs (193 lines)

1. Purpose. Pre-sorts, once per function, the value definitions, structural declarations, arrival parameters, trivial affine locals, structural results and byte literals so later queries are binary searches.
2. Shape. `FunctionInputs` `:14`; `new` `:25`; queries `value_type` `:76`, `trivial_affine_local` `:84`, `arrival_parameter` `:95`, `source_result` `:136`, `byte_literal` `:156`.
3. I/O. Reads `PsiOptimizationFunction`; output queries return refs or `Option`/`LegalizationError`.
4. How it decides. Uniqueness: each query returns `None`/`custody()` unless exactly one row matches by `PlaceId` (`:91-92`, `:104`, `:110-112`, `:146-148`, `:162-164`, `:177`). `arrival_parameter` also requires declaration kind `Parameter` with matching position and presence in `structural_parameters`, or `BlockParameter` of a non-entry block with matching position (`:115-132`). `byte_literal` requires kind `ByteSequenceLiteral`, matching declaration, matching type id and `is_borrowed_byte_view()` (`:165-191`).
5. Cases by shape. Doc at `:152-155` states the refusal.
6. Same job elsewhere. `semantic_analyses.rs` `FunctionContext::build` (`:624`) builds a related root table for place access (different tables).
7. Unused/one-caller. `FunctionInputs` is used widely by `legalization_scalar_graph.rs` (about 40 signature sites in the first 60 lines of an uncapped content grep; the grep was capped at 60 rows so the total is "at least 60").
8. Stated limits. None. 9. Fail-open: none seen (duplicates return `None`/error).

## S4/preparation/legalization_instruction.rs (1200 lines)

1. Purpose. Projects each ordinary target instruction (`TargetUnitOperation`) into a `LegalizedScalarInstruction` of kind `Kind::*`, attaching accepted-obligation fact identities where the operation had a proof obligation.
2. Shape. `project` `:15` (a ~620-line `match` `:36-655`), `result` `:661`, `argument_indices` `:669`, `sum` `:682`, `integer_operand` `:688`, `boolean_operand` `:692`, `scalar` `:703`, `arithmetic_instruction` `:917`. Two tests `:1044`, `:1128`.
3. I/O. Reads the instruction (with `source` metadata), accepted-obligation facts, `ModuleTypes`, target plan, geometry. Calls helpers under `legalization_source::scalar_graph::instruction` (`call_instructions`, `project_reference`, `storage_instructions`).
4. How it decides. Keyed on `TargetUnitOperation` variant. A missing `instruction.source` is an error (`:23-25`). Proof facts are looked up by `(machine, operation, obligation)` (`:31-35`). `SelectMachine` requires the result home to match the declared result value, scalar type and `Callable` shape (`:200-211`). Arithmetic: the exact policy always needs a fact (`:1010-1012`); wrapping divide/remainder needs a fact and `supports_wrapping_division(integer)` (`:950-958`); saturating needs a carrier type (`:938-944`).
5. Cases by shape. `scalar` (`:703-914`) lists `Scalar::Integer` expressions (byte read, lengths, immediate, bitwise, widen, exact cast, shift, arithmetic, trapping) and `Scalar::Boolean` expressions (immediate, not, equal, integer comparisons, byte predicates); `_ => return Err(Error::custody())` (`:910`, `:912`). Boundary settlement realizations are limited to `HostedReadByte` and `HostedWriteByteI32` (`:531-554`); other realizations are `custody()`. `InstallStaticInterrupt` indexes `arguments[0]` and `arguments[5]` for the two calls (`:106`) and `calls` as a two-element array (`:131`). `CleanupCall`, `Continue`, `Return` as instructions are refused (`:652-654`).
6. Same job elsewhere. `legalization_control.rs` does terminators; the two files share `retained_operands`.
7. Unused/one-caller. `project` re-exported through `legalization_source.rs:1958` (one site).
8. Stated limits. Test name states: "atomic_boolean_storage_keeps_its_closed_byte_without_a_source_u8" (`:1044`).
9. Fail-open risks. The `IntegerConstant`, `BooleanConstant`, `IeeeFloatConstant` rows are accepted with only the operand value (`:642-651`), no fact needed (traced: they have no obligation). `Operation::Continue`, `Return`, `CleanupCall` reject. None seen that accept an unknown shape.

---

## Coverage

Read in full (14,946 lines total, all 20 ranges): `S3/ranked_rewrites.rs` 1-1284, `S3/rules.rs` 1-1296, `S3/sccp.rs` 1-1235, `S3/semantic_analyses.rs` 1-2371, `S3/specialization.rs` 1-552, `S3/validation.rs` 1-3151, `S3/value_ranges.rs` 1-106, `S3/verified_unit.rs` 1-872, `03.../src/lib.rs` 1-11, `S4/descriptor_input.rs` 1-439, `S4/emission_facts.rs` 1-171, `S4/lib.rs` 1-72, `S4/preparation/callable_invocations.rs` 1-97, `installed_interrupts.rs` 1-148, `interrupt_installation.rs` 1-47, `legalization.rs` 1-1068, `legalization_control.rs` 1-475, `legalization_descriptor_inputs.rs` 1-158, `legalization_function_inputs.rs` 1-193, `legalization_instruction.rs` 1-1200.
Skimmed: none. Not read: none.
Not read at all because outside the unit: every other file of both crates (for example `pass_manager.rs`, `mod.rs`, `legalization_scalar_graph.rs`); only grep hits in them were used.

## Five things a maintainer most needs to know about this unit

1. Stage 03 admission functions in `validation.rs` and `ranked_rewrites.rs` are closed shape lists: `admit_member_node` (`ranked_rewrites.rs:542-711`) tries about 20 operation shapes in an `if/else if` chain, and each shape has its own `admissible_*` predicate in `validation.rs` that re-checks provenance, uses, definitions and ownership of the node. Unlisted shapes never move. Propose and validate run the same planner (`:453`, `:1022`), so they agree by construction rather than independently.
2. `reconstruct_custody` (`ranked_rewrites.rs:394-409`) does nothing when the unit has no ranking certificates, and `scalar_case_result_contained` (`validation.rs:2536-2543`) lets any operation outside the component pass if its place references cannot be listed. Both have no comment saying why.
3. `verified_unit.rs` gives accepted obligations a zero proof fingerprint when no evidence seal exists, and says so (`:436-445`); the proof kernel's acceptance is "taken on faith" (`:442-443`). Both tests there are `#[ignore]`d. It also reads the `OMEGA_PROOF_MEASUREMENTS` env var inside the compile stage (`:95`).
4. Stage 04's preparation is derived twice on purpose: `prepare_execution_plan` (`legalization.rs:651`) builds the plan and `verify_execution_plan` (`:804`) rebuilds it from the abstract program and the unit, but replay runs only on explicit request (`:802-803`). Every missing proof fact is an error (`custody()`); lookups are by `(machine, owner, obligation)` binary search (`:259`).
5. The callable-invocation fixed point is written twice: `callable_invocations.rs:26-50` and again inline in `legalization.rs:827-861`. Several small "unique row or refuse" lookups are repeated per file (`semantic_analyses.rs`, `ranked_rewrites.rs`, `legalization_function_inputs.rs`).

## What you are unsure about

- Whether callers of `descriptor_input_guard_*` (`target_lowering/unit.rs:959`, `legalization_target.rs:3360`, `references.rs:1570`) treat `Ok(None)` as "no guard needed" or as an error; I did not read those sites.
- Whether any caller can reach `reconstruct_custody` with a cycle component but no ranking certificates.
- Callers of `project` (emission_facts) and `place_aliases`/`ownership_frontiers` were not grepped; I relied on the crate map for the first.
- `RuleRegistryError::UnsupportedOptimizationCombination` appears unused inside this file; I did not grep the rest of the crate.
- I did not count the exact number of `FunctionInputs` call sites (grep output was capped at 60 rows).
