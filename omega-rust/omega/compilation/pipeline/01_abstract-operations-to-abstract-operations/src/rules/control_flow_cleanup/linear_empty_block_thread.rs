//! Linear empty-block threading: the rule, its eligibility and proposal
//! traversal, and the provenance accounting a threaded edge carries.

use std::collections::BTreeSet;

use abstract_operations::AbstractOperation as O;
use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{
    LinearEmptyBlockRewrite, NodeLocation, ProvenanceRewrite, PsiOptimizationFunction,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};
use semantic_vocabulary::BlockId;

use crate::rules::control_flow_cleanup::descriptor_telescope_free;
use crate::rules::control_flow_cleanup::empty_block_thread_bindings::{
    compose_linear_thread_bindings, linear_thread_ownership_is_identity,
};
use crate::rules::control_flow_cleanup::{provenance_realized_at, record_block_effect_drift};
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

impl PsiOptimizationRule for LinearEmptyBlockThreadRule {
    psi_rule_contract!(@adapter);

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
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
                            .filter_map(move |(node, candidate)| {
                                candidate
                                    .successors
                                    .iter()
                                    .any(|edge| edge.target == empty.id)
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
                    || !outgoing_cleanup_actions.is_empty()
                    || *predecessor_target != empty.id
                    || empty.parameters.iter().any(|parameter| {
                        use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine
                                && use_site.value == parameter.value
                                && (use_site.block != empty.id || use_site.node != 0)
                        })
                    })
                    || !linear_thread_ownership_is_identity(
                        unit,
                        function,
                        frontiers,
                        *incoming_edge,
                        empty.id,
                        *outgoing_edge,
                        *target,
                    )
                {
                    continue;
                }
                let Some(_) = compose_linear_thread_bindings(
                    &empty.parameters,
                    incoming_bindings,
                    outgoing_bindings,
                ) else {
                    continue;
                };
                let predecessor = NodeLocation {
                    machine: function.machine,
                    block: predecessor_block.id,
                    node: u32::try_from(*predecessor_node_index)
                        .expect("optimization node indices are u32"),
                };
                let empty_location = NodeLocation {
                    machine: function.machine,
                    block: empty.id,
                    node: 0,
                };
                let Some((affected_blocks, provenance)) =
                    linear_thread_accounting(function, predecessor, empty_location)
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
    }
}

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
    let predecessor_site = output_site;
    let empty_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: empty_edge.psi_edge,
    };

    let mut affected = BTreeSet::from([predecessor.block, empty.block]);
    let mut realized = vec![
        provenance_realized_at(
            predecessor_site,
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
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
            continue;
        }
        record_block_effect_drift(
            function,
            block,
            &mut expected_effect,
            Some(predecessor),
            &mut affected,
            &mut realized,
        )?;
    }
    realized.sort_by_key(optimization_unit::ProvenanceRewrite::canonical_sort_key);
    Some((affected.into_iter().collect(), realized))
}
