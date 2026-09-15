#![forbid(unsafe_code)]

//! Declarative physical-register facts and their independent structural
//! validators.
//!
//! Representation owners define the register vocabulary consumed by clean ISA
//! catalogs and future allocators. This crate deliberately performs no
//! allocation and reaches into no target-global registry.
//!
//! `physical_register_model.rs` is the root: the model one architecture
//! declares and its validator. `register_vocabulary.rs` names units, views
//! and classes, `reservation_profiles.rs` withholds units from allocation,
//! `constraint_catalog.rs` constrains instruction operands, `identities.rs`
//! fingerprints validated declarations and `preservation_storage.rs`
//! describes preserved-register storage.

mod constraint_catalog;
mod identities;
mod physical_register_model;
mod preservation_storage;
mod register_vocabulary;
mod reservation_profiles;
#[cfg(test)]
mod tests;

pub use constraint_catalog::{
    RegisterConstraintCatalog, RegisterConstraintCatalogValidationError, RegisterConstraintFamily,
    RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterOperandConstraint, TargetRegisterEnvironmentConstraintKeys,
    ValidatedRegisterConstraintCatalog, validate_register_constraint_catalog,
};
pub use identities::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    RegisterReservationProfileIdentity, TargetRegisterEnvironmentIdentity,
    target_register_environment_identity,
};
pub use physical_register_model::{
    PhysicalRegisterModel, PreservationConvention, RegisterModelValidationError,
    ValidatedPhysicalRegisterModel, validate_physical_register_model,
};
pub use preservation_storage::{
    FrameAbiPreservationConvention, PreservationStorageCatalog, PreservationStorageCatalogIdentity,
    PreservationStorageCatalogValidationError, PreservationStorageGroup,
    PreservationStorageGroupId, ValidatedPreservationStorageCatalog,
    preservation_storage_catalog_identity, validate_preservation_storage_catalog,
};
pub use register_vocabulary::{
    RegisterClass, RegisterClassId, RegisterUnit, RegisterUnitId, RegisterUnitKind, RegisterView,
    RegisterViewId, RegisterWriteSemantics,
};
pub use reservation_profiles::{
    RegisterReservationOverlay, RegisterReservationProfile,
    RegisterReservationProfileValidationError, ReservationReason,
    ValidatedRegisterReservationProfile, validate_register_reservation_profile,
};
