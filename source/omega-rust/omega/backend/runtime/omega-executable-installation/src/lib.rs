//! Normalized executable-artifact admission and installation ladder.
//!
//! No operation converts arbitrary bytes into executable memory. Immutable
//! artifacts are admitted once and reused; each installation instead consumes
//! one exact destination authority through frozen and validated states.

use std::sync::Arc;

use omega_installation_evidence::InstalledArtifactOccurrenceDigest;
use omega_target::Architecture;
use psi_extents::{
    AddressSpaceId, Extent, ExtentProvenanceId, ExtentRights, MappedExtent, MappingReceiptContext,
};
use psi_layout_plans::{
    EntryStubId, MaterializationDiagnostic, POST_HANDOFF_WRITER_CONTEXT_ABI_V1,
    PlacementConstraints, PlacementSite, PostHandoffWriterInvocationPlan, PostHandoffWriterPlan,
    PostHandoffWriterSource, PostHandoffWriterSourceSlot, RelocationTarget,
};
use sha2::{Digest, Sha256};

macro_rules! normalized_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, InstallationDiagnostic> {
                if identity == 0 {
                    return Err(InstallationDiagnostic(format!(
                        "normalized {} identity cannot be zero",
                        $label
                    )));
                }
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

normalized_id!(ArtifactId, "artifact");
normalized_id!(MachineContractSetId, "machine-contract-set");
normalized_id!(MachineFootprintId, "machine-footprint");
normalized_id!(PlacementPlanId, "placement-plan");
normalized_id!(EntrySetId, "entry-set");
normalized_id!(AdmissionReceiptId, "admission-receipt");
normalized_id!(CodePlacementId, "code-placement");
normalized_id!(InstallationScopeId, "installation-scope");
normalized_id!(FinalValidationId, "final-validation");
normalized_id!(InstalledCodeId, "installed-code");
normalized_id!(MappingQuarantineId, "mapping-quarantine");
normalized_id!(RelocationSetId, "relocation-set");
normalized_id!(
    DestinationPreparationReceiptId,
    "destination-preparation-receipt"
);

macro_rules! normalized_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub(crate) const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn digest(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

normalized_digest!(ArtifactContentDigest);
normalized_digest!(ProofPayloadDigest);
normalized_digest!(FinalBytesDigest);
normalized_digest!(RetirementFactDigest);

macro_rules! canonical_authority_digest {
    ($name:ident, $domain:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            fn from_report_identity_and_canonical_bytes(
                report_identity: u64,
                canonical: &[u8],
            ) -> Self {
                let mut digest = Sha256::new();
                digest.update($domain);
                digest.update(report_identity.to_le_bytes());
                digest.update((canonical.len() as u64).to_le_bytes());
                digest.update(canonical);
                Self(digest.finalize().into())
            }

            pub(crate) const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

canonical_authority_digest!(
    ImportedContractSetDigest,
    b"omega.imported-contract-set.sha256.v1\0"
);
canonical_authority_digest!(
    DeclaredFootprintDigest,
    b"omega.declared-machine-footprint.sha256.v1\0"
);
canonical_authority_digest!(MachineRegimeDigest, b"omega.machine-regime.sha256.v1\0");
canonical_authority_digest!(
    InstallationScopeDigest,
    b"omega.artifact-installation-scope.sha256.v1\0"
);

/// Collision-resistant commitments to the exact authority-bearing values
/// imported by an executable artifact. The compact normalized identities
/// remain report coordinates and are included in each digest's domain-framed
/// preimage; they are never sufficient on their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactAuthorityCommitments {
    imported_contract_report_identity: u64,
    imported_contracts: ImportedContractSetDigest,
    declared_footprint_report_identity: u64,
    declared_footprint: DeclaredFootprintDigest,
    machine_regime_report_identity: u64,
    machine_regime: MachineRegimeDigest,
    installation_scope_report_identity: u64,
    installation_scope: InstallationScopeDigest,
}

impl ArtifactAuthorityCommitments {
    #[allow(clippy::too_many_arguments)]
    pub fn from_canonical_evidence(
        contracts: MachineContractSetId,
        contract_bytes: &[u8],
        footprint: MachineFootprintId,
        footprint_bytes: &[u8],
        regime: Option<(psi_layout_plans::MachineRegimeId, &[u8])>,
        scope: Option<(psi_layout_plans::ArtifactInstallationScopeId, &[u8])>,
    ) -> Self {
        let (regime_identity, regime_bytes) = regime
            .map(|(identity, bytes)| (identity.normalized_identity(), bytes))
            .unwrap_or((0, &[]));
        let (scope_identity, scope_bytes) = scope
            .map(|(identity, bytes)| (identity.normalized_identity(), bytes))
            .unwrap_or((0, &[]));
        Self {
            imported_contract_report_identity: contracts.normalized_identity(),
            imported_contracts: ImportedContractSetDigest::from_report_identity_and_canonical_bytes(
                contracts.normalized_identity(),
                contract_bytes,
            ),
            declared_footprint_report_identity: footprint.normalized_identity(),
            declared_footprint: DeclaredFootprintDigest::from_report_identity_and_canonical_bytes(
                footprint.normalized_identity(),
                footprint_bytes,
            ),
            machine_regime_report_identity: regime_identity,
            machine_regime: MachineRegimeDigest::from_report_identity_and_canonical_bytes(
                regime_identity,
                regime_bytes,
            ),
            installation_scope_report_identity: scope_identity,
            installation_scope: InstallationScopeDigest::from_report_identity_and_canonical_bytes(
                scope_identity,
                scope_bytes,
            ),
        }
    }

    pub const fn imported_contracts(&self) -> ImportedContractSetDigest {
        self.imported_contracts
    }

    pub const fn declared_footprint(&self) -> DeclaredFootprintDigest {
        self.declared_footprint
    }

    pub const fn machine_regime(&self) -> MachineRegimeDigest {
        self.machine_regime
    }

    pub const fn installation_scope(&self) -> InstallationScopeDigest {
        self.installation_scope
    }

    pub(crate) fn from_decoded_digests(
        contracts: MachineContractSetId,
        footprint: MachineFootprintId,
        placement: PlacementConstraints,
        imported_contracts: [u8; 32],
        declared_footprint: [u8; 32],
        machine_regime: [u8; 32],
        installation_scope: [u8; 32],
    ) -> Result<Self, InstallationDiagnostic> {
        if [
            imported_contracts,
            declared_footprint,
            machine_regime,
            installation_scope,
        ]
        .contains(&[0; 32])
        {
            return Err(InstallationDiagnostic(
                "executable-container authority commitments cannot be zero".into(),
            ));
        }
        Ok(Self {
            imported_contract_report_identity: contracts.normalized_identity(),
            imported_contracts: ImportedContractSetDigest::from_digest(imported_contracts),
            declared_footprint_report_identity: footprint.normalized_identity(),
            declared_footprint: DeclaredFootprintDigest::from_digest(declared_footprint),
            machine_regime_report_identity: placement
                .machine_regime()
                .map_or(0, |identity| identity.normalized_identity()),
            machine_regime: MachineRegimeDigest::from_digest(machine_regime),
            installation_scope_report_identity: placement
                .installation_scope()
                .map_or(0, |identity| identity.normalized_identity()),
            installation_scope: InstallationScopeDigest::from_digest(installation_scope),
        })
    }

    fn matches_report_coordinates(
        &self,
        contracts: MachineContractSetId,
        footprint: MachineFootprintId,
        placement: PlacementConstraints,
    ) -> bool {
        self.imported_contract_report_identity == contracts.normalized_identity()
            && self.declared_footprint_report_identity == footprint.normalized_identity()
            && self.machine_regime_report_identity
                == placement
                    .machine_regime()
                    .map_or(0, |identity| identity.normalized_identity())
            && self.installation_scope_report_identity
                == placement
                    .installation_scope()
                    .map_or(0, |identity| identity.normalized_identity())
    }
}

impl RetirementFactDigest {
    /// Derive one provider-defined completion fact from its canonical bytes.
    ///
    /// Retirement gates compare this complete domain-separated digest rather
    /// than a compact provider-selected integer. The canonical bytes remain
    /// provider vocabulary; this layer assigns them no ambient meaning.
    pub fn from_canonical_bytes(canonical: &[u8]) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"omega.retirement-fact.sha256.v1\0");
        digest.update(
            u64::try_from(canonical.len())
                .expect("retirement-fact canonical byte length fits u64")
                .to_le_bytes(),
        );
        digest.update(canonical);
        Self::from_digest(digest.finalize().into())
    }
}

macro_rules! non_authoritative_fingerprint64 {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_compatibility_value(value: u64) -> Result<Self, InstallationDiagnostic> {
                if value == 0 {
                    return Err(InstallationDiagnostic(format!(
                        "non-authoritative {} fingerprint cannot be zero",
                        $label
                    )));
                }
                Ok(Self(value))
            }

            pub const fn compatibility_value(self) -> u64 {
                self.0
            }
        }
    };
}

non_authoritative_fingerprint64!(
    NonAuthoritativeContainerFingerprint64,
    "container-v1 compatibility"
);
non_authoritative_fingerprint64!(
    NonAuthoritativeInformationalFingerprint64,
    "informational-section"
);
non_authoritative_fingerprint64!(
    NonAuthoritativeWriterContextFingerprint64,
    "writer-context replay"
);

mod container;
mod container_bytes;
mod materializer;
mod replacement_quarantine;

pub use container::*;
pub use container_bytes::*;
pub use materializer::*;
pub use replacement_quarantine::*;

#[derive(Debug, PartialEq, Eq)]
struct ArtifactRecord {
    identity: ArtifactId,
    content: ArtifactContentDigest,
    container_fingerprint: NonAuthoritativeContainerFingerprint64,
    architecture: Architecture,
    byte_length: u64,
    code: Vec<u8>,
    contracts: MachineContractSetId,
    declared_footprint: MachineFootprintId,
    placement_plan: PlacementPlanId,
    placement_constraints: PlacementConstraints,
    entry_set: EntrySetId,
    entries: Vec<ArtifactEntry>,
    relocation_set: RelocationSetId,
    relocations: Vec<DecodedArtifactRelocation>,
    authority_commitments: Option<ArtifactAuthorityCommitments>,
}

