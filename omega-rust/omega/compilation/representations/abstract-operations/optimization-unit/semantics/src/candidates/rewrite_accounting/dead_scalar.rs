use crate::candidates::provenance_realized_at;
use optimization_unit::{
    DeadScalarNodeRewrite, NodeLocation, ProvenanceDisposition, PsiOptimizationFunction,
    PsiRealizationSite,
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
    let mut provenance = vec![provenance_realized_at(PsiRealizationSite::Node(patch.location), PsiRealizationSite::Node(patch.location), &removed.provenance, &removed.fuel)];
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
        provenance.push(provenance_realized_at(PsiRealizationSite::Node(old), PsiRealizationSite::Node(new), &node.provenance, &node.fuel));
    }
    let mut blocks = vec![block.id];
    for later in function.blocks.iter().skip(block_position + 1) {
        blocks.push(later.id);
        for (index, node) in later.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: later.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(provenance_realized_at(site, site, &node.provenance, &node.fuel));
        }
    }
    blocks.sort();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((blocks, provenance))
}
