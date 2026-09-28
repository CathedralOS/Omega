use sha2::Digest;

use crate::artifacts::RetainedContainerProof;
use crate::artifacts::{AdmittedArtifact, Artifact, ArtifactAdmissionEvidence, ArtifactEntry};
use crate::authority_digests::ArtifactAuthorityCommitments;
use crate::authority_digests::{
    AdmissionReceiptId, ArtifactContentDigest, ArtifactId, EntrySetId, MachineContractSetId,
    MachineFootprintId, NonAuthoritativeContainerFingerprint64,
    NonAuthoritativeInformationalFingerprint64, PlacementPlanId, ProofPayloadDigest,
    RelocationSetId,
};
use crate::executable_installation::admit_executable;
use crate::installation::InstallationDiagnostic;
use layout_plans::{PlacementConstraints, RelocationTarget};
use sha2::Sha256;
use target::Architecture;
pub const OMEGA_EXECUTABLE_CONTAINER_V1_MARKER: u16 = u16::from_le_bytes(*b"OX");
pub const OMEGA_EXECUTABLE_CONTAINER_V2_MARKER: u16 = u16::from_le_bytes(*b"O2");
pub const OMEGA_EXECUTABLE_CONTAINER_MARKER: u16 = OMEGA_EXECUTABLE_CONTAINER_V2_MARKER;
const CANONICAL_ENTRY_RECORD_BYTES: u64 = 16;
const CANONICAL_RELOCATION_COUNT_BYTES: u64 = 8;
const CANONICAL_RELOCATION_RECORD_BYTES: u64 = 32;

