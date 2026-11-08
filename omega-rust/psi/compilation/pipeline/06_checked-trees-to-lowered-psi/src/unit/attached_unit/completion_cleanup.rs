//! Lower the selected normal edge's owned dispositions in the live value namespace.
//!
//! Source ownership has already separated target and continuation exits. This
//! join keeps that coordinate through value materialization; a state's union of
//! dead roots cannot choose actions for one return. Hook requirements become
//! ordinary edge obligations, proved from the caller contract and selected path.
use super::{CheckedTrees, LoweringError};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::unit::attached_unit::argument_evaluation::{
    Evaluation, ScalarReturnCleanup, StructuralValueStorage,
};
use crate::unit::unit_cleanup::CleanupRegion;
use crate::unit::unit_cleanup::bind_consumed_claims;
mod owner_order;
mod remaining_partition;
use checked_trees::{CheckedScalarBranchDestination, FlowClaimJoinExit, FlowClaimJoinExitKind};
pub(crate) use owner_order::selection;
use terminal_psi::StructuralParameterDeclaration;

/// Borrowed source authority common to sequence and graph completion.
#[derive(Clone, Copy)]
pub(crate) struct NormalExitSource<'a> {
    pub(crate) completion: checked_trees::CheckedNormalCompletion,
    pub(crate) cleanups: &'a [checked_trees::CheckedNormalExitCleanupPlan],
    /// Existing checked no-code dispositions of leading constructor locals.
    pub(crate) trivial_affine_discards: &'a [semantic_vocabulary::PlaceId],
    pub(crate) operations: &'a [checked_trees::CheckedUnitEffectOperationPlan],
    pub(crate) parameters: &'a [checked_trees::CheckedUnitStructuralParameterPlan],
}

/// Existing callable/storage identities used to materialize checked dispositions.
///
/// The call context borrows the emitting machine's call namespace, which a
/// composed state hands to one operation at a time; that namespace's own
/// lifetime is independent of the catalog's control/type borrows, so it is
/// named separately and a site can build the catalog from within
/// `with_call_namespace` without tying the two.
pub(crate) struct CompletionCleanupCatalog<'a, 'calls, 'context> {
    pub(crate) control: &'a checked_trees::CheckedCleanupControlPlans,
    pub(crate) types: &'a [terminal_psi::StructuralTypeDeclaration],
    pub(crate) type_ids: &'a [(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
    pub(crate) calls:
        &'calls mut crate::emission::operation_emission::calls::CallEmissionContext<'context>,
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
/// Working state for the exact source-published normal result tree.
pub(crate) enum NormalResultCleanup {
    NoNormal {
        computation: checked_trees::CheckedScalarComputationHandle,
    },
    Value {
        computation: checked_trees::CheckedScalarComputationHandle,
        actions: Vec<CleanupRegion>,
    },
    Dispatch {
        computation: checked_trees::CheckedScalarComputationHandle,
        arms: Vec<(checked_trees::CheckedScalarDispatchArm, NormalResultCleanup)>,
    },
}

pub(crate) fn scalar_returns(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: &[(
        language_semantics::PermissionClaimIdentity,
        semantic_vocabulary::ClaimId,
    )],
    checked: &CheckedTrees,
    source: NormalExitSource<'_>,
    control: &checked_trees::CheckedUnitScalarControlPlan,
    catalog: &mut CompletionCleanupCatalog<'_, '_, '_>,
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
    control
        .selected_copies
        .iter()
        .map(|row| {
            let destination = CheckedScalarBranchDestination::Return {
                statement_ordinal: row.statement_ordinal,
                is_continuation: row.is_continuation,
            };
            let exit = self::destination(source, &destination)?;
            let plan = plan_for_exit(source, exit)?;
            let (actions, result) = if plan.result_completion.is_valid() {
                (
                    Vec::new(),
                    Some(normal_result_cleanup(
                        identities,
                        claims,
                        checked,
                        source,
                        exit,
                        plan,
                        plan.result_completion,
                        catalog,
                        parameters,
                        evaluation,
                        operations,
                    )?),
                )
            } else {
                let mut actions = for_planned_actions(
                    identities,
                    claims,
                    checked,
                    source,
                    exit,
                    plan,
                    catalog.control.actions.span_or_empty(plan.actions),
                    catalog,
                    parameters,
                    evaluation,
                    operations,
                )?;
                selection(
                    exit.state_symbol,
                    source.operations,
                    evaluation,
                    operations,
                    &mut actions,
                )?;
                (actions, None)
            };
            Ok(ScalarReturnCleanup {
                exit,
                destination,
                actions,
                result,
                copied_payloads: row.copied_payloads.clone(),
            })
        })
        .collect()
}

