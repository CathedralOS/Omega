//! One-field mutation coverage for the flat native executable publication
//! receipt — the [`ExecutablePublicationReceipt`] family — and its containing
//! report custody.
//!
//! The receipt's producer is `CompileReport::publish_retained_native_artifact`,
//! which mints the containing identity as a three-stage SHA-256 chain from the
//! retained artifact's outputs, the published container bytes, and the
//! destination path: the certificate digest commits to the artifact identity,
//! the semantic and proof bytes, the target, the image-symbol digest, the
//! boundary-contract fingerprint, and the validation and inventory digests and
//! report fingerprints; the evidence digest commits to the artifact identity,
//! certificate, callback-placement fingerprint, inventory and validation
//! digests and fingerprints, and the container commitment; the installation
//! digest commits to the evidence digest, the callback-placement fingerprint,
//! the destination path, and the container commitment.
//!
//! Three replays see different layers of that chain:
//!
//! - `has_consistent_installation_identity` recomputes the evidence and
//!   installation digests from the receipt's own fields, so a substitution
//!   that leaves them stale is rejected by the receipt's own custody, the
//!   report-custody replay, and the admission-time `CompileReport::checked`
//!   construction;
//! - `replay_mint` below re-derives the certificate layer as well — the one
//!   place `boundary_contract_report_fingerprint` reaches the chain, so the
//!   only replay that sees a stale substitution of it;
//! - a substitution whose containing identity is honestly recomputed produces
//!   a different self-consistent record: the `installation_evidence_digest` a
//!   deployment journal pins as the published record identity diverges, and
//!   the report-level joins that bind the destination path and the container
//!   commitment to the package receipt still reject the substitution.
//!
//! The receipt has no wire encoding — construction is its only admitted form —
//! so every field is representable and the coverage splits between the stale
//! and honestly recomputed legs above.

use std::path::{Path, PathBuf};

use crate::executable_publication::{
    executable_container_digest, native_publication_certificate_digest,
    native_publication_evidence_digest,
};
use crate::package::{
    NativePackagePublicationReceipt, PackageComponentDigest, PackagePublicationComponent,
};
use crate::{
    CompileOutputKind, CompileReport, ExecutableInstallationEvidenceDigest,
    ExecutablePublicationReceipt, NativePublicationCertificateDigest,
    NativePublicationEvidenceDigest, executable_installation_evidence_digest,
};

/// The producer inputs a flat publication receipt commits to. The semantic
/// and proof bytes, the target, and the image-symbol digest never appear on
/// the receipt itself — the certificate digest commits to them — so the mint
/// replay holds them fixed while the receipt's claimed fields vary.
struct PublicationInputs {
    output_path: PathBuf,
    native_artifact_identity: [u8; 32],
    semantic_bytes: Vec<u8>,
    proof_bytes: Vec<u8>,
    target: target::NativeTarget,
    final_image_symbol_digest: image::FinalImageSymbolDigest,
    boundary_contract_report_fingerprint: Option<u64>,
    compiler_text_validation_digest: image::CompilerTextDerivationDigest,
    compiler_function_validation_digest: image::CompilerFunctionValidationDigest,
    compiler_function_validation_report_fingerprint: u64,
    inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
    callback_placement_identity_report_fingerprint: u64,
    container_bytes: Vec<u8>,
}

/// A real `CompilerFunctionValidationDigest` minted over an evidence record,
/// so a substitution carries a well-formed foreign value rather than noise.
fn function_validation_digest(
    final_text_validation_report_fingerprint: u64,
) -> image::CompilerFunctionValidationDigest {
    image::CompilerFunctionValidationEvidence {
        function_count: 1,
        instruction_count: 2,
        zero_width_instruction_count: 0,
        frame_prologue_byte_count: 4,
        frame_epilogue_byte_count: 8,
        fragment_manifest_report_fingerprint: 3,
        frame_application_report_fingerprint: 4,
        boundary_contract_report_fingerprint: Some(2),
        final_region_binding_report_fingerprint: 6,
        final_text_validation_report_fingerprint,
    }
    .evidence_digest()
}

