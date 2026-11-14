//! Validates field access demands and their compatibility with supplied backing.
//!
//! `plan` joins layout geometry, field decisions and reach into the normalized
//! representation consumed by compiler stages. `plan_policy` checks operations;
//! `resources` checks provider profiles, range supply and relocation requirements.
//! These checks do not establish runtime views or issue backing authority.

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
mod plan;

pub(crate) use normalized_identity::normalized_identity;
mod plan_policy;
mod resources;
pub use resources::run_relocation::validate_run_relocation_resources;

pub use access_fields::diagnostic::AccessPlanDiagnostic;
pub use access_fields::{
    AccessExposure, AccessFieldEntry, AccessFieldKey, AccessFields, AccessFieldsId,
    AccessPermissions, AtomicPermissions, AuthorizedFieldAccess, EffectFootprint, ExternalRead,
    FieldAccess, FieldAccessDescriptor, FieldRegion, LogicalFieldExtent, LogicalFieldFragment,
    ObservationModel, RelativeEffectFootprint, ValidatedAccessFields,
};
pub use plan::{
    AccessPlan, AccessPlanId, AccessPlanSnapshot, ValidatedAccessPlan, validate_access_plan,
};
pub use plan_policy::access_fields_validation::validate_access_fields;
pub use plan_policy::access_operations::{AccessOperation, AtomicAccessOperation, BorrowPolarity};
pub use plan_policy::boundary_reach::{BoundaryReach, BoundaryServiceReachId};
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