pub fn normalized_proof_payload_digest(proof: &[u8]) -> ProofPayloadDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.proof-payload.sha256.v1\0");
    digest.update((proof.len() as u64).to_le_bytes());
    digest.update(proof);
    ProofPayloadDigest::from_digest(digest.finalize().into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerLimits {
    pub max_total_bytes: u64,
    pub max_sections: usize,
    pub max_section_bytes: u64,
    pub max_relocations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerSection {
    pub kind: ContainerSectionKind,
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerSectionKind {
    Code,
    Relocations(RelocationSetId),
    Contracts(MachineContractSetId),
    Footprint(MachineFootprintId),
    Placement(PlacementPlanId),
    Entries(EntrySetId),
    Proof(ProofPayloadDigest),
    AuthorityCommitments(ArtifactAuthorityCommitments),
    Informational(NonAuthoritativeInformationalFingerprint64),
    /// An unrecognized optional section is informational by definition. It
    /// cannot supply an identity used by admission.
    Unknown {
        identity: u64,
        required: bool,
    },
}

/// Closed semantic relocation vocabulary accepted from the checked artifact
/// schema. These are normalized relocation meanings, not object-format
/// record numbers; the target writer translates them to its native format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtifactRelocationKind {
    Absolute64,
    X86Relative32,
    Aarch64Page21,
    Aarch64PageOffset12,
    Aarch64Branch26,
}

impl ArtifactRelocationKind {
    const fn byte_width(self) -> u64 {
        match self {
            Self::Absolute64 => 8,
            Self::X86Relative32
            | Self::Aarch64Page21
            | Self::Aarch64PageOffset12
            | Self::Aarch64Branch26 => 4,
        }
    }

    const fn supports(self, architecture: Architecture) -> bool {
        match self {
            Self::Absolute64 => true,
            Self::X86Relative32 => matches!(architecture, Architecture::X86_64),
            Self::Aarch64Page21 | Self::Aarch64PageOffset12 | Self::Aarch64Branch26 => {
                matches!(architecture, Architecture::Aarch64)
            }
        }
    }
}

/// One checked-schema relocation record before semantic validation. The
/// target remains symbolic: decoding an artifact never exposes a numeric code
/// or data address to source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedArtifactRelocation {
    pub kind: ArtifactRelocationKind,
    pub destination_offset: u64,
    pub target: RelocationTarget,
    pub addend: i64,
}

/// Output of checked schema/layout decoding, before semantic container
/// validation. This type deliberately has no scripts, constructors, imports,
/// or recursive section form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedArtifactContainer {
    pub format_marker: u16,
    pub total_length: u64,
    pub artifact: ArtifactId,
    pub content_fingerprint: NonAuthoritativeContainerFingerprint64,
    pub architecture: Architecture,
    pub code_length: u64,
    /// Exact decoded executable bytes. The post-decode validator binds these
    /// bytes into normalized content identity before any admission fact exists.
    pub code: Vec<u8>,
    /// Compact imported-contract report coordinate. Version-2 authority is
    /// the matching digest in `authority_commitments`.
    pub contracts: MachineContractSetId,
    /// Compact declared-footprint report coordinate. Version-2 authority is
    /// the matching digest in `authority_commitments`.
    pub declared_footprint: MachineFootprintId,
    pub placement_plan: PlacementPlanId,
    /// Checked decode of the canonical placement section. Compact regime and
    /// installation-scope values here are report coordinates; version-2
    /// authority is the matching digest in `authority_commitments`.
    pub placement_constraints: PlacementConstraints,
    /// Checked decode of the compiler-selected entry set. Entry identities are
    /// sealed materialization symbols; offsets are interpreted only by the
    /// installation/provider layer.
    pub entry_set: EntrySetId,
    pub entries: Vec<ArtifactEntry>,
    pub relocation_set: RelocationSetId,
    pub relocations: Vec<DecodedArtifactRelocation>,
    pub proof_payload: ProofPayloadDigest,
    /// Exact identity-invisible proof bytes. Admission binds these bytes even
    /// though executable-content identity deliberately excludes them.
    pub proof: Vec<u8>,
    /// Present only for container v2. Version 1 remains decodable for tooling
    /// compatibility but cannot produce admission evidence.
    pub authority_commitments: Option<ArtifactAuthorityCommitments>,
    pub sections: Vec<ContainerSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedArtifactContainer {
    artifact: Artifact,
    proof_payload: ProofPayloadDigest,
    proof: Vec<u8>,
    informational_sections: Vec<NonAuthoritativeInformationalFingerprint64>,
    unknown_informational_sections: Vec<u64>,
}

impl ValidatedArtifactContainer {
    pub const fn artifact(&self) -> &Artifact {
        &self.artifact
    }

    pub fn relocation_set(&self) -> RelocationSetId {
        self.artifact.relocation_set()
    }

    /// Canonical destination-order relocation records. Their symbolic targets
    /// remain sealed identities; this projection grants no resolver.
    pub fn relocations(&self) -> &[DecodedArtifactRelocation] {
        self.artifact.relocations()
    }

    pub const fn proof_payload(&self) -> ProofPayloadDigest {
        self.proof_payload
    }

    pub fn proof(&self) -> &[u8] {
        &self.proof
    }

    pub fn informational_sections(&self) -> &[NonAuthoritativeInformationalFingerprint64] {
        &self.informational_sections
    }

    pub fn unknown_informational_sections(&self) -> &[u64] {
        &self.unknown_informational_sections
    }
}

/// Validator-authored admission evidence bound to one exact checked container,
/// including its identity-invisible proof payload. Informational sections are
/// deliberately absent because they carry no admission authority.
#[derive(Debug, PartialEq, Eq)]
pub struct ValidatedContainerAdmissionEvidence {
    receipt: AdmissionReceiptId,
    artifact: Artifact,
    proof_payload: ProofPayloadDigest,
    proof: Vec<u8>,
    accepted: bool,
}

impl ValidatedContainerAdmissionEvidence {
    pub fn from_validator(
        receipt: AdmissionReceiptId,
        container: &ValidatedArtifactContainer,
        accepted: bool,
    ) -> Self {
        Self {
            receipt,
            artifact: container.artifact.clone(),
            proof_payload: container.proof_payload,
            proof: container.proof.clone(),
            accepted,
        }
    }
}

/// Establishes the reusable executable qualification only from acceptance
/// evidence for this exact checked container and proof payload. The verifier
/// remains provider/PCC infrastructure; this gate prevents substitution or
/// replay across its normalized input.
pub fn admit_validated_container(
    container: &ValidatedArtifactContainer,
    evidence: ValidatedContainerAdmissionEvidence,
) -> Result<AdmittedArtifact, InstallationDiagnostic> {
    if evidence.proof_payload != container.proof_payload {
        return Err(InstallationDiagnostic(
            "artifact admission evidence names a different proof payload".into(),
        ));
    }
    if evidence.proof != container.proof {
        return Err(InstallationDiagnostic(
            "artifact admission evidence names different exact proof bytes".into(),
        ));
    }
    if evidence.artifact != container.artifact {
        return Err(InstallationDiagnostic(
            "artifact admission evidence names a different validated container".into(),
        ));
    }
    let mut admitted = admit_executable(
        &container.artifact,
        ArtifactAdmissionEvidence::from_validator(
            evidence.receipt,
            &evidence.artifact,
            evidence.accepted,
        ),
    )?;
    admitted.container_proof = Some(RetainedContainerProof {
        digest: container.proof_payload,
        bytes: container.proof.clone(),
    });
    Ok(admitted)
}

pub fn validate_decoded_container(
    decoded: DecodedArtifactContainer,
    limits: ContainerLimits,
) -> Result<ValidatedArtifactContainer, InstallationDiagnostic> {
    if limits.max_total_bytes == 0
        || limits.max_sections == 0
        || limits.max_section_bytes == 0
        || limits.max_relocations == 0
    {
        return Err(InstallationDiagnostic(
            "artifact-container limits must all be nonzero".into(),
        ));
    }
    if !matches!(
        decoded.format_marker,
        OMEGA_EXECUTABLE_CONTAINER_V1_MARKER | OMEGA_EXECUTABLE_CONTAINER_V2_MARKER
    ) {
        return Err(InstallationDiagnostic(format!(
            "unsupported Omega executable container marker 0x{:04x}",
            decoded.format_marker
        )));
    }
    if decoded.total_length == 0 || decoded.total_length > limits.max_total_bytes {
        return Err(InstallationDiagnostic(format!(
            "artifact container length {} exceeds configured bound {}",
            decoded.total_length, limits.max_total_bytes
        )));
    }
    if decoded.sections.len() > limits.max_sections {
        return Err(InstallationDiagnostic(format!(
            "artifact container has {} sections, exceeding configured bound {}",
            decoded.sections.len(),
            limits.max_sections
        )));
    }
    let decoded_code_length = u64::try_from(decoded.code.len()).map_err(|_| {
        InstallationDiagnostic(
            "decoded artifact code length cannot be represented by the container".into(),
        )
    })?;
    if decoded_code_length != decoded.code_length {
        return Err(InstallationDiagnostic(format!(
            "decoded artifact has {} code byte(s), canonical header declares {}",
            decoded.code.len(),
            decoded.code_length
        )));
    }
    if decoded.proof_payload != normalized_proof_payload_digest(&decoded.proof) {
        return Err(InstallationDiagnostic(
            "proof-payload identity does not match the exact proof bytes".into(),
        ));
    }

    let mut ranges = Vec::with_capacity(decoded.sections.len());
    let mut code = 0;
    let mut relocations = 0;
    let mut contracts = 0;
    let mut footprint = 0;
    let mut placement = 0;
    let mut entries = 0;
    let mut proof = 0;
    let mut authority_commitments = 0;
    let mut informational = Vec::new();
    let mut unknown_informational = Vec::new();
    for section in &decoded.sections {
        if section.length == 0 || section.length > limits.max_section_bytes {
            return Err(InstallationDiagnostic(format!(
                "artifact section length {} is empty or exceeds configured bound {}",
                section.length, limits.max_section_bytes
            )));
        }
        let end = section
            .offset
            .checked_add(section.length)
            .ok_or_else(|| InstallationDiagnostic("artifact section range overflows".into()))?;
        if end > decoded.total_length {
            return Err(InstallationDiagnostic(format!(
                "artifact section {}..{} exceeds {}-byte container",
                section.offset, end, decoded.total_length
            )));
        }
        ranges.push((section.offset, end));

        match section.kind {
            ContainerSectionKind::Code => {
                code += 1;
                if section.length != decoded.code_length {
                    return Err(InstallationDiagnostic(
                        "code section length does not match canonical header".into(),
                    ));
                }
            }
            ContainerSectionKind::Relocations(identity) => {
                relocations += 1;
                require_identity("relocation", identity, decoded.relocation_set)?;
                let relocation_count = u64::try_from(decoded.relocations.len()).map_err(|_| {
                    InstallationDiagnostic(
                        "artifact relocation count cannot be represented by the container".into(),
                    )
                })?;
                let expected_length = relocation_count
                    .checked_mul(CANONICAL_RELOCATION_RECORD_BYTES)
                    .and_then(|records| records.checked_add(CANONICAL_RELOCATION_COUNT_BYTES))
                    .ok_or_else(|| {
                        InstallationDiagnostic("artifact relocation-set length overflows".into())
                    })?;
                if section.length != expected_length {
                    return Err(InstallationDiagnostic(format!(
                        "relocation section length {} does not match {} canonical relocations",
                        section.length,
                        decoded.relocations.len()
                    )));
                }
            }
            ContainerSectionKind::Contracts(identity) => {
                contracts += 1;
                require_identity("contract", identity, decoded.contracts)?;
            }
            ContainerSectionKind::Footprint(identity) => {
                footprint += 1;
                require_identity("footprint", identity, decoded.declared_footprint)?;
            }
            ContainerSectionKind::Placement(identity) => {
                placement += 1;
                require_identity("placement", identity, decoded.placement_plan)?;
            }
            ContainerSectionKind::Entries(identity) => {
                entries += 1;
                require_identity("entry-set", identity, decoded.entry_set)?;
                let entry_count = u64::try_from(decoded.entries.len()).map_err(|_| {
                    InstallationDiagnostic(
                        "artifact entry count cannot be represented by the container".into(),
                    )
                })?;
                let expected_length = entry_count
                    .checked_mul(CANONICAL_ENTRY_RECORD_BYTES)
                    .ok_or_else(|| {
                        InstallationDiagnostic("artifact entry-set length overflows".into())
                    })?;
                if section.length != expected_length {
                    return Err(InstallationDiagnostic(format!(
                        "entry-set section length {} does not match {} canonical entries",
                        section.length,
                        decoded.entries.len()
                    )));
                }
            }
            ContainerSectionKind::Proof(identity) => {
                proof += 1;
                require_identity("proof", identity, decoded.proof_payload)?;
                if section.length != decoded.proof.len() as u64 {
                    return Err(InstallationDiagnostic(
                        "proof section length does not match exact decoded proof bytes".into(),
                    ));
                }
            }
            ContainerSectionKind::AuthorityCommitments(commitments) => {
                authority_commitments += 1;
                if Some(commitments) != decoded.authority_commitments {
                    return Err(InstallationDiagnostic(
                        "authority-commitment section does not match decoded strong evidence"
                            .into(),
                    ));
                }
                if section.length != 128 {
                    return Err(InstallationDiagnostic(
                        "authority-commitment section must contain four 32-byte digests".into(),
                    ));
                }
            }
            ContainerSectionKind::Informational(identity) => informational.push(identity),
            ContainerSectionKind::Unknown { identity, required } => {
                if required {
                    return Err(InstallationDiagnostic(format!(
                        "unknown required artifact section {identity}"
                    )));
                }
                unknown_informational.push(identity);
            }
        }
    }

    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(InstallationDiagnostic(
                "artifact container sections overlap".into(),
            ));
        }
    }
    for (name, count) in [
        ("code", code),
        ("relocations", relocations),
        ("contracts", contracts),
        ("footprint", footprint),
        ("placement", placement),
        ("entry-set", entries),
        ("proof", proof),
    ] {
        if count != 1 {
            return Err(InstallationDiagnostic(format!(
                "artifact container requires exactly one {name} section, found {count}"
            )));
        }
    }
    match decoded.format_marker {
        OMEGA_EXECUTABLE_CONTAINER_V1_MARKER => {
            if decoded.authority_commitments.is_some() || authority_commitments != 0 {
                return Err(InstallationDiagnostic(
                    "container-v1 cannot carry v2 authority commitments".into(),
                ));
            }
        }
        OMEGA_EXECUTABLE_CONTAINER_V2_MARKER => {
            if decoded.authority_commitments.is_none() || authority_commitments != 1 {
                return Err(InstallationDiagnostic(format!(
                    "container-v2 requires exactly one authority-commitment section, found {authority_commitments}"
                )));
            }
        }
        _ => unreachable!("format marker checked above"),
    }

    let relocations = validate_decoded_relocations(
        decoded.relocations,
        decoded.architecture,
        decoded.code_length,
        limits.max_relocations,
    )?;
    let (_, computed_fingerprint) = derive_artifact_content_commitments(
        decoded.architecture,
        &decoded.code,
        decoded.contracts,
        decoded.declared_footprint,
        decoded.placement_plan,
        decoded.placement_constraints,
        decoded.entry_set,
        &decoded.entries,
        decoded.relocation_set,
        &relocations,
        decoded.authority_commitments.as_ref(),
    )?;
    if computed_fingerprint != decoded.content_fingerprint {
        return Err(InstallationDiagnostic(format!(
            "container-v1 content fingerprint {} does not match normalized compatibility fingerprint {}",
            decoded.content_fingerprint.compatibility_value(),
            computed_fingerprint.compatibility_value()
        )));
    }
    let artifact = match decoded.authority_commitments {
        Some(commitments) => Artifact::from_canonical_decode(
            decoded.artifact,
            decoded.architecture,
            decoded.code,
            decoded.contracts,
            decoded.declared_footprint,
            decoded.placement_plan,
            decoded.placement_constraints,
            decoded.entry_set,
            decoded.entries,
            decoded.relocation_set,
            relocations,
            commitments,
        )?,
        None => Artifact::from_legacy_v1_decode(
            decoded.artifact,
            decoded.architecture,
            decoded.code,
            decoded.contracts,
            decoded.declared_footprint,
            decoded.placement_plan,
            decoded.placement_constraints,
            decoded.entry_set,
            decoded.entries,
            decoded.relocation_set,
            relocations,
        )?,
    };
    Ok(ValidatedArtifactContainer {
        artifact,
        proof_payload: decoded.proof_payload,
        proof: decoded.proof,
        informational_sections: informational,
        unknown_informational_sections: unknown_informational,
    })
}

