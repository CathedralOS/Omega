//! Optimizer module role: stage group. Scalar-identity accounting families.
//!
//! Both obligation-free and proof-certified identities remove one scalar node
//! and substitute its live result. Their semantic admission remains separate;
//! only exact custody reconstruction is shared here: each family's
//! reconstruction names its own patch, then runs the shared dead-node
//! accounting plus the use-site provenance rows below.

use crate::candidates::rewrite_accounting::dead_scalar::reconstruct_dead_scalar_node_accounting;
use optimization_unit::{
    DeadScalarNodeRewrite, NodeLocation, ProofCertifiedScalarIdentityRewrite,
    ProvenanceDisposition, PsiOptimizationFunction, PsiRealizationSite, TotalScalarIdentityRewrite,
};
use semantic_vocabulary::{BlockId, OperationId, ScalarType, ValueId};

pub(crate) fn reconstruct_proof_certified_scalar_identity_accounting(
    function: &PsiOptimizationFunction,
    patch: ProofCertifiedScalarIdentityRewrite,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    reconstruct_scalar_identity_accounting(
        function,
        patch.location,
        patch.source_operation,
        patch.result,
        ScalarType::Integer(patch.scalar_type),
    )
}

pub(crate) fn reconstruct_total_scalar_identity_accounting(
    function: &PsiOptimizationFunction,
    patch: TotalScalarIdentityRewrite,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    reconstruct_scalar_identity_accounting(
        function,
        patch.location,
        patch.source_operation,
        patch.result,
        ScalarType::Integer(patch.scalar_type),
    )
}

fn reconstruct_scalar_identity_accounting(
    function: &PsiOptimizationFunction,
    location: NodeLocation,
    source_operation: OperationId,
    result: ValueId,
    scalar_type: ScalarType,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let dead = DeadScalarNodeRewrite {
        location,
        source_operation,
        result,
        scalar_type,
    };
    let (mut blocks, mut provenance) = reconstruct_dead_scalar_node_accounting(function, dead)?;
    for use_block in &function.blocks {
        if blocks.contains(&use_block.id)
            || !use_block
                .nodes
                .iter()
                .flat_map(|node| &node.uses)
                .any(|row| row.value == result)
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
