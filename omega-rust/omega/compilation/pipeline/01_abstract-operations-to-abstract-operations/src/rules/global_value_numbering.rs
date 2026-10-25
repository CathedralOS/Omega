//! Optimizer module role: executable entrance. Global value numbering: the
//! pass's exact rule order, one file per rule beneath it -- same-block,
//! dominator and phi-translated common-subexpression elimination in their
//! total, proof-certified and compatible-policy forms, and the seven
//! obligation-free total-scalar identity rules. The three CSE contracts and
//! the exact effect admission every rule consults live here;
//! `expression_keys` owns the canonical expression identities,
//! `total_scalar_identities` the identity rules' shared traversal, and
//! `phi_translated_accounting` the join-parameter provenance accounting.

use std::collections::{BTreeMap, BTreeSet};

use optimization_core::{
    AcceptedObligationFactIdentity, AnalysisInvalidationSet, AnalysisKind, AnalysisSet,
    OptimizationRuleContract, OptimizationSafetyClass,
};
use optimization_unit::{
    NodeLocation, OptimizationFact, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiRewriteCandidate,
};
use semantic_vocabulary::OperationId;

use expression_keys::ScalarExpressionRow;
use semantic_vocabulary::{BlockId, MachineId, ScalarType, ValueId};

use crate::EffectSummaryAnalysis;
use crate::rules::GLOBAL_VALUE_NUMBERING_PASS_NAME;
use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::catalog::BuiltInRuleRegistration;

mod bitwise_absorbing_literal_identity;
mod bitwise_neutral_literal_identity;
mod dominator_proof_certified_compatible_policy_scalar_gvn;
mod dominator_proof_certified_scalar_gvn;
mod dominator_total_scalar_gvn;
mod expression_keys;
mod phi_translated_accounting;
mod phi_translated_obligation_free_scalar_gvn;
mod phi_translated_proof_certified_compatible_policy_scalar_gvn;
mod phi_translated_proof_certified_scalar_gvn;
mod same_block_proof_certified_compatible_policy_scalar_cse;
mod same_block_proof_certified_scalar_cse;
mod same_block_total_scalar_cse;
mod saturating_multiply_zero_annihilation;
mod saturating_neutral_arithmetic_identity;
mod total_scalar_identities;
mod wrapping_multiply_zero_annihilation;
mod wrapping_neutral_arithmetic_identity;
mod wrapping_shift_zero_count_identity;

pub(crate) use bitwise_absorbing_literal_identity::BitwiseAbsorbingLiteralIdentityRule;
pub(crate) use bitwise_neutral_literal_identity::BitwiseNeutralLiteralIdentityRule;
pub(crate) use dominator_proof_certified_compatible_policy_scalar_gvn::DominatorProofCertifiedCompatiblePolicyScalarGvnRule;
pub(crate) use dominator_proof_certified_scalar_gvn::DominatorProofCertifiedScalarGvnRule;
pub(crate) use dominator_total_scalar_gvn::DominatorTotalScalarGvnRule;
pub(crate) use phi_translated_obligation_free_scalar_gvn::PhiTranslatedObligationFreeScalarGvnRule;
pub(crate) use phi_translated_proof_certified_compatible_policy_scalar_gvn::PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule;
pub(crate) use phi_translated_proof_certified_scalar_gvn::PhiTranslatedProofCertifiedScalarGvnRule;
pub(crate) use same_block_proof_certified_compatible_policy_scalar_cse::SameBlockProofCertifiedCompatiblePolicyScalarCseRule;
pub(crate) use same_block_proof_certified_scalar_cse::SameBlockProofCertifiedScalarCseRule;
pub(crate) use same_block_total_scalar_cse::SameBlockTotalScalarCseRule;
pub(crate) use saturating_multiply_zero_annihilation::SaturatingMultiplyZeroAnnihilationRule;
pub(crate) use saturating_neutral_arithmetic_identity::SaturatingNeutralArithmeticIdentityRule;
pub(crate) use wrapping_multiply_zero_annihilation::WrappingMultiplyZeroAnnihilationRule;
pub(crate) use wrapping_neutral_arithmetic_identity::WrappingNeutralArithmeticIdentityRule;
pub(crate) use wrapping_shift_zero_count_identity::WrappingShiftZeroCountIdentityRule;