/// Derive the normalizer-owned executable-content identity for one checked
/// schema decode. Container byte order, section order, proof payloads, and
/// informational sections do not participate; executable bytes and every
/// published semantic commitment do.
pub fn normalized_decoded_content_digest(
    decoded: &DecodedArtifactContainer,
) -> Result<ArtifactContentDigest, InstallationDiagnostic> {
    let relocations = validate_decoded_relocations(
        decoded.relocations.clone(),
        decoded.architecture,
        decoded.code_length,
        decoded.relocations.len().max(1),
    )?;
    derive_artifact_content_commitments(
        decoded.architecture,
        &decoded.code,
        decoded.contracts,
        decoded.declared_footprint,
        decoded.placement_plan,
        decoded.placement_constraints,
        decoded.entry_set,
        &decoded.entries,
        decoded.relocation_set,
        &relocations,
        decoded.authority_commitments.as_ref(),
    )
    .map(|(digest, _)| digest)
}

/// Legacy container-v1 checksum of the normalized semantic payload. This is
/// retained for wire compatibility only and never substitutes for the strong
/// content digest or exact artifact replay.
pub fn non_authoritative_decoded_container_fingerprint(
    decoded: &DecodedArtifactContainer,
) -> Result<NonAuthoritativeContainerFingerprint64, InstallationDiagnostic> {
    let relocations = validate_decoded_relocations(
        decoded.relocations.clone(),
        decoded.architecture,
        decoded.code_length,
        decoded.relocations.len().max(1),
    )?;
    derive_artifact_content_commitments(
        decoded.architecture,
        &decoded.code,
        decoded.contracts,
        decoded.declared_footprint,
        decoded.placement_plan,
        decoded.placement_constraints,
        decoded.entry_set,
        &decoded.entries,
        decoded.relocation_set,
        &relocations,
        decoded.authority_commitments.as_ref(),
    )
    .map(|(_, fingerprint)| fingerprint)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_artifact_content_commitments(
    architecture: Architecture,
    code: &[u8],
    contracts: MachineContractSetId,
    footprint: MachineFootprintId,
    placement_plan: PlacementPlanId,
    placement: PlacementConstraints,
    entry_set: EntrySetId,
    entries: &[ArtifactEntry],
    relocation_set: RelocationSetId,
    relocations: &[DecodedArtifactRelocation],
    authority_commitments: Option<&ArtifactAuthorityCommitments>,
) -> Result<
    (
        ArtifactContentDigest,
        NonAuthoritativeContainerFingerprint64,
    ),
    InstallationDiagnostic,
> {
    let mut digest = Sha256::new();
    digest.update(if authority_commitments.is_some() {
        b"omega.executable-content.sha256.v2\0".as_slice()
    } else {
        b"omega.executable-content.sha256.v1\0".as_slice()
    });
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_bytes(&mut fingerprint, b"omega-executable-content-v1");
    content_commitment_bytes(
        &mut digest,
        &mut fingerprint,
        &[match architecture {
            Architecture::Aarch64 => 1,
            Architecture::X86_64 => 2,
        }],
    );
    content_commitment_bytes(
        &mut digest,
        &mut fingerprint,
        &(code.len() as u64).to_le_bytes(),
    );
    content_commitment_bytes(&mut digest, &mut fingerprint, code);
    for identity in [
        contracts.normalized_identity(),
        footprint.normalized_identity(),
        placement_plan.normalized_identity(),
        entry_set.normalized_identity(),
        relocation_set.normalized_identity(),
    ] {
        content_commitment_bytes(&mut digest, &mut fingerprint, &identity.to_le_bytes());
    }
    fingerprint_placement(&mut digest, &mut fingerprint, placement);

    let mut entries = entries.to_vec();
    entries.sort_unstable_by_key(|entry| (entry.identity(), entry.code_offset()));
    content_commitment_bytes(
        &mut digest,
        &mut fingerprint,
        &(entries.len() as u64).to_le_bytes(),
    );
    for entry in entries {
        content_commitment_bytes(
            &mut digest,
            &mut fingerprint,
            &entry.identity().normalized_identity().to_le_bytes(),
        );
        content_commitment_bytes(
            &mut digest,
            &mut fingerprint,
            &entry.code_offset().to_le_bytes(),
        );
    }

    content_commitment_bytes(
        &mut digest,
        &mut fingerprint,
        &(relocations.len() as u64).to_le_bytes(),
    );
    for relocation in relocations {
        content_commitment_bytes(
            &mut digest,
            &mut fingerprint,
            &[match relocation.kind {
                ArtifactRelocationKind::Absolute64 => 1,
                ArtifactRelocationKind::X86Relative32 => 2,
                ArtifactRelocationKind::Aarch64Page21 => 3,
                ArtifactRelocationKind::Aarch64PageOffset12 => 4,
                ArtifactRelocationKind::Aarch64Branch26 => 5,
            }],
        );
        content_commitment_bytes(
            &mut digest,
            &mut fingerprint,
            &relocation.destination_offset.to_le_bytes(),
        );
        match relocation.target {
            RelocationTarget::Data(identity) => {
                content_commitment_bytes(&mut digest, &mut fingerprint, &[1]);
                content_commitment_bytes(
                    &mut digest,
                    &mut fingerprint,
                    &identity.normalized_identity().to_le_bytes(),
                );
            }
            RelocationTarget::Entry(identity) => {
                content_commitment_bytes(&mut digest, &mut fingerprint, &[2]);
                content_commitment_bytes(
                    &mut digest,
                    &mut fingerprint,
                    &identity.normalized_identity().to_le_bytes(),
                );
            }
        }
        content_commitment_bytes(
            &mut digest,
            &mut fingerprint,
            &relocation.addend.to_le_bytes(),
        );
    }

    if let Some(commitments) = authority_commitments {
        digest.update(commitments.imported_contracts().as_bytes());
        digest.update(commitments.declared_footprint().as_bytes());
        digest.update(commitments.machine_regime().as_bytes());
        digest.update(commitments.installation_scope().as_bytes());
    }

    Ok((
        ArtifactContentDigest::from_digest(digest.finalize().into()),
        NonAuthoritativeContainerFingerprint64::from_compatibility_value(if fingerprint == 0 {
            1
        } else {
            fingerprint
        })?,
    ))
}

fn fingerprint_placement(
    digest: &mut Sha256,
    compatibility_fingerprint: &mut u64,
    placement: PlacementConstraints,
) {
    if let Some(range) = placement.permitted_range() {
        content_commitment_bytes(digest, compatibility_fingerprint, &[1]);
        content_commitment_bytes(
            digest,
            compatibility_fingerprint,
            &range.start_inclusive().to_le_bytes(),
        );
        content_commitment_bytes(
            digest,
            compatibility_fingerprint,
            &range.end_exclusive().to_le_bytes(),
        );
    } else {
        content_commitment_bytes(digest, compatibility_fingerprint, &[0]);
    }
    content_commitment_bytes(
        digest,
        compatibility_fingerprint,
        &placement.alignment().to_le_bytes(),
    );
    content_commitment_bytes(
        digest,
        compatibility_fingerprint,
        &[match placement.phase() {
            layout_plans::PlacementPhase::Build => 1,
            layout_plans::PlacementPhase::Load => 2,
            layout_plans::PlacementPhase::PostHandoff => 3,
        }],
    );
    if let Some(regime) = placement.machine_regime() {
        content_commitment_bytes(digest, compatibility_fingerprint, &[1]);
        content_commitment_bytes(
            digest,
            compatibility_fingerprint,
            &regime.normalized_identity().to_le_bytes(),
        );
    } else {
        content_commitment_bytes(digest, compatibility_fingerprint, &[0]);
    }
    if let Some(scope) = placement.installation_scope() {
        content_commitment_bytes(digest, compatibility_fingerprint, &[1]);
        content_commitment_bytes(
            digest,
            compatibility_fingerprint,
            &scope.normalized_identity().to_le_bytes(),
        );
    } else {
        content_commitment_bytes(digest, compatibility_fingerprint, &[0]);
    }
}

fn content_commitment_bytes(digest: &mut Sha256, fingerprint: &mut u64, bytes: &[u8]) {
    digest.update(bytes);
    fingerprint_bytes(fingerprint, bytes);
}

fn fingerprint_bytes(fingerprint: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *fingerprint ^= u64::from(*byte);
        *fingerprint = fingerprint.wrapping_mul(0x100_0000_01b3);
    }
}

