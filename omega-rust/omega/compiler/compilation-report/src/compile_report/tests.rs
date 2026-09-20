use crate::{CompileOutputKind, CompileReport, ExecutablePublicationReceipt, package};

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

fn receipt(path: &str) -> ExecutablePublicationReceipt {
    let path: std::path::PathBuf = path.into();
    let certificate = crate::NativePublicationCertificateDigest::from_digest([1; 32]);
    let text_validation = image::CompilerTextDerivationDigest::from_digest([3; 32]);
    let function_validation = function_validation_digest(7);
    let inventory = image::PlacedExecutableRegionInventoryDigest::from_digest([5; 32]);
    let container = crate::ExecutableContainerDigest::from_digest([7; 32]);
    let native_artifact_identity = [9; 32];
    let publication = crate::executable_publication::native_publication_evidence_digest(
        &native_artifact_identity,
        certificate,
        8,
        inventory,
        2,
        text_validation,
        function_validation,
        4,
        6,
        container,
    );
    let installation =
        crate::executable_installation_evidence_digest(publication, 8, &path, 6, container);
    ExecutablePublicationReceipt::new(
        path,
        native_artifact_identity,
        certificate,
        8,
        Some(2),
        inventory,
        2,
        text_validation,
        function_validation,
        4,
        publication,
        6,
        container,
        installation,
    )
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

#[test]
fn flat_installation_v1_digest_is_byte_identical() {
    let digest = crate::executable_installation_evidence_digest(
        crate::NativePublicationEvidenceDigest::from_digest([1; 32]),
        8,
        std::path::Path::new("build/main"),
        6,
        crate::ExecutableContainerDigest::from_digest([7; 32]),
    );
    assert_eq!(
        digest.as_bytes(),
        &[
            67, 169, 237, 176, 22, 91, 25, 36, 38, 178, 255, 56, 96, 186, 99, 95, 193, 42, 167,
            158, 31, 5, 40, 172, 217, 194, 16, 200, 66, 170, 145, 45,
        ]
    );
}

#[test]
fn flat_publication_rejects_evidence_drift() {
    let mutations: &[fn(&mut ExecutablePublicationReceipt)] = &[
        |receipt| receipt.native_artifact_identity[0] ^= 1,
        |receipt| {
            receipt.certificate_digest =
                crate::NativePublicationCertificateDigest::from_digest([99; 32])
        },
        |receipt| receipt.callback_placement_identity_report_fingerprint ^= 1,
        |receipt| {
            receipt.inventory_digest =
                image::PlacedExecutableRegionInventoryDigest::from_digest([99; 32])
        },
        |receipt| receipt.inventory_report_fingerprint ^= 1,
        |receipt| {
            receipt.compiler_text_validation_digest =
                image::CompilerTextDerivationDigest::from_digest([99; 32])
        },
        |receipt| receipt.compiler_function_validation_digest = function_validation_digest(99),
        |receipt| receipt.compiler_function_validation_report_fingerprint ^= 1,
        |receipt| {
            receipt.publication_evidence_digest =
                crate::NativePublicationEvidenceDigest::from_digest([99; 32])
        },
        |receipt| receipt.container_byte_count += 1,
        |receipt| {
            receipt.container_digest = crate::ExecutableContainerDigest::from_digest([99; 32])
        },
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut changed = receipt("build/main");
        mutate(&mut changed);
        let changed = report(true, CompileOutputKind::NativeExecutable, Some(changed));
        assert!(
            !changed.has_consistent_executable_publication_custody(),
            "mutation {index}"
        );
        assert!(
            changed.checked_native_executable_path().is_none(),
            "mutation {index}"
        );
    }
}

#[test]
fn rollback_receipt_is_forbidden_on_check_only_products() {
    let selected = optimization_core::OptimizationSelections::new([
        optimization_core::Optimization::ControlFlowCleanup,
    ])
    .unwrap();
    let requested = selected.clone();
    let rollback = crate::OptimizationRollbackReceipt::new(selected, requested);
    let mut native = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(receipt("build/main")),
    );
    native.optimization_rollback = Some(rollback.clone());
    assert!(native.has_consistent_executable_publication_custody());

    let mut check = report(false, CompileOutputKind::CheckOnly, None);
    check.optimization_rollback = Some(rollback);
    assert!(!check.has_consistent_executable_publication_custody());
}

