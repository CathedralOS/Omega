//! Path-qualified empty-block threading: the rule, its eligibility and
//! proposal traversal, and the provenance accounting a threaded path carries.

use std::collections::BTreeSet;

use abstract_operations::AbstractOperation as O;
use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{
    NodeLocation, PathQualifiedEmptyBlockRewrite, ProvenanceRewrite, PsiOptimizationFunction,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};
use semantic_vocabulary::{BlockId, EdgeId};

use crate::rules::control_flow_cleanup::descriptor_telescope_free;
use crate::rules::control_flow_cleanup::empty_block_thread_bindings::{
    compose_linear_thread_bindings, linear_thread_ownership_is_identity,
};
use crate::rules::control_flow_cleanup::{provenance_realized_at, record_block_effect_drift};
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
    required_analysis!(analyses, AnalysisKind::OwnershipFrontiers => Some(AnalysisProduct::OwnershipFrontiers(frontiers)));
    let mut candidates = Vec::new();
    for function in &unit.functions {
        // Descriptor telescopes require structural substitution, outside this scalar rewrite.
        if !descriptor_telescope_free(function) {
            continue;
        }
        for empty in &function.blocks {
            if empty.id == function.entry || empty.nodes.len() != 1 {
                continue;
            }
            let O::Jump {
                structural_bindings: _,
                psi_edge: outgoing_edge,
                target,
                bindings: outgoing_bindings,
                cleanup_actions: outgoing_cleanup_actions,
            } = &empty.nodes[0].operation
            else {
                continue;
            };
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
                                .filter(move |edge| edge.target == empty.id)
                                .map(move |edge| (block, node_index, node, edge))
                        })
                })
                .collect::<Vec<_>>();
            if !outgoing_cleanup_actions.is_empty()
                || incoming.is_empty()
                || (incoming.len() == 1 && matches!(incoming[0].2.operation, O::Jump { .. }))
                || incoming
                    .iter()
                    .any(|(_, _, _, edge)| !edge.cleanup_actions.is_empty())
                || empty.parameters.iter().any(|parameter| {
                    use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine
                            && use_site.value == parameter.value
                            && (use_site.block != empty.id || use_site.node != 0)
                    })
                })
            {
                continue;
            }
            if incoming.iter().any(|(_, _, _, edge)| {
                compose_linear_thread_bindings(&empty.parameters, &edge.bindings, outgoing_bindings)
                    .is_none()
                    || !linear_thread_ownership_is_identity(
                        unit,
                        function,
                        frontiers,
                        edge.psi_edge,
                        empty.id,
                        *outgoing_edge,
                        *target,
                    )
            }) {
                continue;
            }
            let empty_location = NodeLocation {
                machine: function.machine,
                block: empty.id,
                node: 0,
            };
            let incoming_edges = incoming
                .iter()
                .map(|(_, _, _, edge)| edge.psi_edge)
                .collect::<Vec<_>>();
            let Some((affected_blocks, provenance)) =
                path_thread_accounting(function, empty_location, &incoming_edges)
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
                        empty: empty_location,
                        outgoing_edge: *outgoing_edge,
                        target: *target,
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
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
            continue;
        }
        record_block_effect_drift(
            function,
            block,
            &mut expected_effect,
            None,
            &mut affected,
            &mut realized,
        )?;
    }
    realized.sort_by_key(optimization_unit::ProvenanceRewrite::canonical_sort_key);
    Some((affected.into_iter().collect(), realized))
}
