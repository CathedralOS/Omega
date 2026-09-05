use super::*;

pub(crate) fn reconstruct_shared_terminal_fusion_accounting(
    function: &PsiOptimizationFunction,
    patch: SharedJumpFusionRewrite,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let predecessor = function
        .blocks
        .iter()
        .find(|block| block.id == patch.predecessor.block)?;
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)?;
    let target = function
        .blocks
        .iter()
        .find(|block| block.id == patch.target)?;
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
        optimization_unit::ProvenanceRewrite {
            input: input_edge,
            disposition: ProvenanceDisposition::RealizedAt(output_clone),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        },
        optimization_unit::ProvenanceRewrite {
            input: input_terminal,
            disposition: ProvenanceDisposition::RealizedAt(output_clone),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        },
        optimization_unit::ProvenanceRewrite {
            input: input_terminal,
            disposition: ProvenanceDisposition::RealizedAt(input_terminal),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        },
    ];
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    let mut blocks = vec![patch.predecessor.block, patch.target];
    blocks.sort();
    blocks.dedup();
    Some((blocks, provenance))
}
