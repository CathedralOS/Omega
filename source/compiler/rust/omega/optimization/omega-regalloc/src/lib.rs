#![forbid(unsafe_code)]

//! Compatibility facade for the register-allocation subsystem.
//!
//! The declarative physical-register and instruction-constraint model is owned
//! by `omega-register-model`. This crate deliberately performs no allocation
//! yet and re-exports that vocabulary so existing consumers do not have to
//! migrate atomically.

pub use omega_register_model::*;

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
