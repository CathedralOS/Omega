//! Executable container encoding and validation tests.

use crate::artifacts::ArtifactAdmissionEvidence;
use crate::artifacts::ArtifactEntry;
use crate::artifacts::container::{
    ArtifactRelocationKind, ContainerLimits, ContainerSection, ContainerSectionKind,
    DecodedArtifactContainer, DecodedArtifactRelocation, OMEGA_EXECUTABLE_CONTAINER_MARKER,
    OMEGA_EXECUTABLE_CONTAINER_V1_MARKER, ValidatedArtifactContainer,
    ValidatedContainerAdmissionEvidence, admit_validated_container,
    non_authoritative_decoded_container_fingerprint, normalized_decoded_content_digest,
    normalized_proof_payload_digest, validate_decoded_container,
};
use crate::authority_digests::ArtifactAuthorityCommitments;
use crate::authority_digests::{
    AdmissionReceiptId, ArtifactId, EntrySetId, MachineContractSetId, MachineFootprintId,
    NonAuthoritativeContainerFingerprint64, NonAuthoritativeInformationalFingerprint64,
    PlacementPlanId, RelocationSetId,
};
use crate::executable_installation::admit_executable;
use crate::installation::InstallationDiagnostic;
use layout_plans::EntryStubId;
use layout_plans::{
    ArtifactInstallationScopeId, DataSymbolId, MachineRegimeId, PlacementAddressRange,
    PlacementPhase,
};
use layout_plans::{PlacementConstraints, RelocationTarget};
use target::Architecture;

fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, InstallationDiagnostic>) -> T {
    constructor(identity).expect("normalized identity")
}

fn limits() -> ContainerLimits {
    ContainerLimits {
        max_total_bytes: 4096,
        max_sections: 16,
        max_section_bytes: 1024,
        max_relocations: 16,
    }
}

fn decoded() -> DecodedArtifactContainer {
    let contracts = id(3, MachineContractSetId::from_normalized_identity);
    let footprint = id(4, MachineFootprintId::from_normalized_identity);
    let placement = id(5, PlacementPlanId::from_normalized_identity);
    let relocations = id(6, RelocationSetId::from_normalized_identity);
    let proof_bytes = vec![0xa5; 64];
    let proof = normalized_proof_payload_digest(&proof_bytes);
    let entry_set = id(8, EntrySetId::from_normalized_identity);
    let entry = EntryStubId::from_normalized_identity(9).expect("entry identity");
    let authority_commitments = ArtifactAuthorityCommitments::from_canonical_evidence(
        contracts,
        b"test imported contract set",
        footprint,
        b"test declared footprint",
        None,
        None,
    );
    let mut decoded = DecodedArtifactContainer {
        format_marker: OMEGA_EXECUTABLE_CONTAINER_MARKER,
        total_length: 576,
        artifact: id(1, ArtifactId::from_normalized_identity),
        content_fingerprint: NonAuthoritativeContainerFingerprint64::from_compatibility_value(2)
            .unwrap(),
        architecture: Architecture::X86_64,
        code_length: 64,
        code: vec![0x90; 64],
        contracts,
        declared_footprint: footprint,
        placement_plan: placement,
        placement_constraints: PlacementConstraints::unconstrained(
            layout_plans::PlacementPhase::Load,
        ),
        entry_set,
        entries: vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        relocation_set: relocations,
        relocations: vec![DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::X86Relative32,
            destination_offset: 32,
            target: RelocationTarget::Entry(entry),
            addend: -4,
        }],
        proof_payload: proof,
        proof: proof_bytes,
        authority_commitments: Some(authority_commitments),
        sections: vec![
            ContainerSection {
                kind: ContainerSectionKind::Code,
                offset: 0,
                length: 64,
            },
            ContainerSection {
                kind: ContainerSectionKind::Relocations(relocations),
                offset: 64,
                length: 40,
            },
            ContainerSection {
                kind: ContainerSectionKind::Contracts(contracts),
                offset: 144,
                length: 64,
            },
            ContainerSection {
                kind: ContainerSectionKind::Footprint(footprint),
                offset: 208,
                length: 64,
            },
            ContainerSection {
                kind: ContainerSectionKind::Placement(placement),
                offset: 272,
                length: 64,
            },
            ContainerSection {
                kind: ContainerSectionKind::Entries(entry_set),
                offset: 336,
                length: 16,
            },
            ContainerSection {
                kind: ContainerSectionKind::Proof(proof),
                offset: 384,
                length: 64,
            },
            ContainerSection {
                kind: ContainerSectionKind::AuthorityCommitments(authority_commitments),
                offset: 448,
                length: 128,
            },
        ],
    };
    decoded.content_fingerprint =
        non_authoritative_decoded_container_fingerprint(&decoded).expect("content fingerprint");
    decoded
}

