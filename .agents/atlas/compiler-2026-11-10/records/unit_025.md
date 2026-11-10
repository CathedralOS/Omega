# Unit 25 notes

Files covered (all under `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/`):
1. `candidates.rs` lines 1-10699
2. `current_ownership.rs` lines 1-9965
3. `current_value_ranges.rs` lines 1-500

(Notes are written progressively; see Coverage at the end for what was actually read.)

## candidates.rs (read progressively; section 1: lines 1-3000)

### Purpose
Header comment (candidates.rs:1-9): every proposed `PsiRewriteCandidate` is routed to an independent replay checker for the pass family that proposed it. Each checker validates the analysis contract, replays the rewrite on a clone of the input unit, checks provenance disposition and work conservation, rebuilds derived metadata and unit identity, revalidates the whole unit, and confirms everything outside the declared region is untouched, then issues `ValidatedPsiRewrite`.

### Shape (lines 1-3000)
The file is a tree of nested `pub(crate) mod` per rewrite family, each with a private `mod replay`/`validation` holding `validate(input, candidate)` and a thin public `validate_*_candidate` wrapper that calls `contract::admit` first.
- `candidate_patch!` macro (19-25): destructure `candidate.patch()` into one `PsiRewritePatch` variant else `CandidatePatchMismatch`.
- `case_membership_specialization` (27-291): `MembershipEvidence` (55), `membership_evidence` (62), `admit_membership_node` (76), `plan_memberships` (119), `folded_node` (144), `validate` (223), wrapper `validate_case_membership_specialization_candidate` (277). Replaces `StructuralCaseMembership` nodes with `BooleanConstant` when a case is established (`established_case_at`) or the type has a sole case (`resolved_sole_case`).
- `constant_installation` (292-352): `install_evaluated_constant` (305) overwrites a node with a constant op, clears `uses/successors/ownership`, revalidates, and compares closed scalar observation and live boundary before/after.
- `contract` (353-465): `RuleContract` (368) and a `rule_rows!` table (404-416) of 11 rules: ADJACENT_BLOCK_MERGE v5, NON_ADJACENT_BLOCK_MERGE v1, LINEAR_EMPTY_BLOCK_THREAD v2, PATH_QUALIFIED_EMPTY_BLOCK_THREAD v1, SHARED_TERMINAL_JUMP_FUSION v2, CONSTANT_CONDITIONAL_FOLD v5, UNREACHABLE_PRIVATE_MACHINE_PRUNING v1, REDUNDANT_BLOCK_PARAMETER v1, CASE_MEMBERSHIP_SPECIALIZATION v1, FIELD_VALUE_SPECIALIZATION v1, STATE_ARGUMENT_SPECIALIZATION v1; each with required analyses, invalidated analyses, safety class, optional predicted cost delta. `admit_origin` (418) runs `validate_psi_optimization_unit` and checks `candidate.input() == input.identity`; `admit_roster` (429); `admit` (447) checks rule identity then roster.
- `control_flow_cleanup` (466-2037): sub-modules `block_merging::{adjacent,non_adjacent}` (475-822), `constant_conditionals` (823-1089; also `reconstruct_conditional_fold_accounting` 978), `empty_block_threading::{linear,path_qualified}` (1090-1502), `shared_jump_fusion` (1503-1731; own `independently_post_dominates` 1649), `unreachable_private_machines` (1732-1962), helpers `patched_jump_bindings` (1973), `require_scalar_bindings` (1994), `checked_scalar_substitutions` (2007).
- `copy_propagation` (2038-2504): `normalize_redundant_parameter_observation_input/operation` (2054/2113), `unchanged_outside_redundant_parameter_region` (2189), `rewrite_block_parameter_operation` (2223), `validate_redundant_block_parameter_candidate` (2308 inner, 2493 wrapper). Removes a block parameter whose every incoming binding is the same replacement value.
- `dead_scalar_elimination` (2505-2922): `independently_validated_dead_scalar_operation_family` (2522), `dead_scalar_family` rule-by-bytes (2733), `validate_dead_scalar_node_candidate` (2801, 2906).
- `field_value_specialization` (2923-...): begins `folded_node` (2944) turning `StructuralScalarFieldRead` into Boolean/Integer constants (continues past 3000).

### Inputs/outputs
Reads `PsiOptimizationUnit`, `PsiRewriteCandidate`; produces `ValidatedPsiRewrite { unit, candidate, validator, provenance }` (e.g. 1803-1810, 2481-2488) via `accept_region_rewrite` (defined later in file, used e.g. 266, 585, 924). Each replay clones the unit and mutates `output` in a closure passed to `accept_region_rewrite`.

### How it decides (keyed on)
- Rule admission is keyed on byte-string rule identity: `OptimizationRuleIdentity::from_canonical_bytes(contract.rule_domain)` (453-455) and in `dead_scalar_family` chain of `==` on byte literals (2736-2756). Identity keyed on canonical bytes, not display names.
- Patch/candidate agreement keyed on `NodeLocation`/`BlockId`/`EdgeId`/`ValueId` equality (e.g. 228, 506, 553, 1187).
- Membership fold keyed on `StructuralPlaceDeclaration.id`, path, and `StructuralCaseId` (94-116).
- Dead scalar keyed on the `AbstractOperation` variant list (2522-2594) plus obligation presence (2596) and function facts (2837-2875).

### Cases by shape
- Block merging adjacent: target must be exactly the next block (`target_position != predecessor_position + 1`, 518), predecessor merge node must be last (543), first node of target must be a node with no successors and Operation provenance, or Edge provenance with return/return_structural_or_crash, or a 2-successor Conditional with empty provenance (526-542).
- Non-adjacent merge: target nonempty and not a lone `Jump` (689-693).
- `require_scalar_bindings` (1994-2005): any block with `structural_parameters` makes all cleanup rewrites (merge, thread, fusion) reject with `CandidateIncomingBindingMismatch`. Only scalar bindings supported.
- Linear thread: exactly one empty-block node (`let [empty_node]`, 1135) and exactly one incoming predecessor (1170).
- Path-qualified thread: any incoming edge with `cleanup_actions` non-empty rejects (1366); a single incoming whose node is a `Jump` rejects (1401-1411).
- Shared terminal fusion: target block has exactly one node `[terminal]` that is return-shaped with Edge provenance (1550-1561); needs >= 2 incoming edges (1569).
- Redundant block parameter: `WriteOnlyPrimitiveStore { range_obligation: Some(_) }` operations are left unrewritten in `rewrite_block_parameter_operation` (2274-2277), while normalization path has no such arm (falls to `rewrite_value_operands`).
- Dead scalar: three families only (Literal, UnconditionallyTotal, ProofCertified), operation list 2531-2544; all storage/call/atomic/etc. variants return None (2545-2592).

