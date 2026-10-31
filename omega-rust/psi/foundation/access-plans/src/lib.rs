#![forbid(unsafe_code)]

//! Normalized access policy for access views.
//!
//! `LayoutPlan` owns representation and geometry with no access rights.
//! `AccessPlan` contains one layout plan, the per-field decisions
//! (`AccessFields`: observation and the exact primitive operations permitted
//! over that geometry) and the required boundary reach. A layout plan never
//! acquires MMIO vocabulary, and no arbitrary-offset volatile escape hatch can
//! bypass plan validation, because access decisions exist only inside an
//! access plan that was validated against its layout.
//!
//! Start at `access_fields.rs`: the decisions, their validated form, the sealed
//! field descriptors and the diagnostic every operation reports. Around it,
//! `plan_policy/access_operations.rs` names what a field can be asked to do,
//! `plan_policy/boundary_reach.rs` which services an access plan may reach,
//! `resources/resource_profile.rs` what a provider region supplies, and
//! `placements/access_plan.rs` joins layout, fields and reach. The remaining
//! modules are the operations: validation, admission, custody, views,
//! projection and primitive specialization, each owning the carriers it moves
//! an access view through.
//!
//! The modules are grouped by what they decide: `plan_policy/` what a plan
//! permits, `resources/` what a provider supplies, `placements/` how a plan
//! lands on a range, and `primitive_access/` how one access lowers.

/// Every carrier in this crate reports the same diagnostic field under the
/// same accessor name.
macro_rules! diagnostic_accessor {
    () => {
        /// The diagnostic the carrier reports.
        pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
            &self.diagnostic
        }
    };
}

pub(crate) use optimization::digest_encoder;
mod access_fields;
mod normalized_identity;
mod placements;

pub(crate) use normalized_identity::normalized_identity;
mod plan_policy;
mod primitive_access;
mod resources;
#[cfg(test)]
mod tests;

pub use access_fields::diagnostic::AccessPlanDiagnostic;
pub use access_fields::{
    AccessExposure, AccessFieldEntry, AccessFieldKey, AccessFields, AccessFieldsId,
    AccessPermissions, AtomicPermissions, AuthorizedFieldAccess, EffectFootprint, ExternalRead,
    FieldAccess, FieldAccessDescriptor, FieldRegion, LogicalFieldExtent, LogicalFieldFragment,
    ObservationModel, RelativeEffectFootprint, ValidatedAccessFields,
};
pub use placements::access_admission::{
    AccessAdmission, AccessAdmissionId, AccessOccurrenceId, AccessRejection,
    PlaceEstablishmentError,
};
pub use placements::access_admission::{admit_access, admit_owned_access, place};
pub use placements::access_plan::{
    AccessPlan, AccessPlanId, AccessPlanSnapshot, ValidatedAccessPlan, validate_access_plan,
};
pub use placements::atomic_resident_views::{
    BorrowedAtomicResidentRetirementError, EstablishedBorrowedAtomicResidentAccess,
};
pub use placements::borrowed_view::{AccessView, AccessViewRetirementError};
pub use placements::owned_access_lifecycle::{
    DormantOwnedResident, EstablishedOwnedAccess, OwnedAccessAdmission, OwnedAccessRejection,
    OwnedResidentRetirementError, OwnedResidentViewEstablishmentError, OwnedStableAdoptionError,
};
pub use placements::owned_atomic_resident_custody::{
    DormantOwnedAtomicResident, EstablishedOwnedAtomicAccess, OwnedAtomicAdoptionError,
    OwnedAtomicResidentRetirementError, OwnedAtomicResidentViewEstablishmentError,
    adopt_owned_atomic,
};
pub use placements::owned_external_correspondence::{
    EstablishedOwnedExternalAccess, OwnedCorrespondedExternalAdmission, OwnedExternalAdoptionError,
    OwnedExternalRetirementError, OwnedExternalViewEstablishmentError, adopt_owned_external,
};
pub use placements::owned_resident_custody::adopt_owned_stable;
pub use placements::resident_views::{
    BorrowedResidentRetirementError, EstablishedBorrowedResidentAccess,
};
pub use placements::schema_correspondence::{
    AdmittedSchemaDeviceCorrespondence, DeviceRevisionPredicateId, RuntimeDeviceRevisionEvidence,
    RuntimeDeviceRevisionObservationId, SchemaCorrespondedAccessAdmission,
    SchemaCorrespondedAccessView, SchemaCorrespondedPlaceEstablishmentError,
    SchemaCorrespondedPlaceRetirementError, SchemaCorrespondenceAccessBindingError,
    SchemaCorrespondenceProviderId, SchemaCorrespondenceSourceId,
    SchemaDeviceCorrespondenceAdmissionError, SchemaDeviceCorrespondenceGrant,
    SchemaDeviceCorrespondenceGrantError, SchemaDeviceCorrespondenceReceiptContext,
    StableDeviceInstanceId, bind_schema_correspondence_to_access_plan,
};
pub use plan_policy::access_fields_validation::validate_access_fields;
pub use plan_policy::access_operations::{AccessOperation, AtomicAccessOperation, BorrowPolarity};
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
pub use primitive_access::field_projection::{AccessFieldAccess, AccessFieldProjection};
pub use primitive_access::primitive_request::PrimitiveAccessRequest;
pub use primitive_access::primitive_specialization::{
    AtomicPrimitiveAccessRejection, AtomicPrimitiveAccessRequest, ExternalPrimitiveAccessRejection,
    ExternalPrimitiveAccessRequest, ExternalPrimitiveOperation,
    StableCompoundMutationAccessRejection, StableCompoundMutationAccessRequest,
    StablePrimitiveAccessRejection, StablePrimitiveAccessRequest, StablePrimitiveOperation,
};
pub use resources::device_operation_requirements::{
    DeviceOperation, DeviceOperationCoordinates, DeviceOperationProviderPlanId,
    DeviceOperationRequirement, DeviceOperationRequirementId,
    DeviceOperationStructuralClosureError, DeviceOrderingScopeId, DeviceOrderingScopeOccurrence,
    DeviceOrderingScopeOccurrenceId, ProviderAssertedDeviceOperationClaim,
    StructurallyClosedDeviceOperationRequirement, StructurallyClosedDeviceOperationRequirements,
    structurally_close_device_operation_requirements,
};
pub use resources::resource_compatibility::{
    AccessResourceCompatibility, BaseCongruence, EffectiveFieldSupply, EffectiveSupplyKind,
};
pub use resources::resource_compatibility::{
    InitializationResourceCompatibility, require_stable_region_supply, validate_access_resources,
    validate_initialization_resources,
};
pub use resources::resource_profile::{
    AtomicCapability, AtomicTransferRule, ExternalCapability, ExternalReadBehavior,
    PeerWritability, ResourceProfile, ResourceProfileId, ResourceRegion, StableCapability,
    TransferRule, ValidatedResourceProfile,
};
pub use resources::resource_profile_admission::{
    AdmittedResourceProfile, ResourceProfileAdmissionError, ResourceProfileGrant,
    ResourceProfileReceiptId,
};
pub use resources::resource_profile_validation::validate_resource_profile;
