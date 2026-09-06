//! Independent operational, invocation, reach and capability summaries.
//! These records are not another combined program or an inference engine.

pub mod capabilities;
pub mod invocations;
pub mod operational;
pub mod service_reach;

pub use capabilities::{CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan};
pub use invocations::{InvocationInferencePlan, InvocationTarget, MachineInvocationInference};
pub use operational::{CallOperational, MachineOperational, OperationalPlan, StateOperational};
pub use service_reach::{
    CallServiceReachInference, InstallationReachRequirement, MachineServiceReachInference,
    ServiceReachInferencePlan, StateServiceReachInference,
};
