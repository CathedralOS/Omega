// Receipt-free selected provider meaning, carrying no acceptance or execution
// authority.

mod authority;
mod binding_validation;
mod families;
mod methods;
mod plans;
mod rows;
mod service_validation;
mod signature;
mod validation;

pub use authority::PackagePolicyServiceAuthority;
pub(crate) use authority::{
    PackagePolicyServiceProgressPremise, PackagePolicyServiceProgressRoute,
};
pub(crate) use families::{PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate};
pub use methods::PackagePolicyServiceMethod;
pub use plans::PackagePolicyProviderPlan;
pub use plans::PackagePolicySelectedProviders;

pub use rows::{
    PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall, PackagePolicyProviderRow,
};
pub(crate) use service_validation::validate_service_methods;
pub(crate) fn validate_native_service_method(
    method: &PackagePolicyServiceMethod,
    target: target::TargetProfile,
) -> Result<(), &'static str> {
    validate_service_methods(from_ref(method), &method.requirement_owner, target)
}
pub use signature::PackagePolicyServiceSignature;
use std::slice::from_ref;