#[test]
fn flat_publication_rejects_receipt_and_output_kind_drift() {
    let flat = receipt("build/main");
    assert!(flat.has_consistent_installation_identity());
    let native = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    assert!(native.has_consistent_executable_publication_custody());
    assert_eq!(
        native.checked_native_executable_path(),
        Some(std::path::Path::new("build/main")),
    );
    let mut changed_kind = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    changed_kind.output_kind = CompileOutputKind::ObjectContainer;
    assert!(changed_kind.checked_native_executable_path().is_none());
    let check_only = report(false, CompileOutputKind::CheckOnly, None);
    assert!(check_only.has_consistent_executable_publication_custody());
    assert!(check_only.checked_native_executable_path().is_none());
    let object = report(true, CompileOutputKind::ObjectContainer, None);
    assert!(object.has_consistent_executable_publication_custody());
    assert!(object.checked_native_executable_path().is_none());
    assert!(
        !report(false, CompileOutputKind::CheckOnly, Some(flat.clone()),)
            .has_consistent_executable_publication_custody()
    );
    assert!(
        CompileReport::checked(
            "Main/main.omg".into(),
            1,
            false,
            CompileOutputKind::CheckOnly,
            Some(flat.clone()),
        )
        .is_err()
    );
    let missing_native_receipt = report(true, CompileOutputKind::NativeExecutable, None);
    assert!(!missing_native_receipt.has_consistent_executable_publication_custody());
    assert!(
        missing_native_receipt
            .checked_native_executable_path()
            .is_none()
    );
    let dropped_output = report(
        false,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    assert!(!dropped_output.has_consistent_executable_publication_custody());
    assert!(dropped_output.checked_native_executable_path().is_none());
    assert!(
        !report(true, CompileOutputKind::ObjectContainer, Some(flat.clone()),)
            .has_consistent_executable_publication_custody()
    );
    let mut changed = flat.clone();
    changed.installation_evidence_digest =
        crate::ExecutableInstallationEvidenceDigest::from_digest([99; 32]);
    let changed = report(true, CompileOutputKind::NativeExecutable, Some(changed));
    assert!(!changed.has_consistent_executable_publication_custody());
    assert!(changed.checked_native_executable_path().is_none());
    let mut changed = flat.clone();
    changed.output_path = "build/redirected-main".into();
    let changed = report(true, CompileOutputKind::NativeExecutable, Some(changed));
    assert!(!changed.has_consistent_executable_publication_custody());
    assert!(changed.checked_native_executable_path().is_none());
    let mut changed = flat.clone();
    changed.compiler_function_validation_report_fingerprint ^= 1;
    let changed = report(true, CompileOutputKind::NativeExecutable, Some(changed));
    assert!(!changed.has_consistent_executable_publication_custody());
    assert!(changed.checked_native_executable_path().is_none());
    let mut compact_collision = flat.clone();
    compact_collision.compiler_function_validation_digest = function_validation_digest(99);
    assert_eq!(
        compact_collision.compiler_function_validation_report_fingerprint,
        flat.compiler_function_validation_report_fingerprint,
        "the adversary preserves the compact report identity",
    );
    let compact_collision = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(compact_collision),
    );
    assert!(
        !compact_collision.has_consistent_executable_publication_custody(),
        "strong function-validation drift must reject even with a collision-equal report fingerprint",
    );
    assert!(compact_collision.checked_native_executable_path().is_none());
    let retained = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    assert!(retained.wrote_output());
    assert_eq!(retained.root_path(), std::path::Path::new("Main/main.omg"));
    assert_eq!(retained.output_kind(), CompileOutputKind::NativeExecutable);
    assert_eq!(retained.executable_publication(), Some(&flat));
}