### Same job elsewhere (inside this section)
- `reconstruct_*_accounting` plus `canonicalize_provenance`/`realized_at_own_site`/`proven_unreachable_at_own_site` are shared across families (e.g. 217-219, 1007-1012, 1922-1944).
- Adjacent and non-adjacent merge replay closures are near-duplicates (596-631 vs 757-803), differing in which provenance is extended (first.successors.is_empty() vs first.provenance.is_empty()).
- Normalization of a redundant parameter exists twice: `normalize_redundant_parameter_observation_operation` (2113-2187, errors on mismatch) and `rewrite_block_parameter_operation` (2223-2283, `expect`/panics via `remove` on mismatch).
- `independently_post_dominates` (1649) is a local post-dominator fixpoint; `independent_reachable_dominators` is imported from `global_value_numbering` for dominators (657). (Suspected: a post-dominator analysis also exists in analysis crates; not read.)

### Fail-open risks (section 1)
- `checked_scalar_substitutions(..., |_, _| true)` in shared terminal fusion (1581-1586): the `admits` predicate (which in the merge paths checks dominance, 557-570, 720-733) is always true. Stated? No comment at site. Reasoning (inferred): fusion clones a terminal into the predecessor, so parameter uses are substituted rather than dominance-sensitive. Mark "open".
- `unreachable_private_machine_complement` (1834-1879): reachability roots are entry, dynamic-dispatch table rows, provider candidates, functions with `attachment`; references via `validator_machine_references`. A machine referenced only through something `operation_machine_references` or cleanup rosters don't expose would be pruned (fails closed toward removal, not acceptance). Not traced.
- `validator_active_source_ordinals` (1813-1832): ordinal gaps from `pruned_machines`; if `unit.functions` is shorter than expected a function is silently not mapped and the `source_ordinals[machine]` index at 1782 would panic. Open.

### Stated limits (section 1)
- None quoted; limits are encoded as rejections above.

## candidates.rs section 2: lines 3001-6100 (read in full)

### Shape
- `field_value_specialization` (2923-3131): `replay::validate` (3024) checks `SpecializeFieldValue` patch: `cyclic_machines(input)` containing the machine rejects (3039); `field_evidence(function, place)` (3043); replays `field_value_reads` and requires exact equality with `patch.reads` (3049-3052); substitutions must equal `expected_substitutions` (3008, 3053). Constants become `BooleanConstant`/`IntegerConstant` via `folded_node` (2944); `Forward` rows rewrite uses to the forwarded initializer and retire the read node (3078-3113). Wrapper `validate_field_value_specialization_candidate` (3120) is `pub fn`.
- `global_value_numbering` (3132-5761): sub-modules
  - `admission` (3139): `independent_cse_expression` (3151) by `ScalarCseProofClass::{ObligationFree, ProofCertified, CompatiblePolicy}`; CompatiblePolicy returns `None` here (3185) and is handled by separate leader/redundant functions; `independently_accepted_operation_fact` (3189) requires an `OptimizationFact::OperationObligationReference` in `function.facts` AND an `accepted_obligation_fact` on the unit.
  - `dominance_reconstruction` (3219): `independent_reachable_dominators` (3224, iterative set-intersection dominators over reachable blocks, `BTreeMap<BlockId, BTreeSet<BlockId>>`), `independently_replacement_dominates_uses` (3304).
  - `expression_keys` (3352): key enums `IndependentTotalScalarExpressionKey` (3961), `IndependentProofScalarExpressionKey` (3985), `IndependentCompatiblePolicyScalarExpressionKey` (4001), `IndependentScalarExpressionKey` (4010); `independent_pair` (3952) sorts commutative operand pairs by ValueId. Total keys at 3658-3942; proof keys at 3528-3645; compatible-policy at 3366-3516.
  - `local_and_dominating` (4016-4416): `validate_scalar_common_subexpression_candidate` (4043) for same-block and dominating CSE; requires canonical leader (first matching earlier node in the block, 4257-4277; for dominating, min by dominator-set size then location, 4286-4318).
  - `phi_translated` (4417-4763): `validate_phi_translated_scalar_common_subexpression_candidate` (4444): translates redundant op through each incoming edge's bindings, finds a canonical leader per edge, requires >= 2 incoming (4684), then adds a new block parameter and rebinds every incoming edge.
  - `rule_catalog` (4764-4874): `ScalarCseScope`, `ScalarCseProofClass`, `scoped_proof_class` (4801), `phi_translated_proof_class` (4840): rule identity tables of byte strings (4806-4859).
  - `total_scalar_identity` (4875-5677): `classification` (4931-5447) with `bitwise_absorbing`, `bitwise_neutral`, `saturating`, `wrapping` classifiers; `evidence::independently_validate_law_literal` (5460); `validation::validate_total_scalar_identity_candidate` (5538); `exact_rule_validator_bytes` (5612) maps identity kind to rule/validator byte domains (7 rule domains, 5523-5536).
  - `scalar_value_types` (5689) builds `BTreeMap<ValueId, ScalarType>` from function parameters, block parameters and node definitions.
  - Public wrappers: `validate_local_...` (5712), `validate_dominating_...` (5727), `validate_phi_translated_...` (5742), `validate_candidate_origin` (5755).
- `observation` (5762-5842): `ClosedScalarObservationBoundary` (5768) and `reconstruct_closed_scalar_node_boundary` (5774): backwards liveness fixpoint on `node.definitions`/`node.uses`/`block.parameters`; returns live_in/live_out of one node.
- `outside_region` (5843-5863): `validate_outside_region` (5849): every input block not in `expected_blocks` must be equal (`==`) in the output function.
- `proof_check_elision` (5864-...): `validate_proof_certified_scalar_identity_candidate` (5891), `identity_classification` (6038-) for exact-policy identities (Add zero, Subtract zero, Multiply one, Divide one ...).

