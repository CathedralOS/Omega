# Record: unit_025 (area 12-native-representations)

Unit: one file, `source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/src/candidates.rs`, lines 1-10799 (10,799 lines; the Read tool showed 10,800 numbered lines, the last blank). Written paths below are relative to the crate `src/` and cite `candidates.rs:LINE` as `L<line>`. Checkout `4f3c255d...`. Read only; nothing built or run. The crate map section 2.14 was read first.

## 1. Purpose

`candidates.rs` is the independent checker for rewrites that stage 03 (`abstract-operations-to-abstract-operations`) proposes on a `PsiOptimizationUnit`. Stage 03 hands in a `PsiRewriteCandidate` (a patch plus claimed analyses, provenance, substitutions, witnesses); this file recomputes from the unit what the patch must be, applies it to a clone of the unit, re-runs whole-unit validation, and returns a `ValidatedPsiRewrite` or an `OptimizationUnitValidationError::Candidate*` error (file header comment, L1-9).

The file is one tree of nested inline modules (about 90 `mod` blocks, no sub-files). One top-level function dispatches on the patch kind (L10462). Each rewrite family has its own module with a `validate*` entry plus a private `replay` module that does the work.

## 2. Shape (entry points and shared types)

- `validate_psi_rewrite_candidate` (`pub`, L10462): first asks `is_proof_check_elision_rule(candidate.rule())` (L10466); if yes it goes to `validate_proof_check_elision_candidate` regardless of patch kind. Otherwise it matches every `PsiRewritePatch` variant (L10469-10525), one arm per family, no wildcard.
- `ValidatedPsiRewrite` (`pub struct`, L10529): fields `unit`, `candidate: OptimizationCandidateIdentity`, `validator: OptimizationValidatorIdentity`, `provenance: Vec<ProvenanceRewrite>`, all `pub(in crate::candidates)`; accessors `unit`, `candidate`, `validator`, `provenance`, `into_unit` (L10536-10555). The only constructors are inside this file (`accept_region_rewrite` L10793, the pruning replay L1803, `redundant_block_parameter` L2481, and the constant evaluators L3879/L8673/L6451).
- `accept_region_rewrite` (`pub(crate)`, L10762): the common finish. Checks `candidate.affected_blocks() == expected_blocks` and `candidate.provenance() == provenance` (L10771), clones the unit, runs the family closure, calls `crate::unit_validation::revalidate_rewrite_output` (L10780), checks every input block outside `expected_blocks` is byte-equal in the output (`outside_region::validate_outside_region`, L5908-5921), checks every other function is still present and equal (L10786-10792), then builds `ValidatedPsiRewrite` with a validator identity from a hard-coded byte string.
- Shared helpers: `candidate_function/block/node` (L10602, 10715, 10703), `rebuild_node_metadata` (L10611), `set_node_effect_link` (L10630), `land_folded_nodes` (L10644), `sweep_output_nodes` (L10666, renumbers effect links and rebuilds metadata for every node), `retire_node_custody` (L10744, deletes a node and hands its provenance and work to the next node in the block), `require_realized_rows` (L10682), `realize_node_provenance` (L10724), `canonicalize_provenance` (L10758), `provenance_realized_at` / `realized_at_own_site` / `proven_unreachable_at_own_site` (L10558-10600). Macro `candidate_patch!` (L19-25) pulls a patch variant out or returns `CandidatePatchMismatch`.
- Rule contracts: `contract` module (L353-465): `RuleContract` (L368: `rule_domain`, `required_analyses`, `invalidated_analyses`, `safety_class`, `predicted_cost_delta`) and a `rule_rows!` table of 11 rows (L404-416). `admit_origin` (L418: runs `validate_psi_optimization_unit(input)` then `candidate.input() == input.identity`), `admit_roster` (L429), `admit` (L447: origin, rule identity, roster).

Family modules, with the entry function and the patch variant:

