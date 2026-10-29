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

/// `Display` for the crate's `<context> invalid: {self:?}` error convention.
macro_rules! invalid_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }
    };
}

pub(crate) use optimization_core::codec_bytes;
pub(crate) use optimization_core::digest_encoder;
mod register_model;
#[cfg(test)]
mod tests;

pub use register_model::constraint_catalog::{
    RegisterConstraintCatalog, RegisterConstraintCatalogValidationError, RegisterConstraintFamily,
    RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterOperandConstraint, TargetRegisterEnvironmentConstraintKeys,
    ValidatedRegisterConstraintCatalog, call_constraint_keys, encode_constraint_key,
    find_register_constraint, find_register_constraint_mut, validate_register_constraint_catalog,
    views_with_access,
};
pub use register_model::identities::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    RegisterReservationProfileIdentity, TargetRegisterEnvironmentIdentity,
    infrastructure_environment_agrees, target_register_environment_identity, write_semantics_tag,
};
pub use register_model::preservation_storage::{
    FrameAbiPreservationConvention, PreservationStorageCatalog, PreservationStorageCatalogIdentity,
    PreservationStorageCatalogValidationError, PreservationStorageGroup,
    PreservationStorageGroupId, ValidatedPreservationStorageCatalog,
    validate_preservation_storage_catalog,
};
pub use register_model::register_vocabulary::{
    RegisterClass, RegisterClassId, RegisterUnit, RegisterUnitId, RegisterUnitKind, RegisterView,
    RegisterViewId, RegisterWriteSemantics, Sha256RegisterDigest, sha256_length,
    sha256_register_units,
};
pub(crate) use register_model::reservation_profiles::RegisterReservationOverlay;
pub use register_model::reservation_profiles::{
    RegisterReservationProfile, RegisterReservationProfileValidationError, ReservationReason,
    ValidatedRegisterReservationProfile, reservation_overlay, sorted_units,
    validate_register_reservation_profile,
};
#[cfg(feature = "test-support")]
pub use register_model::test_support;
pub use register_model::{
    PhysicalModelBuilder, PhysicalRegisterModel, PreservationConvention,
    RegisterModelValidationError, ValidatedPhysicalRegisterModel, complement, units_for,
    validate_physical_register_model, view_ids,
};
