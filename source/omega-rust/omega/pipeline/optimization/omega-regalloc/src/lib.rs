#![forbid(unsafe_code)]

//! Register-allocation and exact machine-lowering optimization entrance.
//!
//! The declarative physical-register and instruction-constraint model is owned
//! by `omega-register-model` and remains re-exported for compatibility. The
//! entrance is organized by read-only analyses, allocation decisions, and
//! explicit independently validated rules.

mod allocation;
mod analyses;
mod rules;

pub use allocation::*;
pub use analyses::*;
pub use omega_register_model::*;
pub use rules::*;

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
