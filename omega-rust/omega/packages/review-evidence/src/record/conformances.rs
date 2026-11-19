// Receipt-free, source-handle-free selected conformance policy components.

#[path = "conformances_policy.rs"]
mod policy;

pub(crate) use policy::{
    PackagePolicyClosedConformanceApplication, PackagePolicyConformanceConstArgument,
    PackagePolicyConformanceRow,
};
