//! Rejoin a forwarded loan carrier to the state-local home it keeps live. Disposal
//! authority and ordering belong to the shared selected-exit cleanup route,
//! not another reconstruction of calls and constructor moves here.
use super::super::super::CheckedUnitEffectOperationPlan;
use super::super::CheckedTrees;
use super::CheckedComposedUnitControlStatePlan;

#[derive(Clone, Copy)]
pub(super) struct LoanBacking {
    pub(super) binding: u32,
    pub(super) completed: bool,
    /// The whole supplying descriptor has a checked selected-exit death.
    /// Its backing is independently owned; this never moves through a loan.
    pub(super) retiring_carrier: Option<u32>,
}

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
    let lent = established_loan_source(checked, operation)?;
    inline_local_backing(state.operations[..position].iter(), lent)
}

/// Resolve a transported carrier against its actual formation frame. A
/// descriptor crossing does not imply external storage: both a named reference
/// and `&local` in an edge operand can denote the state's owned home.
/// The completion flag distinguishes an operand home from a state-local home; their
/// establishment scopes determine which cleanup owner can retain them.
pub(super) fn transferred_loan_backing(
    checked: &CheckedTrees,
    operations: &[CheckedUnitEffectOperationPlan],
    transfer: &checked_trees::CheckedStructuralControlTransferPlan,
) -> Option<LoanBacking> {
    use checked_trees::CheckedStructuralControlTransferSourcePlan as Transfer;
    if let Some(binding) = transfer.retained_loan_backing {
        if let Transfer::ProjectedLoan { subject, .. } = &transfer.source
            && subject.source.current_result_binding(operations) == Some(binding)
            && !subject
                .path
                .contains(&checked_trees::CheckedUnitStructuralPathSegment::Referent)
        {
            // The ordinary owner-loan path already names this home directly.
            return None;
        }
        let retiring_carrier = match &transfer.source {
            Transfer::ProjectedLoan {
                subject,
                retired_carrier_backing: Some(backing),
                ..
            } if *backing == binding => subject.source.current_result_binding(operations),
            _ => None,
        };
        return Some(LoanBacking {
            binding,
            completed: false,
            retiring_carrier,
        });
    }
    match &transfer.source {
        Transfer::EstablishedValue {
            operands,
            establishment,
            presentation,
        } => {
            if !matches!(
                presentation.path.first(),
                Some(checked_trees::CheckedUnitStructuralPathSegment::Referent)
            ) {
                return None;
            }
            let lent = established_loan_source(checked, establishment)?;
            let binding = inline_local_backing(operations.iter().chain(operands.iter()), lent)?;
            let completed = operands.iter().any(|operation| {
                operation
                    .structural_result_binding()
                    .is_some_and(|result| result.binding_ordinal == binding)
            });
            Some(LoanBacking {
                binding,
                completed,
                retiring_carrier: None,
            })
        }
        Transfer::StructuralResult { binding_ordinal } => {
            let carrier = *binding_ordinal;
            let (position, operation) = operations.iter().enumerate().find(|(_, operation)| {
                operation
                    .structural_result_binding()
                    .is_some_and(|result| result.binding_ordinal == carrier)
            })?;
            let lent = established_loan_source(checked, operation)?;
            let binding = inline_local_backing(operations[..position].iter(), lent)?;
            Some(LoanBacking {
                binding,
                completed: false,
                retiring_carrier: None,
            })
        }
        _ => None,
    }
}

/// An inline projection retains its containing home. Crossing a descriptor
/// instead needs the captured permission relation retained by checking; an
/// initializer is not provenance for a later stored or returned reference.
fn inline_local_backing<'plans>(
    operations: impl DoubleEndedIterator<Item = &'plans CheckedUnitEffectOperationPlan>,
    lent: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Option<u32> {
    if lent
        .path
        .contains(&checked_trees::CheckedUnitStructuralPathSegment::Referent)
    {
        return None;
    }
    lent.source.current_result_binding(operations)
}

fn established_loan_source<'plans>(
    checked: &'plans CheckedTrees,
    operation: &'plans CheckedUnitEffectOperationPlan,
) -> Option<&'plans checked_trees::CheckedUnitStructuralArgumentPlan> {
    operation
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
        })
}

/// The storage an edge borrows, independent of whether its reference formation
/// is a direct projection or an already completed operand. A completed
/// descriptor lends its source only through its explicit referent presentation.
pub(super) fn loan_subject<'plans>(
    source: &'plans checked_trees::CheckedStructuralControlTransferSourcePlan,
    values: &'plans checked_trees::CheckedStructuralValuePlans,
) -> Option<(
    &'plans checked_trees::CheckedUnitStructuralArgumentPlan,
    bool,
)> {
    use checked_trees::{
        CheckedStructuralControlTransferSourcePlan as Source,
        CheckedUnitStructuralPathSegment as Path,
    };
    match source {
        Source::ProjectedLoan { subject, .. } => Some((subject, false)),
        Source::EstablishedValue {
            establishment,
            presentation,
            ..
        } if matches!(presentation.path.first(), Some(Path::Referent)) => establishment
            .established_reference_source(values)
            .map(|subject| (subject, presentation.path.len() != 1)),
        _ => None,
    }
}
