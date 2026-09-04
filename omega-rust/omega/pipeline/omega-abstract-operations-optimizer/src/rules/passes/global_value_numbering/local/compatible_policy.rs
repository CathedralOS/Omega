//! Same-block elimination from proof-bearing redundants to total-policy leaders.

use std::collections::BTreeMap;

use omega_optimization_core::{AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass};
use omega_optimization_unit::{
    LocalScalarCommonSubexpressionRewrite, NodeLocation, OptimizationFact, PsiOptimizationUnit,
    PsiRewriteCandidate,
};

use crate::rules::passes::support::{accepted_obligation_fact, node_elision_accounting};
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::super::{
    effect_admission::exact_pure_scalar_effect,
    expression_keys::{compatible_policy_scalar_leader, compatible_policy_scalar_redundant},
};
use super::same_block_contract;

#[derive(Debug, Clone, Copy, Default)]
pub struct SameBlockProofCertifiedCompatiblePolicyScalarCseRule;

impl SameBlockProofCertifiedCompatiblePolicyScalarCseRule {
    pub fn contract() -> OptimizationRuleContract {
        same_block_contract(
            b"omega.psi-rule.same-block-proof-certified-compatible-policy-scalar-cse.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for SameBlockProofCertifiedCompatiblePolicyScalarCseRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
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
            for block in &function.blocks {
                let mut leaders = BTreeMap::new();
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
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
                        leaders.entry(key).or_insert((
                            node_index,
                            operation,
                            result,
                            scalar_type,
                        ));
                    }
                    let Some((key, operation, result, scalar_type)) =
                        compatible_policy_scalar_redundant(&node.operation)
                    else {
                        continue;
                    };
                    let Some(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, operation).ok()
                    else {
                        continue;
                    };
                    if !exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) || !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == result
                    }) {
                        continue;
                    }
                    let Some((leader, leader_operation, leader_result, leader_type)) =
                        leaders.get(&key).copied()
                    else {
                        continue;
                    };
                    if leader_type != scalar_type {
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
                    let redundant = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let Some((affected_blocks, provenance)) =
                        node_elision_accounting(function, redundant, result)
                    else {
                        continue;
                    };
                    candidates.push(
                        PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
                            unit.identity,
                            Self::contract(),
                            affected_blocks,
                            provenance,
                            obligation_fact,
                            -1,
                            LocalScalarCommonSubexpressionRewrite {
                                leader: NodeLocation {
                                    machine: function.machine,
                                    block: block.id,
                                    node: leader,
                                },
                                redundant,
                                leader_operation,
                                redundant_operation: operation,
                                leader_result,
                                redundant_result: result,
                                scalar_type,
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}
