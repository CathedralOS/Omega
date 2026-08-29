//! Obligation-free same-block common-subexpression elimination.

use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct SameBlockTotalScalarCseRule;


impl SameBlockTotalScalarCseRule {
    pub fn contract() -> OptimizationRuleContract {
        same_block_contract(
            b"omega.psi-rule.same-block-obligation-free-total-scalar-cse.v1",
            OptimizationSafetyClass::ExactOperationSemantics,
        )
    }
}

impl PsiOptimizationRule for SameBlockTotalScalarCseRule {
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
            for block in &function.blocks {
                let mut leaders = BTreeMap::new();
                for (index, node) in block.nodes.iter().enumerate() {
                    let Some((key, operation, result, scalar_type)) =
                        total_scalar_expression(&node.operation, &value_types)
                    else {
                        continue;
                    };
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let pure = effects.nodes.iter().any(|row| {
                        row.revision == unit.identity
                            && row.machine == function.machine
                            && row.block == block.id
                            && row.node == node_index
                            && row.class == crate::EffectClass::PureScalar
                            && row.observable == crate::EffectKnowledge::No
                            && row.structural_state == crate::EffectKnowledge::No
                            && row.crash == crate::EffectKnowledge::No
                            && row.suspension == crate::EffectKnowledge::No
                    });
                    if !pure {
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
                    let patch = LocalScalarCommonSubexpressionRewrite {
                        leader: leader_location,
                        redundant: redundant_location,
                        leader_operation,
                        redundant_operation: operation,
                        leader_result,
                        redundant_result: result,
                        scalar_type,
                    };
                    candidates.push(
                        PsiRewriteCandidate::new_local_scalar_common_subexpression(
                            unit.identity,
                            Self::contract(),
                            affected_blocks,
                            provenance,
                            -1,
                            patch,
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}
