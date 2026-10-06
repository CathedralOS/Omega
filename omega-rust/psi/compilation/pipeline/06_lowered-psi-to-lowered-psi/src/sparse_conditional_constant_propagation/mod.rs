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
mod validation;

pub(super) use validation::SparseConditionalConstantPropagationRewriteError;

use crate::PsiOptimizationStageError;
use lowered_psi::LoweredPsi;
use validation::validate_sparse_conditional_constant_propagation;

pub(super) fn propagate(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
    let mut after = before.clone();
    for machine in &mut after.semantic_module.machines {
        folding::fold(machine);
    }
    if let Err(error) = validate_sparse_conditional_constant_propagation(
        &before.semantic_module,
        &after.semantic_module,
    ) {
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