### How it decides
- Rule identity by byte-string `from_canonical_bytes` compare: 4806-4859, 5523-5536, 5672.
- CSE leader identity by expression key (operation kind + operand ValueIds + integer type) at `local_and_dominating` 4170; commutative ops normalised by ValueId order (3952).
- Obligation admission keyed on presence of `OptimizationFact::OperationObligationReference { support == operation }` in `function.facts` (e.g. 4139-4142, 4185-4191) plus `input.accepted_obligation_fact(...)` identity (3207-3216).
- Law-literal identity (x + 0 etc.) keyed on the law operand being a materialized integer literal node whose value equals the law constant, plus an `OptimizationFact::IntegerConstant` and a recomputed fact identity (5460-5499).
- Liveness keyed on ValueId sets (5779-5838).

### Cases by shape
- Total scalar identity: fixed list of 25 identity kinds across 4 classifiers (5240-5280; wrapping Add/Sub/Mul/shift, saturating Add/Sub/Mul, bitwise And/Or/Xor with zero/all-ones). `typed_integer` uses `value as i128` for signed (5436); `all_ones` special-cases signed -1 / unsigned maximum (5441-5446).
- Compatible-policy CSE: only shift and arithmetic Add/Sub/Mul; Divide/Remainder return None (3512-3514). Proof-certified keys cover Exact add/sub/mul/div/rem plus Wrapping/Saturating divide/remainder (3602-3633); Wrapping/Saturating Add/Sub/Mul return None (3630-3633).
- Total CSE: Boolean/integer constants, Not, Equal, Less/LessOrEqual, bitwise Not/And/Or/Xor, Widen, wrapping shift, wrapping+saturating add/sub/mul (3671-3940). Everything else `_ => return None` (3940).
- Same-block CSE: leader must precede redundant in same block; dominating: different block (4110-4121). Redundant node must not be the last node in its block (4107, 4473).
- Phi-translated: `expected_incoming.len() < 2` rejects (4684).
- `leader.successors`, `ownership` must be empty and definitions exactly one (4336-4358).

### Same job elsewhere
- Dominators: `independent_reachable_dominators` (3224) is imported by `control_flow_cleanup` block merging (657). Post-dominators: `independently_post_dominates` local to shared_jump_fusion (1649). Suspected (not read) duplicates in analysis crates; the word "independent" in names means these deliberately re-derive facts rather than trust the producer.
- Two identity classifiers: `total_scalar_identity::classification` (4931) for obligation-free identities and `proof_check_elision::identity_classification` (6038) for exact/proof-certified identities; two near-identical validators (5538 vs 5891) with identical outer structure (`admit_origin`, `admit_roster` with ScalarConstants/UseDefinition/EffectSummaries, substitutions equal single `ScalarSubstitution`, law literal check).
- Dead-obligation fact checks (`OperationObligationReference` + `accepted_obligation_fact`) are written inline at least at: 2837-2851, 3189-3217, 5985-5995, 4139, 4185, 4226, 4515, 4615-4631 (read).
- Liveness fixpoint at 5774 vs dominator fixpoint 3224 vs post-dominator fixpoint 1649: three hand-written dataflow loops.

### Fail-open risks
- `checked_scalar_substitutions(..., |_, _| true)` at 1585 (carried from section 1): open.
- `phi_translated` leader availability (4590-4596): a leader in the same block as the source counts as available iff `node_index + 1 < source.nodes.len()` i.e. not the terminator; in other blocks iff dominator. No explicit check that leader precedes use within the source block beyond "not last node". Inferred: nodes in a block run before the terminator edge, so any non-terminal node is available at the edge. Traced (read), no wrong program seen.
- `ScalarCseProofClass::CompatiblePolicy` `(None, Some(redundant))` arm (4225-4249): leader must have no obligation fact, redundant must have an accepted obligation fact. Dropping an exact-policy (obligation-carrying) op for a wrapping/saturating leader is allowed only through this arm; the key equality at 4170 compares `CompatiblePolicy` keys which erase policy (3466-3516: ShiftLeft/Add etc. have no policy field). So the compatible-policy key equates Exact `a+b` with Wrapping/Saturating `a+b`. Stated? no comment at site; the rule name says "compatible-policy". Read detail: leader side accepts only `obligation.is_none()` rows (3377, i.e. Wrapping or Saturating policy, 3449-3451) and redundant side only `obligation.map(..)` rows (3397, Exact policy), so the redundant is always an Exact op whose obligation is separately required as an accepted fact (4234-4247); the key does not distinguish Wrapping from Saturating leaders, so a Saturating leader can replace an Exact redundant op. Whether that is sound rests on the Exact obligation proving no overflow (inferred, not traced).
- `unchanged`: `validate_outside_region` compares whole blocks (5856), fail-closed.

### Stated limits
- None quoted in this range.

## candidates.rs section 3: lines 6101-7500 (read in full)

### Shape
- `proof_check_elision` continued: `identity_classification::independent_proof_certified_scalar_identity` (6060) recognises exact/wrapping/saturating identities (add/sub zero, mul one, div one, zero dividend, mul zero, exact shift by zero count / of zero value / of -1 right shift). `integer_constant_law` (6225-6418): `IndependentIntegerConstantLaw` (6266) is a table row `{rule_domain, validator_domain, required_analyses, recognise: fn, verify_evidence: fn}` and `validate_integer_constant_law_candidate` (6304) replaces an integer op by `IntegerConstant` through `constant_installation::install_evaluated_constant` (6373). `literal_operand::materialized_integer_constant` (6433). `rule_catalog` (6465-6593): `OPERAND_SUBSTITUTION_RULES` table (6493-6500; 6 rules), `ProofCheckValidationRoute::{DeadScalar, OperandSubstitution, IntegerConstantLaw}` (6480) with `for_rule` (6518), `integer_constant_laws()` (6539; 5 laws: self-subtract, self-remainder, self-divide, remainder-by-one, signed remainder-by-negative-one). `same_operand` (6594) and `unit_divisor` (6662) hold the law recognisers/evidence. Entry `validate_proof_check_elision_candidate` (6796) dispatches by route.
- `rewrite_accounting` (6814-...): shape-specific "accounting" reconstruction (expected affected blocks + provenance rows) per rewrite: `adjacent_merge` (6825; ownership witness `reconstruct_adjacent_merge_ownership_witness` 6847, accounting 6890), `common_subexpression` (6983), `dead_scalar` (7037; `reconstruct_dead_scalar_node_accounting` 7047, `reconstruct_removed_result_use_accounting` 7096), `non_adjacent_merge` (7131; also tracks effect-index shift per block 7193-7218), `scalar_identity` (7254; macro `scalar_identity_reconstruction!` 7262 generates two functions that both delegate to `reconstruct_removed_result_use_accounting`), `substitutions` (7289; `rewrite_scalar_value_uses` 7293, `rewrite_successor_operation` 7323), `terminal_fusion` (7360), `threading` (7414-: `reconstruct_linear_thread_bindings` 7427, `reconstruct_linear_thread_ownership_is_identity` 7461, accounting from 7493).

