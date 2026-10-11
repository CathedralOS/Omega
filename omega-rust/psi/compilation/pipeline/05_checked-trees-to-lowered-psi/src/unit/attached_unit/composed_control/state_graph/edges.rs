//! Authored successor operands and ownership cleanup at edges and returns.
//!
//! Cleanup owner rosters use the shared ownership-facts query: a root can die
//! on several mutually exclusive exits without being disposed twice. Edge
//! bindings and return transfers select the actual residual cleanup below.
use super::super::super::{CheckedUnitEffectOperationPlan, unsupported};
use super::super::LoweringError;
use super::{
    CheckedComposedUnitControlStatePlan, CheckedStructuralControlSuccessorPlan, subslices,
};

pub(in crate::unit::attached_unit::composed_control) fn successors(
    state: &CheckedComposedUnitControlStatePlan,
) -> Vec<&CheckedStructuralControlSuccessorPlan> {
    state.successors()
}

/// The transfer shapes the edge emitter lowers, read from the plan alone:
/// stage 04 resolved each argument's source and path from the authored
/// transition, so nothing here rejoins it to source.
pub(super) fn admit_transfers(
    state: &CheckedComposedUnitControlStatePlan,
    edge: &CheckedStructuralControlSuccessorPlan,
    target: &CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    use checked_trees::CheckedStructuralAccess as Access;
    use checked_trees::CheckedStructuralControlTransferSourcePlan as Source;
    use checked_trees::CheckedUnitStructuralArgumentSourcePlan as Subject;
    for (target, transfer) in target.structural_parameters.iter().zip(&edge.transfers) {
        if let Some(argument) = subslices::transfer_range(&transfer.source)
            && let Some(range) = crate::unit::attached_unit::view_ranges::range(argument)
        {
            let collection = &argument.path;
            match range.root {
                checked_trees::CheckedStorageRoot::Parameter { index } => {
                    let source = state.structural_parameters.get(index as usize).ok_or(
                        LoweringError::Unsupported("Unit graph borrowed transfer source missing"),
                    )?;
                    // The field owner's root only lends the array: it must be
                    // readable and unqualified. The projection stays inside
                    // the rooted subject's own storage: members of records and
                    // selected case payloads, and array elements those
                    // members name. A reference crossing would lend storage
                    // another place owns, so it stays refused here.
                    if !collection.is_empty()
                        && (!collection.iter().all(|segment| {
                            matches!(
                                segment,
                                checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                                    | checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                                    | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                                        _
                                    )
                                    | checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                                        _
                                    )
                            )
                        }) || !matches!(
                            source.access,
                            Access::SharedBorrow | Access::MutableBorrow
                        ) || (target.access == Access::MutableBorrow
                            && source.access != Access::MutableBorrow)
                            || !source.qualifications.is_empty()
                            || !source.projected_qualifications.is_empty())
                    {
                        return unsupported(
                            "Unit graph subslice field owner cannot lend the requested view",
                        );
                    }
                }
                checked_trees::CheckedStorageRoot::ViewLocal { .. } => {
                    if !collection.is_empty() {
                        return unsupported("Unit graph subslice field range lost its owner");
                    }
                }
                checked_trees::CheckedStorageRoot::OwnedLocal { .. } => {
                    return unsupported(
                        "Unit graph borrowed transfer source is an owned local, not a view",
                    );
                }
            }
            continue;
        }
        match &transfer.source {
            Source::EstablishedValue {
                operands,
                establishment,
            } => {
                for operand in operands.iter() {
                    match operand {
                        CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                                operand_source: Some(_),
                                ..
                            },
                        )
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. },
                        )
                        | CheckedUnitEffectOperationPlan::Call(_) => {}
                        _ => return unsupported("Unit graph operand is not call preparation"),
                    }
                }
                match establishment.as_ref() {
                    CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                            operand_source: None,
                            discard_result_on_return: false,
                            ..
                        },
                    ) => {}
                    CheckedUnitEffectOperationPlan::Call(call)
                        if call.discard_result_on_return() == Some(false) => {}
                    CheckedUnitEffectOperationPlan::Call(_) => {
                        return unsupported(
                            "Unit graph call argument is discarded before transfer",
                        );
                    }
                    _ => return unsupported("Unit graph argument is not a fresh value producer"),
                }
            }
            // The edge emitter lends an existing place: a parameter or a
            // retained result binding.
            Source::ProjectedLoan { subject, .. }
                if matches!(subject.source, Subject::StructuralLocal { .. })
                    && subject
                        .source
                        .current_result_binding(&state.operations)
                        .is_none() =>
            {
                return unsupported("Unit graph named loan lost its producing carrier metadata");
            }
            Source::ProjectedLoan { subject, .. }
                if !matches!(
                    subject.source,
                    Subject::Parameter { .. }
                        | Subject::StructuralResult { .. }
                        | Subject::StructuralLocal { .. }
                ) =>
            {
                return unsupported("Unit graph projected loan has no existing source place");
            }
            Source::ProjectedValue { subject } => match subject.source {
                Subject::Parameter { parameter_index } => {
                    let retained = state
                        .structural_parameters
                        .get(parameter_index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph projected subject missing",
                        ))?;
                    if retained.access == Access::WriteOnlyBorrow {
                        return unsupported("Unit graph projected subject is write-only");
                    }
                }
                Subject::StructuralResult { .. } => {}
                _ => return unsupported("Unit graph projected subject unsupported"),
            },
            Source::Parameter { .. }
            | Source::StructuralResult { .. }
            | Source::ProjectedLoan { .. }
            | Source::CasePayload { .. }
            | Source::ByteSequenceLiteral { .. }
            | Source::ViewSubslice(_) => {}
        }
    }
    Ok(())
}
