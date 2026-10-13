//! Lower the selected normal edge's owned dispositions in the live value namespace.
//!
//! Source ownership has already separated target and continuation exits. This
//! join keeps that coordinate through value materialization; a state's union of
//! dead roots cannot choose actions for one return. Hook requirements become
//! ordinary edge obligations, proved from the caller contract and selected path.
use super::super::{CheckedTrees, LoweringError, catalogs};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::unit::attached_unit::argument_evaluation::{Evaluation, ScalarReturnCleanup};
use crate::unit::unit_cleanup::CleanupRegion;
use checked_trees::{
    CheckedComposedUnitControlStatePlan, CheckedScalarBranchDestination, FlowClaimJoinExit,
    FlowClaimJoinExitKind,
};
use terminal_psi::StructuralParameterDeclaration;

mod case_capture;
pub(super) use case_capture::retire_captured_subjects;

pub(super) fn destination(
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
    coordinate(state, *statement_ordinal as usize, *is_continuation)
}

/// Keep each authored scalar return's cleanup before its private result join.
/// Preparing action identities does not discharge their prerequisites; complete
/// graph proof production observes the selected path and outgoing call effects.
pub(super) fn scalar_returns(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
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
            let mut actions = for_exit_regions(
                identities,
                claims,
                checked,
                state,
                cleanup.exit,
                catalogs,
                parameters,
                evaluation,
                operations,
            )?;
            selection(evaluation, operations, &mut actions)?;
            Ok(ScalarReturnCleanup {
                destination,
                actions,
            })
        })
        .collect()
}

/// Merge exit dispositions in reverse establishment order, including anonymous
/// producers inside a checked value. A reference local alone is not the full
/// roster: its backing can itself hold another temporary's descriptor. Retain
/// each owner's authored action order and substitute selected storage at that
/// same position. This grants no disposal authority to a production row.
pub(super) fn selection(
    evaluation: &Evaluation,
    operations: &OperationBuffer,
    actions: &mut Vec<CleanupRegion>,
) -> Result<(), LoweringError> {
    let mut roots = Vec::new();
    for place in operations.structural_establishments.iter().copied().rev() {
        let disposed = actions
            .iter()
            .any(|region| region.action.place() == evaluation.current_structural_place(place));
        roots.push((place, disposed));
    }
    roots.extend(
        evaluation
            .cleanup_parameter_places()
            .into_iter()
            .map(|place| (place, false)),
    );
    let mut action_roots = Vec::new();
    for action in actions.iter() {
        if !action_roots.contains(&action.action.place()) {
            action_roots.push(action.action.place());
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
        let retained = previous
            .iter()
            .filter(|region| region.action.place() == place);
        if retained.clone().next().is_some() {
            actions.extend(retained.cloned());
        } else {
            actions.extend(evaluation.normal_discard_regions(place, &evaluation.structural_types)?);
        }
    }
    Ok(())
}

/// The exit a state's normal cleanup row names at `statement_index`: its
/// transition's continuation when `continuation`, otherwise its target or
/// final expression. Stage 04 records exactly one row per exit.
pub(super) fn coordinate(
    state: &CheckedComposedUnitControlStatePlan,
    statement_index: usize,
    continuation: bool,
) -> Result<FlowClaimJoinExit, LoweringError> {
    let mut rows = state.normal_exit_cleanups.iter().filter(|row| {
        row.exit.statement_index == statement_index
            && if continuation {
                row.exit.kind == FlowClaimJoinExitKind::TransitionContinuation
            } else {
                matches!(
                    row.exit.kind,
                    FlowClaimJoinExitKind::TransitionTarget
                        | FlowClaimJoinExitKind::FinalExpression
                )
            }
    });
    let row = rows.next().ok_or(LoweringError::Unsupported(
        "normal exit has no cleanup plan",
    ))?;
    if rows.next().is_some() {
        return Err(LoweringError::Unsupported(
            "normal exit has more than one cleanup plan",
        ));
    }
    Ok(row.exit)
}

/// Join the exact selected edge's authored and deferred dispositions before
/// production cleanup selects storage. The shared reference-completion owner
/// preserves holder dependency order and prevents a second descriptor death.
#[allow(clippy::too_many_arguments)]
pub(super) fn for_exit_regions(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    exit: FlowClaimJoinExit,
    catalogs: &mut catalogs::ComposedCatalogs<'_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<CleanupRegion>, LoweringError> {
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
    let rows = crate::unit::unit_cleanup::lower_checked_cleanup_regions(
        identities,
        checked,
        crate::unit::unit_cleanup::outstanding_actions(
            &selected_exit.actions,
            &state.operations,
            &state.structural_parameters,
        ),
        &selected_exit.selected_regions,
        catalogs.cleanup_control,
        &catalogs.structural_types,
        &catalogs.type_ids,
        |identities, source, path| {
            let mut storage =
                evaluation.cleanup_storage(source, parameters, |binding_ordinal| {
                    let (_, result) = operations
                        .structural_values
                        .iter()
                        .find(|(ordinal, _)| *ordinal == binding_ordinal)
                        .ok_or(LoweringError::Unsupported(
                            "cleanup result is not established",
                        ))?;
                    Ok(result.into())
                })?;
            storage.path.extend(crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                identities, path, storage.selected_type, &catalogs.structural_types, None, &mut std::iter::empty(), None,
            )?);
            Ok((storage.place, storage.path))
        },
        |cleanup, place, structural_type, path| {
            let mut action = crate::unit::unit_cleanup::lower_nominal_cleanup_action(
                cleanup,
                place,
                structural_type,
                path,
                &catalogs.scalar_calls.machine_ids,
                &receivers,
                &catalogs.scalar_calls.requirement_counts,
                &mut catalogs.scalar_calls.next_call_obligation,
            )?;
            crate::unit::unit_cleanup::bind_consumed_claims(&mut action, cleanup, claims.bindings)?;
            Ok(action)
        },
    )?;
    crate::unit::unit_cleanup::lower_reference_completion_regions(
        identities,
        &state.operations,
        rows,
        parameters,
        evaluation,
        operations,
        |binding| {
            let (_, result) = operations
                .structural_values
                .iter()
                .find(|(ordinal, _)| *ordinal == binding)
                .ok_or(LoweringError::Unsupported(
                    "deferred reference release has no established carrier",
                ))?;
            Ok(result.into())
        },
    )
}