#[test]
fn canonical_bounded_container_produces_only_an_artifact_candidate() {
    let entry = EntryStubId::from_normalized_identity(9).expect("entry identity");
    let container = validate_decoded_container(decoded(), limits()).expect("container");
    assert_eq!(container.artifact().identity().normalized_identity(), 1);
    assert_eq!(container.artifact().architecture(), Architecture::X86_64);
    assert_eq!(container.artifact().byte_length(), 64);
    assert_eq!(container.artifact().code(), &[0x90; 64]);
    assert_eq!(container.proof(), &[0xa5; 64]);
    assert_eq!(container.artifact().entries().len(), 1);
    assert_eq!(container.artifact().entries()[0].identity(), entry);
    assert_eq!(container.artifact().entries()[0].code_offset(), 16);
    assert_eq!(
        container.relocations(),
        &[DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::X86Relative32,
            destination_offset: 32,
            target: RelocationTarget::Entry(entry),
            addend: -4,
        }]
    );
    assert_eq!(
        container.artifact().placement_constraints(),
        PlacementConstraints::unconstrained(layout_plans::PlacementPhase::Load)
    );
}

#[test]
fn stale_container_marker_rejects() {
    let mut stale = decoded();
    stale.format_marker = u16::from_le_bytes(*b"NO");
    let error = validate_decoded_container(stale, limits()).expect_err("stale marker");
    assert!(
        error
            .0
            .contains("unsupported Omega executable container marker")
    );
}

#[test]
fn validated_container_admission_binds_the_exact_proof_payload() {
    let container = validate_decoded_container(decoded(), limits()).expect("container");
    let evidence = ValidatedContainerAdmissionEvidence::from_validator(
        id(70, AdmissionReceiptId::from_normalized_identity),
        &container,
        true,
    );
    let admitted = admit_validated_container(&container, evidence).expect("exact proof admission");
    assert_eq!(
        admitted.artifact().identity(),
        container.artifact().identity()
    );
    assert_eq!(
        admitted.admission(),
        id(70, AdmissionReceiptId::from_normalized_identity)
    );
    let retained = admitted
        .container_proof
        .as_ref()
        .expect("validated-container admission retains proof custody");
    assert_eq!(retained.digest, container.proof_payload());
    assert_eq!(retained.bytes, container.proof());

    let mut changed = decoded();
    changed.proof[0] ^= 1;
    changed.proof_payload = normalized_proof_payload_digest(&changed.proof);
    changed.sections[6].kind = ContainerSectionKind::Proof(changed.proof_payload);
    let changed = validate_decoded_container(changed, limits()).expect("changed proof");
    let replay = ValidatedContainerAdmissionEvidence::from_validator(
        id(71, AdmissionReceiptId::from_normalized_identity),
        &changed,
        true,
    );
    let error = admit_validated_container(&container, replay)
        .expect_err("acceptance for another proof payload must not replay");
    assert!(error.0.contains("different proof payload"));

    let changed_bytes = changed;
    assert_eq!(
        changed_bytes.artifact().content(),
        container.artifact().content(),
        "proof evidence remains outside executable promise identity"
    );
    let mut replay = ValidatedContainerAdmissionEvidence::from_validator(
        id(72, AdmissionReceiptId::from_normalized_identity),
        &changed_bytes,
        true,
    );
    replay.proof_payload = container.proof_payload;
    let error = admit_validated_container(&container, replay)
        .expect_err("acceptance for different proof bytes must not replay");
    assert!(error.0.contains("different exact proof bytes"));
}

#[test]
fn informational_sections_do_not_gain_admission_authority() {
    let baseline = validate_decoded_container(decoded(), limits()).expect("baseline");
    let evidence = ValidatedContainerAdmissionEvidence::from_validator(
        id(70, AdmissionReceiptId::from_normalized_identity),
        &baseline,
        true,
    );

    let mut decorated = decoded();
    decorated.total_length = 640;
    decorated.sections.push(ContainerSection {
        kind: ContainerSectionKind::Informational(
            NonAuthoritativeInformationalFingerprint64::from_compatibility_value(88).unwrap(),
        ),
        offset: 576,
        length: 64,
    });
    let decorated =
        validate_decoded_container(decorated, limits()).expect("informational decoration");
    admit_validated_container(&decorated, evidence)
        .expect("informational sections stay outside admission identity");
}

#[test]
fn content_identity_binds_the_instruction_set_architecture() {
    let mut changed = decoded();
    changed.relocations[0].kind = ArtifactRelocationKind::Absolute64;
    changed.content_fingerprint = non_authoritative_decoded_container_fingerprint(&changed)
        .expect("portable relocation fingerprint");
    changed.architecture = Architecture::Aarch64;

    let error =
        validate_decoded_container(changed, limits()).expect_err("architecture drift rejects");
    assert!(error.0.contains("content fingerprint"));
}

