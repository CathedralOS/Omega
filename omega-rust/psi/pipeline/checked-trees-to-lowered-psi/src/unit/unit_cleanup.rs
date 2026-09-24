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
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralParameterPlan};
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

pub(super) fn patch_nominal_cleanup_member(
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
        if cleanup_target.state != cleanup.cleanup_state
            || cleanup_target.contract_report_fingerprint
                != cleanup.cleanup_contract_report_fingerprint
            || cleanup_target.attachment_type_identity.as_deref()
                != Some(cleanup.type_identity.as_str())
            || lookup_type_id(type_ids, &cleanup.type_identity)? != parameter.structural_type
        {
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
