//! Selected provider evidence retained across lowering, artifact replay and installation.
//!
//! Start at `provider_evidence.rs`: `SelectedProviderPlanFacts` retains the exact
//! selected plan set, execution scope, opaque admissions and installation reach.
//! Its identity commits those records; compact report coordinates never replace
//! the retained plans or strong commitments during replay.
//!
//! `provider_evidence/providers/` owns provider plans, approval registries and
//! boundary-call audits. `authority/` owns terminal mechanism classifications
//! and service permissions; `executable_scopes/` owns executable manifests and
//! scope admission; `component_eras/` owns era ledgers and progress evidence.
//! These are realization records, not the source language's effect analysis.

mod provider_evidence;

pub use provider_evidence::authority::service_terminal_authority_permission::ServiceTerminalAuthorityPermission;
pub use provider_evidence::authority::terminal_authority::{
    CheckedPhysicalOperationIdentity, CheckedPhysicalTerminalMechanismIdentity,
    CheckedSyscallArgumentContractIdentity, CompilerIntrinsicExecutionIdentity,
    CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation, NormalizedForeignArgumentContract,
    NormalizedForeignTerminalMechanismIdentity, PortableFilesystemAuthorityFacet,
    SyscallTerminalMechanismIdentity, TerminalAuthorityClass, TerminalAuthorityClosureLeaf,
    TerminalAuthorityClosureReviewBuildError, TerminalAuthorityClosureReviewReceipt,
    TerminalAuthorityDisposition, TerminalAuthorityPermissionPolicyIdentity,
    TerminalAuthorityPolicyIdentity, TerminalMechanismIdentity,
    compiler_intrinsic_execution_identity_bytes, terminal_mechanism_identity_bytes,
};
pub use provider_evidence::component_eras::component_era_entry_ledger::{
    ActiveComponentEraEntry, ComponentEraCandidate, ComponentEraEntryLedger,
    ComponentEraEntryReceipt, ComponentEraEntryState, ComponentEraLeaveReceipt,
    ComponentEraLedgerId, ComponentEraPublicationReceipt, ComponentEraQuiescenceReceipt,
    ComponentEraRetirementReceipt, EraEntryError, EraLeaveError, EraPublicationError,
    EraQuiescenceError, EraRetirementError, ProgramLocalRootEpochLease,
    ProgramLocalRootEpochLeaseAcquisitionError, ProgramLocalRootEpochLeaseId,
    ProgramLocalRootEpochLeaseReleaseError,
};
pub use provider_evidence::component_eras::component_progress_manifest::{
    CheckedComponentProgressDemand, ComponentBuildBoundProgressDemand, ComponentProgressManifest,
    ComponentProgressManifestDigest,
};
pub use provider_evidence::executable_scopes::coexisting_executable_eras::{
    AdmittedExecutableEra, AttributedContainmentEvidence, AttributedManifestCompleteness,
    CoexistingExecutableTcbEntry, CoexistingExecutableTcbReport, CoexistingExecutableTcbSet,
    CoexistingScopeCompleteness, ExecutableManifestSource,
};
pub use provider_evidence::executable_scopes::executable_tcb_manifest::{
    ContainmentEvidence, ContainmentGuarantee, ExecutableEntryOrigin, ExecutableIdentity,
    ExecutableTcbEntry, ExecutableTcbManifest, ExecutionScope, ImplementationEvidence,
    IncompleteCause, OmegaRuntimeExecutableAdmissionCandidate, OmegaRuntimeExecutableLedger,
    OpaqueClosureEvidence, OpaqueExecutableAdmissionCandidate, OpaqueInProcessBinding,
    ProviderIdentity, RuntimeExecutableClosureEvidence, ScopeCompleteness,
    SelectedProviderRequirement, ValidatedOpaqueExecutableAdmission,
};
pub use provider_evidence::executable_scopes::executable_tcb_profile::{
    ExactExecutableTcbAllowance, ExecutableTcbProfile, ExecutableTcbProfileAcceptance,
    ExecutableTcbProfileRejection, ExecutableTcbProfileViolation, IncompleteScopePolicy,
    evaluate_executable_tcb_profile,
};
pub use provider_evidence::executable_scopes::isolated_executable_scopes::{
    AdmittedIsolatedExecutableScope, ExecutableTcbManifestSet, IsolatedExecutableScopeCandidate,
};
pub use provider_evidence::executable_scopes::process_static_services::{
    ActiveServiceRegistration, AtomicServiceHandoverReceipt, ProcessStaticServiceContract,
    ProcessStaticServicePolicy, ProcessStaticServiceRegistry, ServiceHandoverCompletion,
    ServiceHandoverError, ServiceRegistrationCandidate, ServiceRegistrationError,
};
pub use provider_evidence::providers::boundary_call_audit::{
    BoundaryCallCoordinate, UnapprovedBoundaryCall, audit_boundary_provider_calls,
    build_boundary_provider_approval_registry,
};
pub use provider_evidence::providers::foreign_locator::{
    ForeignLocatorCandidate, ForeignLocatorIdentityDigest, ForeignLocatorValidationError,
    NormalizedForeignLocator, normalize_foreign_locator,
};
pub use provider_evidence::providers::provider_approval::{
    BoundaryCallApproval, BoundaryProviderApproval, BoundaryProviderApprovalRegistry,
};
pub use provider_evidence::providers::provider_plan;
pub use provider_evidence::{
    InstallationReachResolution, SelectedProviderClosureDigest, SelectedProviderPlanFacts,
};
