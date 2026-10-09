//! Structural Unit cleanup lowering.
//!
//! Nominal cleanup contracts are prepared with ordinary callable signatures
//! (`contexts`), then one finalizer installs the checked actions for entries
//! and called members alike. `ordered` only admits and roots a selected nominal
//! entry; partial-affine cleanup lives in `partial`.
//! Build cleanup requirements and their obligation identities here, but leave
//! certificates to final operation proof emission. Owned-field requirements are
//! validity-scoped observations, not permanent assumption slots; their available
//! premises are known only after the complete caller and cleanup edge exist.

use super::{
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedTrees, CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralTypeShape, LoweringError, MachineId, NominalAffineCleanup, PlaceId,
    PrimitiveType, Proposition, ScalarTerm, StructuralTypeId, StructuralTypeShape, TerminalMachine,
    Terminator, lookup_machine_id, lookup_type_id, lower_unit_closure, obligation_id, unsupported,
};
use checked_trees::CheckedUnitNominalAffineCleanupPlan;
use symbols::SymbolHandle;
pub(crate) mod contexts;
mod custody;
mod ordered;
mod partial;
use ordered::lower_ordered_nominal_affine_unit_cleanup_machine;

/// Emission scratch for one checked region. Conditions remain control, not a
/// property of the cleanup callable or authority to discharge its contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CleanupCaseCondition {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) case: semantic_vocabulary::StructuralCaseId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CleanupRegion {
    pub(crate) conditions: Vec<CleanupCaseCondition>,
    pub(crate) action: terminal_psi::TerminalAffineCleanupAction,
}

pub(crate) fn lower_cleanup_conditions(
    control: &checked_trees::CheckedCleanupControlPlans,
    selected: arena::HandleSpan<checked_trees::CheckedCleanupCaseCondition>,
    types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    mut storage: impl FnMut(
        &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Result<
        (PlaceId, Vec<terminal_psi::StructuralPathSegment>),
        LoweringError,
    >,
) -> Result<Vec<CleanupCaseCondition>, LoweringError> {
    if selected.is_empty() {
        return Ok(Vec::new());
    }
    let conditions = control
        .conditions
        .span(selected)
        .ok_or(LoweringError::Unsupported(
            "cleanup conditions lost their owning catalog",
        ))?;
    conditions
        .iter()
        .map(|condition| {
            let path = if condition.path.is_empty() {
                &[][..]
            } else {
                control
                    .paths
                    .span(condition.path)
                    .ok_or(LoweringError::Unsupported(
                        "cleanup condition lost its retained path",
                    ))?
            };
            let (place, path) = storage(path)?;
            let structural_type = lookup_type_id(type_ids, &condition.type_identity)?;
            let (_, case) = crate::expression_preparation::bindings::structural_cases::resolve_at(
                types,
                structural_type,
                &[],
                &condition.case_identity,
            )?;
            Ok(CleanupCaseCondition {
                place,
                path,
                structural_type,
                case,
            })
        })
        .collect()
}

/// Lower a nominal affine Unit cleanup machine. One route serves any number
/// of structural parameters: each root owns exactly one cleanup action, and
/// the actions run in reverse declaration order (`ordered`).
pub(crate) fn lower_nominal_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    lower_ordered_nominal_affine_unit_cleanup_machine(checked, nominal)
}

/// Selected cleanup operations have the same hook contract for Unit and scalar exits.
pub(crate) fn continuation_cleanups(
    operations: &[CheckedUnitEffectOperationPlan],
) -> impl Iterator<Item = &CheckedUnitNominalAffineCleanupPlan> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                cleanup_actions, ..
            } => Some(cleanup_actions),
            CheckedUnitEffectOperationPlan::Complete {
                parameter_cleanup_actions,
                ..
            } => Some(parameter_cleanup_actions),
            _ => None,
        })
        .flatten()
        .filter_map(checked_trees::CheckedNormalCleanupAction::nominal)
}

