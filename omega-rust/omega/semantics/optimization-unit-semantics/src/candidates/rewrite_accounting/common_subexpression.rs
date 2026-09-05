use super::*;

pub(crate) fn reconstruct_local_cse_accounting(
    function: &PsiOptimizationFunction,
    patch: LocalScalarCommonSubexpressionRewrite,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let dead = DeadScalarNodeRewrite {
        location: patch.redundant,
        source_operation: patch.redundant_operation,
        result: patch.redundant_result,
        scalar_type: patch.scalar_type,
    };
    let (mut blocks, mut provenance) = reconstruct_dead_scalar_node_accounting(function, dead)?;
    for use_block in &function.blocks {
        if blocks.contains(&use_block.id)
            || !use_block
                .nodes
                .iter()
                .flat_map(|node| &node.uses)
                .any(|row| row.value == patch.redundant_result)
        {
            continue;
        }
        blocks.push(use_block.id);
        for (index, node) in use_block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: use_block.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
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

pub(crate) fn reconstruct_phi_translated_cse_accounting(
    function: &PsiOptimizationFunction,
    patch: &PhiTranslatedScalarGvnRewrite,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let dead = DeadScalarNodeRewrite {
        location: patch.redundant,
        source_operation: patch.redundant_operation,
        result: patch.redundant_result,
        scalar_type: patch.scalar_type,
    };
    let (mut blocks, mut provenance) = reconstruct_dead_scalar_node_accounting(function, dead)?;
    for incoming in &patch.incoming {
        let edge = function
            .blocks
            .iter()
            .find(|block| block.id == incoming.source)?
            .nodes
            .iter()
            .flat_map(|node| &node.successors)
            .find(|edge| edge.psi_edge == incoming.edge && edge.target == patch.redundant.block)?;
        blocks.push(incoming.source);
        let site = PsiRealizationSite::Edge {
            machine: function.machine,
            edge: incoming.edge,
        };
        provenance.push(optimization_unit::ProvenanceRewrite {
            input: site,
            disposition: ProvenanceDisposition::RealizedAt(site),
            sources: edge.provenance.clone(),
            fuel: edge.fuel.clone(),
        });
    }
    blocks.sort();
    blocks.dedup();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((blocks, provenance))
}