/// One structurally valid package receipt whose inner executable
/// component is bound to `exe` custody by construction. The receipt is
/// self-consistent; report custody decides whether it also matches the
/// executable receipt it claims to contain.
fn package_receipt(
    root: &str,
    executable_container_digest: crate::ExecutableContainerDigest,
    executable_byte_count: usize,
) -> package::NativePackagePublicationReceipt {
    package::NativePackagePublicationReceipt::new(
        root.into(),
        "window-app".to_owned(),
        build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app")
            .expect("valid identifier"),
        executable_container_digest,
        executable_byte_count,
        vec![
            package::PackagePublicationComponent {
                relative_path: "Contents/Info.plist".into(),
                byte_count: 4,
                digest: package::PackageComponentDigest::from_digest([3; 32]),
            },
            package::PackagePublicationComponent {
                relative_path: "Contents/MacOS/window-app".into(),
                byte_count: executable_byte_count,
                digest: package::PackageComponentDigest::from_digest([7; 32]),
            },
        ],
    )
}

#[test]
fn package_publication_rejects_custody_join_drift() {
    let inner = "build/window-app.app/Contents/MacOS/window-app";
    let flat = receipt(inner);
    // The exe receipt's container commitment is [7;32] over 6 bytes; the
    // matching package repeats that exact commitment.
    let package = package_receipt(
        "build/window-app.app",
        crate::ExecutableContainerDigest::from_digest([7; 32]),
        6,
    );
    let mut published = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    published.package_publication = Some(package);
    published.application_name = Some("window-app".to_owned());
    published.application_identifier =
        Some(build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app").unwrap());
    published.application_intent = Some(build_evaluation::HostedApplicationIntent::Gui);
    assert!(published.has_consistent_executable_publication_custody());
    assert_eq!(
        published.checked_native_package_path(),
        Some(std::path::Path::new("build/window-app.app")),
    );
    assert_eq!(
        published.checked_native_executable_path(),
        Some(std::path::Path::new(inner)),
    );

    // A package root that does not contain the receipt's inner executable
    // fails the structural join.
    let mut wrong_root = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    wrong_root.package_publication = Some(package_receipt(
        "build/other-app.app",
        crate::ExecutableContainerDigest::from_digest([7; 32]),
        6,
    ));
    wrong_root.application_name = Some("window-app".to_owned());
    wrong_root.application_identifier =
        Some(build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app").unwrap());
    assert!(!wrong_root.has_consistent_executable_publication_custody());
    assert!(wrong_root.checked_native_package_path().is_none());
    assert!(wrong_root.checked_native_executable_path().is_none());

    // An executable commitment that disagrees with the executable
    // receipt's container digest fails the join.
    let mut wrong_container = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    wrong_container.package_publication = Some(package_receipt(
        "build/window-app.app",
        crate::ExecutableContainerDigest::from_digest([8; 32]),
        6,
    ));
    wrong_container.application_name = Some("window-app".to_owned());
    wrong_container.application_identifier =
        Some(build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app").unwrap());
    assert!(!wrong_container.has_consistent_executable_publication_custody());
    assert!(wrong_container.checked_native_package_path().is_none());

    // Report-level application metadata that disagrees with the package
    // receipt fails the join.
    let mut wrong_metadata = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    wrong_metadata.package_publication = Some(package_receipt(
        "build/window-app.app",
        crate::ExecutableContainerDigest::from_digest([7; 32]),
        6,
    ));
    wrong_metadata.application_name = Some("other-app".to_owned());
    wrong_metadata.application_identifier =
        Some(build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app").unwrap());
    assert!(!wrong_metadata.has_consistent_executable_publication_custody());
    assert!(wrong_metadata.checked_native_package_path().is_none());

    // A flat publication carries no package receipt and no package path.
    let flat_only = report(true, CompileOutputKind::NativeExecutable, Some(flat));
    assert!(flat_only.has_consistent_executable_publication_custody());
    assert!(flat_only.checked_native_package_path().is_none());
}
