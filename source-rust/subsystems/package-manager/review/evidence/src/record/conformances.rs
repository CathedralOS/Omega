// Receipt-free, source-handle-free selected conformance policy components.

#[path = "conformances_policy.rs"]
mod policy;

#[cfg(any(test, feature = "full-policy-recovery"))]
pub(crate) use policy::PackagePolicyConformanceRow;
pub(crate) use policy::{
    PackagePolicyClosedConformanceApplication, PackagePolicyConformanceConstArgument,
};