/// Lower an ordered checked disposition in the caller's live storage namespace.
/// Call continuations and normal exits share action geometry; the caller supplies
/// its binding resolution and hook-obligation allocator, not another cleanup policy.
pub(crate) fn lower_checked_cleanup_actions<'action>(
    actions: impl IntoIterator<Item = &'action checked_trees::CheckedNormalCleanupAction>,
    types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    mut storage: impl FnMut(
        &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
    ) -> Result<(PlaceId, StructuralTypeId), LoweringError>,
    mut nominal: impl FnMut(
        &CheckedUnitNominalAffineCleanupPlan,
        PlaceId,
        StructuralTypeId,
        Vec<terminal_psi::StructuralPathSegment>,
    ) -> Result<NominalAffineCleanup, LoweringError>,
) -> Result<Vec<terminal_psi::TerminalAffineCleanupAction>, LoweringError> {
    actions.into_iter().map(|action| {
        if !action.case_conditions().is_empty() {
            return unsupported("selected-case cleanup control has no executable edge yet");
        }
        let (place, root_type) = storage(action.source())?;
        match action {
            checked_trees::CheckedNormalCleanupAction::RetireHome { .. } => Ok(terminal_psi::TerminalAffineCleanupAction::RetireHome(place)),
            checked_trees::CheckedNormalCleanupAction::Discard(discard) if discard.path.is_empty() => Ok(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)),
            _ => {
                let structural_type = lookup_type_id(type_ids, action.type_identity())?;
                let path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                    action.path(), root_type, types, None, &mut std::iter::empty(), None,
                )?;
                match action {
                    checked_trees::CheckedNormalCleanupAction::Discard(_) => Ok(terminal_psi::TerminalAffineCleanupAction::DiscardResidual(terminal_psi::StructuralAffineDiscard { place, path, structural_type })),
                    checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) => Ok(terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal(cleanup, place, structural_type, path)?)),
                    checked_trees::CheckedNormalCleanupAction::RetireHome { .. } => Err(LoweringError::Unsupported("home retirement has no subtree action")),
                }
            }
        }
    }).collect()
}

pub(crate) fn computation_cleanups(
    checked: &CheckedTrees,
    machine: SymbolHandle,
) -> Result<Vec<&CheckedUnitNominalAffineCleanupPlan>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots_for_machine(machine);
    let mut cleanups = Vec::new();
    for identity in
        crate::expression_preparation::computation_graph::reachable_nodes(checked, &roots)?
    {
        let retained = match plans.nodes.get(identity).kind {
            checked_trees::CheckedScalarComputationKind::Call {
                temporary_cleanups, ..
            } => temporary_cleanups,
            _ => continue,
        };
        for row in plans.temporary_cleanups.span_or_empty(retained) {
            cleanups.extend(
                plans
                    .temporary_cleanup_actions
                    .span_or_empty(row.actions)
                    .iter()
                    .filter_map(checked_trees::CheckedNormalCleanupAction::nominal),
            );
        }
    }
    Ok(cleanups)
}

/// Every published action participates in hook subject/receiver preparation.
pub(crate) fn selected_cleanups(
    checked: &CheckedTrees,
) -> impl Iterator<Item = &CheckedUnitNominalAffineCleanupPlan> {
    let composed_states = || {
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .flat_map(|machine| &machine.states)
    };
    checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
        .iter()
        .flat_map(|nominal| &nominal.cleanups)
        .chain(
            checked
                .facts
                .values
                .scalar_computations
                .temporary_cleanup_actions
                .iter()
                .filter_map(|(_, action)| action.nominal()),
        )
        .chain(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter()
                .flat_map(|machine| continuation_cleanups(&machine.operations)),
        )
        .chain(composed_states().flat_map(|state| continuation_cleanups(&state.operations)))
        .chain(
            composed_states()
                .flat_map(|state| &state.normal_exit_cleanups)
                .flat_map(|exit| &exit.actions)
                .filter_map(|action| match action {
                    checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) => {
                        Some(cleanup)
                    }
                    checked_trees::CheckedNormalCleanupAction::Discard(_)
                    | checked_trees::CheckedNormalCleanupAction::RetireHome { .. } => None,
                }),
        )
}

/// Materialize one checked normal-cleanup operation in the shared closure namespace.
/// The caller owns return value custody; this operation only borrows its exact owner.
pub(crate) fn lower_nominal_cleanup_action(
    cleanup: &CheckedUnitNominalAffineCleanupPlan,
    place: PlaceId,
    structural_type: StructuralTypeId,
    path: Vec<terminal_psi::StructuralPathSegment>,
    machine_ids: &[(SymbolHandle, MachineId)],
    receivers: &[(SymbolHandle, PlaceId)],
    requirement_counts: &[(SymbolHandle, usize)],
    next_obligation: &mut u64,
) -> Result<NominalAffineCleanup, LoweringError> {
    materialize_nominal_cleanup_action(
        cleanup,
        place,
        structural_type,
        path,
        machine_ids,
        receivers.iter().find_map(|(source, receiver)| {
            (*source == cleanup.cleanup_machine).then_some(*receiver)
        }),
        requirement_count(requirement_counts, cleanup.cleanup_machine)?,
        &mut || crate::terminal_identities::allocate_dense(next_obligation).map(obligation_id),
    )
}

