//! Cross-block elimination from proof-bearing redundants to total-policy leaders.

use optimization_core::{AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{
    DominatingScalarCommonSubexpressionRewrite,
    PsiOptimizationUnit, PsiRewriteCandidate,
};

use crate::rules::global_value_numbering::collect_compatible_policy_scalar_rows;
use crate::analyses::block_dominates;
use crate::rules::global_value_numbering::dominating_contract;
use crate::rules::node_elision::node_elision_accounting;
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DominatorProofCertifiedCompatiblePolicyScalarGvnRule;

impl DominatorProofCertifiedCompatiblePolicyScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        dominating_contract(
            b"omega.psi-rule.dominator-proof-certified-compatible-policy-scalar-gvn.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for DominatorProofCertifiedCompatiblePolicyScalarGvnRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
        required_analysis!(analyses, AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators)));
        let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .function(function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            let (leaders, redundants) =
                collect_compatible_policy_scalar_rows(unit, function, effects);
            for (key, redundant, redundant_operation, redundant_result, scalar_type, fact) in
                redundants
            {
                if !use_definitions.uses.iter().any(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == redundant_result
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
                        *candidate_key == key
                            && *candidate_type == scalar_type
                            && location.block != redundant.block
                            && block_dominates(machine_dominators, location.block, redundant.block)
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
                if !use_definitions
                    .uses
                    .iter()
                    .filter(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == redundant_result
                    })
                    .all(|(_, use_site)| {
                        if leader.block == use_site.block {
                            leader.node < use_site.node
                        } else {
                            block_dominates(machine_dominators, leader.block, use_site.block)
                        }
                    })
                {
                    continue;
                }
                let Some((affected_blocks, provenance)) =
                    node_elision_accounting(function, redundant, redundant_result)
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        fact,
                        -1,
                        DominatingScalarCommonSubexpressionRewrite {
                            leader: *leader,
                            redundant,
                            leader_operation: *leader_operation,
                            redundant_operation,
                            leader_result: *leader_result,
                            redundant_result,
                            scalar_type,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}
