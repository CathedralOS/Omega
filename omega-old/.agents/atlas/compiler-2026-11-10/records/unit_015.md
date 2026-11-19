# Unit 15 notes

Files covered (all under `omega-rust/omega/compilation/pipeline/`):
1. `01_abstract-operations-to-abstract-operations/src/pass_manager.rs` (1552)
2. `01_.../src/proof_check_elision.rs` (910)
3. `01_.../src/publication.rs` (380)
4. `01_.../src/ranked_cycles_validation.rs` (961)
5. `01_.../src/ranked_rewrites.rs` (1278)
6. `01_.../src/rules.rs` (1284)
7. `01_.../src/sccp.rs` (1246)
8. `01_.../src/semantic_analyses.rs` (2374)
9. `01_.../src/specialization.rs` (552)
10. `01_.../src/validation.rs` (3171)
11. `01_.../src/value_ranges.rs` (290)
12. `02_abstract-operations-to-target-operations/src/control_flow.rs` (3104)
13. `02_.../src/control_flow_calls.rs` (2162)
14. `02_.../src/control_flow_operations.rs` (1425)

(Prefix `P1` = `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/`; `P2` = `.../02_abstract-operations-to-target-operations/src/`.)

---
## 1. P1/pass_manager.rs (1-1552) — read in full

**Purpose.** Runs the selected Psi optimization passes on a `PsiOptimizationUnit` to a fixed point, one pass step at a time, checks work budgets and a per-pass "convergence measure" that must strictly fall on every commit, and records baseline or externally supplied (replayed) decisions into manifests and a transformation ledger. Also defines the session type that wraps a verified unit and the error enums for the run.

**Shape.**
- `mod accounting` (:8): `register_revision` (:22, errors on a repeated unit identity = oscillation), `build_pass_manifest` (:38), `charge` (:78, errors when counter == limit), `add_usage` (:90, checked adds), `convergence_measure` (:220), helper counters (:118-:326).
- `mod baseline::choose_baseline` (:333): min by `(predicted_cost_delta, candidate)`, chosen only if delta < 0 else `Skip(NotProfitable)`.
- `mod execution` (:346): `run_registries` (:385) -> `run_registries_inner` (:402) loops over `PsiPassStep`s; `run_unit_inner_with_retention` (:539) is the generic rewrite-registry loop; `run_loop_invariant_scalar_motion_step` (:714) is a separate hand-written loop for the one non-registry pass; `chain_ledger` (:888), `finish_loop_invariant_scalar_motion_step` (:916), `loop_invariant_member_node_count` (:942), `validate_candidate_contract` (:963).
- `mod external_policy` (:991): `candidate_features::derive` (:999), `context::expected_context`/`mismatch` (:1024/:1041), `recording::external_points_from_manifest_decisions` (:1072), `replay::ExternalDecisionReplayCursor` (:1138; `new` :1144, `choose` :1166, `require_exhausted` :1201).
- Top-level: `baseline_psi_cost_model_identity` (:1267), `baseline_outcome_verdict` (:1271), `VerifiedPsiOptimizationSession{input, unit, cycle_components}` (:1287; `new` :1294, `from_transformed` :1324), `OptimizationRun` (:1409), `OptimizationRunError` (:1463, 28 variants), `CandidateContractAxis` (:1503), `ExternalDecisionReplayError` (:1512), `ExternalDecisionContextAxis` (:1538).

**Inputs/outputs.** In: a `VerifiedPsiOptimizationSession`, `OptimizationSelections`, `PsiOptimizationSelections`, `&[PsiPassStep]` (from `crate::rules`), per-pass `OptimizationWorkBudget`, optionally an `ExternalDecisionLog` (`run_registries_inner` :408; the public wrapper `run_registries` always passes `None`, :398). Out: `OptimizationRun` with commits, validated candidates, usage, baseline and external decision logs, pass manifests, ledger, identity bundle (:514-:527).

**How it decides.**
- Per iteration, rules in a registry are tried in order; the first rule that yields validated candidates and whose outcome is `Choose` wins and the loop restarts (`break` :660). `Skip` continues to the next rule (:662). Key: rule order in the registry and `predicted_cost_delta` (:338).
- Baseline accepts only candidates with `predicted_cost_delta < 0` (:339).
- Fixed-point termination: a pass ends when no rule chooses a candidate (:665).
- Convergence measure is selected by comparing the registry's pass identity to byte-string identities `omega.psi-pass.<name>.vN` (:224-:260); an unrecognised pass falls through to `integer_evaluation_operation_count` (:259). So the measure is keyed on pass-identity bytes, with a literal version number in each (e.g. `control-flow-cleanup.v13`, `proof-check-elision.v12`, `global-value-numbering.v14`).
- Commit is rejected if the measure does not strictly decrease (:692).
- Replay: legal candidate set must equal the recorded one after sorting by candidate id, and input and rule must match (:1180-:1185).

**Cases by shape.**
- `convergence_measure` is a fixed list of seven passes plus a default (:244-:260): cfg cleanup -> `control_flow_structure_count`; copy propagation -> `block_parameter_count`; dead pure scalar -> `dead_total_scalar_operation_count`; proof elision -> `proof_certified_scalar_operation_count`; GVN -> total node count; state specialization -> `dispatch_chain_depth_measure`; representation specialization -> `representation_observation_count`; everything else -> `integer_evaluation_operation_count`.
- `PsiPassStep` has exactly two shapes: `RewriteRegistry` and `LoopInvariantScalarMotion` (match :451-:470). The loop-invariant step has its own loop that duplicates the registry loop (see "Same job" below).
- `dispatch_chain_depth_measure` only counts blocks ending in `Conditional` (:298); cycles return 0 (`!visiting.insert` :290).

**Same job elsewhere.** `run_loop_invariant_scalar_motion_step` (:714-:886) repeats the structure of `run_unit_inner_with_retention` (:539-:712): charge iterations, propose, charge candidates, validate-contract, dedupe, charge validation, validate, record features, choose (replay or baseline), record outcome, push decision records, apply, register revision, check measure, charge commit, push commit. Both read. Suspected other site for contract-axis checking: none seen.

**Unused / one-caller.** `run_registries` (:385): `Grep -c "run_registries\b"` over omega-rust gives 3 in pass_manager.rs (def, re-export, use) + 2 in lib.rs, so one outside caller file (lib.rs). The `supplied_external_decisions` parameter of `run_registries_inner` is always `None` from `run_registries` (:398); grep `run_registries_inner` has no other caller in this file (read) -> the replay branch (:433-:439, :489-:501, :624-:627, :797-:804) is reachable only if another caller exists; I grepped `run_registries\b` only (the `_inner` name was not grepped separately: suspected dead from the outside). `convergence_measure` (:220): 2 call sites, both here (grep). `publish_optimization_run` is in publication.rs (below).

**Stated limits.** None stated beyond the module header.

**Fail-open risks.**
- `convergence_measure` default arm (:258-:260): any pass whose identity is not in the list of seven gets `integer_evaluation_operation_count`. A new pass that does not reduce integer-evaluation ops would fail closed (`NonDecreasingConvergenceMeasure` :693) rather than accept; so fail-closed (traced by reading :692).
- `dispatch_chain_depth_measure` returns 0 for revisited block on a cycle (:290): measure may undercount; a non-decreasing check then errors, fail-closed. Traced.
- `run_registries_inner` `.expect("the canonical Psi selection is duplicate-free")` (:417) is a panic, not an accept.
- `build_pass_manifest` uses `assert!` for "every committed candidate has an applied manifest decision" (:51-:57): panics rather than errors. Stated.
- `build_pass_manifest` returns `Ok(None)` when `step.pass()` is None (:46-:48); caller then errors with `MissingPassManifest` (:475). Fail-closed. Traced.

---
## 2. P1/proof_check_elision.rs (1-910) — read in full

**Purpose.** Defines the "proof-check elision" pass: eleven rules that rewrite or delete scalar integer operations whose arithmetic obligation is already proven (identity laws like `x+0`, `x*1`, `x*0`, `x/1`, `0/x`, shifts by zero count, shift of zero, self-subtract, self-divide, self-remainder, remainder by one, signed remainder by -1, exact arithmetic shift right of -1), plus a twelfth rule that deletes dead unused proof-certified scalar nodes.

**Shape.** `mod identity_rewrite` (:25): `propose_proof_certified_integer_constants` (:40, whole-node replacement by constant from a `classify` fn), `propose_proof_certified_scalar_identities` (:101, replacement by an existing operand given a constant fact), `ProofCertifiedScalarIdentityShape` (:196). Rule modules: `negative_one_shift_right` (:205), `proof_certified_dead_scalar_elimination` (:268, `classify` :287), `remainder_by_one` (:320), `scalar_identity_rules` (:351, macro :367, five shape fns :409-:632), `self_divide` (:634), `self_remainder` (:670), `self_subtract` (:703), `signed_remainder_by_negative_one` (:742). `proof_check_elision_rule_contract` (:775): version 1, invalidates UseDefinition+EffectSummaries, safety class `ProofCertified`. `built_in_registrations` (:804): 12 registrations numbered 0..11. `propose_proof_certified_remainder_by_constant` (:833).

**Inputs/outputs.** Reads `RuleAnalysisView` products: `ScalarConstants`, `UseDefinition`, `EffectSummaries`, `ValueLiveness` (dead rule), the unit's functions/blocks/nodes. Produces `PsiRewriteCandidate`s (`new_proof_certified_integer_constant_replacement`, `new_proof_certified_scalar_identity`, `new_literal_proof_certified_integer_constant_replacement`) with predicted delta -1.

**How it decides.**
- A node qualifies only if its effect row `is_pure_scalar_baseline(unit.identity)` (:64, :120, :874) and `accepted_obligation_fact(...)` is Ok (:67, :152, :877); otherwise silently `continue`.
- Operand constant matched by value: `constants.literal_integer_constant(machine, operand)` equals expected `IntegerValue` (:138-:142, :861-:870).
- Result must have a use (`use_definitions.uses` scan :56, :147, :855) -> unused results are left for dead-code rules.
- Type check on replacement and (for arithmetic) identity operand equals result scalar type (:126-:136).
- Keyed on value ids, operation ids, types; no spelling.

