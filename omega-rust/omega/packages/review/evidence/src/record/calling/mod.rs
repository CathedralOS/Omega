//! Receipt-free components of the published boundary calling contract.
//!
//! The physical component is not a complete boundary policy: exact requirement,
//! target, native telescope, and named callback associations belong beside it.

mod physical;

pub use physical::{
    PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
    PackagePolicyMachineState, PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract,
    PackagePolicyPreemption, PackagePolicyStatePlan,
};