#[test]
fn aarch64_entry_offsets_are_instruction_aligned() {
    let entry = EntryStubId::from_normalized_identity(9).expect("entry identity");
    let mut aarch64 = decoded();
    aarch64.architecture = Architecture::Aarch64;
    aarch64.entries = vec![ArtifactEntry::from_canonical_decode(entry, 17)];
    aarch64.relocations[0].kind = ArtifactRelocationKind::Absolute64;
    aarch64.content_fingerprint =
        non_authoritative_decoded_container_fingerprint(&aarch64).expect("candidate fingerprint");

    let error =
        validate_decoded_container(aarch64, limits()).expect_err("unaligned AArch64 entry rejects");
    assert!(error.0.contains("not instruction-aligned"));

    let mut x86 = decoded();
    x86.entries = vec![ArtifactEntry::from_canonical_decode(entry, 17)];
    x86.content_fingerprint =
        non_authoritative_decoded_container_fingerprint(&x86).expect("x86 candidate fingerprint");
    validate_decoded_container(x86, limits()).expect("x86 permits byte-aligned entries");
}

#[test]
fn unknown_required_rejects_while_unknown_optional_is_informational() {
    let mut optional = decoded();
    optional.total_length = 640;
    optional.sections.push(ContainerSection {
        kind: ContainerSectionKind::Unknown {
            identity: 99,
            required: false,
        },
        offset: 576,
        length: 64,
    });
    let container = validate_decoded_container(optional, limits()).expect("optional information");
    assert_eq!(container.unknown_informational_sections(), &[99]);

    let mut required = decoded();
    required.total_length = 640;
    required.sections.push(ContainerSection {
        kind: ContainerSectionKind::Unknown {
            identity: 99,
            required: true,
        },
        offset: 576,
        length: 64,
    });
    let error = validate_decoded_container(required, limits()).expect_err("required unknown");
    assert!(error.0.contains("unknown required"));
}

#[test]
fn duplicate_missing_overlapping_and_out_of_bounds_sections_reject() {
    let mut duplicate = decoded();
    duplicate.total_length = 640;
    duplicate.sections.push(ContainerSection {
        kind: ContainerSectionKind::Code,
        offset: 576,
        length: 64,
    });
    let error = validate_decoded_container(duplicate, limits()).expect_err("duplicate code");
    assert!(error.0.contains("exactly one code"));

    let mut missing = decoded();
    missing.sections.remove(6);
    let error = validate_decoded_container(missing, limits()).expect_err("missing proof");
    assert!(error.0.contains("exactly one proof"));

    let mut overlapping = decoded();
    overlapping.sections[1].offset = 32;
    let error = validate_decoded_container(overlapping, limits()).expect_err("overlap");
    assert!(error.0.contains("overlap"));

    let mut outside = decoded();
    outside.sections[6].offset = 620;
    let error = validate_decoded_container(outside, limits()).expect_err("outside");
    assert!(error.0.contains("exceeds"));
}

#[test]
fn missing_duplicate_and_out_of_bounds_entries_reject() {
    let mut missing = decoded();
    missing.entries.clear();
    let error = validate_decoded_container(missing, limits()).expect_err("missing entry");
    assert!(error.0.contains("does not match 0 canonical entries"));

    let mut duplicate = decoded();
    duplicate.entries.push(duplicate.entries[0]);
    duplicate.sections[5].length = 32;
    duplicate.content_fingerprint = non_authoritative_decoded_container_fingerprint(&duplicate)
        .expect("duplicate content fingerprint");
    let error = validate_decoded_container(duplicate, limits()).expect_err("duplicate entry");
    assert!(error.0.contains("must be unique"));

    let mut outside = decoded();
    let identity = EntryStubId::from_normalized_identity(10).expect("entry identity");
    outside.entries = vec![ArtifactEntry::from_canonical_decode(identity, 64)];
    outside.content_fingerprint = non_authoritative_decoded_container_fingerprint(&outside)
        .expect("outside content fingerprint");
    let error = validate_decoded_container(outside, limits()).expect_err("outside entry");
    assert!(error.0.contains("lies outside"));

    let mut mismatched_length = decoded();
    mismatched_length.sections[5].length = 32;
    let error = validate_decoded_container(mismatched_length, limits())
        .expect_err("entry section length must bind its decoded records");
    assert!(error.0.contains("does not match 1 canonical entries"));
}

#[test]
fn relocation_records_are_bounded_canonical_and_non_overlapping() {
    let entry = EntryStubId::from_normalized_identity(9).expect("entry identity");
    let mut canonical = decoded();
    canonical.relocations = vec![
        DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::Absolute64,
            destination_offset: 48,
            target: RelocationTarget::Entry(entry),
            addend: 0,
        },
        DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::X86Relative32,
            destination_offset: 8,
            target: RelocationTarget::Entry(entry),
            addend: -4,
        },
    ];
    canonical.sections[1].length = 72;
    canonical.content_fingerprint = non_authoritative_decoded_container_fingerprint(&canonical)
        .expect("relocation content fingerprint");
    let validated = validate_decoded_container(canonical, limits()).expect("canonical relocations");
    assert_eq!(validated.relocations()[0].destination_offset, 8);
    assert_eq!(validated.relocations()[1].destination_offset, 48);

    let mut overlapping = decoded();
    overlapping.relocations = vec![
        DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::Absolute64,
            destination_offset: 8,
            target: RelocationTarget::Entry(entry),
            addend: 0,
        },
        DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::X86Relative32,
            destination_offset: 12,
            target: RelocationTarget::Entry(entry),
            addend: 0,
        },
    ];
    overlapping.sections[1].length = 72;
    let error = validate_decoded_container(overlapping, limits()).expect_err("overlap rejects");
    assert!(error.0.contains("overlaps"));

    let mut outside = decoded();
    outside.relocations[0].destination_offset = 62;
    let error = validate_decoded_container(outside, limits()).expect_err("outside rejects");
    assert!(error.0.contains("exceeds 64-byte code"));

    let mut too_many = decoded();
    let mut strict_limits = limits();
    strict_limits.max_relocations = 1;
    too_many.relocations.push(DecodedArtifactRelocation {
        kind: ArtifactRelocationKind::Aarch64Page21,
        destination_offset: 40,
        target: RelocationTarget::Entry(entry),
        addend: 0,
    });
    too_many.sections[1].length = 72;
    let error = validate_decoded_container(too_many, strict_limits).expect_err("bound rejects");
    assert!(error.0.contains("exceeding configured bound"));
}

