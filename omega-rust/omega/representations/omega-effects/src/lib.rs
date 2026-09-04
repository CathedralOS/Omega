//! Retained evidence, not transforms. Which provider realizes each boundary
//! requirement, what physical authority that realization exercises, and which
//! executable bytes end up in a scope - each as a carrier with a canonical byte
//! encoding somebody downstream will rehash.
//!
//! Because everything here is hashed, the encoding rules are the interface, and
//! two of them will surprise you.
//!
//! **Endianness splits by file, cleanly, and nobody says so.**
//! `terminal_authority.rs` writes big-endian at all 14 of its sites.
//! `capabilities/provider_plan.rs` (17 sites), `selected_provider_plans.rs` (9)
//! and `component_progress_manifest.rs` (10) write little-endian at all 36 of
//! theirs. No file mixes the two. Hash the same `u64` in `terminal_authority.rs`
//! and in `provider_plan.rs` and you get two different byte strings, which is
//! fine as long as nothing ever moves a hashing helper between them.
//!
//! **Declaration order is the wire format.** `TerminalAuthorityClass` has 14
//! variants and `canonical_tag()` returns the declaration index for each, which
//! `terminal_authority_class_order_matches_canonical_tags` asserts over every
//! one. Reordering that enum for readability silently rewrites every stored
//! disposition. The compiler-intrinsic family tags are the opposite case and
//! deliberately NOT in declaration order - `LinuxExitGroupI32` is 0,
//! `BuiltinFunction` 1, and `LinuxWriteByteI32` 5 - because those were assigned
//! as families were added. Role tags for `terminal_mechanism_identity_bytes`
//! follow the same append-only rule: `CompilerIntrinsic` 0, `NormalizedForeign`
//! 1, `CheckedPhysical` 2, and `Syscall` took 3 as the fourth role.
//!
//! Six NUL-terminated domain prefixes separate the digest families. Five carry a
//! `.sha256` segment; `omega.terminal-authority.closure-review.v1\0` does not,
//! and still feeds SHA-256.

//! Every identity is carried twice, as a compact FNV-1a `u64` beside a SHA-256
//! digest, and a decision has to present both. The compact one is not a weaker
//! hash of the same thing - it is provably lossy by construction, because the
//! FNV rendering never writes `requirement_owner` or `calling_plan_commitment`,
//! so two structurally different plans genuinely share a report coordinate.
//! That is why it is spelled as a coordinate everywhere and never as authority.
//!
//! `SelectedProviderRequirement` has a hand-written `PartialEq` that compares
//! `requirement_identity` and ignores `method`. Deriving it is what you would do
//! by reflex and it breaks two things at once: renaming a readable method label
//! would reject a TCB allowance that is otherwise identical, and
//! `known_entries.contains(&entry)` would stop deduplicating rows that describe
//! the same requirement. An overload-identity change still rejects, because that
//! lives in `requirement_identity`.
//!
//! `TerminalAuthorityPolicyIdentity` and
//! `TerminalAuthorityPermissionPolicyIdentity` have byte-identical layout - a
//! `u32` version and a 32-byte commitment - identical method sets, and are two
//! types anyway. The closure-review identity hashes them in fixed positional
//! order, so passing a physical-classification policy where a service-permission
//! policy belongs would produce a well-formed review of the wrong thing with no
//! error anywhere. Same shape as the carrier split in `omega-runtime-abi`, same
//! reason: identical layout is exactly when the type system is the only thing
//! left to catch a swap.
//!
//! The closure-leaf uniqueness key excludes the mechanism while the sort order
//! includes it. `same_closure_leaf_key` compares only service schema,
//! requirement identity and provider plan; `compare_closure_leaves` breaks ties
//! on `terminal_mechanism_identity_bytes`. Adding the mechanism to the key
//! reads like a tightening and is the opposite: it would let one requirement
//! under one provider plan report two different physical mechanisms and still
//! pass review as two distinct leaves.

//! Consumed by `omega-visualizations` (which renders these carriers as JSON
//! manifests) and `omega-artifacts` (which writes them beside a build).
//!
//! @Incomplete: `derive_static_manifest` maps `ProviderBinding::Syscall` to
//! `None` - neither a known executable entry nor an attributed incompleteness
//! cause. Every other non-checked binding (`Import`,
//! `StringBackedImportBootstrap`, `VtableSlot`, `VtableField`, `TableFunction`)
//! produces one or the other. A syscall row therefore vanishes from the
//! manifest silently, and a TCB manifest that omits syscalls understates the
//! trusted computing base rather than reporting that it could not describe it.
//!
//! @Note: `RuntimeExecutableClosureEvidence` looks like live output from here
//! and is not reachable in production. Its only producer is
//! `OmegaRuntimeExecutableLedger`, whose single use outside this crate sits in
//! `omega-visualizations/src/executable_tcb_manifest.rs:661`, below that file's
//! `#[cfg(test)]`. The CONSUMER at line 190 of the same file is production and
//! is reached from `omega-compiler/src/pipeline/artifacts.rs`, so a real
//! compilation runs a JSON writer over an evidence slice that is always empty.
//! Neither crate can see that on its own: from here the type has a production
//! consumer, and from there it has a producer.

