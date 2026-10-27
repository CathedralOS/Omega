//! The adjacent unique-predecessor block merge: the rule, its eligibility
//! and proposal traversal, and the roster-compaction provenance accounting
//! the merge carries.

use std::collections::BTreeSet;

use abstract_operations::AbstractOperation as O;
use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{
    AdjacentBlockMergeRewrite, NodeLocation, ProvenanceRewrite, PsiOptimizationFunction,
    PsiOptimizationUnit, PsiProvenance, PsiRealizationSite, PsiRewriteCandidate,
    ScalarSubstitution,
};
use semantic_vocabulary::BlockId;

use crate::rules::control_flow_cleanup::block_merge_substitutions::merge_substitutions;
use crate::rules::control_flow_cleanup::descriptor_telescope_free;
use crate::rules::control_flow_cleanup::merge_boundary_ownership::merge_boundary_ownership_witness;
use crate::rules::control_flow_cleanup::{provenance_realized_at, record_node_provenance};
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    AdjacentBlockMergeRule,
    crate::rules::control_flow_cleanup::control_flow_cleanup_rule_contract(
        b"omega.psi-rule.adjacent-single-predecessor-block-merge.v5",
        5,
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
            AnalysisKind::OwnershipFrontiers,
        ]),
    ),
);

impl PsiOptimizationRule for AdjacentBlockMergeRule {
    psi_rule_contract!(@adapter);

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
        required_analysis!(analyses, AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators)));
        required_analysis!(analyses, AnalysisKind::UseDefinition => Some(AnalysisProduct::UseDefinition(use_definitions)));
        required_analysis!(analyses, AnalysisKind::OwnershipFrontiers => Some(AnalysisProduct::OwnershipFrontiers(frontiers)));
        let mut candidates = Vec::new();
        for function in &unit.functions {
            // Descriptor telescopes require structural substitution, outside this scalar rewrite.
            if !descriptor_telescope_free(function) {
                continue;
            }
            let machine_dominators = dominators
                .function(function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            for adjacent in function.blocks.windows(2) {
                let [predecessor, target] = adjacent else {
                    unreachable!("two-block window")
                };
                let eligible_first = target.nodes.first().is_some_and(|node| {
                    (node.successors.is_empty()
                        && (matches!(node.provenance.first(), Some(PsiProvenance::Operation(_)))
                            || (matches!(node.provenance.first(), Some(PsiProvenance::Edge(_)))
                                && matches!(
                                    node.operation,
                                    abstract_operations::return_or_return_unit_pattern!()
                                        | abstract_operations::return_structural_or_crash_pattern!()
                                ))))
                        || (matches!(node.operation, O::Conditional { .. })
                            && node.successors.len() == 2
                            && node.provenance.is_empty())
                });
                if target.id == function.entry || !eligible_first {
                    continue;
                }
                let Some((predecessor_index, predecessor_node)) =
                    predecessor.nodes.iter().enumerate().last()
                else {
                    continue;
                };
                let O::Jump {
                    structural_bindings: _,
                    psi_edge: incoming_edge,
                    target: jump_target,
                    bindings,
                    cleanup_actions,
                } = &predecessor_node.operation
                else {
                    continue;
                };
                if !cleanup_actions.is_empty()
                    || *jump_target != target.id
                    || function
                        .block_edges()
                        .filter(|edge| edge.target == target.id)
                        .count()
                        != 1
                {
                    continue;
                }
                let Some(ownership_witness) = merge_boundary_ownership_witness(
                    unit,
                    function,
                    frontiers,
                    *incoming_edge,
                    target.id,
                ) else {
                    continue;
                };
                let Some(substitutions) = merge_substitutions(
                    function.machine,
                    target,
                    bindings,
                    machine_dominators,
                    use_definitions,
                ) else {
                    continue;
                };
                let predecessor_location = NodeLocation {
                    machine: function.machine,
                    block: predecessor.id,
                    node: u32::try_from(predecessor_index)
                        .expect("optimization node index fits u32"),
                };
                let Some((affected_blocks, provenance)) = adjacent_merge_accounting(
                    function,
                    predecessor_location,
                    target.id,
                    &substitutions,
                ) else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_adjacent_block_merge(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        substitutions,
                        provenance,
                        ownership_witness,
                        -2,
                        AdjacentBlockMergeRewrite {
                            predecessor: predecessor_location,
                            incoming_edge: *incoming_edge,
                            target: target.id,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

pub(super) fn adjacent_merge_accounting(
    function: &PsiOptimizationFunction,
    predecessor: NodeLocation,
    target: BlockId,
    substitutions: &[ScalarSubstitution],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_position = function.block_index(predecessor.block)?;
    let target_position = function.block_index(target)?;
    if target_position != predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor_node = function.blocks[predecessor_position]
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let incoming = predecessor_node.successors.first()?;
    let target_block = &function.blocks[target_position];
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: incoming.psi_edge,
    };
    let mut affected = BTreeSet::from([predecessor.block, target]);
    let first = target_block.nodes.first()?;
    let mut realized = if !first.provenance.is_empty() {
        vec![provenance_realized_at(
            incoming_site,
            PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: predecessor.block,
                node: predecessor.node,
            }),
            &incoming.provenance,
            &incoming.fuel,
        )]
    } else if !first.successors.is_empty() {
        first
            .successors
            .iter()
            .map(|successor| {
                provenance_realized_at(
                    incoming_site,
                    PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: successor.psi_edge,
                    },
                    &incoming.provenance,
                    &incoming.fuel,
                )
            })
            .collect()
    } else {
        return None;
    };
    for (node_index, node) in target_block.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.block,
            node: predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(provenance_realized_at(
            input,
            output,
            &node.provenance,
            &node.fuel,
        ));
    }
    for block in function.blocks.iter().skip(target_position + 1) {
        affected.insert(block.id);
        record_node_provenance(
            function.machine,
            block,
            block.nodes.iter().enumerate(),
            &mut realized,
        )?;
    }
    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        if affected.contains(&block.id) {
            continue;
        }
        let changed_nodes = block
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.uses
                    .iter()
                    .any(|row| substituted_values.contains(&row.value))
            })
            .collect::<Vec<_>>();
        if changed_nodes.is_empty() {
            continue;
        }
        affected.insert(block.id);
        record_node_provenance(
            function.machine,
            block,
            changed_nodes.into_iter(),
            &mut realized,
        )?;
    }
    realized.sort_by_key(optimization_unit::ProvenanceRewrite::canonical_sort_key);
    Some((affected.into_iter().collect(), realized))
}
