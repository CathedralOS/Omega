//! Substitutable-field inventory for the executable-container custody
//! matrix driven by `tests.rs`'s
//! `executable_container_rejects_every_one_field_substitution`.
//!
//! The declared lanes are the `DecodedArtifactContainer` record's
//! substitution axes: the claimed artifact identity, the executable
//! content and its placement report coordinates, the entry and relocation
//! rosters, the strong authority commitments, the exact proof payload, the
//! envelope directory axes canonicalization erases, and the roster-order
//! axes canonicalization sorts. Keeping the inventory in its own file
//! names the boundary between what is declared substitutable and the
//! shared `run_one_field_substitution_matrix` driver that exercises every
//! leg.
//!
//! Outcomes split three ways across the family's checker chain — the
//! honest compatibility-fingerprint recompute, canonical
//! `validate_decoded_container`, then `admit_validated_container`
//! replaying the authentic evidence. A representable substitution
//! survives validation and the replay refuses to bind it; a
//! non-representable one rejects at validation with its named invariant;
//! an informational substitution is admitted because the axis was never
//! artifact custody.
//!
//! Named axes that are deliberately not lanes: `content_fingerprint` is
//! written only by the honest recompute — a substituted value is stale
//! data rather than a representable axis, and the test asserts the drift
//! rejection directly; the admission evidence's own fields (`accepted`,
//! the bound artifact) belong to a different record family the gate legs
//! tamper with directly; a zero normalized identity is refused by every
//! identity constructor before any record exists.

use super::{decoded, id};
use crate::artifacts::ArtifactEntry;
use crate::artifacts::container::{
    ArtifactRelocationKind, ContainerSectionKind, DecodedArtifactContainer,
    DecodedArtifactRelocation, OMEGA_EXECUTABLE_CONTAINER_V1_MARKER,
    normalized_proof_payload_digest,
};
use crate::authority_digests::{
    ArtifactAuthorityCommitments, ArtifactId, EntrySetId, MachineContractSetId, MachineFootprintId,
    PlacementPlanId, RelocationSetId,
};
use layout_plans::{
    ArtifactInstallationScopeId, DataSymbolId, EntryStubId, MachineRegimeId, PlacementAddressRange,
    PlacementConstraints, PlacementPhase, RelocationTarget,
};
use target::Architecture;

optimization_core::custody_field_inventory! {
    /// One substitutable lane of the decoded `DecodedArtifactContainer`
    /// record; checked by the fingerprint-recompute, canonical-validation,
    /// and admission-replay chain in
    /// `executable_container_rejects_every_one_field_substitution`.
    /// Coupled axes are single lanes: the `Architecture` move carries the
    /// relocation kind the new architecture can still carry, `CodeExtent`
    /// carries the header/section/relocation joins that keep the shorter
    /// code honest, and each report coordinate carries its section-record
    /// join — the `*WithoutRecommit` lanes name exactly the same
    /// coordinate with the strong commitments left stale.
    pub enum ExecutableContainerFieldForTest {
        // Representable lanes: canonical validation passes and the
        // admission replay refuses to bind the substitution.
        ClaimedArtifactIdentity,
        Architecture,
        CodeBytes,
        CodeExtent,
        Contracts,
        DeclaredFootprint,
        PlacementPlan,
        PlacementPhase,
        PlacementAlignment,
        PlacementPermittedRange,
        PlacementMachineRegime,
        PlacementInstallationScope,
        EntrySet,
        EntryIdentity,
        EntryCodeOffset,
        EntryRoster,
        RelocationSet,
        RelocationKind,
        RelocationDestination,
        RelocationTarget,
        RelocationAddend,
        RelocationRoster,
        AuthorityCommitments,
        ProofPayload,
        // Informational lanes: the whole chain admits the substitution —
        // the axis canonicalizes away before artifact custody.
        TotalLengthBeyondSections,
        SectionRosterOrder,
        SectionPayloadCoordinates,
        // The version-1 marker axis: a consistent v1 substitution still
        // validates for compatibility (and fails replay plus its own
        // admission), while v1 carrying v2 commitments rejects validation.
        FormatMarkerV1,
        // Canonical-validation rejections: the substitution cannot be
        // carried into a checked container at all.
        FormatMarkerUnknown,
        FormatMarkerV1WithCommitments,
        HeaderCodeLength,
        TotalLengthBelowSections,
        RelocationSectionIdentity,
        ProofSectionIdentity,
        ContractSectionIdentity,
        ContractsWithoutRecommit,
        FootprintWithoutRecommit,
        RegimeWithoutRecommit,
        ScopeWithoutRecommit,
        CommitmentsDropped,
        CommitmentSectionDropped,
        ContractSectionDropped,
        EntriesDropped,
        EntrySectionEmptied,
        EntriesDuplicated,
        EntryOutsideCode,
        EntryCodeAlignment,
        RelocationOutsideCode,
        RelocationKindForeignArchitecture,
        RelocationsOverlap,
        ProofPayloadAlone,
        // Roster-order lanes: an inserted member leaves the roster in
        // non-canonical order; canonicalization sorts it, so custody moves
        // and the sorted form is the identical artifact.
        RelocationRosterPermuted,
        EntryRosterPermuted,
    }
}