/// The exact local rule order for this pass.
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, SameBlockTotalScalarCseRule),
        BuiltInRuleRegistration::new(1, SameBlockProofCertifiedScalarCseRule),
        BuiltInRuleRegistration::new(2, DominatorTotalScalarGvnRule),
        BuiltInRuleRegistration::new(3, DominatorProofCertifiedScalarGvnRule),
        BuiltInRuleRegistration::new(4, PhiTranslatedObligationFreeScalarGvnRule),
        BuiltInRuleRegistration::new(5, PhiTranslatedProofCertifiedScalarGvnRule),
        BuiltInRuleRegistration::new(6, SameBlockProofCertifiedCompatiblePolicyScalarCseRule),
        BuiltInRuleRegistration::new(7, DominatorProofCertifiedCompatiblePolicyScalarGvnRule),
        BuiltInRuleRegistration::new(8, PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule),
        BuiltInRuleRegistration::new(9, WrappingNeutralArithmeticIdentityRule),
        BuiltInRuleRegistration::new(10, WrappingShiftZeroCountIdentityRule),
        BuiltInRuleRegistration::new(11, WrappingMultiplyZeroAnnihilationRule),
        BuiltInRuleRegistration::new(12, SaturatingNeutralArithmeticIdentityRule),
        BuiltInRuleRegistration::new(13, SaturatingMultiplyZeroAnnihilationRule),
        BuiltInRuleRegistration::new(14, BitwiseNeutralLiteralIdentityRule),
        BuiltInRuleRegistration::new(15, BitwiseAbsorbingLiteralIdentityRule),
    ]
}

pub(super) fn same_block_contract(
    rule_name: &'static [u8],
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        rule_name,
        GLOBAL_VALUE_NUMBERING_PASS_NAME,
        1,
        AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        safety,
    )
}

pub(super) fn dominating_contract(
    rule_name: &'static [u8],
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        rule_name,
        GLOBAL_VALUE_NUMBERING_PASS_NAME,
        1,
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        safety,
    )
}

pub(super) fn phi_translated_contract(
    rule_name: &'static [u8],
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    dominating_contract(rule_name, safety)
}

pub(super) fn exact_pure_scalar_effect(
    unit: &PsiOptimizationUnit,
    effects: &EffectSummaryAnalysis,
    machine: MachineId,
    block: BlockId,
    node: u32,
) -> bool {
    effects.nodes.iter().any(|row| {
        row.revision == unit.identity
            && row.machine == machine
            && row.block == block
            && row.node == node
            && row.class == crate::EffectClass::PureScalar
            && row.observable == crate::EffectKnowledge::No
            && row.structural_state == crate::EffectKnowledge::No
            && row.crash == crate::EffectKnowledge::No
            && row.suspension == crate::EffectKnowledge::No
    })
}

/// Build a fresh rule-local type roster in declaration order. Later block
/// parameters and node definitions keep the original last-write-wins behavior.
fn function_value_types(function: &PsiOptimizationFunction) -> BTreeMap<ValueId, ScalarType> {
    function
        .parameters
        .iter()
        .map(|row| (row.value, row.scalar_type))
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .map(|row| (row.value, row.scalar_type))
        }))
        .chain(function.blocks.iter().flat_map(|block| {
            block.nodes.iter().flat_map(|node| {
                node.definitions
                    .iter()
                    .map(|row| (row.value, row.scalar_type))
            })
        }))
        .collect()
}