/// Canonically decoded entry in one executable artifact. The offset remains
/// sealed inside the installation/provider layer; ordinary Omega code sees at
/// most the compiler-issued [`EntryStubId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactEntry {
    identity: EntryStubId,
    code_offset: u64,
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
pub struct Artifact(Arc<ArtifactRecord>);

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
    fn from_legacy_v1_decode(
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

    fn entry(&self, identity: EntryStubId) -> Option<ArtifactEntry> {
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
    receipt: AdmissionReceiptId,
    artifact: Artifact,
    accepted: bool,
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
    artifact: Artifact,
    admission: AdmissionReceiptId,
    container_proof: Option<RetainedContainerProof>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RetainedContainerProof {
    digest: ProofPayloadDigest,
    bytes: Vec<u8>,
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

pub fn admit_executable(
    artifact: &Artifact,
    evidence: ArtifactAdmissionEvidence,
) -> Result<AdmittedArtifact, InstallationDiagnostic> {
    if artifact.0.authority_commitments.is_none() {
        return Err(InstallationDiagnostic(
            "container-v1 compatibility candidates lack strong authority commitments and cannot be admitted"
                .into(),
        ));
    }
    if !evidence.accepted {
        return Err(InstallationDiagnostic(
            "artifact validator did not accept executable eligibility".into(),
        ));
    }
    if evidence.artifact != *artifact {
        return Err(InstallationDiagnostic(
            "artifact admission evidence does not match canonical candidate".into(),
        ));
    }
    Ok(AdmittedArtifact {
        artifact: artifact.clone(),
        admission: evidence.receipt,
        container_proof: None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationAudience {
    DormantLocal,
    FutureFetcher,
}

/// One-shot authority over an exact W+NX destination.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CodePlacementExtentEvidence {
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    era: psi_extents::MappingEraId,
    lineage: psi_extents::ExtentLineageId,
}

impl CodePlacementExtentEvidence {
    fn from_extent(extent: &Extent) -> Self {
        Self {
            base: extent.base(),
            length: extent.length(),
            address_space: extent.address_space(),
            rights: extent.rights().clone(),
            provenance: extent.provenance(),
            era: extent.era(),
            lineage: extent.lineage_root(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CodePlacementAuthority {
    placement: CodePlacementId,
    scope: InstallationScopeId,
    audience: InstallationAudience,
    extent: CodePlacementExtentEvidence,
    required_rights: ExtentRights,
    constraints: PlacementConstraints,
    site: PlacementSite,
}

impl CodePlacementAuthority {
    #[allow(clippy::too_many_arguments)]
    pub fn from_admitted_provider(
        placement: CodePlacementId,
        scope: InstallationScopeId,
        audience: InstallationAudience,
        extent: &Extent,
        required_rights: ExtentRights,
        constraints: PlacementConstraints,
        site: PlacementSite,
    ) -> Self {
        Self {
            placement,
            scope,
            audience,
            extent: CodePlacementExtentEvidence::from_extent(extent),
            required_rights,
            constraints,
            site,
        }
    }

    pub fn claim(self, extent: Extent) -> Result<CodePlacement, Box<PlacementClaimError>> {
        let mismatch = if CodePlacementExtentEvidence::from_extent(&extent) != self.extent {
            Some(
                "extent does not match the exact range, space, rights, provenance, era, and lineage bound by code-placement authority"
                    .into(),
            )
        } else if !extent.rights().contains(&self.required_rights) {
            Some("extent lacks rights required by code-placement authority".into())
        } else if self.site.base_address != extent.base() {
            Some("placement site base does not match destination Extent".into())
        } else if self
            .site
            .installation_scope
            .is_none_or(|scope| scope.normalized_identity() != self.scope.normalized_identity())
        {
            Some("placement site does not carry the exact installation scope".into())
        } else {
            match usize::try_from(extent.length()) {
                Ok(length) => self
                    .constraints
                    .validate_site(length, self.site)
                    .err()
                    .map(|diagnostic| diagnostic.0),
                Err(_) => {
                    Some("placement length cannot be represented by the host validator".into())
                }
            }
        };
        if let Some(message) = mismatch {
            return Err(Box::new(PlacementClaimError {
                authority: self,
                extent,
                diagnostic: InstallationDiagnostic(message),
            }));
        }
        Ok(CodePlacement {
            placement: self.placement,
            scope: self.scope,
            audience: self.audience,
            constraints: self.constraints,
            extent,
        })
    }
}

#[derive(Debug)]
pub struct PlacementClaimError {
    authority: CodePlacementAuthority,
    extent: Extent,
    diagnostic: InstallationDiagnostic,
}

impl PlacementClaimError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (CodePlacementAuthority, Extent) {
        (self.authority, self.extent)
    }
}

/// Linear W+NX placement state. It carries no executable permission.
#[derive(Debug)]
pub struct CodePlacement {
    placement: CodePlacementId,
    scope: InstallationScopeId,
    audience: InstallationAudience,
    constraints: PlacementConstraints,
    extent: Extent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodePlacementEvidence {
    placement: CodePlacementId,
    scope: InstallationScopeId,
    audience: InstallationAudience,
    constraints: PlacementConstraints,
    extent: CodePlacementExtentEvidence,
}

impl CodePlacementEvidence {
    fn from_placement(placement: &CodePlacement) -> Self {
        Self {
            placement: placement.placement,
            scope: placement.scope,
            audience: placement.audience,
            constraints: placement.constraints,
            extent: CodePlacementExtentEvidence::from_extent(&placement.extent),
        }
    }
}

impl CodePlacement {
    pub const fn identity(&self) -> CodePlacementId {
        self.placement
    }

    pub const fn base(&self) -> u64 {
        self.extent.base()
    }

    pub const fn length(&self) -> u64 {
        self.extent.length()
    }
}

/// Provider result after declared sections/relocations were written and all
/// write authority over the final bytes was frozen.
#[derive(Debug, PartialEq, Eq)]
pub struct MaterializationReceipt {
    materialized: MaterializedArtifactBytes,
    realized_footprint: MachineFootprintId,
    writes_frozen: bool,
}

impl MaterializationReceipt {
    /// Bind a provider's write/freeze receipt to the exact output of the
    /// canonical materializer instead of restating its artifact/placement
    /// identities independently.
    pub fn from_materialized(
        materialized: &MaterializedArtifactBytes,
        realized_footprint: MachineFootprintId,
        writes_frozen: bool,
    ) -> Self {
        Self {
            materialized: materialized.clone(),
            realized_footprint,
            writes_frozen,
        }
    }
}

pub fn materialize_and_freeze(
    artifact: &AdmittedArtifact,
    placement: CodePlacement,
    materialized: MaterializedArtifactBytes,
    receipt: MaterializationReceipt,
) -> Result<FrozenPlacement, Box<MaterializationError>> {
    let mismatch = if materialized.admission_evidence() != artifact {
        Some("canonical materializer output does not retain the exact admitted artifact")
    } else if materialized.placement_evidence()
        != &CodePlacementEvidence::from_placement(&placement)
    {
        Some("canonical materializer output does not retain the exact code placement")
    } else if materialized.placement_plan() != artifact.artifact.0.placement_plan {
        Some("canonical materializer output did not use the admitted placement plan")
    } else if materialized.bytes().len() as u64 != artifact.artifact.0.byte_length {
        Some("canonical materializer output has the wrong executable byte length")
    } else if receipt.materialized != materialized {
        Some("materialization receipt does not bind the exact canonical output")
    } else if placement.extent.length() < artifact.artifact.0.byte_length {
        Some("code placement is smaller than the admitted artifact")
    } else if placement.constraints != artifact.artifact.0.placement_constraints {
        Some("code placement constraints do not match the admitted artifact")
    } else if !receipt.writes_frozen {
        Some("materialization did not freeze write authority over final bytes")
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(MaterializationError {
            placement,
            materialized,
            receipt,
            diagnostic: InstallationDiagnostic(message.into()),
        }));
    }
    Ok(FrozenPlacement {
        artifact: artifact.clone(),
        placement,
        materialized,
        realized_footprint: receipt.realized_footprint,
    })
}

#[derive(Debug)]
pub struct MaterializationError {
    placement: CodePlacement,
    materialized: MaterializedArtifactBytes,
    receipt: MaterializationReceipt,
    diagnostic: InstallationDiagnostic,
}

impl MaterializationError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        CodePlacement,
        MaterializedArtifactBytes,
        MaterializationReceipt,
    ) {
        (self.placement, self.materialized, self.receipt)
    }
}

/// Linear R+NX placement whose exact bytes can no longer change.
#[derive(Debug)]
pub struct FrozenPlacement {
    artifact: AdmittedArtifact,
    placement: CodePlacement,
    materialized: MaterializedArtifactBytes,
    realized_footprint: MachineFootprintId,
}

impl FrozenPlacement {
    /// Exact immutable byte snapshot whose write authority the provider froze.
    /// Final footprint/PCC validators inspect this provider-side view; it is
    /// not an Omega source-visible byte-to-code escape hatch.
    pub fn bytes(&self) -> &[u8] {
        self.materialized.bytes()
    }

    pub const fn final_bytes(&self) -> FinalBytesDigest {
        self.materialized.final_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalValidationCertificate {
    identity: FinalValidationId,
    artifact: Artifact,
    admission: AdmissionReceiptId,
    placement: CodePlacementEvidence,
    final_bytes_digest: FinalBytesDigest,
    final_bytes: Vec<u8>,
    realized_footprint: MachineFootprintId,
    accepted: bool,
}

impl FinalValidationCertificate {
    /// Construct validation evidence from the exact frozen carrier inspected
    /// by the validator. Compact normalized IDs remain useful report keys but
    /// are not accepted as collision-resistant content evidence.
    pub fn from_validator(
        identity: FinalValidationId,
        frozen: &FrozenPlacement,
        accepted: bool,
    ) -> Self {
        Self {
            identity,
            artifact: frozen.artifact.artifact.clone(),
            admission: frozen.artifact.admission,
            placement: CodePlacementEvidence::from_placement(&frozen.placement),
            final_bytes_digest: frozen.materialized.final_bytes(),
            final_bytes: frozen.materialized.bytes().to_vec(),
            realized_footprint: frozen.realized_footprint,
            accepted,
        }
    }
}

pub fn validate_final_placement(
    frozen: FrozenPlacement,
    certificate: &FinalValidationCertificate,
) -> Result<ValidatedPlacement, Box<FrozenPlacementError>> {
    let matches = certificate.accepted
        && certificate.artifact == frozen.artifact.artifact
        && certificate.admission == frozen.artifact.admission
        && certificate.placement == CodePlacementEvidence::from_placement(&frozen.placement)
        && certificate.final_bytes_digest == frozen.materialized.final_bytes()
        && certificate.final_bytes == frozen.materialized.bytes()
        && certificate.realized_footprint == frozen.realized_footprint;
    if !matches {
        return Err(Box::new(FrozenPlacementError {
            frozen,
            diagnostic: InstallationDiagnostic(
                "final validation certificate does not match frozen placement".into(),
            ),
        }));
    }
    Ok(ValidatedPlacement {
        frozen,
        validation: certificate.identity,
    })
}

#[derive(Debug)]
pub struct FrozenPlacementError {
    frozen: FrozenPlacement,
    diagnostic: InstallationDiagnostic,
}

impl FrozenPlacementError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_frozen(self) -> FrozenPlacement {
        self.frozen
    }
}

/// Linear R+NX placement bound to checked final bytes and footprint.
#[derive(Debug)]
pub struct ValidatedPlacement {
    frozen: FrozenPlacement,
    validation: FinalValidationId,
}

/// Exact provider-side evidence for one validated placement. Normalized IDs
/// remain report keys; authorization compares the canonical artifact, frozen
/// bytes, placement geometry, authority lineage, and validation result.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidatedPlacementEvidence {
    admission_evidence: AdmittedArtifact,
    artifact: Artifact,
    admission: AdmissionReceiptId,
    placement: CodePlacementId,
    scope: InstallationScopeId,
    audience: InstallationAudience,
    constraints: PlacementConstraints,
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    era: psi_extents::MappingEraId,
    lineage: psi_extents::ExtentLineageId,
    final_bytes: Vec<u8>,
    realized_footprint: MachineFootprintId,
    validation: FinalValidationId,
}

impl ValidatedPlacementEvidence {
    fn from_validated(validated: &ValidatedPlacement) -> Self {
        let frozen = &validated.frozen;
        let extent = &frozen.placement.extent;
        Self {
            admission_evidence: frozen.artifact.clone(),
            artifact: frozen.artifact.artifact.clone(),
            admission: frozen.artifact.admission,
            placement: frozen.placement.placement,
            scope: frozen.placement.scope,
            audience: frozen.placement.audience,
            constraints: frozen.placement.constraints,
            base: extent.base(),
            length: extent.length(),
            address_space: extent.address_space(),
            rights: extent.rights().clone(),
            provenance: extent.provenance(),
            era: extent.era(),
            lineage: extent.lineage_root(),
            final_bytes: frozen.materialized.bytes().to_vec(),
            realized_footprint: frozen.realized_footprint,
            validation: validated.validation,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InstallAuthority {
    validated: ValidatedPlacementEvidence,
}

impl InstallAuthority {
    pub fn from_admitted_provider(validated: &ValidatedPlacement) -> Self {
        Self {
            validated: ValidatedPlacementEvidence::from_validated(validated),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WxEnforcement {
    HardwareEnforced,
    ConventionOnly,
    Unsupported,
}

#[derive(Debug, PartialEq, Eq)]
pub struct InstallationReceipt {
    installed: InstalledCodeId,
    validated: ValidatedPlacementEvidence,
    visibility_complete: bool,
    wx: WxEnforcement,
}

impl InstallationReceipt {
    pub fn from_provider(
        installed: InstalledCodeId,
        validated: &ValidatedPlacement,
        visibility_complete: bool,
        wx: WxEnforcement,
    ) -> Self {
        Self {
            installed,
            validated: ValidatedPlacementEvidence::from_validated(validated),
            visibility_complete,
            wx,
        }
    }
}

pub fn install_validated(
    validated: ValidatedPlacement,
    authority: InstallAuthority,
    receipt: InstallationReceipt,
) -> Result<InstalledCode, Box<InstallationError>> {
    let evidence = ValidatedPlacementEvidence::from_validated(&validated);
    let mismatch = if authority.validated != evidence {
        Some("install authority is not scoped to this validated placement")
    } else if receipt.validated != evidence {
        Some("installation receipt does not match validated placement")
    } else if !receipt.visibility_complete {
        Some("installation did not complete instruction-fetch visibility")
    } else if receipt.wx == WxEnforcement::Unsupported {
        Some("provider does not support executable installation")
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(InstallationError {
            validated,
            authority,
            receipt,
            diagnostic: InstallationDiagnostic(message.into()),
        }));
    }

    Ok(InstalledCode {
        identity: receipt.installed,
        validated,
        wx: receipt.wx,
        installation_registry_claimed: false,
    })
}

#[derive(Debug)]
pub struct InstallationError {
    validated: ValidatedPlacement,
    authority: InstallAuthority,
    receipt: InstallationReceipt,
    diagnostic: InstallationDiagnostic,
}

impl InstallationError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedPlacement, InstallAuthority, InstallationReceipt) {
        (self.validated, self.authority, self.receipt)
    }
}

/// Linear installed claim. It exposes identity/reporting only; callable entry
/// references are derived by the separate CFI/entry-reference gate.
#[derive(Debug)]
pub struct InstalledCode {
    identity: InstalledCodeId,
    validated: ValidatedPlacement,
    wx: WxEnforcement,
    installation_registry_claimed: bool,
}

/// One-shot authority to create the canonical installation-wide registry for
/// one exact installed-code occurrence.
///
/// This value is deliberately opaque and non-clonable. Compact installation
/// and artifact IDs are report keys only; equality retains the complete
/// installed-code evidence, including the exact placement scope and bytes.
#[derive(Debug, PartialEq, Eq)]
pub struct InstallationRegistryAuthority {
    installed: InstalledCodeEvidence,
}

/// Opaque provider-private words for one checked post-handoff entry writer.
/// Word zero is the exact destination base and the remaining words are the
/// dense, first-occurrence-ordered source slots. The numeric words have no
/// public accessor and this carrier is deliberately non-clonable.
#[derive(PartialEq, Eq)]
pub struct ResolvedPostHandoffEntryWriterContext {
    installed_evidence: InstalledCodeEvidence,
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    destination_site: PlacementSite,
    destination_len: usize,
    invocation: PostHandoffWriterInvocationPlan,
    packed_words: Vec<u64>,
    non_authoritative_fingerprint: NonAuthoritativeWriterContextFingerprint64,
}

/// Provider receipt establishing the runtime properties needed before a
/// generated writer may reach one activated mapping. The exact mapping
/// evidence is sealed in `mapping`; compact receipt identity is report-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestinationPreparationReceipt {
    identity: DestinationPreparationReceiptId,
    mapping: MappingReceiptContext,
    required_write_rights: ExtentRights,
    pinned: bool,
    unpublished: bool,
}

impl DestinationPreparationReceipt {
    pub fn from_admitted_provider(
        identity: DestinationPreparationReceiptId,
        mapping: &MappingReceiptContext,
        required_write_rights: ExtentRights,
        pinned: bool,
        unpublished: bool,
    ) -> Self {
        Self {
            identity,
            mapping: mapping.clone(),
            required_write_rights,
            pinned,
            unpublished,
        }
    }

    pub const fn identity(&self) -> DestinationPreparationReceiptId {
        self.identity
    }
}

/// Linear provider-side destination ready for one post-handoff writer.
///
/// `MappedExtent` establishes an activated translation and exact custody;
/// `receipt` establishes pinning and non-publication for that same mapping;
/// `required_write_rights` names the target/provider-defined write authority.
/// The byte slice is the provider's concrete mutable view and cannot outlive
/// this carrier. No source-language write-only view is implied.
#[derive(Debug)]
pub struct PreparedPostHandoffWriterDestination<'mapping, 'bytes> {
    mapping: MappedExtent<'mapping>,
    receipt: DestinationPreparationReceipt,
    site: PlacementSite,
    bytes: &'bytes mut [u8],
}

/// A prepared destination whose activated mapping, provider receipt, write
/// rights, pinning, unpublished state, placement, and byte geometry have been
/// replayed before symbolic-source resolution.
#[derive(Debug)]
#[must_use = "validated prepared destination retains mapping and byte custody"]
pub struct ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes> {
    destination: PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
}

#[derive(Debug)]
pub struct PreparedPostHandoffWriterDestinationValidationError<'mapping, 'bytes> {
    destination: PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: InstallationDiagnostic,
}

impl<'mapping, 'bytes> PreparedPostHandoffWriterDestinationValidationError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_destination(self) -> PreparedPostHandoffWriterDestination<'mapping, 'bytes> {
        self.destination
    }
}

impl<'mapping, 'bytes> PreparedPostHandoffWriterDestination<'mapping, 'bytes> {
    pub fn claim(
        mapping: MappedExtent<'mapping>,
        receipt: DestinationPreparationReceipt,
        site: PlacementSite,
        bytes: &'bytes mut [u8],
    ) -> Result<Self, Box<DestinationClaimError<'mapping, 'bytes>>> {
        if let Err(diagnostic) =
            validate_post_handoff_destination_binding(&mapping, &receipt, site, bytes.len())
        {
            return Err(Box::new(DestinationClaimError {
                mapping,
                receipt,
                site,
                bytes,
                diagnostic,
            }));
        }
        Ok(Self {
            mapping,
            receipt,
            site,
            bytes,
        })
    }

    pub const fn site(&self) -> PlacementSite {
        self.site
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Independently replay the exact activated mapping, provider receipt,
    /// placement, and byte-view geometry before a resolver observes symbolic
    /// source values. This borrows the carrier and grants no write or
    /// publication authority.
    pub fn validate_for_writer_preparation(&self) -> Result<(), InstallationDiagnostic> {
        validate_post_handoff_destination_binding(
            &self.mapping,
            &self.receipt,
            self.site,
            self.bytes.len(),
        )
    }

    /// Consume this destination into replayed custody before a resolver may
    /// observe symbolic sources. Rejection returns the exact raw destination.
    pub fn into_validated_for_writer_preparation(
        self,
    ) -> Result<
        ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
        Box<PreparedPostHandoffWriterDestinationValidationError<'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.validate_for_writer_preparation() {
            return Err(Box::new(
                PreparedPostHandoffWriterDestinationValidationError {
                    destination: self,
                    diagnostic,
                },
            ));
        }
        Ok(ValidatedPreparedPostHandoffWriterDestination { destination: self })
    }
}

impl<'mapping, 'bytes> ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes> {
    pub const fn site(&self) -> PlacementSite {
        self.destination.site
    }

    pub fn len(&self) -> usize {
        self.destination.bytes.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.destination.bytes.is_empty()
    }

    pub fn into_destination(self) -> PreparedPostHandoffWriterDestination<'mapping, 'bytes> {
        self.destination
    }
}

fn validate_post_handoff_destination_binding(
    mapping: &MappedExtent<'_>,
    receipt: &DestinationPreparationReceipt,
    site: PlacementSite,
    byte_len: usize,
) -> Result<(), InstallationDiagnostic> {
    let mismatch = if receipt.mapping != mapping.receipt_context() {
        Some("destination preparation receipt does not bind the exact activated mapping")
    } else if !receipt.pinned {
        Some("destination preparation receipt does not establish pinning")
    } else if !receipt.unpublished {
        Some("destination preparation receipt does not establish an unpublished destination")
    } else if receipt.required_write_rights.identities().next().is_none() {
        Some("destination preparation receipt names no writer right")
    } else if !mapping.rights().contains(&receipt.required_write_rights) {
        Some("activated destination mapping lacks required writer rights")
    } else if site.phase != psi_layout_plans::PlacementPhase::PostHandoff {
        Some("prepared writer destination is not in the post-handoff placement phase")
    } else if site.base_address != mapping.base() {
        Some("prepared writer destination base does not match the activated mapping")
    } else if usize::try_from(mapping.length()).ok() != Some(byte_len) {
        Some("prepared writer byte view does not cover the exact activated mapping")
    } else {
        None
    };
    mismatch.map_or(Ok(()), |message| {
        Err(InstallationDiagnostic(message.into()))
    })
}

#[derive(Debug)]
pub struct DestinationClaimError<'mapping, 'bytes> {
    mapping: MappedExtent<'mapping>,
    receipt: DestinationPreparationReceipt,
    site: PlacementSite,
    bytes: &'bytes mut [u8],
    diagnostic: InstallationDiagnostic,
}

impl<'mapping, 'bytes> DestinationClaimError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        MappedExtent<'mapping>,
        DestinationPreparationReceipt,
        PlacementSite,
        &'bytes mut [u8],
    ) {
        (self.mapping, self.receipt, self.site, self.bytes)
    }
}

/// Exact destination after successful generated writing. It remains
/// unpublished and retains the activated mapping for consumer-specific
/// validation and eventual publication or recovery.
#[derive(Debug)]
pub struct WrittenPostHandoffWriterDestination<'mapping, 'bytes> {
    mapping: MappedExtent<'mapping>,
    receipt: DestinationPreparationReceipt,
    site: PlacementSite,
    bytes: &'bytes mut [u8],
    context: ResolvedPostHandoffEntryWriterContext,
}

/// A still-unpublished written destination whose exact installed context,
/// activated mapping, provider receipt, placement, and byte geometry have
/// been replayed before observation.
#[derive(Debug)]
#[must_use = "validated written destination retains mapping and byte custody"]
pub struct ValidatedWrittenPostHandoffWriterDestination<'mapping, 'bytes> {
    written: WrittenPostHandoffWriterDestination<'mapping, 'bytes>,
}

/// Validation rejection preserves the complete written destination so its
/// exact retained evidence can be repaired and retried without reconstruction.
#[derive(Debug)]
pub struct WrittenPostHandoffWriterConsumerValidationError<'mapping, 'bytes> {
    written: WrittenPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: InstallationDiagnostic,
}

impl<'mapping, 'bytes> WrittenPostHandoffWriterConsumerValidationError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_written(self) -> WrittenPostHandoffWriterDestination<'mapping, 'bytes> {
        self.written
    }

    pub fn into_prepared_parts(
        self,
    ) -> (
        ResolvedPostHandoffEntryWriterContext,
        ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    ) {
        let (context, destination) = self.written.into_prepared_parts();
        (
            context,
            ValidatedPreparedPostHandoffWriterDestination { destination },
        )
    }
}

impl<'mapping, 'bytes> WrittenPostHandoffWriterDestination<'mapping, 'bytes> {
    pub const fn installed_code(&self) -> InstalledCodeId {
        self.context.installed_code()
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.context.artifact()
    }

    pub const fn site(&self) -> PlacementSite {
        self.site
    }

    pub const fn non_authoritative_writer_context_fingerprint(
        &self,
    ) -> NonAuthoritativeWriterContextFingerprint64 {
        self.context.non_authoritative_fingerprint()
    }

    pub fn binds_invocation(&self, invocation: &PostHandoffWriterInvocationPlan) -> bool {
        self.context.binds_invocation(invocation)
    }

    pub const fn normalized_fragment_report_fingerprint(&self) -> u64 {
        self.context.normalized_fragment_report_fingerprint()
    }

    /// Independently replay the exact non-clonable writer context and
    /// destination preparation before an owning consumer validates semantic
    /// contents or publishes the mapping. This establishes no consumer value
    /// and performs no publication.
    pub fn validate_for_consumer(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<(), InstallationDiagnostic> {
        installed_code.validate_written_post_handoff_context(
            &self.context,
            self.site,
            self.bytes.len(),
        )?;
        validate_post_handoff_destination_binding(
            &self.mapping,
            &self.receipt,
            self.site,
            self.bytes.len(),
        )
    }

    /// Consume this still-unpublished destination only after exact replay.
    /// Rejection exposes no context or bytes and returns complete custody.
    pub fn into_validated_for_consumer(
        self,
        installed_code: &InstalledCode,
    ) -> Result<
        ValidatedWrittenPostHandoffWriterDestination<'mapping, 'bytes>,
        Box<WrittenPostHandoffWriterConsumerValidationError<'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.validate_for_consumer(installed_code) {
            return Err(Box::new(WrittenPostHandoffWriterConsumerValidationError {
                written: self,
                diagnostic,
            }));
        }
        Ok(ValidatedWrittenPostHandoffWriterDestination { written: self })
    }

    /// Recover the exact non-clonable context and still-unpublished prepared
    /// destination when a later consumer rejects validation. No byte or
    /// authority is reconstructed by this transition.
    fn into_prepared_parts(
        self,
    ) -> (
        ResolvedPostHandoffEntryWriterContext,
        PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    ) {
        let Self {
            mapping,
            receipt,
            site,
            bytes,
            context,
        } = self;
        (
            context,
            PreparedPostHandoffWriterDestination {
                mapping,
                receipt,
                site,
                bytes,
            },
        )
    }

    fn into_parts(
        self,
    ) -> (
        MappedExtent<'mapping>,
        DestinationPreparationReceipt,
        PlacementSite,
        &'bytes mut [u8],
    ) {
        (self.mapping, self.receipt, self.site, self.bytes)
    }
}

impl<'mapping, 'bytes> ValidatedWrittenPostHandoffWriterDestination<'mapping, 'bytes> {
    pub const fn installed_code(&self) -> InstalledCodeId {
        self.written.installed_code()
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.written.artifact()
    }

    pub const fn site(&self) -> PlacementSite {
        self.written.site()
    }

    pub const fn non_authoritative_writer_context_fingerprint(
        &self,
    ) -> NonAuthoritativeWriterContextFingerprint64 {
        self.written.non_authoritative_writer_context_fingerprint()
    }

    pub fn binds_invocation(&self, invocation: &PostHandoffWriterInvocationPlan) -> bool {
        self.written.binds_invocation(invocation)
    }

    pub const fn normalized_fragment_report_fingerprint(&self) -> u64 {
        self.written.normalized_fragment_report_fingerprint()
    }

    /// Replay the installed realization and destination preparation without
    /// downgrading this already-validated custody carrier.
    pub fn validate_for_consumer(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<(), InstallationDiagnostic> {
        self.written.validate_for_consumer(installed_code)
    }

    pub const fn context(&self) -> &ResolvedPostHandoffEntryWriterContext {
        &self.written.context
    }

    /// Bytes remain unpublished; this is observation after exact replay.
    pub fn bytes(&self) -> &[u8] {
        self.written.bytes
    }

    pub fn into_written(self) -> WrittenPostHandoffWriterDestination<'mapping, 'bytes> {
        self.written
    }

    pub fn into_prepared_parts(
        self,
    ) -> (
        ResolvedPostHandoffEntryWriterContext,
        ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    ) {
        let (context, destination) = self.written.into_prepared_parts();
        (
            context,
            ValidatedPreparedPostHandoffWriterDestination { destination },
        )
    }

    pub fn into_parts(
        self,
    ) -> (
        MappedExtent<'mapping>,
        DestinationPreparationReceipt,
        PlacementSite,
        &'bytes mut [u8],
    ) {
        self.written.into_parts()
    }
}

#[derive(Debug)]
pub struct DestinationWriteError<'mapping, 'bytes> {
    context: ResolvedPostHandoffEntryWriterContext,
    destination: ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: MaterializationDiagnostic,
}

impl<'mapping, 'bytes> DestinationWriteError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &MaterializationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ResolvedPostHandoffEntryWriterContext,
        ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    ) {
        (self.context, self.destination)
    }
}

impl std::fmt::Debug for ResolvedPostHandoffEntryWriterContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResolvedPostHandoffEntryWriterContext")
            .field("installed_code", &self.installed_code)
            .field("artifact", &self.artifact)
            .field("destination_len", &self.destination_len)
            .field("source_slot_count", &self.invocation.sources().len())
            .field(
                "normalized_fragment_report_fingerprint",
                &format_args!("{:016x}", self.invocation.fragment().report_fingerprint()),
            )
            .field(
                "non_authoritative_fingerprint",
                &format_args!(
                    "{:016x}",
                    self.non_authoritative_fingerprint.compatibility_value()
                ),
            )
            .finish()
    }
}