/// One honest input set: the destination is the inner executable of the
/// `window-app` package tree so the same receipt binds a flat report and a
/// packaged one.
fn honest_inputs() -> PublicationInputs {
    let image = image::FinalImage::with_capacity(
        target::NativeTarget::linux_x64(),
        image::FinalImageMemory::default(),
        arena::Handle::invalid(),
        0,
        0,
        0,
    );
    PublicationInputs {
        output_path: "build/window-app.app/Contents/MacOS/window-app".into(),
        native_artifact_identity: [9; 32],
        semantic_bytes: b"canonical terminal semantics".to_vec(),
        proof_bytes: b"terminal proof bytes".to_vec(),
        target: target::NativeTarget::linux_x64(),
        final_image_symbol_digest: image::final_image_symbol_digest(&image),
        boundary_contract_report_fingerprint: Some(2),
        compiler_text_validation_digest: image::CompilerTextDerivationDigest::from_digest([3; 32]),
        compiler_function_validation_digest: function_validation_digest(7),
        compiler_function_validation_report_fingerprint: 4,
        inventory_digest: image::PlacedExecutableRegionInventoryDigest::from_digest([5; 32]),
        inventory_report_fingerprint: 6,
        callback_placement_identity_report_fingerprint: 8,
        container_bytes: b"the exact published executable container bytes".to_vec(),
    }
}

/// Mint the receipt exactly as `publish_retained_native_artifact` does:
/// certificate over the retained artifact's outputs, evidence over the
/// certificate and container commitment, installation over the evidence and
/// the destination.
fn mint(inputs: &PublicationInputs) -> ExecutablePublicationReceipt {
    let container_digest = executable_container_digest(&inputs.container_bytes);
    let certificate_digest = native_publication_certificate_digest(
        &inputs.native_artifact_identity,
        &inputs.semantic_bytes,
        &inputs.proof_bytes,
        inputs.target,
        inputs.final_image_symbol_digest,
        inputs.boundary_contract_report_fingerprint,
        inputs.compiler_text_validation_digest,
        inputs.compiler_function_validation_digest,
        inputs.compiler_function_validation_report_fingerprint,
        inputs.inventory_digest,
        inputs.inventory_report_fingerprint,
    );
    let publication_evidence_digest = native_publication_evidence_digest(
        &inputs.native_artifact_identity,
        certificate_digest,
        inputs.callback_placement_identity_report_fingerprint,
        inputs.inventory_digest,
        inputs.inventory_report_fingerprint,
        inputs.compiler_text_validation_digest,
        inputs.compiler_function_validation_digest,
        inputs.compiler_function_validation_report_fingerprint,
        inputs.container_bytes.len(),
        container_digest,
    );
    let installation_evidence_digest = executable_installation_evidence_digest(
        publication_evidence_digest,
        inputs.callback_placement_identity_report_fingerprint,
        &inputs.output_path,
        inputs.container_bytes.len(),
        container_digest,
    );
    ExecutablePublicationReceipt::new(
        inputs.output_path.clone(),
        inputs.native_artifact_identity,
        certificate_digest,
        inputs.callback_placement_identity_report_fingerprint,
        inputs.boundary_contract_report_fingerprint,
        inputs.inventory_digest,
        inputs.inventory_report_fingerprint,
        inputs.compiler_text_validation_digest,
        inputs.compiler_function_validation_digest,
        inputs.compiler_function_validation_report_fingerprint,
        publication_evidence_digest,
        inputs.container_bytes.len(),
        container_digest,
        installation_evidence_digest,
    )
}

/// Re-derive the certificate the receipt's claimed fields commit to, holding
/// the producer inputs the receipt does not carry — the artifact's semantic
/// and proof bytes, target, and image-symbol digest — at their retained
/// values. A claimed field the stored certificate was not minted over
/// disagrees here even when the downstream digests are honestly recomputed.
fn claimed_certificate(
    receipt: &ExecutablePublicationReceipt,
    inputs: &PublicationInputs,
) -> NativePublicationCertificateDigest {
    native_publication_certificate_digest(
        &receipt.native_artifact_identity,
        &inputs.semantic_bytes,
        &inputs.proof_bytes,
        inputs.target,
        inputs.final_image_symbol_digest,
        receipt.boundary_contract_report_fingerprint,
        receipt.compiler_text_validation_digest,
        receipt.compiler_function_validation_digest,
        receipt.compiler_function_validation_report_fingerprint,
        receipt.inventory_digest,
        receipt.inventory_report_fingerprint,
    )
}

