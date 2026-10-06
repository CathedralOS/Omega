//! Lower the selected normal edge's owned dispositions in the live value namespace.
//!
//! Source ownership has already separated target and continuation exits. This
//! join keeps that coordinate through value materialization; a state's union of
//! dead roots cannot choose actions for one return. Hook requirements become
//! ordinary edge obligations, proved from the caller contract and selected path.
use super::super::{CheckedTrees, LoweringError, catalogs};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::unit::attached_unit::argument_evaluation::{Evaluation, ScalarReturnCleanup};
use checked_trees::{
    CheckedComposedUnitControlStatePlan, CheckedNormalCleanupAction,
    CheckedScalarBranchDestination, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, FlowClaimJoinExit, FlowClaimJoinExitKind,
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

/// Keep each authored scalar return's cleanup before its private result join.
/// Preparing action identities does not discharge their prerequisites; complete
/// graph proof production observes the selected path and outgoing call effects.
pub(super) fn scalar_returns(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    catalogs: &mut catalogs::ComposedCatalogs<'_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<ScalarReturnCleanup>, LoweringError> {
    if state.normal_exit_cleanups.is_empty() {
        return Err(LoweringError::Unsupported(
            "scalar return has no source cleanup coordinates",
        ));
    }
    state
        .normal_exit_cleanups
        .iter()
        .map(|cleanup| {
            let destination = CheckedScalarBranchDestination::Return {
                statement_ordinal: u32::try_from(cleanup.exit.statement_index).map_err(|_| {
                    LoweringError::Unsupported("scalar return statement ordinal exceeds u32")
                })?,
                is_continuation: cleanup.exit.kind == FlowClaimJoinExitKind::TransitionContinuation,
            };
            if self::destination(checked, state, &destination)? != cleanup.exit {
                return Err(LoweringError::Unsupported(
                    "scalar return cleanup coordinate drifted",
                ));
            }
            let mut actions = for_exit(
                checked,
                state,
                cleanup.exit,
                catalogs,
                parameters,
                evaluation,
                operations,
            )?;
            selection(state, evaluation, operations, &mut actions)?;
            Ok(ScalarReturnCleanup {
                destination,
                actions,
            })
        })
        .collect()
}

/// Replace selected-owner source rows at their original establishment positions.
/// Returns and private scalar completion edges share this receipt substitution;
/// action groups stay intact instead of regrouping nominal and residual work.
pub(super) fn selection(
    state: &CheckedComposedUnitControlStatePlan,
    evaluation: &Evaluation,
    operations: &OperationBuffer,
    actions: &mut Vec<TerminalAffineCleanupAction>,
) -> Result<(), LoweringError> {
    if evaluation.selection_cleanups.is_empty() {
        return Ok(());
    }
    let mut roots = Vec::new();
    for operation in state.operations.iter().rev() {
        if let Some((result, _)) = operation.discardable_structural_result() {
            let place =
                super::case_emission::result(state, result.binding_ordinal, operations)?.place();
            // The operation roster supplies order, not disposition authority:
            // one arm may consume a local that another arm must clean up.
            let disposed = actions
                .iter()
                .any(|action| action.place() == evaluation.current_structural_place(place));
            roots.push((place, disposed));
        }
    }
    let mut parameter_sources = Vec::new();
    for cleanup in &evaluation.selection_cleanups {
        for source in &cleanup.sources {
            if let Some((position, _)) = evaluation
                .structural_parameters
                .iter()
                .find(|(_, declaration)| declaration.place == *source)
            {
                parameter_sources.push((*position, *source));
            }
        }
    }
    parameter_sources.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
    parameter_sources.dedup_by_key(|(_, place)| *place);
    roots.extend(
        parameter_sources
            .into_iter()
            .map(|(_, place)| (place, false)),
    );
    let mut action_roots = Vec::new();
    for action in actions.iter() {
        if !action_roots.contains(&action.place()) {
            action_roots.push(action.place());
        }
    }
    for place in action_roots {
        if !roots
            .iter()
            .any(|(source, _)| evaluation.current_structural_place(*source) == place)
        {
            roots.push((place, true));
        }
    }
    let selected = evaluation.selection_return_discards(roots)?;
    let previous = std::mem::take(actions);
    for place in selected {
        let retained = previous.iter().filter(|action| action.place() == place);
        if retained.clone().next().is_some() {
            actions.extend(retained.cloned());
        } else {
            actions.push(TerminalAffineCleanupAction::DiscardRoot(place));
        }
    }
    Ok(())
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

/// Dispositions a continuation cleanup in the body already committed.
///
/// Stage 04 records the state's whole dying complement on every normal exit
/// and separately records the cleanup each body boundary runs, so an exit's
/// action stream repeats what the body already discharged before control
/// reached any exit. Only a disposition whose last use belongs to the exit
/// remains for its edge to commit; repeating one disposes the same owner
/// twice. The ordinary return and selection routes subtract the same rows
/// through `result_custody::retired_in_body` and
/// `result_custody::statement_discards`.
fn discharged_in_body(
    state: &CheckedComposedUnitControlStatePlan,
) -> Vec<&CheckedNormalCleanupAction> {
    state
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                cleanup_actions, ..
            } => Some(cleanup_actions),
            _ => None,
        })
        .flatten()
        .collect()
}

