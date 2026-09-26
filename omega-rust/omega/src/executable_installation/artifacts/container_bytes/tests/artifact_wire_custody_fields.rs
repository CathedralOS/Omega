//! Substitutable-field inventory for the artifact half of the
//! executable-container wire custody matrix driven by `tests.rs`'s
//! `executable_container_wire_rejects_every_one_field_substitution`.
//!
//! The declared lanes are the `Artifact` record's substitution axes as the
//! canonical encoder carries them onto the wire: the claimed artifact
//! identity, the executable content and its placement report coordinates,
//! the entry and relocation rosters, and the strong authority commitments.
//! `ArtifactSeed` is the mutable copy of every field; each lane substitutes
//! once and `Artifact::from_canonical_decode` honestly recomputes the
//! containing content digest and container fingerprint.
//!
//! Outcomes split three ways across the family's checker chain — canonical
//! artifact construction, the canonical encoder, then a canonical decode
//! whose admission replays the authentic evidence. A representable
//! substitution survives construction, encoding, and decode, and the replay
//! refuses to bind it; a substitution construction cannot carry rejects
//! there with its named invariant; one the encoder cannot carry rejects at
//! encoding with its configured bound.
//!
//! Named axes that are deliberately not lanes: the exact proof payload is
//! an encoder argument rather than an artifact field — the empty-proof and
//! version-1 encoder refusals are asserted directly, and the decoded-record
//! matrix in `container/tests` carries the proof lanes; raw wire bytes the
//! encoder derives are the separate wire-byte inventory.

use crate::executable_installation::artifacts::Artifact;
use crate::executable_installation::artifacts::ArtifactEntry;
use crate::executable_installation::artifacts::container::{
    ArtifactRelocationKind, DecodedArtifactRelocation,
};
use crate::executable_installation::authority_digests::{
    ArtifactAuthorityCommitments, ArtifactId, EntrySetId, MachineContractSetId, MachineFootprintId,
    PlacementPlanId, RelocationSetId,
};
use crate::executable_installation::installation::InstallationDiagnostic;
use optimization_core::MutationOutcome;
use target::Architecture;
use terminal_psi::layout_plans::{
    ArtifactInstallationScopeId, DataSymbolId, EntryStubId, MachineRegimeId, PlacementAddressRange,
    PlacementConstraints, PlacementPhase, RelocationTarget,
};

optimization_core::custody_field_inventory! {
    /// One substitutable lane of the `Artifact` record a canonical container
    /// encodes; checked by the construction, encoding, and admission-replay
    /// chain in `executable_container_wire_rejects_every_one_field_substitution`.
    /// Coupled axes are single lanes: the `Architecture` move carries the
    /// relocation kind the new architecture can still carry, `CodeExtent`
    /// carries the relocation destination that keeps the shorter code
    /// honest, and each report coordinate carries its recomputed strong
    /// commitments — the `*WithoutRecommit` lanes name exactly the same
    /// coordinate with the strong commitments left stale.
    pub enum ArtifactWireFieldForTest {
        // Representable lanes: construction, encoding, and decode pass and
        // the admission replay refuses to bind the substitution.
        ClaimedArtifactIdentity,
        Architecture,
        CodeBytes,
        CodeExtent,
        Contracts,
        DeclaredFootprint,
        PlacementPlan,
        PlacementAlignment,
        PlacementPhase,
        PlacementPermittedRange,
        PlacementMachineRegime,
        PlacementInstallationScope,
        PlacementMachineRegimeDropped,
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
        // Construction rejections: canonical artifact construction cannot
        // carry the substitution at all.
        CodeEmptied,
        EntriesDropped,
        EntriesDuplicated,
        EntryOutsideCode,
        ContractsWithoutRecommit,
        FootprintWithoutRecommit,
        RegimeDroppedWithoutRecommit,
        // Encoding rejections: the artifact constructs, but the canonical
        // encoder refuses to exceed a configured container bound.
        RelocationRosterBeyondBound,
        CodeBeyondSectionBound,
    }
}

