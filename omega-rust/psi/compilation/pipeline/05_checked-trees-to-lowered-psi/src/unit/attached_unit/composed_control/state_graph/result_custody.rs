//! Rejoin a forwarded view to the state-local home it keeps live. Disposal
//! authority and ordering belong to the shared selected-exit cleanup route,
//! not another reconstruction of calls and constructor moves here.
use super::super::super::CheckedUnitEffectOperationPlan;
use super::super::CheckedTrees;
use super::CheckedComposedUnitControlStatePlan;

/// A call or constructed-value result that no automatic return discard
/// retires: the selected edge owns its disposal.
pub(super) fn result(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<&checked_trees::CheckedUnitStructuralResultBindingPlan> {
    operation
        .discardable_structural_result()
        .and_then(|(result, discard_result_on_return)| {
            (!discard_result_on_return).then_some(result)
        })
}

/// The source-state result whose home the view `binding_ordinal` lends. A
/// `let` view (`&mut room.exits[..]`) is a loan of the local it was
/// formed from, so an edge carrying the view must keep that local's home live
/// with the loan, exactly as a projected loan keeps its backing local. `None`
/// for a non-view result or a view of storage the state does not own (a
/// parameter outlives every state of the activation).
pub(super) fn view_backing(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    binding_ordinal: u32,
) -> Option<u32> {
    let value = state
        .operations
        .iter()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                    result, value, ..
                },
            ) if result.binding_ordinal == binding_ordinal => Some(*value),
            _ => None,
        })?;
    let nodes = &checked.facts.values.structural_values.nodes;
    let lent = match &nodes.get(nodes.is_valid(value).then_some(value)?).kind {
        checked_trees::CheckedStructuralValueKind::BorrowedSliceView { source, .. }
        | checked_trees::CheckedStructuralValueKind::MutableSliceView { source, .. } => source,
        _ => return None,
    };
    match lent.source {
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } => Some(binding_ordinal),
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            let mut homes = state
                .operations
                .iter()
                .filter_map(result)
                .filter(|home| home.local.symbol == symbol);
            let home = homes.next()?;
            homes.next().is_none().then_some(home.binding_ordinal)
        }
        _ => None,
    }
}
