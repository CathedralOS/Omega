use super::*;

pub(crate) fn reconstruct_proof_certified_scalar_identity_accounting(
    function: &PsiOptimizationFunction,
    patch: ProofCertifiedScalarIdentityRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let dead = DeadScalarNodeRewrite {
        location: patch.location,
        source_operation: patch.source_operation,
        result: patch.result,
        scalar_type: ScalarType::Integer(patch.scalar_type),
    };
    let (mut blocks, mut provenance) = reconstruct_dead_scalar_node_accounting(function, dead)?;
    for use_block in &function.blocks {
        if blocks.contains(&use_block.id)
            || !use_block
                .nodes
                .iter()
                .flat_map(|node| &node.uses)
                .any(|row| row.value == patch.result)
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
            provenance.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
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
