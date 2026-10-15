//! Rejoin a forwarded loan carrier to the state-local home it keeps live. Disposal
//! authority and ordering belong to the shared selected-exit cleanup route,
//! not another reconstruction of calls and constructor moves here.
use super::super::super::CheckedUnitEffectOperationPlan;
use super::super::CheckedTrees;
use super::CheckedComposedUnitControlStatePlan;

/// The source-state result whose home the carrier `binding_ordinal` lends. A
/// reference or view (`&mut room.exits[..]`) is a loan of the local it was
/// formed from, so an edge carrying that carrier must keep that local's home live
/// with the loan, exactly as a projected loan keeps its backing local. `None`
/// for a non-loan result or a loan of storage the state does not own (a
/// parameter outlives every state of the activation).
pub(super) fn local_loan_backing(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    binding_ordinal: u32,
) -> Option<u32> {
    let (position, operation) = state.operations.iter().enumerate().find(|(_, operation)| {
        operation
            .structural_result_binding()
            .is_some_and(|result| result.binding_ordinal == binding_ordinal)
    })?;
    let lent = operation
        .established_reference_source(&checked.facts.values.structural_values)
        .or_else(|| {
            let value = match operation {
                CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
                ) => *value,
                _ => return None,
            };
            let nodes = &checked.facts.values.structural_values.nodes;
            match &nodes.get(nodes.is_valid(value).then_some(value)?).kind {
                checked_trees::CheckedStructuralValueKind::BorrowedSliceView { source, .. }
                | checked_trees::CheckedStructuralValueKind::MutableSliceView { source, .. } => {
                    Some(source)
                }
                _ => None,
            }
        })?;
    lent.source
        .current_result_binding(&state.operations[..position])
}