/// The named independent-checker verdict for a substituted container.
/// Canonical-validation verdicts name the invariant the mutated record
/// breaks; replay verdicts name the admission seam that refuses the
/// substituted record; `EnvelopeInformational` names a substitution the
/// whole chain admits because the axis carries no custody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutableContainerCustodyCheck {
    // Canonical-validation verdicts.
    /// The format marker is not a supported container version.
    UnsupportedFormatMarker,
    /// A version-1 marker cannot carry version-2 authority commitments.
    V1CarriesStrongCommitments,
    /// The declared code length disagrees with the decoded code bytes.
    HeaderCodeLengthMismatch,
    /// The declared total length places a section out of bounds.
    DeclaredLengthBelowSections,
    /// The relocation section's identity must restate the header's
    /// relocation set.
    RelocationSectionIdentityMismatch,
    /// The proof section's identity must restate the exact proof payload.
    ProofSectionIdentityMismatch,
    /// The contracts section's identity must restate the header's
    /// contract report coordinate.
    ContractSectionIdentityMismatch,
    /// Version-2 strong commitments must match their compact report
    /// coordinates.
    CompactReportCoordinates,
    /// Dropping the decoded commitments while their section stays is not
    /// canonical.
    CommitmentsDropped,
    /// A version-2 container carries exactly one authority-commitment
    /// section.
    CommitmentSectionRequired,
    /// A container carries exactly one contracts section.
    ContractsSectionRequired,
    /// The entries section length must bind the decoded entry roster.
    EntryCountMismatch,
    /// An entry section may not be empty.
    EntrySectionBound,
    /// Entry identities must be unique.
    EntriesNotUnique,
    /// Every entry offset must lie inside the code extent.
    EntryOutsideCode,
    /// AArch64 entry offsets must be instruction-aligned.
    EntryNotAligned,
    /// Every relocation field must lie inside the code extent.
    RelocationOutsideCode,
    /// A relocation kind must be representable on the architecture.
    RelocationForeignArchitecture,
    /// Relocation fields may not overlap.
    RelocationsOverlap,
    /// The proof payload digest must match the exact proof bytes.
    ProofBytesNotExact,
    // Admission-replay verdicts.
    /// The substituted record validates but replays to a different
    /// validated container than the authentic evidence binds.
    DifferentValidatedContainer,
    /// The substituted proof payload differs from the payload the
    /// authentic evidence binds.
    DifferentProofPayload,
    // The whole chain admits the substitution.
    /// The axis is envelope data canonicalization erases: the substitution
    /// produces the identical artifact and the authentic evidence still
    /// admits it.
    EnvelopeInformational,
}

