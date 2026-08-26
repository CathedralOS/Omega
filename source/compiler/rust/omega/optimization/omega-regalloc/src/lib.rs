#![forbid(unsafe_code)]

//! Register-allocation subsystem bring-up.
//!
//! The declarative physical-register and instruction-constraint model is owned
//! by `omega-register-model` and remains re-exported for compatibility. The
//! production analyses compute and independently validate bounded
//! selected-instruction liveness, block-local live-range fragments, and
//! virtual-register interference. They perform no allocation.

mod compute;
mod identity;
mod live_range_compute;
mod live_range_identity;
mod live_range_model;
mod live_range_validate;
mod model;
mod validate;

pub use identity::terminal_liveness_identity;
pub use live_range_identity::terminal_live_range_identity;
pub use live_range_model::*;
pub use live_range_validate::validate_terminal_live_ranges;
pub use model::*;
pub use omega_register_model::*;
pub use validate::validate_terminal_liveness;

use omega_terminal_target_operations_to_selected_instructions::ValidatedTerminalSelectedInstructions;

/// Compute and then independently replay the bounded selected-CFG liveness
/// analysis. The result grants no interval, allocation, emission, or
/// publication authority.
pub fn analyze_terminal_liveness(
    selected: &ValidatedTerminalSelectedInstructions,
) -> Result<ValidatedTerminalLiveness, TerminalLivenessError> {
    let plan = compute::compute_terminal_liveness(selected)?;
    validate_terminal_liveness(selected, plan)
}

/// Derive bounded block-local live-range fragments and virtual-register
/// interference from one exact validated selected CFG and its validated
/// liveness facts. The result grants no allocation, emission, or publication
/// authority.
pub fn analyze_terminal_live_ranges(
    selected: &ValidatedTerminalSelectedInstructions,
    liveness: &ValidatedTerminalLiveness,
) -> Result<ValidatedTerminalLiveRanges, TerminalLiveRangeError> {
    live_range_validate::revalidate_liveness_custody(selected, liveness)?;
    let plan = live_range_compute::compute_terminal_live_ranges(selected, liveness)?;
    validate_terminal_live_ranges(selected, liveness, plan)
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