/// The named independent-checker verdict for one container wire
/// substitution: the seam that refused it and the diagnostic fragment the
/// refusal carries. The family's `InstallationDiagnostic` is a string
/// payload, so a verdict classifies by the declared fragments; a rejection
/// outside the declared vocabulary is a shape the family does not declare
/// and panics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerWireRejection {
    /// `Artifact::from_canonical_decode` refused the substituted record.
    Construction(&'static str),
    /// `encode_executable_container` refused the substituted artifact.
    Encoding(&'static str),
    /// `decode_executable_container` refused the substituted wire bytes.
    Decode(&'static str),
    /// `admit_validated_container` refused to bind the substituted
    /// container to the authentic evidence.
    Replay(&'static str),
}

/// Classify `diagnostic` into the one declared fragment it names. A
/// declared fragment nested inside another contained fragment
/// (`must be required` inside `container-v2 authority commitments must be
/// required`) is not a separate match; two independent contained fragments
/// are an ambiguous vocabulary and panic, as does a rejection naming none.
pub fn classify_container_wire_rejection(
    diagnostic: &InstallationDiagnostic,
    vocabulary: impl IntoIterator<Item = ContainerWireRejection>,
) -> ContainerWireRejection {
    let mut contained: Vec<ContainerWireRejection> = vocabulary
        .into_iter()
        .filter(|verdict| diagnostic.0.contains(verdict.fragment()))
        .collect();
    contained.sort_unstable_by_key(|verdict| verdict.fragment());
    contained.dedup();
    let maximal: Vec<ContainerWireRejection> = contained
        .iter()
        .copied()
        .filter(|verdict| {
            !contained.iter().any(|other| {
                other.fragment() != verdict.fragment()
                    && other.fragment().contains(verdict.fragment())
            })
        })
        .collect();
    match maximal.as_slice() {
        [verdict] => *verdict,
        [] => panic!("rejection outside the declared vocabulary: {diagnostic}"),
        ambiguous => {
            panic!("rejection {diagnostic} names several declared fragments: {ambiguous:?}")
        }
    }
}

impl ContainerWireRejection {
    /// The diagnostic fragment this verdict requires.
    pub const fn fragment(self) -> &'static str {
        match self {
            Self::Construction(fragment)
            | Self::Encoding(fragment)
            | Self::Decode(fragment)
            | Self::Replay(fragment) => fragment,
        }
    }
}

/// Mutable copy of every `Artifact` field so each representable axis can be
/// substituted once while `Artifact::from_canonical_decode` honestly
/// recomputes the containing content digest and container fingerprint. The
/// seed is the matrix's retained custody view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactSeed {
    pub identity: ArtifactId,
    pub architecture: Architecture,
    pub code: Vec<u8>,
    pub contracts: MachineContractSetId,
    pub declared_footprint: MachineFootprintId,
    pub placement_plan: PlacementPlanId,
    pub placement_constraints: PlacementConstraints,
    pub entry_set: EntrySetId,
    pub entries: Vec<ArtifactEntry>,
    pub relocation_set: RelocationSetId,
    pub relocations: Vec<DecodedArtifactRelocation>,
    pub authority_commitments: ArtifactAuthorityCommitments,
}

impl ArtifactSeed {
    pub fn from(artifact: &Artifact) -> Self {
        Self {
            identity: artifact.0.identity,
            architecture: artifact.0.architecture,
            code: artifact.0.code.clone(),
            contracts: artifact.0.contracts,
            declared_footprint: artifact.0.declared_footprint,
            placement_plan: artifact.0.placement_plan,
            placement_constraints: artifact.0.placement_constraints,
            entry_set: artifact.0.entry_set,
            entries: artifact.0.entries.clone(),
            relocation_set: artifact.0.relocation_set,
            relocations: artifact.0.relocations.clone(),
            authority_commitments: artifact
                .0
                .authority_commitments
                .expect("strong fixture carries commitments"),
        }
    }

    /// Honestly recompute the strong commitments for the seed's current report
    /// coordinates, matching the canonical evidence the fixture provider held.
    fn recommit(&mut self) {
        self.authority_commitments = commitments_with_contract_evidence(
            self,
            b"canonical imported contract set",
            self.placement_constraints.machine_regime(),
        );
    }

    /// Canonical construction over the seed's current fields.
    pub fn construct(self) -> Result<Artifact, InstallationDiagnostic> {
        Artifact::from_canonical_decode(
            self.identity,
            self.architecture,
            self.code,
            self.contracts,
            self.declared_footprint,
            self.placement_plan,
            self.placement_constraints,
            self.entry_set,
            self.entries,
            self.relocation_set,
            self.relocations,
            self.authority_commitments,
        )
    }

    pub fn into_artifact(self) -> Artifact {
        self.construct()
            .expect("mutated artifact must remain canonically constructible")
    }
}

/// Strong commitments over the seed's report coordinates, with `contracts`
/// as the imported contract-set evidence bytes and `regime` as the machine
/// regime coordinate they commit to.
fn commitments_with_contract_evidence(
    seed: &ArtifactSeed,
    contracts: &[u8],
    regime: Option<MachineRegimeId>,
) -> ArtifactAuthorityCommitments {
    ArtifactAuthorityCommitments::from_canonical_evidence(
        seed.contracts,
        contracts,
        seed.declared_footprint,
        b"canonical declared footprint",
        regime.map(|regime| (regime, b"canonical machine regime".as_slice())),
        seed.placement_constraints
            .installation_scope()
            .map(|scope| (scope, b"canonical installation scope".as_slice())),
    )
}

/// Rebuild the seed's placement constraints with exactly one coordinate
/// moved by `edit`.
fn replace_placement(
    seed: &mut ArtifactSeed,
    edit: impl FnOnce(&mut PlacementEdit),
    context: &str,
) {
    let current = seed.placement_constraints;
    let mut placement = PlacementEdit {
        permitted_range: current.permitted_range(),
        alignment: current.alignment(),
        phase: current.phase(),
        machine_regime: current.machine_regime(),
        installation_scope: current.installation_scope(),
    };
    edit(&mut placement);
    seed.placement_constraints = PlacementConstraints::new(
        placement.permitted_range,
        placement.alignment,
        placement.phase,
        placement.machine_regime,
        placement.installation_scope,
    )
    .expect(context);
}

/// The five `PlacementConstraints` coordinates one placement lane edits.
struct PlacementEdit {
    permitted_range: Option<PlacementAddressRange>,
    alignment: u64,
    phase: PlacementPhase,
    machine_regime: Option<MachineRegimeId>,
    installation_scope: Option<ArtifactInstallationScopeId>,
}

/// The family's honest-recomputation hook: mutate exactly the declared
/// lane of `seed`, carrying only the coupled axes the lane names, so
/// canonical construction recomputes the content digest and container
/// fingerprint over the substitution. `donor` supplies the foreign artifact
/// identity the `ClaimedArtifactIdentity` lane borrows; the remaining lanes
/// take fixed non-identity alternates.
pub fn substitute_artifact_wire_for_test(
    seed: &mut ArtifactSeed,
    field: ArtifactWireFieldForTest,
    donor: &ArtifactSeed,
) {
    use ArtifactWireFieldForTest as Lane;
    match field {
        // The claimed normalized identity rides outside the content digest
        // and is bound only by the admission evidence.
        Lane::ClaimedArtifactIdentity => seed.identity = donor.identity,
        // Aarch64 cannot carry the fixture's X86Relative32 relocation, so
        // the axis carries its relocation kind along to stay representable.
        Lane::Architecture => {
            seed.architecture = Architecture::Aarch64;
            seed.relocations[0].kind = ArtifactRelocationKind::Absolute64;
        }
        Lane::CodeBytes => seed.code[0] ^= 1,
        Lane::CodeExtent => {
            seed.code.truncate(32);
            seed.relocations[0].destination_offset = 8;
        }
        Lane::Contracts => {
            seed.contracts = MachineContractSetId::from_normalized_identity(13).expect("contracts");
            seed.recommit();
        }
        Lane::DeclaredFootprint => {
            seed.declared_footprint =
                MachineFootprintId::from_normalized_identity(14).expect("footprint");
            seed.recommit();
        }
        Lane::PlacementPlan => {
            seed.placement_plan = PlacementPlanId::from_normalized_identity(15).expect("plan");
        }
        Lane::PlacementAlignment => {
            replace_placement(seed, |placement| placement.alignment = 8, "alignment");
        }
        Lane::PlacementPhase => replace_placement(
            seed,
            |placement| placement.phase = PlacementPhase::PostHandoff,
            "phase",
        ),
        Lane::PlacementPermittedRange => replace_placement(
            seed,
            |placement| {
                placement.permitted_range =
                    Some(PlacementAddressRange::new(0x1000, 0x2000).expect("permitted range"));
            },
            "range",
        ),
        Lane::PlacementMachineRegime => {
            replace_placement(
                seed,
                |placement| {
                    placement.machine_regime =
                        Some(MachineRegimeId::from_normalized_identity(12).expect("regime"));
                },
                "regime",
            );
            seed.recommit();
        }
        Lane::PlacementInstallationScope => {
            replace_placement(
                seed,
                |placement| {
                    placement.installation_scope = Some(
                        ArtifactInstallationScopeId::from_normalized_identity(13).expect("scope"),
                    );
                },
                "scope",
            );
            seed.recommit();
        }
        Lane::PlacementMachineRegimeDropped => {
            replace_placement(
                seed,
                |placement| placement.machine_regime = None,
                "dropped regime",
            );
            seed.recommit();
        }
        Lane::EntrySet => {
            seed.entry_set = EntrySetId::from_normalized_identity(18).expect("entry set");
        }
        Lane::EntryIdentity => {
            seed.entries[0] = ArtifactEntry::from_canonical_decode(
                EntryStubId::from_normalized_identity(10).expect("entry"),
                seed.entries[0].code_offset(),
            );
        }
        Lane::EntryCodeOffset => {
            seed.entries[0] = ArtifactEntry::from_canonical_decode(seed.entries[0].identity(), 24);
        }
        Lane::EntryRoster => {
            seed.entries.push(ArtifactEntry::from_canonical_decode(
                EntryStubId::from_normalized_identity(10).expect("entry"),
                24,
            ));
        }
        Lane::RelocationSet => {
            seed.relocation_set = RelocationSetId::from_normalized_identity(16).expect("set");
        }
        Lane::RelocationKind => seed.relocations[0].kind = ArtifactRelocationKind::Absolute64,
        Lane::RelocationDestination => seed.relocations[0].destination_offset = 40,
        Lane::RelocationTarget => {
            seed.relocations[0].target = RelocationTarget::Data(
                DataSymbolId::from_normalized_identity(12).expect("data symbol"),
            );
        }
        Lane::RelocationAddend => seed.relocations[0].addend = 7,
        Lane::RelocationRoster => seed.relocations.clear(),
        Lane::AuthorityCommitments => {
            seed.authority_commitments = commitments_with_contract_evidence(
                seed,
                b"forged imported contract set",
                seed.placement_constraints.machine_regime(),
            );
        }
        // ── construction rejections ──
        Lane::CodeEmptied => seed.code.clear(),
        Lane::EntriesDropped => seed.entries.clear(),
        Lane::EntriesDuplicated => seed.entries.push(seed.entries[0]),
        Lane::EntryOutsideCode => {
            seed.entries[0] = ArtifactEntry::from_canonical_decode(seed.entries[0].identity(), 64);
        }
        Lane::ContractsWithoutRecommit => {
            seed.contracts = MachineContractSetId::from_normalized_identity(13).expect("id");
        }
        Lane::FootprintWithoutRecommit => {
            seed.declared_footprint = MachineFootprintId::from_normalized_identity(14).expect("id");
        }
        Lane::RegimeDroppedWithoutRecommit => replace_placement(
            seed,
            |placement| placement.machine_regime = None,
            "dropped regime",
        ),
        // ── encoding rejections ──
        // Seventeen in-bounds relocations over a 128-byte code section: the
        // artifact constructs, but the roster exceeds the configured bound.
        Lane::RelocationRosterBeyondBound => {
            let entry = seed.entries[0].identity();
            seed.code = vec![0x90; 128];
            seed.relocations = (0..17)
                .map(|index| DecodedArtifactRelocation {
                    kind: ArtifactRelocationKind::X86Relative32,
                    destination_offset: index * 4,
                    target: RelocationTarget::Entry(entry),
                    addend: 0,
                })
                .collect();
        }
        // A 2048-byte code section keeps its entry and relocation inside the
        // extent, but exceeds the configured section bound.
        Lane::CodeBeyondSectionBound => {
            seed.code = vec![0x90; 2048];
            seed.entries[0] =
                ArtifactEntry::from_canonical_decode(seed.entries[0].identity(), 2000);
            seed.relocations[0].destination_offset = 1992;
        }
    }
}

/// The exact checker verdict each declared lane must produce.
pub fn artifact_wire_custody_outcome(
    field: ArtifactWireFieldForTest,
) -> MutationOutcome<ContainerWireRejection> {
    use ArtifactWireFieldForTest as Lane;
    use ContainerWireRejection::{Construction, Encoding, Replay};
    let verdict = match field {
        Lane::ClaimedArtifactIdentity
        | Lane::Architecture
        | Lane::CodeBytes
        | Lane::CodeExtent
        | Lane::Contracts
        | Lane::DeclaredFootprint
        | Lane::PlacementPlan
        | Lane::PlacementAlignment
        | Lane::PlacementPhase
        | Lane::PlacementPermittedRange
        | Lane::PlacementMachineRegime
        | Lane::PlacementInstallationScope
        | Lane::PlacementMachineRegimeDropped
        | Lane::EntrySet
        | Lane::EntryIdentity
        | Lane::EntryCodeOffset
        | Lane::EntryRoster
        | Lane::RelocationSet
        | Lane::RelocationKind
        | Lane::RelocationDestination
        | Lane::RelocationTarget
        | Lane::RelocationAddend
        | Lane::RelocationRoster
        | Lane::AuthorityCommitments => Replay("different validated container"),
        Lane::CodeEmptied => Construction("cannot have empty content"),
        Lane::EntriesDropped => Construction("at least one selected entry"),
        Lane::EntriesDuplicated => Construction("must be unique"),
        Lane::EntryOutsideCode => Construction("lies outside"),
        Lane::ContractsWithoutRecommit
        | Lane::FootprintWithoutRecommit
        | Lane::RegimeDroppedWithoutRecommit => Construction("compact report coordinates"),
        Lane::RelocationRosterBeyondBound => Encoding("exceeding configured bound"),
        Lane::CodeBeyondSectionBound => Encoding("is empty or exceeds configured bound"),
    };
    MutationOutcome::ExactError(verdict)
}

/// Every verdict the artifact family declares, for the checker's
/// classification.
pub fn artifact_wire_vocabulary() -> impl Iterator<Item = ContainerWireRejection> {
    ArtifactWireFieldForTest::INVENTORY.iter().map(|&field| {
        match artifact_wire_custody_outcome(field) {
            MutationOutcome::ExactError(verdict) => verdict,
            MutationOutcome::RebuiltCustodyDiffers => {
                unreachable!("container wire lanes declare exact rejections")
            }
        }
    })
}