| Family | Module / entry | Lines |
|---|---|---|
| case membership specialization | `case_membership_specialization::validate_case_membership_specialization_candidate` (`pub`) | L27-291 |
| installing a constant at a node | `constant_installation::install_evaluated_constant` | L292-352 |
| adjacent block merge | `control_flow_cleanup::block_merging::adjacent` | L475-648 |
| non-adjacent block merge | `block_merging::non_adjacent` | L649-819 |
| constant conditional fold | `constant_conditionals` | L823-1089 |
| linear empty block thread | `empty_block_threading::linear` | L1091-1295 |
| path-qualified empty block thread | `empty_block_threading::path_qualified` | L1296-1499 |
| shared terminal jump fusion | `shared_jump_fusion` | L1503-1731 |
| unreachable private machine pruning | `unreachable_private_machines` | L1732-1962 |
| redundant block parameter (copy propagation) | `copy_propagation` | L2038-2504 |
| dead scalar elimination | `dead_scalar_elimination` | L2505-2924 |
| field value specialization | `field_value_specialization::validate_field_value_specialization_candidate` (`pub`) | L2925-3133 |
| scalar CSE / GVN (same block, dominating, phi-translated) and total scalar identities | `global_value_numbering` | L3134-5820 |
| closed-node live boundary | `observation::reconstruct_closed_scalar_node_boundary` | L5821-5901 |
| proof-check elision (identity operand substitution, integer constant laws, dead scalar route) | `proof_check_elision` | L5923-6909 |
| provenance accounting reconstructors | `rewrite_accounting` | L6910-7786 |
| SCCP-based boolean and integer constant evaluation | `sparse_conditional_constant_propagation` | L7787-9657 |
| state argument specialization | `state_specialization::validate_state_argument_specialization_candidate` (`pub`) | L9658-10304 |
| operand rewriting | `value_operand_rewrites::rewrite_value_operands` | L10305-10440 |

## 3. Inputs and outputs

Input: `&PsiOptimizationUnit` (from crate `optimization_unit`) and `&PsiRewriteCandidate`; plus `abstract_operations::AbstractOperation` (matched everywhere), `semantic_vocabulary` ids, `optimization_core` analysis/rule/validator identities, `terminal_psi` records (L53, L1892). Calls into other files of this crate: `crate::unit_validation::{revalidate_rewrite_output, derived_metadata::{expected_definitions, expected_edges, expected_ownership, expected_uses}}` (L334, L1108, L2289), `crate::current_value_ranges::independently_reconstruct_value_range_fact_at` (L8203, L8374), `crate::validate_psi_optimization_unit` (L366). Output: a new `PsiOptimizationUnit` inside `ValidatedPsiRewrite`, or an error. No global state, no I/O. Every family clones the whole unit (`input.clone()`: L314, L1795, L2438, L10775).

Errors used (all `OptimizationUnitValidationError::`): `CandidatePatchMismatch`, `CandidateAnalysisContractMismatch`, `CandidateLocationMissing`, `CandidateReachabilityMismatch`, `CandidateIncomingBindingMismatch`, `CandidateSubstitutionMismatch`, `CandidateProvenanceMismatch`, `CandidateWorkMismatch`, `CandidateObservationMismatch`, `CandidateLiveBoundaryMismatch`, `CandidateOperandFactMismatch`, `CandidateEvaluationMismatch`, `CandidateAcceptedObligationFactMismatch`, `CandidateSafetyClassMismatch`, `CandidateOutsideRegionMismatch`, `CandidateBlockParameterMismatch`, `CandidateRegionObservationUnavailable`, `CandidateRegionObservationMismatch`, `CandidateInputMismatch`, `CurrentValueRangeFactMismatch` (grepped by reading; used as listed).

## 4. How it decides (what each judgment is keyed on)

General pattern: the validator recomputes what the patch should contain and compares it to the candidate with `!=`. Keyed on identities (ids: `MachineId`, `BlockId`, `EdgeId`, `ValueId`, `OperationId`, `NodeLocation{machine,block,node-index}`) and positions (node index within a block, block index within `function.blocks`), plus hashed identities of rules and validators.