/// Carriers a deferred reference release leaves for this exit to retire.
///
/// Stage 04 schedules a loan whose last use is the state's transition one
/// statement past the authored body, where the per-statement emitter has no
/// position, so the selected edge owns the disposal (`result_custody` states
/// the same division: only loans whose last use belongs to a transition remain
/// for the selected edge to drop). Ordinary releases instead reach the body as
/// an emitted `ReleaseReference` and need nothing here.
///
/// These carriers precede the exit's owned roots. `ownership.md` requires that
/// "an owner cannot die before cleanup depending on its borrow", and the
/// borrowed owner dies on this same edge; a carrier left for a later exit
/// outlives the storage it names.
fn deferred_carrier_releases(
    state: &CheckedComposedUnitControlStatePlan,
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<TerminalAffineCleanupAction>, LoweringError> {
    let mut carriers = Vec::new();
    // Reverse operation order is reverse declaration order: the last carrier
    // established is the first retired.
    for operation in state.operations.iter().rev() {
        let CheckedUnitEffectOperationPlan::ReleaseReference {
            binding_ordinal,
            path,
            defer_to_exit: true,
            ..
        } = operation
        else {
            continue;
        };
        if !path.is_empty() {
            return Err(LoweringError::Unsupported(
                "deferred reference release is not its whole carrier",
            ));
        }
        let (_, result) = operations
            .structural_values
            .iter()
            .find(|(ordinal, _)| ordinal == binding_ordinal)
            .ok_or(LoweringError::Unsupported(
                "deferred reference release has no established carrier",
            ))?;
        let place = evaluation.current_structural_place(result.place());
        if !carriers.contains(&place) {
            carriers.push(place);
        }
    }
    Ok(carriers
        .into_iter()
        .map(TerminalAffineCleanupAction::DiscardRoot)
        .collect())
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
    let discharged = discharged_in_body(state);
    let rows = selected_exit.actions.iter().filter(|action|
        !discharged.iter().any(|committed| *committed == *action)
    ).map(|action| {
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
                let structural_type = crate::terminal_identities::lookup_type_id(&catalogs.type_ids, &cleanup.type_identity)?;
                let path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                    &cleanup.path, root_type, &catalogs.structural_types, None, &mut std::iter::empty(), None,
                )?;
                Ok(TerminalAffineCleanupAction::InvokeNominal(crate::unit::unit_cleanup::lower_nominal_cleanup_action(
                    checked, cleanup, place, structural_type, path, &catalogs.type_ids,
                    &catalogs.scalar_calls.machine_ids, &receivers, &catalogs.scalar_calls.requirement_counts,
                    &mut catalogs.scalar_calls.next_call_obligation,
                )?))
            }
        }
    }).collect::<Result<Vec<_>, _>>()?;
    let mut actions = deferred_carrier_releases(state, evaluation, operations)?;
    actions.retain(|carrier| !rows.iter().any(|row| row.place() == carrier.place()));
    actions.extend(rows);
    Ok(actions)
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
