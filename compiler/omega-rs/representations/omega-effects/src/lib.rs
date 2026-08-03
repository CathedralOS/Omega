mod capabilities;
mod executable_tcb_manifest;
mod executable_tcb_profile;
mod selected_provider_plans;

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
pub use executable_tcb_manifest::{
    ContainmentEvidence, ContainmentGuarantee, ExecutableEntryOrigin, ExecutableIdentity,
    ExecutableTcbEntry, ExecutableTcbManifest, ExecutionScope, ImplementationEvidence,
    IncompleteCause, OpaqueClosureEvidence, OpaqueExecutableAdmissionCandidate,
    OpaqueInProcessBinding, ProviderIdentity, ScopeCompleteness,
    ValidatedOpaqueExecutableAdmission,
};
pub use executable_tcb_profile::{
    ExactExecutableTcbAllowance, ExecutableTcbProfile, ExecutableTcbProfileAcceptance,
    ExecutableTcbProfileRejection, ExecutableTcbProfileViolation, IncompleteScopePolicy,
    evaluate_executable_tcb_profile,
};
pub use selected_provider_plans::SelectedProviderPlanFacts;
