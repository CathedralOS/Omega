#![forbid(unsafe_code)]

//! Register-allocation subsystem bring-up.
//!
//! The declarative physical-register and instruction-constraint model is owned
//! by `omega-register-model` and remains re-exported for compatibility. The
//! first production analysis computes and independently validates bounded
//! selected-instruction liveness. It performs no allocation.

mod compute;
mod identity;
mod model;
mod validate;

pub use identity::terminal_liveness_identity;
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
