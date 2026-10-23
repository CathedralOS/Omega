use crate::candidates::{canonicalize_provenance, provenance_realized_at};
use optimization_unit::{
    NodeLocation, NonAdjacentBlockMergeRewrite, PsiOptimizationFunction, PsiRealizationSite,
    ScalarSubstitution,
};
use semantic_vocabulary::BlockId;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn reconstruct_non_adjacent_merge_accounting(
    function: &PsiOptimizationFunction,
    patch: NonAdjacentBlockMergeRewrite,
    substitutions: &[ScalarSubstitution],
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let predecessor_position = function.block_index(patch.predecessor.block)?;
    let target_position = function.block_index(patch.target)?;
    if target_position == predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor = &function.blocks[predecessor_position];
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node.edge(patch.incoming_edge)?;
    let target = &function.blocks[target_position];
    let first = target.nodes.first()?;
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let mut realized = if !first.provenance.is_empty() {
        vec![provenance_realized_at(
            incoming_site,
            PsiRealizationSite::Node(patch.predecessor),
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

    for (node_index, node) in target.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target.id,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.id,
            node: patch
                .predecessor
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

    let mut input_effect = 0u64;
    let mut input_starts = BTreeMap::new();
    for block in &function.blocks {
        input_starts.insert(block.id, input_effect);
        input_effect = input_effect.checked_add(u64::try_from(block.nodes.len()).ok()?)?;
    }
    let mut output_effect = 0u64;
    let mut effect_shifted = BTreeSet::new();
    for block in &function.blocks {
        if block.id == patch.target {
            continue;
        }
        if input_starts.get(&block.id).copied()? != output_effect {
            effect_shifted.insert(block.id);
        }
        let output_nodes = if block.id == patch.predecessor.block {
            block
                .nodes
                .len()
                .checked_sub(1)?
                .checked_add(target.nodes.len())?
        } else {
            block.nodes.len()
        };
        output_effect = output_effect.checked_add(u64::try_from(output_nodes).ok()?)?;
    }

    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([patch.predecessor.block, patch.target]);
    affected.extend(effect_shifted.iter().copied());
    for block in &function.blocks {
        if block.id == patch.target {
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
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node.provenance.is_empty()
                || (!effect_shifted.contains(&block.id) && !changed_uses.contains(&node_index))
            {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(provenance_realized_at(
                site,
                site,
                &node.provenance,
                &node.fuel,
            ));
        }
    }
    canonicalize_provenance(&mut realized);
    Some((affected.into_iter().collect(), realized))
}