### How it decides
- Rule route keyed on byte-identity of rule domain (6519-6529, 6505, 6513).
- Operand-substitution identity: pattern on (overflow policy, operator, identity kind) triples (6080-6147), the identity operand must be a materialized `IntegerConstant` node of the exact value (5954-5966), plus `OptimizationFact::IntegerConstant` fact and recomputed `literal_scalar_constant_fact_identity` (5967-5974), plus the accepted obligation fact identity (5985-5996).
- Integer-constant-law: the node must have no events, no successors, no ownership, `crash == No` and `suspension == No` in `observation_at` (6347-6352); left/right must be live-in and result live-out (6357-6362); provenance must equal exactly one `realized_at_own_site` row (6364-6371); output obligation facts must be unchanged (6387).
- Ownership-frontier identity for merges and threads keyed on `unit.ownership_frontier_facts` rows by `(machine, site)` and equality of `.snapshot` across consecutive rows (6858-6872, 7477-7490).

### Cases by shape
- Self-divide: only fixed carrier and not signed 1-bit (`positive_one_is_representable`, 6637-6644); self-remainder has no carrier check (6621-6630); remainder-by-one needs `left != right`, fixed carrier (6717-6720), and divisor literal `scalar_type.one()` or signed -1 (6684-6693).
- Exact shift right of -1 only for fixed signed carriers (6195-6196).
- `substitutions::rewrite_scalar_value_uses` (7293-7321) skips `StructuralCase` entirely (`O::StructuralCase { .. } => {}`, 7315) and relies on `rewrite_value_operands` for all other ops. `rewrite_successor_operation` (7323-7358) handles only `Jump` and `Conditional`; every other op returns false.
- Linear-thread bindings: composes by replacing outgoing arguments that are empty-block parameters with the incoming argument (7448-7455).
- `threading` ownership identity: if NO ownership fact exists for any of the sites, accepts iff function has no `structural_parameters`, `entry_claim_declarations`, `declared_places` (7482-7486, same at 6863-6868).

### Same job elsewhere
- Ownership-frontier "identity" check is written twice: `reconstruct_adjacent_merge_ownership_witness` (6847, 3 sites, returns witness) and `reconstruct_linear_thread_ownership_is_identity` (7461, 6 sites, returns bool); the empty-facts fallback condition is the same expression in both (6864-6866 vs 7483-7485).
- `reconstruct_dead_scalar_node_accounting` (7047) is the common base for `reconstruct_removed_result_use_accounting` (7096), CSE (6995, 7008) and scalar-identity (7262 macro): all count as one family.
- `rewrite_scalar_value_uses` (7293) vs `rewrite_block_parameter_operation` (2223) vs `normalize_redundant_parameter_observation_operation` (2113): three per-operation use-rewriters that each special-case Jump/Conditional/StructuralCase and fall to `value_operand_rewrites::rewrite_value_operands` (defined elsewhere in this file, not yet read).

### Unused or one-caller code
(Greps run at the end; see "Grep counts" below.)

### Fail-open risks
- `7315 O::StructuralCase { .. } => {}` in `rewrite_scalar_value_uses`: a StructuralCase op carrying a use of the substituted value would not be rewritten. Stated? no comment. Traced: all callers (block merges, CSE, identity) are guarded by `require_scalar_bindings` for merges and by the nodes being scalar ops; whether StructuralCase has scalar value operands was not checked (open).
- `reconstruct_adjacent_merge_ownership_witness` (6863-6868) and `reconstruct_linear_thread_ownership_is_identity` (7482-7486): when the unit has no ownership facts at all for these sites, ownership is accepted as identity if the function has no structural params / entry claims / declared places. Input reaching it: any function without structural/claim/place declarations. Traced (read): a function with none of those has no ownership frontier to disturb (inferred).
- `integer_self_remainder_shape` (6621): no check on carrier, accepted for any integer type with an obligation (`constant_law_shape` requires `arithmetic.obligation()?`, 6409), so only ops carrying a proof obligation qualify.

## candidates.rs section 4: lines 7501-9000 (read in full)

### Shape
- `rewrite_accounting::threading` continued: `reconstruct_linear_thread_accounting` (7493), `reconstruct_path_thread_accounting` (7542), `effect_audit` (7590: walks all non-empty blocks and flags any node whose `effect.input/output` is not the running counter `expected_effect`, `expected_effect+1`, adds its block to `affected` and realizes provenance for it). Same counter walk appears inline in `reconstruct_conditional_fold_accounting` (1017-1068).
- Free helpers in `rewrite_accounting`: `realize_merged_target_provenance` (7626), `preserve_edge_custody` (7654), `rewrite_scalar_substitutions` (7670; implements substitution by calling `rewrite_block_parameter_operation` with a fabricated `RedundantBlockParameterRewrite { position: 0 }` at 7677-7687).
- `sparse_conditional_constant_propagation` (7691-...): `boolean_candidate::validate_boolean_evaluation_candidate` (7716), `boolean_evaluation` (7793; `boolean_literals` 7794, `integer_comparisons` 7868 with `constant`, `range_against_constant` 7923, `range_against_range` 8137; `rule_identity::validate` 8403 maps operation variant to a single rule name), `integer_candidate::validate_integer_evaluation_candidate` (8483), `integer_evaluation` (8587-: `binary_operation_shape::recognize` 8595, `binary_semantics::evaluate` 8688, `literal_facts` 8787 with `literal_integer_fact` 8834, `direct_literal_integer_fact` 8852, `literal_boolean_fact` 8889, `rule_identity` 8907).

