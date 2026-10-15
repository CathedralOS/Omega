//! Lower the selected normal edge's owned dispositions in the live value namespace.
//!
//! Source ownership has already separated target and continuation exits. This
//! join keeps that coordinate through value materialization; a state's union of
//! dead roots cannot choose actions for one return. Hook requirements become
//! ordinary edge obligations, proved from the caller contract and selected path.
use super::{CheckedTrees, LoweringError};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::unit::attached_unit::argument_evaluation::{Evaluation, ScalarReturnCleanup};
use crate::unit::unit_cleanup::CleanupRegion;
use checked_trees::{CheckedScalarBranchDestination, FlowClaimJoinExit, FlowClaimJoinExitKind};
use terminal_psi::StructuralParameterDeclaration;

/// Borrowed source authority common to sequence and graph completion.
#[derive(Clone, Copy)]
pub(crate) struct NormalExitSource<'a> {
    pub(crate) cleanups: &'a [checked_trees::CheckedNormalExitCleanupPlan],
    /// Existing checked no-code dispositions of leading constructor locals.
    pub(crate) trivial_affine_discards: &'a [semantic_vocabulary::PlaceId],
    pub(crate) operations: &'a [checked_trees::CheckedUnitEffectOperationPlan],
    pub(crate) parameters: &'a [checked_trees::CheckedUnitStructuralParameterPlan],
}

/// Existing callable/storage identities used to materialize checked dispositions.
pub(crate) struct CompletionCleanupCatalog<'a, 'calls> {
    pub(crate) control: &'a checked_trees::CheckedCleanupControlPlans,
    pub(crate) types: &'a [terminal_psi::StructuralTypeDeclaration],
    pub(crate) type_ids: &'a [(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
    pub(crate) calls:
        &'calls mut crate::emission::operation_emission::calls::CallEmissionContext<'a>,
}

pub(crate) fn destination(
    source: NormalExitSource<'_>,
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
    coordinate(source, *statement_ordinal as usize, *is_continuation)
}

/// Keep each authored scalar return's cleanup before its private result join.
/// Preparing action identities does not discharge their prerequisites; complete
/// graph proof production observes the selected path and outgoing call effects.
pub(crate) fn scalar_returns(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: &[(
        language_semantics::PermissionClaimIdentity,
        semantic_vocabulary::ClaimId,
    )],
    checked: &CheckedTrees,
    source: NormalExitSource<'_>,
    control: &checked_trees::CheckedUnitScalarControlPlan,
    catalog: &mut CompletionCleanupCatalog<'_, '_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<ScalarReturnCleanup>, LoweringError> {
    if source.cleanups.is_empty() {
        return Err(LoweringError::Unsupported(
            "scalar return has no source cleanup coordinates",
        ));
    }
    // Source admission retains all authored exits. Only the checked control
    // plan selects runtime returns; preparing dead cleanup rows would allocate
    // orphan completion blocks and their obligations. Every selected return
    // still requires its unique, exact source-owned cleanup coordinate.
    let destinations =
        crate::unit::attached_unit::scalar_completion::control::returning_destinations(
            checked, control,
        )?;
    destinations
        .into_iter()
        .map(|destination| {
            let exit = self::destination(source, &destination)?;
            let mut actions = for_exit_regions(
                identities, claims, checked, source, exit, catalog, parameters, evaluation,
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
pub(crate) fn selection(
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
pub(crate) fn coordinate(
    source: NormalExitSource<'_>,
    statement_index: usize,
    continuation: bool,
) -> Result<FlowClaimJoinExit, LoweringError> {
    let mut rows = source.cleanups.iter().filter(|row| {
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
pub(crate) fn for_exit_regions(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: &[(
        language_semantics::PermissionClaimIdentity,
        semantic_vocabulary::ClaimId,
    )],
    checked: &CheckedTrees,
    source: NormalExitSource<'_>,
    exit: FlowClaimJoinExit,
    catalog: &mut CompletionCleanupCatalog<'_, '_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<CleanupRegion>, LoweringError> {
    let mut selected = source.cleanups.iter().filter(|plan| plan.exit == exit);
    let selected_exit = selected.next().ok_or(LoweringError::Unsupported(
        "normal return is missing its exact cleanup plan",
    ))?;
    if selected.next().is_some() {
        return Err(LoweringError::Unsupported(
            "normal cleanup repeats its selected exit",
        ));
    }
    let rows = crate::unit::unit_cleanup::lower_checked_cleanup_regions(
        identities,
        checked,
        crate::unit::unit_cleanup::outstanding_actions(
            catalog
                .control
                .normal_exit_actions
                .span_or_empty(selected_exit.actions),
            source.operations,
            source.parameters,
        ),
        catalog
            .control
            .normal_exit_regions
            .span_or_empty(selected_exit.selected_regions),
        catalog.control,
        catalog.types,
        catalog.type_ids,
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
                identities, path, storage.selected_type, catalog.types, None, &mut std::iter::empty(), None,
            )?);
            Ok((storage.place, storage.path))
        },
        |place| evaluation.carrier_multiplicity(place, operations),
        |cleanup, place, structural_type, path| {
            let mut action = crate::unit::unit_cleanup::lower_nominal_cleanup_action(
                cleanup,
                place,
                structural_type,
                path,
                catalog.calls.machine_ids,
                catalog.calls.cleanup_receivers,
                catalog.calls.requirement_counts,
                &mut catalog.calls.next_obligation_identity,
            )?;
            crate::unit::unit_cleanup::bind_consumed_claims(
                &mut action,
                &cleanup.consumed_claims,
                claims,
            )?;
            Ok(action)
        },
    )?;
    let mut regions = crate::unit::unit_cleanup::lower_reference_completion_regions(
        identities,
        source.operations,
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
    )?;
    regions.extend(source.trivial_affine_discards.iter().map(|place| {
        CleanupRegion::unconditional(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
            *place,
        ))
    }));
    Ok(regions)
}
