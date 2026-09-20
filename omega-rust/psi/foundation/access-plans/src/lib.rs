#![forbid(unsafe_code)]

//! Normalized access policy for placed views.
//!
//! `LayoutPlan` owns geometry. `AccessPlan` owns observation and the exact
//! primitive operations permitted over that geometry. Keeping them separate
//! prevents wire layouts from acquiring MMIO vocabulary and prevents an
//! arbitrary-offset volatile escape hatch from bypassing plan validation.
//!
//! Start at `access_plan.rs`: the plan, its validated form, the sealed field
//! descriptors and the diagnostic every operation reports. Around it, `access_operations.rs` names what a field
//! can be asked to do, `boundary_reach.rs` which services a placement may
//! reach, `resource_profile.rs` what a provider region supplies, and
//! `placement_plan.rs` joins layout, access and reach. The remaining modules
//! are the operations: validation, admission, custody, views, projection and
//! primitive specialization, each owning the carriers it moves a placement
//! through.
//!
//! The modules are grouped by what they decide: `plan_policy/` what a plan
//! permits, `resources/` what a provider supplies, `placements/` how a plan
//! lands on a range, and `primitive_access/` how one access lowers.

mod access_plan;
mod placements;
mod plan_policy;
mod primitive_access;
mod resources;
#[cfg(test)]
mod tests;

pub use access_plan::diagnostic::AccessPlanDiagnostic;
pub use access_plan::{
    AccessExposure, AccessFieldEntry, AccessFieldKey, AccessPermissions, AccessPlan, AccessPlanId,
    AtomicPermissions, AuthorizedFieldAccess, EffectFootprint, ExternalRead, FieldAccess,
    FieldAccessDescriptor, LogicalFieldExtent, LogicalFieldFragment, ObservationModel,
    RelativeEffectFootprint, ValidatedAccessPlan,
};
pub use placements::atomic_resident_views::{
    BorrowedAtomicResidentRetirementError, EstablishedBorrowedAtomicResidentPlacement,
};
pub use placements::borrowed_view::{PlacedView, PlacedViewRetirementError};
pub use placements::owned_atomic_resident_custody::{
    DormantOwnedAtomicResident, EstablishedOwnedAtomicPlacement, OwnedAtomicAdoptionError,
    OwnedAtomicResidentRetirementError, OwnedAtomicResidentViewEstablishmentError,
    adopt_owned_atomic,
};
pub use placements::owned_placement_lifecycle::{
    DormantOwnedResident, EstablishedOwnedPlacement, OwnedPlacementAdmission,
    OwnedPlacementRejection, OwnedResidentRetirementError, OwnedResidentViewEstablishmentError,
    OwnedStableAdoptionError,
};
pub use placements::owned_resident_custody::adopt_owned_stable;
pub use placements::placement_admission::{
    PlaceEstablishmentError, PlacedOccurrenceId, PlacementAdmission, PlacementAdmissionId,
    PlacementRejection,
};
pub use placements::placement_admission::{admit_owned_placement, admit_placement, place};
pub use placements::placement_plan::{
    PlacementPlan, PlacementPlanId, ValidatedPlacementPlan, validate_placement_plan,
};
pub use placements::resident_views::{
    BorrowedResidentRetirementError, EstablishedBorrowedResidentPlacement,
};
pub use placements::schema_correspondence::{
    AdmittedSchemaDeviceCorrespondence, DeviceRevisionPredicateId, RuntimeDeviceRevisionEvidence,
    RuntimeDeviceRevisionObservationId, SchemaCorrespondedPlaceEstablishmentError,
    SchemaCorrespondedPlaceRetirementError, SchemaCorrespondedPlacedView,
    SchemaCorrespondedPlacementAdmission, SchemaCorrespondencePlacementBindingError,
    SchemaCorrespondenceProviderId, SchemaCorrespondenceSourceId,
    SchemaDeviceCorrespondenceAdmissionError, SchemaDeviceCorrespondenceGrant,
    SchemaDeviceCorrespondenceGrantError, SchemaDeviceCorrespondenceReceiptContext,
    StableDeviceInstanceId, bind_schema_correspondence_to_placement,
};
pub use plan_policy::access_operations::{AccessOperation, AtomicAccessOperation, BorrowPolarity};
pub use plan_policy::access_plan_validation::validate_access_plan;
pub use plan_policy::authorization::effect_footprints_conflict;
pub use plan_policy::boundary_reach::{BoundaryReach, BoundaryServiceReachId};
pub use primitive_access::corresponded_atomic::{
    CorrespondedAtomicPrimitiveAccessRejection, CorrespondedAtomicPrimitiveAccessRequest,
};
pub use primitive_access::corresponded_external::{
    CorrespondedExternalPrimitiveAccessRejection, CorrespondedExternalPrimitiveAccessRequest,
};
pub use primitive_access::corresponded_stable::{
    CorrespondedStablePrimitiveAccessRejection, CorrespondedStablePrimitiveAccessRequest,
};
pub use primitive_access::corresponded_stable_compound::{
    CorrespondedStableCompoundMutationAccessRejection,
    CorrespondedStableCompoundMutationAccessRequest,
};
pub use primitive_access::field_projection::{PlacedFieldAccess, PlacedFieldProjection};
pub use primitive_access::primitive_request::PrimitiveAccessRequest;
pub use primitive_access::primitive_specialization::{
    AtomicPrimitiveAccessRejection, AtomicPrimitiveAccessRequest, ExternalPrimitiveAccessRejection,
    ExternalPrimitiveAccessRequest, ExternalPrimitiveOperation,
    StableCompoundMutationAccessRejection, StableCompoundMutationAccessRequest,
    StablePrimitiveAccessRejection, StablePrimitiveAccessRequest, StablePrimitiveOperation,
};
pub use resources::device_operation_requirements::{
    DeviceOperation, DeviceOperationProviderPlanId, DeviceOperationRequirement,
    DeviceOperationRequirementId, DeviceOperationStructuralClosureError, DeviceOrderingScopeId,
    DeviceOrderingScopeOccurrence, DeviceOrderingScopeOccurrenceId,
    ProviderAssertedDeviceOperationClaim, StructurallyClosedDeviceOperationRequirement,
    StructurallyClosedDeviceOperationRequirements,
    structurally_close_device_operation_requirements,
};
pub use resources::resource_compatibility::validate_placement_resources;
pub use resources::resource_compatibility::{
    BaseCongruence, EffectiveFieldSupply, EffectiveSupplyKind, PlacementResourceCompatibility,
};
pub use resources::resource_profile::{
    AtomicCapability, AtomicTransferRule, ExternalCapability, ExternalReadBehavior,
    ResourceProfile, ResourceProfileId, ResourceRegion, StableCapability, TransferRule,
    ValidatedResourceProfile,
};
pub use resources::resource_profile_admission::{
    AdmittedResourceProfile, ResourceProfileAdmissionError, ResourceProfileGrant,
    ResourceProfileReceiptId,
};
pub use resources::resource_profile_validation::validate_resource_profile;
