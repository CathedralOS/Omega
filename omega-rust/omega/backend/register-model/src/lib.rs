#![forbid(unsafe_code)]

//! Declarative physical-register facts and their independent structural
//! validators.
//!
//! Representation owners define the register vocabulary consumed by clean ISA
//! catalogs and future allocators. This crate deliberately performs no
//! allocation and reaches into no target-global registry.
//!
//! Start at `register_model.rs`: the model one architecture declares and its
//! validator, leading into `register_vocabulary` (units, views and classes),
//! `reservation_profiles` (units withheld from allocation),
//! `constraint_catalog` (instruction operand constraints), `identities`
//! (fingerprints of validated declarations) and `preservation_storage`
//! (preserved-register storage).

mod register_model;
#[cfg(test)]
mod tests;

pub use register_model::constraint_catalog::{
    RegisterConstraintCatalog, RegisterConstraintCatalogValidationError, RegisterConstraintFamily,
    RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterOperandConstraint, TargetRegisterEnvironmentConstraintKeys,
    ValidatedRegisterConstraintCatalog, validate_register_constraint_catalog,
};
pub use register_model::identities::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    RegisterReservationProfileIdentity, TargetRegisterEnvironmentIdentity,
    target_register_environment_identity,
};
pub use register_model::preservation_storage::{
    FrameAbiPreservationConvention, PreservationStorageCatalog, PreservationStorageCatalogIdentity,
    PreservationStorageCatalogValidationError, PreservationStorageGroup,
    PreservationStorageGroupId, ValidatedPreservationStorageCatalog,
    preservation_storage_catalog_identity, validate_preservation_storage_catalog,
};
pub use register_model::register_vocabulary::{
    RegisterClass, RegisterClassId, RegisterUnit, RegisterUnitId, RegisterUnitKind, RegisterView,
    RegisterViewId, RegisterWriteSemantics,
};
pub use register_model::reservation_profiles::{
    RegisterReservationOverlay, RegisterReservationProfile,
    RegisterReservationProfileValidationError, ReservationReason,
    ValidatedRegisterReservationProfile, validate_register_reservation_profile,
};
pub use register_model::{
    PhysicalRegisterModel, PreservationConvention, RegisterModelValidationError,
    ValidatedPhysicalRegisterModel, validate_physical_register_model,
};