- **Rule identity, from spelling.** Rule and validator identities are made with `OptimizationRuleIdentity::from_canonical_bytes(b"omega.psi-rule....vN")` and compared by value (L453, L2738-2758, L4803-4875, L6552-6559, L6607-6646, L8039-8076, L8250-8266, L8509-8519, L9016-9112). So which family and sub-law a candidate belongs to is decided by comparing the candidate's rule hash against a list of byte-string literals. The same family gets different version suffixes on the rule and the validator: rule `constant-conditional-fold.v5` (L410) vs validator `constant-conditional-fold.v4` (L929); rule `redundant-block-parameter.v1` (L412) vs validator `.v2` (L2485); rule `adjacent-single-predecessor-block-merge.v5` (L405) vs validator `.v5` (L590). Read, not interpreted.
- **Dispatch by rule first, then by patch.** `is_proof_check_elision_rule` (L6594) is tried before the patch match (L10466), so a candidate whose rule is one of the proof-check rules goes to the proof-check router (L6892) whatever its patch. `RemoveDeadScalarNode` patches go to `validate_dead_scalar_node_candidate` (L10496), which itself re-checks the rule against `dead_scalar_family` (L2735).
- **Block merges** (L501-635, L672-806): keyed on node location of the predecessor's last node (`patch.predecessor`), the incoming `EdgeId`, the target `BlockId`, and block adjacency in `function.blocks` (`target_position == predecessor_position + 1`, L518; the non-adjacent one rejects exactly that, L690). Both require exactly one incoming edge to the target (L553, L710).
- **Constant conditional fold** (L847-976): keyed on the condition's `ScalarConstantFactIdentity` via `literal_boolean_fact`; selected arm chosen by the recomputed boolean (L873).
- **Empty block threading**: keyed on `patch.empty.node == 0` and a single-node block (L1123, L1135); jump bindings composed through `reconstruct_linear_thread_bindings` (L7523).
- **Shared terminal jump fusion** (L1528-1647): target must be a one-node block ending in a return pattern whose first provenance is an `Edge` (L1553-1562); needs at least two incoming edges (L1569); `independently_post_dominates` (L1649) is a local fixed-point post-dominator computation over `expected_edges` of each block's last node.
- **Unreachable private machine pruning** (L1750-1811): recomputes the complement of the reachable set from `unit.entry`, dispatch-table rows, provider candidates, and functions with `attachment.is_some()` (L1842-1859), then follows `validator_machine_references` (L1881): cleanup-roster `InvokeOwned` machines plus `unit.operation_machine_references(operation)`. Candidate must match exactly (L1771-1793).
- **Redundant block parameter** (L2308-2489): keyed on `(machine, block, position, parameter ValueId)`; requires every edge into the block to carry the same `replacement` argument (L2404-2410) and the replacement to have the same `ScalarType` (L2346); then compares closed-region observations before and after (`reconstruct_psi_closed_region_observation`, L2426-2479).
- **Dead scalar** (L2803-2906): keyed on operation shape through `independently_validated_dead_scalar_shape` (L2605) and rule family; the node must have no uses (`function.node_uses()`, L2831), not be live-out (L2830), and `retirable_scalar_node` plus `receiver_custody_distinct` (both from `optimization_unit`, L2801) must hold.
- **Scalar CSE/GVN** (L4045-4417 local/dominating, L4446-4764 phi-translated): keyed on `IndependentScalarExpressionKey` (an enum of operator, types and value ids, with commutative operands ordered by `independent_pair`, L3954; non-commutative ones keep order), on node location, and on `independent_reachable_dominators` (L3226). The validator requires the candidate's leader to be the canonical leader: first earlier node in the same block (L4258-4279) or, for dominating scope, the `min_by_key` of `(dominator-set size, location)` (L4313-4320).
- **Total scalar identities** (L5597-5669) and **proof certified identities** (L5950-6095): keyed on `TotalScalarIdentityKind` / `ProofCertifiedScalarIdentityKind` plus operation shape; the neutral literal (0, 1, all-ones) must be a materialised `IntegerConstant` node (`materialized_integer_constant`, L6492) whose `IntegerConstant` fact and fact identity match.
- **Integer constant laws** (L6363-6459): five laws in a table (`integer_constant_laws`, L6598-6647) each with a `recognise` and `verify_evidence` function pointer.
- **Scalar evaluation** (L8579-8681, L7812-7887): keyed on witness facts (`IntegerEvaluationWitness`) that must match identities recomputed by `validator_scalar_constant_facts` (L9319): a private SCCP re-run (`ValidatorSccpValue` Unknown/Constant/Overdefined, L9313).
- **State argument specialization** (L9741-9880 `plan_dispatch`): keyed on the dispatch block's final `Conditional` and whether each incoming edge's binding resolves to a constant via `constants` map, `delivered_argument` (L9882, walks single-predecessor chains), or `call_result_constant` (L9922: callee with exactly one `Return` and a known constant fact). `cyclic_machines` (L9682) excludes any machine whose block graph has a cycle (DFS `has_cycle` L9704).
- **Case membership / field value specialization**: recompute replay rows (`plan_memberships` L119, `field_value_reads` imported from `optimization_unit` L2941) and require `patch.memberships == replayed` (L249) / `patch.reads == replayed` (L3052). Both exclude `cyclic_machines` machines (L238, L3041).
- **Source spelling / display names**: no judgment uses a display name or a `format!`. The only string compares are the byte-string rule and validator identities above.