/// Re-derive the evidence digest the receipt's claimed fields commit to.
fn claimed_evidence(receipt: &ExecutablePublicationReceipt) -> NativePublicationEvidenceDigest {
    native_publication_evidence_digest(
        &receipt.native_artifact_identity,
        receipt.certificate_digest,
        receipt.callback_placement_identity_report_fingerprint,
        receipt.inventory_digest,
        receipt.inventory_report_fingerprint,
        receipt.compiler_text_validation_digest,
        receipt.compiler_function_validation_digest,
        receipt.compiler_function_validation_report_fingerprint,
        receipt.container_byte_count,
        receipt.container_digest,
    )
}

/// Re-derive the installation digest the receipt's claimed fields commit to.
fn claimed_installation(
    receipt: &ExecutablePublicationReceipt,
) -> ExecutableInstallationEvidenceDigest {
    executable_installation_evidence_digest(
        receipt.publication_evidence_digest,
        receipt.callback_placement_identity_report_fingerprint,
        &receipt.output_path,
        receipt.container_byte_count,
        receipt.container_digest,
    )
}

/// Replay the producer mint over the receipt's claimed fields: every stored
/// digest must equal the digest the claimed inputs produce. This checks one
/// layer deeper than `has_consistent_installation_identity`, which recomputes
/// the evidence and installation digests but cannot see inside the
/// certificate.
fn replay_mint(receipt: &ExecutablePublicationReceipt, inputs: &PublicationInputs) -> bool {
    claimed_certificate(receipt, inputs) == receipt.certificate_digest
        && claimed_evidence(receipt) == receipt.publication_evidence_digest
        && claimed_installation(receipt) == receipt.installation_evidence_digest
}

/// Honestly recompute the containing identity after a substitution, exactly
/// as a mutated-but-self-consistent record must: certificate over the claimed
/// inputs, then evidence, then installation. Callers that substitute the
/// certificate digest itself keep their foreign claim and recompute only the
/// downstream stages.
fn recompute_containing_identity(
    receipt: &mut ExecutablePublicationReceipt,
    inputs: &PublicationInputs,
) {
    receipt.certificate_digest = claimed_certificate(receipt, inputs);
    receipt.publication_evidence_digest = claimed_evidence(receipt);
    receipt.installation_evidence_digest = claimed_installation(receipt);
}

fn report(
    wrote_output: bool,
    output_kind: CompileOutputKind,
    flat: Option<ExecutablePublicationReceipt>,
) -> CompileReport {
    CompileReport {
        root_path: "Main/main.omg".into(),
        source_file_count: 1,
        wrote_output,
        output_kind,
        retained_native_artifact: None,
        artifact: None,
        executable_publication: flat,
        optimization_rollback: None,
        production_manifest: None,
        trust_admission_settlement: Default::default(),
        pcc_requests: build_evaluation::PccRequests::default(),
        terminal_admission_profile: proof_admission::AdmissionProfile::default(),
        pcc_publications: Vec::new(),
        application_name: None,
        application_intent: None,
        application_identifier: None,
        package_publication: None,
    }
}