**Cases by shape.** Fixed lists:
- Divide-by-one: all three overflow policies (:423-:433).
- Multiply-by-zero: Exact only (:456).
- Identity laws (`proof_certified_scalar_identity_shapes` :485): Exact only; Add (zero left/right), Subtract (zero right), Multiply (one left/right); Divide/Remainder -> none (:512); shifts with Exact policy by zero count.
- Zero dividend: all three policies for Divide and Remainder (:568-:574).
- Zero value shift: Exact policy only (:604).
- `-1` shift right: Exact, Right, `IntegerCarrier::Fixed`, Signed only (:224-:228).
- Self-divide: Fixed carrier, and not signed 1-bit (:656-:660).
- Self-remainder: no type restriction (:693).
- Self-subtract: Exact policy only (:728-:731).
- Remainder by constant: Fixed carrier (:853), `signed_only` for -1 rule (:854), `operands.left == operands.right` excluded (:852), constant must be admitted by type (:867).
- Dead scalar elimination classifies three operation shapes: `IntegerExactCast`, Exact `IntegerShift`, `IntegerArithmetic` with an obligation (:289-:311).

**Same job elsewhere.** `propose_proof_certified_remainder_by_constant` (:833) is a third proposer beside the two in `identity_rewrite` with near-identical loop structure (pure-baseline check, obligation fact, NodeLocation, candidate). Also the nodes-with-use / pure-scalar / obligation filter is repeated at :56-:71, :113-:158, :847-:880.

**Unused / one-caller.** `built_in_registrations` (:804) — one caller expected in rules.rs (checked when I read rules.rs).

**Stated limits.** None.

**Fail-open risks.** Missing effect row: `is_none_or(|row| !pure)` -> skip (rejects) (:64). Missing constant: `continue`. All fail-closed (traced by reading). The `expected_constant` closure for remainder-by-one is `constant == scalar_type.one()` (:346), keyed on value. "none seen" for accept-on-unknown.

---
## 3. P1/publication.rs (1-380) — read in full

**Purpose.** Turns a finished `OptimizationRun` into an `AbstractOperationPlan` (the executable form the later stage consumes) and a sealed pre-physical optimization manifest, keeping all the evidence (commits, decisions, ledger, identity bundle) alongside.

**Shape.** `OptimizedAbstractProjectionError` (:13, 6 variants); `source::project_plan` (:118) rebuilds the plan from `PsiOptimizationUnit` functions via `source::function::project` (:35); `validate_function_roster` (:140); `project_parameter` (:168); `publish_optimization_run` (:206); `ValidatedOptimizedAbstractPlan` (:243) with accessors and `into_lowering_parts` (:351); `work_usage` (:372).

**Inputs/outputs.** In: `OptimizationRun`. Out: `ValidatedOptimizedAbstractPlan{plan: Arc<AbstractOperationPlan>, replay_input, replay_unit, evidence, validation, pre_physical_manifest}`.

**How it decides.** Roster check: source plan function count must equal active + pruned machines and each source function must be either the next active function (same machine) or have a `pruned_machines` custody row with matching ordinal and machine (:144-:164). Parameter check: definition site must equal the expected site (:173). Psi selections must equal `selections.for_phase(Psi)` (:209).

**Cases by shape.** None (no fixed shape lists).

**Same job elsewhere.** `work_usage` (:372) converts `OptimizationRunUsage` to `OptimizationWorkUsage` field-by-field; the same conversion is written inline at pass_manager.rs:66-72.

**Unused / one-caller.** `publish_optimization_run`: grep gives definition + lib.rs:119 import + lib.rs:153 call -> one call site.

**Stated limits.** None. **Fail-open risks.** none seen.

---
## 4. P1/ranked_cycles_validation.rs (1-961) — read in full

**Purpose.** Finds the cycles (strongly connected block groups) of each function, re-derives them independently from both Terminal Psi and the optimization unit, requires the two to agree, and replays the "unsigned countdown" ranking certificates (loops of the shape `while 0 < n { ...; n = n - 1 }`) from both sides requiring equality.

**Shape.** `mod components::cyclic_components` (:18; SCC with size > 1 or a self-loop). `mod countdown_ranking` (:30): `current::derive` (:203) / `derive_one` (:221) / `derive_from_header` (:239) over the optimization unit; `current::invariant_constants::{resolve :66, validate_canonical_preheader_suffix :135, sole_preheader :183}`; `terminal::project` (:418) / `project_one` (:431) over Terminal Psi; `rederive_exact_certificates` (:612). `mod graph` (:652): `CanonicalControlGraph`, `terminal_graph` (:686), `optimization_graph` (:731). `mod ordinary::rederive_components` (:777), `mod replay::rederive_exact_components` (:815), `mod topology` (:825). `validate_exact_ranked_cycles` (:906) -> `RankedCycleAdmission`. `ValidatedOptimizerCycleComponents` (:918), `ValidatedOptimizerRankingCertificates` (:945).

**Inputs/outputs.** In: Terminal Psi module (`input.context().module()`), seed `AbstractFunction`s, the `PsiOptimizationUnit`. Out: `OptimizerCycleComponentSnapshot` (components with id{machine, internal_edges}, members, entries, exits) and `OptimizerRankingCertificateSnapshot`.

**How it decides.**
- Components from Terminal Psi (via `terminal_semantics::control_graph::control_cycle_members` :844) must equal components from the unit's graph (:799); mismatch -> `RankedCycleTopologyMismatch`.
- Machines with no terminal cycles are skipped (:794-:796).
- Countdown certificate on the unit side: the header must end in `Conditional` whose condition is `IntegerLessThan(zero, rank_param)` with `rank_param` a block parameter of Fixed Unsigned integer type (:246-:281); a unique candidate header per component (:226-:236); the back edge jump passes `ExactIntegerSubtract` with obligation on the rank param (:322-:340); zero and one constants must be single-definition `IntegerConstant` nodes located in the original block or the unique preheader (:92-:94), with fixed provenance/definition shape (:107-:121).
- Terminal side: `TerminalRankedScc::Natural` rank data, exactly one `Strict` edge (:480), same structure.
- Terminal and current certificates compared by `!=` (:631).
- Keyed on block/value/operation ids and structural shape, not names.

**Cases by shape.** Only one ranking shape is recognised: unsigned fixed-width `0 < n` guard with `n - 1` descent (:246-:341, :435-:566). Terminal side returns `None` for components not matching (:441-:566); those components are dropped from `recognized` (:619-:624) and get no certificate. Signed ranks rejected (:464). Entries: `sole_preheader` requires exactly one entry edge (:187).

**Same job elsewhere.** `terminal_graph` (:686) and `optimization_graph` (:731) both turn control terminators into `CycleComponentEdge`s; same job on two representations. `countdown_ranking::current::derive_from_header` (:239) and `terminal::project_one` (:431) are two matchers of the same shape (cross-checked on purpose via :631). `cyclic_components` (:18) uses `crate::cycles::strongly_connected_components` (not read). Suspected: `blocks::shared_entry_source` in validation.rs duplicates `sole_preheader` (:183) — see validation.rs notes.

**Unused / one-caller.** `validate_exact_ranked_cycles` (:906): grep below in validation notes.

**Stated limits.** None.

**Fail-open risks.** A component with no recognised countdown shape is silently left without a certificate (`project` returns `Ok(None)` :428/:620; `recognized` excludes it :621). Whether a later consumer rejects uncertificated cycles was not traced in this file (open). The unit-side `derive_one` errors if any recognised component has no unit-side certificate (:230-:232), so mismatch in the recognised set is caught. Terminal `project` returns `None` also on any unexpected shape, meaning a cycle with a legit-but-different ranking is not an error here.

---
## 5. P1/ranked_rewrites.rs (1-1278) — read in full

**Purpose.** Implements one optimization, "loop invariant scalar motion": for each cycle component with a single preheader it finds nodes in the loop body whose results do not change across iterations (scalars, byte literals, records, arrays, cases, trivial affine locals, byte/place reads, subslices, several call shapes), moves them into the preheader, rebinds custody and ownership facts, and later independently re-derives the whole plan to validate a candidate.

**Shape.** `mod apply` (:31): `realize::realize` (:53), `operation_location` (:272), `refresh_coordinates_effects_and_facts` (:297), `validated` (:346), `reconstruct_custody` (:394). `mod propose` (:411): `all` (:423), `component_candidate` (:446), `PlanEvidence` (:492), `admit_member_node` (:541), `roots_relocate` (:712), `mutable_borrowers_relocate` (:737), `owned_consumers_relocate` (:752), `component_plan` (:767), `certificate_operations` (:974). `mod validate` (:992): `candidate` (:1000), `reconstruct_provenance` (:1066), `unique_source` (:1127). Top: `propose_/validate_/apply_loop_invariant_scalar_motion` (:1154-:1173), `contract()` (:1190, safety class `StructuralIdentity`, invalidates all analyses, requires EffectSummaries), `LoopInvariantScalarMotionError` (:1243).

**Inputs/outputs.** In: `VerifiedPsiOptimizationSession`. Uses `crate::validation::{invariant_calls, invariant_operations, member_blocks, place_observations, relocation_rewrites}` for the per-shape admission tests (those are in validation.rs). Out: `LoopInvariantScalarMotionCandidate` (list of `LoopInvariantScalarRelocation`), a new `PsiOptimizationUnit`, provenance rows, a one-record `PsiTransformationLedger`.

**How it decides.**
- Planning is a greedy fixpoint over component members, repeating until no new node is admitted (:823-:964). A node is admitted if its block is "guaranteed executed" and the preheader unconditionally enters the component (:550, :788-:792), and one admission test from a chain matches (:589-:704).
- Nodes producing storage also require `component_members_unobservable` (:569-:577).
- Place roots produced must have all mutable borrowers (and for restricted roots all owned consumers) also relocate (:853-:868).
- Validation: `*candidate != expected` where expected is a fresh plan recomputed from the session (:1038-:1047) -> candidate must equal the independently recomputed plan; also re-validates transformed unit and `reconstruct_custody` (:1049-:1054), and provenance must be non-empty (:1056).
- Nodes are matched by `PsiProvenance::Operation(operation)` as `provenance.first()` (:90, :283, :1139) — identity of source operation.