impl ResolvedPostHandoffEntryWriterContext {
    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn source_slot_count(&self) -> usize {
        self.invocation.source_slot_count()
    }

    pub const fn packed_byte_len(&self) -> usize {
        self.packed_words.len() * std::mem::size_of::<u64>()
    }

    pub const fn non_authoritative_fingerprint(
        &self,
    ) -> NonAuthoritativeWriterContextFingerprint64 {
        self.non_authoritative_fingerprint
    }

    pub const fn context_abi(&self) -> u64 {
        self.invocation.fragment().context_abi()
    }

    pub const fn normalized_fragment_report_fingerprint(&self) -> u64 {
        self.invocation.fragment().report_fingerprint()
    }

    /// Report whether this opaque, once-resolved context is the invocation
    /// sibling of one exact reusable fragment plan. Numeric packed words remain
    /// inaccessible.
    pub fn binds_invocation(&self, invocation: &PostHandoffWriterInvocationPlan) -> bool {
        self.invocation == *invocation
    }

    /// Replay this sealed context against the exact installed realization and
    /// destination geometry without resolving or exposing its numeric words.
    pub fn validate_for_destination(
        &self,
        installed_code: &InstalledCode,
        destination_site: PlacementSite,
        destination_len: usize,
    ) -> Result<(), InstallationDiagnostic> {
        installed_code.validate_written_post_handoff_context(
            self,
            destination_site,
            destination_len,
        )
    }
}

impl InstalledCode {
    pub const fn identity(&self) -> InstalledCodeId {
        self.identity
    }

    pub fn artifact(&self) -> ArtifactId {
        self.validated.frozen.artifact.artifact.0.identity
    }

    /// Exact installation scope carried by this admitted placement.
    pub const fn installation_scope(&self) -> InstallationScopeId {
        self.validated.frozen.placement.scope
    }

    /// Issue the sole registry authority for this installed-code occurrence.
    /// Dropping the authority does not reopen issuance: the burned claim is
    /// retained by `InstalledCode` until that occurrence is retired.
    pub fn claim_installation_registry(
        &mut self,
    ) -> Result<InstallationRegistryAuthority, InstallationDiagnostic> {
        if self.installation_registry_claimed {
            return Err(InstallationDiagnostic(
                "installation registry authority was already issued for this installed-code occurrence"
                    .into(),
            ));
        }
        self.installation_registry_claimed = true;
        Ok(InstallationRegistryAuthority {
            installed: InstalledCodeEvidence::from_installed(self),
        })
    }

    /// Test whether this exact installed realization came from one
    /// relocation-free artifact whose canonical and frozen bytes both equal
    /// the expected bytes. The bytes remain provider-side: callers receive
    /// only the sealed equality result needed to bind higher-level
    /// certificates.
    pub fn binds_exact_unrelocated_artifact_bytes(&self, expected: &[u8]) -> bool {
        let frozen = &self.validated.frozen;
        frozen.artifact.artifact.0.relocations.is_empty()
            && frozen.artifact.artifact.0.code == expected
            && frozen.materialized.bytes() == expected
    }

    /// Test both sides of a relocatable installation: the exact frozen
    /// compiler-authored bytes before relocation and the exact materialized
    /// bytes after the admitted relocation set was applied. No bytes or
    /// addresses cross this evidence boundary.
    pub fn binds_exact_materialized_artifact_bytes(
        &self,
        expected_unrelocated: &[u8],
        expected_materialized: &[u8],
    ) -> bool {
        let frozen = &self.validated.frozen;
        frozen.artifact.artifact.0.code == expected_unrelocated
            && frozen.materialized.bytes() == expected_materialized
    }

    /// Test the exact admitted code offset of one selected entry without
    /// exposing a resolved address.
    pub fn binds_entry_offset(&self, entry: EntryStubId, expected_offset: u64) -> bool {
        self.validated
            .frozen
            .artifact
            .artifact
            .entry(entry)
            .is_some_and(|candidate| candidate.code_offset == expected_offset)
    }

