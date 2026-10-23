//! Removed-result accounting for the common-subexpression families.
//!
//! A same-block or dominating CSE removes the redundant node and substitutes
//! its live result, so its accounting is the shared removed-result use-site
//! spine. A phi-translated join additionally moves every incoming edge's
//! leader onto the join parameter, so it builds on the same dead-node spine
//! and names those edges itself.
use crate::candidates::canonicalize_provenance;
use crate::candidates::provenance_realized_at;
use crate::candidates::rewrite_accounting::dead_scalar::{
    reconstruct_dead_scalar_node_accounting, reconstruct_removed_result_use_accounting,
};
use optimization_unit::{
    DeadScalarNodeRewrite, LocalScalarCommonSubexpressionRewrite, PhiTranslatedScalarGvnRewrite,
    PsiOptimizationFunction, PsiRealizationSite,
};
use semantic_vocabulary::BlockId;

pub(crate) fn reconstruct_local_cse_accounting(
    function: &PsiOptimizationFunction,
    patch: LocalScalarCommonSubexpressionRewrite,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    reconstruct_removed_result_use_accounting(
        function,
        patch.redundant,
        patch.redundant_operation,
        patch.redundant_result,
        patch.scalar_type,
    )
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
            .block(incoming.source)?
            .successors()
            .find(|edge| edge.psi_edge == incoming.edge && edge.target == patch.redundant.block)?;
        blocks.push(incoming.source);
        let site = PsiRealizationSite::Edge {
            machine: function.machine,
            edge: incoming.edge,
        };
        provenance.push(provenance_realized_at(
            site,
            site,
            &edge.provenance,
            &edge.fuel,
        ));
    }
    blocks.sort();
    blocks.dedup();
    canonicalize_provenance(&mut provenance);
    Some((blocks, provenance))
}