/// One located same-block leader/redundant pair, resolved through the
/// accounting the rule's own candidate constructor consumes. `E` is the
/// rule's per-candidate payload — unit for the obligation-free rule, the
/// accepted obligation fact for the proof-certified and compatible-policy
/// ones.
pub(super) struct SameBlockScalarCseMatch<E> {
    pub affected_blocks: Vec<BlockId>,
    pub provenance: Vec<optimization_unit::ProvenanceRewrite>,
    pub extras: E,
    pub patch: optimization_unit::LocalScalarCommonSubexpressionRewrite,
}

/// The shared same-block scalar CSE proposal: per block the first eligible
/// node per expression key becomes the leader, and each later live,
/// provably-pure duplicate whose next node carries none of its provenance is
/// the redundant the rule rewrites onto it. `context` builds the rule's
/// per-function lookup (`function_value_types` for the obligation-free rule,
/// unit for the proof-certified ones); `leader` and `redundant` carry the
/// rule's own key, eligibility and payload — the compatible-policy rule
/// classifies its obligation-free leaders and its proof-bearing redundants
/// separately; `emit` is its candidate constructor.
pub(super) fn propose_same_block_scalar_cse<K, C, E>(
    unit: &PsiOptimizationUnit,
    analyses: crate::RuleAnalysisView<'_>,
    context: impl Fn(&PsiOptimizationFunction) -> C,
    leader: impl Fn(
        &C,
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &abstract_operations::AbstractOperation,
    ) -> Option<ScalarExpressionRow<K>>,
    redundant: impl Fn(
        &C,
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &abstract_operations::AbstractOperation,
    ) -> Option<(ScalarExpressionRow<K>, E)>,
    emit: impl Fn(SameBlockScalarCseMatch<E>) -> Result<PsiRewriteCandidate, crate::RuleProposalError>,
) -> Result<Vec<PsiRewriteCandidate>, crate::RuleProposalError>
where
    K: Ord,
    E: Copy,
{
    use crate::rules::node_elision::node_elision_accounting;

    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
    let mut candidates = Vec::new();
    for function in &unit.functions {
        let context = context(function);
        for block in &function.blocks {
            let mut leaders = BTreeMap::new();
            for (index, node) in block.nodes.iter().enumerate() {
                let leader_row = leader(&context, unit, function, &node.operation);
                let redundant_row = redundant(&context, unit, function, &node.operation);
                if leader_row.is_none() && redundant_row.is_none() {
                    continue;
                }
                let node_index = u32::try_from(index).expect("optimization node index fits u32");
                if !exact_pure_scalar_effect(unit, effects, function.machine, block.id, node_index)
                {
                    continue;
                }
                let mut establishes_leader = false;
                if let Some(row) = leader_row
                    && !leaders.contains_key(&row.0)
                {
                    leaders.insert(row.0, (node_index, row.1, row.2, row.3));
                    establishes_leader = true;
                }
                if establishes_leader {
                    continue;
                }
                let Some((row, extras)) = redundant_row else {
                    continue;
                };
                let (key, operation, result, scalar_type) = row;
                let Some((leader, leader_operation, leader_result, leader_type)) =
                    leaders.get(&key).copied()
                else {
                    continue;
                };
                if leader_type != scalar_type
                    || !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == result
                    })
                {
                    continue;
                }
                let Some(receiver) = block.nodes.get(index + 1) else {
                    continue;
                };
                if receiver
                    .provenance
                    .iter()
                    .any(|source| node.provenance.contains(source))
                {
                    continue;
                }
                let leader_location = optimization_unit::NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: leader,
                };
                let redundant_location = optimization_unit::NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                let Some((affected_blocks, provenance)) =
                    node_elision_accounting(function, redundant_location, result)
                else {
                    continue;
                };
                candidates.push(emit(SameBlockScalarCseMatch {
                    affected_blocks,
                    provenance,
                    extras,
                    patch: optimization_unit::LocalScalarCommonSubexpressionRewrite {
                        leader: leader_location,
                        redundant: redundant_location,
                        leader_operation,
                        redundant_operation: operation,
                        leader_result,
                        redundant_result: result,
                        scalar_type,
                    },
                })?);
            }
        }
    }
    Ok(candidates)
}