    pub fn architecture(&self) -> Architecture {
        self.validated.frozen.artifact.artifact.0.architecture
    }

    pub const fn placement(&self) -> CodePlacementId {
        self.validated.frozen.placement.placement
    }

    pub const fn validation(&self) -> FinalValidationId {
        self.validated.validation
    }

    pub const fn wx(&self) -> WxEnforcement {
        self.wx
    }

    pub fn receipt_context(&self) -> InstalledCodeContext {
        InstalledCodeContext(InstalledCodeEvidence::from_installed(self))
    }

    pub fn occurrence_digest(&self) -> InstalledArtifactOccurrenceDigest {
        installed_artifact_occurrence_digest(&InstalledCodeEvidence::from_installed(self))
    }

    /// Returns a sealed target only for an entry admitted with this installed
    /// artifact. The numeric address stays private to writer execution.
    pub fn selected_entry_target(
        &self,
        identity: EntryStubId,
    ) -> Result<RelocationTarget, InstallationDiagnostic> {
        self.validated
            .frozen
            .artifact
            .selected_entry_target(identity)
    }

    /// Check the exact post-handoff entry writer without resolving an entry
    /// address into a public value or changing destination bytes. This is the
    /// provider-side preparation gate used before compiler-generated writer
    /// lowering. Pre-resolved entry fragments must equal the address from this
    /// exact installed realization; unresolved fragments must be members of
    /// this artifact's admitted entry set.
    pub fn validate_post_handoff_entry_writer(
        &self,
        plan: &PostHandoffWriterPlan,
        destination_len: usize,
        destination_site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        let invocation = plan.lower_reusable_fragment()?;
        self.validate_post_handoff_entry_writer_invocation(
            plan,
            &invocation,
            destination_len,
            destination_site,
        )
        .map(|_| ())
    }

    fn validate_post_handoff_entry_writer_invocation(
        &self,
        plan: &PostHandoffWriterPlan,
        invocation: &PostHandoffWriterInvocationPlan,
        destination_len: usize,
        destination_site: PlacementSite,
    ) -> Result<Vec<u64>, MaterializationDiagnostic> {
        plan.validate(destination_len, destination_site)?;
        let mut source_values = Vec::with_capacity(invocation.sources().len());
        for slot in invocation.sources() {
            let value = match slot.source {
                PostHandoffWriterSource::Resolve(target) => {
                    if target != slot.target || !self.contains_entry_target(target) {
                        return Err(MaterializationDiagnostic(format!(
                            "post-handoff writer target {target:?} is not an admitted entry in the exact installed artifact"
                        )));
                    }
                    self.resolve_entry_target(target).ok_or_else(|| {
                        MaterializationDiagnostic(format!(
                            "post-handoff writer could not resolve admitted target {target:?}"
                        ))
                    })?
                }
                PostHandoffWriterSource::Resolved(value) => match slot.target {
                    RelocationTarget::Entry(_)
                        if self.resolve_entry_target(slot.target) == Some(value) =>
                    {
                        value
                    }
                    RelocationTarget::Entry(_) => {
                        return Err(MaterializationDiagnostic(format!(
                            "post-handoff writer pre-resolved entry {:?} does not match the exact installed realization",
                            slot.target
                        )));
                    }
                    RelocationTarget::Data(_) => {
                        return Err(MaterializationDiagnostic(format!(
                            "post-handoff entry writer target {:?} is not an admitted entry in the exact installed artifact",
                            slot.target
                        )));
                    }
                },
            };
            source_values.push(value);
        }
        invocation.validate_source_values(&source_values)?;
        Ok(source_values)
    }

    /// Resolve every distinct source exactly once into an opaque packed
    /// provider context. No numeric code or destination address is returned to
    /// the caller; only the sealed carrier may be passed to checked execution.
    pub fn populate_post_handoff_entry_writer_context(
        &self,
        plan: &PostHandoffWriterPlan,
        destination_len: usize,
        destination_site: PlacementSite,
    ) -> Result<ResolvedPostHandoffEntryWriterContext, MaterializationDiagnostic> {
        let invocation = plan.lower_reusable_fragment()?;
        let source_values = self.validate_post_handoff_entry_writer_invocation(
            plan,
            &invocation,
            destination_len,
            destination_site,
        )?;

        let mut packed_words = Vec::with_capacity(invocation.sources().len() + 1);
        packed_words.push(destination_site.base_address);
        packed_words.extend(source_values);
        let non_authoritative_fingerprint =
            non_authoritative_post_handoff_entry_writer_context_fingerprint(
                self.identity,
                self.artifact(),
                destination_site,
                destination_len,
                &invocation,
                &packed_words,
            );
        Ok(ResolvedPostHandoffEntryWriterContext {
            installed_evidence: InstalledCodeEvidence::from_installed(self),
            installed_code: self.identity,
            artifact: self.artifact(),
            destination_site,
            destination_len,
            invocation,
            packed_words,
            non_authoritative_fingerprint,
        })
    }

    /// Execute with the exact once-resolved values sealed into `context`.
    /// Context/plan/site drift rejects before destination mutation.
    pub fn execute_populated_post_handoff_entry_writer(
        &self,
        context: &ResolvedPostHandoffEntryWriterContext,
        plan: &PostHandoffWriterPlan,
        destination: &mut [u8],
        destination_site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        let invocation = plan.lower_reusable_fragment()?;
        if context.installed_evidence != InstalledCodeEvidence::from_installed(self)
            || context.installed_code != self.identity
            || context.artifact != self.artifact()
            || context.destination_site != destination_site
            || context.destination_len != destination.len()
            || context.invocation != invocation
            || context.context_abi() != POST_HANDOFF_WRITER_CONTEXT_ABI_V1
            || context.packed_words.first().copied() != Some(destination_site.base_address)
            || context.packed_words.len() != context.invocation.sources().len() + 1
        {
            return Err(MaterializationDiagnostic(
                "populated post-handoff writer context does not bind the exact installed code, plan, destination, and packed geometry"
                    .into(),
            ));
        }
        self.validate_post_handoff_entry_writer(plan, destination.len(), destination_site)?;
        plan.execute(destination, destination_site, |target| {
            context
                .invocation
                .sources()
                .iter()
                .zip(&context.packed_words[1..])
                .find_map(|(slot, value)| {
                    (slot.target == target
                        && slot.source == PostHandoffWriterSource::Resolve(target))
                    .then_some(*value)
                })
        })
    }

    fn validate_written_post_handoff_context(
        &self,
        context: &ResolvedPostHandoffEntryWriterContext,
        destination_site: PlacementSite,
        destination_len: usize,
    ) -> Result<(), InstallationDiagnostic> {
        context
            .invocation
            .validate_structure()
            .map_err(|diagnostic| InstallationDiagnostic(diagnostic.0))?;
        if context.installed_evidence != InstalledCodeEvidence::from_installed(self)
            || context.installed_code != self.identity
            || context.artifact != self.artifact()
            || context.destination_site != destination_site
            || context.destination_len != destination_len
            || context.context_abi() != POST_HANDOFF_WRITER_CONTEXT_ABI_V1
            || context.packed_words.first().copied() != Some(destination_site.base_address)
            || context.packed_words.len() != context.invocation.sources().len() + 1
        {
            return Err(InstallationDiagnostic(
                "written post-handoff destination does not retain its exact installed context, invocation, and destination geometry"
                    .into(),
            ));
        }
        let source_values = &context.packed_words[1..];
        context
            .invocation
            .validate_source_values(source_values)
            .map_err(|diagnostic| InstallationDiagnostic(diagnostic.0))?;
        for (slot, value) in context.invocation.sources().iter().zip(source_values) {
            let (exact, mismatch) = match slot.source {
                PostHandoffWriterSource::Resolve(target) => (
                    target == slot.target && self.resolve_entry_target(target) == Some(*value),
                    "is not an admitted entry in the exact installed artifact",
                ),
                PostHandoffWriterSource::Resolved(expected) => (
                    expected == *value && self.resolve_entry_target(slot.target) == Some(expected),
                    "does not match the exact installed realization",
                ),
            };
            if !exact {
                return Err(InstallationDiagnostic(format!(
                    "written post-handoff destination source slot for {:?} {mismatch}",
                    slot.target
                )));
            }
        }
        let replayed_fingerprint = non_authoritative_post_handoff_entry_writer_context_fingerprint(
            context.installed_code,
            context.artifact,
            context.destination_site,
            context.destination_len,
            &context.invocation,
            &context.packed_words,
        );
        if replayed_fingerprint != context.non_authoritative_fingerprint {
            return Err(InstallationDiagnostic(
                "written post-handoff destination context non-authoritative fingerprint fails exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Consume one exact activated, pinned, writable, unpublished destination
    /// and its non-clonable once-resolved context, then execute the writer into
    /// it. Failure returns both linear inputs; success retains the exact
    /// context with the still-unpublished destination for independent consumer
    /// replay before semantic validation and publication.
    pub fn write_prepared_post_handoff_destination<'mapping, 'bytes>(
        &self,
        context: ResolvedPostHandoffEntryWriterContext,
        plan: &PostHandoffWriterPlan,
        destination: ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    ) -> Result<
        WrittenPostHandoffWriterDestination<'mapping, 'bytes>,
        Box<DestinationWriteError<'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.execute_populated_post_handoff_entry_writer(
            &context,
            plan,
            destination.destination.bytes,
            destination.destination.site,
        ) {
            return Err(Box::new(DestinationWriteError {
                context,
                destination,
                diagnostic,
            }));
        }
        let PreparedPostHandoffWriterDestination {
            mapping,
            receipt,
            site,
            bytes,
        } = destination.destination;
        Ok(WrittenPostHandoffWriterDestination {
            mapping,
            receipt,
            site,
            bytes,
            context,
        })
    }

    /// Executes an atomic post-handoff writer using this installed code as the
    /// resolver authority for entry targets. Data symbols and entries from any
    /// other artifact fail before the destination is published.
    pub fn execute_post_handoff_entry_writer(
        &self,
        plan: &PostHandoffWriterPlan,
        destination: &mut [u8],
        destination_site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        self.validate_post_handoff_entry_writer(plan, destination.len(), destination_site)?;
        plan.execute(destination, destination_site, |target| {
            self.resolve_entry_target(target)
        })
    }

    fn resolve_entry_target(&self, target: RelocationTarget) -> Option<u64> {
        match target {
            RelocationTarget::Entry(identity) => self
                .validated
                .frozen
                .artifact
                .artifact
                .entry(identity)
                .and_then(|entry| {
                    self.validated
                        .frozen
                        .placement
                        .extent
                        .base()
                        .checked_add(entry.code_offset)
                }),
            RelocationTarget::Data(_) => None,
        }
    }

    fn contains_entry_target(&self, target: RelocationTarget) -> bool {
        match target {
            RelocationTarget::Entry(identity) => self
                .validated
                .frozen
                .artifact
                .artifact
                .entry(identity)
                .is_some(),
            RelocationTarget::Data(_) => false,
        }
    }
}

impl InstallationRegistryAuthority {
    pub const fn installation_scope(&self) -> InstallationScopeId {
        self.installed.validated.scope
    }

    /// Replay the opaque authority against one exact installed realization.
    /// This compares full evidence rather than normalized report identities.
    pub fn matches(&self, installed: &InstalledCode) -> bool {
        self.installed == InstalledCodeEvidence::from_installed(installed)
    }
}

fn non_authoritative_post_handoff_entry_writer_context_fingerprint(
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    destination_site: PlacementSite,
    destination_len: usize,
    invocation: &PostHandoffWriterInvocationPlan,
    packed_words: &[u64],
) -> NonAuthoritativeWriterContextFingerprint64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(installed_code.normalized_identity());
    mix(artifact.normalized_identity());
    mix(destination_site.base_address);
    mix(destination_len as u64);
    mix(invocation.fragment().report_fingerprint());
    mix(invocation.sources().len() as u64);
    for PostHandoffWriterSourceSlot { target, source } in invocation.sources() {
        match target {
            RelocationTarget::Entry(entry) => {
                mix(1);
                mix(entry.normalized_identity());
            }
            RelocationTarget::Data(data) => {
                mix(2);
                mix(data.normalized_identity());
            }
        }
        match source {
            PostHandoffWriterSource::Resolved(_) => mix(3),
            PostHandoffWriterSource::Resolve(RelocationTarget::Entry(entry)) => {
                mix(4);
                mix(entry.normalized_identity());
            }
            PostHandoffWriterSource::Resolve(RelocationTarget::Data(data)) => {
                mix(5);
                mix(data.normalized_identity());
            }
        }
    }
    mix(invocation.fit_constraints().len() as u64);
    for constraint in invocation.fit_constraints() {
        mix(constraint.source_slot as u64);
        mix(constraint.fit.source_width_bits.into());
        mix(constraint.fit.stored_width_bits.into());
        mix(match constraint.fit.interpretation {
            psi_layout_plans::IntegerInterpretation::Signed => 1,
            psi_layout_plans::IntegerInterpretation::Unsigned => 2,
        });
        for byte in constraint.field.as_bytes() {
            mix(u64::from(*byte));
        }
        mix(0xff);
    }
    mix(packed_words.len() as u64);
    for word in packed_words {
        mix(*word);
    }
    NonAuthoritativeWriterContextFingerprint64::from_compatibility_value(if hash == 0 {
        0xcbf2_9ce4_8422_2325
    } else {
        hash
    })
    .expect("fixed FNV normalization replaces zero")
}

/// One-shot authority to retire one exact installed realization. Required
/// completion facts are open provider vocabulary; quiescence and permission
/// transition remain mandatory lifecycle gates.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledCodeEvidence {
    installed: InstalledCodeId,
    validated: ValidatedPlacementEvidence,
    wx: WxEnforcement,
}

/// Opaque exact installed-realization context for downstream provider
/// admissions. It exposes no bytes, addresses, or constructors; consumers can
/// retain and compare it without reducing authority to compact report IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCodeContext(InstalledCodeEvidence);

impl InstalledCodeContext {
    pub fn occurrence_digest(&self) -> InstalledArtifactOccurrenceDigest {
        installed_artifact_occurrence_digest(&self.0)
    }
}

impl InstalledCodeEvidence {
    fn from_installed(installed: &InstalledCode) -> Self {
        Self {
            installed: installed.identity,
            validated: ValidatedPlacementEvidence::from_validated(&installed.validated),
            wx: installed.wx,
        }
    }
}

