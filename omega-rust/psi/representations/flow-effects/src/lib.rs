#![forbid(unsafe_code)]

//! Target-neutral effect records. Start at `flow_effects`.
//!
//! The records state which effects a machine, state, or call may perform, in
//! the form later stages and package review consume. This crate holds the
//! vocabulary and its storage only; effect inference lives in the checking
//! stage and effect validation in the validation crate.

pub mod flow_effects;
pub use flow_effects::{
    CallOperational, CallServiceReachInference, CapabilityFlowFact, CapabilityFlowKind,
    CapabilityFlowPlan, InstallationReachRequirement, InvocationInferencePlan, InvocationTarget,
    MachineInvocationInference, MachineOperational, MachineServiceReachInference, OperationalPlan,
    ServiceReachDependency, ServiceReachInferencePlan, StateOperational,
    StateServiceReachInference, StaticMachineCallBinding, capabilities, invocations, operational,
    service_reach,
};
