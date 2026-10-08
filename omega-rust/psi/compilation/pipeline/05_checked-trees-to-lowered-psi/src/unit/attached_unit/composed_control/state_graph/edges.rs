//! Authored successor operands and ownership cleanup at edges and returns.
//!
//! Cleanup owner rosters use the shared ownership-facts query: a root can die
//! on several mutually exclusive exits without being disposed twice. Edge
//! bindings and return transfers select the actual residual cleanup below.
use super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::super::{CheckedUnitEffectOperationPlan, Multiplicity, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::{
    CheckedComposedUnitControlStatePlan, CheckedStructuralControlSuccessorPlan, result_custody,
    subslices,
};
use checked_trees::statement::StatementNode;

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
        if let Some((root, _, collection, _, _)) = subslices::transfer_range(&transfer.source) {
            match root {
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
                        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                            operand_source: Some(_),
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                        | CheckedUnitEffectOperationPlan::Call(_) => {}
                        _ => return unsupported("Unit graph operand is not call preparation"),
                    }
                }
                match establishment.as_ref() {
                    CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                        operand_source: None,
                        discard_result_on_return: false,
                        ..
                    } => {}
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
                if !matches!(
                    subject.source,
                    Subject::Parameter { .. } | Subject::StructuralResult { .. }
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
            | Source::ByteSequenceSubslice { .. }
            | Source::ElementViewSubslice { .. } => {}
        }
    }
    Ok(())
}

/// Reconstruct the remaining whole-parameter drops at an ordinary return.
/// Call consumption and return disposal are separate uses of the same source root.
pub(super) fn return_discards(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<Vec<usize>, LoweringError> {
    let parameters = checked.state_parameters(source);
    // Admission rejoins each Unit arm to an ordinary Terminal target. These
    // selected completions transfer no result, just like a direct Unit return;
    // jumps in the same chain retain their separate edge cleanup.
    let guarded_unit_completion = match &state.terminator {
        CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } => {
            arms.iter().any(|arm| {
                matches!(arm, checked_trees::CheckedGuardedJumpArmPlan::Return(plan)
                    if matches!(plan.return_arm, checked_trees::CheckedConditionalReturnArm::Unit { .. }))
            }) || matches!(fallback, checked_trees::CheckedGuardedJumpFallbackPlan::Return(
                checked_trees::CheckedConditionalReturnArm::Unit { .. }
            ))
        }
        _ => false,
    };
    // Unit and scalar completion transfer no structural result. Both retain
    // the same local custody until return disposal; structural completion
    // instead rejoins the returned owner's transfer before residual cleanup.
    let local_discards = if guarded_unit_completion
        || matches!(
            state.terminator,
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. }
            | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                return_arm: checked_trees::CheckedConditionalReturnArm::Unit { .. }
                    | checked_trees::CheckedConditionalReturnArm::Scalar { .. }
                    | checked_trees::CheckedConditionalReturnArm::Call { .. },
                ..
            }
            // Each arm completes the machine, after its terminal call if any.
            | CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion { .. }
        ) {
        result_custody::local_discards(checked, machine, source, state, None)?
    } else {
        Vec::new()
    };
    let early_discards = result_custody::statement_discards(state);
    let mut drops = Vec::new();
    for event in checked
        .facts
        .flow
        .ownership
        .state_exit_cleanup_roots(machine, state.state)
        .ok_or(LoweringError::Unsupported(
            "Unit return exit cleanup is inconsistent",
        ))?
    {
        if state.operations.iter().filter_map(result_custody::result).any(|result| {
            (local_discards.contains(&result.binding_ordinal)
                || early_discards.contains(&result.binding_ordinal))
                && matches!(checked.statement_table.statements(source.statement_nodes).get(result.statement_index as usize),
                    Some(StatementNode::LocalData(local)) if event.root == facts::PlaceRoot::Symbol(local.symbol))
        }) {
            continue;
        }
        let index = state
            .structural_parameters
            .iter()
            .position(|parameter| {
                parameters
                    .get(parameter.position as usize)
                    .is_some_and(|source| event.root == facts::PlaceRoot::Symbol(source.symbol))
            })
            .ok_or(LoweringError::Unsupported(
                "Unit return has an unaccounted local drop",
            ))?;
        drops.push(index);
    }
    // The source query joins cleanup owners across mutually exclusive exits
    // in first-occurrence order. An earlier branch can transfer a later
    // parameter, so that union is not an executable disposal schedule.
    // Order the authenticated owners by declaration before checking the
    // roster and subtracting consumed roots; surviving owners keep that order.
    drops.sort_unstable_by_key(|parameter_index| {
        std::cmp::Reverse(state.structural_parameters[*parameter_index].position)
    });
    // A whole owned parameter a call consumes needs no exit drop: the call's
    // admission rejoined that move to its authored operand, and its callee
    // owns the value's disposal. Structural return events likewise describe
    // residual custody after the authored value transfer; returning a
    // parameter or moving it into a constructor can remove its exit drop.
    // Source value replay checks those transfers; Terminal independently
    // requires the remaining live frontier, including canonical reverse
    // order, before publication.
    let mut consumed = Vec::new();
    for operation in &state.operations {
        for argument in operation
            .call_structural_arguments()
            .iter()
            .filter(|argument| argument.access == checked_trees::CheckedStructuralAccess::Owned)
        {
            if let Some(index) = argument.source_parameter_index() {
                if !argument.path.is_empty() {
                    return unsupported("Unit return has a partial parameter move");
                }
                consumed.push(index as usize);
            }
        }
    }
    // A closed-sum edge disposes an affine subject before entering the arm's
    // return block. Its checked exit drop remains required evidence, but must
    // not become a second disposal at that return. A copyable subject has no
    // disposal debt: it must not acquire an affine-drop receipt just because
    // the same case route also handles affine values. Boolean guard
    // observations retain their ordinary return cleanup.
    if let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, .. } = &state.terminator
        && let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            subject.source
    {
        let index = parameter_index as usize;
        let parameter =
            state
                .structural_parameters
                .get(index)
                .ok_or(LoweringError::Unsupported(
                    "case return lost its consumed parameter",
                ))?;
        if parameter.multiplicity == Multiplicity::Affine && drops.contains(&index) {
            consumed.push(index);
        }
    }
    drops.retain(|index| !consumed.contains(index));
    Ok(drops)
}
