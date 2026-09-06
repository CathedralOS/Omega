#![forbid(unsafe_code)]

//! Target-neutral operational, reach, invocation, and capability-flow facts.

mod capabilities;
mod invocations;
mod operational;
mod service_reach;

pub use capabilities::{CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan};
pub use invocations::{InvocationInferencePlan, InvocationTarget, MachineInvocationInference};
pub use operational::{CallOperational, MachineOperational, OperationalPlan, StateOperational};
pub use service_reach::{
    CallServiceReachInference, InstallationReachRequirement, MachineServiceReachInference,
    ServiceReachInferencePlan, StateServiceReachInference,
};