/// One located dominating-leader/redundant pair, resolved through the
/// accounting the rule's own candidate constructor consumes. `E` is the
/// rule's per-expression payload — unit for the obligation-free rule,
/// the accepted obligation fact for the proof-certified one.
pub(super) struct DominatingScalarGvnMatch<E> {
    pub affected_blocks: Vec<BlockId>,
    pub provenance: Vec<optimization_unit::ProvenanceRewrite>,
    pub extras: E,
    pub patch: optimization_unit::DominatingScalarCommonSubexpressionRewrite,
}

/// The shared dominating-leader GVN proposal: gather each function's
/// provably-pure scalar expressions, then for every live redundant whose
/// following node carries none of its provenance, pick the same-key
/// expression in the shallowest dominating block whose replacement
/// definition still dominates every use. `context` builds the rule's
/// per-function lookup; `leader` and `redundant` carry the rule's own key,
/// eligibility and payload — the compatible-policy rule classifies its
/// obligation-free leaders and its proof-bearing redundants separately;
/// `emit` its candidate constructor.
pub(super) fn propose_dominating_scalar_gvn<K, C, E>(
    unit: &PsiOptimizationUnit,
    analyses: crate::RuleAnalysisView<'_>,
    context: impl Fn(&PsiOptimizationFunction) -> C,
    leader: impl Fn(
        &C,
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &abstract_operations::AbstractOperation,
    ) -> Option<ScalarExpressionRow<K>>,
    redundant: impl Fn(
        &C,
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &abstract_operations::AbstractOperation,
    ) -> Option<(ScalarExpressionRow<K>, E)>,
    emit: impl Fn(DominatingScalarGvnMatch<E>) -> Result<PsiRewriteCandidate, crate::RuleProposalError>,
) -> Result<Vec<PsiRewriteCandidate>, crate::RuleProposalError>
where
    K: Ord,
    E: Copy,
{
    use crate::rules::node_elision::node_elision_accounting;

    let Some(crate::AnalysisProduct::Dominators(dominators)) =
        analyses.get(AnalysisKind::Dominators)
    else {
        return Err(crate::RuleProposalError::MissingAnalysis(
            AnalysisKind::Dominators,
        ));
    };
    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
    let mut candidates = Vec::new();
    for function in &unit.functions {
        let machine_dominators = dominators
            .function(function.machine)
            .map(|(_, rows)| rows.as_slice())
            .unwrap_or_default();
        let context = context(function);
        let mut leaders = Vec::new();
        let mut expressions = Vec::new();
        for block in &function.blocks {
            for (index, node) in block.nodes.iter().enumerate() {
                let leader_row = leader(&context, unit, function, &node.operation);
                let redundant_row = redundant(&context, unit, function, &node.operation);
                if leader_row.is_none() && redundant_row.is_none() {
                    continue;
                }
                let node_index = u32::try_from(index).expect("optimization node index fits u32");
                if !exact_pure_scalar_effect(unit, effects, function.machine, block.id, node_index)
                {
                    continue;
                }
                let location = optimization_unit::NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                if let Some(row) = leader_row {
                    leaders.push((row.0, location, row.1, row.2, row.3));
                }
                if let Some((row, extras)) = redundant_row {
                    expressions.push((row.0, location, row.1, row.2, row.3, extras));
                }
            }
        }
        for (key, redundant, redundant_operation, redundant_result, scalar_type, extras) in
            &expressions
        {
            if !use_definitions.uses.iter().any(|(machine, use_site)| {
                *machine == function.machine && use_site.value == *redundant_result
            }) {
                continue;
            }
            let Some(redundant_block) = function.block(redundant.block) else {
                continue;
            };
            let redundant_index = usize::try_from(redundant.node).expect("u32 fits usize");
            let Some(redundant_node) = redundant_block.nodes.get(redundant_index) else {
                continue;
            };
            let Some(receiver) = redundant_block.nodes.get(redundant_index + 1) else {
                continue;
            };
            if receiver
                .provenance
                .iter()
                .any(|source| redundant_node.provenance.contains(source))
            {
                continue;
            }
            let leader = leaders
                .iter()
                .filter(|(candidate_key, location, _, _, candidate_type)| {
                    candidate_key == key
                        && *candidate_type == *scalar_type
                        && location.block != redundant.block
                        && crate::analyses::block_dominates(
                            machine_dominators,
                            location.block,
                            redundant.block,
                        )
                })
                .min_by_key(|(_, location, _, _, _)| {
                    let depth = machine_dominators
                        .iter()
                        .find(|(block, _)| *block == location.block)
                        .map_or(usize::MAX, |(_, rows)| rows.len());
                    (depth, *location)
                });
            let Some((_, leader, leader_operation, leader_result, _)) = leader else {
                continue;
            };
            let replacement_definition = optimization_unit::ValueDefinition {
                value: *leader_result,
                scalar_type: *scalar_type,
                site: optimization_unit::ValueDefinitionSite::Node {
                    block: leader.block,
                    node: leader.node,
                },
            };
            if !use_definitions
                .uses
                .iter()
                .filter(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == *redundant_result
                })
                .all(|(_, use_site)| match replacement_definition.site {
                    optimization_unit::ValueDefinitionSite::Node { block, node }
                        if block == use_site.block =>
                    {
                        node < use_site.node
                    }
                    optimization_unit::ValueDefinitionSite::Node { block, .. } => {
                        crate::analyses::block_dominates(machine_dominators, block, use_site.block)
                    }
                    _ => false,
                })
            {
                continue;
            }
            let Some((affected_blocks, provenance)) =
                node_elision_accounting(function, *redundant, *redundant_result)
            else {
                continue;
            };
            candidates.push(emit(DominatingScalarGvnMatch {
                affected_blocks,
                provenance,
                extras: *extras,
                patch: optimization_unit::DominatingScalarCommonSubexpressionRewrite {
                    leader: *leader,
                    redundant: *redundant,
                    leader_operation: *leader_operation,
                    redundant_operation: *redundant_operation,
                    leader_result: *leader_result,
                    redundant_result: *redundant_result,
                    scalar_type: *scalar_type,
                },
            })?);
        }
    }
    Ok(candidates)
}

