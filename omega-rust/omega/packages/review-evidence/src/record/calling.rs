//! Receipt-free components of the published boundary calling contract.
//!
//! The physical component is not a complete boundary policy: exact requirement,
//! target, native telescope, and named callback associations belong beside it.

mod application;
mod callbacks;
mod opaque;
mod physical;

pub use application::{
};
pub(crate) use application::{
    PackagePolicyCallingParameter, PackagePolicyCallingPlan, PackagePolicyNativeParameter,
    PackagePolicyNativeParameterOrigin};
pub use callbacks::{
};
pub(crate) use callbacks::{
    PackagePolicyCallbackBinder, PackagePolicyCallbackDemand, PackagePolicyCallbackDestination,
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication, PackagePolicyCallbackMaterialization,
    PackagePolicyCallbacks};
pub(crate) use opaque::PackagePolicyCallingOpaqueUse;

pub use physical::{
};
pub(crate) use physical::{
    PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
    PackagePolicyMachineState, PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract,
    PackagePolicyPreemption, PackagePolicyStatePlan};
