#![forbid(unsafe_code)]

//! Register-allocation subsystem bring-up.
//!
//! The declarative physical-register and instruction-constraint model is owned
//! by `omega-register-model` and remains re-exported for compatibility. The
//! production analyses compute and independently validate bounded
//! selected-instruction liveness, block-local live-range fragments, and
//! virtual-register interference. A later bounded lane assigns deterministic
//! physical homes only for transition-free, spill-free inputs.

mod allocation_legality_compute;
mod allocation_legality_identity;
mod allocation_legality_model;
mod allocation_legality_validate;
mod allocator_availability_compute;
mod allocator_availability_identity;
mod allocator_availability_model;
mod allocator_availability_validate;
mod compute;
mod fixed_view_copy_codec;
mod fixed_view_copy_compute;
mod fixed_view_copy_identity;
mod fixed_view_copy_model;
mod fixed_view_copy_validate;
mod home_assignment_compute;
mod home_assignment_identity;
mod home_assignment_model;
mod home_assignment_validate;
mod identity;
mod literal_fold_compute;
mod literal_fold_identity;
mod literal_fold_model;
mod literal_fold_transform;
mod literal_fold_validate;
mod live_range_compute;
mod live_range_identity;
mod live_range_model;
mod live_range_validate;
mod model;
mod post_allocation_manifest;
mod pressure_rematerialization_compute;
mod pressure_rematerialization_identity;
mod pressure_rematerialization_model;
mod pressure_rematerialization_validate;
mod recovery_classification_compute;
mod recovery_classification_identity;
mod recovery_classification_model;
mod recovery_classification_validate;
mod selected_analysis_input;
mod spill_choice_compute;
mod spill_choice_identity;
mod spill_choice_model;
mod spill_choice_validate;
mod validate;

pub use allocation_legality_identity::allocation_legality_identity;
pub use allocation_legality_model::*;
pub use allocation_legality_validate::validate_allocation_legality;
pub use allocator_availability_identity::allocator_availability_identity;
pub use allocator_availability_model::*;
pub use allocator_availability_validate::validate_allocator_availability;
pub use fixed_view_copy_identity::fixed_view_copy_identity;
pub use fixed_view_copy_model::*;
pub use fixed_view_copy_validate::validate_fixed_view_copies;
pub use home_assignment_identity::register_home_identity;
pub use home_assignment_model::*;
pub use home_assignment_validate::validate_register_homes;
pub use identity::liveness_identity;
pub use literal_fold_identity::literal_fold_identity;
pub use literal_fold_model::*;
pub use literal_fold_validate::validate_literal_fold;
pub use live_range_identity::live_range_identity;
pub use live_range_model::*;
pub use live_range_validate::validate_live_ranges;
pub use model::*;
pub use omega_register_model::*;
pub use post_allocation_manifest::*;
pub use pressure_rematerialization_identity::pressure_rematerialization_identity;
pub use pressure_rematerialization_model::*;
pub use pressure_rematerialization_validate::validate_pressure_rematerialization;
pub use recovery_classification_identity::recovery_classification_identity;
pub use recovery_classification_model::*;
pub use recovery_classification_validate::validate_recovery_classifications;
pub use selected_analysis_input::ValidatedSelectedAnalysis;
pub use spill_choice_identity::spill_choice_identity;
pub use spill_choice_model::*;
pub use spill_choice_validate::validate_spill_choices;
pub use validate::validate_liveness;

use omega_target_operations_to_selected_instructions::ValidatedSelectedInstructions;

/// Materialize and independently replay one exact named policy controlling
/// unconstrained physical-view availability. This is allocator input only; it
/// grants no fixed-operand override, home assignment, or emission authority.
pub fn materialize_allocator_availability(
    register_environment: TargetRegisterEnvironmentIdentity,
    target: omega_target::NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: AllocatorAvailabilityPolicy,
) -> Result<ValidatedAllocatorAvailability, AllocatorAvailabilityError> {
    let plan = allocator_availability_compute::compute_terminal_allocator_availability(
        register_environment,
        target,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
    )?;
    validate_allocator_availability(
        register_environment,
        target,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}

/// Compute and then independently replay the bounded selected-CFG liveness
/// analysis. The result grants no interval, allocation, emission, or
/// publication authority.
pub fn analyze_liveness<S: ValidatedSelectedAnalysis>(
    selected: &S,
) -> Result<ValidatedLiveness, LivenessError> {
    let plan = compute::compute_terminal_liveness(selected)?;
    validate_liveness(selected, plan)
}

/// Derive bounded block-local live-range fragments and virtual-register
/// interference from one exact validated selected CFG and its validated
/// liveness facts. The result grants no allocation, emission, or publication
/// authority.
pub fn analyze_live_ranges<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    live_range_validate::revalidate_liveness_custody(selected, liveness)?;
    let plan = live_range_compute::compute_terminal_live_ranges(selected, liveness)?;
    validate_live_ranges(selected, liveness, plan)
}