mod capabilities;
mod coexisting_executable_eras;
mod component_era_entry_ledger;
mod component_progress_manifest;
mod executable_tcb_manifest;
mod executable_tcb_profile;
mod isolated_executable_scopes;
mod process_static_services;
mod selected_provider_plans;
mod service_terminal_authority_permission;
mod terminal_authority;

pub use capabilities::analysis::{
    BoundaryCallCoordinate, UnapprovedBoundaryCall, audit_boundary_provider_calls,
    build_boundary_provider_approval_registry,
};
pub use capabilities::foreign_locator::{
    ForeignLocatorCandidate, ForeignLocatorIdentityDigest, ForeignLocatorValidationError,
    NormalizedForeignLocator, normalize_foreign_locator,
};
pub use capabilities::provider_approval::{
    BoundaryCallApproval, BoundaryProviderApproval, BoundaryProviderApprovalRegistry,
};
pub use capabilities::provider_plan;
pub use coexisting_executable_eras::{
    AdmittedExecutableEra, AttributedContainmentEvidence, AttributedManifestCompleteness,
    CoexistingExecutableTcbEntry, CoexistingExecutableTcbReport, CoexistingExecutableTcbSet,
    CoexistingScopeCompleteness, ExecutableManifestSource,
};
pub use component_era_entry_ledger::{
    ActiveComponentEraEntry, ComponentEraCandidate, ComponentEraEntryLedger,
    ComponentEraEntryReceipt, ComponentEraEntryState, ComponentEraLeaveReceipt,
    ComponentEraLedgerId, ComponentEraPublicationReceipt, ComponentEraQuiescenceReceipt,
    ComponentEraRetirementReceipt, EraEntryError, EraLeaveError, EraPublicationError,
    EraQuiescenceError, EraRetirementError, ProgramLocalRootEpochLease,
    ProgramLocalRootEpochLeaseAcquisitionError, ProgramLocalRootEpochLeaseId,
    ProgramLocalRootEpochLeaseReleaseError,
};
pub use component_progress_manifest::{
    CheckedComponentProgressDemand, ComponentBuildBoundProgressDemand, ComponentProgressManifest,
    ComponentProgressManifestDigest,
};
pub use executable_tcb_manifest::{
    ContainmentEvidence, ContainmentGuarantee, ExecutableEntryOrigin, ExecutableIdentity,
    ExecutableTcbEntry, ExecutableTcbManifest, ExecutionScope, ImplementationEvidence,
    IncompleteCause, OmegaRuntimeExecutableAdmissionCandidate, OmegaRuntimeExecutableLedger,
    OpaqueClosureEvidence, OpaqueExecutableAdmissionCandidate, OpaqueInProcessBinding,
    ProviderIdentity, RuntimeExecutableClosureEvidence, ScopeCompleteness,
    SelectedProviderRequirement, ValidatedOpaqueExecutableAdmission,
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
pub use selected_provider_plans::{
    InstallationReachResolution, SelectedProviderClosureDigest, SelectedProviderPlanFacts,
};
pub use service_terminal_authority_permission::ServiceTerminalAuthorityPermission;
pub use terminal_authority::{
    CheckedPhysicalOperationIdentity, CheckedPhysicalTerminalMechanismIdentity,
    CheckedSyscallArgumentContractIdentity, CompilerIntrinsicExecutionIdentity,
    CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    NormalizedForeignTerminalMechanismIdentity, PortableFilesystemAuthorityFacet,
    SyscallTerminalMechanismIdentity, TerminalAuthorityClass, TerminalAuthorityClosureLeaf,
    TerminalAuthorityClosureReviewBuildError, TerminalAuthorityClosureReviewReceipt,
    TerminalAuthorityDisposition, TerminalAuthorityPermissionPolicyIdentity,
    TerminalAuthorityPolicyIdentity, TerminalMechanismIdentity,
    compiler_intrinsic_execution_identity_bytes, terminal_mechanism_identity_bytes,
};
