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
//! without listing direct uses: folding never removes an identity, a ranked
//! machine is left unchanged, and proof-bearing closures stay frozen until
//! proof-context transport is implemented.
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
    let mut after = before.clone();
    // Call composition can include a callee's axioms in another machine's
    // obligation. Protect the complete closure, not just the obligation owner.
    if questions.obligations().is_empty() {
        for machine in &mut after.semantic_module.machines {
            folding::fold(machine);
        }
    }
    validate_sparse_conditional_constant_propagation(
        &before.semantic_module,
        &after.semantic_module,
    )
    .map_err(PsiOptimizationStageError::InvalidSparseConditionalConstantPropagationRewrite)?;
    if let Some(debug) = after.debug_map.as_mut() {
        debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
            .map_err(PsiOptimizationStageError::InvalidSemantic)?;
    }
    Ok(after)
}
