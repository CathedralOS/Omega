//! Normalized executable-artifact admission and installation ladder.
//!
//! No operation converts arbitrary bytes into executable memory. Immutable
//! artifacts are admitted once and reused; each installation instead consumes
//! one exact destination authority through frozen and validated states.
//!
//! The transitions below follow lifecycle order. Callers supply the provider
//! evidence between transitions; the state carriers and receipt checks follow.
//! `post_handoff_writer` owns the separate installed-entry writer protocol.

use std::sync::Arc;

use extents::{AddressSpaceId, Extent, ExtentProvenanceId, ExtentRights};
use installation_evidence::InstalledArtifactOccurrenceDigest;
use layout_plans::{EntryStubId, PlacementConstraints, PlacementSite, RelocationTarget};
use sha2::{Digest, Sha256};
use target::Architecture;

mod container;
mod container_bytes;
mod materializer;
mod post_handoff_writer;
mod replacement_quarantine;

pub use container::*;
pub use container_bytes::*;
pub use materializer::*;
pub use post_handoff_writer::*;
pub use replacement_quarantine::*;

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
        regime: Option<(layout_plans::MachineRegimeId, &[u8])>,
        scope: Option<(layout_plans::ArtifactInstallationScopeId, &[u8])>,
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
    era: extents::MappingEraId,
    lineage: extents::ExtentLineageId,
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
    era: extents::MappingEraId,
    lineage: extents::ExtentLineageId,
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

    /// Test the exact materialized byte interval beginning at one admitted
    /// entry. The installed image remains provider-side: callers can bind a
    /// generated stub to its frozen executable bytes without receiving either
    /// the image or an executable address.
    pub fn binds_exact_materialized_entry_bytes(
        &self,
        entry: EntryStubId,
        expected: &[u8],
    ) -> bool {
        if expected.is_empty() {
            return false;
        }
        let frozen = &self.validated.frozen;
        let Some(candidate) = frozen.artifact.artifact.entry(entry) else {
            return false;
        };
        let Ok(start) = usize::try_from(candidate.code_offset) else {
            return false;
        };
        let Some(end) = start.checked_add(expected.len()) else {
            return false;
        };
        frozen.materialized.bytes().get(start..end) == Some(expected)
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
        layout_plans::PlacementPhase::Build => 1,
        layout_plans::PlacementPhase::Load => 2,
        layout_plans::PlacementPhase::PostHandoff => 3,
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
mod tests;

#[cfg(test)]
mod test_support;
