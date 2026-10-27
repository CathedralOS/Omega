//! Folding of constant conditional branches.

use std::collections::BTreeSet;

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisKind, AnalysisSet, OptimizationRuleContract, OptimizationSafetyClass,
};
use optimization_unit::{
    ConstantConditionalRewrite, NodeLocation, ProvenanceRewrite, PsiOptimizationFunction,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
    reachable_blocks_after_conditional_fold,
};
use semantic_vocabulary::{BlockId, EdgeId};

use crate::rules::control_flow_cleanup::{
    provenance_proven_unreachable_at, provenance_realized_at, record_block_effect_drift,
};
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    ConstantConditionalFoldRule,
    crate::rules::control_flow_cleanup::call_graph_control_flow_cleanup_rule_contract(
        b"omega.psi-rule.constant-conditional-fold.v5",
        5,
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::ScalarConstants,
        ]),
        OptimizationSafetyClass::ExactOperationSemantics,
    ),
);

impl PsiOptimizationRule for ConstantConditionalFoldRule {
    psi_rule_contract!(@adapter);

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
        required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let O::Conditional {
                        condition,
                        when_true,
                        when_false,
                    } = &node.operation
                    else {
                        continue;
                    };
                    let Some((constant, condition_fact)) =
                        constants.boolean_constant(function.machine, *condition)
                    else {
                        continue;
                    };
                    let (selected, rejected) = if constant {
                        (when_true, when_false)
                    } else {
                        (when_false, when_true)
                    };
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("optimization node indices are u32"),
                    };
                    let Some(reachable) = reachable_blocks_after_conditional_fold(
                        function,
                        block.id,
                        selected.psi_edge,
                    ) else {
                        continue;
                    };
                    if function.blocks.iter().any(|block| {
                        !reachable.contains(&block.id) && !block.structural_parameters.is_empty()
                    }) {
                        continue;
                    }
                    let Some((affected_blocks, provenance)) = conditional_fold_accounting(
                        function,
                        location,
                        selected.psi_edge,
                        rejected.psi_edge,
                        &reachable,
                    ) else {
                        continue;
                    };
                    candidates.push(
                        PsiRewriteCandidate::new_constant_conditional(
                            unit.identity,
                            Self::contract(),
                            affected_blocks,
                            provenance,
                            condition_fact,
                            -1,
                            ConstantConditionalRewrite {
                                location,
                                condition: *condition,
                                constant,
                                selected_edge: selected.psi_edge,
                                rejected_edge: rejected.psi_edge,
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

fn conditional_fold_accounting(
    function: &PsiOptimizationFunction,
    decision: NodeLocation,
    selected_edge: EdgeId,
    rejected_edge: EdgeId,
    reachable: &BTreeSet<BlockId>,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let decision_node = function
        .block(decision.block)?
        .nodes
        .get(usize::try_from(decision.node).ok()?)?;
    let selected = decision_node.edge(selected_edge)?;
    let rejected = decision_node.edge(rejected_edge)?;
    let selected_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: selected_edge,
    };
    let rejected_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: rejected_edge,
    };
    let removed = function
        .blocks
        .iter()
        .map(|block| block.id)
        .filter(|block| !reachable.contains(block))
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([decision.block]);
    affected.extend(removed.iter().copied());
    let mut realized = vec![provenance_realized_at(
        selected_site,
        selected_site,
        &selected.provenance,
        &selected.fuel,
    )];
    let mut unreachable = vec![provenance_proven_unreachable_at(
        rejected_site,
        rejected_site,
        &rejected.provenance,
        &rejected.fuel,
    )];
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if removed.contains(&block.id) {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                };
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    unreachable.push(provenance_proven_unreachable_at(
                        site,
                        site,
                        &node.provenance,
                        &node.fuel,
                    ));
                }
                for edge in &node.successors {
                    let site = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    unreachable.push(provenance_proven_unreachable_at(
                        site,
                        site,
                        &edge.provenance,
                        &edge.fuel,
                    ));
                }
            }
            continue;
        }
        record_block_effect_drift(
            function,
            block,
            &mut expected_effect,
            Some(decision),
            &mut affected,
            &mut realized,
        )?;
    }
    realized.extend(unreachable);
    realized.sort_by_key(optimization_unit::ProvenanceRewrite::canonical_sort_key);
    Some((affected.into_iter().collect(), realized))
}