**Cases by shape.** `admit_member_node` is a long if-else chain over fixed shapes (:589-:704): scalar leaf; invariant place read; byte read; field byte read; subslice; byte literal; primitive local; record; scalar array; scalar case; trivial affine local; scalar call; crash-continuation call; unit call; crash-continuation unit call; structural scalar call; crash-continuation structural scalar call; structural call; crash-continuation structural call; fallback `invariant_scalar_operand_substitution`. Result mapping in `component_plan` (:869-:937) lists the same shapes; any other shape with zero definitions returns `CandidateMismatch` (:934). `shared_entry_source(component)` requires one preheader.

**Same job elsewhere.** `realize::operation_location` (:272) and `validate::unique_source` (:1127) both find the unique node by `provenance.first()`. `certificate_operations` (:974) vs ranking certificate shape in ranked_cycles_validation.rs:985. `apply::realize` has two nearly identical crash-continuation branches for structural and non-structural call (:152-:160).

**Unused / one-caller.** `LOOP_INVARIANT_SCALAR_MOTION_PASS_NAME` (:1175): grep later. `propose_loop_invariant_scalar_motion` etc. are used from pass_manager.rs:365-368 (read).

**Stated limits.** None in comments.

**Fail-open risks.** The `admit_member_node` fallback arm (:699-:704) does not decline unknown shapes but sends them to `invariant_scalar_operand_substitution`, which returns `Option`; `None` -> not admitted (traced by `?`). The `_ =>` in `component_plan` result match errors. `reconstruct_custody` only runs analyses if ranking certificates is non-empty (:397); for units without certified cycles the custody reconstruction is skipped (stated nowhere; traced: no certificates means no countdown analyses to check). No accept-on-missing seen.

---
## 6. P1/rules.rs (1-1284) — read in full

**Purpose.** The registry of built-in Psi optimization passes: names, rule contracts, macros that stamp out rule types, an ordered rule registry, the nine-entry pass catalog, shared helpers for dead-node elision and for fetching an accepted obligation, and three small rule modules (case-membership specialization, field-value specialization, state-argument specialization) plus copy propagation (redundant block parameter) and two dead-scalar rules. Re-exports other pass modules (`control_flow_cleanup`, `global_value_numbering`, `proof_check_elision`, `sccp`).

**Shape.**
- Pass-name byte constants (:8-:19): SCCP `...sparse-conditional-constant-propagation.v5`, CFG `control-flow-cleanup.v13`, copy `copy-propagation.v1`, dead-scalar `dead-pure-scalar-elimination.v2`, proof-elision `v12`, GVN `v14`, state spec `v1`, representation spec `v1`.
- `standard_rule_contract` (:21), `representation_specialization_contract` (:40).
- Macros `psi_rule_contract!` (:56), `psi_rule_impl!` (:100), `required_analysis!` (:114).
- `accepted_obligations::accepted_obligation_fact` (:129).
- `catalog` (:163): `PsiPassCatalogEntry` (:185), `PSI_PASS_CATALOG: [_; 9]` (:221), `BuiltInRuleRegistration` (:258), `assemble_built_in_registry` (:272).
- `node_elision` (:290): `DeadScalarShape` (:306), `propose_unproved_dead_scalar_nodes` (:320), `propose_proof_certified_dead_scalar_nodes` (:329), `propose` (:344), `node_elision_accounting` (:431).
- `registry` (:481): `RuleAnalysisView` (:496), `RuleProposalError` (:530), trait `PsiOptimizationRule` (:542), `RuleScheduleKey` (:553), `RuleRegistryError` (:568), `OrderedRuleRegistry` (:581).
- Rules: `case_membership_specialization` (:644), `copy_propagation::redundant_block_parameter` (:717), `dead_scalar_elimination` (:886; two rules), `field_value_specialization` (:1033), `state_specialization` (:1115).
- `PsiPassStep` (:1223: `RewriteRegistry` or `LoopInvariantScalarMotion`), `built_in_psi_steps_for_selections` (:1268).

**Inputs/outputs.** Selections -> `Vec<PsiPassStep>` (:1268-:1283). Rules read `RuleAnalysisView` products and the unit; they output `PsiRewriteCandidate`s.

**How it decides.**
- Registry ordering: registrations sorted by `schedule_ordinal` and `assert_eq!` that ordinals are exactly 0..n contiguous (:275-:282); mixed pass identities or duplicate rule identities rejected (:600-:610). Keyed on byte-string identities.
- `RuleAnalysisView::get` returns `None` if the kind is not in the rule's requested set, even when the product exists (:513).
- Dead scalar node proposal (:344): requires a following node in the same block (:360) whose provenance shares no source with this node (:363-:369); liveness row present and result not live at exit (:372-:378); effect row pure scalar baseline (:379). Missing liveness row or missing effect row -> skip (`is_none_or`).
- Redundant block parameter (:753): non-entry blocks; all incoming edge arguments equal one replacement different from the parameter and the replacement dominates the parameter's uses (:790-:803).
- Case-membership, field-value and state specialization: skipped for machines in `scc.cyclic_machines(unit)` ("frozen", :663, :1054, :1159), so cyclic machines are never specialized.
- `built_in_psi_steps_for_selections` (:1268): unsupported optimization (not in the 9-entry catalog) is an error (:1271-:1277); steps come out in catalog order, not selection order (:1278-:1283).

**Cases by shape.**
- Dead scalar classification: literal rule handles `IntegerConstant` and `BooleanConstant` only (:919-:941); the "unconditionally total" rule enumerates Boolean not/equal/or pair patterns, integer eq/less/le, bitwise not/and/or/xor patterns, widen, arithmetic with no obligation, wrapping shifts (:978-:1014).
- Pass step has exactly two shapes (:1223).

**Same job elsewhere.**
- The pass-identity byte strings (:8-:19) are repeated as literals in `pass_manager.rs:224-:243` (grep `omega\.psi-pass\.`: 8 in rules.rs, 7 in pass_manager.rs, 1 in ranked_rewrites.rs) — same identities written twice rather than using these constants.
- `dead_unconditionally_total_scalar_elimination::classify` (:978) and `pass_manager.rs::dead_total_scalar_operation_count` (:149-:176) both enumerate the same set of "total scalar" operations (one as a classifier, one as the measure). Read both.
- `proof_certified_dead_scalar_elimination::classify` (proof_check_elision.rs:287) and `proof_certified_scalar_operation_count` (pass_manager.rs:178) likewise.
- `integer_evaluation_operation_count` (pass_manager.rs:118) lists the sccp folds.

**Unused / one-caller.** `built_in_psi_steps_for_selections`: 1 use in lib.rs:181 (grep). `PSI_PASS_CATALOG`: used in rules.rs only. `WrappingIntegerAddConstantsRule` re-export is `#[cfg(any(test, feature = "test-support"))]` (:1265). `RuleRegistryError::UnsupportedOptimizationCombination` (:570) — not grepped (suspected unused in this unit).

**Stated limits.** None.

**Fail-open risks.** none seen. Skips go in the rejecting direction (missing analysis row -> skip; missing analysis product -> `MissingAnalysis` error :117).

---
## 7. P1/sccp.rs (1-1246) — read in full

**Purpose.** The sparse-conditional-constant-propagation rule family: 39 rules that replace an operation whose operands are known constants (or known value ranges) with a constant, one rule per operation/policy kind. The macro `constant_fold_rules!` (:8) stamps the rule struct and `PsiOptimizationRule` impl.

**Shape.** `boolean_constants` (:35; Boolean not/equal, integer equal/less/le over constants), `integer_binary_constants` (:186; 22 rules over arithmetic by policy, shifts, bitwise), `integer_binary_shapes::classify` (:367), `integer_cast_constants` (:466; `ExactIntegerCastConstantsRule`), `integer_unary_constants` (:547; widen, bitwise-not), `range_against_constant` (:639; 6 rules), `range_against_range` (:806; 3 rules). Shared: `built_in_registrations` (:1006; 39 entries ordinals 0..38), `constant_evaluation_contract` (:1050; pass SCCP v5, invalidates UseDefinition only), `propose_integer_constant_folds` (:1072), `propose_boolean_evaluations` (:1128), `range_comparison_contract` (:1172; always `ProofCertified`), `integer_constant` (:1186), `integer_value_type` (:1203).

**Inputs/outputs.** Reads `ScalarConstants` and (range rules) `ValueRanges` analysis products and `accepted_obligation_fact`. Produces `new_integer_evaluation` / `new_boolean_evaluation` candidates with witness (`IntegerEvaluationWitness::{Unary, Binary, ProofCertifiedBinary, ProofCertifiedUnary, RangeAgainstConstant, RangeAgainstRange}`), predicted delta -1.

**How it decides.**
- A fold fires if both operands have an integer constant fact in `constants.facts` with `fact.valid_in.machine == machine && fact.value == value` and `fact.identity` is `Some` (:1191-:1200). Keyed on (machine, value id); no program-point check on constant facts, unlike range facts which call `ranges.fact_applies_at(...)` (:751, :898).
- Result evaluated by the type's own function (`arithmetic.evaluate`, `policy.evaluate`, `bitwise_*`) which returns `None` on a case it will not fold (:325-:343) — e.g. overflow in Exact mode -> not folded.
- Rules of safety class `ProofCertified` also need `accepted_obligation_fact` (:353-:358, :543). Exact add/sub/mul/div/rem/shift, saturating and wrapping divide/remainder are `ProofCertified`; saturating/wrapping add/sub/mul and bitwise are `ExactOperationSemantics` (:206-:227).
- Range-vs-constant requires the constant to be a literal (`literal_integer_constant` :738), operand types equal (:740), and a range fact whose `support` matches `ValueRangeSupport::AcceptedOperationProof` (:747-:750).
- Range-vs-range: when both operands are the same value id, answers without range facts: `Some(!matches!(kind, LessThan))` (:933-:935) i.e. x==x true, x<=x true, x<x false.

**Cases by shape.** Fixed lists as above; `integer_binary_shapes::classify` matches only IntegerShift, IntegerArithmetic, IntegerBitwiseAnd/Or/Xor (:374-:455), `_ => None`. Range rules cover Equal/LessThan/LessOrEqual only (no not-equal, greater-than; presumably lowered earlier — inferred).