/// A packaged publication report: the flat receipt joined to the package
/// receipt the authentic publication produced — the authentic package root,
/// name, identifier, and container commitment — plus the retained application
/// metadata.
fn packaged_report(
    flat: &ExecutablePublicationReceipt,
    authentic: &ExecutablePublicationReceipt,
) -> CompileReport {
    let mut report = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    report.package_publication = Some(NativePackagePublicationReceipt::new(
        PathBuf::from("build/window-app.app"),
        "window-app".to_owned(),
        build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app")
            .expect("valid identifier"),
        authentic.container_digest,
        authentic.container_byte_count,
        vec![
            PackagePublicationComponent {
                relative_path: Path::new("Contents").join("Info.plist"),
                byte_count: 4,
                digest: PackageComponentDigest::from_digest([3; 32]),
            },
            PackagePublicationComponent {
                relative_path: Path::new("Contents").join("MacOS").join("window-app"),
                byte_count: authentic.container_byte_count,
                digest: PackageComponentDigest::from_digest([7; 32]),
            },
        ],
    ));
    report.application_name = Some("window-app".to_owned());
    report.application_identifier = Some(
        build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app")
            .expect("valid identifier"),
    );
    report.application_intent = Some(build_evaluation::HostedApplicationIntent::Gui);
    report
}