## 5. Cases by shape

- "`let [empty_node] = empty_block.nodes.as_slice()`" (L1135, L1339): empty-block threading handles only a block with exactly one node (a jump).
- Path-qualified thread: refuses any incoming edge with non-empty `cleanup_actions` (L1366); refuses when the only incoming predecessor ends in a `Jump` (L1402-1413) (that case belongs to the linear rule).
- `require_scalar_bindings` (L1994): every block merge/thread/fusion refuses any function where a block has `structural_parameters`. `CandidateIncomingBindingMismatch`.
- `patched_jump_bindings` (L1973): the predecessor operation must be `O::Jump` with empty `cleanup_actions` (L1988); a `Conditional` arm is not accepted for the merges. (The path-qualified thread and phi-translated CSE do use `rewrite_successor_operation`, which handles `Jump` and `Conditional` only, L7419-7454.)
- Adjacent merge `eligible_first` (L526-542): target's first node must be (a) no successors and first provenance an `Operation`, or `Edge` with a return pattern, or (b) a `Conditional` with two successors and empty provenance. Other terminal operations are refused.
- Constant conditional: `reachable_blocks_after_conditional_fold`; refuses if an unreachable block has structural parameters (L895-898).
- Dead scalar `independently_validated_dead_scalar_operation_family` (L2522-2596): an exhaustive match, no wildcard; families `Literal`, `UnconditionallyTotal`, `ProofCertified`; everything else (storage, calls, structural, float, atomics, control flow, activation claims...) returns `None`.
- Expression keys for CSE (L3354-4017): three key enums. Total keys cover boolean constant/not/equal, integer constant/equal/less/less-or-equal/bitwise/widen, wrapping and saturating add/subtract/multiply, wrapping shifts. Proof keys cover exact cast, exact shifts, exact arithmetic, and wrapping/saturating divide and remainder. Compatible-policy keys cover shifts and add/subtract/multiply but return `None` for divide and remainder (L3514).
- Total scalar identity table: 29 `TotalScalarIdentityKind` values routed by four sub-classifiers (L5246-5281); validator names come from seven rule-domain constants (L5582-5595).
- Proof identity: 13+ `Kind` arms in one `match (policy, operator, identity)` (L6139-6206); `ExactIntegerShiftRightNegativeOneValue` additionally requires `IntegerCarrier::Fixed` and signed (L6254).
- Remainder-by-unit-divisor: `IntegerCarrier::Fixed` only (L6813); self-divide requires fixed carrier and that `one` is admitted (L5696-5699 style at L6696-6699); width-parameterised types are tested to be refused (tests L6727-6755, L5457-5504).
- `all_ones` for an unsigned type with unknown maximum returns `None` (L5443-5450); comment: "A missing unsigned endpoint supplies no law literal."
- Dispatch specialization `plan_dispatch`: dispatch block must not be the entry, have no structural parameters (L9749), its last node must be `Conditional`, with either a direct parameter condition (empty prefix) or `integer_comparison_condition(...)`; incoming owner nodes must be `Jump` or `Conditional` with empty structural bindings and cleanup (L9798-9835). If every incoming edge resolves, `edges.clear()` (L9876-9878) and the caller rejects the empty plan (L9255).
- `rewrite_value_operands` (L10309-10439): an exhaustive match over `AbstractOperation` with no wildcard arm, so a new operation variant fails to compile there. `rewrite_scalar_value_uses` (L7389) handles `Jump`, `Conditional`, `StructuralCase => {}`, else delegates.
- Scalar evaluation `binary_operation_shape::recognize` (L8691): arithmetic, shift, bitwise and/or/xor; everything else `CandidatePatchMismatch`. `validate_scalar_evaluation_candidate` (L9625) lists the 17 other patch variants explicitly and returns `CandidatePatchMismatch`.
- Range comparisons: `classify` for range-vs-constant maps six rule byte-strings to six kinds (L8035-8095); range-vs-range three (L8246-8285); constants three (comparison) in `rule_identity` (L8505-8524).

## 6. Same job elsewhere