### How it decides
- Boolean/integer folding keyed on: rule identity bytes per operation variant (8413-8421, 8920-8996), a `scalar_evaluation_witness` carried by the candidate naming `ScalarConstantFactIdentity` / `ValueRangeFactIdentity` per operand, and recomputed facts from `validator_scalar_constant_facts(input, function)` (8840) or `current_value_ranges::independently_reconstruct_value_range_fact_at` (8107, 8278, 8288).
- Range comparison folding is on integer min/max intervals: e.g. `RangeLessThanConstant` true if `max < c`, false if `!(min < c)` (8022-8027); range-against-range with `same_value` shortcut: same value compared to itself is `true` unless kind is `LessThan` (8200-8205).
- Safety class derived from policy: Exact arithmetic/shift => ProofCertified, Wrapping/Saturating add/sub/mul and wrapping shift => ExactOperationSemantics; Wrapping/Saturating divide/remainder => ProofCertified (8700-8750). Candidate must declare the same class (8513, 7757).
- Integer evaluation requires an accepted obligation fact exactly when class is ProofCertified (8516-8551).

### Cases by shape
- Boolean eval: only `BooleanNot`, `BooleanEqual` (literals), and `IntegerEqual/LessThan/LessOrEqual` (8449-8457). Constant folding of boolean candidates is limited to those five ops.
- Integer eval `recognize` (8595-8676): IntegerArithmetic, IntegerShift, BitwiseAnd/Or/Xor; unary ops (exact cast, widen, bitwise not) go through a different function not yet read.
- Range comparisons: six one-sided kinds (8011-8043) and three pair kinds (8144-8148).
- `rewrite_successor_operation` (7323) only handles Jump and Conditional (see section 3).

### Same job elsewhere
- Literal-constant-fact lookup exists as `literal_integer_fact`/`literal_boolean_fact` (8834/8889, via `validator_scalar_constant_facts`), `direct_literal_integer_fact` (8852, direct node lookup), `proof_check_elision::literal_operand::materialized_integer_constant` (6433) and `total_scalar_identity::evidence::independently_validate_law_literal` (5460) and `unit_divisor::materialized_divisor_evidence` (6730): at least four routes to "this operand is a specific integer literal".
- Constant-install pattern: `constant_installation::install_evaluated_constant` (305) is called from boolean eval (7770), integer eval (8563) and integer-constant laws (6373): the single install path for evaluated constants in this range (3 callers read).
- Rule-name-by-operation tables: `boolean_evaluation::rule_identity` (8403), `integer_evaluation::rule_identity` (8907) and `contract::rule_rows!` (404) each hold byte-string rule names.

### Fail-open risks
- `range_against_range::evaluate` with `same_value` (8200-8205): returns true for Equal and LessOrEqual, false for LessThan, with no check that the value is not subject to differing observations at the node (same ValueId is the same SSA value, inferred). Traced as sound for SSA values.
- Boolean eval `rule_identity::validate` mismatch is fail-closed (8424-8426).
- None else seen in this range.

## candidates.rs section 5: lines 9001-10699 (read in full)

### Shape
- `integer_evaluation::unary_operations::evaluate` (9029): exact cast, widen, bitwise-not; `evaluate_integer_operation` (9111) = rule-name check, unary, else binary shape + operand literals + `binary_semantics::evaluate`.
- `sparse_conditional_constant_propagation::observation` (9169): `observation_at` (9174) calls `reconstruct_psi_observation_model(unit)` (from `optimization_unit`) and linear-searches the node rows by (machine, block, node); `same_closed_scalar_observation` (9188) compares 11 fields.
- `snapshot_reconstruction` (9205-9518): `ValidatorSccpValue {Unknown, Constant, Overdefined}` (9217); `validator_scalar_constant_facts` (9223) re-runs a sparse conditional constant propagation over one function: parameters Overdefined (9258), block parameters Unknown (9262), node definitions Overdefined (9265), `OptimizationFact::{BooleanConstant,IntegerConstant}` seeded Unknown until the supporting block is reachable (9284-9309), then a fixpoint following Jump/StructuralCase edges always and Conditional edges by constant condition (9331-9360); returns `(ValueId, constant, ScalarConstantFactIdentity)` where identity is either a literal fact identity or `derived_sccp_scalar_constant_fact_identity` over the snapshot (9405-9424). `validator_sccp_snapshot` (9430) builds `SccpMachineSnapshot`. `validator_integer_value_type` (9507).
- `validate_scalar_evaluation_candidate` (9527): dispatch on integer vs boolean patch; every other `PsiRewritePatch` variant listed explicitly returns `CandidatePatchMismatch` (9538-9556).
- `state_specialization` (9560-10206): `cyclic_machines` (9584; DFS cycle detection per function, used by case-membership 238, field-value 3039, state-argument 10147 to reject looping machines), `plan_dispatch` (9643), `delivered_argument` (9784), `call_result_constant` (9824), `fused_node` (9853), `fused_edge` (9997), `node_uses` (10047), `accepted_provenance` (10073), `validate` (10126), `validate_state_argument_specialization_candidate` (10192). Threads constant arguments of a state-dispatch block: when a predecessor passes a known constant to a dispatch block whose terminator is a Conditional on that parameter (directly or via `integer_comparison_condition`), the predecessor edge is rewritten to jump straight to the resolved target.
- `value_operand_rewrites::rewrite_value_operands` (10211-10339): one exhaustive match over `AbstractOperation` calling `replace` on each ValueId operand.
- Top level: `validate_psi_rewrite_candidate` (10362) is the single public dispatcher (match on `PsiRewritePatch`, 10369-10425; proof-check-elision rules routed first by rule identity at 10366). `ValidatedPsiRewrite` (10429) with private-to-module fields (`pub(in crate::candidates)`). Helpers: `provenance_realized_at` (10458), `realized_at_own_site` (10472), `proven_unreachable_at_own_site` (10480), `provenance_proven_unreachable_at` (10488), `candidate_function` (10502), `rebuild_node_metadata` (10511), `set_node_effect_link` (10530), `land_folded_nodes` (10544), `sweep_output_nodes` (10566), `require_realized_rows` (10582), `candidate_node` (10603), `candidate_block` (10615), `realize_node_provenance` (10624), `retire_node_custody` (10644), `canonicalize_provenance` (10658), `accept_region_rewrite` (10662).

