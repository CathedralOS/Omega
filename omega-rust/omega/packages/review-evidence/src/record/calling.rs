//! Receipt-free components of the published boundary calling contract.
//!
//! The physical component is not a complete boundary policy: exact requirement,
//! target, native telescope, and named callback associations belong beside it.

mod application;
mod callbacks;
mod opaque;
mod physical;

pub(crate) use application::{
    PackagePolicyCallingParameter, PackagePolicyCallingPlan, PackagePolicyNativeParameter,
    PackagePolicyNativeParameterOrigin};

pub(crate) use callbacks::{
    PackagePolicyCallbackBinder, PackagePolicyCallbackDemand, PackagePolicyCallbackDestination,
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication, PackagePolicyCallbackMaterialization,
    PackagePolicyCallbacks};
pub(crate) use opaque::PackagePolicyCallingOpaqueUse;

pub(crate) use physical::{
    PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
    PackagePolicyMachineState, PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract,
    PackagePolicyPreemption, PackagePolicyStatePlan};
