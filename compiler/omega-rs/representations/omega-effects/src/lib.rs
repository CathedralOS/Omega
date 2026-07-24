mod capabilities;
mod operational;
mod service_reach;

pub use capabilities::analysis::{
    UnapprovedBoundaryCall, audit_boundary_provider_calls,
    build_boundary_provider_approval_registry,
};
pub use capabilities::provider_approval::{
    BoundaryCallApproval, BoundaryProviderApproval, BoundaryProviderApprovalRegistry,
};
pub use capabilities::provider_plan;
pub use capabilities::providers::{
    BoundaryProvider, BoundaryProviderRegistry, build_provider_registry, validate_provider_bindings,
};
pub use capabilities::{CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan};
pub use operational::{
    CallOperational, MachineOperational, OperationalPlan, StateOperational, infer_operational_may,
};
pub use service_reach::{
    CallServiceReachInference, MachineServiceReachInference, ServiceReachInferencePlan,
    StateServiceReachInference, infer_service_reaches,
};
