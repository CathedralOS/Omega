use crate::candidates::{canonicalize_provenance, provenance_realized_at, realize_node_provenance};
use optimization_unit::{
    DeadScalarNodeRewrite, NodeLocation, PsiOptimizationFunction, PsiRealizationSite,
};
use semantic_vocabulary::BlockId;

pub(crate) fn reconstruct_dead_scalar_node_accounting(
    function: &PsiOptimizationFunction,
    patch: DeadScalarNodeRewrite,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let block_position = function.block_index(patch.location.block)?;
    let node_position = usize::try_from(patch.location.node).ok()?;
    let block = &function.blocks[block_position];
    let removed = block.nodes.get(node_position)?;
    block.nodes.get(node_position.checked_add(1)?)?;
    let mut provenance = vec![provenance_realized_at(
        PsiRealizationSite::Node(patch.location),
        PsiRealizationSite::Node(patch.location),
        &removed.provenance,
        &removed.fuel,
    )];
    for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
        if node.provenance.is_empty() {
            continue;
        }
        let old = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        };
        let new = NodeLocation {
            node: old.node.checked_sub(1)?,
            ..old
        };
        provenance.push(provenance_realized_at(
            PsiRealizationSite::Node(old),
            PsiRealizationSite::Node(new),
            &node.provenance,
            &node.fuel,
        ));
    }
    let mut blocks = vec![block.id];
    for later in function.blocks.iter().skip(block_position + 1) {
        blocks.push(later.id);
        realize_node_provenance(
            function.machine,
            later.id,
            later.nodes.iter().enumerate(),
            &mut provenance,
        )?;
    }
    blocks.sort();
    canonicalize_provenance(&mut provenance);
    Some((blocks, provenance))
}

/// The dead-node accounting plus every block using `result`, whose realized
/// provenance rows the removal shifts in place: each use block is affected,
/// and every provenance-bearing node in it realizes its own site. Shared by
/// every scalar-identity and common-subexpression family whose patch removes
/// one scalar node and substitutes its live result.
pub(crate) fn reconstruct_removed_result_use_accounting(
    function: &PsiOptimizationFunction,
    location: NodeLocation,
    source_operation: semantic_vocabulary::OperationId,
    result: semantic_vocabulary::ValueId,
    scalar_type: semantic_vocabulary::ScalarType,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let dead = DeadScalarNodeRewrite {
        location,
        source_operation,
        result,
        scalar_type,
    };
    let (mut blocks, mut provenance) = reconstruct_dead_scalar_node_accounting(function, dead)?;
    for use_block in &function.blocks {
        if blocks.contains(&use_block.id) || !use_block.uses().any(|row| row.value == result) {
            continue;
        }
        blocks.push(use_block.id);
        realize_node_provenance(
            function.machine,
            use_block.id,
            use_block.nodes.iter().enumerate(),
            &mut provenance,
        )?;
    }
    blocks.sort();
    blocks.dedup();
    canonicalize_provenance(&mut provenance);
    Some((blocks, provenance))
}