- Dominators: `independent_reachable_dominators` (L3226) is an iterative set-intersection dominator computation. Greps for `independent_reachable_dominators|fn dominators|fn compute_dominators|fn post_dominators` under `psi-to-native` show other implementations at `pipeline/03_.../control_flow_analyses.rs:231` (`dominators(unit, reverse)`) and `representations/.../optimization-unit/src/structural_places.rs:571` (`pub fn dominators(`). Not read; "suspected" separate implementations. Inside this crate, `current_ownership.rs:1994, 2849` and `current_value_ranges.rs:6, 62, 73` call the one at L3226 (grepped).
- Post-dominators: `independently_post_dominates` (L1649) is a private fixed-point loop; stage 03's `PostDominators` analysis is suspected to compute the same thing (`reverse` argument at `control_flow_analyses.rs:231`, not read).
- Within this file the same logic appears twice or more:
  - Adjacent and non-adjacent merge `replay::validate` (L501-635 vs L672-806) share about 80 percent of their steps and body of the output closure.
  - Linear and path-qualified thread share the "empty block parameter used elsewhere" check verbatim (L1140-1153 vs L1344-1357).
  - Case-membership and field-value specialization both do evidence, replay, `folded_node`, `land_folded_nodes` (L144-206, L2946-3008).
  - Scalar CSE leader selection is duplicated for same-block (L4258), dominating (L4288), and phi-translated (L4590-4675).
  - Obligation-fact checks `function.facts.iter().any(|fact| matches!(fact, OptimizationFact::OperationObligationReference {...}))` appear at L2839, L2861, L4141, L4187, L4228, L4518, L4617, L4630, L5647, L6017, L6044, L6849.
  - Three "is this literal materialised and does its fact match" blocks: L5525-5553 (total identity), L6008-6033 (proof identity), L6832-6865 (unit divisor).
  - `reconstruct_*_ownership_is_identity` (L6934, L7557) are the same "all-or-none frontier facts with the same snapshot" check over 3 and 6 sites.
  - `has_cycle` (L9704) vs `cyclic_machines` callers: one use site each in 3 families.
- The "producer" side (stage 03 `control_flow_cleanup.rs`, `rules.rs`, `proof_check_elision.rs`, `semantic_analyses.rs`, `value_ranges.rs`) reuses names of items defined here: `reconstruct_linear_thread_bindings`/`..._ownership_is_identity` are imported by `pipeline/03_.../control_flow_cleanup.rs:472, 540, 548, 919, 977, 983` (grepped), so the "independent" checker's helpers also serve the proposer. `materialized_integer_constant|independently_accepted_operation_fact|validator_scalar_constant_facts|observation_at(|literal_boolean_fact|scalar_value_definition` outside this file: 16 occurrences in 6 files (`grep -c`, includes `optimization_unit` hits). `cyclic_machines|validator_active_source_ordinals|unreachable_private_machine_complement|validator_machine_references|pruned_machine_provenance` outside this file: 9 occurrences in 4 stage-03 files (`rules.rs`, `control_flow_cleanup.rs`, `control_flow_analyses.rs`, `control_flow_analyses_tests.rs`). Stage 03 therefore calls helpers named "validator_*" (suspected shared code, not read).

## 7. Unused or one-caller code

Grep: `validate_psi_rewrite_candidate|validate_case_membership_specialization_candidate|validate_field_value_specialization_candidate|validate_state_argument_specialization_candidate` across `source-rust` = 21 occurrences in 4 files. Outside `candidates.rs`:
- `validate_psi_rewrite_candidate`: `pass_manager.rs:363` (import), `:643` (the one production call), plus `control_flow_analyses_tests.rs` (import L197, L308; calls L275, L401).
- `validate_case_membership_specialization_candidate`, `validate_field_value_specialization_candidate`, `validate_state_argument_specialization_candidate`: re-exported `pub` from `lib.rs:240-243`; no call sites outside `candidates.rs` and `lib.rs` (the grep with `candidates.rs` excluded shows only `lib.rs` lines). They are called only inside this file by `validate_psi_rewrite_candidate`'s match (L10519-10524, L10516). So these three have 0 external callers and 1 internal caller.
- `lib.rs:241` `pub(crate) use ... validate_dead_scalar_node_candidate` and `:245` `reconstruct_closed_scalar_node_boundary` are crate internal (used here at L2828, L8845 and in other crate files; not counted).
- `reconstruct_linear_thread_bindings` and `reconstruct_linear_thread_ownership_is_identity`: `pub fn` in a `pub(crate) mod` (L7523, L7557), re-exported by `lib.rs:79, 81`; callers outside: 3 each in `control_flow_cleanup.rs` (grepped above).
- `pub fn reconstruct_linear_thread_bindings` is also called here at L1190, L1371.
- `ValidatedPsiRewrite`: `pub`; external use in `pass_manager.rs:363` (import); accessors not separately counted.
- Comment-claimed canonical/single ways: the file header (L1-9) says candidates are routed "to the independent replay checker of the pass family"; `validate_psi_rewrite_candidate` is that router and has one production caller (`pass_manager.rs:643`).
- `candidate_validation::validate_proof_certified_scalar_identity_candidate` is `pub(crate)` and re-exported `pub(crate)` at L6885; called at L6901 only (inside file), grepped by reading.
- `rule_catalog` in `dead_scalar_elimination`: `expected_safety`, `validator_bytes` used once each (L2913, L2891).

