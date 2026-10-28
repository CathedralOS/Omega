//! Receipt-free selected provider meaning; no acceptance or execution authority.

mod authority;
mod binding_validation;
mod families;
mod methods;
mod plans;
mod rows;
mod service_validation;
mod signature;
mod validation;

pub(crate) use authority::{
    PackagePolicyServiceAuthority, PackagePolicyServiceProgressPremise,
    PackagePolicyServiceProgressRoute,
};
pub(crate) use families::{PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate};
pub use methods::PackagePolicyServiceMethod;
pub(crate) use plans::PackagePolicyProviderPlan;
pub use plans::PackagePolicySelectedProviders;

pub(crate) use rows::{
    PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall, PackagePolicyProviderRow,
};
pub(crate) use service_validation::validate_service_methods;
pub(crate) fn validate_native_service_method(
    method: &PackagePolicyServiceMethod,
    target: target::TargetProfile,
) -> Result<(), &'static str> {
    validate_service_methods(
        std::slice::from_ref(method),
        &method.requirement_owner,
        target,
    )
}
pub(crate) use signature::PackagePolicyServiceSignature;
