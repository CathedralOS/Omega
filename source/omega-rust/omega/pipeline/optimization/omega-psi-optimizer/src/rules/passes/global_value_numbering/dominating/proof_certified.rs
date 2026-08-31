//! Proof-certified cross-block elimination from dominating leaders.

use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct DominatorProofCertifiedScalarGvnRule;

impl DominatorProofCertifiedScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        dominating_contract(
            b"omega.psi-rule.dominator-proof-certified-total-scalar-gvn.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for DominatorProofCertifiedScalarGvnRule {
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
            let mut expressions = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
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
                    if !exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) {
                        continue;
                    }
                    expressions.push((
                        key,
                        NodeLocation {
                            machine: function.machine,
                            block: block.id,
                            node: node_index,
                        },
                        operation,
                        result,
                        scalar_type,
                        obligation_fact,
                    ));
                }
            }
            for (
                key,
                redundant,
                redundant_operation,
                redundant_result,
                scalar_type,
                obligation_fact,
            ) in &expressions
            {
                if !use_definitions.uses.iter().any(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == *redundant_result
                }) {
                    continue;
                }
                let Some(redundant_block) = function
                    .blocks
                    .iter()
                    .find(|block| block.id == redundant.block)
                else {
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
                let leader = expressions
                    .iter()
                    .filter(|(candidate_key, location, _, _, candidate_type, _)| {
                        candidate_key == key
                            && *candidate_type == *scalar_type
                            && location.block != redundant.block
                            && block_dominates(machine_dominators, location.block, redundant.block)
                    })
                    .min_by_key(|(_, location, _, _, _, _)| {
                        let depth = machine_dominators
                            .iter()
                            .find(|(block, _)| *block == location.block)
                            .map_or(usize::MAX, |(_, rows)| rows.len());
                        (depth, *location)
                    });
                let Some((_, leader, leader_operation, leader_result, _, _)) = leader else {
                    continue;
                };
                let replacement_definition = omega_optimization_unit::ValueDefinition {
                    value: *leader_result,
                    scalar_type: *scalar_type,
                    site: omega_optimization_unit::ValueDefinitionSite::Node {
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
                        omega_optimization_unit::ValueDefinitionSite::Node { block, node }
                            if block == use_site.block =>
                        {
                            node < use_site.node
                        }
                        omega_optimization_unit::ValueDefinitionSite::Node { block, .. } => {
                            block_dominates(machine_dominators, block, use_site.block)
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
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        *obligation_fact,
                        -1,
                        DominatingScalarCommonSubexpressionRewrite {
                            leader: *leader,
                            redundant: *redundant,
                            leader_operation: *leader_operation,
                            redundant_operation: *redundant_operation,
                            leader_result: *leader_result,
                            redundant_result: *redundant_result,
                            scalar_type: *scalar_type,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}