## 8. Stated limits

Few explanatory comments exist in the file (read all). Comment lines:
- L1-9 header (above).
- L5444-5445: "A missing unsigned endpoint supplies no law literal. Zero identities above remain independent of that endpoint."
- No comment uses "unsupported", "migration", "remains", or "TODO" (read; not separately grepped for this unit).
- Rustfmt skip attributes at L388, L403, L6551 (tables).
- `#[allow(clippy::too_many_arguments)]` at L8162, L8352.

## 9. Fail-open risks

None of these has a comment at the site saying why accepting is right.

1. L6959-6963 `reconstruct_adjacent_merge_ownership_witness`: if none of the three ownership-frontier facts exist for the incoming edge entry, edge exit and target block entry, it returns an empty `OwnershipFrontierWitness` provided the function has no `structural_parameters`, `entry_claim_declarations`, `declared_places`. Wrong program that could pass: a function with ownership events but no recorded frontier facts and none of those three declaration sets. Classification: open (not traced to where `ownership_frontier_facts` get populated). The same shape at L7578-7581 (`reconstruct_linear_thread_ownership_is_identity`, returns `true`). Also the candidate must carry the same witness (L578), so the candidate side must also produce the empty witness.
2. L1585 `checked_scalar_substitutions(candidate, &target.parameters, bindings, |_, _| true)` in shared terminal jump fusion: the dominance test used by the two merges (`independently_replacement_dominates_uses`, L562, L725) is replaced by "always admit". What could pass: a substitution whose replacement does not dominate the target's uses. Traced (partly, by reading): the clone replaces parameters with the predecessor edge's arguments inside the predecessor's own terminal node (L1604-1627), so uses are at the predecessor's last node, which is where the arguments are available; but `rewrite_scalar_substitutions` is applied only to the clone and not to other blocks. Marked "traced by reading, not verified by test".
3. L5913-5919 `validate_outside_region`: compares only blocks that exist in the input. A block present in the output but absent from the input would not be flagged here; the stated catch is `revalidate_rewrite_output` (L10780) and the `functions.contains` check (L10786-10792), which only looks at input functions. A function added to the output that was not in the input is not checked in this file. Open (the rest of the crate was not read).
4. L2274-2277 `rewrite_block_parameter_operation`: `O::WriteOnlyPrimitiveStore { range_obligation: Some(_), .. } => {}` leaves the operation unchanged, so a use of the removed parameter in that operand stays. Wrong program: a redundant parameter used by such a store would keep the stale `ValueId`. Traced by reading: the next steps rebuild metadata (L2457-2460) and call `revalidate_rewrite_output` (L2463), which is expected to reject an undefined value, but I did not read that function. Open.
5. L7411 `rewrite_scalar_value_uses`: `O::StructuralCase { .. } => {}` does no substitution. Wrong program: a scalar value used as input to a structural case would not be replaced after a CSE or merge substitution. The operation's own scalar inputs are listed as none in `rewrite_value_operands` (L10405, `StructuralCase {..} => {}`), so consistent; open on whether `StructuralCase` can read a scalar.
6. L9840-9843 `constants.get(...).copied().or_else(... call_result_constant ...)`: a callee with a single `Return` and one constant fact makes the caller's value constant. A recursive or non-terminating callee (never returns) has no `Return` and yields `None`; a callee that can crash before returning is treated as returning the constant when it does return, which is valid for the dispatch arm taken after the call. Stated: none. Open.
7. L1957 `validate_unreachable_private_machines_candidate` accepts a candidate whose `affected_blocks` is empty, as required; not a risk.
8. L9876-9878: all-edges-resolve returns empty and rejects. This fails closed.
9. L6586 `ProofCheckValidationRoute::for_rule`: unknown rule returns `None`, and the router returns `CandidateAnalysisContractMismatch` (L6906): fails closed. `dead_scalar_family` unknown rule: `None`, closed.
10. L10466 `is_proof_check_elision_rule` early return: a rule on the proof-check list routes to proof-check validation even when the patch kind is a different family; the specific validators then reject a patch of the wrong kind with `candidate_patch!` mismatches (L5969, L6372). Fails closed.
11. L3187 `ScalarCseProofClass::CompatiblePolicy => None` in `independent_cse_expression`: callers special-case `CompatiblePolicy` before calling it (L4127-4130, L4153-4157). Fails closed.
12. `.expect("... exists")` at L317, L320, L760, L1436, L2440, L4403 and similar: these panic rather than accept; each is after a `candidate_function`/`block` check, so reachable only if the clone differs from the input.