fn installed_artifact_occurrence_digest(
    evidence: &InstalledCodeEvidence,
) -> InstalledArtifactOccurrenceDigest {
    fn bytes(digest: &mut Sha256, value: &[u8]) {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value);
    }

    fn optional_u64(digest: &mut Sha256, value: Option<u64>) {
        match value {
            Some(value) => {
                digest.update([1]);
                digest.update(value.to_le_bytes());
            }
            None => digest.update([0]),
        }
    }

    let validated = &evidence.validated;
    let admitted = &validated.admission_evidence;
    let constraints = validated.constraints;
    let mut digest = Sha256::new();
    digest.update(b"omega.installed-artifact-occurrence.sha256.v1\0");
    digest.update(admitted.artifact.content().digest());
    digest.update(
        admitted
            .artifact
            .identity()
            .normalized_identity()
            .to_le_bytes(),
    );
    digest.update(admitted.admission.normalized_identity().to_le_bytes());
    match &admitted.container_proof {
        Some(proof) => {
            digest.update([1]);
            digest.update(proof.digest.digest());
            bytes(&mut digest, &proof.bytes);
        }
        None => digest.update([0]),
    }
    digest.update(evidence.installed.normalized_identity().to_le_bytes());
    digest.update(validated.placement.normalized_identity().to_le_bytes());
    digest.update(validated.scope.normalized_identity().to_le_bytes());
    digest.update([match validated.audience {
        InstallationAudience::DormantLocal => 1,
        InstallationAudience::FutureFetcher => 2,
    }]);
    match constraints.permitted_range() {
        Some(range) => {
            digest.update([1]);
            digest.update(range.start_inclusive().to_le_bytes());
            digest.update(range.end_exclusive().to_le_bytes());
        }
        None => digest.update([0]),
    }
    digest.update(constraints.alignment().to_le_bytes());
    digest.update([match constraints.phase() {
        psi_layout_plans::PlacementPhase::Build => 1,
        psi_layout_plans::PlacementPhase::Load => 2,
        psi_layout_plans::PlacementPhase::PostHandoff => 3,
    }]);
    optional_u64(
        &mut digest,
        constraints
            .machine_regime()
            .map(|identity| identity.normalized_identity()),
    );
    optional_u64(
        &mut digest,
        constraints
            .installation_scope()
            .map(|identity| identity.normalized_identity()),
    );
    digest.update(validated.base.to_le_bytes());
    digest.update(validated.length.to_le_bytes());
    digest.update(validated.address_space.normalized_identity().to_le_bytes());
    digest.update((validated.rights.identities().count() as u64).to_le_bytes());
    for right in validated.rights.identities() {
        digest.update(right.normalized_identity().to_le_bytes());
    }
    digest.update(validated.provenance.normalized_identity().to_le_bytes());
    digest.update(validated.era.normalized_identity().to_le_bytes());
    digest.update(validated.lineage.normalized_identity().to_le_bytes());
    bytes(&mut digest, &validated.final_bytes);
    digest.update(
        validated
            .realized_footprint
            .normalized_identity()
            .to_le_bytes(),
    );
    digest.update(validated.validation.normalized_identity().to_le_bytes());
    digest.update([match evidence.wx {
        WxEnforcement::HardwareEnforced => 1,
        WxEnforcement::ConventionOnly => 2,
        WxEnforcement::Unsupported => 3,
    }]);
    InstalledArtifactOccurrenceDigest::from_sha256(digest.finalize().into())
}

#[derive(Debug, PartialEq, Eq)]
pub struct RetirementAuthority {
    installed: InstalledCodeEvidence,
    required_facts: std::collections::BTreeSet<RetirementFactDigest>,
}

