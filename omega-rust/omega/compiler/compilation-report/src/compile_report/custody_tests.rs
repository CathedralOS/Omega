//! One-field mutation coverage for the native publication receipt families —
//! the flat [`ExecutablePublicationReceipt`] and the macOS package
//! [`NativePackagePublicationReceipt`] — each with its containing report
//! custody.
//!
//! The flat receipt's producer is `CompileReport::publish_retained_native_artifact`,
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
//!
//! The package receipt's producer is `publish_macos_application_package`,
//! which mints `package_evidence_digest` over the package root, the authored
//! application name and identifier, the inner executable's container
//! commitment, and the ordered member rows — each member's package-relative
//! path, byte count, and component digest, plus the member count itself. The
//! retained `executable_byte_count` never enters that digest: its replay
//! binding is the `Contents/MacOS/<name>` member's own byte count and the flat
//! receipt's container byte count at the report-level join. The same join
//! cross-binds the package root through `inner_executable_path`, the
//! container commitment, and the retained application name and identifier
//! metadata; member internals sit outside the join, so a member-level
//! substitution honestly recomputed is a different self-consistent record and
//! the divergent published identity is the rejection. Substitutions the
//! executable-member join cannot adopt — a renamed or duplicated
//! `Contents/MacOS/<name>` member, a count disagreeing with the retained byte
//! count, a member roster without exactly one inner executable — reject even
//! under an honestly recomputed containing identity.

use std::path::{Path, PathBuf};