/// The diagnostic fragment each canonical-validation verdict declares.
/// The family's `InstallationDiagnostic` is a string payload, so the
/// verdict vocabulary classifies by the same needles the legacy battery
/// asserted; a validation rejection outside this vocabulary is a shape
/// the family does not declare and panics.
const VALIDATION_NEEDLES: &[(ExecutableContainerCustodyCheck, &str)] = &[
    (
        ExecutableContainerCustodyCheck::UnsupportedFormatMarker,
        "unsupported Omega executable container marker",
    ),
    (
        ExecutableContainerCustodyCheck::V1CarriesStrongCommitments,
        "container-v1 cannot carry v2 authority commitments",
    ),
    (
        ExecutableContainerCustodyCheck::HeaderCodeLengthMismatch,
        "canonical header declares",
    ),
    (
        ExecutableContainerCustodyCheck::DeclaredLengthBelowSections,
        "exceeds 512-byte container",
    ),
    (
        ExecutableContainerCustodyCheck::RelocationSectionIdentityMismatch,
        "relocation section identity does not match canonical header",
    ),
    (
        ExecutableContainerCustodyCheck::ProofSectionIdentityMismatch,
        "proof section identity does not match canonical header",
    ),
    (
        ExecutableContainerCustodyCheck::ContractSectionIdentityMismatch,
        "contract section identity does not match canonical header",
    ),
    (
        ExecutableContainerCustodyCheck::CompactReportCoordinates,
        "compact report coordinates",
    ),
    (
        ExecutableContainerCustodyCheck::CommitmentsDropped,
        "does not match decoded strong evidence",
    ),
    (
        ExecutableContainerCustodyCheck::CommitmentSectionRequired,
        "container-v2 requires exactly one authority-commitment section",
    ),
    (
        ExecutableContainerCustodyCheck::ContractsSectionRequired,
        "exactly one contracts section",
    ),
    (
        ExecutableContainerCustodyCheck::EntryCountMismatch,
        "does not match 0 canonical entries",
    ),
    (
        ExecutableContainerCustodyCheck::EntrySectionBound,
        "is empty or exceeds configured bound",
    ),
    (
        ExecutableContainerCustodyCheck::EntriesNotUnique,
        "must be unique",
    ),
    (
        ExecutableContainerCustodyCheck::EntryOutsideCode,
        "lies outside",
    ),
    (
        ExecutableContainerCustodyCheck::EntryNotAligned,
        "not instruction-aligned",
    ),
    (
        ExecutableContainerCustodyCheck::RelocationOutsideCode,
        "exceeds 64-byte code",
    ),
    (
        ExecutableContainerCustodyCheck::RelocationForeignArchitecture,
        "incompatible with architecture",
    ),
    (
        ExecutableContainerCustodyCheck::RelocationsOverlap,
        "overlaps",
    ),
    (
        ExecutableContainerCustodyCheck::ProofBytesNotExact,
        "does not match the exact proof bytes",
    ),
];

/// Classify a canonical-validation rejection into the family's named
/// vocabulary.
pub fn classify_container_validation(
    error: &crate::installation::InstallationDiagnostic,
) -> ExecutableContainerCustodyCheck {
    for (verdict, needle) in VALIDATION_NEEDLES {
        if error.0.contains(needle) {
            return *verdict;
        }
    }
    panic!("validation rejection outside the declared vocabulary: {error}");
}

/// Classify an admission-replay rejection into the family's named
/// vocabulary.
pub fn classify_container_replay(
    error: &crate::installation::InstallationDiagnostic,
) -> ExecutableContainerCustodyCheck {
    if error.0.contains("different proof payload") {
        ExecutableContainerCustodyCheck::DifferentProofPayload
    } else if error.0.contains("different validated container") {
        ExecutableContainerCustodyCheck::DifferentValidatedContainer
    } else {
        panic!("admission replay rejection outside the declared vocabulary: {error}");
    }
}

/// An authentic foreign container: a different claimed artifact identity
/// still validates canonically — the claimed identity rides outside the
/// content digest — so its retained custody differs from the honest
/// record the matrix builds and the `ClaimedArtifactIdentity` leg borrows
/// it.
pub fn foreign_container_donor() -> DecodedArtifactContainer {
    let mut donor = decoded();
    donor.artifact = id(2, ArtifactId::from_normalized_identity);
    donor
}

