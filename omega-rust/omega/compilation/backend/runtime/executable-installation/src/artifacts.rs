/*
We define executable artifacts (`Artifact`, `AdmittedArtifact`), their decoded
entries (`ArtifactEntry`), normalized relocation records
(`ArtifactRelocationKind`, `DecodedArtifactRelocation`), and the target
installation audience (`InstallationAudience`).

Decoding an artifact produces an immutable candidate (`Artifact`) that grants
no placement or execute authority; only an `AdmittedArtifact` carrying a sealed
`AdmissionReceiptId` can resolve an entry identity into a `RelocationTarget`.
*/

use crate::authority_digests::ArtifactAuthorityCommitments;
use crate::authority_digests::NonAuthoritativeContainerFingerprint64;
use crate::authority_digests::{
    AccessPlanId, AdmissionReceiptId, ArtifactContentDigest, ArtifactId, EntrySetId,
    MachineContractSetId, MachineFootprintId, RelocationSetId,
};
use crate::installation::InstallationDiagnostic;
use layout_plans::{EntryStubId, PlacementConstraints, RelocationTarget};
use std::sync::Arc;
use target::Architecture;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtifactRelocationKind {
    Absolute64,
    X86Relative32,
    Aarch64Page21,
    Aarch64PageOffset12,
    Aarch64Branch26,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedArtifactRelocation {
    pub kind: ArtifactRelocationKind,
    pub destination_offset: u64,
    pub target: RelocationTarget, // Symbolic target; decoding never exposes a numeric code or data address.
    pub addend: i64,
}

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
    pub(crate) access_plan: AccessPlanId,
    pub(crate) placement_constraints: PlacementConstraints,
    pub(crate) entry_set: EntrySetId,
    pub(crate) entries: Vec<ArtifactEntry>,
    pub(crate) relocation_set: RelocationSetId,
    pub(crate) relocations: Vec<DecodedArtifactRelocation>,
    pub(crate) authority_commitments: Option<ArtifactAuthorityCommitments>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactEntry {
    pub(crate) identity: EntryStubId,
    pub(crate) code_offset: u64, // Sealed inside the installation/provider layer.
}

impl ArtifactEntry {
    pub const fn identity(self) -> EntryStubId {
        self.identity
    }

    pub const fn code_offset(self) -> u64 {
        self.code_offset
    }
}

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

    pub fn code(&self) -> &[u8] {
        &self.0.code
    }

    pub fn entries(&self) -> &[ArtifactEntry] {
        &self.0.entries
    }

    pub fn relocation_set(&self) -> RelocationSetId {
        self.0.relocation_set
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdmittedArtifact {
    pub(crate) artifact: Artifact,
    pub(crate) admission: AdmissionReceiptId,
}

impl AdmittedArtifact {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    dead_code,
    reason = "spec audiences (executable_installation.md#visibility-and-retirement) await TASKS.md COMPONENT-SUBSTRATE"
)]
pub(crate) enum InstallationAudience {
    DormantLocal,
    FutureFetcher,
    PossibleCurrentExecutor,
}
