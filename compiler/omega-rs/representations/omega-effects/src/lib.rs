mod capabilities;
mod coexisting_executable_eras;
mod executable_tcb_manifest;
mod executable_tcb_profile;
mod isolated_executable_scopes;
mod process_static_services;
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
pub use coexisting_executable_eras::{
    AdmittedExecutableEra, AttributedContainmentEvidence, AttributedManifestCompleteness,
    CoexistingExecutableTcbEntry, CoexistingExecutableTcbReport, CoexistingExecutableTcbSet,
    CoexistingScopeCompleteness, ExecutableManifestSource,
};
pub use executable_tcb_manifest::{
    ContainmentEvidence, ContainmentGuarantee, ExecutableEntryOrigin, ExecutableIdentity,
    ExecutableTcbEntry, ExecutableTcbManifest, ExecutionScope, ImplementationEvidence,
    IncompleteCause, OmegaRuntimeExecutableAdmissionCandidate, OmegaRuntimeExecutableLedger,
    OpaqueClosureEvidence, OpaqueExecutableAdmissionCandidate, OpaqueInProcessBinding,
    ProviderIdentity, RuntimeExecutableClosureEvidence, ScopeCompleteness,
    ValidatedOpaqueExecutableAdmission,
};
pub use executable_tcb_profile::{
    ExactExecutableTcbAllowance, ExecutableTcbProfile, ExecutableTcbProfileAcceptance,
    ExecutableTcbProfileRejection, ExecutableTcbProfileViolation, IncompleteScopePolicy,
    evaluate_executable_tcb_profile,
};
pub use isolated_executable_scopes::{
    AdmittedIsolatedExecutableScope, ExecutableTcbManifestSet, IsolatedExecutableScopeCandidate,
};
pub use process_static_services::{
    ActiveServiceRegistration, AtomicServiceHandoverReceipt, ProcessStaticServiceContract,
    ProcessStaticServicePolicy, ProcessStaticServiceRegistry, ServiceHandoverCompletion,
    ServiceHandoverError, ServiceRegistrationCandidate, ServiceRegistrationError,
};
pub use selected_provider_plans::SelectedProviderPlanFacts;
