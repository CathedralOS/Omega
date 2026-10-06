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
    CheckedUnitStructuralTypeShape, LoweredPsi, LoweringError, MachineId, NominalAffineCleanup,
    PlaceId, PrimitiveType, Proposition, ScalarTerm, ScalarType, StructuralFieldType,
    StructuralTypeId, StructuralTypeShape, TerminalMachine, Terminator, lookup_machine_id,
    lookup_type_id, lower_unit_closure, obligation_id, unique_unit_machine, unsupported,
};
use crate::unit::attached_unit::bodies::UnitPlans;
use checked_trees::CheckedUnitNominalAffineCleanupPlan;
use symbols::SymbolHandle;
pub(crate) mod contexts;
mod custody;
mod ordered;
mod partial;
use ordered::lower_ordered_nominal_affine_unit_cleanup_machine;
pub(crate) use partial::checked_partial_affine_residuals;

/// Lower a nominal affine Unit cleanup machine. One route serves any number
/// of structural parameters: each root owns exactly one cleanup action, and
/// the actions run in reverse declaration order (`ordered`).
pub(crate) fn lower_nominal_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
) -> Result<LoweredPsi, LoweringError> {
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
            _ => None,
        })
        .flatten()
        .filter_map(checked_trees::CheckedNormalCleanupAction::nominal)
}

pub(crate) fn computation_cleanups(
    checked: &CheckedTrees,
    machine: SymbolHandle,
) -> Result<Vec<&CheckedUnitNominalAffineCleanupPlan>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    let mut cleanups = Vec::new();
    for identity in
        crate::expression_preparation::computation_graph::reachable_nodes(checked, &roots)?
    {
        let retained = match plans.nodes.get(identity).kind {
            checked_trees::CheckedScalarComputationKind::Call {
                temporary_cleanups, ..
            }
            | checked_trees::CheckedScalarComputationKind::BoundaryCall {
                temporary_cleanups,
                ..
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
        .chain(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .iter()
                .flat_map(|machine| &machine.states)
                .flat_map(|state| continuation_cleanups(&state.operations)),
        )
        .chain(
            checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .machines
                .iter()
                .flat_map(|plan| &plan.cleanup_actions)
                .filter_map(|action| match action {
                    checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) => {
                        Some(cleanup)
                    }
                    checked_trees::CheckedNormalCleanupAction::Discard(_) => None,
                }),
        )
        .chain(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .iter()
                .flat_map(|machine| &machine.states)
                .flat_map(|state| &state.normal_exit_cleanups)
                .flat_map(|exit| &exit.actions)
                .filter_map(|action| match action {
                    checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) => {
                        Some(cleanup)
                    }
                    checked_trees::CheckedNormalCleanupAction::Discard(_) => None,
                }),
        )
}

/// Materialize one checked normal-cleanup operation in the shared closure namespace.
/// The caller owns return value custody; this operation only borrows its exact owner.
pub(crate) fn lower_nominal_cleanup_action(
    checked: &CheckedTrees,
    cleanup: &CheckedUnitNominalAffineCleanupPlan,
    place: PlaceId,
    structural_type: StructuralTypeId,
    path: Vec<terminal_psi::StructuralPathSegment>,
    type_ids: &[(String, StructuralTypeId)],
    machine_ids: &[(SymbolHandle, MachineId)],
    receivers: &[(SymbolHandle, PlaceId)],
    requirement_counts: &[(SymbolHandle, usize)],
    next_obligation: &mut u64,
) -> Result<NominalAffineCleanup, LoweringError> {
    unique_unit_machine(
        UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
        cleanup.cleanup_machine,
    )?;
    if lookup_type_id(type_ids, &cleanup.type_identity)? != structural_type {
        return unsupported("nominal cleanup receiver type differs from its checked subject");
    }
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
                } => {
                    let machine_ids = calls.machine_ids;
                    let receiver = calls
                        .cleanup_receivers
                        .iter()
                        .find_map(|(source, receiver)| {
                            (*source == cleanup.cleanup_machine).then_some(*receiver)
                        });
                    terminal_psi::TerminalAffineCleanupAction::InvokeNominal(
                        materialize_nominal_cleanup_action(
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
                        )?,
                    )
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
        unique_unit_machine(
            UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
            cleanup.cleanup_machine,
        )?;
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
            emitted.attachment,
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
    let mut cleanups = Vec::with_capacity(nominal.cleanups.len());
    for cleanup in &nominal.cleanups {
        let parameter = machine
            .structural_parameters
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
            checked,
            cleanup,
            parameter.place,
            parameter.structural_type,
            Vec::new(),
            type_ids,
            machine_ids,
            receivers,
            requirement_counts,
            next_obligation,
        )?;
        action.consumed_claims = machine
            .entry_claims
            .iter()
            .filter(|claim| claim.input == parameter.place)
            .map(|claim| claim.claim)
            .collect();
        action.consumed_claims.sort_unstable();
        cleanups.push(action);
    }
    let [block] = machine.blocks.as_mut_slice() else {
        return unsupported("nominal affine Unit member terminal control drifted");
    };
    let edge = block.terminator.edge();
    block.terminator = Terminator::ReturnUnit {
        edge,
        cleanup_actions: cleanups
            .into_iter()
            .map(terminal_psi::TerminalAffineCleanupAction::InvokeNominal)
            .collect(),
    };
    Ok(())
}

fn is_bounded_nominal_cleanup_record(shape: &CheckedUnitStructuralTypeShape) -> bool {
    match shape {
        CheckedUnitStructuralTypeShape::Record { fields } => fields.iter().all(|field| {
            !field.relevance.is_erased()
                && matches!(
                    &field.field_type,
                    CheckedUnitStructuralFieldType::Scalar(
                        PrimitiveType::Bool
                            | PrimitiveType::I8
                            | PrimitiveType::I16
                            | PrimitiveType::I32
                            | PrimitiveType::I64
                            | PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                            | PrimitiveType::Addr
                    )
                )
        }),
        CheckedUnitStructuralTypeShape::Reference { .. }
        | CheckedUnitStructuralTypeShape::FusedServiceBinding
        | CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
        | CheckedUnitStructuralTypeShape::ByteSequence(_)
        | CheckedUnitStructuralTypeShape::FixedArray { .. }
        | CheckedUnitStructuralTypeShape::BorrowedSliceView { .. }
        | CheckedUnitStructuralTypeShape::Sum { .. }
        | CheckedUnitStructuralTypeShape::Mixed { .. } => false,
    }
}

pub(crate) fn lower_partial_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    partial: &CheckedPartialAffineUnitCleanupMachinePlan,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    partial::lower_partial_affine_unit_cleanup_machine(checked, partial)
}