/// Bind a selected action to the closure's executable identities. Target and
/// subject selection belong to the checked producer; this only allocates each
/// action's independent obligation roster in the emitting body's namespace.
fn materialize_nominal_cleanup_action(
    cleanup: &CheckedUnitNominalAffineCleanupPlan,
    place: PlaceId,
    structural_type: StructuralTypeId,
    path: Vec<terminal_psi::StructuralPathSegment>,
    machine_ids: &[(SymbolHandle, MachineId)],
    cleanup_receiver: Option<PlaceId>,
    requirement_count: usize,
    allocate_requirement: &mut impl FnMut() -> Result<semantic_vocabulary::ObligationId, LoweringError>,
) -> Result<NominalAffineCleanup, LoweringError> {
    Ok(NominalAffineCleanup {
        place,
        path,
        structural_type,
        cleanup_machine: lookup_machine_id(machine_ids, cleanup.cleanup_machine)?,
        consumed_claims: Vec::new(),
        cleanup_receiver,
        requirement_obligations: (0..requirement_count)
            .map(|_| allocate_requirement())
            .collect::<Result<Vec<_>, _>>()?,
    })
}

/// Cleanup and authored calls owe the same normalized callable roster, including
/// runtime prerequisites merged with authored clauses. Source clause counts and
/// a separate hook predicate parser cannot predict that published roster.
fn requirement_count(
    counts: &[(SymbolHandle, usize)],
    machine: SymbolHandle,
) -> Result<usize, LoweringError> {
    counts
        .iter()
        .find_map(|(source, count)| (*source == machine).then_some(*count))
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup target prerequisite signature is absent",
        ))
}

/// Scalar operand expansion reserves storage before the shared call closure is
/// allocated. Preserve selected hook identity until emission instead of
/// inventing a provisional machine or obligation identity during expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreparedNormalCleanupAction {
    Resolved(terminal_psi::TerminalAffineCleanupAction),
    InvokeNominal {
        cleanup: CheckedUnitNominalAffineCleanupPlan,
        place: PlaceId,
        structural_type: StructuralTypeId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        consumed_claims: Vec<language_semantics::PermissionClaimIdentity>,
    },
}

impl PreparedNormalCleanupAction {
    pub(crate) fn discard_root(place: PlaceId) -> Self {
        Self::Resolved(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
            place,
        ))
    }

    pub(crate) fn discard_residual(discard: terminal_psi::StructuralAffineDiscard) -> Self {
        Self::Resolved(terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
            discard,
        ))
    }
}

impl From<&terminal_psi::TerminalAffineCleanupAction> for PreparedNormalCleanupAction {
    fn from(action: &terminal_psi::TerminalAffineCleanupAction) -> Self {
        Self::Resolved(action.clone())
    }
}

pub(crate) fn emit_prepared_cleanup_actions(
    actions: &[PreparedNormalCleanupAction],
    calls: &mut crate::emission::operation_emission::calls::CallEmissionContext<'_>,
) -> Result<Vec<terminal_psi::TerminalAffineCleanupAction>, LoweringError> {
    actions
        .iter()
        .map(|action| {
            Ok(match action {
                PreparedNormalCleanupAction::Resolved(action) => action.clone(),
                PreparedNormalCleanupAction::InvokeNominal {
                    cleanup,
                    place,
                    structural_type,
                    path,
                    consumed_claims,
                } => {
                    let machine_ids = calls.machine_ids;
                    let receiver = calls
                        .cleanup_receivers
                        .iter()
                        .find_map(|(source, receiver)| {
                            (*source == cleanup.cleanup_machine).then_some(*receiver)
                        });
                    let mut action = materialize_nominal_cleanup_action(
                        cleanup,
                        *place,
                        *structural_type,
                        path.clone(),
                        machine_ids,
                        receiver,
                        requirement_count(calls.requirement_counts, cleanup.cleanup_machine)?,
                        // Scalar helpers own bounded identity ranges. Use
                        // their allocator rather than an unbounded counter.
                        &mut || calls.allocate_requirement(),
                    )?;
                    action.consumed_claims = consumed_claims
                        .iter()
                        .map(|identity| {
                            crate::terminal_identities::lookup_claim_id(
                                calls.claims.bindings(),
                                *identity,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    terminal_psi::TerminalAffineCleanupAction::InvokeNominal(action)
                }
            })
        })
        .collect()
}

/// A nominal consuming member (a seeded `drop<T>` specialization) emitted the
/// ordinary complete-only body; its return edge still invokes the exact
/// owner-attached `::drop` hook the roster selected for each parameter. A hook
/// that keeps a borrowed subject declares an ordinary structural parameter;
/// its authored parameter name does not select the operand. Retain the
/// selected hook's nominal owner independently of implicit attachment storage,
/// then collect its exact receiver place for the edge's loan.
pub(super) fn finalize_nominal_cleanup_actions(
    checked: &CheckedTrees,
    machines: &mut [TerminalMachine],
    type_ids: &[(String, StructuralTypeId)],
    machine_ids: &[(SymbolHandle, MachineId)],
    receivers: &[(SymbolHandle, PlaceId)],
    requirement_counts: &[(SymbolHandle, usize)],
    next_obligation: &mut u64,
) -> Result<(), LoweringError> {
    for cleanup in selected_cleanups(checked) {
        let Ok(target_id) = lookup_machine_id(machine_ids, cleanup.cleanup_machine) else {
            continue;
        };
        let owner = lookup_type_id(type_ids, &cleanup.type_identity)?;
        let emitted = machines
            .iter_mut()
            .find(|machine| machine.id == target_id)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup target was not emitted",
            ))?;
        // Terminal attachment names semantic ownership, not an implicit
        // argument. A static named subject still carries the same selected
        // T::drop owner while its ordinary parameter carries the storage.
        emitted.attachment = Some(owner);
        emitted.nominal_cleanup_subject = crate::machine_lowering::nominal_cleanup_subject(
            checked,
            cleanup.cleanup_machine,
            type_ids,
        )?;
    }
    for nominal in &checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
    {
        let Ok(terminal_member) = lookup_machine_id(machine_ids, nominal.machine.machine) else {
            continue;
        };
        let member = machines
            .iter_mut()
            .find(|member| member.id == terminal_member)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup member was not emitted in the terminal closure",
            ))?;
        finalize_nominal_cleanup_machine(
            checked,
            nominal,
            member,
            type_ids,
            machine_ids,
            receivers,
            requirement_counts,
            next_obligation,
        )?;
    }
    Ok(())
}

