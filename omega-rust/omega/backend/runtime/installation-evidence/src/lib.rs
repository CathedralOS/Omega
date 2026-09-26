#![forbid(unsafe_code)]

//! Read-only projections of admitted installation evidence consumed below
//! Omega orchestration.
//!
//! These traits carry no constructors and grant no authority. Orchestration
//! owns the sealed values that implement them; lowering and image emission can
//! inspect only the exact identities required to preserve those decisions.

use std::collections::BTreeSet;

/// Collision-resistant commitment to one exact installed executable
/// occurrence. The executable-installation owner derives the bytes from its
/// private artifact, placement, final-byte, and provider evidence; lower
/// lifecycle representations only retain and compare the commitment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstalledArtifactOccurrenceDigest([u8; 32]);

impl InstalledArtifactOccurrenceDigest {
    pub const fn from_sha256(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact admitted provider-execution identity projected into terminal
/// lowering and installation records.
pub trait ProviderExecutionEvidence: std::fmt::Debug {
    /// Canonical terminal requirement identity selected by this admitted
    /// execution. Lowering compares this value with the exact bodyless
    /// boundary declaration before it projects the numeric execution record.
    fn requirement_identity(&self) -> &str;
    fn provider_plan_report_identity(&self) -> u64;
    fn provider_execution_report_identity(&self) -> u64;
    fn provider_execution_report_fingerprint(&self) -> u64;
    fn normalized_root_report_identity(&self) -> u64;
    fn boundary_contract_report_fingerprint(&self) -> u64;
}

/// Exact caller-local claim source retained by one admitted installed-provider
/// call. This mirrors terminal abstract custody without making the
/// installation evidence crate depend on an Omega lowering representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProviderCompletionClaimSource {
    pub claim: semantic_vocabulary::ClaimId,
    pub entry: Option<terminal_psi::EntryClaim>,
    pub content: Option<terminal_psi::ContentEntryClaim>,
}

/// Read-only projection of one boundary occurrence admitted as a call to an
/// exact checked provider machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProviderCallEvidence {
    pub caller: semantic_vocabulary::MachineId,
    pub psi_operation: semantic_vocabulary::OperationId,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub provider: terminal_psi::ProviderCandidateConformance,
    /// Exact caller-local result established by successful provider completion.
    pub result: terminal_psi::OperationResult,
    /// Ordered scalar values supplied by the exact admitted call occurrence.
    pub scalar_arguments: Vec<semantic_vocabulary::ValueId>,
    pub structural_arguments: Vec<terminal_psi::StructuralArgument>,
    pub completion_claim_sources: Vec<InstalledProviderCompletionClaimSource>,
    pub completion_receipts: Vec<terminal_psi::CompletionReceipt>,
}

/// Dependency-light view of an opaque provider installation admitted against
/// one exact terminal-Psi artifact. Returning owned projections exposes no
/// mutable installation map or construction authority.
pub trait ProviderInstallationEvidence: std::fmt::Debug {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity;
    fn installed_provider_calls(&self) -> Vec<InstalledProviderCallEvidence>;
}

/// Read-only projection of one opaque, installation-owned component progress
/// acceptance. These compact identities enter the terminal installation
/// record and artifact fingerprint, but never substitute for the retained
/// acceptance value at publication.
pub trait ComponentProgressAcceptanceEvidence: std::fmt::Debug {
    fn component_progress_manifest_identity(&self) -> u64;
    fn component_progress_acceptance_identity(&self) -> u64;
}

/// Relocation-free terminal object facts required to bind installed entry and
/// fixed-fuel evidence.
pub trait ObjectEvidence {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity;
    fn target(&self) -> target::NativeTarget;
    fn architecture(&self) -> target::Architecture {
        self.target().architecture
    }
    fn text_bytes(&self) -> &[u8];
    fn function_text_offset(&self, machine: semantic_vocabulary::MachineId) -> Option<usize>;
}

/// Emitter-derived stack closure for one terminal entry.
pub trait StackDemandEvidence {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity;
    fn architecture(&self) -> target::Architecture;
    fn entry(&self) -> semantic_vocabulary::MachineId;
    fn ceiling_bytes(&self) -> u64;
    fn stack_alignment(&self) -> u32;
    fn contributing_machines(&self) -> &BTreeSet<semantic_vocabulary::MachineId>;
    /// Non-authoritative compact report coordinates for every admitted opaque
    /// same-stack leaf contributing to this exact closure.
    fn admitted_stack_contribution_report_identities(&self) -> BTreeSet<u64>;
    /// Strong commitments to the complete admitted opaque same-stack claims.
    /// These remain distinct from the compact report coordinates above.
    fn admitted_stack_contribution_commitments(&self) -> BTreeSet<[u8; 32]>;
}
