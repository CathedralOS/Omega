#![forbid(unsafe_code)]

//! Normalized access policy for placed views.
//!
//! `LayoutPlan` owns geometry. `AccessPlan` owns observation and the exact
//! primitive operations permitted over that geometry. Keeping them separate
//! prevents wire layouts from acquiring MMIO vocabulary and prevents an
//! arbitrary-offset volatile escape hatch from bypassing plan validation.
//!
//! `access_plan.rs` is the root: the plan, its validated form and the sealed
//! field descriptors. Around it, `access_operations.rs` names what a field
//! can be asked to do, `boundary_reach.rs` which services a placement may
//! reach, `resource_profile.rs` what a provider region supplies, and
//! `placement_plan.rs` joins layout, access and reach. The remaining modules
//! are the operations: validation, admission, custody, views, projection and
//! primitive specialization, each owning the carriers it moves a placement
//! through.

mod access_operations;
mod access_plan;
mod access_plan_validation;
mod atomic_resident_views;
mod authorization;
mod borrowed_view;
mod boundary_reach;
mod corresponded_atomic;
mod corresponded_external;
mod corresponded_stable;
mod corresponded_stable_compound;
mod device_operation_requirements;
mod diagnostic;
mod field_projection;
mod normalized_identities;
mod owned_atomic_resident_custody;
mod owned_placement_lifecycle;
mod owned_resident_custody;
mod placement_admission;
mod placement_authority;
mod placement_plan;
mod primitive_request;
mod primitive_specialization;
mod resident_views;
mod resource_compatibility;
mod resource_profile;
mod resource_profile_admission;
mod resource_profile_validation;
mod schema_correspondence;
#[cfg(test)]
mod tests;

pub use access_operations::{AccessOperation, AtomicAccessOperation, BorrowPolarity};
pub use access_plan::{
    AccessExposure, AccessFieldEntry, AccessFieldKey, AccessPermissions, AccessPlan, AccessPlanId,
    AtomicPermissions, AuthorizedFieldAccess, EffectFootprint, ExternalRead, FieldAccess,
    FieldAccessDescriptor, LogicalFieldExtent, LogicalFieldFragment, ObservationModel,
    RelativeEffectFootprint, ValidatedAccessPlan,
};
pub use access_plan_validation::validate_access_plan;
pub use atomic_resident_views::{
    BorrowedAtomicResidentRetirementError, EstablishedBorrowedAtomicResidentPlacement,
};
pub use authorization::effect_footprints_conflict;
pub use borrowed_view::{PlacedView, PlacedViewRetirementError};
pub use boundary_reach::{BoundaryReach, BoundaryServiceReachId};
pub use corresponded_atomic::{
    CorrespondedAtomicPrimitiveAccessRejection, CorrespondedAtomicPrimitiveAccessRequest,
};
pub use corresponded_external::{
    CorrespondedExternalPrimitiveAccessRejection, CorrespondedExternalPrimitiveAccessRequest,
};
pub use corresponded_stable::{
    CorrespondedStablePrimitiveAccessRejection, CorrespondedStablePrimitiveAccessRequest,
};
pub use corresponded_stable_compound::{
    CorrespondedStableCompoundMutationAccessRejection,
    CorrespondedStableCompoundMutationAccessRequest,
};
pub use device_operation_requirements::{
    DeviceOperation, DeviceOperationProviderPlanId, DeviceOperationRequirement,
    DeviceOperationRequirementId, DeviceOperationStructuralClosureError, DeviceOrderingScopeId,
    ProviderAssertedDeviceOperationClaim, StructurallyClosedDeviceOperationRequirement,
    StructurallyClosedDeviceOperationRequirements,
    structurally_close_device_operation_requirements,
};
pub use diagnostic::AccessPlanDiagnostic;
pub use field_projection::{PlacedFieldAccess, PlacedFieldProjection};
pub use owned_atomic_resident_custody::{
    DormantOwnedAtomicResident, EstablishedOwnedAtomicPlacement, OwnedAtomicAdoptionError,
    OwnedAtomicResidentRetirementError, OwnedAtomicResidentViewEstablishmentError,
    adopt_owned_atomic,
};
pub use owned_placement_lifecycle::{
    DormantOwnedResident, EstablishedOwnedPlacement, OwnedPlacementAdmission,
    OwnedPlacementRejection, OwnedResidentRetirementError, OwnedResidentViewEstablishmentError,
    OwnedStableAdoptionError,
};
pub use owned_resident_custody::adopt_owned_stable;
pub use placement_admission::{
    PlaceEstablishmentError, PlacedOccurrenceId, PlacementAdmission, PlacementAdmissionId,
    PlacementRejection,
};
pub use placement_admission::{admit_owned_placement, admit_placement, place};
pub use placement_plan::{
    PlacementPlan, PlacementPlanId, ValidatedPlacementPlan, validate_placement_plan,
};
pub use primitive_request::PrimitiveAccessRequest;
pub use primitive_specialization::{
    AtomicPrimitiveAccessRejection, AtomicPrimitiveAccessRequest, ExternalPrimitiveAccessRejection,
    ExternalPrimitiveAccessRequest, ExternalPrimitiveOperation,
    StableCompoundMutationAccessRejection, StableCompoundMutationAccessRequest,
    StablePrimitiveAccessRejection, StablePrimitiveAccessRequest, StablePrimitiveOperation,
};
pub use resident_views::{BorrowedResidentRetirementError, EstablishedBorrowedResidentPlacement};
pub use resource_compatibility::validate_placement_resources;
pub use resource_compatibility::{
    BaseCongruence, EffectiveFieldSupply, EffectiveSupplyKind, PlacementResourceCompatibility,
};
pub use resource_profile::{
    AtomicCapability, AtomicTransferRule, ExternalCapability, ExternalReadBehavior,
    ResourceProfile, ResourceProfileId, ResourceRegion, StableCapability, TransferRule,
    ValidatedResourceProfile,
};
pub use resource_profile_admission::{
    AdmittedResourceProfile, ResourceProfileAdmissionError, ResourceProfileGrant,
    ResourceProfileReceiptId,
};
pub use resource_profile_validation::validate_resource_profile;
pub use schema_correspondence::{
    AdmittedSchemaDeviceCorrespondence, DeviceRevisionPredicateId, RuntimeDeviceRevisionEvidence,
    RuntimeDeviceRevisionObservationId, SchemaCorrespondedPlaceEstablishmentError,
    SchemaCorrespondedPlaceRetirementError, SchemaCorrespondedPlacedView,
    SchemaCorrespondedPlacementAdmission, SchemaCorrespondencePlacementBindingError,
    SchemaCorrespondenceProviderId, SchemaCorrespondenceSourceId,
    SchemaDeviceCorrespondenceAdmissionError, SchemaDeviceCorrespondenceGrant,
    SchemaDeviceCorrespondenceGrantError, SchemaDeviceCorrespondenceReceiptContext,
    StableDeviceInstanceId, bind_schema_correspondence_to_placement,
};