**Same job elsewhere.** `range_against_constant::evaluate` (:772) and `range_against_range::evaluate` (:924) share the min/max comparison logic with different operands; both `contract` fns in those modules ignore their `_safety` argument (:660, :827). `integer_value_type` (:1203) is a lookup of a value's scalar type over parameters/block parameters/node definitions; `scalar_value_definition` in `optimization_unit` (used proof_check_elision.rs:126) does a similar lookup (suspected, not read).

**Unused / one-caller.** `ExactIntegerCastConstantsRule` is `pub(crate)` whereas the others are `pub` (:977). `node_location` (:1240) — not grepped. `built_in_registrations` called from rules.rs:224 (read).

**Stated limits.** None.

**Fail-open risks.** none seen. `unreachable!()` in :164 for kind combos excluded by the match arm (panic, not accept). The constant facts lookup `find_map` takes the first matching fact for (machine, value) (:1191); traced after reading semantic_analyses.rs:1748/:1981: facts come from a per-function `BTreeMap<ValueId, _>`, so there is at most one fact per (machine, value) and "first" is the only one.

---
## 8. P1/semantic_analyses.rs (1-2374) — read in full

**Purpose.** The analyses over a `PsiOptimizationUnit` that rules consume: what each operation may do (effects), ownership-frontier facts, which places each function reads/writes, place aliasing, sparse conditional constants with executable edges, use/definition lists, and backward value liveness. Each is a plain function from unit to an analysis struct; `analyses.rs` (not in my unit) dispatches them by `AnalysisKind`.

**Shape.**
- `effect_summaries` (:11): `EffectClass` {PureScalar, StructuralState, InternalCall, BoundaryCall, Service, Control} (:24), `NodeEffectSummary` (:34, `is_pure_scalar_baseline` :48 requires same revision, class PureScalar and all four knowledges `No`), `FunctionEffectSummary` (:78), `effect_summaries` (:90), `transitive_function_effects` (:137, fixpoint over callee sets), `operation_effect` (:281, an exhaustive `match` with no `_` arm).
- `ownership_frontiers` (:378): copies `unit.ownership_frontier_facts` into analysis facts (:412).
- `place_access_summaries` (:428): `FunctionContext` (:576) resolves a (place, path) through aliases and block-parameter bindings to `Parameter | External | Internal` (:692); `scan_operation` (:873) lists per-operation reads/writes; fixpoint over callee contributions (:1270-:1356); `FunctionPlaceAccessSummary` (:500) has `unsummarized_targets`.
- `place_aliases` (:1451): groups ownership-frontier claims by `(root, path)` view and counts claims with no input root (`unrooted_claims`).
- `sparse_conditional_constants` (:1553): lattice `Unknown/Constant/Overdefined` (:1698); `ScalarConstantFact` (:1576) with `valid_in: ValueFactRegion{revision, machine, value}`; `ScalarConstantAnalysis::{literal_integer_constant, boolean_constant}` (:1628/:1639); `ExecutableEdgeFact` (:1675); `sparse_conditional_constants` (:1704), `sccp_machine_snapshot` (:2029).
- `use_definitions` (:2130): flat lists of (machine, definition) and (machine, use); `replacement_dominates_parameter_uses` (:2143).
- `value_liveness` (:2203): per block/node entry and exit sets.
- `semantically_reachable_blocks` (:2349): reachability over `node.successors` of each block's last node.

**Inputs/outputs.** In: `&PsiOptimizationUnit` only. Out: the analysis structs, each stamped with `unit.identity` as `revision` where it carries one.

**How it decides.**
- Effects are keyed on the operation variant only (:291-:375). Calls: `May` for all four effects (:349); callee machine not in unit or no callee -> `May` for all (:183-:190); dynamic structural call -> `May` (:192-:203).
- `is_pure_scalar_baseline` (:48) is the single predicate many rules use for "this node is a pure scalar": it also checks `revision == unit.identity`.
- SCCP: constant facts for operation results come only from `function.facts` literal rows (`IntegerConstant`/`BooleanConstant`) (:1775-:1807); all other node definitions start `Overdefined` (:1770-:1772); function parameters `Overdefined` (:1763); block parameters `Unknown` then merged from jump/branch bindings (:1901-:1910). A literal row becomes `Constant` only when its block is reachable (:1824-:1832). Conditional with constant Boolean follows only the chosen successor (:1843-:1864); with Integer constant, `Unknown` or missing condition -> no successors yet (:1869-:1871).
- Constant fact identity: literal support gets `literal_scalar_constant_fact_identity`, otherwise `derived_sccp_scalar_constant_fact_identity` from the machine snapshot (:1987-:2008); if neither -> `identity: None`, and consumers drop facts with `None` identity (`extract(...).zip(fact.identity)` :1662).
- `literal_integer_constant` (:1628) restricts to facts whose support is exactly one operation and no edges (:1660, :1598-:1603).
- Facts are one per `(machine, value)` because built from a `BTreeMap<ValueId, _>` per function (:1748, :1981) — read.
- Liveness: successor list covers only `Jump` and `Conditional` (:2241-:2249); `_ => Vec::new()`.
- Place resolution: parameter places resolve to `Parameter`; block-parameter places follow all recorded jump/branch bindings (:702-:713); a revisited place returns `External` (:684-:686); places of kinds `ProviderAttachment` or unknown (`None`) -> `External` (:744-:746); `Result` and local kinds without a referent step -> `Internal` (:731-:742).

**Cases by shape.**
- `operation_effect` is a closed list of every operation variant in one `match` (:291-:375); adding a variant is a compile error (inferred from the absence of `_`).
- `scan_operation` (:901-:1225) likewise has no `_` arm: every operation variant is listed in a read/write arm or in the final no-op arm (:1196-:1224).
- `value_liveness` successors: Jump, Conditional only (:2241-:2249). `StructuralCase` (a terminator with successor edges, handled in SCCP :1838 and in `optimization_graph` ranked_cycles_validation.rs:754) is not listed. Consequence read from code: a block ending in a `StructuralCase` has empty live-out in this analysis, so values used only in its case targets are not in `exit` sets for nodes in that block. Whether any rule or validator is exposed to this I did not trace (open). Consumer: `node_elision::propose` (rules.rs:372-:378) tests `row.exit.contains(&shape.result)`.
- SCCP `Overdefined` for payload parameters of every `StructuralCase` successor (:1884-:1895).

**Same job elsewhere.**
- Successor/edge enumeration from a block terminator is repeated: `value_liveness` (:2241), `semantically_reachable_blocks` (:2361, uses `node.successors` generally), SCCP (:1836-:1874), ranked_cycles_validation.rs `optimization_graph` (:731-:763). Four separate walks, each with its own set of handled terminators.
- Reading `unit.facts` literal rows for constants: `sparse_conditional_constants` (:1775-:1807) vs `value_ranges.rs` (see notes 11).
- The "service" operations list (`ActivateTaskRegister`, `ReadTaskRegister`, `ReloadSegmentDescriptors`, `ReadSegmentSelector`, `GlobalDescriptorTable`, `PortWrite`, `RegisterAccess`) appears at :205-:214 (service id collection), :341-:360 (effect classes) and :905-:928 (access attribution).
- `bind_arguments` (:751) handles Jump and Conditional structural bindings, not `StructuralCase` (:648-:659).

