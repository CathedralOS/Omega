//! Borrowed establishment checks supply before exposing an external pointer.
//!
//! The exact source/profile occurrence join is owned by provider admission.
//! This emitter reads only those descriptors, never the represented content.
//! It publishes a pointer only in Ready; later accessors perform each declared
//! transfer. Rejection has no backing effect and carries no storage ownership.

use super::{
    Builder, ImageHome, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, TerminalPlacementLifecyclePlan, failure, fragmented_scalar,
    operand_indices, outcome, owner_word, resource_profile, result_layout,
};
use crate::SelectedInstructionError;

pub(super) fn emit(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    plan: &TerminalPlacementLifecyclePlan,
) -> Result<(), SelectedInstructionError> {
    use super::super::super::operation_control;
    use super::super::{leaf_copy, record};
    let LegalizedScalarInstructionKind::PlacementLifecycle {
        operands: abstract_operations::PlacementLifecycleOperands::View { storage, profile },
        indices,
        result,
        ..
    } = &row.kind
    else {
        return Err(failure(
            "borrowed placement emitter lost its exact operands",
        ));
    };
    let terminal_psi::TerminalPlacementLifecycleExecution::CompilerPrimitive {
        profile_type: Some(profile_type),
        ..
    } = plan.execution
    else {
        return Err(failure("borrowed placement lost its original profile type"));
    };
    let types = &source
        .structural
        .as_ref()
        .ok_or_else(|| failure("borrowed placement has no structural signature"))?
        .structural_types;
    let layout = result_layout::ResultLayout::derive(types, result.structural_type, plan)?;
    if !matches!(layout.returned, result_layout::ReturnedLayout::Borrowed)
        || layout.ready_owner.shape.byte_size != 8
    {
        return Err(failure(
            "borrowed placement result changed its reference or empty rejection",
        ));
    }
    let mut remaining = indices.as_slice();
    let storage_indices = operand_indices(storage, &mut remaining)?;
    let profile_indices = operand_indices(profile, &mut remaining)?;
    if !remaining.is_empty() {
        return Err(failure(
            "borrowed placement retained unrelated operand selectors",
        ));
    }
    let projected = leaf_copy::project_operand(source, row, builder, storage, storage_indices)?;
    let owner_home = ImageHome {
        storage: fragmented_scalar::StorageAccess::Place(projected.indices),
        place: storage.place,
        pointer: projected
            .pointer
            .ok_or_else(|| failure("borrowed extent has no descriptor home"))?,
        byte_offset: projected.byte_offset,
    };
    let base = owner_word(types, row, builder, plan, owner_home, plan.storage.base)?;
    let available = owner_word(types, row, builder, plan, owner_home, plan.storage.length)?;
    let projected = leaf_copy::project_operand(source, row, builder, profile, profile_indices)?;
    let profile_home = ImageHome {
        storage: fragmented_scalar::StorageAccess::Place(projected.indices),
        place: profile.place,
        pointer: projected
            .pointer
            .ok_or_else(|| failure("borrowed profile has no original home"))?,
        byte_offset: projected.byte_offset,
    };
    let pointer = record::initialize_storage(row, builder, result.place, layout.shape)?;
    let home = ImageHome {
        storage: fragmented_scalar::StorageAccess::Place(&[]),
        place: result.place,
        pointer,
        byte_offset: 0,
    };
    let scratch = ImageHome::scratch(
        builder,
        row,
        result.place,
        resource_profile::compatibility::COMPATIBILITY_SCRATCH_BYTES,
    )?;
    let assessment = resource_profile::compatibility::assess(
        types,
        profile_type,
        builder,
        row,
        profile_home,
        scratch,
        available,
        &plan.placement,
        plan.operation,
    )?;
    operation_control::require_nonzero(builder, row, assessment.well_formed)?;
    let required = plan
        .placement
        .layout()
        .size
        .ok_or_else(|| failure("borrowed placement requires fixed view geometry"))?;
    let decision = outcome::decide(
        builder,
        row,
        result.place,
        base,
        available,
        required,
        assessment.modulus,
        assessment.residue,
        assessment.compatible,
    )?;
    let required = fragmented_scalar::constant(builder, row, result.place, required)?;
    outcome::write_failure(
        builder,
        row,
        home,
        &layout,
        &decision,
        [required, available],
        [assessment.modulus, assessment.residue],
    )?;
    operation_control::when_nonzero(builder, row, decision.ready, |builder| {
        outcome::write_word(builder, row, home, layout.ready_owner.byte_offset, 8, base)
    })?;
    outcome::write_word(builder, row, home, layout.tag, 4, decision.outcome_tag)?;
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}
