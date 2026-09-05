#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Register-allocation and exact machine-lowering surfaces.
//!
//! The declarative physical-register and instruction-constraint model is owned
//! by `omega-register-model`. Physical-home data and its canonical codec belong
//! to `omega-register-homes`; both remain re-exported here. This owner computes
//! and independently validates allocations, but decoding home data grants no
//! admission. The entrance separates analyses, allocation decisions, and rules.

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