/// Derive exact per-point physical-view candidates and incompatible entry to
/// operand fixed-view transition requirements. This grants no home assignment
/// or copy-insertion authority.
pub fn analyze_allocation_legality(
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<ValidatedAllocationLegality, AllocationLegalityError> {
    let plan = allocation_legality_compute::compute_terminal_allocation_legality(
        ranges,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    validate_allocation_legality(
        ranges,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}

/// Materialize an explicitly selected, exact fixed-view copy policy and then
/// independently reconstruct its complete selected CFG. No policy is selected
/// implicitly; the result grants no allocation, emission, or publication
/// authority.
#[allow(clippy::too_many_arguments)]
pub fn materialize_fixed_view_copies(
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: FixedViewCopyPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedFixedViewCopies, FixedViewCopyError> {
    let plan = fixed_view_copy_compute::compute_terminal_fixed_view_copies(
        selected,
        ranges,
        legality,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_fixed_view_copies(
        selected,
        ranges,
        legality,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}

/// Assign deterministic physical views for the bounded transition-free lane.
/// Unresolved fixed-view transitions and pressure requiring spills reject.
/// The result grants no instruction-emission or publication authority.
#[allow(clippy::too_many_arguments)]
pub fn assign_register_homes(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
    let plan = home_assignment_compute::compute_terminal_register_homes(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    validate_register_homes(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}

/// Select the deterministic recovery victim at the first supported local
/// pressure point in each function. This deliberately does not materialize a
/// spill, reload, rematerialization, stack slot, frame, or machine instruction.
#[allow(clippy::too_many_arguments)]
pub fn choose_spill_victims(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: SpillChoicePolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedSpillChoices, SpillChoiceError> {
    let plan = spill_choice_compute::compute_terminal_spill_choices(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_spill_choices(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}

/// Classify the already selected pressure victim under one exact recovery
/// eligibility policy. The result is analysis evidence only: it neither picks
/// a recovery strategy nor changes code, fuel, storage, frames, or emission.
pub fn classify_pressure_recovery<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    policy: RecoveryClassificationPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedRecoveryClassifications, RecoveryClassificationError> {
    let plan = recovery_classification_compute::compute_terminal_recovery_classifications(
        selected,
        ranges,
        legality,
        spill_choices,
        policy,
        budget,
    )?;
    validate_recovery_classifications(selected, ranges, legality, spill_choices, plan)
}

/// Fold one already-classified incoming unsigned-12-bit literal into its
/// immediately following exact-add consumer. This is one explicit named
/// transformation, not a generic rematerializer, optimizer level, or loop.
#[allow(clippy::too_many_arguments)]
pub fn fold_selected_incoming_literal<S: ValidatedSelectedAnalysis>(
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
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: LiteralFoldPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedLiteralFold, LiteralFoldError> {
    let plan = literal_fold_compute::compute_terminal_literal_fold(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_literal_fold(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}

/// Insert one value-lineage-only, zero-fuel rematerialization immediately
/// before either the sole future flexible use or the first use of an exact
/// multiple-use suffix of an already-classified active resident. The semantic
/// source materialization and its charge remain intact.
#[allow(clippy::too_many_arguments)]
pub fn rematerialize_selected_active_resident<S: ValidatedSelectedAnalysis>(
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
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: PressureRematerializationPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedPressureRematerialization, PressureRematerializationError> {
    let plan = pressure_rematerialization_compute::compute_terminal_pressure_rematerialization(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_pressure_rematerialization(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_facade_retains_the_register_model_api() {
        let validator: fn(
            PhysicalRegisterModel,
        )
            -> Result<ValidatedPhysicalRegisterModel, RegisterModelValidationError> =
            validate_physical_register_model;
        let catalog_validator: fn(
            RegisterConstraintCatalog,
            &ValidatedPhysicalRegisterModel,
        ) -> Result<
            ValidatedRegisterConstraintCatalog,
            RegisterConstraintCatalogValidationError,
        > = validate_register_constraint_catalog;

        let _ = (validator, catalog_validator);
    }
}
