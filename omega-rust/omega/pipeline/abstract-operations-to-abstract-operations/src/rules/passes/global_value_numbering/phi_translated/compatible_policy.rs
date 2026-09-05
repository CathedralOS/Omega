//! Proof-certified translation from exact redundants to total-policy leaders.

use std::collections::{BTreeMap, BTreeSet};

use optimization_core::{AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{
    NodeLocation, OptimizationFact, PhiTranslatedScalarGvnRewrite, PhiTranslatedScalarIncoming,
    PsiOptimizationUnit, PsiRewriteCandidate,
};

use crate::rules::passes::support::{accepted_obligation_fact, block_dominates};
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::super::{
    effect_admission::exact_pure_scalar_effect,
    expression_keys::{compatible_policy_scalar_leader, compatible_policy_scalar_redundant},
};
use super::{accounting::phi_translated_cse_accounting, phi_translated_contract};

#[derive(Debug, Clone, Copy, Default)]
pub struct PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule;

impl PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        phi_translated_contract(
            b"omega.psi-rule.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };

        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            let value_types = function
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
                .collect::<BTreeMap<_, _>>();
            let mut leaders = Vec::new();
            let mut redundants = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    if let Some((key, operation, result, scalar_type)) =
                        compatible_policy_scalar_leader(&node.operation)
                        && exact_pure_scalar_effect(
                            unit,
                            effects,
                            function.machine,
                            block.id,
                            node_index,
                        )
                        && !function.facts.iter().any(|fact| {
                            matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                                if *support == operation)
                        })
                    {
                        leaders.push((key, location, operation, result, scalar_type));
                    }
                    if let Some((key, operation, result, scalar_type)) =
                        compatible_policy_scalar_redundant(&node.operation)
                        && let Ok(obligation_fact) =
                            accepted_obligation_fact(unit, function.machine, operation)
                        && exact_pure_scalar_effect(
                            unit,
                            effects,
                            function.machine,
                            block.id,
                            node_index,
                        )
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

            for (
                key,
                redundant,
                redundant_operation,
                redundant_result,
                scalar_type,
                obligation_fact,
            ) in redundants
            {
                let Some(join) = function
                    .blocks
                    .iter()
                    .find(|block| block.id == redundant.block)
                else {
                    continue;
                };
                let parameter_values = join
                    .parameters
                    .iter()
                    .map(|parameter| parameter.value)
                    .collect::<BTreeSet<_>>();
                if join.id == function.entry
                    || parameter_values.is_empty()
                    || !key.references_any(&parameter_values)
                    || !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == redundant_result
                    })
                {
                    continue;
                }
                let redundant_index = usize::try_from(redundant.node).expect("u32 fits usize");
                let Some(redundant_node) = join.nodes.get(redundant_index) else {
                    continue;
                };
                let Some(receiver) = join.nodes.get(redundant_index + 1) else {
                    continue;
                };
                if receiver
                    .provenance
                    .iter()
                    .any(|source| redundant_node.provenance.contains(source))
                {
                    continue;
                }

                let mut incoming = Vec::new();
                let mut complete = true;
                for source in &function.blocks {
                    for (owner_index, owner) in source.nodes.iter().enumerate() {
                        let owner_index =
                            u32::try_from(owner_index).expect("optimization node index fits u32");
                        for edge in owner
                            .successors
                            .iter()
                            .filter(|edge| edge.target == join.id)
                        {
                            if edge.bindings.len() != join.parameters.len() {
                                complete = false;
                                continue;
                            }
                            let mut translation = BTreeMap::new();
                            for (parameter, binding) in join.parameters.iter().zip(&edge.bindings) {
                                if binding.parameter != parameter.value
                                    || binding.scalar_type != parameter.scalar_type
                                    || value_types.get(&binding.argument)
                                        != Some(&binding.scalar_type)
                                {
                                    complete = false;
                                    break;
                                }
                                translation.insert(parameter.value, binding.argument);
                            }
                            if !complete {
                                continue;
                            }
                            let translated_key = key.translate(&translation);
                            let leader = leaders
                                .iter()
                                .filter(|(candidate_key, location, _, _, candidate_type)| {
                                    candidate_key == &translated_key
                                        && candidate_type == &scalar_type
                                        && ((location.block == source.id
                                            && location.node < owner_index)
                                            || (location.block != source.id
                                                && block_dominates(
                                                    machine_dominators,
                                                    location.block,
                                                    source.id,
                                                )))
                                })
                                .min_by_key(|(_, location, _, _, _)| {
                                    let depth = machine_dominators
                                        .iter()
                                        .find(|(candidate, _)| *candidate == location.block)
                                        .map_or(usize::MAX, |(_, rows)| rows.len());
                                    (depth, *location)
                                });
                            let Some((_, leader, leader_operation, leader_result, _)) = leader
                            else {
                                complete = false;
                                continue;
                            };
                            incoming.push(PhiTranslatedScalarIncoming {
                                source: source.id,
                                edge: edge.psi_edge,
                                leader: *leader,
                                leader_operation: *leader_operation,
                                leader_result: *leader_result,
                            });
                        }
                    }
                }
                if !complete || incoming.len() < 2 {
                    continue;
                }
                incoming.sort_by_key(|row| (row.edge, row.source));
                let Some((affected_blocks, provenance)) =
                    phi_translated_cse_accounting(function, redundant, &incoming)
                else {
                    continue;
                };
                let parameter_position = u32::try_from(join.parameters.len())
                    .expect("optimization block parameter count fits u32");
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        obligation_fact,
                        -1,
                        PhiTranslatedScalarGvnRewrite {
                            redundant,
                            redundant_operation,
                            redundant_result,
                            scalar_type,
                            parameter_position,
                            incoming,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}