Fail-open summary: none seen of type "unknown rule/variant accepted"; items 1 to 5 are the places where the absence of data leads to acceptance, all "open" except item 2 "traced (partly)".

## Coverage

- Read in full: lines 1-10799 of `candidates.rs` (10,799 lines), in nine Read calls (1-1400, 1401-2800, 2801-4200, 4201-5600, 5601-7000, 7001-8400, 8401-9800, 9801-10800).
- Skimmed: none.
- Not read: none.
- Commands run for counts: Grep count for `#[test]` in the file (2 hits: L5457, L6727); Grep count of the four public validate function names across `source-rust` (21 occurrences in 4 files); Grep (content) of the same names plus `reconstruct_linear_thread_*` excluding `candidates.rs`; Grep count for `reconstruct_closed_scalar_node_boundary|fn independent_reachable_dominators|fn independently_post_dominates|fn cyclic_machines|fn has_cycle` (17 occurrences in 3 files, 15 in `candidates.rs`); Grep for dominator function names under `psi-to-native`; Grep count of seven helper names excluding `candidates.rs` (16 occurrences in 6 files) and of five pruning helper names (9 in 4 files).

## Five things a maintainer most needs to know about this unit

1. The whole file is a recomputing validator, not a rule engine: every family rebuilds the expected patch, expected affected blocks and expected provenance from the input unit and compares with `!=` (e.g. L249, L3052, L9258). Changing what a rewrite emits means changing both the proposer in stage 03 and the replay here.
2. Which family a candidate belongs to is decided by the candidate's rule identity hash compared to byte-string literals, and the proof-check rules are routed before the patch kind is looked at (L10466). Rule and validator version suffixes differ for the same family (L410 vs L929; L412 vs L2485).
3. `accept_region_rewrite` (L10762) is the one common gate: it requires exact affected blocks and provenance, revalidates the whole output unit, and checks that blocks outside the region are untouched. Families that do not use it (unreachable pruning L1750, redundant block parameter L2308, constant installation L292 and the integer-constant law and SCCP evaluators) do their own checks and build `ValidatedPsiRewrite` themselves.
4. Three specializations (case membership, field value, state argument) refuse any machine whose block graph contains a cycle (`cyclic_machines`, L9682). Everything else is acyclic-or-cyclic agnostic.
5. Dominators, post-dominators, SCCP and live-boundary analyses are re-implemented locally in this file as "independent" versions, and some of these helpers are also imported by stage 03 and by `current_ownership.rs`/`current_value_ranges.rs` in this crate (so "independent" is by code path, not by crate).

## What you are unsure about

- Whether `revalidate_rewrite_output` (in `unit_validation.rs`, not in this unit) closes the fail-open items 3 and 4: I did not read it.
- Whether the dominator and SCCP re-implementations here are byte-for-byte equal in meaning to stage 03's and `optimization_unit`'s; I only grepped their names.
- Whether `StructuralCase` operations can read scalar values (item 5).
- The meaning of the different version suffixes (v4/v5, v1/v2); I recorded only the mismatch.
- The count of "about 90 `mod` blocks" is from reading; I did not run a count command for `mod ` declarations.
