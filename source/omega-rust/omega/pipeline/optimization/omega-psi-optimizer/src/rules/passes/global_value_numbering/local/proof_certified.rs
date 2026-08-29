//! Proof-certified same-block common-subexpression elimination.

use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct SameBlockProofCertifiedScalarCseRule;

impl SameBlockProofCertifiedScalarCseRule {
    pub fn contract() -> OptimizationRuleContract {
        same_block_contract(
            b"omega.psi-rule.same-block-proof-certified-total-scalar-cse.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for SameBlockProofCertifiedScalarCseRule {
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
                    let Some((key, operation, result, scalar_type)) =
                        proof_certified_scalar_expression(&node.operation)
                    else {
                        continue;
                    };
                    let Some(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, operation).ok()
                    else {
                        continue;
                    };
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    if !exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) {
                        continue;
                    }
                    let Some((leader, leader_operation, leader_result, leader_type)) =
                        leaders.get(&key).copied()
                    else {
                        leaders.insert(key, (node_index, operation, result, scalar_type));
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
                    let leader_location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: leader,
                    };
                    let redundant_location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let Some((affected_blocks, provenance)) =
                        local_cse_accounting(function, redundant_location, result)
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
                                leader: leader_location,
                                redundant: redundant_location,
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
