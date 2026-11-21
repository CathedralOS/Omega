// Receipt-free components of the published boundary calling contract.
//
// The physical component is not a complete boundary policy on its own: exact
// requirement, target, native telescope, and named callback associations belong
// beside it.

mod application;
mod callbacks;
mod opaque;
mod physical;

pub use application::{
    PackagePolicyCallingParameter, PackagePolicyCallingPlan, PackagePolicyNativeParameter,
    PackagePolicyNativeParameterOrigin,
};

pub use callbacks::PackagePolicyCallbacks;
pub(crate) use callbacks::{
    PackagePolicyCallbackBinder, PackagePolicyCallbackDemand, PackagePolicyCallbackDestination,
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication, PackagePolicyCallbackMaterialization,
};
pub(crate) use opaque::PackagePolicyCallingOpaqueUse;

pub use physical::{
    PackagePolicyEntryControl, PackagePolicyMachineRegime, PackagePolicyMachineState,
    PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract, PackagePolicyStatePlan,
};
pub(crate) use physical::{PackagePolicyEntryStack, PackagePolicyPreemption};