impl RetirementAuthority {
    pub fn from_admitted_provider(
        installed: &InstalledCode,
        required_facts: impl IntoIterator<Item = RetirementFactDigest>,
    ) -> Self {
        Self {
            installed: InstalledCodeEvidence::from_installed(installed),
            required_facts: required_facts.into_iter().collect(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RetirementReceipt {
    installed: InstalledCodeEvidence,
    executors_quiesced: bool,
    execute_disabled: bool,
    write_authority_restored: bool,
    established_facts: std::collections::BTreeSet<RetirementFactDigest>,
}

impl RetirementReceipt {
    pub fn from_provider(
        installed: &InstalledCode,
        executors_quiesced: bool,
        execute_disabled: bool,
        write_authority_restored: bool,
        established_facts: impl IntoIterator<Item = RetirementFactDigest>,
    ) -> Self {
        Self {
            installed: InstalledCodeEvidence::from_installed(installed),
            executors_quiesced,
            execute_disabled,
            write_authority_restored,
            established_facts: established_facts.into_iter().collect(),
        }
    }
}

/// Result of synchronous retirement. The previous artifact remains reusable;
/// the exact destination returns to the W+NX `CodePlacement` state.
#[derive(Debug)]
pub struct RetiredInstallation {
    previous_artifact: AdmittedArtifact,
    placement: CodePlacement,
}

impl RetiredInstallation {
    pub const fn previous_artifact(&self) -> &AdmittedArtifact {
        &self.previous_artifact
    }

    pub fn into_placement(self) -> CodePlacement {
        self.placement
    }
}

pub fn retire_installed(
    installed: InstalledCode,
    authority: RetirementAuthority,
    receipt: RetirementReceipt,
) -> Result<RetiredInstallation, Box<RetirementError>> {
    let evidence = InstalledCodeEvidence::from_installed(&installed);
    let mismatch = if authority.installed != evidence {
        Some("retirement authority is not scoped to this installed code")
    } else if receipt.installed != evidence {
        Some("retirement receipt does not match installed code")
    } else if !receipt.executors_quiesced {
        Some("retirement receipt does not establish executor quiescence")
    } else if !receipt.execute_disabled {
        Some("retirement receipt does not establish execute removal")
    } else if !receipt.write_authority_restored {
        Some("retirement receipt does not restore placement write authority")
    } else if !authority
        .required_facts
        .is_subset(&receipt.established_facts)
    {
        Some("retirement receipt lacks required completion facts")
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(RetirementError {
            installed,
            authority,
            receipt,
            diagnostic: InstallationDiagnostic(message.into()),
        }));
    }

    let ValidatedPlacement { frozen, .. } = installed.validated;
    Ok(RetiredInstallation {
        previous_artifact: frozen.artifact,
        placement: frozen.placement,
    })
}

#[derive(Debug)]
pub struct RetirementError {
    installed: InstalledCode,
    authority: RetirementAuthority,
    receipt: RetirementReceipt,
    diagnostic: InstallationDiagnostic,
}

impl RetirementError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InstalledCode, RetirementAuthority, RetirementReceipt) {
        (self.installed, self.authority, self.receipt)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationDiagnostic(pub String);

impl std::fmt::Display for InstallationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for InstallationDiagnostic {}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_extents::{
        ExtentDiagnostic, ExtentLineageId, ExtentRightId, ExtentRootGrant, MappingEraId,
        MappingGrant, MappingGrantId, MappingId, MappingSourceMode, TranslationActivationFactId,
        TranslationActivationReceipt, TranslationInstallObligations, TranslationReleaseObligations,
        map_owned,
    };
    use psi_layout_plans::{
        ArtifactInstallationScopeId, ByteOrder, IntegerInterpretation, MaterializationWrite,
        PlacementAddressRange, PlacementPhase, PostHandoffWriterSource, PostHandoffWriterStep,
        StoredIntegerFit,
    };

    fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, InstallationDiagnostic>) -> T {
        constructor(identity).expect("normalized installation identity")
    }

    fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    fn extent_provider_issuance(seed: u64) -> psi_extents::ExtentProviderIssuance {
        let base = seed * 16;
        psi_extents::ExtentProviderIssuance::from_normalized_identities([
            base + 1,
            base + 2,
            base + 3,
            base + 4,
            base + 5,
            base + 6,
            base + 7,
            base + 8,
            base + 9,
            base + 10,
            base + 11,
            base + 12,
            base + 13,
        ])
        .expect("normalized provider issuance")
    }

    fn entry_id(identity: u64) -> EntryStubId {
        EntryStubId::from_normalized_identity(identity).expect("normalized entry identity")
    }

    fn rights(identities: &[u64]) -> ExtentRights {
        ExtentRights::from_normalized_identities(
            identities
                .iter()
                .copied()
                .map(|identity| extent_id(identity, ExtentRightId::from_normalized_identity)),
        )
    }

    fn artifact_placement_constraints() -> PlacementConstraints {
        let scope = ArtifactInstallationScopeId::from_normalized_identity(61)
            .expect("artifact installation scope");
        PlacementConstraints::new(
            Some(PlacementAddressRange::new(0x1000, 0x1_0000).expect("placement range")),
            4096,
            PlacementPhase::PostHandoff,
            None,
            Some(scope),
        )
        .expect("placement constraints")
    }

    fn authority_commitments(constraints: PlacementConstraints) -> ArtifactAuthorityCommitments {
        ArtifactAuthorityCommitments::from_canonical_evidence(
            id(30, MachineContractSetId::from_normalized_identity),
            b"test imported contract set",
            id(31, MachineFootprintId::from_normalized_identity),
            b"test declared footprint",
            constraints
                .machine_regime()
                .map(|identity| (identity, b"test machine regime".as_slice())),
            constraints
                .installation_scope()
                .map(|identity| (identity, b"test installation scope".as_slice())),
        )
    }

    fn artifact(identity: u64) -> Artifact {
        artifact_with(
            identity,
            artifact_placement_constraints(),
            id(33, EntrySetId::from_normalized_identity),
            entry_id(identity + 1000),
        )
    }

    fn colliding_artifact(identity: u64, fill: u8) -> Artifact {
        let constraints = artifact_placement_constraints();
        Artifact::from_canonical_decode(
            id(identity, ArtifactId::from_normalized_identity),
            Architecture::X86_64,
            vec![fill; 64],
            id(30, MachineContractSetId::from_normalized_identity),
            id(31, MachineFootprintId::from_normalized_identity),
            id(32, PlacementPlanId::from_normalized_identity),
            constraints,
            id(33, EntrySetId::from_normalized_identity),
            vec![ArtifactEntry::from_canonical_decode(
                entry_id(identity + 1000),
                16,
            )],
            id(34, RelocationSetId::from_normalized_identity),
            Vec::new(),
            authority_commitments(constraints),
        )
        .expect("colliding artifact")
    }

    fn artifact_with(
        identity: u64,
        constraints: PlacementConstraints,
        entry_set: EntrySetId,
        entry: EntryStubId,
    ) -> Artifact {
        let commitments = authority_commitments(constraints);
        Artifact::from_canonical_decode(
            id(identity, ArtifactId::from_normalized_identity),
            Architecture::X86_64,
            vec![0; 64],
            id(30, MachineContractSetId::from_normalized_identity),
            id(31, MachineFootprintId::from_normalized_identity),
            id(32, PlacementPlanId::from_normalized_identity),
            constraints,
            entry_set,
            vec![ArtifactEntry::from_canonical_decode(entry, 16)],
            id(34, RelocationSetId::from_normalized_identity),
            Vec::new(),
            commitments,
        )
        .expect("artifact")
    }

    fn admit(candidate: &Artifact) -> AdmittedArtifact {
        admit_executable(
            candidate,
            ArtifactAdmissionEvidence::from_validator(
                id(40, AdmissionReceiptId::from_normalized_identity),
                candidate,
                true,
            ),
        )
        .expect("admitted artifact")
    }

    fn placement_extent(lineage: u64, base: u64, length: u64) -> Extent {
        ExtentRootGrant::from_admitted_provider(
            extent_provider_issuance(lineage),
            extent_id(lineage, ExtentLineageId::from_normalized_identity),
            extent_id(50, AddressSpaceId::from_normalized_identity),
            rights(&[51]),
            extent_id(52, ExtentProvenanceId::from_normalized_identity),
            extent_id(53, MappingEraId::from_normalized_identity),
        )
        .mint(base, length)
        .expect("placement extent")
    }

    fn activated_writer_mapping(base: u64, length: u64) -> MappedExtent<'static> {
        let source_space = extent_id(150, AddressSpaceId::from_normalized_identity);
        let destination_space = extent_id(151, AddressSpaceId::from_normalized_identity);
        let source_rights = rights(&[152]);
        let destination_rights = rights(&[153]);
        let writer_rights = rights(&[154]);
        let source = ExtentRootGrant::from_admitted_provider(
            extent_provider_issuance(150),
            extent_id(150, ExtentLineageId::from_normalized_identity),
            source_space,
            source_rights.clone(),
            extent_id(155, ExtentProvenanceId::from_normalized_identity),
            extent_id(156, MappingEraId::from_normalized_identity),
        )
        .mint(0x20_000, length)
        .expect("writer mapping source");
        let destination = ExtentRootGrant::from_admitted_provider(
            extent_provider_issuance(151),
            extent_id(151, ExtentLineageId::from_normalized_identity),
            destination_space,
            destination_rights.clone(),
            extent_id(157, ExtentProvenanceId::from_normalized_identity),
            extent_id(158, MappingEraId::from_normalized_identity),
        )
        .mint(base, length)
        .expect("writer mapping destination");
        let activation = extent_id(159, TranslationActivationFactId::from_normalized_identity);
        let grant = MappingGrant::from_admitted_provider(
            extent_id(160, MappingGrantId::from_normalized_identity),
            MappingSourceMode::Owned,
            source_space,
            destination_space,
            source_rights,
            destination_rights,
            writer_rights,
            extent_id(161, ExtentProvenanceId::from_normalized_identity),
            extent_id(162, MappingEraId::from_normalized_identity),
            TranslationInstallObligations::from_normalized_facts([activation]),
            TranslationReleaseObligations::default(),
        );
        let pending = map_owned(
            source,
            destination,
            extent_id(163, MappingId::from_normalized_identity),
            &grant,
        )
        .expect("writer pending mapping");
        let receipt = TranslationActivationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            true,
            [activation],
        );
        pending.complete(receipt).expect("activated writer mapping")
    }

    fn prepared_destination_receipt(
        mapping: &MappedExtent<'_>,
        identity: u64,
    ) -> DestinationPreparationReceipt {
        DestinationPreparationReceipt::from_admitted_provider(
            id(
                identity,
                DestinationPreparationReceiptId::from_normalized_identity,
            ),
            &mapping.receipt_context(),
            rights(&[154]),
            true,
            true,
        )
    }

    fn placement_authority(placement: u64, base: u64, length: u64) -> CodePlacementAuthority {
        placement_authority_with_constraints(
            placement,
            base,
            length,
            artifact_placement_constraints(),
        )
    }

    fn placement_authority_with_constraints(
        placement: u64,
        base: u64,
        length: u64,
        constraints: PlacementConstraints,
    ) -> CodePlacementAuthority {
        let scope = ArtifactInstallationScopeId::from_normalized_identity(61)
            .expect("artifact installation scope");
        let extent = placement_extent(placement, base, length);
        CodePlacementAuthority::from_admitted_provider(
            id(placement, CodePlacementId::from_normalized_identity),
            id(61, InstallationScopeId::from_normalized_identity),
            InstallationAudience::FutureFetcher,
            &extent,
            rights(&[51]),
            constraints,
            PlacementSite {
                base_address: base,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: Some(scope),
            },
        )
    }

    fn relocatable_artifact(
        identity: u64,
        architecture: Architecture,
        code: Vec<u8>,
        relocations: Vec<DecodedArtifactRelocation>,
    ) -> Artifact {
        let constraints = artifact_placement_constraints();
        Artifact::from_canonical_decode(
            id(identity, ArtifactId::from_normalized_identity),
            architecture,
            code,
            id(30, MachineContractSetId::from_normalized_identity),
            id(31, MachineFootprintId::from_normalized_identity),
            id(32, PlacementPlanId::from_normalized_identity),
            constraints,
            id(33, EntrySetId::from_normalized_identity),
            vec![ArtifactEntry::from_canonical_decode(
                entry_id(identity + 1000),
                0,
            )],
            id(34, RelocationSetId::from_normalized_identity),
            relocations,
            authority_commitments(constraints),
        )
        .expect("relocatable artifact")
    }

    fn frozen(admitted: &AdmittedArtifact, placement_identity: u64, base: u64) -> FrozenPlacement {
        let placement = placement_authority(placement_identity, base, 4096)
            .claim(placement_extent(placement_identity, base, 4096))
            .expect("placement");
        let materialized = materialize_admitted_artifact(admitted, &placement, |_| None)
            .expect("artifact without relocations materializes");
        materialize_and_freeze(
            admitted,
            placement,
            materialized.clone(),
            MaterializationReceipt::from_materialized(
                &materialized,
                id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("frozen placement")
    }

    fn certificate(frozen: &FrozenPlacement, identity: u64) -> FinalValidationCertificate {
        FinalValidationCertificate::from_validator(
            id(identity, FinalValidationId::from_normalized_identity),
            frozen,
            true,
        )
    }

    fn installed_code(admitted: &AdmittedArtifact, placement: u64, base: u64) -> InstalledCode {
        let frozen = frozen(admitted, placement, base);
        let certificate = certificate(&frozen, 80 + placement);
        let validated =
            validate_final_placement(frozen, &certificate).expect("validated placement");
        let authority = InstallAuthority::from_admitted_provider(&validated);
        let receipt = InstallationReceipt::from_provider(
            id(200 + placement, InstalledCodeId::from_normalized_identity),
            &validated,
            true,
            WxEnforcement::HardwareEnforced,
        );
        install_validated(validated, authority, receipt).expect("installed code")
    }

    #[test]
    fn canonical_materializer_patches_x86_relative_targets_and_binds_the_receipt() {
        let target = RelocationTarget::Entry(entry_id(1001));
        let mut code = vec![0x90; 64];
        code[0] = 0xe8;
        code[1..5].fill(0);
        let candidate = relocatable_artifact(
            1,
            Architecture::X86_64,
            code,
            vec![DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::X86Relative32,
                destination_offset: 1,
                target,
                addend: -4,
            }],
        );
        let admitted = admit(&candidate);
        let placement = placement_authority(100, 0x1000, 4096)
            .claim(placement_extent(100, 0x1000, 4096))
            .expect("placement");

        let materialized = materialize_admitted_artifact(&admitted, &placement, |candidate| {
            (candidate == target).then_some(0x1024)
        })
        .expect("materialized bytes");
        assert_eq!(
            i32::from_le_bytes(materialized.bytes()[1..5].try_into().unwrap()),
            0x1b
        );

        let frozen = materialize_and_freeze(
            &admitted,
            placement,
            materialized.clone(),
            MaterializationReceipt::from_materialized(
                &materialized,
                id(31, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("receipt is bound to canonical materializer output");
        assert_eq!(frozen.bytes(), materialized.bytes());
        assert_eq!(frozen.final_bytes(), materialized.final_bytes());

        let certificate = FinalValidationCertificate::from_validator(
            id(181, FinalValidationId::from_normalized_identity),
            &frozen,
            true,
        );
        let validated = validate_final_placement(frozen, &certificate).unwrap();
        let authority = InstallAuthority::from_admitted_provider(&validated);
        let receipt = InstallationReceipt::from_provider(
            id(281, InstalledCodeId::from_normalized_identity),
            &validated,
            true,
            WxEnforcement::HardwareEnforced,
        );
        let installed = install_validated(validated, authority, receipt).unwrap();
        assert!(
            installed
                .binds_exact_materialized_artifact_bytes(candidate.code(), materialized.bytes())
        );
        let mut changed_source = candidate.code().to_vec();
        changed_source[0] ^= 1;
        assert!(
            !installed
                .binds_exact_materialized_artifact_bytes(&changed_source, materialized.bytes())
        );
        let mut changed_final = materialized.bytes().to_vec();
        changed_final[1] ^= 1;
        assert!(
            !installed.binds_exact_materialized_artifact_bytes(candidate.code(), &changed_final)
        );
    }

    #[test]
    fn aarch64_materialization_validates_the_relocated_instruction_shape() {
        let target = RelocationTarget::Entry(entry_id(1002));
        let mut branch = vec![0; 64];
        branch[..4].copy_from_slice(&0x9400_0000u32.to_le_bytes());
        let candidate = relocatable_artifact(
            2,
            Architecture::Aarch64,
            branch,
            vec![DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Aarch64Branch26,
                destination_offset: 0,
                target,
                addend: 0,
            }],
        );
        let admitted = admit(&candidate);
        let placement = placement_authority(101, 0x1000, 4096)
            .claim(placement_extent(101, 0x1000, 4096))
            .expect("placement");
        let materialized = materialize_admitted_artifact(&admitted, &placement, |_| Some(0x1010))
            .expect("AArch64 branch materialization");
        assert_eq!(
            u32::from_le_bytes(materialized.bytes()[..4].try_into().unwrap()),
            0x9400_0004
        );

        let invalid = relocatable_artifact(
            3,
            Architecture::Aarch64,
            vec![0; 64],
            vec![DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Aarch64Branch26,
                destination_offset: 0,
                target,
                addend: 0,
            }],
        );
        let invalid = admit(&invalid);
        let error = materialize_admitted_artifact(&invalid, &placement, |_| Some(0x1010))
            .expect_err("relocation cannot rewrite an arbitrary instruction");
        assert!(error.0.contains("B/BL"));
    }

    #[test]
    fn aarch64_materialization_patches_page_pairs_and_absolute_data() {
        let target = RelocationTarget::Entry(entry_id(1004));
        let mut code = vec![0; 64];
        code[..4].copy_from_slice(&0x9000_0000u32.to_le_bytes());
        code[4..8].copy_from_slice(&0x9100_0000u32.to_le_bytes());
        let candidate = relocatable_artifact(
            4,
            Architecture::Aarch64,
            code,
            vec![
                DecodedArtifactRelocation {
                    kind: ArtifactRelocationKind::Aarch64Page21,
                    destination_offset: 0,
                    target,
                    addend: 0,
                },
                DecodedArtifactRelocation {
                    kind: ArtifactRelocationKind::Aarch64PageOffset12,
                    destination_offset: 4,
                    target,
                    addend: 0,
                },
                DecodedArtifactRelocation {
                    kind: ArtifactRelocationKind::Absolute64,
                    destination_offset: 8,
                    target,
                    addend: -6,
                },
            ],
        );
        let admitted = admit(&candidate);
        let placement = placement_authority(102, 0x1000, 4096)
            .claim(placement_extent(102, 0x1000, 4096))
            .expect("placement");
        let materialized = materialize_admitted_artifact(&admitted, &placement, |_| Some(0x3456))
            .expect("AArch64 page-pair materialization");

        assert_eq!(
            u32::from_le_bytes(materialized.bytes()[..4].try_into().unwrap()),
            0xd000_0000
        );
        assert_eq!(
            u32::from_le_bytes(materialized.bytes()[4..8].try_into().unwrap()),
            0x9111_5800
        );
        assert_eq!(
            u64::from_le_bytes(materialized.bytes()[8..16].try_into().unwrap()),
            0x3450
        );
    }

    #[test]
    fn admitted_artifact_is_reusable_but_each_placement_is_linear() {
        let candidate = artifact(1);
        let admitted = admit(&candidate);
        let second_reference = admitted.clone();

        let frozen = frozen(&admitted, 100, 0x1000);
        let certificate = certificate(&frozen, 180);
        let validated =
            validate_final_placement(frozen, &certificate).expect("validated placement");
        let authority = InstallAuthority::from_admitted_provider(&validated);
        let receipt = InstallationReceipt::from_provider(
            id(200, InstalledCodeId::from_normalized_identity),
            &validated,
            true,
            WxEnforcement::HardwareEnforced,
        );
        let installed = install_validated(validated, authority, receipt).expect("installed code");
        assert_eq!(installed.artifact(), second_reference.artifact().identity());
        assert_eq!(installed.wx(), WxEnforcement::HardwareEnforced);
    }

    #[test]
    fn installation_receipt_cannot_substitute_colliding_normalized_artifact() {
        let first = admit(&colliding_artifact(1, 0x90));
        let second = admit(&colliding_artifact(1, 0xcc));

        let first_frozen = frozen(&first, 114, 0xc000);
        let first_certificate = certificate(&first_frozen, 194);
        let first_validated = validate_final_placement(first_frozen, &first_certificate)
            .expect("first validated placement");

        let second_frozen = frozen(&second, 114, 0xc000);
        let second_certificate = certificate(&second_frozen, 194);
        let second_validated = validate_final_placement(second_frozen, &second_certificate)
            .expect("second validated placement");

        let authority = InstallAuthority::from_admitted_provider(&first_validated);
        let substituted_receipt = InstallationReceipt::from_provider(
            id(314, InstalledCodeId::from_normalized_identity),
            &second_validated,
            true,
            WxEnforcement::HardwareEnforced,
        );
        let error = install_validated(first_validated, authority, substituted_receipt)
            .expect_err("exact frozen bytes must outrank colliding report identities");
        assert!(error.diagnostic().0.contains("receipt"));
    }

    #[test]
    fn artifact_content_digest_is_derived_from_exact_semantics() {
        let first = colliding_artifact(1, 0x90);
        let second = colliding_artifact(1, 0xcc);

        assert_eq!(first.identity(), second.identity());
        assert_ne!(first.code(), second.code());
        assert_ne!(first.content(), second.content());
        assert_ne!(first.content().digest(), second.content().digest());
    }

    #[test]
    fn local_fnv_sites_are_explicitly_non_authoritative() {
        let root = include_str!("lib.rs");
        let container = include_str!("container.rs");
        let container_bytes = include_str!("container_bytes.rs");
        let materializer = include_str!("materializer.rs");

        for forbidden in [
            ["normalized_id!", "(ArtifactContent"].concat(),
            ["normalized_id!", "(ProofPayload"].concat(),
            ["normalized_id!", "(FinalBytes"].concat(),
            ["pub const fn", " from_digest"].concat(),
        ] {
            assert!(!root.contains(&forbidden));
        }
        assert!(root.contains("non_authoritative_post_handoff_entry_writer_context_fingerprint"));
        assert!(container.contains("NonAuthoritativeContainerFingerprint64"));
        assert!(container_bytes.contains("non_authoritative_informational_section_fingerprint"));
        assert!(materializer.contains("FinalBytesDigest"));

        let fnv_offset_basis = ["0x", "cbf"].concat();
        let fnv_offset_basis_count = [root, container, container_bytes, materializer]
            .into_iter()
            .map(|source| source.matches(&fnv_offset_basis).count())
            .sum::<usize>();
        assert_eq!(
            fnv_offset_basis_count, 4,
            "new FNV sites require explicit non-authoritative classification"
        );
    }

    #[test]
    fn materialization_cannot_substitute_another_artifact() {
        let first = admit(&artifact(1));
        let second = admit(&artifact(2));
        let placement = placement_authority(101, 0x2000, 4096)
            .claim(placement_extent(101, 0x2000, 4096))
            .expect("placement");
        let first_materialized = materialize_admitted_artifact(&first, &placement, |_| None)
            .expect("first artifact materialization");
        let second_materialized = materialize_admitted_artifact(&second, &placement, |_| None)
            .expect("second artifact materialization");
        let error = materialize_and_freeze(
            &first,
            placement,
            first_materialized,
            MaterializationReceipt::from_materialized(
                &second_materialized,
                id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect_err("artifact substitution rejects");
        assert!(error.diagnostic().0.contains("exact canonical output"));
        let (_placement, _materialized, _receipt) = (*error).into_parts();
    }

    #[test]
    fn canonical_materializer_output_cannot_substitute_another_artifact() {
        let first = admit(&colliding_artifact(1, 0x90));
        let second = admit(&colliding_artifact(1, 0xcc));
        let placement = placement_authority(111, 0x9000, 4096)
            .claim(placement_extent(111, 0x9000, 4096))
            .expect("placement");
        let materialized = materialize_admitted_artifact(&second, &placement, |_| None)
            .expect("second artifact materialization");
        let receipt = MaterializationReceipt::from_materialized(
            &materialized,
            id(71, MachineFootprintId::from_normalized_identity),
            true,
        );
        let error = materialize_and_freeze(&first, placement, materialized, receipt)
            .expect_err("canonical output substitution rejects");
        assert!(
            error
                .diagnostic()
                .0
                .contains("materializer output does not retain the exact admitted artifact")
        );
        let (_placement, _materialized, _receipt) = (*error).into_parts();
    }

    #[test]
    fn final_bytes_digest_binds_content_and_placement_base() {
        let admitted = admit(&artifact(1));
        let first_placement = placement_authority(114, 0x4000, 4096)
            .claim(placement_extent(114, 0x4000, 4096))
            .expect("first placement");
        let second_placement = placement_authority(115, 0x8000, 4096)
            .claim(placement_extent(115, 0x8000, 4096))
            .expect("second placement");
        let first = materialize_admitted_artifact(&admitted, &first_placement, |_| None)
            .expect("first materialization");
        let second = materialize_admitted_artifact(&admitted, &second_placement, |_| None)
            .expect("second materialization");

        assert_eq!(first.bytes(), second.bytes());
        assert_ne!(first.base_address(), second.base_address());
        assert_ne!(first.final_bytes(), second.final_bytes());
        assert_ne!(first.final_bytes().digest(), second.final_bytes().digest());
    }

    #[test]
    fn admission_evidence_cannot_substitute_placement_constraints() {
        let candidate = artifact(1);
        let weaker = PlacementConstraints::unconstrained(PlacementPhase::PostHandoff);
        let substituted = artifact_with(
            1,
            weaker,
            candidate.0.entry_set,
            candidate.0.entries[0].identity,
        );
        let error = admit_executable(
            &candidate,
            ArtifactAdmissionEvidence::from_validator(
                id(40, AdmissionReceiptId::from_normalized_identity),
                &substituted,
                true,
            ),
        )
        .expect_err("admission evidence must pin the decoded placement constraints");
        assert!(error.0.contains("does not match canonical candidate"));
    }

    #[test]
    fn admission_evidence_cannot_substitute_the_selected_entry_set() {
        let candidate = artifact(1);
        let substituted = artifact_with(
            1,
            candidate.0.placement_constraints,
            id(34, EntrySetId::from_normalized_identity),
            candidate.0.entries[0].identity,
        );
        let error = admit_executable(
            &candidate,
            ArtifactAdmissionEvidence::from_validator(
                id(40, AdmissionReceiptId::from_normalized_identity),
                &substituted,
                true,
            ),
        )
        .expect_err("admission evidence must pin the decoded entry set");
        assert!(error.0.contains("does not match canonical candidate"));
    }

    #[test]
    fn admitted_artifact_selects_only_its_canonical_entry_targets() {
        let candidate = artifact(1);
        let selected = entry_id(1001);
        let admitted = admit(&candidate);
        assert_eq!(
            admitted
                .selected_entry_target(selected)
                .expect("selected entry target"),
            RelocationTarget::Entry(selected)
        );
        let foreign = entry_id(1002);
        assert!(admitted.selected_entry_target(foreign).is_err());
    }

    #[test]
    fn installed_code_resolves_only_its_entries_for_atomic_post_handoff_writers() {
        let admitted = admit(&artifact(1));
        let installed = installed_code(&admitted, 110, 0x8000);
        let selected = entry_id(1001);
        let target = installed
            .selected_entry_target(selected)
            .expect("installed selected entry");
        let writer = |target| PostHandoffWriterPlan {
            byte_len: 8,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "address".into(),
                    target,
                    container_byte_offset: 0,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 64,
                    stored_integer_fit: None,
                },
                source: PostHandoffWriterSource::Resolve(target),
            }],
        };
        let destination_site = PlacementSite {
            base_address: 0x9000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: None,
        };

        let mut destination = [0u8; 8];
        installed
            .execute_post_handoff_entry_writer(&writer(target), &mut destination, destination_site)
            .expect("installed entry writer");
        assert_eq!(u64::from_le_bytes(destination), 0x8010);

        let mut narrow = writer(target);
        narrow.steps[0].write.container_width_bits = 16;
        narrow.steps[0].write.width = 16;
        narrow.steps[0].write.stored_integer_fit = Some(StoredIntegerFit {
            source_width_bits: 64,
            stored_width_bits: 16,
            interpretation: IntegerInterpretation::Signed,
        });
        let error = installed
            .populate_post_handoff_entry_writer_context(
                &narrow,
                destination.len(),
                destination_site,
            )
            .expect_err("an installed address outside stored range cannot populate a context");
        assert!(error.0.contains("does not fit"), "{}", error.0);

        let mut checked_writer = writer(target);
        let mut low_half = checked_writer.steps[0].clone();
        low_half.write.width = 32;
        let mut high_half = low_half.clone();
        high_half.write.destination_lsb = 32;
        high_half.write.source_lsb = 32;
        checked_writer.steps = vec![low_half, high_half];
        let context = installed
            .populate_post_handoff_entry_writer_context(
                &checked_writer,
                destination.len(),
                destination_site,
            )
            .expect("installed resolver populates opaque writer context");
        assert_eq!(context.installed_code(), installed.identity());
        assert_eq!(context.artifact(), installed.artifact());
        assert_eq!(context.source_slot_count(), 1);
        assert_eq!(context.packed_byte_len(), 16);
        assert_eq!(context.context_abi(), POST_HANDOFF_WRITER_CONTEXT_ABI_V1);
        let invocation = checked_writer
            .lower_reusable_fragment()
            .expect("checked writer has one reusable fragment");
        assert!(context.binds_invocation(&invocation));
        assert_eq!(
            context.normalized_fragment_report_fingerprint(),
            invocation.fragment().report_fingerprint()
        );
        assert_ne!(
            context
                .non_authoritative_fingerprint()
                .compatibility_value(),
            0
        );
        let context_debug = format!("{context:?}");
        assert!(!context_debug.contains("packed_words"));
        assert!(!context_debug.contains("destination_site"));
        let mut populated_destination = [0u8; 8];
        installed
            .execute_populated_post_handoff_entry_writer(
                &context,
                &checked_writer,
                &mut populated_destination,
                destination_site,
            )
            .expect("populated context executes without public address resolution");
        assert_eq!(u64::from_le_bytes(populated_destination), 0x8010);
        let mut unchanged_context_destination = [0xa5; 8];
        let error = installed
            .execute_populated_post_handoff_entry_writer(
                &context,
                &checked_writer,
                &mut unchanged_context_destination,
                PlacementSite {
                    base_address: destination_site.base_address + 8,
                    ..destination_site
                },
            )
            .expect_err("destination-site drift must reject the populated context");
        assert!(error.0.contains("exact installed code, plan, destination"));
        assert_eq!(unchanged_context_destination, [0xa5; 8]);

        let foreign = RelocationTarget::Entry(entry_id(1002));
        let mut unchanged = [0xa5u8; 8];
        let error = installed
            .execute_post_handoff_entry_writer(&writer(foreign), &mut unchanged, destination_site)
            .expect_err("foreign artifact entry must not resolve");
        assert!(error.0.contains("exact installed artifact"));
        assert_eq!(unchanged, [0xa5; 8]);

        let mut stale = writer(target);
        stale.steps[0].source = PostHandoffWriterSource::Resolved(0xdead_beef);
        let error = installed
            .execute_post_handoff_entry_writer(&stale, &mut unchanged, destination_site)
            .expect_err("pre-resolved address from another realization must reject");
        assert!(error.0.contains("exact installed realization"));
        assert_eq!(unchanged, [0xa5; 8]);
    }

    #[test]
    fn writer_context_cannot_substitute_collision_equal_installed_realization() {
        let first = installed_code(&admit(&colliding_artifact(1, 0x90)), 110, 0x8000);
        let second = installed_code(&admit(&colliding_artifact(1, 0xcc)), 110, 0x8000);
        let target = RelocationTarget::Entry(entry_id(1001));
        let writer = PostHandoffWriterPlan {
            byte_len: 8,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "address".into(),
                    target,
                    container_byte_offset: 0,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 64,
                    stored_integer_fit: None,
                },
                source: PostHandoffWriterSource::Resolve(target),
            }],
        };
        let site = PlacementSite {
            base_address: 0x9000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: None,
        };
        let context = second
            .populate_post_handoff_entry_writer_context(&writer, 8, site)
            .expect("second realization produces a context");
        let mut destination = [0xa5; 8];
        let error = first
            .execute_populated_post_handoff_entry_writer(&context, &writer, &mut destination, site)
            .expect_err("exact installed realization mismatch must reject");
        assert!(error.0.contains("exact installed code"));
        assert_eq!(destination, [0xa5; 8]);
    }

    #[test]
    fn prepared_writer_consumes_an_activated_pinned_writable_unpublished_destination() {
        let target = RelocationTarget::Entry(entry_id(1001));
        let installed = installed_code(&admit(&artifact(1)), 106, 0x8000);
        let writer = PostHandoffWriterPlan {
            byte_len: 8,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "address".into(),
                    target,
                    container_byte_offset: 0,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 64,
                    stored_integer_fit: None,
                },
                source: PostHandoffWriterSource::Resolve(target),
            }],
        };
        let site = PlacementSite {
            base_address: 0x9000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: None,
        };
        let context = installed
            .populate_post_handoff_entry_writer_context(&writer, 8, site)
            .expect("exact installed writer context");
        let context_fingerprint = context.non_authoritative_fingerprint();
        let invocation = writer
            .lower_reusable_fragment()
            .expect("exact retained invocation");
        let mapping = activated_writer_mapping(site.base_address, 8);
        let receipt = prepared_destination_receipt(&mapping, 170);
        let mut bytes = [0u8; 8];
        let destination =
            PreparedPostHandoffWriterDestination::claim(mapping, receipt, site, &mut bytes)
                .expect("activated pinned writable unpublished destination");
        let destination = destination
            .into_validated_for_writer_preparation()
            .expect("destination replay precedes symbolic-source writing");
        let mut written = installed
            .write_prepared_post_handoff_destination(context, &writer, destination)
            .expect("prepared writer consumes destination");
        assert_eq!(written.installed_code(), installed.identity());
        assert_eq!(written.artifact(), installed.artifact());
        assert_eq!(written.site(), site);
        assert_eq!(
            written.non_authoritative_writer_context_fingerprint(),
            context_fingerprint
        );
        assert!(written.binds_invocation(&invocation));
        written.context.non_authoritative_fingerprint =
            NonAuthoritativeWriterContextFingerprint64::from_compatibility_value(
                context_fingerprint.compatibility_value() ^ 1,
            )
            .unwrap();
        let error = written
            .into_validated_for_consumer(&installed)
            .expect_err("consumer replay must reject context corruption");
        assert!(
            error
                .diagnostic()
                .0
                .contains("fingerprint fails exact replay")
        );
        let mut written = (*error).into_written();
        written.context.non_authoritative_fingerprint = context_fingerprint;
        let written = written
            .into_validated_for_consumer(&installed)
            .expect("repaired exact context supports consumer retry");
        assert!(written.context().binds_invocation(&invocation));
        assert_eq!(
            u64::from_le_bytes(written.bytes().try_into().unwrap()),
            0x8010
        );
        let (_mapping, receipt, returned_site, returned_bytes) = written.into_parts();
        assert_eq!(receipt.identity().normalized_identity(), 170);
        assert_eq!(returned_site, site);
        assert_eq!(
            u64::from_le_bytes(returned_bytes.try_into().unwrap()),
            0x8010
        );
    }

    #[test]
    fn destination_claim_and_writer_failure_return_linear_authority() {
        let site = PlacementSite {
            base_address: 0x9000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: None,
        };
        let mapping = activated_writer_mapping(site.base_address, 8);
        let stale_mapping = activated_writer_mapping(0xa000, 8);
        let stale_receipt = prepared_destination_receipt(&stale_mapping, 171);
        let mut bytes = [0xa5; 8];
        let error =
            PreparedPostHandoffWriterDestination::claim(mapping, stale_receipt, site, &mut bytes)
                .expect_err("receipt from another activated mapping must reject");
        assert!(error.diagnostic().0.contains("exact activated mapping"));
        let (mapping, _receipt, returned_site, returned_bytes) = (*error).into_parts();
        assert_eq!(mapping.base(), site.base_address);
        assert_eq!(returned_site, site);
        assert_eq!(returned_bytes, &[0xa5; 8]);

        let receipt = prepared_destination_receipt(&mapping, 172);
        let mut destination =
            PreparedPostHandoffWriterDestination::claim(mapping, receipt, site, returned_bytes)
                .expect("returned mapping and bytes remain usable");
        destination
            .validate_for_writer_preparation()
            .expect("prepared destination replays exact mapping custody");
        destination.receipt.unpublished = false;
        let error = destination
            .into_validated_for_writer_preparation()
            .expect_err("preparation replay must reject publication-state drift");
        assert!(error.diagnostic().0.contains("unpublished destination"));
        let mut destination = (*error).into_destination();
        assert_eq!(destination.bytes, &[0xa5; 8]);
        destination.receipt.unpublished = true;
        let destination = destination
            .into_validated_for_writer_preparation()
            .expect("repaired destination remains available for exact retry");
        let installed = installed_code(&admit(&artifact(1)), 107, 0x8000);
        let target = RelocationTarget::Entry(entry_id(1001));
        let writer = PostHandoffWriterPlan {
            byte_len: 8,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "address".into(),
                    target,
                    container_byte_offset: 0,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 64,
                    stored_integer_fit: None,
                },
                source: PostHandoffWriterSource::Resolve(target),
            }],
        };
        let context = installed
            .populate_post_handoff_entry_writer_context(&writer, 8, site)
            .expect("writer context");
        let mut drifted_writer = writer.clone();
        drifted_writer.byte_order = ByteOrder::BigEndian;
        let error = installed
            .write_prepared_post_handoff_destination(context, &drifted_writer, destination)
            .expect_err("writer/context drift must reject before mutation");
        assert!(
            error
                .diagnostic()
                .0
                .contains("exact installed code, plan, destination")
        );
        let (context, destination) = (*error).into_parts();
        assert_eq!(context.installed_code(), installed.identity());
        assert_eq!(destination.site(), site);
        assert_eq!(destination.len(), 8);
        assert_eq!(destination.destination.bytes, &[0xa5; 8]);
        let written = installed
            .write_prepared_post_handoff_destination(context, &writer, destination)
            .expect("returned context and destination support corrected retry");
        let written = written
            .into_validated_for_consumer(&installed)
            .expect("corrected retry validates before byte observation");
        assert_eq!(
            u64::from_le_bytes(written.bytes().try_into().unwrap()),
            0x8010
        );
    }

    #[test]
    fn destination_receipt_must_name_an_established_nonempty_writer_right() {
        let site = PlacementSite {
            base_address: 0x9000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: None,
        };
        let mapping = activated_writer_mapping(site.base_address, 8);
        let receipt = DestinationPreparationReceipt::from_admitted_provider(
            id(
                173,
                DestinationPreparationReceiptId::from_normalized_identity,
            ),
            &mapping.receipt_context(),
            ExtentRights::none(),
            true,
            true,
        );
        let mut bytes = [0xa5; 8];
        let error = PreparedPostHandoffWriterDestination::claim(mapping, receipt, site, &mut bytes)
            .expect_err("an empty right requirement must not authorize writing");
        assert!(error.diagnostic().0.contains("no writer right"));
        let (_mapping, _receipt, _site, returned_bytes) = (*error).into_parts();
        assert_eq!(returned_bytes, &[0xa5; 8]);
    }

    #[test]
    fn provider_context_slots_follow_symbolic_targets_not_equal_addresses() {
        let first = entry_id(2001);
        let second = entry_id(2002);
        let candidate = Artifact::from_canonical_decode(
            id(1001, ArtifactId::from_normalized_identity),
            Architecture::X86_64,
            vec![0; 64],
            id(30, MachineContractSetId::from_normalized_identity),
            id(31, MachineFootprintId::from_normalized_identity),
            id(32, PlacementPlanId::from_normalized_identity),
            artifact_placement_constraints(),
            id(33, EntrySetId::from_normalized_identity),
            vec![
                ArtifactEntry::from_canonical_decode(first, 16),
                ArtifactEntry::from_canonical_decode(second, 16),
            ],
            id(34, RelocationSetId::from_normalized_identity),
            Vec::new(),
            authority_commitments(artifact_placement_constraints()),
        )
        .expect("two symbolic entries may select one code address");
        let admitted = admit(&candidate);
        let installed = installed_code(&admitted, 111, 0x8000);
        let first = RelocationTarget::Entry(first);
        let second = RelocationTarget::Entry(second);
        let step = |target, container_byte_offset| PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "address".into(),
                target,
                container_byte_offset,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolve(target),
        };
        let writer = PostHandoffWriterPlan {
            byte_len: 16,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![step(first, 0), step(second, 8)],
        };
        let destination_site = PlacementSite {
            base_address: 0x9000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: None,
        };

        let context = installed
            .populate_post_handoff_entry_writer_context(&writer, 16, destination_site)
            .expect("equal addresses retain two target-indexed slots");
        assert_eq!(context.source_slot_count(), 2);
        assert_eq!(context.packed_byte_len(), 24);
        assert!(
            context.binds_invocation(
                &writer
                    .lower_reusable_fragment()
                    .expect("target-indexed fragment invocation")
            )
        );
        let mut destination = [0; 16];
        installed
            .execute_populated_post_handoff_entry_writer(
                &context,
                &writer,
                &mut destination,
                destination_site,
            )
            .expect("both symbolic slots execute");
        assert_eq!(
            u64::from_le_bytes(destination[0..8].try_into().unwrap()),
            0x8010
        );
        assert_eq!(
            u64::from_le_bytes(destination[8..16].try_into().unwrap()),
            0x8010
        );
    }

    #[test]
    fn final_certificate_is_bound_to_one_placement_and_final_bytes() {
        let admitted = admit(&artifact(1));
        let target = frozen(&admitted, 102, 0x3000);
        let foreign = frozen(&admitted, 103, 0x3000);
        let certificate = certificate(&foreign, 183);
        let error = validate_final_placement(target, &certificate)
            .expect_err("certificate transplant rejects");
        assert!(error.diagnostic().0.contains("does not match"));
    }

    #[test]
    fn final_certificate_retains_the_exact_frozen_byte_snapshot() {
        let admitted = admit(&artifact(1));
        let frozen = frozen(&admitted, 113, 0xb000);
        let mut certificate = certificate(&frozen, 193);
        certificate.final_bytes[0] ^= 1;

        let error = validate_final_placement(frozen, &certificate)
            .expect_err("a certificate for substituted exact bytes must reject");
        assert!(error.diagnostic().0.contains("does not match"));
    }

    #[test]
    fn unsupported_execute_transition_preserves_all_linear_inputs() {
        let admitted = admit(&artifact(1));
        let frozen = frozen(&admitted, 104, 0x4000);
        let certificate = certificate(&frozen, 184);
        let validated =
            validate_final_placement(frozen, &certificate).expect("validated placement");
        let authority = InstallAuthority::from_admitted_provider(&validated);
        let receipt = InstallationReceipt::from_provider(
            id(204, InstalledCodeId::from_normalized_identity),
            &validated,
            true,
            WxEnforcement::Unsupported,
        );
        let error = install_validated(validated, authority, receipt)
            .expect_err("unsupported provider rejects");
        assert!(error.diagnostic().0.contains("does not support"));
        let (_validated, _authority, _receipt) = (*error).into_parts();
    }

    #[test]
    fn materialization_uses_admitted_artifact_size_not_a_caller_hint() {
        let admitted = admit(&artifact(1));
        let placement = placement_authority(105, 0x5000, 32)
            .claim(placement_extent(105, 0x5000, 32))
            .expect("qualified but undersized destination");
        let error = materialize_admitted_artifact(&admitted, &placement, |_| None)
            .expect_err("artifact cannot fit");
        assert!(error.0.contains("smaller"));
    }

    #[test]
    fn materialization_rejects_placement_constraint_substitution() {
        let admitted = admit(&artifact(1));
        let substituted =
            PlacementConstraints::new(None, 1, PlacementPhase::PostHandoff, None, None)
                .expect("weaker substituted constraints");
        let placement = placement_authority_with_constraints(109, 0x8000, 4096, substituted)
            .claim(placement_extent(109, 0x8000, 4096))
            .expect("substituted constraints independently accept the site");
        let error = materialize_admitted_artifact(&admitted, &placement, |_| None)
            .expect_err("provider cannot substitute weaker placement constraints");
        assert!(error.0.contains("constraints do not match"));
    }

    #[test]
    fn failed_placement_claim_returns_extent_and_one_shot_authority() {
        let extent = ExtentRootGrant::from_admitted_provider(
            extent_provider_issuance(106),
            extent_id(106, ExtentLineageId::from_normalized_identity),
            extent_id(50, AddressSpaceId::from_normalized_identity),
            ExtentRights::none(),
            extent_id(52, ExtentProvenanceId::from_normalized_identity),
            extent_id(53, MappingEraId::from_normalized_identity),
        )
        .mint(0x6000, 4096)
        .expect("placement extent without required rights");
        let error = placement_authority(106, 0x6000, 4096)
            .claim(extent)
            .expect_err("missing placement right");
        assert!(error.diagnostic().0.contains("exact range"));
        let (_authority, extent) = (*error).into_parts();
        assert_eq!(extent.base(), 0x6000);
    }

    #[test]
    fn placement_authority_rejects_same_address_from_another_lineage() {
        let admitted_extent = placement_extent(116, 0xe000, 4096);
        let substituted_extent = placement_extent(117, 0xe000, 4096);
        let authority = CodePlacementAuthority::from_admitted_provider(
            id(116, CodePlacementId::from_normalized_identity),
            id(61, InstallationScopeId::from_normalized_identity),
            InstallationAudience::FutureFetcher,
            &admitted_extent,
            rights(&[51]),
            artifact_placement_constraints(),
            PlacementSite {
                base_address: 0xe000,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: Some(
                    ArtifactInstallationScopeId::from_normalized_identity(61)
                        .expect("installation scope"),
                ),
            },
        );

        let error = authority
            .claim(substituted_extent)
            .expect_err("same address and rights do not imply the same range authority");
        assert!(error.diagnostic().0.contains("lineage"));
    }

    #[test]
    fn retirement_requires_quiescence_then_returns_writable_placement() {
        let admitted = admit(&artifact(1));
        let installed = installed_code(&admitted, 107, 0x7000);
        let retirement_fact =
            RetirementFactDigest::from_canonical_bytes(b"provider.timer-drain.complete.v1");
        let authority = RetirementAuthority::from_admitted_provider(&installed, [retirement_fact]);
        let receipt =
            RetirementReceipt::from_provider(&installed, false, true, true, [retirement_fact]);
        let error = retire_installed(installed, authority, receipt)
            .expect_err("visibility is not quiescence");
        assert!(error.diagnostic().0.contains("quiescence"));
        let (installed, authority, _) = (*error).into_parts();

        let receipt =
            RetirementReceipt::from_provider(&installed, true, true, true, [retirement_fact]);
        let retired = retire_installed(installed, authority, receipt).expect("retired code");
        assert_eq!(
            retired.previous_artifact().artifact().identity(),
            admitted.artifact().identity()
        );

        let replacement = admit(&artifact(2));
        let placement = retired.into_placement();
        let materialized = materialize_admitted_artifact(&replacement, &placement, |_| None)
            .expect("replacement materialization");
        materialize_and_freeze(
            &replacement,
            placement,
            materialized.clone(),
            MaterializationReceipt::from_materialized(
                &materialized,
                id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("placement reusable only after quiescent retirement");
    }

    #[test]
    fn incomplete_drain_quarantines_capacity_without_returning_placement() {
        let admitted = admit(&artifact(1));
        let installed = installed_code(&admitted, 117, 0xe000);
        let installed_identity = installed.identity();
        let installed_context = installed.receipt_context();
        let quarantine = id(401, MappingQuarantineId::from_normalized_identity);
        let receipt = MappingQuarantineReceipt::from_provider(
            &installed,
            quarantine,
            true,
            true,
            true,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 2,
            },
        );

        let quarantined = quarantine_installed(installed, receipt).expect("fail-closed quarantine");
        assert_eq!(quarantined.installed_code(), installed_identity);
        assert_eq!(quarantined.attributed_capacity_loss(), 4096);
        assert!(matches!(
            quarantined.cause(),
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 2
            }
        ));
        let fault = quarantined
            .stale_entry_fault(&installed_context)
            .expect("stale entry names quarantined realization");
        assert_eq!(fault.quarantine(), quarantine);
        assert!(!fault.discharged_obligations());
    }

    #[test]
    fn quarantine_requires_execute_removal_unmapping_and_reservation() {
        let admitted = admit(&artifact(1));
        let installed = installed_code(&admitted, 118, 0xf000);
        let quarantine = id(402, MappingQuarantineId::from_normalized_identity);
        let incomplete = MappingQuarantineReceipt::from_provider(
            &installed,
            quarantine,
            true,
            false,
            true,
            MappingQuarantineCause::PossibleOpaqueHolder {
                provider_identity: "OpaqueCodec".into(),
            },
        );
        let error = quarantine_installed(installed, incomplete)
            .expect_err("still-mapped range cannot become quarantine");
        assert!(error.diagnostic().0.contains("unmapped/trapping"));
        let (installed, _) = (*error).into_parts();

        let complete = MappingQuarantineReceipt::from_provider(
            &installed,
            quarantine,
            true,
            true,
            true,
            MappingQuarantineCause::PossibleOpaqueHolder {
                provider_identity: "OpaqueCodec".into(),
            },
        );
        let quarantined =
            quarantine_installed(installed, complete).expect("opaque holder stays quarantined");
        assert!(matches!(
            quarantined.cause(),
            MappingQuarantineCause::PossibleOpaqueHolder { provider_identity }
                if provider_identity == "OpaqueCodec"
        ));
    }

    #[test]
    fn quarantine_fault_rejects_an_unrelated_installed_identity() {
        let admitted = admit(&artifact(1));
        let installed = installed_code(&admitted, 119, 0xc000);
        let receipt = MappingQuarantineReceipt::from_provider(
            &installed,
            id(403, MappingQuarantineId::from_normalized_identity),
            true,
            true,
            true,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 1,
            },
        );
        let quarantined = quarantine_installed(installed, receipt).expect("quarantined");
        let unrelated = installed_code(&admit(&artifact(2)), 999, 0xd000).receipt_context();
        assert!(
            quarantined
                .stale_entry_fault(&unrelated)
                .expect_err("unrelated identity is not this stale entry")
                .0
                .contains("does not name")
        );
    }

    #[test]
    fn quarantine_fault_rejects_a_collision_equal_report_identity() {
        let first = admit(&colliding_artifact(1, 0x90));
        let second = admit(&colliding_artifact(1, 0xcc));
        let first_installed = installed_code(&first, 120, 0xd000);
        let second_installed = installed_code(&second, 120, 0xd000);
        let exact_context = first_installed.receipt_context();
        let colliding_context = second_installed.receipt_context();
        assert_eq!(
            first_installed.identity(),
            second_installed.identity(),
            "the adversary controls a collision-equal compact report identity"
        );

        let receipt = MappingQuarantineReceipt::from_provider(
            &first_installed,
            id(404, MappingQuarantineId::from_normalized_identity),
            true,
            true,
            true,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 1,
            },
        );
        let quarantined = quarantine_installed(first_installed, receipt).expect("exact quarantine");

        quarantined
            .stale_entry_fault(&exact_context)
            .expect("the exact quarantined realization faults");
        let error = quarantined
            .stale_entry_fault(&colliding_context)
            .expect_err("a compact-ID collision must not forge stale-entry evidence");
        assert!(error.0.contains("does not name"));
    }

    #[test]
    fn quarantine_receipt_rejects_a_collision_equal_installed_realization() {
        let first = admit(&colliding_artifact(1, 0x90));
        let second = admit(&colliding_artifact(1, 0xcc));
        let first_installed = installed_code(&first, 122, 0xd000);
        let second_installed = installed_code(&second, 122, 0xd000);
        assert_eq!(first_installed.identity(), second_installed.identity());

        let substituted_receipt = MappingQuarantineReceipt::from_provider(
            &second_installed,
            id(405, MappingQuarantineId::from_normalized_identity),
            true,
            true,
            true,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 1,
            },
        );
        let error = quarantine_installed(first_installed, substituted_receipt)
            .expect_err("quarantine must bind the complete installed realization");
        assert!(error.diagnostic().0.contains("does not match"));
        let (first_installed, _) = (*error).into_parts();
        assert!(first_installed.binds_exact_unrelocated_artifact_bytes(&[0x90; 64]));
    }

    #[test]
    fn retirement_completion_facts_are_strong_domain_separated_commitments() {
        let fact = RetirementFactDigest::from_canonical_bytes(b"provider.timer-drain.complete.v1");
        let changed =
            RetirementFactDigest::from_canonical_bytes(b"provider.timer-drain.complete.v2");
        let proof = normalized_proof_payload_digest(b"provider.timer-drain.complete.v1");

        assert_ne!(
            fact, changed,
            "fact-byte mutation must change the commitment"
        );
        assert_ne!(
            fact.digest(),
            proof.digest(),
            "equal payload bytes in another authority domain must not collide by construction"
        );
    }

    #[test]
    fn retirement_rejects_a_different_exact_completion_fact() {
        let admitted = admit(&artifact(1));
        let installed = installed_code(&admitted, 121, 0xd000);
        let required = RetirementFactDigest::from_canonical_bytes(b"provider.drain.complete.v1");
        let substituted =
            RetirementFactDigest::from_canonical_bytes(b"provider.cache-flush.complete.v1");
        let authority = RetirementAuthority::from_admitted_provider(&installed, [required]);
        let receipt = RetirementReceipt::from_provider(&installed, true, true, true, [substituted]);

        let error = retire_installed(installed, authority, receipt)
            .expect_err("another provider fact cannot discharge retirement");
        assert!(error.diagnostic().0.contains("completion facts"));
    }

    #[test]
    fn placement_claim_validates_actual_extent_site_against_plan_constraints() {
        let error = placement_authority(108, 0x7101, 4096)
            .claim(placement_extent(108, 0x7101, 4096))
            .expect_err("misaligned site rejects before materialization");
        assert!(error.diagnostic().0.contains("not aligned"));
        let (_authority, extent) = (*error).into_parts();
        assert_eq!(extent.base(), 0x7101);
    }

    #[test]
    fn retirement_receipt_cannot_substitute_colliding_installed_realization() {
        let first = admit(&colliding_artifact(1, 0x90));
        let second = admit(&colliding_artifact(1, 0xcc));
        let first_installed = installed_code(&first, 115, 0xd000);
        let second_installed = installed_code(&second, 115, 0xd000);

        let authority =
            RetirementAuthority::from_admitted_provider(&first_installed, std::iter::empty());
        let substituted_receipt = RetirementReceipt::from_provider(
            &second_installed,
            true,
            true,
            true,
            std::iter::empty(),
        );
        let error = retire_installed(first_installed, authority, substituted_receipt)
            .expect_err("retirement must bind the exact installed realization");
        assert!(error.diagnostic().0.contains("receipt"));
    }
}
