use crate::candidates::{canonicalize_provenance, provenance_realized_at};
use optimization_unit::{
    NodeLocation, PsiOptimizationFunction, PsiRealizationSite, SharedJumpFusionRewrite,
};
use semantic_vocabulary::BlockId;

pub(crate) fn reconstruct_shared_terminal_fusion_accounting(
    function: &PsiOptimizationFunction,
    patch: SharedJumpFusionRewrite,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let predecessor = function.block(patch.predecessor.block)?;
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node.edge(patch.incoming_edge)?;
    let target = function.block(patch.target)?;
    let [terminal] = target.nodes.as_slice() else {
        return None;
    };
    let input_edge = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let input_terminal = PsiRealizationSite::Node(NodeLocation {
        machine: function.machine,
        block: patch.target,
        node: 0,
    });
    let output_clone = PsiRealizationSite::Node(patch.predecessor);
    let mut provenance = vec![
        provenance_realized_at(
            input_edge,
            output_clone,
            &incoming.provenance,
            &incoming.fuel,
        ),
        provenance_realized_at(
            input_terminal,
            output_clone,
            &terminal.provenance,
            &terminal.fuel,
        ),
        provenance_realized_at(
            input_terminal,
            input_terminal,
            &terminal.provenance,
            &terminal.fuel,
        ),
    ];
    canonicalize_provenance(&mut provenance);
    let mut blocks = vec![patch.predecessor.block, patch.target];
    blocks.sort();
    blocks.dedup();
    Some((blocks, provenance))
}
