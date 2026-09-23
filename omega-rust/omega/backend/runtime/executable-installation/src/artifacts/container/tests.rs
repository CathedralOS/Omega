//! Executable container encoding and validation tests.

mod executable_container_custody_fields;

use std::cell::Cell;

use executable_container_custody_fields::{
    ExecutableContainerCustodyCheck, ExecutableContainerFieldForTest, classify_container_replay,
    classify_container_validation, executable_container_custody_outcome, foreign_container_donor,
    substitute_executable_container_for_test,
};

use crate::artifacts::ArtifactAdmissionEvidence;
use crate::artifacts::ArtifactEntry;
use crate::artifacts::container::{
    ArtifactRelocationKind, ContainerLimits, ContainerSection, ContainerSectionKind,
    DecodedArtifactContainer, DecodedArtifactRelocation, OMEGA_EXECUTABLE_CONTAINER_MARKER,
    ValidatedArtifactContainer, ValidatedContainerAdmissionEvidence, admit_validated_container,
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
use layout_plans::{ArtifactInstallationScopeId, DataSymbolId, MachineRegimeId};
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

/// Honestly recompute the containing compatibility fingerprint over a mutated
/// candidate, then run the full canonical validation. A substitution that is
/// representable survives this and yields a different artifact.
fn reidentified(mut changed: DecodedArtifactContainer) -> ValidatedArtifactContainer {
    changed.content_fingerprint = non_authoritative_decoded_container_fingerprint(&changed)
        .expect("honest compatibility-fingerprint recompute");
    validate_decoded_container(changed, limits())
        .expect("representable substitution must remain canonical")
}

/// Every declared lane of the `DecodedArtifactContainer` record — the claimed
/// artifact identity, content and placement axes, entry and relocation
/// rosters, the strong authority commitments, the exact proof payload, the
/// envelope directory axes canonicalization erases, and the roster-order axes
/// canonicalization sorts — is substituted independently through the shared
/// `run_one_field_substitution_matrix` driver. The inventory lives in
/// `tests/executable_container_custody_fields.rs`; the independent checker is
/// the canonical seam order the honest record already survives: honest
/// compatibility-fingerprint recompute, `validate_decoded_container`, then
/// `admit_validated_container` replaying the authentic evidence.
#[test]
fn executable_container_rejects_every_one_field_substitution() {
    let authentic = validate_decoded_container(decoded(), limits()).expect("authentic container");

    // The driver substitutes before every check, so `leg` always names the
    // lane an internal invariant failure belongs to.
    let leg = Cell::new(ExecutableContainerFieldForTest::ClaimedArtifactIdentity);
    // The family's independent checker: honestly recompute the containing
    // compatibility identity, run canonical validation, then replay the
    // authentic admission evidence. A validation refusal classifies into the
    // named canonical invariant; a replay refusal classifies into the
    // admission seam; an outright admission means the axis carried no
    // custody. The `Ok` arm is never constructed — every declared lane ends
    // at one of the three named verdicts.
    let check = |container: &DecodedArtifactContainer|
     -> Result<DecodedArtifactContainer, ExecutableContainerCustodyCheck> {
        let mut changed = container.clone();
        if let Ok(fingerprint) = non_authoritative_decoded_container_fingerprint(&changed) {
            changed.content_fingerprint = fingerprint;
        }
        let validated = match validate_decoded_container(changed, limits()) {
            Ok(validated) => validated,
            Err(error) => {
                return Err(classify_container_validation(&error));
            }
        };
        let evidence = ValidatedContainerAdmissionEvidence::from_validator(
            id(70, AdmissionReceiptId::from_normalized_identity),
            &authentic,
            true,
        );
        match admit_validated_container(&validated, evidence) {
            Err(error) => Err(classify_container_replay(&error)),
            Ok(_) => Err(ExecutableContainerCustodyCheck::EnvelopeInformational),
        }
    };
    // Per-leg assertions the uniform verdict cannot carry: the two axes
    // bound outside the content digest must leave it unmoved, roster order
    // must canonicalize to the identical artifact, and a consistent
    // version-1 candidate must fail closed even under its own evidence.
    let joined_replay = |container: &DecodedArtifactContainer, field| {
        match field {
            ExecutableContainerFieldForTest::ClaimedArtifactIdentity => {
                let substituted = reidentified(container.clone());
                assert_eq!(
                    substituted.artifact().content(),
                    authentic.artifact().content(),
                    "{:?}: the claimed artifact identity rides outside the content digest",
                    leg.get(),
                );
            }
            ExecutableContainerFieldForTest::ProofPayload => {
                let substituted = reidentified(container.clone());
                assert_eq!(
                    substituted.artifact().content(),
                    authentic.artifact().content(),
                    "{:?}: proof bytes remain outside executable content identity",
                    leg.get(),
                );
            }
            ExecutableContainerFieldForTest::RelocationRosterPermuted => {
                let mut canonical = container.clone();
                canonical.relocations.sort_unstable_by_key(|relocation| {
                    (
                        relocation.destination_offset,
                        relocation.kind,
                        relocation.target,
                        relocation.addend,
                    )
                });
                assert_eq!(
                    reidentified(canonical).artifact(),
                    reidentified(container.clone()).artifact(),
                    "{:?}: relocation roster order must canonicalize",
                    leg.get(),
                );
            }
            ExecutableContainerFieldForTest::EntryRosterPermuted => {
                let mut canonical = container.clone();
                canonical
                    .entries
                    .sort_unstable_by_key(|entry| (entry.identity(), entry.code_offset()));
                assert_eq!(
                    reidentified(canonical).artifact(),
                    reidentified(container.clone()).artifact(),
                    "{:?}: entry roster order must canonicalize",
                    leg.get(),
                );
            }
            ExecutableContainerFieldForTest::FormatMarkerV1 => {
                // A consistent version-1 encoding still decodes for
                // compatibility but its own honest evidence cannot admit
                // it — the marker axis fails closed.
                let substituted = reidentified(container.clone());
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
            }
            _ => {}
        }
    };

    optimization_core::run_one_field_substitution_matrix(
        &optimization_core::OneFieldSubstitutionMatrix {
            family: "executable artifact container",
            fields: ExecutableContainerFieldForTest::INVENTORY,
            honest: &decoded,
            donor: foreign_container_donor(),
            custody: &|container: &DecodedArtifactContainer| container.clone(),
            substitute: &|container, field, donor| {
                leg.set(field);
                substitute_executable_container_for_test(container, field, donor);
            },
            check: &check,
            outcome: &executable_container_custody_outcome,
            joined_replay: Some(&joined_replay),
        },
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

    // A fingerprint drift that no honest recompute produces rejects the same
    // canonical check.
    let mut changed = decoded();
    changed.code[0] ^= 1;
    let error = validate_decoded_container(changed, limits())
        .expect_err("a stale compatibility fingerprint must reject");
    assert!(error.0.contains("content fingerprint"));
}
