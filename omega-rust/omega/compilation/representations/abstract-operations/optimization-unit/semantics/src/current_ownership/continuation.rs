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
    let mut remaining = actions;
    while let Some(action) = remaining.first() {
        let place = action.place();
        if !eligible.any(|candidate| candidate == place) {
            return Err(mismatch());
        }
        let count = remaining
            .iter()
            .take_while(|action| action.place() == place)
            .count();
        let (group, rest) = remaining.split_at(count);
        super::cleanup::consume_owner_cleanup(
            function,
            functions,
            structural_types,
            block,
            frontier,
            place,
            group,
        )?;
        remaining = rest;
    }
    Ok(())
}
