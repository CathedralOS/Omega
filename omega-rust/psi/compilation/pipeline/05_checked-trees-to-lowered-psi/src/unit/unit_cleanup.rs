//! Structural Unit cleanup lowering.
//!
//! Nominal cleanup lowers through one route for one or more roots
//! (`ordered`); partial-affine cleanup lives in `partial`.
//! Build cleanup requirements and their obligation identities here, but leave
//! certificates to final operation proof emission. Owned-field requirements are
//! validity-scoped observations, not permanent assumption slots; their available
//! premises are known only after the complete caller and cleanup edge exist.

use super::{
    BTreeSet, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedTrees, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape, LoweredPsi, LoweringError,
    MachineId, Multiplicity, NominalAffineCleanup, PlaceId, PrimitiveType, Proposition, ScalarTerm,
    ScalarType, ServiceReachInterface, ServiceReachPlan, ServiceReachSummary, StructuralFieldType,
    StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalMachineResult, Terminator,
    checked_unit_call_closure_including, dense_identity, lookup_machine_id, lookup_type_id,
    lower_unit_closure, machine_id, obligation_id, place_id, unique_unit_machine, unsupported,
};
use crate::unit::attached_unit::bodies::UnitPlans;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitEffectMachinePlan, CheckedUnitNominalAffineCleanupPlan,
};
use symbols::SymbolHandle;
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

/// A nominal consuming member (a seeded `drop<T>` specialization) emitted the
/// ordinary complete-only body; its return edge still invokes the exact
/// owner-attached `::drop` hook the roster selected for each parameter. A hook
/// that keeps a borrowed subject declares an ordinary structural parameter;
/// its name and legacy `self` spelling do not select the operand. Retain the
/// selected hook's nominal owner independently of implicit attachment storage,
/// then collect its exact receiver place for the edge's loan.
pub(super) fn patch_nominal_cleanup_members(
    checked: &CheckedTrees,
    entry: SymbolHandle,
    machines: &mut [TerminalMachine],
    type_ids: &[(String, StructuralTypeId)],
    machine_ids: &[(SymbolHandle, MachineId)],
) -> Result<(), LoweringError> {
    for nominal in &checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
    {
        for cleanup in &nominal.cleanups {
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
    }
    let cleanup_receivers: std::collections::BTreeMap<MachineId, PlaceId> = machines
        .iter()
        .filter_map(|machine| {
            machine
                .structural_parameters
                .iter()
                .find(|parameter| {
                    matches!(
                        parameter.access,
                        terminal_psi::StructuralAccess::MutableBorrow
                            | terminal_psi::StructuralAccess::WriteOnlyBorrow
                    )
                })
                .map(|parameter| (machine.id, parameter.place))
        })
        .collect();
    for nominal in &checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
    {
        if nominal.machine.machine == entry || nominal.machine.attachment_type_identity.is_some() {
            continue;
        }
        let Ok(terminal_member) = lookup_machine_id(machine_ids, nominal.machine.machine) else {
            continue;
        };
        let member = machines
            .iter_mut()
            .find(|member| member.id == terminal_member)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup member was not emitted in the terminal closure",
            ))?;
        patch_nominal_cleanup_member(
            checked,
            nominal,
            member,
            type_ids,
            machine_ids,
            &cleanup_receivers,
        )?;
    }
    Ok(())
}

fn patch_nominal_cleanup_member(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
    machine: &mut TerminalMachine,
    type_ids: &[(String, StructuralTypeId)],
    machine_ids: &[(SymbolHandle, MachineId)],
    cleanup_receivers: &std::collections::BTreeMap<MachineId, PlaceId>,
) -> Result<(), LoweringError> {
    let plan = &nominal.machine;
    if plan.attachment_type_identity.is_some()
        || !nominal.caller_requirements.is_empty()
        || nominal
            .cleanups
            .iter()
            .any(|cleanup| !cleanup.requirements.is_empty())
    {
        return unsupported("member nominal affine Unit cleanup requires the isolated entry lane");
    }
    if machine.attachment.is_some() {
        return unsupported("nominal affine Unit member is unexpectedly attached");
    }
    let mut cleanups = Vec::with_capacity(nominal.cleanups.len());
    for cleanup in &nominal.cleanups {
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.position == cleanup.source_parameter_index)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup member parameter is absent from its terminal signature",
            ))?;
        let cleanup_target = unique_unit_machine(
            UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
            cleanup.cleanup_machine,
        )?;
        validate_nominal_cleanup_target(cleanup_target, cleanup)?;
        if lookup_type_id(type_ids, &cleanup.type_identity)? != parameter.structural_type {
            return unsupported("nominal cleanup member target identity drifted");
        }
        let hook_machine = lookup_machine_id(machine_ids, cleanup.cleanup_machine)?;
        cleanups.push(NominalAffineCleanup {
            place: parameter.place,
            structural_type: parameter.structural_type,
            cleanup_machine: hook_machine,
            cleanup_receiver: cleanup_receivers.get(&hook_machine).copied(),
            requirement_obligations: Vec::new(),
        });
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