/// The exit a state's normal cleanup row names at `statement_index`: its
/// transition's continuation when `continuation`, otherwise its target or
/// final expression. Psi 05 records exactly one row per exit.
pub(crate) fn coordinate(
    source: NormalExitSource<'_>,
    statement_index: usize,
    continuation: bool,
) -> Result<FlowClaimJoinExit, LoweringError> {
    optional_coordinate(source, statement_index, continuation)?.ok_or(LoweringError::Unsupported(
        "normal exit requested from a no-normal invocation",
    ))
}

/// A selected source transfer still owes its exact cleanup even when its
/// invocation cannot return. Only an absent exit in a checked NoNormal body
/// permits the physical unreachable successor to omit cleanup.
pub(crate) fn optional_coordinate(
    source: NormalExitSource<'_>,
    statement_index: usize,
    continuation: bool,
) -> Result<Option<FlowClaimJoinExit>, LoweringError> {
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
    let Some(row) = rows.next() else {
        return match source.completion {
            checked_trees::CheckedNormalCompletion::NoNormal => Ok(None),
            checked_trees::CheckedNormalCompletion::Possible => Err(LoweringError::Unsupported(
                "normal exit has no cleanup plan",
            )),
        };
    };
    if rows.next().is_some() {
        return Err(LoweringError::Unsupported(
            "normal exit has more than one cleanup plan",
        ));
    }
    Ok(Some(row.exit))
}

/// Join the exact selected edge's authored and deferred dispositions before
/// production cleanup selects storage. The shared reference-completion owner
/// preserves holder dependency order and prevents a second descriptor death.
#[allow(clippy::too_many_arguments)]
fn plan_for_exit<'a>(
    source: NormalExitSource<'a>,
    exit: FlowClaimJoinExit,
) -> Result<&'a checked_trees::CheckedNormalExitCleanupPlan, LoweringError> {
    let mut selected = source.cleanups.iter().filter(|plan| plan.exit == exit);
    let selected_exit = selected.next().ok_or(LoweringError::Unsupported(
        "normal return is missing its exact cleanup plan",
    ))?;
    if selected.next().is_some() {
        return Err(LoweringError::Unsupported(
            "normal cleanup repeats its selected exit",
        ));
    }
    Ok(selected_exit)
}

pub(crate) fn for_exit_regions(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: &[(
        language_semantics::PermissionClaimIdentity,
        semantic_vocabulary::ClaimId,
    )],
    checked: &CheckedTrees,
    source: NormalExitSource<'_>,
    exit: FlowClaimJoinExit,
    catalog: &mut CompletionCleanupCatalog<'_, '_, '_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<CleanupRegion>, LoweringError> {
    let plan = plan_for_exit(source, exit)?;
    for_planned_actions(
        identities,
        claims,
        checked,
        source,
        exit,
        plan,
        catalog.control.actions.span_or_empty(plan.actions),
        catalog,
        parameters,
        evaluation,
        operations,
    )
}