/// Rebuild the strong authority commitments for the exact report
/// coordinates one mutated container carries. The commitment bytes are
/// provider evidence; this helper derives them honestly so the coordinate
/// join stays canonical.
fn commitments_for(container: &DecodedArtifactContainer) -> ArtifactAuthorityCommitments {
    ArtifactAuthorityCommitments::from_canonical_evidence(
        container.contracts,
        b"test imported contract set",
        container.declared_footprint,
        b"test declared footprint",
        container
            .placement_constraints
            .machine_regime()
            .map(|regime| (regime, b"test machine regime".as_slice())),
        container
            .placement_constraints
            .installation_scope()
            .map(|scope| (scope, b"test installation scope".as_slice())),
    )
}

/// Honestly rewrite every copy of the strong commitments a mutated
/// container carries: the decoded field and the authority-commitment
/// section kind are a canonical join, so both must move together.
fn recommit(container: &mut DecodedArtifactContainer) {
    let commitments = commitments_for(container);
    container.authority_commitments = Some(commitments);
    for section in &mut container.sections {
        if let ContainerSectionKind::AuthorityCommitments(_) = section.kind {
            section.kind = ContainerSectionKind::AuthorityCommitments(commitments);
        }
    }
}

/// The family's honest-recomputation hook: mutate exactly the declared
/// lane of `container`, maintaining only the canonical joins inside the
/// record (section kinds restating header coordinates, section lengths
/// binding rosters, strong commitments restating report coordinates), so
/// the compatibility fingerprint and the admission replay recompute over
/// the substitution. `donor` supplies the foreign artifact identity the
/// `ClaimedArtifactIdentity` leg borrows; the remaining legs take fixed
/// non-identity alternates.
pub fn substitute_executable_container_for_test(
    container: &mut DecodedArtifactContainer,
    field: ExecutableContainerFieldForTest,
    donor: &DecodedArtifactContainer,
) {
    use ExecutableContainerFieldForTest as Leg;
    match field {
        // The claimed normalized identity rides outside the content
        // digest: it is bound only by the admission evidence.
        Leg::ClaimedArtifactIdentity => container.artifact = donor.artifact,
        // Aarch64 cannot carry the fixture's X86Relative32 relocation, so
        // the axis carries its relocation kind along to stay representable.
        Leg::Architecture => {
            container.architecture = Architecture::Aarch64;
            container.relocations[0].kind = ArtifactRelocationKind::Absolute64;
        }
        Leg::CodeBytes => container.code[0] ^= 1,
        Leg::CodeExtent => {
            container.code.truncate(32);
            container.code_length = 32;
            container.sections[0].length = 32;
            container.relocations[0].destination_offset = 8;
        }
        Leg::Contracts => {
            container.contracts = id(13, MachineContractSetId::from_normalized_identity);
            container.sections[2].kind = ContainerSectionKind::Contracts(container.contracts);
            recommit(container);
        }
        Leg::DeclaredFootprint => {
            container.declared_footprint = id(14, MachineFootprintId::from_normalized_identity);
            container.sections[3].kind =
                ContainerSectionKind::Footprint(container.declared_footprint);
            recommit(container);
        }
        Leg::PlacementPlan => {
            container.placement_plan = id(15, PlacementPlanId::from_normalized_identity);
            container.sections[4].kind = ContainerSectionKind::Placement(container.placement_plan);
        }
        Leg::PlacementPhase => {
            container.placement_constraints =
                PlacementConstraints::unconstrained(PlacementPhase::PostHandoff);
        }
        Leg::PlacementAlignment => {
            container.placement_constraints =
                PlacementConstraints::new(None, 8, PlacementPhase::Load, None, None)
                    .expect("aligned placement constraints");
        }
        Leg::PlacementPermittedRange => {
            container.placement_constraints = PlacementConstraints::new(
                Some(PlacementAddressRange::new(0x1000, 0x2000).expect("permitted range")),
                1,
                PlacementPhase::Load,
                None,
                None,
            )
            .expect("ranged placement constraints");
        }
        Leg::PlacementMachineRegime => {
            let regime = MachineRegimeId::from_normalized_identity(10).expect("regime");
            container.placement_constraints =
                PlacementConstraints::new(None, 1, PlacementPhase::Load, Some(regime), None)
                    .expect("regime placement constraints");
            recommit(container);
        }
        Leg::PlacementInstallationScope => {
            let scope = ArtifactInstallationScopeId::from_normalized_identity(11).expect("scope");
            container.placement_constraints =
                PlacementConstraints::new(None, 1, PlacementPhase::Load, None, Some(scope))
                    .expect("scope placement constraints");
            recommit(container);
        }
        Leg::EntrySet => {
            container.entry_set = id(18, EntrySetId::from_normalized_identity);
            container.sections[5].kind = ContainerSectionKind::Entries(container.entry_set);
        }
        Leg::EntryIdentity => {
            container.entries[0] = ArtifactEntry::from_canonical_decode(
                EntryStubId::from_normalized_identity(10).expect("entry identity"),
                container.entries[0].code_offset(),
            );
        }
        Leg::EntryCodeOffset => {
            container.entries[0] =
                ArtifactEntry::from_canonical_decode(container.entries[0].identity(), 24);
        }
        Leg::EntryRoster => {
            container.entries.push(ArtifactEntry::from_canonical_decode(
                EntryStubId::from_normalized_identity(10).expect("entry identity"),
                24,
            ));
            container.sections[5].length = 32;
        }
        Leg::RelocationSet => {
            container.relocation_set = id(16, RelocationSetId::from_normalized_identity);
            container.sections[1].kind =
                ContainerSectionKind::Relocations(container.relocation_set);
        }
        Leg::RelocationKind => {
            container.relocations[0].kind = ArtifactRelocationKind::Absolute64;
        }
        Leg::RelocationDestination => container.relocations[0].destination_offset = 40,
        Leg::RelocationTarget => {
            container.relocations[0].target = RelocationTarget::Data(
                DataSymbolId::from_normalized_identity(12).expect("data symbol"),
            );
        }
        Leg::RelocationAddend => container.relocations[0].addend = 7,
        Leg::RelocationRoster => {
            container.relocations.clear();
            container.sections[1].length = 8;
        }
        Leg::AuthorityCommitments => {
            let forged = ArtifactAuthorityCommitments::from_canonical_evidence(
                container.contracts,
                b"forged imported contract set",
                container.declared_footprint,
                b"test declared footprint",
                None,
                None,
            );
            container.authority_commitments = Some(forged);
            for section in &mut container.sections {
                if let ContainerSectionKind::AuthorityCommitments(_) = section.kind {
                    section.kind = ContainerSectionKind::AuthorityCommitments(forged);
                }
            }
        }
        // The exact proof payload is admission evidence: it stays outside
        // the executable content identity yet replay still binds the exact
        // bytes.
        Leg::ProofPayload => {
            container.proof[0] ^= 1;
            container.proof_payload = normalized_proof_payload_digest(&container.proof);
            container.sections[6].kind = ContainerSectionKind::Proof(container.proof_payload);
        }
        // ── informational axes ──
        Leg::TotalLengthBeyondSections => container.total_length = 640,
        Leg::SectionRosterOrder => container.sections.swap(2, 3),
        Leg::SectionPayloadCoordinates => {
            container.sections[2].offset = 208;
            container.sections[3].offset = 144;
        }
        // ── canonical-validation rejections ──
        Leg::FormatMarkerUnknown => container.format_marker = 0x9999,
        // A consistent version-1 container: the marker move requires
        // dropping the version-2-only authority-commitment join — it
        // validates for compatibility yet cannot be admitted.
        Leg::FormatMarkerV1 => {
            container.format_marker = OMEGA_EXECUTABLE_CONTAINER_V1_MARKER;
            container.authority_commitments = None;
            container.sections.pop();
        }
        Leg::FormatMarkerV1WithCommitments => {
            container.format_marker = OMEGA_EXECUTABLE_CONTAINER_V1_MARKER;
        }
        Leg::HeaderCodeLength => container.code_length = 32,
        Leg::TotalLengthBelowSections => container.total_length = 512,
        Leg::RelocationSectionIdentity => {
            container.sections[1].kind = ContainerSectionKind::Relocations(id(
                99,
                RelocationSetId::from_normalized_identity,
            ));
        }
        Leg::ProofSectionIdentity => {
            container.sections[6].kind =
                ContainerSectionKind::Proof(normalized_proof_payload_digest(b"x"));
        }
        Leg::ContractSectionIdentity => {
            container.contracts = id(13, MachineContractSetId::from_normalized_identity);
        }
        Leg::ContractsWithoutRecommit => {
            container.contracts = id(13, MachineContractSetId::from_normalized_identity);
            container.sections[2].kind = ContainerSectionKind::Contracts(container.contracts);
        }
        Leg::FootprintWithoutRecommit => {
            container.declared_footprint = id(14, MachineFootprintId::from_normalized_identity);
            container.sections[3].kind =
                ContainerSectionKind::Footprint(container.declared_footprint);
        }
        Leg::RegimeWithoutRecommit => {
            container.placement_constraints = PlacementConstraints::new(
                None,
                1,
                PlacementPhase::Load,
                Some(MachineRegimeId::from_normalized_identity(10).expect("regime")),
                None,
            )
            .expect("regime placement constraints");
        }
        Leg::ScopeWithoutRecommit => {
            container.placement_constraints = PlacementConstraints::new(
                None,
                1,
                PlacementPhase::Load,
                None,
                Some(ArtifactInstallationScopeId::from_normalized_identity(11).expect("scope")),
            )
            .expect("scope placement constraints");
        }
        Leg::CommitmentsDropped => container.authority_commitments = None,
        Leg::CommitmentSectionDropped => {
            container.authority_commitments = None;
            container.sections.pop();
        }
        Leg::ContractSectionDropped => {
            container.sections.remove(2);
        }
        Leg::EntriesDropped => container.entries.clear(),
        Leg::EntrySectionEmptied => {
            container.entries.clear();
            container.sections[5].length = 0;
        }
        Leg::EntriesDuplicated => {
            container.entries.push(container.entries[0]);
            container.sections[5].length = 32;
        }
        Leg::EntryOutsideCode => {
            container.entries[0] =
                ArtifactEntry::from_canonical_decode(container.entries[0].identity(), 64);
        }
        // The unaligned-entry axis needs an Aarch64 container to see the
        // alignment rule, so it stages the representable Aarch64 form and
        // moves one entry off an instruction boundary.
        Leg::EntryCodeAlignment => {
            container.architecture = Architecture::Aarch64;
            container.relocations[0].kind = ArtifactRelocationKind::Absolute64;
            container.entries[0] =
                ArtifactEntry::from_canonical_decode(container.entries[0].identity(), 17);
        }
        Leg::RelocationOutsideCode => container.relocations[0].destination_offset = 62,
        Leg::RelocationKindForeignArchitecture => {
            container.relocations[0].kind = ArtifactRelocationKind::Aarch64Branch26;
        }
        Leg::RelocationsOverlap => {
            container.relocations.push(container.relocations[0]);
            container.sections[1].length = 72;
        }
        Leg::ProofPayloadAlone => {
            container.proof_payload = normalized_proof_payload_digest(b"different proof");
        }
        // ── roster-order canonicalization ──
        // The pushed member leaves the roster in non-canonical order:
        // relocations canonically sort by destination (8 ahead of 32) and
        // entries by identity (9 ahead of 10), so custody moves while the
        // sorted form is the identical artifact.
        Leg::RelocationRosterPermuted => {
            container.relocations.push(DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Absolute64,
                destination_offset: 8,
                target: RelocationTarget::Data(
                    DataSymbolId::from_normalized_identity(12).expect("data symbol"),
                ),
                addend: 0,
            });
            container.sections[1].length = 72;
        }
        Leg::EntryRosterPermuted => {
            container.entries.push(ArtifactEntry::from_canonical_decode(
                EntryStubId::from_normalized_identity(10).expect("entry identity"),
                24,
            ));
            container.entries.swap(0, 1);
            container.sections[5].length = 32;
        }
    }
}

