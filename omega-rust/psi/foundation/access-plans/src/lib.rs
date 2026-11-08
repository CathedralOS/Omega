/*
We define normalized access policy (`AccessPlan`, `AccessFields`, and
`ValidatedAccessPlan`), provider resource profiles (`ResourceProfile`), range
placements and resident/external custody views (`placements`), and sealed
primitive access specialization (`primitive_access`) over `LayoutPlan` geometry.
*/

/*
Layout geometry alone never confers MMIO, atomic, or volatile memory rights, and
source code must never bypass access validation via raw offset arithmetic. By
joining a validated `LayoutPlanReport` with per-field observation models,
allowed primitive operations (`plan_policy`), boundary service reach, and
provider-supplied `ResourceProfile` capabilities (`resources`), we guarantee
that every stable, atomic, or external access request is authorized against both
its layout and its backing address-space extent before lowering.
*/

/*
`access_fields.rs` defines field descriptors and `AccessPlanDiagnostic`;
`plan_policy.rs` validates field access permissions and footprint conflicts;
`resources.rs` governs provider resource profiles and device operation closure;
`placements.rs` manages borrowed/owned resident and schema-corresponded views;
`primitive_access.rs` projects fields into sealed specialization requests.
*/

#![forbid(unsafe_code)]

macro_rules! diagnostic_accessor {
    () => {
        pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
            &self.diagnostic
        }
    };
}

pub(crate) use optimization::digest_encoder;
mod access_fields;
mod normalized_identity {
    macro_rules! normalized_identity {
        ($name:ident, $label:literal $(, $field_vis:vis)?) => {
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name($($field_vis)? u64);
            impl compiler_storage::Record for $name {
                fn write(&self, output: &mut compiler_storage::RecordWriter) -> Result<(), compiler_storage::RecordError> { compiler_storage::Record::write(&self.0, output) }
                fn read<Context>(input: &mut compiler_storage::RecordReader<'_, Context>) -> Result<Self, compiler_storage::RecordError> {
                    Self::from_normalized_identity(compiler_storage::Record::read(input)?)
                        .map_err(|_| input.error(concat!($label, " identity cannot be zero")))
                }
            }

            impl $name {
                pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
                    if identity == 0 {
                        return Err(AccessPlanDiagnostic(
                            concat!($label, " cannot be zero").into(),
                        ));
                    }
                    Ok(Self(identity))
                }

                pub const fn normalized_identity(self) -> u64 {
                    self.0
                }
            }
        };
    }

    pub(crate) use normalized_identity;
}
mod placements;

pub(crate) use normalized_identity::normalized_identity;
mod plan_policy;
mod primitive_access;
mod resources;
pub use resources::run_relocation::validate_run_relocation_resources;

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

#[cfg(test)]
mod tests;