/// One phi-translated rule's per-function expression roster: the in-scope
/// leader rows, and the redundancy rows the rule rewrites onto them with each
/// row's own payload `E` — unit for the obligation-free rule, the accepted
/// obligation fact for the proof-certified and compatible-policy ones.
pub(super) type PhiTranslatedScalarGvnRows<K, E> = (
    Vec<(K, NodeLocation, OperationId, ValueId, ScalarType)>,
    Vec<(K, NodeLocation, OperationId, ValueId, ScalarType, E)>,
);

/// One located phi-translated redundancy resolved through the accounting the
/// rule's own candidate constructor consumes.
pub(super) struct PhiTranslatedScalarGvnMatch<E> {
    pub affected_blocks: Vec<BlockId>,
    pub provenance: Vec<optimization_unit::ProvenanceRewrite>,
    pub extras: E,
    pub patch: optimization_unit::PhiTranslatedScalarGvnRewrite,
}

/// The shared phi-translated scalar CSE proposal: per function each rule
/// names its own leader/redundancy roster (its expression keys, eligibility
/// and payloads), then every redundant whose incoming joins all translate the
/// key to an existing in-scope leader becomes one candidate. `rows` owns the
/// rule's classification; `key_references_parameters` and `translate` name
/// that key's operand query and join translation; `emit` is the rule's
/// candidate constructor.
pub(super) fn propose_phi_translated_scalar_gvn<K, E>(
    unit: &PsiOptimizationUnit,
    analyses: crate::RuleAnalysisView<'_>,
    rows: impl Fn(
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &EffectSummaryAnalysis,
        &BTreeMap<ValueId, ScalarType>,
    ) -> PhiTranslatedScalarGvnRows<K, E>,
    key_references_parameters: impl Fn(K, &BTreeSet<ValueId>) -> bool,
    translate: impl Fn(K, &BTreeMap<ValueId, ValueId>) -> Option<K>,
    emit: impl Fn(
        PhiTranslatedScalarGvnMatch<E>,
    ) -> Result<PsiRewriteCandidate, crate::RuleProposalError>,
) -> Result<Vec<PsiRewriteCandidate>, crate::RuleProposalError>
where
    K: Ord + Copy,
    E: Copy,
{
    let Some(crate::AnalysisProduct::ControlFlowGraph(_)) =
        analyses.get(AnalysisKind::ControlFlowGraph)
    else {
        return Err(crate::RuleProposalError::MissingAnalysis(
            AnalysisKind::ControlFlowGraph,
        ));
    };
    let Some(crate::AnalysisProduct::Dominators(dominators)) =
        analyses.get(AnalysisKind::Dominators)
    else {
        return Err(crate::RuleProposalError::MissingAnalysis(
            AnalysisKind::Dominators,
        ));
    };
    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
    let mut candidates = Vec::new();
    for function in &unit.functions {
        let machine_dominators = dominators
            .function(function.machine)
            .map(|(_, rows)| rows.as_slice())
            .unwrap_or_default();
        let value_types = function_value_types(function);
        let (leaders, redundants) = rows(unit, function, effects, &value_types);
        for (key, redundant, redundant_operation, redundant_result, scalar_type, extras) in
            redundants
        {
            let Some((incoming, affected_blocks, provenance, parameter_position)) =
                phi_translated_accounting::collect_phi_translated_incoming(
                    function,
                    redundant,
                    redundant_result,
                    scalar_type,
                    use_definitions,
                    &value_types,
                    machine_dominators,
                    &leaders,
                    |parameters| key_references_parameters(key, parameters),
                    |translation| translate(key, translation),
                )
            else {
                continue;
            };
            candidates.push(emit(PhiTranslatedScalarGvnMatch {
                affected_blocks,
                provenance,
                extras,
                patch: optimization_unit::PhiTranslatedScalarGvnRewrite {
                    redundant,
                    redundant_operation,
                    redundant_result,
                    scalar_type,
                    parameter_position,
                    incoming,
                },
            })?);
        }
    }
    Ok(candidates)
}

