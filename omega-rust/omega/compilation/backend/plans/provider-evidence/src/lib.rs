//! Selected provider evidence retained across lowering, artifact replay and installation.
//!
//! Start at `provider_evidence.rs`: `SelectedProviderPlanFacts` retains the exact
//! selected plan set, execution scope, opaque admissions and installation reach.
//! Its identity commits those records; compact report coordinates never replace
//! the retained plans or strong commitments during replay.
//!
//! `provider_evidence/providers/` owns provider plans and their canonical encoding.
//! `authority/` owns terminal mechanism classifications
//! and service permissions; `executable_scopes/` owns executable manifests and
//! scope admission, including source-free selected-composition exclusion replay;
//! `component_eras/` owns era ledgers and progress evidence.
//! These are realization records, not the source language's effect analysis.

mod fnv1a;
mod provider_evidence;
pub mod provider_grants;

pub use provider_evidence::executable_scopes::behavior_exclusions::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind, BehaviorExclusionReport,
    BehaviorExclusionVerdict, BehaviorExclusions, authored_behavior_exclusion_set_in,
    establish_behavior_exclusions,
};

pub(crate) use provider_evidence::SelectedProviderClosureDigest;
pub use provider_evidence::authority::service_terminal_authority_permission::ServiceTerminalAuthorityPermission;
pub use provider_evidence::authority::terminal_authority::{
    CheckedPhysicalOperationIdentity, CheckedPhysicalTerminalMechanismIdentity,
    CheckedSyscallArgumentContractIdentity, CompilerIntrinsicExecutionIdentity,
    CompilerNumericType, CompilerPlacedScalarOperation, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation, NormalizedForeignArgumentContract,
    NormalizedForeignTerminalMechanismIdentity, PortableFilesystemAuthorityFacet,
    SyscallTerminalMechanismIdentity, TerminalAuthorityClass, TerminalAuthorityClosureCustody,
    TerminalAuthorityClosureLeaf, TerminalAuthorityClosureLeafOrigin,
    TerminalAuthorityClosureReviewReceipt, TerminalAuthorityDisposition,
    TerminalAuthorityPermissionPolicyIdentity, TerminalAuthorityPolicyIdentity,
    TerminalMechanismIdentity, compiler_intrinsic_execution_identity_bytes,
    compiler_placed_scalar_contract_identity, numeric_type_tag, primitive_float_operation_tag,
    primitive_integer_comparison_operation_tag, terminal_mechanism_identity_bytes,
};
pub use provider_evidence::component_eras::component_era_entry_ledger::{
    EraEntryError, EraQuiescenceError, EraRetirementError,
};
pub use provider_evidence::component_eras::component_progress_manifest::{
    CheckedComponentProgressDemand, ComponentProgressManifest,
};
pub(crate) use provider_evidence::executable_scopes::behavior_exclusions::BehaviorExclusion;
pub(crate) use provider_evidence::executable_scopes::executable_tcb_manifest::{
    ContainmentGuarantee, ExecutionScope, OpaqueExecutableAdmissionCandidate,
    OpaqueInProcessBinding, ValidatedOpaqueExecutableAdmission,
};
pub use provider_evidence::providers::foreign_locator::{
    ForeignLocatorCandidate, ForeignLocatorIdentityDigest, ForeignLocatorValidationError,
    NormalizedForeignLocator, normalize_foreign_locator,
};
pub use provider_evidence::providers::provider_plan;
pub use provider_evidence::{
    InstallationReachResolution, SelectedExternalRequirementApplication, SelectedProviderPlanFacts,
    SelectedRequirementSpecialization, SelectedTargetFamilyRealization,
};

#[cfg(test)]
mod source_guards;
