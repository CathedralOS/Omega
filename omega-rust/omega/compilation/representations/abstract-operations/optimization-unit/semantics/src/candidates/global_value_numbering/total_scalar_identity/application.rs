//! Independently apply an admitted total scalar identity.
use crate::candidates::accept_region_rewrite;
use crate::candidates::retire_node_custody;
use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
use crate::candidates::sweep_output_nodes;
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

            sweep_output_nodes(output_function, |node| {
                rewrite_scalar_value_uses(&mut node.operation, patch.result, patch.replacement);
                Ok(())
            })?;
            Ok(())
        },
    )
}