#[allow(clippy::too_many_arguments)]
fn for_planned_actions(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: &[(
        language_semantics::PermissionClaimIdentity,
        semantic_vocabulary::ClaimId,
    )],
    checked: &CheckedTrees,
    source: NormalExitSource<'_>,
    exit: FlowClaimJoinExit,
    selected_exit: &checked_trees::CheckedNormalExitCleanupPlan,
    selected_actions: &[checked_trees::CheckedNormalCleanupAction],
    catalog: &mut CompletionCleanupCatalog<'_, '_, '_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<CleanupRegion>, LoweringError> {
    let cleanup_control = catalog.control;
    let planned = crate::unit::unit_cleanup::outstanding_actions(
        selected_actions,
        source.operations,
        source.parameters,
        cleanup_control,
    );
    let mut actions = Vec::with_capacity(planned.len());
    for action in planned {
        // Planned source enums do not describe the entire emitted prefix:
        // nested calls can already have disposed this exact physical home.
        // Rebinding clears the receipt; source consumption alone is insufficient.
        if !matches!(
            action,
            checked_trees::CheckedNormalCleanupAction::InvokeNominal(_)
        ) && evaluation.cleanup_source_is_retired(action.source(), parameters, |binding| {
            result_storage(operations, binding)
        })? {
            continue;
        }
        actions.push(action);
    }
    let declaration_orders = actions
        .iter()
        .map(|action| {
            owner_order::declaring_order(checked, exit.state_symbol, source, action.source())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut rows = lower_regions(
        identities,
        claims,
        checked,
        actions,
        cleanup_control
            .normal_exit_regions
            .span_or_empty(selected_exit.selected_regions),
        catalog,
        parameters,
        evaluation,
        operations,
    )?;
    for (row, order) in rows.iter_mut().zip(declaration_orders) {
        row.declaration_order = order;
    }
    rows = remaining_partition::retain(evaluation, rows, catalog.types, operations)?;
    let mut regions = crate::unit::unit_cleanup::lower_reference_completion_regions(
        identities,
        checked,
        source.operations,
        rows,
        parameters,
        evaluation,
        operations,
        |identities, owner| {
            lower_regions(
                identities,
                claims,
                checked,
                cleanup_control.actions.span_or_empty(owner.actions),
                std::slice::from_ref(&owner.owner),
                catalog,
                parameters,
                evaluation,
                operations,
            )
        },
        |binding| result_storage(operations, binding),
    )?;
    regions.extend(source.trivial_affine_discards.iter().map(|place| {
        CleanupRegion::unconditional(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
            *place,
        ))
    }));
    Ok(regions)
}

#[allow(clippy::too_many_arguments)]
fn normal_result_cleanup(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: &[(
        language_semantics::PermissionClaimIdentity,
        semantic_vocabulary::ClaimId,
    )],
    checked: &CheckedTrees,
    source: NormalExitSource<'_>,
    exit: FlowClaimJoinExit,
    plan: &checked_trees::CheckedNormalExitCleanupPlan,
    result: arena::Handle<checked_trees::CheckedNormalResultCompletion>,
    catalog: &mut CompletionCleanupCatalog<'_, '_, '_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<NormalResultCleanup, LoweringError> {
    let control = catalog.control;
    Ok(match control.normal_result_completions.get(result) {
        checked_trees::CheckedNormalResultCompletion::NoNormal { computation } => {
            NormalResultCleanup::NoNormal {
                computation: *computation,
            }
        }
        checked_trees::CheckedNormalResultCompletion::Value {
            computation,
            actions,
        } => {
            let mut actions = for_planned_actions(
                identities,
                claims,
                checked,
                source,
                exit,
                plan,
                control.actions.span_or_empty(*actions),
                catalog,
                parameters,
                evaluation,
                operations,
            )?;
            selection(
                exit.state_symbol,
                source.operations,
                evaluation,
                operations,
                &mut actions,
            )?;
            NormalResultCleanup::Value {
                computation: *computation,
                actions,
            }
        }
        checked_trees::CheckedNormalResultCompletion::Dispatch { computation, arms } => {
            let mut results = Vec::new();
            for arm in control.normal_result_arms.span_or_empty(*arms) {
                let child = normal_result_cleanup(
                    identities, claims, checked, source, exit, plan, arm.result, catalog,
                    parameters, evaluation, operations,
                )?;
                results.push((
                    checked
                        .facts
                        .values
                        .scalar_computations
                        .dispatch_arms
                        .get(arm.arm)
                        .clone(),
                    child,
                ));
            }
            NormalResultCleanup::Dispatch {
                computation: *computation,
                arms: results,
            }
        }
    })
}

fn result_storage(
    operations: &OperationBuffer,
    binding: u32,
) -> Result<StructuralValueStorage, LoweringError> {
    let (_, result) = operations
        .structural_values
        .iter()
        .find(|(ordinal, _)| *ordinal == binding)
        .ok_or(LoweringError::Unsupported(
            "cleanup result is not established",
        ))?;
    Ok(result.into())
}

/// Every completion action resolves through the same captured value namespace.
#[allow(clippy::too_many_arguments)]
fn lower_regions<'actions>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: &[(
        language_semantics::PermissionClaimIdentity,
        semantic_vocabulary::ClaimId,
    )],
    checked: &CheckedTrees,
    actions: impl IntoIterator<Item = &'actions checked_trees::CheckedNormalCleanupAction>,
    selected_regions: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    catalog: &mut CompletionCleanupCatalog<'_, '_, '_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<CleanupRegion>, LoweringError> {
    crate::unit::unit_cleanup::lower_checked_cleanup_regions(
        identities,
        checked,
        actions,
        selected_regions,
        catalog.control,
        catalog.types,
        catalog.type_ids,
        |identities, source, path| {
            let mut storage = evaluation.cleanup_value_storage(source, parameters, |binding| {
                result_storage(operations, binding)
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
            bind_consumed_claims(&mut action, &cleanup.consumed_claims, claims)?;
            Ok(action)
        },
    )
}