#[test]
fn architecture_specific_relocations_reject_during_candidate_validation() {
    let mut aarch64_in_x86 = decoded();
    aarch64_in_x86.relocations[0].kind = ArtifactRelocationKind::Aarch64Branch26;
    let error = validate_decoded_container(aarch64_in_x86, limits())
        .expect_err("AArch64 relocation in x86 artifact rejects");
    assert!(error.0.contains("incompatible with architecture X86_64"));

    let mut x86_in_aarch64 = decoded();
    x86_in_aarch64.architecture = Architecture::Aarch64;
    let error = validate_decoded_container(x86_in_aarch64, limits())
        .expect_err("x86 relocation in AArch64 artifact rejects");
    assert!(error.0.contains("incompatible with architecture Aarch64"));

    let mut portable_absolute = decoded();
    portable_absolute.architecture = Architecture::Aarch64;
    portable_absolute.relocations[0].kind = ArtifactRelocationKind::Absolute64;
    portable_absolute.content_fingerprint =
        non_authoritative_decoded_container_fingerprint(&portable_absolute)
            .expect("absolute relocation is valid on AArch64");
    validate_decoded_container(portable_absolute, limits())
        .expect("absolute relocation remains architecture-neutral");
}

#[test]
fn content_identity_binds_code_and_normalized_semantics_not_evidence() {
    let baseline = decoded();
    let baseline_identity =
        normalized_decoded_content_digest(&baseline).expect("baseline strong digest");

    let mut changed_code = decoded();
    changed_code.code[0] ^= 1;
    let error = validate_decoded_container(changed_code, limits()).expect_err("code drift rejects");
    assert!(error.0.contains("content fingerprint"));

    let mut changed_relocation = decoded();
    changed_relocation.relocations[0].addend = 1;
    let error = validate_decoded_container(changed_relocation, limits())
        .expect_err("relocation drift rejects");
    assert!(error.0.contains("content fingerprint"));

    let mut reordered = decoded();
    let entry = EntryStubId::from_normalized_identity(10).expect("entry identity");
    reordered
        .entries
        .push(ArtifactEntry::from_canonical_decode(entry, 24));
    reordered.sections[5].length = 32;
    reordered.content_fingerprint =
        non_authoritative_decoded_container_fingerprint(&reordered).expect("two-entry fingerprint");
    let two_entry_identity =
        normalized_decoded_content_digest(&reordered).expect("two-entry strong digest");
    reordered.entries.reverse();
    let validated =
        validate_decoded_container(reordered, limits()).expect("entry order normalizes");
    assert_eq!(validated.artifact().content(), two_entry_identity);

    let mut changed_proof = decoded();
    changed_proof.proof[0] ^= 1;
    changed_proof.proof_payload = normalized_proof_payload_digest(&changed_proof.proof);
    changed_proof.sections[6].kind = ContainerSectionKind::Proof(changed_proof.proof_payload);
    let validated = validate_decoded_container(changed_proof, limits()).expect("proof is evidence");
    assert_eq!(validated.artifact().content(), baseline_identity);
}

/// Rebuild the strong authority commitments for the exact report coordinates
/// one mutated container carries. The commitment bytes are provider evidence;
/// this helper derives them honestly so the coordinate join stays canonical.
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

/// Honestly rewrite every copy of the strong commitments a mutated container
/// carries: the decoded field and the authority-commitment section kind are a
/// canonical join, so both must move together.
fn recommit(changed: &mut DecodedArtifactContainer) {
    let commitments = commitments_for(changed);
    changed.authority_commitments = Some(commitments);
    for section in &mut changed.sections {
        if let ContainerSectionKind::AuthorityCommitments(_) = section.kind {
            section.kind = ContainerSectionKind::AuthorityCommitments(commitments);
        }
    }
}

/// Honestly recompute the containing compatibility fingerprint over a mutated
/// candidate, then run the full canonical validation. A substitution that is
/// representable survives this and yields a different artifact.
fn reidentified(mut changed: DecodedArtifactContainer) -> ValidatedArtifactContainer {
    changed.content_fingerprint = non_authoritative_decoded_container_fingerprint(&changed)
        .expect("honest compatibility-fingerprint recompute");
    validate_decoded_container(changed, limits())
        .expect("representable substitution must remain canonical")
}

