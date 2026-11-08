//! Path-qualified empty-block threading: the rule, its eligibility and
//! proposal traversal, and the provenance accounting a threaded path carries.

use std::collections::BTreeSet;

use abstract_operations::AbstractOperation as O;
use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{
    NodeLocation, PathQualifiedEmptyBlockRewrite, ProvenanceRewrite, PsiOptimizationFunction,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};
use optimization_unit_semantics::{
    reconstruct_linear_thread_bindings, reconstruct_linear_thread_ownership_is_identity,
};
use semantic_vocabulary::{BlockId, EdgeId};

use crate::rules::control_flow_cleanup::scalar_cleanup_functions;
use crate::rules::control_flow_cleanup::{
    provenance_realized_at, thread_accounting_tail, threadable_empty_blocks,
};
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    PathQualifiedEmptyBlockThreadRule,
    crate::rules::control_flow_cleanup::control_flow_cleanup_rule_contract(
        b"omega.psi-rule.path-qualified-empty-block-thread.v1",
        1,
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::UseDefinition,
            AnalysisKind::OwnershipFrontiers,
        ]),
    ),
);

psi_rule_impl!(PathQualifiedEmptyBlockThreadRule, |unit, analyses| {
    required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
    required_analysis!(analyses, AnalysisKind::UseDefinition => Some(AnalysisProduct::UseDefinition(use_definitions)));
    required_analysis!(analyses, AnalysisKind::OwnershipFrontiers => Some(_));
    let mut candidates = Vec::new();
    for function in scalar_cleanup_functions(&unit.functions) {
        for empty in threadable_empty_blocks(function, use_definitions) {
            let empty_id = empty.block.id;
            let incoming = function
                .blocks
                .iter()
                .flat_map(|block| {
                    block
                        .nodes
                        .iter()
                        .enumerate()
                        .flat_map(move |(node_index, node)| {
                            node.successors
                                .iter()
                                .filter(move |edge| edge.target == empty_id)
                                .map(move |edge| (block, node_index, node, edge))
                        })
                })
                .collect::<Vec<_>>();
            if incoming.is_empty()
                || (incoming.len() == 1 && matches!(incoming[0].2.operation, O::Jump { .. }))
                || incoming
                    .iter()
                    .any(|(_, _, _, edge)| !edge.cleanup_actions.is_empty())
            {
                continue;
            }
            if incoming.iter().any(|(_, _, _, edge)| {
                reconstruct_linear_thread_bindings(
                    &empty.block.parameters,
                    &edge.bindings,
                    empty.bindings,
                )
                .is_none()
                    || !reconstruct_linear_thread_ownership_is_identity(
                        unit,
                        function,
                        edge.psi_edge,
                        empty.block.id,
                        empty.edge,
                        empty.target,
                    )
            }) {
                continue;
            }
            let incoming_edges = incoming
                .iter()
                .map(|(_, _, _, edge)| edge.psi_edge)
                .collect::<Vec<_>>();
            let Some((affected_blocks, provenance)) =
                path_thread_accounting(function, empty.location, &incoming_edges)
            else {
                continue;
            };
            candidates.push(
                PsiRewriteCandidate::new_path_qualified_empty_block(
                    unit.identity,
                    Self::contract(),
                    affected_blocks,
                    provenance,
                    -3,
                    PathQualifiedEmptyBlockRewrite {
                        empty: empty.location,
                        outgoing_edge: empty.edge,
                        target: empty.target,
                    },
                )
                .map_err(RuleProposalError::InvalidCandidate)?,
            );
        }
    }
    Ok(candidates)
});

pub(super) fn path_thread_accounting(
    function: &PsiOptimizationFunction,
    empty: NodeLocation,
    incoming_edges: &[EdgeId],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let empty_node = function
        .block(empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let outgoing = empty_node.successors.first()?;
    let outgoing_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: outgoing.psi_edge,
    };
    let incoming_set = incoming_edges.iter().copied().collect::<BTreeSet<_>>();
    if incoming_set.len() != incoming_edges.len() || incoming_set.is_empty() {
        return None;
    }
    let mut affected = BTreeSet::from([empty.block]);
    let mut realized = Vec::new();
    for block in &function.blocks {
        for node in &block.nodes {
            for edge in &node.successors {
                if !incoming_set.contains(&edge.psi_edge) || edge.target != empty.block {
                    continue;
                }
                affected.insert(block.id);
                let site = PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: edge.psi_edge,
                };
                realized.push(provenance_realized_at(
                    site,
                    site,
                    &edge.provenance,
                    &edge.fuel,
                ));
                realized.push(provenance_realized_at(
                    outgoing_site,
                    site,
                    &outgoing.provenance,
                    &outgoing.fuel,
                ));
            }
        }
    }
    if realized.len() != incoming_edges.len().checked_mul(2)? {
        return None;
    }
    thread_accounting_tail(function, empty.block, None, affected, realized)
}