use crate::executable_publication::{
    executable_container_digest, native_publication_certificate_digest,
    native_publication_evidence_digest,
};
use crate::package::{
    NativePackageEvidenceDigest, NativePackagePublicationReceipt, PackageComponentDigest,
    PackagePublicationComponent, package_component_digest,
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
        build_outputs: None,
        build_observation: None,
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

/// The inner executable's package-relative coordinate:
/// `Contents/MacOS/<name>`.
fn executable_member_path(name: &str) -> PathBuf {
    Path::new("Contents").join("MacOS").join(name)
}

/// One honest `window-app` package minted exactly as
/// `publish_macos_application_package` records it: the plist member first,
/// the inner executable member bound to the flat receipt's container
/// custody, then the requested `.psi` companion — each row carrying the
/// member's package-relative path, byte count, and component digest in the
/// producer's fixed order.
fn mint_package(
    flat: &ExecutablePublicationReceipt,
    inputs: &PublicationInputs,
) -> NativePackagePublicationReceipt {
    let plist = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist/>".as_slice();
    let psi = inputs.semantic_bytes.as_slice();
    NativePackagePublicationReceipt::new(
        PathBuf::from("build/window-app.app"),
        "window-app".to_owned(),
        build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app")
            .expect("valid identifier"),
        flat.container_digest,
        flat.container_byte_count,
        vec![
            PackagePublicationComponent {
                relative_path: Path::new("Contents").join("Info.plist"),
                byte_count: plist.len(),
                digest: package_component_digest(plist),
            },
            PackagePublicationComponent {
                relative_path: executable_member_path("window-app"),
                byte_count: flat.container_byte_count,
                digest: package_component_digest(&inputs.container_bytes),
            },
            PackagePublicationComponent {
                relative_path: executable_member_path("window-app.psi"),
                byte_count: psi.len(),
                digest: package_component_digest(psi),
            },
        ],
    )
}

/// Honestly recompute the package receipt's containing identity after a
/// substitution. `NativePackagePublicationReceipt::new` is the only mint —
/// the receipt has no wire form — so rebuilding over the mutated fields is
/// the honest recomputation.
fn recomputed_package(
    receipt: &NativePackagePublicationReceipt,
) -> NativePackagePublicationReceipt {
    NativePackagePublicationReceipt::new(
        receipt.package_root.clone(),
        receipt.application_name.clone(),
        receipt.application_identifier.clone(),
        receipt.executable_container_digest,
        receipt.executable_byte_count,
        receipt.components.clone(),
    )
}

/// A packaged publication report over an explicit package receipt: the flat
/// receipt joined to `package`, with the retained application metadata the
/// authentic build recorded.
fn packaged_report_with(
    flat: &ExecutablePublicationReceipt,
    package: NativePackagePublicationReceipt,
) -> CompileReport {
    let mut report = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    report.package_publication = Some(package);
    report.application_name = Some("window-app".to_owned());
    report.application_identifier = Some(
        build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app")
            .expect("valid identifier"),
    );
    report.application_intent = Some(build_evaluation::HostedApplicationIntent::Gui);
    report
}

#[test]
fn native_package_publication_receipt_rejects_every_one_field_substitution() {
    let inputs = honest_inputs();
    let flat = mint(&inputs);
    let authentic = mint_package(&flat, &inputs);
    assert!(
        authentic.has_consistent_package_identity(),
        "the producer mint is self-consistent"
    );
    assert_eq!(
        authentic.inner_executable_path().as_deref(),
        Some(inputs.output_path.as_path()),
        "the package receipt names the flat receipt's destination"
    );
    let published = packaged_report_with(&flat, authentic.clone());
    assert!(published.has_consistent_executable_publication_custody());
    assert_eq!(
        published.checked_native_package_path(),
        Some(Path::new("build/window-app.app"))
    );
    assert_eq!(
        published.checked_native_executable_path(),
        Some(inputs.output_path.as_path())
    );

    // A substitution that keeps the minted evidence digest is rejected by
    // the receipt's own recomputation — the digest mismatch, or the
    // executable-member join for a field the digest does not commit — and by
    // the report-level custody join that replays the receipt first.
    let rejects_stale =
        |name: &'static str, mutate: &dyn Fn(&mut NativePackagePublicationReceipt)| {
            let mut changed = authentic.clone();
            mutate(&mut changed);
            assert_ne!(
                changed, authentic,
                "{name}: substitution changes the receipt"
            );
            assert!(
                !changed.has_consistent_package_identity(),
                "{name}: the receipt's replay rejects a stale containing identity"
            );
            let stale = packaged_report_with(&flat, changed);
            assert!(
                !stale.has_consistent_executable_publication_custody(),
                "{name}: report custody rejects"
            );
            assert!(
                stale.checked_native_package_path().is_none(),
                "{name}: no checked package path escapes"
            );
            assert!(
                stale.checked_native_executable_path().is_none(),
                "{name}: no checked executable path escapes"
            );
        };

    // A substitution whose containing identity is honestly recomputed is a
    // different self-consistent receipt: the package evidence digest a
    // deployment journal pins as the published record identity diverges.
    let recomputed_foreign = |name: &'static str,
                              mutate: &dyn Fn(&mut NativePackagePublicationReceipt)|
     -> NativePackagePublicationReceipt {
        let mut changed = authentic.clone();
        mutate(&mut changed);
        assert_ne!(
            changed, authentic,
            "{name}: substitution changes the receipt"
        );
        let foreign = recomputed_package(&changed);
        assert!(
            foreign.has_consistent_package_identity(),
            "{name}: the honestly recomputed containing identity stays self-consistent"
        );
        assert_ne!(
            foreign.package_evidence_digest, authentic.package_evidence_digest,
            "{name}: the published record identity a deployment journal replays diverges"
        );
        foreign
    };

    // The report-level join sees the package root, the container commitment
    // and byte count, and the retained application name and identifier: a
    // foreign receipt claiming a different value on any of those axes is
    // rejected outright rather than adopted.
    let rejects_at_report_join = |name: &'static str, package: &NativePackagePublicationReceipt| {
        let report = packaged_report_with(&flat, package.clone());
        assert!(
            !report.has_consistent_executable_publication_custody(),
            "{name}: the report-level join rejects"
        );
        assert!(
            report.checked_native_package_path().is_none(),
            "{name}: no checked package path escapes"
        );
    };

    // Member internals sit outside the report join: the foreign receipt is
    // adopted as a different self-consistent publication, and the divergent
    // record identity is what a journal replay rejects.
    let adopted_foreign = |name: &'static str, package: &NativePackagePublicationReceipt| {
        let report = packaged_report_with(&flat, package.clone());
        assert!(
            report.has_consistent_executable_publication_custody(),
            "{name}: the report join cannot see this member axis — the divergent record identity is the rejection"
        );
        assert_eq!(
            report.checked_native_package_path(),
            Some(Path::new("build/window-app.app")),
            "{name}: the foreign receipt's claimed root is adopted verbatim"
        );
    };

    // --- the package root: committed, and joined to the flat destination
    // through `inner_executable_path` ---

    rejects_stale("a substituted package root", &|receipt| {
        receipt.package_root = "build/foreign.app".into();
    });
    let foreign_root = recomputed_foreign("a substituted package root", &|receipt| {
        receipt.package_root = "build/foreign.app".into();
    });
    rejects_at_report_join("a substituted package root", &foreign_root);
    // A fully coupled move — the flat receipt's destination follows the
    // foreign root under its own honestly recomputed installation identity —
    // is a different self-consistent publication: the claimed root is
    // adopted verbatim and the two divergent record identities are the only
    // authority over it.
    let mut moved_flat = flat.clone();
    moved_flat.output_path = "build/foreign.app/Contents/MacOS/window-app".into();
    moved_flat.installation_evidence_digest = claimed_installation(&moved_flat);
    let moved = packaged_report_with(&moved_flat, foreign_root);
    assert!(moved.has_consistent_executable_publication_custody());
    assert_eq!(
        moved.checked_native_package_path(),
        Some(Path::new("build/foreign.app")),
        "the claimed package root is adopted verbatim"
    );
    assert_eq!(
        moved.checked_native_executable_path(),
        Some(Path::new("build/foreign.app/Contents/MacOS/window-app"))
    );

    // --- the application name: committed, names the executable member's
    // exact coordinate, and joins the retained application metadata ---

    rejects_stale("a substituted application name", &|receipt| {
        receipt.application_name = "other-app".to_owned();
    });
    // Honest recomputation cannot adopt a renamed application: no
    // `Contents/MacOS/other-app` member exists, so the executable-member
    // join still rejects even though the recomputed digest diverges.
    let mut renamed = authentic.clone();
    renamed.application_name = "other-app".to_owned();
    let renamed = recomputed_package(&renamed);
    assert_ne!(
        renamed.package_evidence_digest,
        authentic.package_evidence_digest
    );
    assert!(
        !renamed.has_consistent_package_identity(),
        "the executable-member join rejects what the recomputed digest cannot see"
    );
    rejects_at_report_join("a substituted application name", &renamed);
    // Coupled with the member's rename the recomputed receipt is
    // self-consistent again — and both the retained-name join and the
    // inner-executable path join still reject it.
    let mut coupled_name = authentic.clone();
    coupled_name.application_name = "other-app".to_owned();
    coupled_name.components[1].relative_path = executable_member_path("other-app");
    let coupled_name = recomputed_package(&coupled_name);
    assert!(coupled_name.has_consistent_package_identity());
    assert_ne!(
        coupled_name.package_evidence_digest,
        authentic.package_evidence_digest
    );
    rejects_at_report_join("a coupled application-name substitution", &coupled_name);
    // When every joined claim moves together — member path, flat
    // destination, and retained metadata — the foreign name is adopted
    // verbatim as a different publication; the divergent record identities
    // remain the rejection.
    let mut renamed_flat = flat.clone();
    renamed_flat.output_path = "build/window-app.app/Contents/MacOS/other-app".into();
    renamed_flat.installation_evidence_digest = claimed_installation(&renamed_flat);
    let mut moved = packaged_report_with(&renamed_flat, coupled_name);
    moved.application_name = Some("other-app".to_owned());
    assert!(moved.has_consistent_executable_publication_custody());
    assert_eq!(
        moved.checked_native_executable_path(),
        Some(Path::new("build/window-app.app/Contents/MacOS/other-app")),
        "the coupled foreign name is adopted verbatim"
    );

    // --- the application identifier: committed, and joined to the retained
    // metadata ---

    let other_identifier = || {
        build_evaluation::ApplicationIdentifier::new(b"com.other.app").expect("valid identifier")
    };
    rejects_stale("a substituted application identifier", &|receipt| {
        receipt.application_identifier = other_identifier();
    });
    let foreign_identifier =
        recomputed_foreign("a substituted application identifier", &|receipt| {
            receipt.application_identifier = other_identifier();
        });
    rejects_at_report_join("a substituted application identifier", &foreign_identifier);
    // The deeper authority is the publication-time three-way join against
    // the installed plist spelling and the executable's CodeDirectory
    // identity; `package_publication_rejects_a_substituted_executable_identifier`
    // in `package::tests` exercises it against real signed bytes.

    // --- the container commitment: committed, and joined to the flat
    // receipt's container digest ---

    rejects_stale("a substituted container commitment", &|receipt| {
        receipt.executable_container_digest =
            crate::ExecutableContainerDigest::from_digest([0x55; 32]);
    });
    let foreign_commitment = recomputed_foreign("a substituted container commitment", &|receipt| {
        receipt.executable_container_digest =
            crate::ExecutableContainerDigest::from_digest([0x55; 32]);
    });
    rejects_at_report_join("a substituted container commitment", &foreign_commitment);
    // A fully coupled foreign container — flat and package receipts both
    // claiming it under honestly recomputed identities — is a different
    // self-consistent publication; the two divergent record identities are
    // the rejection.
    let mut foreign_container_flat = flat.clone();
    foreign_container_flat.container_digest =
        crate::ExecutableContainerDigest::from_digest([0x55; 32]);
    foreign_container_flat.publication_evidence_digest = claimed_evidence(&foreign_container_flat);
    foreign_container_flat.installation_evidence_digest =
        claimed_installation(&foreign_container_flat);
    let moved = packaged_report_with(&foreign_container_flat, foreign_commitment);
    assert!(
        moved.has_consistent_executable_publication_custody(),
        "the coupled foreign container is a different self-consistent publication"
    );

    // --- the retained executable byte count: outside the digest, bound by
    // the executable member's own byte count ---

    // The digest never commits `executable_byte_count`, so a stale
    // substitution keeps the authentic containing identity — and the
    // executable-member join still rejects it. Recomputation is the identity
    // here: it cannot adopt the drifted count.
    let mut drifted_count = authentic.clone();
    drifted_count.executable_byte_count += 1;
    assert_ne!(drifted_count, authentic);
    assert!(
        !drifted_count.has_consistent_package_identity(),
        "the executable-member byte-count join rejects a drifted retained count"
    );
    let recomputed_count = recomputed_package(&drifted_count);
    assert_eq!(
        recomputed_count.package_evidence_digest, authentic.package_evidence_digest,
        "the containing identity never committed the retained byte count"
    );
    assert!(
        !recomputed_count.has_consistent_package_identity(),
        "recomputation cannot adopt the drifted count"
    );
    rejects_at_report_join("a substituted retained byte count", &recomputed_count);
    // Coupled with the member's own byte count the receipt is self-consistent
    // again — and the flat receipt's container byte count still rejects it.
    let mut coupled_count = authentic.clone();
    coupled_count.executable_byte_count += 1;
    coupled_count.components[1].byte_count += 1;
    let coupled_count = recomputed_package(&coupled_count);
    assert!(coupled_count.has_consistent_package_identity());
    assert_ne!(
        coupled_count.package_evidence_digest,
        authentic.package_evidence_digest
    );
    rejects_at_report_join("a coupled byte-count substitution", &coupled_count);

    // --- the ordered member rows: each member's path, byte count, and
    // component digest is committed, as is the member count and order ---

    // The plist member's fields sit outside every report join.
    rejects_stale("a substituted plist member path", &|receipt| {
        receipt.components[0].relative_path = Path::new("Contents").join("Info-foreign.plist");
    });
    adopted_foreign(
        "a substituted plist member path",
        &recomputed_foreign("a substituted plist member path", &|receipt| {
            receipt.components[0].relative_path = Path::new("Contents").join("Info-foreign.plist");
        }),
    );
    rejects_stale("a substituted plist member byte count", &|receipt| {
        receipt.components[0].byte_count += 1;
    });
    adopted_foreign(
        "a substituted plist member byte count",
        &recomputed_foreign("a substituted plist member byte count", &|receipt| {
            receipt.components[0].byte_count += 1;
        }),
    );
    rejects_stale("a substituted plist member digest", &|receipt| {
        receipt.components[0].digest = PackageComponentDigest::from_digest([0x44; 32]);
    });
    adopted_foreign(
        "a substituted plist member digest",
        &recomputed_foreign("a substituted plist member digest", &|receipt| {
            receipt.components[0].digest = PackageComponentDigest::from_digest([0x44; 32]);
        }),
    );

    // The executable member's path and byte count are replay-bound by the
    // inner-executable coordinate and the retained count: recomputation
    // cannot adopt either substitution.
    rejects_stale("a substituted executable member path", &|receipt| {
        receipt.components[1].relative_path = executable_member_path("renamed");
    });
    let mut renamed_member = authentic.clone();
    renamed_member.components[1].relative_path = executable_member_path("renamed");
    let renamed_member = recomputed_package(&renamed_member);
    assert!(
        !renamed_member.has_consistent_package_identity(),
        "no `Contents/MacOS/window-app` member remains to satisfy the inner-executable join"
    );
    rejects_at_report_join("a substituted executable member path", &renamed_member);
    rejects_stale("a substituted executable member byte count", &|receipt| {
        receipt.components[1].byte_count += 1;
    });
    let mut drifted_member = authentic.clone();
    drifted_member.components[1].byte_count += 1;
    let drifted_member = recomputed_package(&drifted_member);
    assert!(
        !drifted_member.has_consistent_package_identity(),
        "the member's byte count disagrees with the retained executable byte count"
    );
    rejects_at_report_join(
        "a substituted executable member byte count",
        &drifted_member,
    );
    // The executable member's component digest is committed but not joined
    // at report level — the divergent record identity is the rejection.
    rejects_stale("a substituted executable member digest", &|receipt| {
        receipt.components[1].digest = PackageComponentDigest::from_digest([0x66; 32]);
    });
    adopted_foreign(
        "a substituted executable member digest",
        &recomputed_foreign("a substituted executable member digest", &|receipt| {
            receipt.components[1].digest = PackageComponentDigest::from_digest([0x66; 32]);
        }),
    );

    // Companion members replay exactly like the plist member.
    rejects_stale("a substituted companion member path", &|receipt| {
        receipt.components[2].relative_path = executable_member_path("window-app.proof");
    });
    adopted_foreign(
        "a substituted companion member path",
        &recomputed_foreign("a substituted companion member path", &|receipt| {
            receipt.components[2].relative_path = executable_member_path("window-app.proof");
        }),
    );
    rejects_stale("a substituted companion member digest", &|receipt| {
        receipt.components[2].digest = PackageComponentDigest::from_digest([0x77; 32]);
    });
    adopted_foreign(
        "a substituted companion member digest",
        &recomputed_foreign("a substituted companion member digest", &|receipt| {
            receipt.components[2].digest = PackageComponentDigest::from_digest([0x77; 32]);
        }),
    );

    // Member-set mutations: the count and order are committed. Member-set
    // exactness against the installed tree is the publication-time shape
    // replay's authority; the receipt's binding is its committed identity.
    rejects_stale("a dropped plist member", &|receipt| {
        receipt.components.remove(0);
    });
    adopted_foreign(
        "a dropped plist member",
        &recomputed_foreign("a dropped plist member", &|receipt| {
            receipt.components.remove(0);
        }),
    );
    rejects_stale("a dropped companion member", &|receipt| {
        receipt.components.pop();
    });
    adopted_foreign(
        "a dropped companion member",
        &recomputed_foreign("a dropped companion member", &|receipt| {
            receipt.components.pop();
        }),
    );
    // A roster without exactly one inner executable member can never be
    // honestly adopted: `executable_component` requires the coordinate to
    // appear exactly once.
    rejects_stale("a dropped executable member", &|receipt| {
        receipt.components.remove(1);
    });
    let mut missing_executable = authentic.clone();
    missing_executable.components.remove(1);
    let missing_executable = recomputed_package(&missing_executable);
    assert!(
        !missing_executable.has_consistent_package_identity(),
        "the roster carries no inner executable member"
    );
    rejects_at_report_join("a dropped executable member", &missing_executable);
    rejects_stale("a duplicated plist member", &|receipt| {
        let row = receipt.components[0].clone();
        receipt.components.push(row);
    });
    adopted_foreign(
        "a duplicated plist member",
        &recomputed_foreign("a duplicated plist member", &|receipt| {
            let row = receipt.components[0].clone();
            receipt.components.push(row);
        }),
    );
    rejects_stale("a duplicated executable member", &|receipt| {
        let row = receipt.components[1].clone();
        receipt.components.push(row);
    });
    let mut doubled_executable = authentic.clone();
    let executable_row = doubled_executable.components[1].clone();
    doubled_executable.components.push(executable_row);
    let doubled_executable = recomputed_package(&doubled_executable);
    assert!(
        !doubled_executable.has_consistent_package_identity(),
        "the inner executable coordinate must appear exactly once"
    );
    rejects_at_report_join("a duplicated executable member", &doubled_executable);
    rejects_stale("a reordered member roster", &|receipt| {
        receipt.components.swap(0, 1);
    });
    adopted_foreign(
        "a reordered member roster",
        &recomputed_foreign("a reordered member roster", &|receipt| {
            receipt.components.swap(0, 1);
        }),
    );
    rejects_stale("an appended foreign member", &|receipt| {
        receipt.components.push(PackagePublicationComponent {
            relative_path: executable_member_path("window-app.extra"),
            byte_count: 4,
            digest: package_component_digest(b"extra"),
        });
    });
    adopted_foreign(
        "an appended foreign member",
        &recomputed_foreign("an appended foreign member", &|receipt| {
            receipt.components.push(PackagePublicationComponent {
                relative_path: executable_member_path("window-app.extra"),
                byte_count: 4,
                digest: package_component_digest(b"extra"),
            });
        }),
    );
    rejects_stale("an emptied member roster", &|receipt| {
        receipt.components.clear();
    });
    let mut emptied = authentic.clone();
    emptied.components.clear();
    let emptied = recomputed_package(&emptied);
    assert!(
        !emptied.has_consistent_package_identity(),
        "the empty roster carries no inner executable member"
    );
    rejects_at_report_join("an emptied member roster", &emptied);

    // --- the containing identity itself ---

    // A substituted evidence digest can never be honestly recomputed: the
    // recomputation is what produces it.
    rejects_stale("a substituted package evidence digest", &|receipt| {
        receipt.package_evidence_digest = NativePackageEvidenceDigest::from_digest([0x55; 32]);
    });
}
