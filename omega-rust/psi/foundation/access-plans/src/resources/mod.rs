//! What a provider supplies: resource profiles, their validation and
//! admission, compatibility with a placement demand, and device operation
//! requirements.

pub(crate) mod device_operation_requirements;
pub(crate) mod resource_compatibility;
pub(crate) mod resource_profile;
pub(crate) mod resource_profile_admission;
pub(crate) mod resource_profile_validation;