pub(super) fn validate_decoded_relocations(
    mut relocations: Vec<DecodedArtifactRelocation>,
    architecture: Architecture,
    code_length: u64,
    max_relocations: usize,
) -> Result<Vec<DecodedArtifactRelocation>, InstallationDiagnostic> {
    if relocations.len() > max_relocations {
        return Err(InstallationDiagnostic(format!(
            "artifact has {} relocations, exceeding configured bound {}",
            relocations.len(),
            max_relocations
        )));
    }

    relocations.sort_unstable_by_key(|relocation| {
        (
            relocation.destination_offset,
            relocation.kind,
            relocation.target,
            relocation.addend,
        )
    });
    let mut prior_end = 0u64;
    for (index, relocation) in relocations.iter().enumerate() {
        if !relocation.kind.supports(architecture) {
            return Err(InstallationDiagnostic(format!(
                "artifact relocation {:?} is incompatible with architecture {:?}",
                relocation.kind, architecture
            )));
        }
        let width = relocation.kind.byte_width();
        let end = relocation
            .destination_offset
            .checked_add(width)
            .ok_or_else(|| InstallationDiagnostic("artifact relocation range overflows".into()))?;
        if end > code_length {
            return Err(InstallationDiagnostic(format!(
                "artifact relocation at {}..{} exceeds {}-byte code section",
                relocation.destination_offset, end, code_length
            )));
        }
        if index != 0 && relocation.destination_offset < prior_end {
            return Err(InstallationDiagnostic(format!(
                "artifact relocation at byte {} overlaps another relocation field",
                relocation.destination_offset
            )));
        }
        prior_end = end;
    }
    Ok(relocations)
}

fn require_identity<T: Copy + PartialEq>(
    name: &str,
    actual: T,
    expected: T,
) -> Result<(), InstallationDiagnostic> {
    if actual != expected {
        return Err(InstallationDiagnostic(format!(
            "{name} section identity does not match canonical header"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
