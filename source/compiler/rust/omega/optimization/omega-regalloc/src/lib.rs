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
mod compute;
mod fixed_view_copy_compute;
mod fixed_view_copy_identity;
mod fixed_view_copy_model;
mod fixed_view_copy_validate;
mod home_assignment_compute;
mod home_assignment_identity;
mod home_assignment_model;
mod home_assignment_validate;
mod identity;
mod live_range_compute;
mod live_range_identity;
mod live_range_model;
mod live_range_validate;
mod model;
mod selected_analysis_input;
mod validate;

pub use allocation_legality_identity::terminal_allocation_legality_identity;
pub use allocation_legality_model::*;
pub use allocation_legality_validate::validate_terminal_allocation_legality;
pub use fixed_view_copy_identity::terminal_fixed_view_copy_identity;
pub use fixed_view_copy_model::*;
pub use fixed_view_copy_validate::validate_terminal_fixed_view_copies;
pub use home_assignment_identity::terminal_register_home_identity;
pub use home_assignment_model::*;
pub use home_assignment_validate::validate_terminal_register_homes;
pub use identity::terminal_liveness_identity;
pub use live_range_identity::terminal_live_range_identity;
pub use live_range_model::*;
pub use live_range_validate::validate_terminal_live_ranges;
pub use model::*;
pub use omega_register_model::*;
pub use selected_analysis_input::ValidatedTerminalSelectedAnalysis;
pub use validate::validate_terminal_liveness;

use omega_terminal_target_operations_to_selected_instructions::ValidatedTerminalSelectedInstructions;

/// Compute and then independently replay the bounded selected-CFG liveness
/// analysis. The result grants no interval, allocation, emission, or
/// publication authority.
pub fn analyze_terminal_liveness<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
) -> Result<ValidatedTerminalLiveness, TerminalLivenessError> {
    let plan = compute::compute_terminal_liveness(selected)?;
    validate_terminal_liveness(selected, plan)
}

/// Derive bounded block-local live-range fragments and virtual-register
/// interference from one exact validated selected CFG and its validated
/// liveness facts. The result grants no allocation, emission, or publication
/// authority.
pub fn analyze_terminal_live_ranges<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedTerminalLiveness,
) -> Result<ValidatedTerminalLiveRanges, TerminalLiveRangeError> {
    live_range_validate::revalidate_liveness_custody(selected, liveness)?;
    let plan = live_range_compute::compute_terminal_live_ranges(selected, liveness)?;
    validate_terminal_live_ranges(selected, liveness, plan)
}

/// Derive exact per-point physical-view candidates and incompatible entry to
/// operand fixed-view transition requirements. This grants no home assignment
/// or copy-insertion authority.
pub fn analyze_terminal_allocation_legality(
    ranges: &ValidatedTerminalLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<ValidatedTerminalAllocationLegality, TerminalAllocationLegalityError> {
    let plan = allocation_legality_compute::compute_terminal_allocation_legality(
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    validate_terminal_allocation_legality(
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}

/// Materialize the exact named leaf-local fixed-view copy policy and then
/// independently reconstruct its complete selected CFG. The result grants no
/// allocation, emission, or publication authority.
#[allow(clippy::too_many_arguments)]
pub fn materialize_terminal_fixed_view_copies(
    selected: &ValidatedTerminalSelectedInstructions,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: TerminalFixedViewCopyPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedTerminalFixedViewCopies, TerminalFixedViewCopyError> {
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
    validate_terminal_fixed_view_copies(
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
pub fn assign_terminal_register_homes(
    legality: &ValidatedTerminalAllocationLegality,
    ranges: &ValidatedTerminalLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<ValidatedTerminalRegisterHomes, TerminalRegisterHomeError> {
    let plan = home_assignment_compute::compute_terminal_register_homes(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    validate_terminal_register_homes(
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
