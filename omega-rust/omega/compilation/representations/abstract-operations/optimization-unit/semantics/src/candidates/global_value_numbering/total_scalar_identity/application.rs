//! Independently apply an admitted total scalar identity.
use crate::candidates::accept_region_rewrite;
use crate::candidates::retire_node_custody;
use crate::candidates::rewrite_accounting::preserve_edge_custody;
use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
use crate::candidates::set_node_effect_link;
use crate::unit_validation::derived_metadata::{
    expected_definitions, expected_ownership, expected_uses,
};
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_unit::{
    ProvenanceRewrite, PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityRewrite,
};
use semantic_vocabulary::BlockId;

pub(super) fn independently_apply_total_scalar_identity(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    patch: TotalScalarIdentityRewrite,
    node_index: usize,
    affected_blocks: &[BlockId],
    provenance: Vec<ProvenanceRewrite>,
    validator: &[u8],
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    accept_region_rewrite(
        input,
        candidate,
        patch.location.machine,
        affected_blocks,
        validator,
        provenance,
        |output| {
            // The rewrite moves no accepted obligation fact, so any catalog
            // drift is the candidate's claim alone.
            if output.accepted_obligation_facts != input.accepted_obligation_facts {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            let output_function = output
                .function_mut(patch.location.machine)
                .expect("validated candidate function exists");
            let output_block = output_function
                .block_mut(patch.location.block)
                .expect("validated candidate block exists");
            retire_node_custody(output_block, node_index)?;

            let mut effect = 0_u64;
            for block in &mut output_function.blocks {
                for (index, node) in block.nodes.iter_mut().enumerate() {
                    rewrite_scalar_value_uses(&mut node.operation, patch.result, patch.replacement);
                    let index = u32::try_from(index)
                        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
                    node.definitions = expected_definitions(&node.operation, block.id, index);
                    node.uses = expected_uses(&node.operation, block.id, index);
                    node.successors = preserve_edge_custody(node);
                    node.ownership = expected_ownership(&node.operation);
                    effect = set_node_effect_link(node, effect)?;
                }
            }
            Ok(())
        },
    )
}
