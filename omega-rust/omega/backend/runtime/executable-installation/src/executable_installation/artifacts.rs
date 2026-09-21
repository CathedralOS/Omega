//! Executable artifacts, their entries, admission evidence and audiences.

use crate::DecodedArtifactRelocation;
use crate::executable_installation::authority_digests::NonAuthoritativeContainerFingerprint64;
pub(crate) use crate::executable_installation::derive_artifact_content_commitments;
use crate::executable_installation::validate_decoded_relocations;
use crate::executable_installation::{
    AdmissionReceiptId, ArtifactAuthorityCommitments, ArtifactContentDigest, ArtifactId,
    EntrySetId, InstallationDiagnostic, MachineContractSetId, MachineFootprintId, PlacementPlanId,
    ProofPayloadDigest, RelocationSetId,
};
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
    pub const fn from_canonical_decode(identity: EntryStubId, code_offset: u64) -> Self {
        Self {
            identity,
            code_offset,
        }
    }

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
    pub fn from_canonical_decode(
        identity: ArtifactId,
        architecture: Architecture,
        code: Vec<u8>,
        contracts: MachineContractSetId,
        declared_footprint: MachineFootprintId,
        placement_plan: PlacementPlanId,
        placement_constraints: PlacementConstraints,
        entry_set: EntrySetId,
        entries: Vec<ArtifactEntry>,
        relocation_set: RelocationSetId,
        relocations: Vec<DecodedArtifactRelocation>,
        authority_commitments: ArtifactAuthorityCommitments,
    ) -> Result<Self, InstallationDiagnostic> {
        Self::from_decoded_parts(
            identity,
            architecture,
            code,
            contracts,
            declared_footprint,
            placement_plan,
            placement_constraints,
            entry_set,
            entries,
            relocation_set,
            relocations,
            Some(authority_commitments),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_legacy_v1_decode(
        identity: ArtifactId,
        architecture: Architecture,
        code: Vec<u8>,
        contracts: MachineContractSetId,
        declared_footprint: MachineFootprintId,
        placement_plan: PlacementPlanId,
        placement_constraints: PlacementConstraints,
        entry_set: EntrySetId,
        entries: Vec<ArtifactEntry>,
        relocation_set: RelocationSetId,
        relocations: Vec<DecodedArtifactRelocation>,
    ) -> Result<Self, InstallationDiagnostic> {
        Self::from_decoded_parts(
            identity,
            architecture,
            code,
            contracts,
            declared_footprint,
            placement_plan,
            placement_constraints,
            entry_set,
            entries,
            relocation_set,
            relocations,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_decoded_parts(
        identity: ArtifactId,
        architecture: Architecture,
        code: Vec<u8>,
        contracts: MachineContractSetId,
        declared_footprint: MachineFootprintId,
        placement_plan: PlacementPlanId,
        placement_constraints: PlacementConstraints,
        entry_set: EntrySetId,
        mut entries: Vec<ArtifactEntry>,
        relocation_set: RelocationSetId,
        relocations: Vec<DecodedArtifactRelocation>,
        authority_commitments: Option<ArtifactAuthorityCommitments>,
    ) -> Result<Self, InstallationDiagnostic> {
        if authority_commitments.as_ref().is_some_and(|commitments| {
            !commitments.matches_report_coordinates(
                contracts,
                declared_footprint,
                placement_constraints,
            )
        }) {
            return Err(InstallationDiagnostic(
                "strong authority commitments do not match their compact report coordinates".into(),
            ));
        }
        if code.is_empty() {
            return Err(InstallationDiagnostic(
                "executable artifact cannot have empty content".into(),
            ));
        }
        let byte_length = u64::try_from(code.len()).map_err(|_| {
            InstallationDiagnostic(
                "executable artifact byte length cannot be represented by the container".into(),
            )
        })?;
        if entries.is_empty() {
            return Err(InstallationDiagnostic(
                "executable artifact must publish at least one selected entry".into(),
            ));
        }
        entries.sort_unstable_by_key(|entry| entry.identity);
        for entry in &entries {
            if entry.code_offset >= byte_length {
                return Err(InstallationDiagnostic(format!(
                    "artifact entry {:?} offset {} lies outside {} code bytes",
                    entry.identity, entry.code_offset, byte_length
                )));
            }
            if matches!(architecture, Architecture::Aarch64) && entry.code_offset % 4 != 0 {
                return Err(InstallationDiagnostic(format!(
                    "AArch64 artifact entry {:?} offset {} is not instruction-aligned",
                    entry.identity, entry.code_offset
                )));
            }
        }
        if entries
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err(InstallationDiagnostic(
                "artifact entry identities must be unique".into(),
            ));
        }
        let relocations =
            validate_decoded_relocations(relocations, architecture, byte_length, usize::MAX)?;
        let (content, container_fingerprint) = derive_artifact_content_commitments(
            architecture,
            &code,
            contracts,
            declared_footprint,
            placement_plan,
            placement_constraints,
            entry_set,
            &entries,
            relocation_set,
            &relocations,
            authority_commitments.as_ref(),
        )?;
        Ok(Self(Arc::new(ArtifactRecord {
            identity,
            content,
            container_fingerprint,
            architecture,
            byte_length,
            code,
            contracts,
            declared_footprint,
            placement_plan,
            placement_constraints,
            entry_set,
            entries,
            relocation_set,
            relocations,
            authority_commitments,
        })))
    }

    pub fn identity(&self) -> ArtifactId {
        self.0.identity
    }

    pub fn content(&self) -> ArtifactContentDigest {
        self.0.content
    }

    /// Legacy container-v1 checksum retained only for wire compatibility. It
    /// is never sufficient for admission, replay, or installation authority.
    pub fn non_authoritative_container_fingerprint(
        &self,
    ) -> NonAuthoritativeContainerFingerprint64 {
        self.0.container_fingerprint
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

    pub fn placement_constraints(&self) -> PlacementConstraints {
        self.0.placement_constraints
    }

    pub fn entry_set(&self) -> EntrySetId {
        self.0.entry_set
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

    pub fn authority_commitments(&self) -> Option<&ArtifactAuthorityCommitments> {
        self.0.authority_commitments.as_ref()
    }

    pub(crate) fn entry(&self, identity: EntryStubId) -> Option<ArtifactEntry> {
        self.0
            .entries
            .binary_search_by_key(&identity, |entry| entry.identity)
            .ok()
            .map(|index| self.0.entries[index])
    }
}

/// Validator-authored evidence for the reusable executable qualification.
/// This is the normalized receipt carried by provider admission, not something
/// an Omega package can construct.
#[derive(Debug, PartialEq, Eq)]
pub struct ArtifactAdmissionEvidence {
    pub(crate) receipt: AdmissionReceiptId,
    pub(crate) artifact: Artifact,
    pub(crate) accepted: bool,
}

impl ArtifactAdmissionEvidence {
    pub fn from_validator(
        receipt: AdmissionReceiptId,
        artifact: &Artifact,
        accepted: bool,
    ) -> Self {
        Self {
            receipt,
            artifact: artifact.clone(),
            accepted,
        }
    }
}

/// Reusable immutable artifact carrying the sealed `AdmittedExecutable` fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedArtifact {
    pub(crate) artifact: Artifact,
    pub(crate) admission: AdmissionReceiptId,
    pub(crate) container_proof: Option<RetainedContainerProof>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetainedContainerProof {
    pub(crate) digest: ProofPayloadDigest,
    pub(crate) bytes: Vec<u8>,
}

impl AdmittedArtifact {
    pub const fn artifact(&self) -> &Artifact {
        &self.artifact
    }

    pub const fn admission(&self) -> AdmissionReceiptId {
        self.admission
    }

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
pub enum InstallationAudience {
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
