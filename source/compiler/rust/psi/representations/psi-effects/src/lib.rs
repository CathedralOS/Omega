#![forbid(unsafe_code)]

//! Target-neutral operational, reach, invocation, and capability-flow facts.

mod capabilities;
mod invocations;
mod operational;
mod service_reach;

pub use capabilities::{CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan};
pub use invocations::{
    InvocationInferencePlan, InvocationTarget, MachineInvocationInference,
    declared_machine_invocations, declared_signature_invocations,
    has_self_forwarded_boundary_parameter, infer_synchronous_invocations, invocation_target_label,
};
pub use operational::{
    CallOperational, MachineOperational, OperationalPlan, StateOperational, infer_operational_may,
};
pub use service_reach::{
    CallServiceReachInference, InstallationReachRequirement, MachineServiceReachInference,
    ServiceReachInferencePlan, StateServiceReachInference, infer_service_reaches,
};
