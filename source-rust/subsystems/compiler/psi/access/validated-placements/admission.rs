//! Admit complete access decisions against explicit layout and resource inputs.
//! Geometry, provenance and commitment checks retain private issuance. Runtime
//! operation authorization belongs to the interpreter's `storage_access` module; raw policy is vocabulary.
#![forbid(unsafe_code)]
pub(crate) use ::wire::digest_encoder;
use access_vocabulary::*;
mod access_fields;
mod plan;
mod plan_policy;
mod resources;
pub use access_fields::{
    AccessFieldEntry, AccessFieldKey, AccessFields, AccessFieldsId, EffectFootprint,
    FieldAccessDescriptor, FieldRegion, LogicalFieldExtent, LogicalFieldFragment,
    RelativeEffectFootprint, ValidatedAccessFields,
};
pub use plan::{
    AccessPlan, PlacementId, PlacementSnapshot, ValidatedPlacement, normalize_access_plan,
    validate_placement,
};
pub use plan_policy::access_fields_validation::validate_access_fields;
pub use resources::resource_compatibility::{
    AccessResourceCompatibility, BaseCongruence, EffectiveFieldSupply, EffectiveSupplyKind,
    InitializationResourceCompatibility, require_stable_region_supply, validate_access_resources,
    validate_initialization_resources,
};
pub use resources::resource_profile::{ResourceProfileId, ValidatedResourceProfile};
pub use resources::resource_profile_validation::validate_resource_profile;
pub use resources::run_relocation::validate_run_relocation_resources;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
#[cfg(test)]
mod tests;
