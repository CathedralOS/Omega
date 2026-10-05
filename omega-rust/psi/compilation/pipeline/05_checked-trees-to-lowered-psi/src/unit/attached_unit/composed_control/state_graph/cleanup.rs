//! Lower the selected normal edge's owned dispositions in the live value namespace.
//!
//! Source ownership has already separated target and continuation exits. This
//! join keeps that coordinate through value materialization; a state's union of
//! dead roots cannot choose actions for one return. Hook requirements become
//! ordinary edge obligations, proved from the caller contract and selected path.
use super::super::{CheckedTrees, LoweringError, catalogs};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::unit::attached_unit::argument_evaluation::Evaluation;
use checked_trees::{
    CheckedComposedUnitControlStatePlan, CheckedNormalCleanupAction,
    CheckedScalarBranchDestination, CheckedUnitStructuralArgumentSourcePlan, FlowClaimJoinExit,
    FlowClaimJoinExitKind,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    StructuralAffineDiscard, StructuralParameterDeclaration, TerminalAffineCleanupAction,
};

pub(super) fn destination(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    destination: &CheckedScalarBranchDestination,
) -> Result<FlowClaimJoinExit, LoweringError> {
    let CheckedScalarBranchDestination::Return {
        statement_ordinal,
        is_continuation,
    } = destination
    else {
        return Err(LoweringError::Unsupported(
            "cleanup destination is not a normal value return",
        ));
    };
    coordinate(
        checked,
        state,
        *statement_ordinal as usize,
        *is_continuation,
    )
}

pub(super) fn coordinate(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    statement_index: usize,
    continuation: bool,
) -> Result<FlowClaimJoinExit, LoweringError> {
    let (_, authored) = crate::declarations::state_declaration(checked, state.state)?;
    let statement = checked
        .statement_table
        .statements(authored.statement_nodes)
        .get(statement_index);
    let kind = match statement {
        Some(checked_trees::statement::StatementNode::Transition(transition)) => {
            let target = if continuation {
                transition.continuation
            } else {
                transition.target
            };
            if !target.is_valid() {
                return Err(LoweringError::Unsupported(
                    "normal cleanup return target is absent",
                ));
            }
            if continuation {
                FlowClaimJoinExitKind::TransitionContinuation
            } else {
                FlowClaimJoinExitKind::TransitionTarget
            }
        }
        Some(checked_trees::statement::StatementNode::Expression(_)) | None if !continuation => {
            FlowClaimJoinExitKind::FinalExpression
        }
        _ => {
            return Err(LoweringError::Unsupported(
                "normal cleanup return coordinate drifted",
            ));
        }
    };
    Ok(FlowClaimJoinExit {
        state_symbol: state.state,
        statement_index,
        kind,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn for_exit(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    exit: FlowClaimJoinExit,
    catalogs: &mut catalogs::ComposedCatalogs<'_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<TerminalAffineCleanupAction>, LoweringError> {
    let mut selected = state
        .normal_exit_cleanups
        .iter()
        .filter(|plan| plan.exit == exit);
    let selected_exit = selected.next().ok_or(LoweringError::Unsupported(
        "normal return is missing its exact cleanup plan",
    ))?;
    if selected.next().is_some() {
        return Err(LoweringError::Unsupported(
            "normal cleanup repeats its selected exit",
        ));
    }
    let receivers = catalogs
        .signatures
        .iter()
        .filter_map(|signature| {
            signature
                .cleanup_receiver
                .map(|receiver| (signature.source, receiver))
        })
        .collect::<Vec<_>>();
    selected_exit.actions.iter().map(|action| {
        let (place, root_type) = source(action.source(), parameters, evaluation, operations)?;
        match action {
            CheckedNormalCleanupAction::Discard(discard) => {
                let structural_type = crate::terminal_identities::lookup_type_id(&catalogs.type_ids, &discard.type_identity)?;
                if discard.path.is_empty() {
                    Ok(TerminalAffineCleanupAction::DiscardRoot(place))
                } else {
                    let path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                        &discard.path, root_type, &catalogs.structural_types, None, &mut std::iter::empty(), None,
                    )?;
                    Ok(TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard { place, path, structural_type }))
                }
            }
            CheckedNormalCleanupAction::InvokeNominal(cleanup) => {
                Ok(TerminalAffineCleanupAction::InvokeNominal(crate::unit::unit_cleanup::lower_nominal_cleanup_action(
                    checked, cleanup, place, root_type, &catalogs.type_ids,
                    &catalogs.scalar_calls.machine_ids, &receivers, &mut catalogs.scalar_calls.next_call_obligation,
                )?))
            }
        }
    }).collect::<Result<Vec<_>, _>>()
}

fn source(
    source: &CheckedUnitStructuralArgumentSourcePlan,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<(PlaceId, StructuralTypeId), LoweringError> {
    let (place, structural_type) = match source {
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter =
                parameters
                    .get(*parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "normal cleanup parameter is absent",
                    ))?;
            (parameter.place, parameter.structural_type)
        }
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            let local = evaluation
                .structural_locals
                .iter()
                .find(|local| local.root == facts::PlaceRoot::Symbol(*symbol))
                .ok_or(LoweringError::Unsupported(
                    "normal cleanup local is not established",
                ))?;
            if !local.argument.path.is_empty() {
                return Err(LoweringError::Unsupported(
                    "normal cleanup local is not its whole owner",
                ));
            }
            (local.argument.place, local.structural_type)
        }
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
            let (_, result) = operations
                .structural_values
                .iter()
                .find(|(ordinal, _)| ordinal == binding_ordinal)
                .ok_or(LoweringError::Unsupported(
                    "normal cleanup result is not established",
                ))?;
            (result.place(), result.structural_type())
        }
        _ => {
            return Err(LoweringError::Unsupported(
                "normal cleanup source has no owned structural storage",
            ));
        }
    };
    Ok((evaluation.current_structural_place(place), structural_type))
}
