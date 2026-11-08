//! The non-adjacent unique-predecessor block merge: the rule, its
//! eligibility and proposal traversal, and the effect-shift provenance
//! accounting the merge carries.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::AbstractOperation as O;
use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{
    NodeLocation, NonAdjacentBlockMergeRewrite, OptimizationBlock, ProvenanceRewrite,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
    ScalarSubstitution,
};
use semantic_vocabulary::BlockId;

use crate::analyses::block_dominates;
use crate::rules::control_flow_cleanup::block_merge_substitutions::merge_substitutions;
use crate::rules::control_flow_cleanup::merge_boundary_ownership::merge_boundary_ownership_is_identity;
use crate::rules::control_flow_cleanup::scalar_cleanup_functions;
use crate::rules::control_flow_cleanup::{
    merge_candidates, provenance_realized_at, record_merged_node_positions, record_node_provenance,
};
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    NonAdjacentBlockMergeRule,
    crate::rules::control_flow_cleanup::control_flow_cleanup_rule_contract(
        b"omega.psi-rule.non-adjacent-unique-predecessor-block-merge.v1",
        1,
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
            AnalysisKind::OwnershipFrontiers,
        ]),
    ),
);

psi_rule_impl!(NonAdjacentBlockMergeRule, |unit, analyses| {
    required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
    required_analysis!(analyses, AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators)));
    required_analysis!(analyses, AnalysisKind::UseDefinition => Some(AnalysisProduct::UseDefinition(use_definitions)));
    required_analysis!(analyses, AnalysisKind::OwnershipFrontiers => Some(AnalysisProduct::OwnershipFrontiers(frontiers)));

    let mut candidates = Vec::new();
    for function in scalar_cleanup_functions(&unit.functions) {
        let machine_dominators = dominators.rows(function.machine);
        for candidate in merge_candidates(function, |candidate| {
            candidate.target_position != candidate.predecessor_position.saturating_add(1)
                && non_adjacent_merge_target_is_nonempty(candidate.target)
                && block_dominates(
                    machine_dominators,
                    candidate.jump.block.id,
                    candidate.target.id,
                )
        }) {
            let target = candidate.target;
            if !merge_boundary_ownership_is_identity(
                unit,
                function,
                frontiers,
                candidate.jump.edge,
                target.id,
            ) {
                continue;
            }
            let Some(substitutions) = merge_substitutions(
                function.machine,
                target,
                candidate.jump.bindings,
                machine_dominators,
                use_definitions,
            ) else {
                continue;
            };
            let Some((affected_blocks, provenance)) = non_adjacent_merge_accounting(
                function,
                candidate.jump.location,
                target.id,
                &substitutions,
            ) else {
                continue;
            };
            candidates.push(
                PsiRewriteCandidate::new_non_adjacent_block_merge(
                    unit.identity,
                    Self::contract(),
                    affected_blocks,
                    substitutions,
                    provenance,
                    -2,
                    NonAdjacentBlockMergeRewrite {
                        predecessor: candidate.jump.location,
                        incoming_edge: candidate.jump.edge,
                        target: target.id,
                    },
                )
                .map_err(RuleProposalError::InvalidCandidate)?,
            );
        }
    }
    Ok(candidates)
});

fn non_adjacent_merge_target_is_nonempty(target: &OptimizationBlock) -> bool {
    !target.nodes.is_empty()
        && !matches!(target.nodes.as_slice(), [node] if matches!(node.operation, O::Jump { .. }))
}

pub(super) fn non_adjacent_merge_accounting(
    function: &PsiOptimizationFunction,
    predecessor: NodeLocation,
    target: BlockId,
    substitutions: &[ScalarSubstitution],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_position = function.block_index(predecessor.block)?;
    let target_position = function.block_index(target)?;
    if target_position == predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor_block = &function.blocks[predecessor_position];
    let predecessor_node = predecessor_block
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let incoming = predecessor_node.successors.first()?;
    let target_block = &function.blocks[target_position];
    let first = target_block.nodes.first()?;
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: incoming.psi_edge,
    };
    let mut realized = if first.successors.is_empty() {
        vec![provenance_realized_at(
            incoming_site,
            PsiRealizationSite::Node(predecessor),
            &incoming.provenance,
            &incoming.fuel,
        )]
    } else {
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
    };

    record_merged_node_positions(function, predecessor, target, target_block, &mut realized)?;

    let mut input_effect = 0u64;
    let mut input_starts = BTreeMap::new();
    for block in &function.blocks {
        input_starts.insert(block.id, input_effect);
        input_effect = input_effect.checked_add(u64::try_from(block.nodes.len()).ok()?)?;
    }
    let mut output_effect = 0u64;
    let mut effect_shifted = BTreeSet::new();
    for block in &function.blocks {
        if block.id == target {
            continue;
        }
        if input_starts.get(&block.id).copied()? != output_effect {
            effect_shifted.insert(block.id);
        }
        let output_nodes = if block.id == predecessor.block {
            block
                .nodes
                .len()
                .checked_sub(1)?
                .checked_add(target_block.nodes.len())?
        } else {
            block.nodes.len()
        };
        output_effect = output_effect.checked_add(u64::try_from(output_nodes).ok()?)?;
    }

    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([predecessor.block, target]);
    affected.extend(effect_shifted.iter().copied());
    for block in &function.blocks {
        if block.id == target {
            continue;
        }
        let mut changed_uses = BTreeSet::new();
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node
                .uses
                .iter()
                .any(|row| substituted_values.contains(&row.value))
            {
                changed_uses.insert(node_index);
                affected.insert(block.id);
            }
        }
        let rows = block.nodes.iter().enumerate().filter(|(node_index, node)| {
            !node.provenance.is_empty()
                && (effect_shifted.contains(&block.id) || changed_uses.contains(node_index))
        });
        record_node_provenance(function.machine, block, rows, &mut realized)?;
    }
    realized.sort_by_key(optimization_unit::ProvenanceRewrite::canonical_sort_key);
    Some((affected.into_iter().collect(), realized))
}