/// Replay the authentic container's admission evidence against a substituted
/// candidate. The evidence carries the exact original artifact, so every
/// substitution that produces a different artifact must reject.
fn replay_rejects(
    authentic: &ValidatedArtifactContainer,
    substituted: &ValidatedArtifactContainer,
    needle: &str,
) {
    let evidence = ValidatedContainerAdmissionEvidence::from_validator(
        id(70, AdmissionReceiptId::from_normalized_identity),
        authentic,
        true,
    );
    let error = admit_validated_container(substituted, evidence)
        .expect_err("independent replay must reject the substitution");
    assert!(
        error.0.contains(needle),
        "unexpected replay rejection: {}",
        error.0
    );
}

/// Recompute the fingerprint when the mutation still permits one, then assert
/// canonical validation rejects the substitution for the structural reason.
fn rejects_validation(changed: DecodedArtifactContainer, needle: &str) {
    let mut changed = changed;
    if let Ok(fingerprint) = non_authoritative_decoded_container_fingerprint(&changed) {
        changed.content_fingerprint = fingerprint;
    }
    let error = validate_decoded_container(changed, limits())
        .expect_err("non-representable substitution must reject");
    assert!(
        error.0.contains(needle),
        "unexpected validation rejection: {}",
        error.0
    );
}