#[test]
fn executable_publication_receipt_rejects_every_one_field_substitution() {
    let inputs = honest_inputs();
    let authentic = mint(&inputs);
    assert!(
        authentic.has_consistent_installation_identity(),
        "the producer mint is self-consistent"
    );
    assert!(
        replay_mint(&authentic, &inputs),
        "the producer mint replays over the receipt's claimed fields"
    );
    let flat_report = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(authentic.clone()),
    );
    assert!(flat_report.has_consistent_executable_publication_custody());
    assert_eq!(
        flat_report.checked_native_executable_path(),
        Some(inputs.output_path.as_path())
    );
    let packaged = packaged_report(&authentic, &authentic);
    assert!(packaged.has_consistent_executable_publication_custody());
    assert_eq!(
        packaged.checked_native_package_path(),
        Some(Path::new("build/window-app.app"))
    );

    // A substitution that leaves the containing digests stale is rejected by
    // the receipt's own recomputation, the producer mint replay, the report
    // custody join, and the admission-time construction.
    let rejects_stale = |name: &'static str, mutate: &dyn Fn(&mut ExecutablePublicationReceipt)| {
        let mut changed = authentic.clone();
        mutate(&mut changed);
        assert_ne!(
            changed, authentic,
            "{name}: substitution changes the receipt"
        );
        assert!(
            !changed.has_consistent_installation_identity(),
            "{name}: the receipt's digest recomputation rejects a stale containing identity"
        );
        assert!(
            !replay_mint(&changed, &inputs),
            "{name}: the producer mint replay rejects"
        );
        let stale = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(changed.clone()),
        );
        assert!(
            !stale.has_consistent_executable_publication_custody(),
            "{name}: report custody rejects"
        );
        assert!(
            stale.checked_native_executable_path().is_none(),
            "{name}: no checked path escapes"
        );
        assert!(
            CompileReport::checked(
                "Main/main.omg".into(),
                1,
                true,
                CompileOutputKind::NativeExecutable,
                Some(changed),
            )
            .is_err(),
            "{name}: admission construction rejects"
        );
    };

    // A substitution whose containing identity is honestly recomputed is a
    // different self-consistent record — representable and admitted — but its
    // installation identity diverges from the published record a deployment
    // journal pins, so the journal replay still rejects it.
    let rejects_recomputed =
        |name: &'static str,
         mutate: &dyn Fn(&mut ExecutablePublicationReceipt, &PublicationInputs)| {
            let mut changed = authentic.clone();
            mutate(&mut changed, &inputs);
            assert_ne!(
                changed, authentic,
                "{name}: substitution changes the receipt"
            );
            assert!(
                changed.has_consistent_installation_identity(),
                "{name}: the honestly recomputed containing identity stays self-consistent"
            );
            assert!(
                replay_mint(&changed, &inputs),
                "{name}: the recomputed record replays as a self-consistent foreign publication"
            );
            assert_ne!(
                changed.installation_evidence_digest, authentic.installation_evidence_digest,
                "{name}: the published record identity a deployment journal replays diverges"
            );
            assert!(
                CompileReport::checked(
                    "Main/main.omg".into(),
                    1,
                    true,
                    CompileOutputKind::NativeExecutable,
                    Some(changed),
                )
                .is_ok(),
                "{name}: the self-consistent foreign record is admitted as a different publication"
            );
        };

    // --- the destination path: installation layer only ---

    rejects_stale("a substituted output path", &|receipt| {
        receipt.output_path = "build/window-app.app/Contents/MacOS/renamed".into();
    });
    rejects_recomputed(
        "a substituted output path honestly reinstalled",
        &|receipt, _| {
            receipt.output_path = "build/window-app.app/Contents/MacOS/renamed".into();
            receipt.installation_evidence_digest = claimed_installation(receipt);
        },
    );
    // A flat report cannot see the drift: the self-consistent foreign receipt
    // exposes its claimed path — the destination is adopted verbatim and the
    // pinned record identity is the only flat-publication authority over it.
    let mut changed_path = authentic.clone();
    changed_path.output_path = "build/window-app.app/Contents/MacOS/renamed".into();
    changed_path.installation_evidence_digest = claimed_installation(&changed_path);
    let adopted = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(changed_path.clone()),
    );
    assert!(adopted.has_consistent_executable_publication_custody());
    assert_eq!(
        adopted.checked_native_executable_path(),
        Some(Path::new("build/window-app.app/Contents/MacOS/renamed")),
        "the flat receipt's claimed destination is adopted verbatim"
    );
    // The packaged join replays the destination it actually installed: the
    // package receipt's inner executable path still names the authentic leaf.
    assert!(
        !packaged_report(&changed_path, &authentic).has_consistent_executable_publication_custody(),
        "the package/executable path join rejects the substitution"
    );

    // --- the artifact identity: certificate and evidence layers ---

    rejects_stale("a substituted native artifact identity", &|receipt| {
        receipt.native_artifact_identity[0] ^= 0xff;
    });
    rejects_recomputed(
        "a substituted native artifact identity honestly recertified",
        &|receipt, inputs| {
            receipt.native_artifact_identity[0] ^= 0xff;
            recompute_containing_identity(receipt, inputs);
        },
    );
    // A retained production manifest additionally binds the artifact identity
    // at the NativeExecutable join; the mint replay already rejects here.

    // --- the certificate digest: claimed, evidence-committed ---

    rejects_stale("a substituted certificate digest", &|receipt| {
        receipt.certificate_digest = NativePublicationCertificateDigest::from_digest([0x55; 32]);
    });
    // Honestly recomputing the downstream chain over a foreign certificate
    // keeps the receipt self-consistent, but the certificate layer replays
    // the claimed inputs and disagrees — the foreign digest was never minted
    // over them.
    let mut foreign_certificate = authentic.clone();
    foreign_certificate.certificate_digest =
        NativePublicationCertificateDigest::from_digest([0x55; 32]);
    foreign_certificate.publication_evidence_digest = claimed_evidence(&foreign_certificate);
    foreign_certificate.installation_evidence_digest = claimed_installation(&foreign_certificate);
    assert!(
        foreign_certificate.has_consistent_installation_identity(),
        "a foreign certificate under an honestly recomputed downstream chain stays self-consistent"
    );
    assert_ne!(
        claimed_certificate(&foreign_certificate, &inputs),
        foreign_certificate.certificate_digest,
        "the certificate layer replays the claimed inputs and disagrees"
    );
    assert!(!replay_mint(&foreign_certificate, &inputs));
    assert_ne!(
        foreign_certificate.installation_evidence_digest, authentic.installation_evidence_digest,
        "the published record identity a deployment journal replays diverges"
    );

    // --- the compact report coordinates ---

    rejects_stale("a substituted callback-placement fingerprint", &|receipt| {
        receipt.callback_placement_identity_report_fingerprint ^= 1;
    });
    rejects_recomputed(
        "a substituted callback-placement fingerprint honestly reinstalled",
        &|receipt, _| {
            receipt.callback_placement_identity_report_fingerprint ^= 1;
            receipt.publication_evidence_digest = claimed_evidence(receipt);
            receipt.installation_evidence_digest = claimed_installation(receipt);
        },
    );

    // `boundary_contract_report_fingerprint` reaches the chain only through
    // the certificate digest: a stale substitution is invisible to the
    // receipt's own recomputation — the record identity itself is unchanged —
    // and the certificate-layer mint replay is what rejects it.
    for (name, fingerprint) in [
        ("a dropped boundary-contract fingerprint", None),
        ("a substituted boundary-contract fingerprint", Some(9)),
    ] {
        let mut changed = authentic.clone();
        changed.boundary_contract_report_fingerprint = fingerprint;
        assert_ne!(
            changed, authentic,
            "{name}: substitution changes the receipt"
        );
        assert!(
            changed.has_consistent_installation_identity(),
            "{name}: the field is outside the evidence and installation recomputations"
        );
        assert_eq!(
            changed.installation_evidence_digest, authentic.installation_evidence_digest,
            "{name}: the record identity is unchanged by the transitive claim"
        );
        assert_ne!(
            claimed_certificate(&changed, &inputs),
            changed.certificate_digest,
            "{name}: the stored certificate was minted over the authentic fingerprint"
        );
        assert!(
            !replay_mint(&changed, &inputs),
            "{name}: the certificate-layer mint replay rejects"
        );
    }
    rejects_recomputed(
        "a dropped boundary-contract fingerprint honestly recertified",
        &|receipt, inputs| {
            receipt.boundary_contract_report_fingerprint = None;
            recompute_containing_identity(receipt, inputs);
        },
    );
    rejects_recomputed(
        "a substituted boundary-contract fingerprint honestly recertified",
        &|receipt, inputs| {
            receipt.boundary_contract_report_fingerprint = Some(9);
            recompute_containing_identity(receipt, inputs);
        },
    );

    // --- the inventory and validation digests and fingerprints: certificate
    // and evidence layers ---

    rejects_stale("a substituted inventory digest", &|receipt| {
        receipt.inventory_digest =
            image::PlacedExecutableRegionInventoryDigest::from_digest([0x55; 32]);
    });
    rejects_recomputed(
        "a substituted inventory digest honestly recertified",
        &|receipt, inputs| {
            receipt.inventory_digest =
                image::PlacedExecutableRegionInventoryDigest::from_digest([0x55; 32]);
            recompute_containing_identity(receipt, inputs);
        },
    );

    rejects_stale("a substituted inventory fingerprint", &|receipt| {
        receipt.inventory_report_fingerprint ^= 1;
    });
    rejects_recomputed(
        "a substituted inventory fingerprint honestly recertified",
        &|receipt, inputs| {
            receipt.inventory_report_fingerprint ^= 1;
            recompute_containing_identity(receipt, inputs);
        },
    );

    rejects_stale("a substituted text-validation digest", &|receipt| {
        receipt.compiler_text_validation_digest =
            image::CompilerTextDerivationDigest::from_digest([0x55; 32]);
    });
    rejects_recomputed(
        "a substituted text-validation digest honestly recertified",
        &|receipt, inputs| {
            receipt.compiler_text_validation_digest =
                image::CompilerTextDerivationDigest::from_digest([0x55; 32]);
            recompute_containing_identity(receipt, inputs);
        },
    );

    rejects_stale("a substituted function-validation digest", &|receipt| {
        receipt.compiler_function_validation_digest = function_validation_digest(99);
    });
    rejects_recomputed(
        "a substituted function-validation digest honestly recertified",
        &|receipt, inputs| {
            receipt.compiler_function_validation_digest = function_validation_digest(99);
            recompute_containing_identity(receipt, inputs);
        },
    );

    rejects_stale(
        "a substituted function-validation fingerprint",
        &|receipt| {
            receipt.compiler_function_validation_report_fingerprint ^= 1;
        },
    );
    rejects_recomputed(
        "a substituted function-validation fingerprint honestly recertified",
        &|receipt, inputs| {
            receipt.compiler_function_validation_report_fingerprint ^= 1;
            recompute_containing_identity(receipt, inputs);
        },
    );

    // --- the container commitment: evidence and installation layers, and the
    // package cross-binding ---

    rejects_stale("a substituted container byte count", &|receipt| {
        receipt.container_byte_count += 1;
    });
    rejects_recomputed(
        "a substituted container byte count honestly reinstalled",
        &|receipt, _| {
            receipt.container_byte_count += 1;
            receipt.publication_evidence_digest = claimed_evidence(receipt);
            receipt.installation_evidence_digest = claimed_installation(receipt);
        },
    );

    rejects_stale("a substituted container digest", &|receipt| {
        receipt.container_digest = crate::ExecutableContainerDigest::from_digest([0x55; 32]);
    });
    rejects_recomputed(
        "a substituted container digest honestly reinstalled",
        &|receipt, _| {
            receipt.container_digest = crate::ExecutableContainerDigest::from_digest([0x55; 32]);
            receipt.publication_evidence_digest = claimed_evidence(receipt);
            receipt.installation_evidence_digest = claimed_installation(receipt);
        },
    );

    // A coupled substitution keeps the container fields mutually consistent
    // with foreign bytes: still self-consistent under honest recomputation,
    // still a divergent record identity, still rejected by the packaged join.
    let mut foreign_container = authentic.clone();
    let foreign_bytes = b"a different published container".as_slice();
    foreign_container.container_byte_count = foreign_bytes.len();
    foreign_container.container_digest = executable_container_digest(foreign_bytes);
    foreign_container.publication_evidence_digest = claimed_evidence(&foreign_container);
    foreign_container.installation_evidence_digest = claimed_installation(&foreign_container);
    assert!(foreign_container.has_consistent_installation_identity());
    assert!(replay_mint(&foreign_container, &inputs));
    assert_ne!(
        foreign_container.installation_evidence_digest,
        authentic.installation_evidence_digest
    );
    assert!(
        !packaged_report(&foreign_container, &authentic)
            .has_consistent_executable_publication_custody(),
        "the package container-commitment join rejects the foreign container"
    );

    // The package join sees the path and container fields even under honest
    // recomputation; a certificate-layer field it cannot see still diverges
    // the pinned record identity.
    let mut changed_count = authentic.clone();
    changed_count.container_byte_count += 1;
    changed_count.publication_evidence_digest = claimed_evidence(&changed_count);
    changed_count.installation_evidence_digest = claimed_installation(&changed_count);
    assert!(
        !packaged_report(&changed_count, &authentic)
            .has_consistent_executable_publication_custody(),
        "the package byte-count join rejects the substitution"
    );
    let mut changed_digest = authentic.clone();
    changed_digest.container_digest = crate::ExecutableContainerDigest::from_digest([0x55; 32]);
    changed_digest.publication_evidence_digest = claimed_evidence(&changed_digest);
    changed_digest.installation_evidence_digest = claimed_installation(&changed_digest);
    assert!(
        !packaged_report(&changed_digest, &authentic)
            .has_consistent_executable_publication_custody(),
        "the package container-digest join rejects the substitution"
    );
    let mut changed_inventory = authentic.clone();
    changed_inventory.inventory_report_fingerprint ^= 1;
    recompute_containing_identity(&mut changed_inventory, &inputs);
    assert!(
        packaged_report(&changed_inventory, &authentic)
            .has_consistent_executable_publication_custody(),
        "the package join does not cover the inventory fingerprint — the record identity does"
    );

    // --- the containing identities themselves ---

    // A substituted evidence digest can never be honestly recomputed: the
    // recomputation is what produces it. Even under a recomputed downstream
    // installation digest the claimed evidence disagrees with the minted one.
    rejects_stale("a substituted publication evidence digest", &|receipt| {
        receipt.publication_evidence_digest =
            NativePublicationEvidenceDigest::from_digest([0x55; 32]);
    });
    let mut changed_evidence = authentic.clone();
    changed_evidence.publication_evidence_digest =
        NativePublicationEvidenceDigest::from_digest([0x55; 32]);
    changed_evidence.installation_evidence_digest = claimed_installation(&changed_evidence);
    assert!(
        !changed_evidence.has_consistent_installation_identity(),
        "a substituted evidence digest under a recomputed installation still rejects"
    );
    assert!(!replay_mint(&changed_evidence, &inputs));

    rejects_stale("a substituted installation evidence digest", &|receipt| {
        receipt.installation_evidence_digest =
            ExecutableInstallationEvidenceDigest::from_digest([0x55; 32]);
    });
}
