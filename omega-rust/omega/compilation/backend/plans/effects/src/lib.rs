/*
We retain the selected provider realization evidence that accompanies a
compiled program across lowering, artifact replay, and target installation.
Starting at `provider_evidence.rs`, `SelectedProviderPlanFacts` commits the
exact selected provider plans, execution scope, assumed binding selections,
specialization and family realization joins, and installation reach resolutions.
Subordinate modules record provider plans and canonical encodings
(`provider_evidence/providers/`), terminal mechanism classifications and
service permissions (`provider_evidence/authority.rs`), executable scope
admission and source-free behavior exclusion replay
(`provider_evidence/executable_scopes.rs`), and component era ledgers
(`provider_evidence/component_eras.rs`).
*/

/*
We keep realization evidence separate from the source language's flow and
effect analysis so downstream native stages can replay and audit exact
provider selections without re-evaluating Build declarations or loading source
text. Compact 64-bit report fingerprints remain diagnostic and compatibility
coordinates; admission and replay always check the retained plans and
domain-separated SHA-256 commitments directly.
*/

/*
These records form the backend realization contract consumed by compiler
admission, terminal installation, and native code generation:
- `SelectedProviderPlanFacts` and `ProviderPlan`: exact selected provider
  closures and per-slot binding rows.
- `TerminalMechanismIdentity` and `TerminalAuthorityClosureReviewReceipt`:
  role-tagged physical mechanism identities and closure review verdicts.
- `BehaviorExclusions` and `establish_behavior_exclusions`: source-free
  selected-composition absence checking over unoptimized Terminal behavior and
  reviewed physical mechanisms.
*/

pub mod external_providers;
mod provider_evidence;
pub mod provider_grants;

pub use provider_evidence::executable_scopes::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind, BehaviorExclusionReport,
    BehaviorExclusionVerdict, BehaviorExclusions, EvidenceGapKind,
    authored_behavior_exclusion_set_in, establish_behavior_exclusions,
    resolve_authored_behavior_exclusion,
};

pub(crate) use provider_evidence::SelectedProviderClosureDigest;
pub use provider_evidence::authority::{
    CheckedPhysicalOperationIdentity, CheckedPhysicalTerminalMechanismIdentity,
    CheckedSyscallArgumentContractIdentity, CompilerAccessorScalarOperation,
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation, NormalizedForeignArgumentContract,
    NormalizedForeignTerminalMechanismIdentity, PortableFilesystemAuthorityFacet,
    ServiceTerminalAuthorityPermission, SyscallTerminalMechanismIdentity, TerminalAuthorityClass,
    TerminalAuthorityClosureCustody, TerminalAuthorityClosureLeaf,
    TerminalAuthorityClosureLeafOrigin, TerminalAuthorityClosureReviewReceipt,
    TerminalAuthorityDisposition, TerminalAuthorityPermissionPolicyIdentity,
    TerminalAuthorityPolicyIdentity, TerminalMechanismIdentity,
    compiler_accessor_scalar_contract_identity, compiler_intrinsic_execution_identity_bytes,
    primitive_float_operation_tag, primitive_integer_comparison_operation_tag,
    terminal_mechanism_identity_bytes,
};
pub use provider_evidence::component_eras::{
    EraEntryError, EraQuiescenceError, EraRetirementError,
};
pub(crate) use provider_evidence::executable_scopes::ExecutionScope;
pub use provider_evidence::providers::foreign_locator::{
    ForeignLocatorCandidate, ForeignLocatorIdentityDigest, ForeignLocatorValidationError,
    NormalizedForeignLocator, normalize_foreign_locator,
};
pub use provider_evidence::providers::provider_plan;
pub use provider_evidence::{
    AssumedBindingContractSelection, InstallationReachResolution,
    SelectedExternalRequirementApplication, SelectedProviderPlanCandidate,
    SelectedProviderPlanFacts, SelectedRequirementSpecialization, SelectedTargetFamilyRealization,
};

#[cfg(test)]
mod source_guards;
