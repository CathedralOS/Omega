//! Linear empty-block threading: the rule, its eligibility and proposal
//! traversal, and the provenance accounting a threaded edge carries.

use std::collections::BTreeSet;

use abstract_operations::AbstractOperation as O;
use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{
    LinearEmptyBlockRewrite, NodeLocation, ProvenanceRewrite, PsiOptimizationFunction,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};
use optimization_unit_semantics::{
    reconstruct_linear_thread_bindings, reconstruct_linear_thread_ownership_is_identity,
};
use semantic_vocabulary::BlockId;

use crate::rules::control_flow_cleanup::scalar_cleanup_functions;
use crate::rules::control_flow_cleanup::{
    provenance_realized_at, thread_accounting_tail, threadable_empty_blocks,
};
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    LinearEmptyBlockThreadRule,
    crate::rules::control_flow_cleanup::control_flow_cleanup_rule_contract(
        b"omega.psi-rule.linear-empty-block-thread.v2",
        2,
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::UseDefinition,
            AnalysisKind::OwnershipFrontiers,
        ]),
    ),
);

psi_rule_impl!(LinearEmptyBlockThreadRule, |unit, analyses| {
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
                        .filter_map(move |(node, candidate)| {
                            candidate
                                .successors
                                .iter()
                                .any(|edge| edge.target == empty_id)
                                .then_some((block, node, candidate))
                        })
                })
                .collect::<Vec<_>>();
            let [(predecessor_block, predecessor_node_index, predecessor_node)] =
                incoming.as_slice()
            else {
                continue;
            };
            let O::Jump {
                structural_bindings: _,
                psi_edge: incoming_edge,
                target: predecessor_target,
                bindings: incoming_bindings,
                cleanup_actions: incoming_cleanup_actions,
            } = &predecessor_node.operation
            else {
                continue;
            };
            if !incoming_cleanup_actions.is_empty()
                || *predecessor_target != empty.block.id
                || !reconstruct_linear_thread_ownership_is_identity(
                    unit,
                    function,
                    *incoming_edge,
                    empty.block.id,
                    empty.edge,
                    empty.target,
                )
                || reconstruct_linear_thread_bindings(
                    &empty.block.parameters,
                    incoming_bindings,
                    empty.bindings,
                )
                .is_none()
            {
                continue;
            }
            let predecessor = NodeLocation {
                machine: function.machine,
                block: predecessor_block.id,
                node: u32::try_from(*predecessor_node_index)
                    .expect("optimization node indices are u32"),
            };
            let Some((affected_blocks, provenance)) =
                linear_thread_accounting(function, predecessor, empty.location)
            else {
                continue;
            };
            candidates.push(
                PsiRewriteCandidate::new_linear_empty_block(
                    unit.identity,
                    Self::contract(),
                    affected_blocks,
                    provenance,
                    -3,
                    LinearEmptyBlockRewrite {
                        predecessor,
                        incoming_edge: *incoming_edge,
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

pub(super) fn linear_thread_accounting(
    function: &PsiOptimizationFunction,
    predecessor: NodeLocation,
    empty: NodeLocation,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_node = function
        .block(predecessor.block)?
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let empty_node = function
        .block(empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let predecessor_edge = predecessor_node.successors.first()?;
    let empty_edge = empty_node.successors.first()?;
    let output_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: predecessor_edge.psi_edge,
    };
    let empty_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: empty_edge.psi_edge,
    };
    let affected = BTreeSet::from([predecessor.block, empty.block]);
    let realized = vec![
        provenance_realized_at(
            output_site,
            output_site,
            &predecessor_edge.provenance,
            &predecessor_edge.fuel,
        ),
        provenance_realized_at(
            empty_site,
            output_site,
            &empty_edge.provenance,
            &empty_edge.fuel,
        ),
    ];
    thread_accounting_tail(function, empty.block, Some(predecessor), affected, realized)
}