/// The exact checker verdict each declared leg must produce.
pub fn executable_container_custody_outcome(
    field: ExecutableContainerFieldForTest,
) -> optimization_core::MutationOutcome<ExecutableContainerCustodyCheck> {
    use ExecutableContainerFieldForTest as Leg;
    let check = match field {
        Leg::ClaimedArtifactIdentity
        | Leg::Architecture
        | Leg::CodeBytes
        | Leg::CodeExtent
        | Leg::Contracts
        | Leg::DeclaredFootprint
        | Leg::PlacementPlan
        | Leg::PlacementPhase
        | Leg::PlacementAlignment
        | Leg::PlacementPermittedRange
        | Leg::PlacementMachineRegime
        | Leg::PlacementInstallationScope
        | Leg::EntrySet
        | Leg::EntryIdentity
        | Leg::EntryCodeOffset
        | Leg::EntryRoster
        | Leg::RelocationSet
        | Leg::RelocationKind
        | Leg::RelocationDestination
        | Leg::RelocationTarget
        | Leg::RelocationAddend
        | Leg::RelocationRoster
        | Leg::AuthorityCommitments
        | Leg::FormatMarkerV1
        | Leg::RelocationRosterPermuted
        | Leg::EntryRosterPermuted => ExecutableContainerCustodyCheck::DifferentValidatedContainer,
        Leg::ProofPayload => ExecutableContainerCustodyCheck::DifferentProofPayload,
        Leg::TotalLengthBeyondSections
        | Leg::SectionRosterOrder
        | Leg::SectionPayloadCoordinates => ExecutableContainerCustodyCheck::EnvelopeInformational,
        Leg::FormatMarkerUnknown => ExecutableContainerCustodyCheck::UnsupportedFormatMarker,
        Leg::FormatMarkerV1WithCommitments => {
            ExecutableContainerCustodyCheck::V1CarriesStrongCommitments
        }
        Leg::HeaderCodeLength => ExecutableContainerCustodyCheck::HeaderCodeLengthMismatch,
        Leg::TotalLengthBelowSections => {
            ExecutableContainerCustodyCheck::DeclaredLengthBelowSections
        }
        Leg::RelocationSectionIdentity => {
            ExecutableContainerCustodyCheck::RelocationSectionIdentityMismatch
        }
        Leg::ProofSectionIdentity => ExecutableContainerCustodyCheck::ProofSectionIdentityMismatch,
        Leg::ContractSectionIdentity => {
            ExecutableContainerCustodyCheck::ContractSectionIdentityMismatch
        }
        Leg::ContractsWithoutRecommit
        | Leg::FootprintWithoutRecommit
        | Leg::RegimeWithoutRecommit
        | Leg::ScopeWithoutRecommit => ExecutableContainerCustodyCheck::CompactReportCoordinates,
        Leg::CommitmentsDropped => ExecutableContainerCustodyCheck::CommitmentsDropped,
        Leg::CommitmentSectionDropped => ExecutableContainerCustodyCheck::CommitmentSectionRequired,
        Leg::ContractSectionDropped => ExecutableContainerCustodyCheck::ContractsSectionRequired,
        Leg::EntriesDropped => ExecutableContainerCustodyCheck::EntryCountMismatch,
        Leg::EntrySectionEmptied => ExecutableContainerCustodyCheck::EntrySectionBound,
        Leg::EntriesDuplicated => ExecutableContainerCustodyCheck::EntriesNotUnique,
        Leg::EntryOutsideCode => ExecutableContainerCustodyCheck::EntryOutsideCode,
        Leg::EntryCodeAlignment => ExecutableContainerCustodyCheck::EntryNotAligned,
        Leg::RelocationOutsideCode => ExecutableContainerCustodyCheck::RelocationOutsideCode,
        Leg::RelocationKindForeignArchitecture => {
            ExecutableContainerCustodyCheck::RelocationForeignArchitecture
        }
        Leg::RelocationsOverlap => ExecutableContainerCustodyCheck::RelocationsOverlap,
        Leg::ProofPayloadAlone => ExecutableContainerCustodyCheck::ProofBytesNotExact,
    };
    optimization_core::MutationOutcome::ExactError(check)
}