**Unused / one-caller.** Greps (over `omega\*.rs` and the `src` dir):
- `place_access_summaries(` is called only from `analyses.rs:88`. `AnalysisProduct::PlaceAccessSummaries` / `AnalysisKind::PlaceAccessSummaries` appear only in `analyses.rs` (:45, :87): no rule in this crate requests the product (grep). The analysis is computed but I found no consumer in `omega\`.
- `AnalysisKind::PlaceAliases` / `AnalysisProduct::PlaceAliases`: only `analyses.rs:37,67`. No consumer found. `impl PlaceAliasesAnalysis {}` (:1489) and `impl PlaceAliasFunction {}` (:1491) are empty impl blocks.
- `ExecutableEdges`: `AnalysisKind::ExecutableEdges` appears only in `analyses.rs:74` and as an invalidation entry at `rules.rs:1146`; no rule requires it.
- `declared_access(`: 8 call sites in this file, 5 in terminal-psi references.rs, 1 in storage.rs (grep count). The function is `pub(crate)` here and a similarly named one exists elsewhere (suspected same job, not read).
- `ownership_frontiers` analysis: used by `control_flow_cleanup.rs` rules (grep lines :61, :494, :727, :937, :1095).

**Stated limits.** None in comments.

**Fail-open risks.**
- `effect_summaries`: `unit.call_machine_references` returning empty or a callee outside the unit raises effects to `May` (:183-:190) — fail-closed. Traced.
- `place_access_summaries`: callee outside the unit sets `unsummarized = true` and attributes arguments by declared access (:855-:859). Fail-closed. Unknown place kind resolves `External` (:744) — conservative.
- SCCP: unrecognised condition value (missing in `values`) -> no successors (:1871), i.e. treated as not-yet-known, which would leave blocks unreachable if a condition were never populated. Every definition is pre-seeded as `Overdefined` or `Unknown` (:1763-:1772), so a missing entry would only occur for a value not defined anywhere in the function. Traced for that input. Not an accept path for wrong programs, but marks edges `KnownInexecutable` (:1959-:1966) for non-conditional terminators that were never fed; the loop only skips edges of unreachable blocks, and Jump/StructuralCase edges of reachable blocks are always fed (:1838).
- Value liveness missing `StructuralCase` (see above): if a dead-code rule accepted a node because `exit` lacked a value actually used in a case successor, a still-used scalar could be proposed for removal. Reachability of this with a bad program: open (validator in validation.rs not matched against it here).

---
## 9. P1/specialization.rs (1-552) — read in full

**Purpose.** Builds the "plan" structs for three specializations: forwarding or folding field reads from a record place (field value), folding `StructuralCaseMembership` tests whose case is provable (representation / case membership), and bypassing a constant-driven dispatch block by redirecting incoming edges straight to the arm that will be taken (state-argument specialization). The comment says every family excludes machines holding cyclic components (:5-:7); the exclusion itself is in rules.rs (`scc.cyclic_machines`), not here.

**Shape.** `field_value_specialization::propose::plan` (:17) delegates to `optimization_unit::field_evidence` and `field_value_reads` (not in unit). `representation_specialization::{accounting::provenance_rows :49, admission::membership_evidence :87, admission::admit_membership_node :101, propose::plan :154}`. `state_specialization::{accounting::provenance_rows :207, admission::dispatch_evidence :285, incoming_edges :334, admit_incoming_edge :351, delivered_argument :438, call_result_constant :478, propose::plan :517}`.

**Inputs/outputs.** In: unit, function, place or block id, (state) `ScalarConstantAnalysis`. Out: `FieldValueSpecializationRewrite`, `CaseMembershipSpecializationRewrite`, `StateArgumentSpecializationRewrite` plans; `provenance_rows` returns `Vec<ProvenanceRewrite>` or `None`.

**How it decides.**
- Case membership: node must be `StructuralCaseMembership` on the declared place with Boolean result (:109-:121); the proven case comes from `established_case_at` (a producer) or `resolved_sole_case` (type has one case) (:122-:131); `outcome = proven_case == observed case` (:144). Keyed on place ids, case ids, operation ids.
- State specialization dispatch block: not the entry, no structural parameters, last node a `Conditional`; condition is a block parameter directly (no prefix nodes) or an integer comparison found by `integer_comparison_condition` (:293-:313); rejected if the parameter itself already has a constant fact for this revision (:314-:320).
- Incoming edge admitted only if from `Jump` or `Conditional` arm targeting the dispatch with empty structural bindings and cleanup actions on both the operation arm and the edge (:364-:399); the argument bound to the parameter must be a constant fact (:401-:407) or the result of an `O::Call` to a callee with exactly one `Return` whose value has a constant fact (:478-:508); the resolved arm must have no bindings/cleanup and its target no structural parameters (:419-:425).
- `plan` clears `edges` when every incoming edge was admitted (`edges.len() == incoming.len()` :542). No comment states why; effect (read): a dispatch whose predecessors all resolve yields an empty patch and `rules.rs:1171` skips it.

**Cases by shape.**
- `delivered_argument` follows a chain of unique-predecessor block parameters, bounded by block count (:445-:474).
- `call_result_constant` handles only `O::Call` (not `CallStructural`) and only callees with a single `Return` operation (:488, :496-:502).
- Dispatch condition kinds: `DirectParameter` or whatever `integer_comparison_condition` returns (:305-:313).

**Same job elsewhere.** `provenance_rows` in `representation_specialization` (:49) and `state_specialization` (:207) both emit `ProvenanceRewrite`s `RealizedAt`; the node-level realised-provenance builder `node_realized_provenance` in control_flow_cleanup (not read) and `provenance_realized_at` (used in rules.rs:823) do the same job (suspected). Constant fact lookup by `(machine, revision, value)` at :314-:318, :401-:405, :503-:507 is a third copy of `ScalarConstantAnalysis::constant_fact` (semantic_analyses.rs:1650) that also checks `valid_in.revision == unit.identity`, which `constant_fact` does not.

**Unused / one-caller.** `field_value_specialization::propose::plan` called from rules.rs:1062; `representation_specialization::propose::plan` rules.rs:671; `state_specialization::propose::plan` rules.rs:1167 (all read).

**Stated limits.** Header: "Every specialization family excludes machines holding authenticated cyclic components" (:5-:7).

**Fail-open risks.** `call_result_constant` takes the callee's constant fact on its single `Return` value without reading whether the callee's return is reachable or whether the call node has crash continuations (:478-:508). Open (not traced beyond this file). `membership_evidence` `root_type?` returns None on missing types -> no fold (rejecting). none seen otherwise.

---
## 11. P1/value_ranges.rs (1-290) — read in full

**Purpose.** Computes integer value ranges: exact single-value ranges from scalar-constant facts, and bounds extracted from accepted operation proofs ("this operation's obligation says x in [lo, hi]"), valid in blocks dominated by that operation's block. Rules ask `fact_applies_at` before relying on a range.

**Shape.** `constant_facts::collect` (:20), `control_flow::dominated_blocks` (:62), `facts::new` (:84), `proof_facts::extend` (:118), `ValueRangeAnalysis{facts}` (:260), `fact_applies_at` (:265), `value_ranges` (:278).

**Inputs/outputs.** In: unit, `DominatorAnalysis` from `crate::analyses::control_flow::dominators(unit, false)`. Out: sorted `Vec<ValueRangeFact>` (sorted by machine, value, scope, identity :281-:288).

**How it decides.**
- Constant ranges come from `scalar_constants(unit)` facts with Integer value and a `Some(identity)` and a found integer definition (:22-:38).
- Proof ranges: only for reachable blocks (:126); the operation must be the unique node with that `proof_range_goal` operation and have exactly one `OperationObligationReference` fact (:135-:161); the accepted obligation fact's proposition must equal the goal's canonical order key (:170-:179); exactly one matching proof question with canonical certificate, same Terminal Psi, same bundle fingerprint and canonical identity (:180-:198); bounds extracted by `extract_integer_intervals`, only the `Bounds` outcome (:200-:204); the bound value's definition type must equal the extracted type (:208-:212); full-range bounds are dropped (:215-:219).
- `fact_applies_at` (:265): the fact must be in the analysis and `validate_current_value_range_fact_at` must return Ok (:273-:274).

**Cases by shape.** Proof facts only for goals recognised by `optimization_unit::proof_range_goal` (not in unit).

**Same job elsewhere.** `semantically_reachable_blocks` use (:124) shares code with semantic_analyses.rs:2349. Constant facts are recomputed by calling `scalar_constants(unit)` (:22): the SCCP is run again here rather than reusing the cached `ScalarConstants` product (read; whether the pass manager caches is not in this file).

**Unused / one-caller.** `value_ranges` used from `analyses.rs` (suspected; not grepped separately beyond semantic_analyses.rs:2342 re-export). `.expect("internally derived value range has a canonical identity")` at :94 panics if identity can't be built.

**Stated limits.** None.

**Fail-open risks.** none seen: every missing/ambiguous input `continue`s (no fact produced).

---
## 10. P1/validation.rs (1-3171) — read in full

**Purpose.** Despite the name, mostly the "is this node safe to hoist out of a loop" predicate library for loop-invariant scalar motion (the admission tests used by `ranked_rewrites.rs::propose::admit_member_node`), plus three small things: the validated-context entry points for a unit, a "projection sealing" struct that carries identities of the final optimized unit, and the construction of the pre-physical optimization manifest.

**Shape.**
- `context` (:12): `validate_verified_psi_context` (:18), `validate_transformed_psi_context` (:25), `derive_context` (:33): runs `optimization_unit::validate_runtime_evidence_context` then `ranked_cycles::validate_exact_ranked_cycles` (ranked_cycles_validation.rs:906).
- `invariant_calls` (:55): shape recognisers per call kind (`scalar_call_shape` :100, `unit_call_shape` :139, `structural_scalar_call_shape` :176, `structural_call_shape` :217) each split into a no-crash-continuation and a crash-continuation variant; admission fns (`invariant_*_call_admission` :276-:350, `borrow_call_evidence` :411, `invariant_scalar_call_admission` :512, `invariant_crash_continuation_call_admission` :529); `crash_routes_cover` (:569); `unit_effect_summaries` (:585); `scalar_call_callee_pure` (:597); `component_members_unobservable` (:617).
- `invariant_operations` (:657): `admissible_*` recognisers: scalar leaf (:696), scalar computation (:706), place read (:726), byte read (:766), field byte read (:785), subslice (:804), byte literal (:836), primitive local (:843), record (:875), scalar array (:963), scalar case (:998), trivial affine local (:1042) and matching `invariant_*_admission` fns.
- `member_blocks` (:1065): `value_definition_sites` (:1074), `shared_entry_source` (:1090), `guaranteed_executed_member_blocks` (:1099; dominator fixpoint inside the component), `resolve_member_parameters` (:1185), `invariant_member_parameters` (:1253), `member_scalar_operand_substitution` (:1312), `member_parameter_run_result` (:1355).
- `place_observations` (:1389): `component_preserves_place_observations[_tolerating]` (:1410/:1417), `node_preserves_place_observations` (:1470), `exclusive_borrow_call_preserves_place_observations` (:1528), `borrowed_roots_exclusive` (:1579), `member_place_references` (:1645), `place_observation_root_visible` (:1725), `produced_place_root` (:1754), `member_root_producer_count` (:1787), `copyable_owned_argument_root` (:1801), `consumed_owned_argument_root` (:1835), `invariant_observation_root` (:1858), byte/field/subslice admissions (:1884-:1953), `invariant_member_place_parameters` (:2068).
- `prephysical_manifest` (:2142): `projection::expected_record` (:2154), `structural_statistics` (:2195), `project_pre_physical_optimization_manifest` (:2260), `ValidatedPrePhysicalOptimizationManifest` (:2292), error enum (:2303).
- `projection` (:2313): `seal_optimized_abstract_plan_projection` (:2323), `ValidatedOptimizedAbstractPlanProjection` (:2346) with `identity()` (:2395).
- `relocation_rewrites` (:2417): `restricted_produced_root` (:2428), `scalar_case_result_consumed` (:2440), `restricted_member_produced_root` (:2463), `scalar_case_result_contained` (:2490), `rewrite_scalar_case_custody` (:2610), substitution fns (:2762-:2895), `call_crash_continuations` (:2897), `structural_call_crash_continuations` (:2925).
- `scalar_case_frontiers` (:2944): `rewrite_relocated_case_result_frontiers` (:2953), `case_result_owned_sites` (:2994).

**Inputs/outputs.** In: function/component/node/sets of relocating values and roots; `EffectSummaryAnalysis` (computed via `crate::compute_analysis(unit, EffectSummaries)` :588). Out: `Option<BTreeMap<ValueId,ValueId>>` substitutions (value operand rebinding), `Option<(map, Vec<(PlaceId,PlaceId)>)>` (plus place-root rewrites), or `bool`.

**How it decides.**
- Node shape "spine": `provenance.first() == Operation(psi_operation)`, no `successors`, no `ownership` events (`operation_shape_spine` :672-:676), and an exact count/order of `uses` matching the operation's operands (e.g. :692-:694, :828-:832, :976-:981). Keyed on the operation identity and operand positions.
- Call hoisting requires the callee to have `observable`, `crash` and `suspension` all `No` in its function-level effect summary (:597-:607) and every node of every component member to be unobservable (:617-:655): a call member needs a pure callee, anything else needs `observable == Some(No)`; an absent summary row -> false.
- Crash-continuation calls: callee's `verified_contract` crash routes substituted with caller args must equal the call's recorded `crash_continuations` and be covered by the caller's own published crash routes (:375-:387, :555-:566, :569-:583). `crash_routes_cover`: a published route covers when causes match and the route's alternatives are `[Truth]` or a superset (:576-:580).
- Structural call arguments: MutableBorrow/WriteOnly roots must be relocating roots produced exactly once in the component (:443-:449); SharedBorrow roots must land at a root visible in the preheader or be relocating (:451-:463); Owned arguments need empty path and either a copyable root or a consumed-once root (:464-:485).
- `place_observation_root_visible` (:1725): a root is visible at the preheader if it is a function structural parameter, the result place, a provider-attachment place, a preheader structural parameter, or produced by a non-terminator preheader node.
- `scalar_case_result_contained` (:2490-:2608) decides whether an affine place produced in the loop stays inside it, by walking every node in the function and matching a fixed list of operation shapes (:2500-:2565) and edge cleanup/binding rules (:2580-:2593, :2597-:2607).
- `member_scalar_operand_substitution` (:1312): operand defined outside the component is left as is; defined by a member block parameter -> replaced by its resolved representative or by a unique relocating node value; defined by a member node must already be in `relocating`, else `None` (:1348-:1350).
- `guaranteed_executed_member_blocks` (:1099): members that dominate every exit-source in the component (dominator sets by iteration).

**Cases by shape.** The admission fns are closed shape lists; refusals are `None`:
- Calls: `Call` with scalar result and `CallStructural` to `AbstractCallTarget::Machine` with result Unit, Scalar or Structural (Affine or Unrestricted, no qualifications/claims/selected evidence) (:100-:248). Dynamic callee not admitted (pattern requires `Machine`).
- `invariant_operations` recognisers each demand fixed use lists: place read has `uses.is_empty()` (:762); byte read uses exactly `[index, length]` (:692-:694); subslice `[start, end, length]` (:829-:832); record: scalar fields in order, structural fields `Owned` with empty path, multiplicity Unrestricted, or Affine only if all fields scalar (:900-:911); scalar array Unrestricted only (:981); scalar case Unrestricted or Affine, all fields scalar (:1011-:1021); primitive local Unrestricted (:858).
- `node_preserves_place_observations` (:1470-:1525) lists the operation kinds permitted in a loop that hoists a place read; structural calls only with SharedBorrow-only args and no claim transfers, `_ => false`.
- Stated shape limits in code: the field byte read admission requires an empty substitution `.filter(|substitution| substitution.is_empty())` (:1918-:1919).

**Same job elsewhere.**
- `shared_entry_source` (:1090) vs `sole_preheader` (ranked_cycles_validation.rs:183): both find the single block that enters a cycle component; this one allows several entry edges from the same source and returns just the block id, the other requires exactly one entry edge and returns the block plus its last-node index.
- The three place-root landing blocks in `borrow_call_evidence` (:451-:463), `call_argument_root_landing` (:491-:510), `invariant_record_admission` (:944-:955), and `invariant_observation_root` (:1858-:1882): four copies of "visible in preheader or relocating-once, else try the representative".
- `crash continuation` pairs: `invariant_crash_continuation_call_admission` (:529-:567) vs `crash_continuation_borrow_call_admission` (:352-:389) repeat the same check; every shape has a plain and a crash-continuation variant (6 pairs, :127-:262).
- `call_crash_continuations`/`structural_call_crash_continuations` (:2897-:2942) are also used by `ranked_rewrites.rs::apply::realize` (:153-:160) to rebuild crash routes on relocation.
- `semantically_reachable_blocks`, `value_definition_sites`: `value_definition_sites` (:1074) vs `value_ranges`'s/`scalar_value_definition` — lookup of a value's definition site; separate function.

**Unused / one-caller.** Greps (grep -c over omega-rust unless noted):
- `seal_optimized_abstract_plan_projection`: definition + publication.rs:187/:213 only (read).
- `unit_effect_summaries` (:585): used in ranked_rewrites.rs:429 and :1016 (grep earlier lines).
- `place_observations::member_place_references` (:1645) used by relocation_rewrites.rs (:2559) and `place_observations` (:1622) (read).
- The header names `scalar_case_frontiers` as a "boundary verifier" (:2-:5): it is not a verifier; it only rewrites ownership-frontier facts after relocation (`rewrite_relocated_case_result_frontiers` :2953), and its caller is `ranked_rewrites.rs:263` (read).
- Comment-vs-use: `ValidatedOptimizedAbstractPlanProjection` ("Validated...") and `ValidatedPrePhysicalOptimizationManifest` are built by copying identities and recomputing an identity from inputs the caller supplied; there is no check in `seal_optimized_abstract_plan_projection` (:2323-:2343) or `project_pre_physical_optimization_manifest` (:2260-:2289) that compares anything against an independent source except rebuilding the initial unit (:2167-:2172). Read.

**Stated limits.** None as comments.

**Fail-open risks.**
- `scalar_case_result_contained` final arm (:2557-:2564): for an operation not in the earlier arms, if `member_place_references` can enumerate its references, accept iff `picked` isn't among them; if it cannot enumerate (returns false), the node counts as confined when it is NOT a member (`!member`). So a node outside the loop whose operation is not in `member_place_references`' list (e.g. a move, `TransformContent`, `EstablishReference`, element view write; lists at :1650-:1720) and which references `picked` is accepted. No comment states why. Reachability with a bad program: open (not traced; the places in question are produced inside the component, which may make an outside reference impossible by scoping, not verified).
- `rewrite_relocated_case_result_frontiers` (:2959-:2961): `unit.function(*machine)` None -> `continue`, leaving frontier facts unrewritten. Machine comes from the relocation itself, so None is not expected; "inferred".
- `unit_effect_summaries` fallback (:588-:594): if the analysis does not come back as `EffectSummaries`, returns empty rows; with empty rows `component_members_unobservable` is false (:647) and `scalar_call_callee_pure` is false (:597-:607), so fail-closed. Traced.
- `value_definition_sites` returns a `BTreeMap` keyed by value; duplicates overwrite silently (:1086-:1087); not traced whether values can be defined twice.

---
## 12. P2/control_flow.rs (1-3104) — read in full

(P2 = `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/`.)

**Purpose.** Lowers one `AbstractFunction` into a `TargetFunction` made of a block-structured control graph. It walks blocks in dominator-tree order, keeps a `LiveDefinitions` record of what is known at each point (scalar homes, structural homes, views, references, etc.), lowers each non-terminator operation (via `operations::lower_operation`, in control_flow_operations.rs), then lowers the terminator, and repeats until the reference-descriptor storage at block entries stops changing.

**Shape.**
- Child modules pulled in with `#[path]`: control_flow_operations.rs (:33), control_flow_calls.rs (:36), control_flow_storage.rs (:41, not in unit), references.rs (:48, not in unit).
- `mod dominance` (:50): `schedule` (:53), `reverse_postorder` (:93), `solve_sets` (:110, bit-set dominators), tests (:172-:326).
- `mod interrupt_installation` (:329): `lower` (:342), `scope` (:414). `mod program_image` (:430): `lower` (:442). `mod run_custody` (:600): `lower` (:609). `mod static_interrupts` (:675): `lower` (:686). These four lower special `BoundaryCall` forms.
- `mod row` (:516): `RowSite` (:538) bundling function, types, `&mut LiveDefinitions`, op list, provenance.
- `mod structural_case` (:795): `observe` (:812, `StructuralCaseMembership`), `case_source` (:863), `case_projection` (:926), `lower` (:952, the `StructuralCase` terminator).
- `mod terminator` (:1091): `plain_home_cleanup` (:1222), `lower_terminator` (:1353) with arms for ExternalCompletion, Crash, ReturnStructural, Return, StructuralCase, ReturnUnit, Jump, Conditional; `_ => Err(unsupported_control_flow)` (:1830).
- `mod transfers` (:1835): `validate_parameters` (:1848), `re_home_bound_references` (:1874), `enter` (:1978), `is_address_join` (:2005), `address_parameter` (:2014), `address_join_argument` (:2025), `validate_successors` (:2105).
- Types: `ReferenceEdgeStorage` (:2417), `ScalarHomes = Arc<BTreeMap<..>>` (:2426), `LiveDefinitions` (:2429, 20 fields), its methods (:2451-:2539).
- Entry: `pub(crate) fn lower(...)` (:2541-:3104), 14 parameters.

**Inputs/outputs.** In: `AbstractFunction`, target, all functions by machine, dynamic dispatch catalog, borrowed reference sources, stored effect catalog, structural type lookup, boundary machine declarations, settlements, selected calls, scalar ABIs, native callbacks, installed interrupt cohort (:2541-:2555). Out: `TargetFunction{graph: TargetControlGraph{blocks, ...}, provenance, ...}` (:3088-:3103).

**How it decides.**
- Whole-function precondition checks (:2556-:2693): non-empty blocks; result is Unit/Scalar/Structural; every structural parameter is either non-Owned with a nonlinear multiplicity or passes `owned_arguments::is_owned_parameter`; no duplicate value definitions across parameters, block parameters and the result of a closed list of operations (:2585-:2619); block structural parameters must be one of six recognised carrier kinds (byte, immutable byte carrier, element view, address join, owned byte, shared-view-carrier loan, owned parameter) (:2634-:2647); established places unique (:2655-:2682).
- Block layout: `entries[0].operation_offset == 0`, each block's op range non-empty, final op decides successors (:2715-:2747); the entry block must have no incoming edge (:2761).
- Dominator schedule via `dominance::schedule` returns `None` (-> `unsupported_control_flow`) if any block is unreachable from entry (:58-:61, :2764-:2765).
- Fixed-point loop (:2828-:3052): a block is lowered when its dominator's exit state exists and at least one predecessor exit exists; arrivals from several predecessors are merged by `merge_arrivals`; the block is re-lowered only if its entry reference storage, borrowed sources or retired-roots set differ from the last lowering (:2891-:2896). If nothing changed but a block was deferred (missing state), the function errors (:3046-:3049). After the loop all predecessor arrivals are re-merged and must equal the stored entry state (:3053-:3078).
- Conditional condition source: scalar parameter of Boolean type, known Boolean constant, scalar home, or block parameter; otherwise `unsupported_control_flow` (:1777-:1811).
- Edge checks in `validate_successors` (:2105-:2409) are keyed on place ids, structural types, access modes and multiplicities, with errors carrying formatted debug strings of the binding (`format!("... {binding:?}")` :2137, :2329).
- `plain_home_cleanup` (:1222-:1351): each cleanup action requires a live owned home, a disposable view, or a retirable reference arrival; else `InvalidStructuralValueInput`.

**Cases by shape.**
- Block-parameter kinds: the six-way list at :2634-:2647.
- Terminator list: ExternalCompletion, Crash, ReturnStructural, Return, StructuralCase, ReturnUnit, Jump, Conditional (:1392-:1830); anything else (including `InvokeNoNormal`, handled earlier by `calls::lower_no_normal` :3000-:3016) refused.
- Return/ReturnUnit successors: the `.all(...)` over cleanup actions at :2721-:2729 lists all four `TerminalAffineCleanupAction` variants (the same four are the full set in `plain_home_cleanup` :1228-:1348), so the guard is always true (inferred).
- Descriptor return requires a two-word ABI placement `ValueShape::integer(16, 8)` (:1614-:1617); descriptor returns with returned claims or trivial affine locals refused (:1608-:1613).
- StructuralCase: source must have a home, address-join parameter or entry parameter; reference-pointer slots with indices refused (:971-:977); case count must equal declared cases and layout cases (:999-:1001); payload scalar types must have a fixed native shape (:1044-:1045).
- `program_image::lower`: only `NativeTarget::uefi_x64()` (:476); fixed address-64 integer result.
- `External completion` requires `declaration.result.is_unit()` (:1408).

**Same job elsewhere.**
- Successor enumeration for graph building (:2715-:2747) repeats the terminator list of `lower_terminator` and the one in P1 `ranked_cycles_validation::optimization_graph` (:731) (different IR).
- `references::merge_arrivals` use here (:2873, :2509) and `merge_borrowed_arrivals` (:2508, :2878, :3071): the merge of arrival states is written three times in this function (in `reference_arrival`, in the fixed-point loop, and in the post-loop check).
- The `dominance` module re-implements dominators (bit-set) while P1 has `analyses::control_flow::dominators` and `member_blocks::guaranteed_executed_member_blocks` (validation.rs:1099) (different IRs, same relation). The test module keeps the older set-based solver as an oracle (:177-:239).
- The four special `BoundaryCall` lowerers (:329-:793) each re-check `settlement.execution` and `settlement.realization` against an expected `CompilerBuiltinExecution` value (:378-:389, :476-:480, :644-:647, :714-:726).

**Unused / one-caller.** `lower` (:2541): grep not run for callers; the module doc names `lower_control_flow` (:2) but the function is named `lower` — comment/function name mismatch (read; `rg lower_control_flow` not run). `std::env::var_os("OMEGA_COMPILER_CHECK_SITES")` appears at :1237 and :1268; the same variable is read in 13 other files across omega-rust (grep count above, e.g. optimization-core `refusal_site.rs`, terminal-psi codec `foundation_validation.rs`, `psi\...\trace_flags.rs`): diagnostics printing driven by an environment variable inside the compiler. `unsupported_control_flow` appears 71 times in this file and 348 times in the six files of this stage's `src` (grep count): it is the catch-all refusal.

**Stated limits.** Module doc: "reference descriptor storage converges to a fixed point across post-transfer, post-cleanup edges" (:9-:10). Comment at :2424: "Saved block states share unchanged homes." Comment at :91: "Block numbers can run opposite to control flow."

**Fail-open risks.**
- `reference_arrival` failures inside the fixed-point loop are swallowed as `deferred = true; continue` (:2856-:2860, :2868-:2871, :2873-:2877); the error is dropped. Traced: after the loop, if nothing changed and `deferred` is set, the whole function fails (:3046-:3049); if something changed, the loop continues. Finally the post-loop check re-merges and fails on mismatch (:3053-:3078). So a swallowed error cannot lead to acceptance unless the blocks later lower successfully with a different state; the stored state is compared at the end. Traced.
- `plain_home_cleanup` `RetireHome` for a place with a reference arrival is accepted via `retirable_reference_arrival` (:1235); keyed on place identity and reference descriptor equality (:1175-:1180).
- The `.all(..)` guard at :2721 accepts every cleanup action kind (see above): stated nowhere; traced as "all four variants" by reading both lists.
- `address_join_argument` final branch (:2079-:2093) admits a view argument only for SharedBorrow parameters whose producing operation is found by `psi_operation()` search through `function.operations`; unmatched -> `None` -> reject.
- none seen where an unknown input reaches acceptance beyond the above.

---
## 13. P2/control_flow_calls.rs (1-2162) — read in full

**Purpose.** Turns a call operation into a `TargetUnitOperation::Call` (or an `InvokeNoNormal` terminator, or a `CleanupCall`): works out the callee's ABI signature, decides for each structural argument where its bytes come from (a home, a parameter, a block parameter, a view, an address join, a reference) and checks that the actual argument matches the callee's formal, then lands the result (unit, scalar, or a new structural home). Also contains the resolver that rewrites a "selected" boundary call into a plain call.

**Shape.**
- `borrowed_calls` (:12): `direct_referent` (:28), `argument` (:57), `storage_argument` (:223), `address_join_window_argument` (:304), `stored_descriptor_argument` (:349), `byte_argument` (:433).
- `byte_arguments::shared_owner_view` (:661).
- `call_signature` (:805): `PreparedCallSignature` (:825), `prepare` (:831), `From<PreparedFunctionSignature>` (:913).
- `calls` (:938): `CallContract {PublishedScalar, Ordinary}` (:971); `lower_no_normal` (:977); `Invocation` (:1076); `lower_call` (:1089); `land_result` (:1312); `published_scalar_contract` (:1390); `require_exact_target_abi` (:1480); `ordinary_contract` (:1491); `structural_result_home` (:1543); `result_qualifications_match` (:1569); `scalar_argument` (:1599); `structural_argument` (:1623, the argument dispatcher); `lower_cleanup_calls` (:1789).
- `owned_arguments` (:1867): `is_owned_parameter` (:1875), `argument` (:1883).
- `selected_boundary_calls` (:2011): `resolve` (:2021), `retain_origin` (:2126).

**Inputs/outputs.** In: the abstract call operation, `RowSite` (live definitions, op list, provenance), target, all functions, dynamic dispatch catalog, scalar ABIs, prepared signature of the caller, parameter-by-place map. Out: pushes `TargetUnitOperation::Call{call_plan, result, scalar_arguments, arguments, access_bindings, claim_transfers, requirement_obligations, crash_continuations}` (:1281-:1306) and mutates `live` (structural homes, views, reference moves and stored effects: :1279-:1280).

**How it decides.**
- Callee: `AbstractCallTarget::Machine` looked up in `functions` (error `UnknownCallTarget`), or `Dynamic{interface, slot, selector}`: requirement must exist and its `slot` equal; selector operand must be unsigned 64-bit integer (:1161-:1178).
- Dynamic call to a requirement whose formal carrier type is an access-view plan carrier -> `AccessDynamicCallRequiresBinding` (:1180-:1197).
- `Call` (scalar, "published") vs `CallStructural` (ordinary) take different contract routes (:1210-:1225). Published scalar: callee must have no structural parameters and no entry claims, result type must equal callee result type, result must be placed in exactly one register at offset 0, and the callee's ABI recorded in `scalar_abis` must equal the freshly derived one (`require_exact_target_abi` :1480-:1489 compares with `!=`).
- Structural argument source chosen by a fixed priority chain of tests on place identity (:1654-:1785): shared-owner byte view; path with referent; reference formal; owned formal; primitive reference; address join/invocation projection (storage); address join window; block view or view-typed home/view; plain home; otherwise `lower_structural_argument` (in `crate::unit::roster_arguments`, not in unit).
- Each branch compares `argument.access` to `declaration.access`, `multiplicity`, structural type via `borrowed_referent_matches`/`descriptor_value_transfer_matches`, and `destination.shape` to a recomputed shape (e.g. :131-:157, :270-:282, :400-:407, :775-:781, :1901-:1907).
- Cleanup call: callee must have `attachment` and `nominal_cleanup_subject` equal to the cleanup's structural type, unit result, no scalar formals, at most one structural parameter that `is_cleanup_receiver`, and no `requirement_obligations` (:1809-:1849).
- Selected boundary call resolve: result type/multiplicity (structural) or scalar type (scalar) must equal the candidate callee's result; scalar boundary call with no structural arguments or claim transfers becomes `Call`, otherwise `CallStructural` (:2060-:2120). `retain_origin` requires the lowered output to be exactly one `Call` with matching operation, callee and `Authored` origin (:2131-:2158).

**Cases by shape.**
- Byte-view argument shapes: `byte_argument` handles four source kinds, `completed_descriptor`, `live.views` (element-view or byte-view), block view/owned arrival, plain parameter (:496-:636); the carrier size is 8 for `BoundedView`, else 16 (:483-:488, a literal in the code).
- `shared_owner_view` only for `SharedBorrow` of a `BoundedOwned` byte sequence, no `Referent`/`FixedRange` path segment, root multiplicity Unrestricted, no projected qualifications, destination `borrowed_reference(16, 8)` (:670-:783). Returns `Ok(None)` for every other shape so later branches try.
- `byte_arguments::shared_owner_view` rejects roots with claims or qualification establishments with an error (:755-:766).
- Cleanup call: "at most one structural parameter" (:1813) and no scalar parameters (:1843).
- Scalar result of `Call` with the published contract: duplicate result value check (:1328-:1332).

**Same job elsewhere.**
- The four `allowed` matches on `StructuralAccess` (MutableBorrow -> not Owned; SharedBorrow -> SharedBorrow; WriteOnly -> WriteOnly; Owned -> false or arrival) at :206-:213, :254-:261, :588-:595 are three copies of the same table.
- `lower_no_normal` (:977) repeats `lower_call`'s argument lowering (:1235-:1268) for the terminator form: count checks, `call_signature::prepare`, `scalar_argument`, `structural_argument`, `access_call_custody`.
- `TargetStructuralArgument { ... }` struct literal with the same fields is built at 6 sites in this file (:41 via `direct_referent`, :160, :288, :416, :784, :1990).
- `selected_boundary_calls::resolve` (:2021) builds a call from a boundary call; the reverse route (boundary call lowering) is `crate::unit::boundary_call::lower_boundary_call` used from control_flow_operations.rs:1101 (not in unit).

**Unused / one-caller.** Grep results (omega dir): `byte_arguments::shared_owner_view` 1 call (:1654); `borrowed_calls::address_join_window_argument` 1 call (:1735); `borrowed_calls::byte_argument` 1 call (:1750). `PreparedCallSignature::from(PreparedFunctionSignature)` (:913) — callers not grepped.

**Stated limits.** None as comments in this file.

**Fail-open risks.** none seen of the accept-on-unknown kind; every fallthrough is `Err(unsupported_control_flow)`. One `Ok(None)` family: `shared_owner_view` returns `Ok(None)` for non-matching shapes (:683, :726, :736, :743, :753) which hands the argument to the next lowering path, not to acceptance (traced at :1654-:1664).

---
## 14. P2/control_flow_operations.rs (1-1425) — read in full

**Purpose.** The per-operation switch inside block lowering, plus four small lowerers: accessor scalar read/write, IEEE float arithmetic/classify via boundary calls, scalar array establishment, and scalar-source resolution (where a scalar `ValueId` currently lives: integer, home, Boolean immediate, float immediate, block parameter, or function parameter).

**Shape.** `accessor_scalar_access::lower` (:25), `ieee_float_arithmetic::lower` (:123), `operations::lower_operation` (:286-:1236), `scalar_arrays::{shape :1250, establish :1290}`, `scalar_sources::{ScalarSources :1336, source :1358, resolved_source :1366, runtime_indices :1408}`.

**Inputs/outputs.** In: abstract operation plus the same context as the calls file; mutates `RowSite` (`live`, `operations`, `provenance`). Out: pushes one or more `TargetUnitOperation`s and records `psi_operation` in provenance.

**How it decides.**
- First resolves a "selected" boundary call (`selected_calls.get(&(machine, op, boundary))`) into a plain call (:305-:320); after lowering, writes the call's origin back (:1232-:1234).
- If any boundary-call structural argument still holds a moved leaf, refuses (:328-:330).
- One `match` over `AbstractOperation` variants (:332-:1231): activation-claim ops, task/segment/GDT/register/port ops, atomic event, float policy/compare, scalar representation view, scalar array, native code layout, records/arrays/cases, references, `SelectImplementation` (table must exist in `dynamic_dispatch` :738), calls, structural case membership, copy/move/transform, field move/store, zeroed/local/trivial-affine locals, primitive storage ops, byte writes, byte literal, element view, field store, write-only store, `BoundaryCall`, scalar observations, integer/boolean/float constants, integer widen; `_ => Err(unsupported_control_flow)` (:1230).
- `BoundaryCall` dispatch keyed on `settlement.realization` variant (:941-:1156): EmptyRunRelease, StaticInterrupt Install / other, AccessLifecycle, AccessorScalarAccess, ProgramImageRetainedSuffixStart, NearestIeeeFloatArithmetic, Hosted{WriteByteI32, ExitProcessI32, ReadByte} or NormalizedForeignCall; others refused (:1155). Keyed on the settlement binding for the boundary id, not on boundary names.
- Install interrupt: needs exactly two admitted helper machines (Validate at argument index 0, Work at index 5 — literal positions `0` and `5` at :981, :986), each with zero scalar formals and exactly one structural formal of SharedBorrow access (:994-:1005).
- `scalar_arrays::shape` (:1250): nested `FixedArray` with literal lengths ending in a primitive scalar; the loop is bounded by the type count; `ArrayExtent` non-literal -> refused (:1260-:1285).
- `resolved_source` order: integers, scalar homes, Boolean immediates, float immediates, block parameters, then function parameters; else `UnknownValue` (:1371-:1399). `runtime_indices` accepts integer operands of at most 64 bits (:1416-:1418).

**Cases by shape.**
- Operations: `IntegerConstant` only with `ScalarType::Integer` (:1175-:1179); others fall to `_`.
- Scalar arrays: only `FixedArray` of primitive scalars, element count must equal `elements.len()` and every element source must have the array's scalar type (:1304-:1309).
- ieee float classify: result must be `is_unrestricted_plain()` and without qualification establishments (:184); layout from `conventional_float_class_layout` (:190).
- `RegisterAccess`/`GlobalDescriptorTable`/`PortWrite`/`ReleaseActivationClaim`: lowered with no checks beyond type conversion (e.g. `PortWrite` copies `value` without a source lookup, :557-:570).

**Same job elsewhere.**
- `ieee_float_arithmetic::lower` (:123), `accessor_scalar_access::lower` (:25), and the four special lowerers in control_flow.rs (:329-:793) each re-derive the expected execution/realization binding and compare against the settlement (same job: validating a builtin boundary call against its settlement).
- `scalar_arrays::shape` (:1250) has a same-named function in stage 03 (`scalar_graph_input::scalar_arrays::shape`, used at `03_.../src/legalization_target.rs:4022` and `legalization_source.rs:2005`; grep above): suspected duplicate shape computation, 03 side not read.
- `IeeeFloatCompare` inserts into `scalar_homes` without a duplicate check (:654) while `FloatResultPolicy` errors on a duplicate (:617-:622) and `ScalarRepresentationView` uses `insert_scalar_home` (:685); the duplicate-definition scan in control_flow.rs:2585-:2619 covers `IeeeFloatCompare` and `FloatResultPolicy` results beforehand, so the missing check here is covered upstream (traced).
- `unreachable!("inline float arithmetic routes only boundary calls")` (:141) where sibling lowerers return `Err`; the caller only routes `BoundaryCall` there (:1086-:1094), traced.

**Unused / one-caller.** `scalar_arrays::establish`: one caller (:699). `scalar_sources::source`/`resolved_source`: widely used in this stage (not counted). `ScalarSources` is also re-exported via `pub(crate) use cf_operations::{operations, scalar_sources}` (control_flow.rs:35).

**Stated limits.** None in comments.

**Fail-open risks.** none seen; unmatched operations and realizations return `unsupported_control_flow` (:1155, :1230). `PortWrite` passes `value: *value` straight through (:567) with no scalar-source resolution here (the type may be checked downstream, not traced: open).

---
## Coverage, key facts, unknowns

- **Coverage:** read in full, every line, in order: P1/pass_manager.rs 1-1552 (1552); proof_check_elision.rs 1-910 (910); publication.rs 1-380 (380); ranked_cycles_validation.rs 1-961 (961); ranked_rewrites.rs 1-1278 (1278); rules.rs 1-1284 (1284); sccp.rs 1-1246 (1246); semantic_analyses.rs 1-2374 (2374); specialization.rs 1-552 (552); validation.rs 1-3171 (3171); value_ranges.rs 1-290 (290); P2/control_flow.rs 1-3104 (3104); control_flow_calls.rs 1-2162 (2162); control_flow_operations.rs 1-1425 (1425). Total 20,689 lines read, 0 skimmed, 0 not read. Mention of a test module inside control_flow.rs (:172-:326) was read.

- **Five things a maintainer most needs to know about this unit.**
  1. The pass manager picks each pass's convergence measure by comparing the registry's pass identity to byte-string literals copied from rules.rs (pass_manager.rs:224-:260 vs rules.rs:8-:19); a pass not in the list silently gets the integer-operation measure, and a commit is rejected unless the measure strictly falls (pass_manager.rs:692).
  2. Loop-invariant scalar motion is not a registry pass: it has its own hand-written loop in pass_manager.rs:714-:886 (duplicating the registry loop), and its admission logic is spread over ranked_rewrites.rs (planning/validation) and validation.rs (about 2,900 lines of per-shape recognisers); validation.rs is mostly this, not "validation" in general. Each shape has a plain and a crash-continuation twin.
  3. Several analyses are computed with no consumer in `omega\`: `PlaceAccessSummaries`, `PlaceAliases`, `ExecutableEdges` (semantic_analyses.rs; grep found only the dispatch in analyses.rs), and `ValueRanges` recomputes SCCP internally (value_ranges.rs:22).
  4. `value_liveness` follows only Jump and Conditional successors (semantic_analyses.rs:2241-:2249), not StructuralCase; the dead-node proposer uses its `exit` sets (rules.rs:372-:378). Whether any validator covers a still-used scalar in a block ending in a StructuralCase was not traced.
  5. Stage 02 lowers each function through a fixed-point loop whose per-predecessor merge errors are swallowed and retried (control_flow.rs:2856-:2877) and finally re-checked (:3053-:3078); nearly every refusal is the single error `unsupported_control_flow` (71 uses in control_flow.rs, 348 across this stage's six files), so the reason is lost except where a `reason` string or debug `format!` is attached.

- **What you are unsure about.**
  - Whether `run_registries_inner`'s external-decision replay branch is reachable: only `run_registries` (passing `None`) was grepped; `run_registries_inner` was read as called once.
  - Whether the ValueLiveness StructuralCase omission can let a used scalar be removed (the node-elision accounting and candidate validator live in `optimization_unit` / `optimization_unit_semantics`, outside the unit).
  - Whether `scalar_case_result_contained`'s `!member` branch (validation.rs:2557-:2564) is reachable with a bad program.
  - Whether `ValidatedOptimizedAbstractPlanProjection` and `ValidatedPrePhysicalOptimizationManifest` are re-verified downstream; here they only copy identities.
  - Whether `PlaceAccessSummaries`/`PlaceAliases`/`ExecutableEdges` have consumers in crates outside `omega\` (I grepped `omega\` only for the kind names).
  - Callers of `PreparedCallSignature::from` (control_flow_calls.rs:913) and `RuleRegistryError::UnsupportedOptimizationCombination` (rules.rs:570) were not grepped.
  - Stage-03 `scalar_arrays::shape` was only matched by name, not read.