### How it decides
- `accept_region_rewrite` (10662-10699) is the common gate: requires `candidate.affected_blocks() == expected_blocks` and `candidate.provenance() == provenance` (10671); clones input, applies rewrite closure, `revalidate_rewrite_output` (10680), `validate_outside_region` for input blocks not in `expected_blocks` (10681), and requires every other function to be present unchanged in output (10686-10692). Keyed on block ids and struct equality.
- `retire_node_custody` (10644): removes node and moves its provenance/work onto the NEXT node in the block; errors with `CandidateLocationMissing` if there is no next node (10649-10652), which is why many callers reject redundant nodes that are last (e.g. 4107, 4473, 7055).
- `sweep_output_nodes` (10566): renumbers all nodes in the function and rebuilds `definitions/uses/successors/ownership` from the operation (`rebuild_node_metadata`, 10511) with effect links as a running counter 0.. over ALL blocks in order (10570-10577); successors keep existing provenance/work via `preserve_edge_custody` (7654).
- SCCP re-derivation (9223): constant facts keyed on the function's own `facts` plus control-flow reachability; `Conditional` with non-constant, non-Overdefined condition (Unknown) contributes no successors (9357).
- `plan_dispatch` (9643): requires dispatch block not entry, no structural parameters (9651), last node `Conditional`, either direct parameter condition when prefix empty or `integer_comparison_condition` (9663-9671); the parameter itself must not be a known constant (9672); each incoming edge must come from a plain Jump or a Conditional arm with no structural bindings/cleanup actions and a constant for the binding argument; incoming edges that don't qualify are skipped with `continue` (9708-9750); `if edges.len() == incoming.len() { edges.clear(); }` (9778-9780): if ALL incoming edges are specializable no rewrite is planned (the dispatch block would become dead).
- `call_result_constant` (9824-9851): looks up the callee by an `O::Call` whose `result == argument`; callee must have exactly one `O::Return` node, and the returned value must be a constant under `validator_scalar_constant_facts` of the callee; cross-function reasoning based on the unit as given.

### Cases by shape
- Quote 9651: "block.id == function.entry || !block.structural_parameters.is_empty()" (dispatch rejected). Predecessor operation must be `Jump` or `Conditional` (`fused_node` 9882-9993; other ops => `CandidatePatchMismatch`); a Jump predecessor must have exactly one row (9890).
- `has_cycle` (9606): any cycle in the block graph rejects the machine from case-membership, field-value and state-argument specialization (callers 238, 3039, 10147).
- `rewrite_value_operands` match is exhaustive (no `_` arm), so new `AbstractOperation` variants must be added there; control-flow ops (`Jump`, `Conditional`, `StructuralCase`) are `{}` no-ops (10307) and handled by callers.
- Reads of `Return { value }` are replaced (10308), `ReturnUnit` and `return_structural_or_crash_pattern!` are not (10336-10337).

### Same job elsewhere
- Cycle/dominator/post-dominator/liveness/SCCP: five independent hand-written dataflow or graph routines in this file (cycle 9606, dominators 3224, post-dominators 1649, liveness 5774, SCCP 9223).
- `unreachable_private_machine_complement` (1834) is a callgraph reachability worklist; the SCCP reachable-block loop (9311-9395) and `independent_reachable_dominators` (3241-3249) each compute block reachability with their own loop.
- `realize_node_provenance` (10624) vs inline loops that do the same thing: 1048-1068 effect loop, 7590 effect_audit, 1915-1927 `pruned_machine_provenance`.

### Unused or one-caller code
(Grep counts below.)

### Fail-open risks
- `plan_dispatch` (9708-9750): non-qualifying incoming edges are skipped silently (`continue`), so the plan covers a subset of predecessors; this is fail-closed (fewer rewrites). Not a risk.
- `call_result_constant` (9824): uses the unique `Return` of the callee. A callee with exactly one `Return` node but other exit routes (crash) still yields a constant; that is right if the call only completes normally (inferred; not traced). Also does not check the callee is non-recursive or that `Call` carries no other effects; the replay equality `patch.edges == replayed` (10160) ensures the producer and validator agree, but both share this rule, so a wrong rule is not caught by the replay (inferred).
- `rewrite_value_operands` with `O::WriteOnlyPrimitiveStore` is handled by its caller in `rewrite_block_parameter_operation` (2274-2277: `range_obligation: Some(_)` => no rewrite) but NOT in `normalize_redundant_parameter_observation_operation` (2113) or `rewrite_scalar_value_uses` (7293): which arm of `rewrite_value_operands` covers `WriteOnlyPrimitiveStore` is the `primitive_local_triple_pattern!` / `constant_value_triple_pattern!` macro membership (10248, 10319), not read. Open.
- `accept_region_rewrite` (10686-10692): other functions must be unchanged, but `unit.pruned_machines`, `accepted_obligation_facts` and other unit-level tables are only protected by `revalidate_rewrite_output` and the identity recompute (not read), except where individual validators compare them (6387, 8905, 6028). Open.

### Stated limits
- No "unsupported/TODO" comments found anywhere in candidates.rs (only the header comment at 1-9). Searched by reading; no grep was run for comments.

## Grep counts (candidates.rs)
- `validate_psi_rewrite_candidate` (the single public dispatcher, 10362): Grep count mode over omega-rust: candidates.rs 1 (definition), semantics/lib.rs 2, pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager.rs 2.
- `cyclic_machines|install_evaluated_constant|reconstruct_closed_scalar_node_boundary|independent_reachable_dominators` combined count: candidates.rs 32, current_value_ranges.rs 6, current_ownership.rs 2, lib.rs 1, pipeline `rules.rs` 3, pipeline `control_flow_analyses.rs` 1 (45 total). Note `control_flow_analyses.rs:133` defines its own `fn cyclic_machines(&self, unit)` (a second cycle-detection routine in the producer pipeline; not read, "suspected" duplicate of `candidates.rs:9584`).
- `provenance_proven_unreachable_at` is defined twice: `candidates.rs:10488` and `pipeline/01_abstract-operations-to-abstract-operations/src/control_flow_cleanup.rs:1491` (with 5 call sites in that pipeline file at 405, 422, 434, 1388, 1401). Same helper twice across the producer/validator split (suspected duplication; other site not read).
- `preserve_edge_custody` has 1 call site (candidates.rs:10525); `set_node_effect_link` has 1 call site (candidates.rs:10576); both grepped over omega-rust.
- `reconstruct_conditional_fold_accounting` (978): grepped, only caller is candidates.rs:901 (1 call site); `independently_post_dominates` (1649): 1 definition, 1 call (1578).
- `realized_at_own_site` / `canonicalize_provenance` definitions exist only in candidates.rs (10472, 10658) by the same grep (no duplicate `fn` named so in omega-rust).