/// Compatible-policy leader/redundant collection shared by the dominator- and
/// phi-translated proof-certified rules: leaders must be obligation-free exact
/// pure scalars; redundants must carry an accepted obligation fact and the
/// same effect purity.
pub(super) fn collect_compatible_policy_scalar_rows(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    effects: &EffectSummaryAnalysis,
) -> (
    Vec<(
        expression_keys::CompatiblePolicyScalarExpressionKey,
        NodeLocation,
        OperationId,
        ValueId,
        ScalarType,
    )>,
    Vec<(
        expression_keys::CompatiblePolicyScalarExpressionKey,
        NodeLocation,
        OperationId,
        ValueId,
        ScalarType,
        AcceptedObligationFactIdentity,
    )>,
) {
    let mut leaders = Vec::new();
    let mut redundants = Vec::new();
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index).expect("optimization node index fits u32");
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: node_index,
            };
            if let Some((key, operation, result, scalar_type)) =
                expression_keys::compatible_policy_scalar_leader(&node.operation)
                && exact_pure_scalar_effect(unit, effects, function.machine, block.id, node_index)
                && !function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                        if *support == operation)
                })
            {
                leaders.push((key, location, operation, result, scalar_type));
            }
            if let Some((key, operation, result, scalar_type)) =
                expression_keys::compatible_policy_scalar_redundant(&node.operation)
                && let Ok(obligation_fact) =
                    accepted_obligation_fact(unit, function.machine, operation)
                && exact_pure_scalar_effect(unit, effects, function.machine, block.id, node_index)
            {
                redundants.push((
                    key,
                    location,
                    operation,
                    result,
                    scalar_type,
                    obligation_fact,
                ));
            }
        }
    }
    (leaders, redundants)
}
