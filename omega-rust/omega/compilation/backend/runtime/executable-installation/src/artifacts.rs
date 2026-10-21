//! Executable artifacts, their entries, admission evidence and audiences.
//!
//! `container.rs` owns the decoded container — validation of its sections
//! and relocations, and admission of a validated container into an
//! [`AdmittedArtifact`] — and `container_bytes.rs` the wire format that
//! encodes and decodes one.

pub(crate) mod container;
use crate::artifacts::container::DecodedArtifactRelocation;
use crate::authority_digests::ArtifactAuthorityCommitments;
use crate::authority_digests::NonAuthoritativeContainerFingerprint64;
use crate::authority_digests::{
    AdmissionReceiptId, ArtifactContentDigest, ArtifactId, EntrySetId, MachineContractSetId,
    MachineFootprintId, PlacementPlanId, RelocationSetId,
};
use crate::installation::InstallationDiagnostic;
use layout_plans::{EntryStubId, PlacementConstraints, RelocationTarget};
use std::sync::Arc;
use target::Architecture;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ArtifactRecord {
    pub(crate) identity: ArtifactId,
    pub(crate) content: ArtifactContentDigest,
    pub(crate) container_fingerprint: NonAuthoritativeContainerFingerprint64,
    pub(crate) architecture: Architecture,
    pub(crate) byte_length: u64,
    pub(crate) code: Vec<u8>,
    pub(crate) contracts: MachineContractSetId,
    pub(crate) declared_footprint: MachineFootprintId,
    pub(crate) placement_plan: PlacementPlanId,
    pub(crate) placement_constraints: PlacementConstraints,
    pub(crate) entry_set: EntrySetId,
    pub(crate) entries: Vec<ArtifactEntry>,
    pub(crate) relocation_set: RelocationSetId,
    pub(crate) relocations: Vec<DecodedArtifactRelocation>,
    pub(crate) authority_commitments: Option<ArtifactAuthorityCommitments>,
}

/// Canonically decoded entry in one executable artifact. The offset remains
/// sealed inside the installation/provider layer; ordinary Omega code sees at
/// most the compiler-issued [`EntryStubId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactEntry {
    pub(crate) identity: EntryStubId,
    pub(crate) code_offset: u64,
}

impl ArtifactEntry {
    pub const fn identity(self) -> EntryStubId {
        self.identity
    }

    pub const fn code_offset(self) -> u64 {
        self.code_offset
    }
}

/// Immutable canonical decode result. Construction grants no executable
/// eligibility; it is merely the candidate consumed by admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact(pub(crate) Arc<ArtifactRecord>);

impl Artifact {
    pub fn identity(&self) -> ArtifactId {
        self.0.identity
    }

    pub fn content(&self) -> ArtifactContentDigest {
        self.0.content
    }

    pub fn architecture(&self) -> Architecture {
        self.0.architecture
    }

    pub fn byte_length(&self) -> u64 {
        self.0.byte_length
    }

    /// Exact immutable executable bytes bound by this artifact's normalized
    /// content identity. This provider-side projection grants neither
    /// placement nor execute authority.
    pub fn code(&self) -> &[u8] {
        &self.0.code
    }

    pub fn entries(&self) -> &[ArtifactEntry] {
        &self.0.entries
    }

    pub fn relocation_set(&self) -> RelocationSetId {
        self.0.relocation_set
    }

    /// Canonical destination-ordered relocation commitments retained through
    /// admission for the eventual provider materializer. Targets remain sealed
    /// identities; this projection grants no address resolver.
    pub fn relocations(&self) -> &[DecodedArtifactRelocation] {
        &self.0.relocations
    }

    pub(crate) fn entry(&self, identity: EntryStubId) -> Option<ArtifactEntry> {
        self.0
            .entries
            .binary_search_by_key(&identity, |entry| entry.identity)
            .ok()
            .map(|index| self.0.entries[index])
    }
}

/// Reusable immutable artifact carrying the sealed `AdmittedExecutable` fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdmittedArtifact {
    pub(crate) artifact: Artifact,
    pub(crate) admission: AdmissionReceiptId,
}

impl AdmittedArtifact {
    /// Selects a sealed materialization target only when the requested entry
    /// belongs to this admitted artifact's canonical, admission-bound entry
    /// set. Numeric address resolution remains deferred until materialization
    /// has bound the artifact to one exact placement.
    pub fn selected_entry_target(
        &self,
        identity: EntryStubId,
    ) -> Result<RelocationTarget, InstallationDiagnostic> {
        self.artifact
            .entry(identity)
            .map(|_| RelocationTarget::Entry(identity))
            .ok_or_else(|| {
                InstallationDiagnostic(format!(
                    "entry {identity:?} is not present in admitted artifact {:?}",
                    self.artifact.0.identity
                ))
            })
    }
}

/// The audience an admitted installation is scoped to
/// (wiki/spec/build/executable_installation.md#visibility-and-retirement).
/// The audience binds into placement evidence, so a receipt minted for one
/// audience cannot satisfy a validated placement scoped to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallationAudience {
    /// Local installation completion only: the realization has no remote
    /// fetcher and no current executor.
    DormantLocal,
    /// A remote fetcher may enter later: instruction-fetch visibility must
    /// complete before entry.
    FutureFetcher,
    /// An executor may already be running this realization: the sanctioned
    /// alteration route is the live patch-then-drain replacement join, and
    /// retirement still proves quiescence before returning the placement.
    PossibleCurrentExecutor,
}