fn finalize_nominal_cleanup_machine(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
    machine: &mut TerminalMachine,
    type_ids: &[(String, StructuralTypeId)],
    machine_ids: &[(SymbolHandle, MachineId)],
    receivers: &[(SymbolHandle, PlaceId)],
    requirement_counts: &[(SymbolHandle, usize)],
    next_obligation: &mut u64,
) -> Result<(), LoweringError> {
    let plan = &nominal.machine;
    contexts::validate_requirements(checked, nominal)?;
    custody::validate_checked_dispositions(checked, nominal)?;
    if let Some(identity) = plan.attachment_type_identity.as_deref() {
        lookup_type_id(type_ids, identity)?;
    }
    // Every Unit return of the dispatcher disposes of each parameter through
    // its selected hook; each edge's actions own their obligation identities.
    let TerminalMachine {
        structural_parameters,
        entry_claims,
        blocks,
        ..
    } = machine;
    for block in blocks.iter_mut() {
        let Terminator::ReturnUnit { edge, .. } = block.terminator else {
            continue;
        };
        let cleanups = nominal_cleanup_return_actions(
            nominal,
            structural_parameters,
            entry_claims,
            machine_ids,
            receivers,
            requirement_counts,
            next_obligation,
        )?;
        block.terminator = Terminator::ReturnUnit {
            edge,
            cleanup_actions: cleanups
                .into_iter()
                .map(terminal_psi::TerminalAffineCleanupAction::InvokeNominal)
                .collect(),
        };
    }
    Ok(())
}

/// One return edge's hook invocations, one per cleanup row, each consuming
/// the entry claims its parameter carries.
fn nominal_cleanup_return_actions(
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
    structural_parameters: &[terminal_psi::StructuralParameterDeclaration],
    entry_claims: &[terminal_psi::EntryClaim],
    machine_ids: &[(SymbolHandle, MachineId)],
    receivers: &[(SymbolHandle, PlaceId)],
    requirement_counts: &[(SymbolHandle, usize)],
    next_obligation: &mut u64,
) -> Result<Vec<NominalAffineCleanup>, LoweringError> {
    let mut cleanups = Vec::with_capacity(nominal.cleanups.len());
    for cleanup in &nominal.cleanups {
        let parameter = structural_parameters
            .get(
                cleanup
                    .source_parameter_index()
                    .ok_or(LoweringError::Unsupported(
                        "nominal parameter cleanup acquired another source",
                    ))? as usize,
            )
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup member parameter is absent from its terminal signature",
            ))?;
        let mut action = lower_nominal_cleanup_action(
            cleanup,
            parameter.place,
            parameter.structural_type,
            Vec::new(),
            machine_ids,
            receivers,
            requirement_counts,
            next_obligation,
        )?;
        action.consumed_claims = entry_claims
            .iter()
            .filter(|claim| claim.input == parameter.place)
            .map(|claim| claim.claim)
            .collect();
        action.consumed_claims.sort_unstable();
        cleanups.push(action);
    }
    Ok(cleanups)
}

pub(crate) fn lower_partial_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    partial: &CheckedPartialAffineUnitCleanupMachinePlan,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    partial::lower_partial_affine_unit_cleanup_machine(checked, partial)
}
