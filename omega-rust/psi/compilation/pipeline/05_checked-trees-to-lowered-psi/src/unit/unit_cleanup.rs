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
    CheckedUnitStructuralTypeShape, LoweredPsi, LoweringError, MachineId, Multiplicity,
    NominalAffineCleanup, PlaceId, PrimitiveType, Proposition, ScalarTerm, ScalarType,
    StructuralFieldType, StructuralTypeId, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, Terminator, lookup_machine_id, lookup_type_id, lower_unit_closure,
    obligation_id, unique_unit_machine, unsupported,
};
use crate::unit::attached_unit::bodies::UnitPlans;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitEffectMachinePlan, CheckedUnitNominalAffineCleanupPlan,
};
use symbols::SymbolHandle;
pub(crate) mod contexts;
mod ordered;
mod partial;
use ordered::lower_ordered_nominal_affine_unit_cleanup_machine;
pub(crate) use partial::{
    checked_partial_affine_residuals, validate_anonymous_partial_permissions,
};

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
                .flow
                .terminal_structural_scalar_returns
                .machines
                .iter()
                .flat_map(|plan| &plan.cleanup_actions)
                .filter_map(|action| match action {
                    checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                        cleanup,
                    ) => Some(cleanup),
                    checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(_) => {
                        None
                    }
                }),
        )
}

/// Materialize one checked normal-cleanup operation in the shared closure namespace.
/// The caller owns return value custody; this operation only borrows its exact owner.
pub(crate) fn lower_nominal_cleanup_action(
    checked: &CheckedTrees,
    cleanup: &CheckedUnitNominalAffineCleanupPlan,
    parameter: &terminal_psi::StructuralParameterDeclaration,
    type_ids: &[(String, StructuralTypeId)],
    machine_ids: &[(SymbolHandle, MachineId)],
    receivers: &[(SymbolHandle, PlaceId)],
    next_obligation: &mut u64,
) -> Result<NominalAffineCleanup, LoweringError> {
    let target = unique_unit_machine(
        UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
        cleanup.cleanup_machine,
    )?;
    validate_nominal_cleanup_target(target, cleanup)?;
    if lookup_type_id(type_ids, &cleanup.type_identity)? != parameter.structural_type {
        return unsupported("nominal cleanup member target identity drifted");
    }
    Ok(NominalAffineCleanup {
        place: parameter.place,
        structural_type: parameter.structural_type,
        cleanup_machine: lookup_machine_id(machine_ids, cleanup.cleanup_machine)?,
        cleanup_receiver: receivers.iter().find_map(|(source, receiver)| {
            (*source == cleanup.cleanup_machine).then_some(*receiver)
        }),
        requirement_obligations: cleanup
            .requirements
            .iter()
            .map(|_| crate::terminal_identities::allocate_dense(next_obligation).map(obligation_id))
            .collect::<Result<Vec<_>, _>>()?,
    })
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
    next_obligation: &mut u64,
) -> Result<(), LoweringError> {
    for cleanup in selected_cleanups(checked) {
        let Ok(target_id) = lookup_machine_id(machine_ids, cleanup.cleanup_machine) else {
            continue;
        };
        let target = unique_unit_machine(
            UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
            cleanup.cleanup_machine,
        )?;
        validate_nominal_cleanup_target(target, cleanup)?;
        let owner = lookup_type_id(type_ids, &cleanup.type_identity)?;
        let emitted = machines
            .iter_mut()
            .find(|machine| machine.id == target_id)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup target was not emitted",
            ))?;
        if emitted
            .attachment
            .is_some_and(|attachment| attachment != owner)
        {
            return unsupported("nominal cleanup target owner identity drifted");
        }
        // Terminal attachment names semantic ownership, not an implicit
        // argument. A static named subject still carries the same selected
        // T::drop owner while its ordinary parameter carries the storage.
        emitted.attachment = Some(owner);
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
    next_obligation: &mut u64,
) -> Result<(), LoweringError> {
    let plan = &nominal.machine;
    contexts::validate_requirements(checked, nominal)?;
    let expected_attachment = plan
        .attachment_type_identity
        .as_deref()
        .map(|identity| lookup_type_id(type_ids, identity))
        .transpose()?;
    if machine.attachment != expected_attachment {
        return unsupported("nominal affine Unit member attachment drifted");
    }
    if nominal.cleanups.len() != plan.structural_parameters.len()
        || machine.structural_parameters.len() != plan.structural_parameters.len()
        || !machine.parameters.is_empty()
        || machine.result != TerminalMachineResult::Unit
    {
        return unsupported("nominal affine Unit member signature drifted");
    }
    let mut cleanups = Vec::with_capacity(nominal.cleanups.len());
    for (ordinal, cleanup) in nominal.cleanups.iter().enumerate() {
        if cleanup.source_parameter_index as usize != nominal.cleanups.len() - ordinal - 1 {
            return unsupported("nominal cleanup action order drifted");
        }
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.position == cleanup.source_parameter_index)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup member parameter is absent from its terminal signature",
            ))?;
        cleanups.push(lower_nominal_cleanup_action(
            checked,
            cleanup,
            parameter,
            type_ids,
            machine_ids,
            receivers,
            next_obligation,
        )?);
    }
    let entry_block = machine.entry;
    let [block] = machine.blocks.as_mut_slice() else {
        return unsupported("nominal affine Unit member terminal control drifted");
    };
    if !matches!(
        &block.terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ) || block.id != entry_block
        || !block.parameters.is_empty()
        || !block.operations.is_empty()
    {
        return unsupported("nominal affine Unit member body or return drifted");
    }
    let edge = block.terminator.edge();
    block.terminator = Terminator::ReturnUnitNominalAffine { edge, cleanups };
    Ok(())
}

fn validate_nominal_cleanup_target(
    target: &CheckedUnitEffectMachinePlan,
    cleanup: &CheckedUnitNominalAffineCleanupPlan,
) -> Result<(), LoweringError> {
    if target.state != cleanup.cleanup_state
        || target.contract_report_fingerprint != cleanup.cleanup_contract_report_fingerprint
        || target
            .attachment_type_identity
            .as_deref()
            .is_some_and(|owner| owner != cleanup.type_identity)
        || !target.scalar_parameters.is_empty()
        || target.structural_parameters.len() > 1
        || target.structural_parameters.iter().any(|parameter| {
            parameter.position != 0
                || parameter.type_identity != cleanup.type_identity
                || !matches!(
                    parameter.access,
                    CheckedStructuralAccess::MutableBorrow
                        | CheckedStructuralAccess::WriteOnlyBorrow
                )
        })
    {
        return unsupported("nominal cleanup target selection or borrowed subject drifted");
    }
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
