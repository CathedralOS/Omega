//! Independent operational, invocation, reach and capability summaries.
//! These records are not another combined program or an inference engine.

pub mod capabilities;
pub mod invocations;
pub mod operational;
mod proof_applications;
pub mod service_reach;

pub use capabilities::{CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan};
pub use invocations::{
    InvocationInferencePlan, InvocationTarget, MachineInvocationInference, RealizedInvocationTarget,
};
pub use operational::{
    CallOperational, CleanupDependency, MachineOperational, OperationalPlan, StateOperational,
    StaticMachineCallBinding,
};
pub use service_reach::{
    CallServiceReachInference, InstallationReachRequirement, MachineServiceReachInference,
    ServiceReachDependency, ServiceReachInferencePlan, StateServiceReachInference,
};
