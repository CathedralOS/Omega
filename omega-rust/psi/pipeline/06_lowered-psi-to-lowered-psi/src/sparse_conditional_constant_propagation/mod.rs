//! Optimizer module role: stage group. Sparse conditional constant propagation.
//!
//! A goal-free scalar leaf whose operands all resolve to literals — literal
//! operation results or results of leaves already folded under this rule — is
//! a constant computation: it rewrites in place to the literal operation
//! computing its denotation. The folded operation keeps its identity, result
//! declaration, and position, so every value identity, block, edge, debug
//! subject, and qualification roster survives unchanged. Nothing is removed;
//! literal producers a fold leaves unreferenced remain for the dead-scalar
//! rule when it is also selected.
//!
//! Propositions, ranking rows, and custody sidecars name value identities
//! without listing direct uses: folding never removes an identity, and a
//! ranked machine is left unchanged. Inside a proof-bearing module a fold
//! survives only while the reconstructed proof question is unchanged; a fold
//! the question cannot carry verbatim refuses, leaving the complete closure
//! as authored until proof-context transport lets the question move with the
//! rewrite.
//!
//! The scan proposes folds; the independent verifier re-derives the literal
//! for each rewritten operation and checks the exact before/after relation
//! and proof questions.

mod folding;

use crate::PsiOptimizationStageError;
use lowered_psi::LoweredPsi;
use terminal_verifier::{
    reconstruct_optimizable_terminal_obligations, validate_module_for_optimization,
    validate_sparse_conditional_constant_propagation,
};

pub(super) fn propagate(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
    let validated = validate_module_for_optimization(&before.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidModule)?;
    let questions = reconstruct_optimizable_terminal_obligations(validated)
        .map_err(PsiOptimizationStageError::InvalidModule)?;
    // Call composition can include a callee's axioms in another machine's
    // obligation: the verifier's reconstructed-question check is the refusal
    // boundary for the complete closure, not just the obligation owner.
    let proof_bearing = !questions.obligations().is_empty();
    let mut after = before.clone();
    for machine in &mut after.semantic_module.machines {
        folding::fold(machine);
    }
    if let Err(error) = validate_sparse_conditional_constant_propagation(
        &before.semantic_module,
        &after.semantic_module,
    ) {
        // Without proof-context transport a proof-bearing input keeps only a
        // rewrite the reconstructed question survives unchanged; a refused
        // rewrite leaves the module as authored.
        if proof_bearing
            && matches!(
                error,
                terminal_verifier::SparseConditionalConstantPropagationRewriteError::InvalidModule(
                    _
                ) | terminal_verifier::SparseConditionalConstantPropagationRewriteError::ChangedProofQuestion
            )
        {
            return Ok(before);
        }
        return Err(
            PsiOptimizationStageError::InvalidSparseConditionalConstantPropagationRewrite(error),
        );
    }
    if let Some(debug) = after.debug_map.as_mut() {
        debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
            .map_err(PsiOptimizationStageError::InvalidSemantic)?;
    }
    Ok(after)
}
