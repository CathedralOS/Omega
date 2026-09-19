use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use selected_instructions::{SelectedConstraintKeys, ValidatedMachineEffectCatalog};

use crate::{
    LiteralFoldError, ValidatedAllocationLegality, ValidatedAllocatorAvailability,
    ValidatedLiveRanges, ValidatedRecoveryClassifications, ValidatedSelectedAnalysis,
    ValidatedSpillChoices,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_literal_fold_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    recovery: &ValidatedRecoveryClassifications,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    effect_catalog: &ValidatedMachineEffectCatalog,
) -> Result<(), LiteralFoldError> {
    if ranges.receipt().selected() != selected.selected_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != register_environment
        || legality.receipt().allocator_availability() != availability.receipt().identity()
        || availability.receipt().register_environment() != register_environment
        || availability.receipt().physical() != physical.identity()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != selected.selected_plan().target
        || target_register_environment_identity(
            selected.selected_plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        ) != register_environment
        || spill_choices.receipt().ranges() != ranges.receipt().identity()
        || spill_choices.receipt().legality() != legality.receipt().identity()
        || spill_choices.receipt().register_environment() != register_environment
        || spill_choices.receipt().allocator_availability() != availability.receipt().identity()
        || recovery.receipt().selected() != selected.selected_identity()
        || recovery.receipt().ranges() != ranges.receipt().identity()
        || recovery.receipt().legality() != legality.receipt().identity()
        || recovery.receipt().spill_choices() != spill_choices.receipt().identity()
        || recovery.receipt().register_environment() != register_environment
        || recovery.receipt().allocator_availability() != availability.receipt().identity()
        || recovery.receipt().optimization_unit() != selected.optimization_unit_identity()
        || recovery.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || selected.selected_plan().functions.len() != recovery.plan().functions.len()
        // The validator re-derives the machine-effect catalog's binding to
        // this target, constraint catalog, and key inventory rather than
        // trusting the producer's selection of it.
        || effect_catalog.catalog().target != selected.selected_plan().target
        || effect_catalog.catalog().register_constraints != constraints.identity()
        || effect_catalog.catalog().selected_keys != validator_selected_keys(selected_keys)
    {
        return Err(LiteralFoldError::RootMismatch);
    }
    Ok(())
}

/// The validator's own projection of `selected_keys` into the
/// selected-instruction key inventory.
fn validator_selected_keys(
    keys: &TargetRegisterEnvironmentConstraintKeys,
) -> SelectedConstraintKeys {
    SelectedConstraintKeys {
        hosted_write_byte_i32: keys.hosted_write_byte_i32,
        hosted_read_byte: keys.hosted_read_byte,
        hosted_exit_process_i32: keys.hosted_exit_process_i32,
        load64: keys.load64,
        load_packed: keys.load_packed,
        store_packed: keys.store_packed,
        load8: keys.load8,
        load16: keys.load16,
        load32: keys.load32,
        load8_indexed: keys.load8_indexed,
        copy_bytes: keys.copy_bytes,
        store: keys.store,
        address_offset: keys.address_offset,
        store64: keys.store64,
        frame_address: keys.frame_address,
        call_unit: keys.call_unit.clone(),
        call_unit_mixed: keys.call_unit_mixed.clone(),
        call_scalar: keys.call_scalar.clone(),
        call_aggregate: keys.call_aggregate.clone(),
        call_normalized_foreign: keys.call_normalized_foreign.clone(),
        return_aggregate: keys.return_aggregate.clone(),
        materialize_i64: keys.materialize_i64,
        materialize_boolean: keys.materialize_boolean,
        copy_i64: keys.copy_i64,
        float32_to_bits: keys.float32_to_bits,
        float64_to_bits: keys.float64_to_bits,
        bits_to_float32: keys.bits_to_float32,
        bits_to_float64: keys.bits_to_float64,
        add_i64: keys.add_i64,
        subtract_i64: keys.subtract_i64,
        multiply_i64: keys.multiply_i64,
        saturating_subtract_unsigned: keys.saturating_subtract_unsigned,
        saturating_add_u64: keys.saturating_add_u64,
        divide_u64: keys.divide_u64,
        remainder_u64: keys.remainder_u64,
        remainder_i64: keys.remainder_i64,
        divide_i64: keys.divide_i64,
        saturating_add_clamped: keys.saturating_add_clamped,
        saturating_subtract_clamped: keys.saturating_subtract_clamped,
        saturating_divide_signed: keys.saturating_divide_signed,
        add_i64_immediate: keys.add_i64_immediate,
        subtract_i64_immediate: keys.subtract_i64_immediate,
        compare_i64_zero: keys.compare_i64_zero,
        compare_i64: keys.compare_i64,
        compare_i64_immediate: keys.compare_i64_immediate,
        conditional_branch: keys.conditional_branch,
        jump: keys.jump,
        return_float: keys.return_float.clone(),
        return_i64: keys.return_i64,
        return_unit: keys.return_unit,
    }
}
