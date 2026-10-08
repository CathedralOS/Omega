//! Ordered edge cleanup retires each named owner in turn, not the whole frontier.
use crate::OptimizationUnitValidationError;
use crate::current_ownership::CurrentOwnership;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::StructuralTypeDeclaration;

/// Apply the selected edge's stream without grouping whole owners ahead of
/// partial owners or hooks. A residual group is one exact owner's complement;
/// outgoing ownership bindings are installed only after this transaction.
pub(super) fn apply_edge_cleanup_actions(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<semantic_vocabulary::MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &mut CurrentOwnership,
    actions: &[terminal_psi::TerminalAffineCleanupAction],
) -> Result<(), OptimizationUnitValidationError> {
    let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
    let roots = super::cleanup::cleanup_root_order(function, frontier);
    let mut eligible = roots.into_iter();
    for group in actions.chunk_by(|left, right| left.place() == right.place()) {
        let place = group[0].place();
        if !eligible.any(|candidate| candidate == place) {
            return Err(mismatch());
        }
        super::cleanup::consume_owner_cleanup(
            function,
            functions,
            structural_types,
            block,
            frontier,
            place,
            group,
        )?;
    }
    Ok(())
}