## current_value_ranges.rs (lines 1-500, read in full)

### Purpose
Header (1-8): re-derive and validate an optimizer-produced `ValueRangeFact` against the current unit without calling the analysis that produced it: re-derives scalar constants, verifier proposition custody, interval bounds and CFG dominance, and checks the fact's authority reaches the requested operation entry.

### Shape
- `mod availability` (15-119): `validate_current_value_range_fact_at` (25) requires the machine/block exist, `node < block.nodes.len()`, `fact.valid_in.machine == machine`, `value_available_at(...)` and `scope_applies_at(...)`; any failure yields `CurrentValueRangeFactNotApplicable(site)`. `scope_applies_at` (63): `EntireValue` always true; `DominatedOperationEntry{block: anchor, node: anchor_node}` true if same block and `node >= anchor_node`, otherwise `dominated_blocks.binary_search(&block).is_ok()`. `value_available_at` (85): function parameter always; block parameter if defined in same block or its block dominates; node definition earlier in same block or dominating block.
- `mod reconstruction` (120-475): `reconstruct_value_range_fact` (139) handles the two `ValueRangeSupport` variants. `ScalarConstant(identity)` (149): must be `EntireValue` scope with empty `dominated_blocks`, finds an integer constant in `validator_scalar_constant_facts` with that identity, min == max == constant. `AcceptedOperationProof {accepted, question, operation}` (176): scope must be `DominatedOperationEntry` anchored at the operation (189), anchor node must have a `proof_range_goal` (196), anchor unique (`is_unique_proof_anchor` 413), accepted obligation fact identity and canonical proposition must match (205-213), exactly one `proof_questions` row matching owner/obligation/proposition/`canonical_certificate`/`terminal_psi == unit.psi`/proof bundle fingerprint/canonical identity (214-232), then `extract_integer_intervals(&proposition)` bounds for the value (234-248); a bound that is the full type range is rejected (249-252); region `dominated_blocks` recomputed from `independent_reachable_dominators` (254-269). `independently_reconstruct_value_range_fact_at` (286, `pub(crate)`) searches ALL reachable blocks/nodes for an anchor whose reconstructed fact has the given `ValueRangeFactIdentity` AND is available/in-scope at the query site (296-407). `new_fact` (441) computes `value_range_fact_identity`. A local `trait ExactlyOne` (467-474) gives `exactly_one()` on iterators.
- Public surface: `validate_current_value_range_fact` (478, `pub(crate)`: runs `validate_psi_optimization_unit`, reconstructs, requires `expected == *fact`), `validate_current_value_range_fact_at` (491, `pub fn`, re-exported at lib.rs:244), `pub(super) use ... independently_reconstruct_value_range_fact_at` (476).

### How it decides
Keyed on identities and equality: `ValueRangeFactIdentity`, `AcceptedObligationFactIdentity` (`candidate.identity == accepted`, 212), proof-question identity, canonical proposition order key (`terminal_codec::canonical_proposition_order_key`, 203), operation id `operation != scope_operation` (189), and structural equality of the whole reconstructed fact (485). Uniqueness of the anchor = exactly one node whose `proof_range_goal` names the operation and exactly one `OperationObligationReference` fact for (obligation, operation) (413-438).

### Cases by shape
- Only two support kinds (`ScalarConstant`, `AcceptedOperationProof`) are reconstructable (148-283). Integer types only (`as_integer()?`, 156; `ScalarType::Integer` check 239/354-357).
- Interval extraction yields `partial.lower/upper` optional; missing side is the type min/max (243-248); fully unbounded is rejected (249-252, 361-364).

### Same job elsewhere
- Dominator-set computation is `independent_reachable_dominators` from `candidates.rs:3224` (imported at 16, 121); it is recomputed from scratch for every call to `value_available_at` (101, 112) and `independently_reconstruct_value_range_fact_at` (295) (read; no caching).
- A producer-side value-range analysis exists at `pipeline/01_abstract-operations-to-abstract-operations/src/value_ranges.rs` (3 occurrences of the validate/reconstruct names by grep; not read) (suspected parallel implementation of the same range reasoning).

### Unused or one-caller code
Grep (count mode) over omega-rust for `validate_current_value_range_fact_at|validate_current_value_range_fact\b|independently_reconstruct_value_range_fact_at|reconstruct_value_range_fact\b`: lib.rs 2, current_value_ranges.rs 9, candidates.rs 3, pipeline value_ranges.rs 3 (17 total). So the `pub fn` `validate_current_value_range_fact_at` is called from outside the crate only from pipeline `value_ranges.rs` (inferred from the grep file list; not read).

### Stated limits
None found.

### Fail-open risks
- `scope_applies_at` (63-82): for `DominatedOperationEntry`, when `block != anchor`, applicability is `dominated_blocks.binary_search(&block).is_ok()`; this requires `dominated_blocks` to be sorted (binary search). Reconstruction builds it from a `BTreeMap` iteration (254-258, 376-381) so it is sorted for reconstructed facts, and `validate_current_value_range_fact` demands exact equality with the reconstruction before `validate_current_value_range_fact_at` calls `scope_applies_at` (498-499), so the order is traced (read) as safe.
- `value_available_at` for `BlockParameter` (96-104) accepts any query when `definition_block == block`, i.e. regardless of `node`; correct since block parameters are defined at block entry (inferred).
- None other seen.

## current_ownership.rs lines 1-1100 (read in full); lines 1101-9965 NOT READ

### Purpose (from the header comment 1-9 and the part read)
Header: reconstruct and replay live ownership frontiers across each function's CFG from its current operations and signatures; track live claims, non-unrestricted owned places, partial-affine field move paths and outstanding reference loans across block entries, nodes, call regions, address joins and edges, so disposal obligations, loan ceilings and window restoration contracts hold "without trusting cached ownership events". Only the opening of the file was read, so the overall entry points of this file were NOT seen.