#[test]
fn executable_container_rejects_every_one_field_substitution() {
    let authentic = validate_decoded_container(decoded(), limits()).expect("authentic container");

    // The claimed normalized artifact identity is bound only by the admission
    // evidence: it deliberately rides outside the content digest, so an
    // honestly reidentified substitution still replays as a different artifact.
    let mut changed = decoded();
    changed.artifact = id(2, ArtifactId::from_normalized_identity);
    let substituted = reidentified(changed);
    assert_eq!(
        substituted.artifact().content(),
        authentic.artifact().content(),
        "the claimed artifact identity rides outside the content digest",
    );
    assert_ne!(substituted.artifact(), authentic.artifact());
    replay_rejects(&authentic, &substituted, "different validated container");

    // Every content-bound field: substitute the field, honestly recompute the
    // derived directory/commitment joins and the compatibility fingerprint.
    // The substituted container validates canonically, carries a different
    // content identity, and independent admission replay rejects it.
    let axes: Vec<(&str, Box<dyn Fn(&mut DecodedArtifactContainer)>)> = vec![
        (
            "architecture",
            Box::new(|changed| {
                changed.architecture = Architecture::Aarch64;
                changed.relocations[0].kind = ArtifactRelocationKind::Absolute64;
            }),
        ),
        ("code bytes", Box::new(|changed| changed.code[0] ^= 1)),
        (
            "code extent",
            Box::new(|changed| {
                changed.code.truncate(32);
                changed.code_length = 32;
                changed.sections[0].length = 32;
                changed.relocations[0].destination_offset = 8;
            }),
        ),
        (
            "contracts",
            Box::new(|changed| {
                changed.contracts = id(13, MachineContractSetId::from_normalized_identity);
                changed.sections[2].kind = ContainerSectionKind::Contracts(changed.contracts);
                recommit(changed);
            }),
        ),
        (
            "declared footprint",
            Box::new(|changed| {
                changed.declared_footprint = id(14, MachineFootprintId::from_normalized_identity);
                changed.sections[3].kind =
                    ContainerSectionKind::Footprint(changed.declared_footprint);
                recommit(changed);
            }),
        ),
        (
            "placement plan",
            Box::new(|changed| {
                changed.placement_plan = id(15, PlacementPlanId::from_normalized_identity);
                changed.sections[4].kind = ContainerSectionKind::Placement(changed.placement_plan);
            }),
        ),
        (
            "placement phase",
            Box::new(|changed| {
                changed.placement_constraints =
                    PlacementConstraints::unconstrained(PlacementPhase::PostHandoff);
            }),
        ),
        (
            "placement alignment",
            Box::new(|changed| {
                changed.placement_constraints =
                    PlacementConstraints::new(None, 8, PlacementPhase::Load, None, None)
                        .expect("aligned placement constraints");
            }),
        ),
        (
            "placement permitted range",
            Box::new(|changed| {
                changed.placement_constraints = PlacementConstraints::new(
                    Some(PlacementAddressRange::new(0x1000, 0x2000).expect("permitted range")),
                    1,
                    PlacementPhase::Load,
                    None,
                    None,
                )
                .expect("ranged placement constraints");
            }),
        ),
        (
            "placement machine regime",
            Box::new(|changed| {
                let regime = MachineRegimeId::from_normalized_identity(10).expect("regime");
                changed.placement_constraints =
                    PlacementConstraints::new(None, 1, PlacementPhase::Load, Some(regime), None)
                        .expect("regime placement constraints");
                recommit(changed);
            }),
        ),
        (
            "placement installation scope",
            Box::new(|changed| {
                let scope =
                    ArtifactInstallationScopeId::from_normalized_identity(11).expect("scope");
                changed.placement_constraints =
                    PlacementConstraints::new(None, 1, PlacementPhase::Load, None, Some(scope))
                        .expect("scope placement constraints");
                recommit(changed);
            }),
        ),
        (
            "entry set",
            Box::new(|changed| {
                changed.entry_set = id(18, EntrySetId::from_normalized_identity);
                changed.sections[5].kind = ContainerSectionKind::Entries(changed.entry_set);
            }),
        ),
        (
            "entry identity",
            Box::new(|changed| {
                changed.entries[0] = ArtifactEntry::from_canonical_decode(
                    EntryStubId::from_normalized_identity(10).expect("entry identity"),
                    changed.entries[0].code_offset(),
                );
            }),
        ),
        (
            "entry code offset",
            Box::new(|changed| {
                changed.entries[0] =
                    ArtifactEntry::from_canonical_decode(changed.entries[0].identity(), 24);
            }),
        ),
        (
            "entry roster",
            Box::new(|changed| {
                changed.entries.push(ArtifactEntry::from_canonical_decode(
                    EntryStubId::from_normalized_identity(10).expect("entry identity"),
                    24,
                ));
                changed.sections[5].length = 32;
            }),
        ),
        (
            "relocation set",
            Box::new(|changed| {
                changed.relocation_set = id(16, RelocationSetId::from_normalized_identity);
                changed.sections[1].kind =
                    ContainerSectionKind::Relocations(changed.relocation_set);
            }),
        ),
        (
            "relocation kind",
            Box::new(|changed| {
                changed.relocations[0].kind = ArtifactRelocationKind::Absolute64;
            }),
        ),
        (
            "relocation destination",
            Box::new(|changed| {
                changed.relocations[0].destination_offset = 40;
            }),
        ),
        (
            "relocation target",
            Box::new(|changed| {
                changed.relocations[0].target = RelocationTarget::Data(
                    DataSymbolId::from_normalized_identity(12).expect("data symbol"),
                );
            }),
        ),
        (
            "relocation addend",
            Box::new(|changed| changed.relocations[0].addend = 7),
        ),
        (
            "relocation roster",
            Box::new(|changed| {
                changed.relocations.clear();
                changed.sections[1].length = 8;
            }),
        ),
        (
            "authority commitments",
            Box::new(|changed| {
                let forged = ArtifactAuthorityCommitments::from_canonical_evidence(
                    changed.contracts,
                    b"forged imported contract set",
                    changed.declared_footprint,
                    b"test declared footprint",
                    None,
                    None,
                );
                changed.authority_commitments = Some(forged);
                for section in &mut changed.sections {
                    if let ContainerSectionKind::AuthorityCommitments(_) = section.kind {
                        section.kind = ContainerSectionKind::AuthorityCommitments(forged);
                    }
                }
            }),
        ),
    ];
    for (name, mutate) in &axes {
        let mut changed = decoded();
        mutate(&mut changed);
        let substituted = reidentified(changed);
        assert_ne!(
            substituted.artifact().content(),
            authentic.artifact().content(),
            "the {name} substitution must change the normalized content digest",
        );
        assert_ne!(
            substituted.artifact(),
            authentic.artifact(),
            "the {name} substitution must produce a different artifact",
        );
        replay_rejects(&authentic, &substituted, "different validated container");
    }

    // The exact proof payload is admission evidence: it stays outside the
    // executable content identity yet replay still binds the exact bytes.
    let mut changed = decoded();
    changed.proof[0] ^= 1;
    changed.proof_payload = normalized_proof_payload_digest(&changed.proof);
    changed.sections[6].kind = ContainerSectionKind::Proof(changed.proof_payload);
    let substituted = reidentified(changed);
    assert_eq!(
        substituted.artifact().content(),
        authentic.artifact().content(),
        "proof bytes remain outside executable content identity",
    );
    replay_rejects(&authentic, &substituted, "different proof payload");

    // Entry and relocation rosters canonicalize by sort: a permuted honest
    // roster replays to the identical artifact identity, and the identical
    // artifact still cannot borrow the original admission evidence unless the
    // roster itself is unchanged.
    let extra_relocation = DecodedArtifactRelocation {
        kind: ArtifactRelocationKind::Absolute64,
        destination_offset: 8,
        target: RelocationTarget::Data(
            DataSymbolId::from_normalized_identity(12).expect("data symbol"),
        ),
        addend: 0,
    };
    let mut forward = decoded();
    forward.relocations.push(extra_relocation);
    forward.sections[1].length = 72;
    let forward = reidentified(forward);
    let mut permuted = decoded();
    permuted.relocations.push(extra_relocation);
    permuted.relocations.swap(0, 1);
    permuted.sections[1].length = 72;
    let permuted = reidentified(permuted);
    assert_eq!(
        permuted.artifact().content(),
        forward.artifact().content(),
        "relocation roster order must canonicalize",
    );
    assert_eq!(permuted.artifact(), forward.artifact());
    replay_rejects(&authentic, &permuted, "different validated container");

    let mut forward = decoded();
    forward.entries.push(ArtifactEntry::from_canonical_decode(
        EntryStubId::from_normalized_identity(10).expect("entry identity"),
        24,
    ));
    forward.sections[5].length = 32;
    let forward = reidentified(forward);
    let mut permuted = decoded();
    permuted.entries.push(ArtifactEntry::from_canonical_decode(
        EntryStubId::from_normalized_identity(10).expect("entry identity"),
        24,
    ));
    permuted.entries.swap(0, 1);
    permuted.sections[5].length = 32;
    let permuted = reidentified(permuted);
    assert_eq!(
        permuted.artifact().content(),
        forward.artifact().content(),
        "entry roster order must canonicalize",
    );
    assert_eq!(permuted.artifact(), forward.artifact());
    replay_rejects(&authentic, &permuted, "different validated container");

    // Envelope axes the artifact identity deliberately does not carry: the
    // declared total length, the section roster order, and the payload
    // coordinates are wire-canonical joins, so a consistent substitution
    // replays to the identical artifact and the original evidence still admits
    // it — exactly like informational sections.
    let mut reordered = decoded();
    reordered.sections.swap(2, 3);
    let substituted = reidentified(reordered);
    assert_eq!(substituted.artifact(), authentic.artifact());
    let evidence = ValidatedContainerAdmissionEvidence::from_validator(
        id(70, AdmissionReceiptId::from_normalized_identity),
        &authentic,
        true,
    );
    admit_validated_container(&substituted, evidence)
        .expect("section roster order is wire-canonical, not artifact content");

    let mut inflated = decoded();
    inflated.total_length = 640;
    let substituted = reidentified(inflated);
    assert_eq!(substituted.artifact(), authentic.artifact());
    let evidence = ValidatedContainerAdmissionEvidence::from_validator(
        id(70, AdmissionReceiptId::from_normalized_identity),
        &authentic,
        true,
    );
    admit_validated_container(&substituted, evidence)
        .expect("declared length beyond the joined sections is envelope data");

    let mut moved = decoded();
    moved.sections[2].offset = 208;
    moved.sections[3].offset = 144;
    let substituted = reidentified(moved);
    assert_eq!(substituted.artifact(), authentic.artifact());
    let evidence = ValidatedContainerAdmissionEvidence::from_validator(
        id(70, AdmissionReceiptId::from_normalized_identity),
        &authentic,
        true,
    );
    admit_validated_container(&substituted, evidence)
        .expect("payload coordinates are wire-canonical, not artifact content");

    // A consistent version-1 encoding still decodes for compatibility but its
    // own honest evidence cannot admit it — the marker axis fails closed.
    let mut version_one = decoded();
    version_one.format_marker = OMEGA_EXECUTABLE_CONTAINER_V1_MARKER;
    version_one.authority_commitments = None;
    version_one.sections.pop();
    let substituted = reidentified(version_one);
    let evidence = ValidatedContainerAdmissionEvidence::from_validator(
        id(70, AdmissionReceiptId::from_normalized_identity),
        &substituted,
        true,
    );
    let error = admit_validated_container(&substituted, evidence)
        .expect_err("version-1 compatibility candidate cannot be admitted");
    assert!(
        error.0.contains("container-v1 compatibility"),
        "unexpected v1 rejection: {}",
        error.0
    );

    // The admission gate itself binds the acceptance flag and the exact
    // artifact, independent of the container envelope.
    let mut evidence = ArtifactAdmissionEvidence::from_validator(
        id(70, AdmissionReceiptId::from_normalized_identity),
        authentic.artifact(),
        true,
    );
    evidence.accepted = false;
    let error = admit_executable(authentic.artifact(), evidence)
        .expect_err("unaccepted evidence must not admit");
    assert!(error.0.contains("did not accept"));
    let mut colliding = decoded();
    colliding.artifact = id(2, ArtifactId::from_normalized_identity);
    let colliding = reidentified(colliding);
    let evidence = ArtifactAdmissionEvidence::from_validator(
        id(70, AdmissionReceiptId::from_normalized_identity),
        authentic.artifact(),
        true,
    );
    let error = admit_executable(colliding.artifact(), evidence)
        .expect_err("evidence for another artifact must not admit");
    assert!(error.0.contains("does not match canonical candidate"));

    // Zero is never a representable normalized identity: every identity
    // constructor rejects it before any substitution can be encoded.
    assert!(ArtifactId::from_normalized_identity(0).is_err());
    assert!(MachineContractSetId::from_normalized_identity(0).is_err());
    assert!(MachineFootprintId::from_normalized_identity(0).is_err());
    assert!(PlacementPlanId::from_normalized_identity(0).is_err());
    assert!(EntrySetId::from_normalized_identity(0).is_err());
    assert!(RelocationSetId::from_normalized_identity(0).is_err());
    assert!(AdmissionReceiptId::from_normalized_identity(0).is_err());
    assert!(EntryStubId::from_normalized_identity(0).is_err());
    assert!(DataSymbolId::from_normalized_identity(0).is_err());
    assert!(MachineRegimeId::from_normalized_identity(0).is_err());
    assert!(ArtifactInstallationScopeId::from_normalized_identity(0).is_err());
    assert!(NonAuthoritativeContainerFingerprint64::from_compatibility_value(0).is_err());

    // Substitutions that cannot keep the canonical joins reject at validation
    // even under an honest compatibility-fingerprint recompute.
    let mut changed = decoded();
    changed.format_marker = 0x9999;
    rejects_validation(changed, "unsupported Omega executable container marker");

    let mut changed = decoded();
    changed.format_marker = OMEGA_EXECUTABLE_CONTAINER_V1_MARKER;
    rejects_validation(
        changed,
        "container-v1 cannot carry v2 authority commitments",
    );

    let mut changed = decoded();
    changed.code_length = 32;
    rejects_validation(changed, "canonical header declares");

    let mut changed = decoded();
    changed.total_length = 512;
    rejects_validation(changed, "exceeds 512-byte container");

    let mut changed = decoded();
    changed.sections[1].kind =
        ContainerSectionKind::Relocations(id(99, RelocationSetId::from_normalized_identity));
    rejects_validation(
        changed,
        "relocation section identity does not match canonical header",
    );

    let mut changed = decoded();
    changed.sections[6].kind = ContainerSectionKind::Proof(normalized_proof_payload_digest(b"x"));
    rejects_validation(
        changed,
        "proof section identity does not match canonical header",
    );

    let mut changed = decoded();
    changed.contracts = id(13, MachineContractSetId::from_normalized_identity);
    rejects_validation(
        changed,
        "contract section identity does not match canonical header",
    );

    let mut changed = decoded();
    changed.contracts = id(13, MachineContractSetId::from_normalized_identity);
    changed.sections[2].kind = ContainerSectionKind::Contracts(changed.contracts);
    rejects_validation(changed, "compact report coordinates");

    let mut changed = decoded();
    changed.declared_footprint = id(14, MachineFootprintId::from_normalized_identity);
    changed.sections[3].kind = ContainerSectionKind::Footprint(changed.declared_footprint);
    rejects_validation(changed, "compact report coordinates");

    let mut changed = decoded();
    changed.placement_constraints = PlacementConstraints::new(
        None,
        1,
        PlacementPhase::Load,
        Some(MachineRegimeId::from_normalized_identity(10).expect("regime")),
        None,
    )
    .expect("regime placement constraints");
    rejects_validation(changed, "compact report coordinates");

    let mut changed = decoded();
    changed.placement_constraints = PlacementConstraints::new(
        None,
        1,
        PlacementPhase::Load,
        None,
        Some(ArtifactInstallationScopeId::from_normalized_identity(11).expect("scope")),
    )
    .expect("scope placement constraints");
    rejects_validation(changed, "compact report coordinates");

    let mut changed = decoded();
    changed.authority_commitments = None;
    rejects_validation(changed, "does not match decoded strong evidence");

    let mut changed = decoded();
    changed.authority_commitments = None;
    changed.sections.pop();
    rejects_validation(
        changed,
        "container-v2 requires exactly one authority-commitment section",
    );

    let mut changed = decoded();
    changed.sections.remove(2);
    rejects_validation(changed, "exactly one contracts section");

    let mut changed = decoded();
    changed.entries.clear();
    rejects_validation(changed, "does not match 0 canonical entries");

    let mut changed = decoded();
    changed.entries.clear();
    changed.sections[5].length = 0;
    rejects_validation(changed, "is empty or exceeds configured bound");

    let mut changed = decoded();
    changed.entries.push(changed.entries[0]);
    changed.sections[5].length = 32;
    rejects_validation(changed, "must be unique");

    let mut changed = decoded();
    changed.entries[0] = ArtifactEntry::from_canonical_decode(changed.entries[0].identity(), 64);
    rejects_validation(changed, "lies outside");

    let mut changed = decoded();
    changed.architecture = Architecture::Aarch64;
    changed.relocations[0].kind = ArtifactRelocationKind::Absolute64;
    changed.entries[0] = ArtifactEntry::from_canonical_decode(changed.entries[0].identity(), 17);
    rejects_validation(changed, "not instruction-aligned");

    let mut changed = decoded();
    changed.relocations[0].destination_offset = 62;
    rejects_validation(changed, "exceeds 64-byte code");

    let mut changed = decoded();
    changed.relocations[0].kind = ArtifactRelocationKind::Aarch64Branch26;
    rejects_validation(changed, "incompatible with architecture X86_64");

    let mut changed = decoded();
    changed.relocations.push(changed.relocations[0]);
    changed.sections[1].length = 72;
    rejects_validation(changed, "overlaps");

    let mut changed = decoded();
    changed.proof_payload = normalized_proof_payload_digest(b"different proof");
    rejects_validation(changed, "does not match the exact proof bytes");

    // A fingerprint drift that no honest recompute produces rejects the same
    // canonical check.
    let mut changed = decoded();
    changed.code[0] ^= 1;
    let error = validate_decoded_container(changed, limits())
        .expect_err("a stale compatibility fingerprint must reject");
    assert!(error.0.contains("content fingerprint"));
}
