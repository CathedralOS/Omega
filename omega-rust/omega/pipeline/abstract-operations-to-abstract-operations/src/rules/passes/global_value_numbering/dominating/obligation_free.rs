//! Obligation-free cross-block elimination from dominating leaders.

use std::collections::BTreeMap;

use optimization_core::{AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{
    DominatingScalarCommonSubexpressionRewrite, NodeLocation, PsiOptimizationUnit,
    PsiRewriteCandidate,
};

use crate::rules::passes::support::{block_dominates, node_elision_accounting};
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::super::{
    effect_admission::exact_pure_scalar_effect, expression_keys::total_scalar_expression,
};
use super::dominating_contract;

#[derive(Debug, Clone, Copy, Default)]
pub struct DominatorTotalScalarGvnRule;

impl DominatorTotalScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        dominating_contract(
            b"omega.psi-rule.dominator-obligation-free-total-scalar-gvn.v1",
            OptimizationSafetyClass::ExactOperationSemantics,
        )
    }
}

impl PsiOptimizationRule for DominatorTotalScalarGvnRule {
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
            let mut expressions = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let Some((key, operation, result, scalar_type)) =
                        total_scalar_expression(&node.operation, &value_types)
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
                    ));
                }
            }
            for (key, redundant, redundant_operation, redundant_result, scalar_type) in &expressions
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
                    .filter(|(candidate_key, location, _, _, candidate_type)| {
                        candidate_key == key
                            && *candidate_type == *scalar_type
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
                let patch = DominatingScalarCommonSubexpressionRewrite {
                    leader: *leader,
                    redundant: *redundant,
                    leader_operation: *leader_operation,
                    redundant_operation: *redundant_operation,
                    leader_result: *leader_result,
                    redundant_result: *redundant_result,
                    scalar_type: *scalar_type,
                };
                candidates.push(
                    PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
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
        Ok(candidates)
    }
}