### Shape (lines 1-1100 only)
- Imports 11-25 bring in `OwnershipFrontier*` types from `optimization_unit`, `terminal_psi` structural declarations, and `crate::unit_validation::references::LiveReference`.
- `pub(crate) mod address_joins` (27-...), containing:
  - `borrowed_regions` (28-690): `BorrowedRegions` (62-76; fields `reference_sources`, `aliases`, `parents`, `returned`, `sources`, `descriptor_sources`, `symbolic_sources`, `locations`, `effects`, `flow`, `relations`, `writes`), `Access` (55). `BorrowedRegions::new` (86) takes origins from `call_accesses.take_borrowed_origins(machine)` or computes `origins` (151) which builds a descriptor catalog graph with `crate::descriptor_results::optimized_catalog_graph` and a `DescriptorAnalysis` (terminal_semantics), then `origins_with_analysis` (182). `resolve_at` (564), `resolve` (627, delegates to `terminal_semantics::borrowed_storage::resolve`), `overlaps` (658), `conflicted_operation` (669), `regions_overlap_at` (639), `writes_at` (635).
  - `descriptor_loans` (691-...): `compatibility::validate_compatibility` (711), `DescriptorLoan` (774), `DescriptorLoans` (783) with `new` (791; builds "births" of shared-borrow loans from byte-view field stores, stored call effects, element views/byte subslices, structural call results and `StructuralLeafCopy` of shared descriptors, 796-917), `compatibility_requirement` (1032), `conflicted_operation` (1059, read up to line 1100).

### How it decides (part read)
- A function's structural parameters are treated as borrowed "reference sources" when `access != Owned` and the type is not a shared descriptor (191-195); a parameter with no resolvable origins maps to `None` (194) or to the analysis result (240).
- Descriptor aliasing is keyed on `(Position = (BlockId, node index), PlaceId, path)` and on `DescriptorCallOrigin` variants; `Argument` origins become aliases, while `Invocation`, `Literal`, `AccessFormation`, `Empty` are dropped from alias lists (430-438).
- A loan "escapes" when a stored call effect's destination has no exact location (837-839); for escaped births the authorized set is cleared and `live` is set to every position reachable from the birth (930-946).
- `compatibility_requirement` (1032-1053) only emits a requirement when the loan has an incoming origin, access is `SharedBorrow`, and the demanded root is an exclusive (`Owned`/`MutableBorrow`/`WriteOnlyBorrow`) function parameter (1040-1041, 982-994).
- `validate_compatibility` (711-760): empty requirements => `Ok` (716); a missing `view_graph` or any compatibility failure => `CurrentSharedJoinOriginDisturbed`; residual requirements on the unit entry or provider candidates => error (747-757).

### Cases by shape (part read)
- Alias-building handles only: `EstablishElementView` (376), `element_view_or_byte_subslice_pattern!` (392), `StructuralLeafCopy` with `StructuralCopyPath::Ordinary` and an Unrestricted shared-descriptor result (403-418), returned structural call results with `Argument` origins (447-464), and non-owned structural bindings on successor edges (465-474). Everything else produces no alias row (`_ => None`, 419).
- `resolve_at` (564-614) only follows `descriptor_sources` for projections with `indirect == true`, else falls back to `resolve`.

### Fail-open risks (part read)
- `origins_with_analysis` (204): if `graph` or `analysis` is `None` (catalog graph failed to build, 163-172), `reference_sources`, `returned`, `sources`, `locations`, `effects` and `flow` stay empty, and only the syntactic alias rows (370-476) are produced. Then `compatibility` requirements and descriptor flow come out empty, and `validate_compatibility` (716) returns `Ok` when requirements are empty. What wrong program gets through: a function for which the descriptor catalog graph fails to build would produce no loans from stored call effects, so those conflicts would not be reported. Open (the callers of `optimized_catalog_graph` returning `None` were not traced).
- `access_location` (553-562) falls back to the longest known prefix path (including the empty path) (559-561); a location is returned for a shorter path if none exists for the full path. Open.

### Same job elsewhere / unused code / stated limits
Not assessed for this file (only 1,100 of 9,965 lines read; no grep run for its items).

## Coverage, summary, uncertainty

- **Coverage:**
  - `candidates.rs` lines 1-10699: read in full (10,699 lines). No part skimmed.
  - `current_value_ranges.rs` lines 1-500: read in full (500 lines).
  - `current_ownership.rs` lines 1-1100: read in full (1,100 lines). Lines 1101-9965: NOT READ (8,865 lines).
  - Totals: read 12,299 lines; not read 8,865 lines; unit total 21,164.
- **Five things a maintainer most needs to know about this unit (candidates.rs + current_value_ranges.rs):**
  1. `validate_psi_rewrite_candidate` (candidates.rs:10362) is the single dispatcher; every family validator replays the rewrite on a clone and funnels through `accept_region_rewrite` (10662), which requires `affected_blocks` and `provenance` equality, revalidates the output unit, and checks all blocks outside the region and all other functions are unchanged.
  2. Rule/validator identity is byte-string compare of canonical names (e.g. 453, 2736-2756, 4806-4859, 5523-5536, 6493-6500, 7943-7978, 8413-8993); each rewrite family hard-codes its own list of operation shapes and rejects the rest with `CandidatePatchMismatch`.
  3. The validators deliberately re-derive facts (dominators 3224, post-dominators 1649, liveness 5774, SCCP constants 9223, cycles 9606, value ranges current_value_ranges.rs:139-411) instead of reusing the producer's analyses; these are separate hand-written fixpoints that are recomputed per call without caching.
  4. `require_scalar_bindings` (1994) makes all block merge/thread/fusion validators reject any function that has a block with `structural_parameters`; `cyclic_machines` (9584) makes case-membership, field-value and state-argument specialization reject any machine whose block graph has a cycle.
  5. `retire_node_custody` (10644) moves provenance/work onto the next node and fails if there is none, so every node-removal rewrite excludes the last node of a block (checks at 4107, 4473, 7055); `sweep_output_nodes` (10566) renumbers effect links over the whole function after every rewrite.
- **Unsure about:** whether `StructuralCase`/`WriteOnlyPrimitiveStore` arms in the three value-rewriters (2113, 2223, 7293) are handled consistently (which macro patterns inside `rewrite_value_operands` 10211-10339 cover them was not expanded); whether the compatible-policy CSE key (3366-3516) equating Saturating and Wrapping leaders with Exact redundants is sound (relies on obligation proof, not traced); the exact definitions of `optimization_unit` helpers used throughout (`established_case_at`, `field_evidence`, `retirable_scalar_node`, `receiver_custody_distinct`, `reconstruct_psi_observation_model`) were not read; `current_ownership.rs` is not covered in these notes.
<!-- END -->
