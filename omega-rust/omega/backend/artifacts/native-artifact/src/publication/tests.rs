use super::*;

fn receipt(output_path: PathBuf) -> ExecutablePublicationReceipt {
    let symbol_image = image::FinalImage::with_capacity(
        target::NativeTarget::linux_x64(),
        image::FinalImageMemory::default(),
        image::FinalImageSymbolHandle::invalid(),
        0,
        0,
        0,
    );
    let container = executable_container_digest(b"native");
    let text = image::CompilerTextDerivationDigest::from_digest([3; 32]);
    let function = image::CompilerFunctionValidationDigest::from_digest([4; 32]);
    let inventory = image::PlacedExecutableRegionInventoryDigest::from_digest([5; 32]);
    let certificate = native_publication_certificate_digest(
        &[1; 32],
        b"psi",
        b"proof",
        target::NativeTarget::linux_x64(),
        image::final_image_symbol_digest(&symbol_image),
        Some(17),
        text,
        function,
        19,
        inventory,
        23,
    );
    let evidence = native_publication_evidence_digest(
        &[1; 32],
        certificate,
        29,
        inventory,
        23,
        text,
        function,
        19,
        6,
        container,
    );
    let installation =
        executable_installation_evidence_digest(evidence, 29, &output_path, 6, container);
    ExecutablePublicationReceipt::new(
        output_path,
        [1; 32],
        certificate,
        29,
        Some(17),
        inventory,
        23,
        text,
        function,
        19,
        evidence,
        6,
        container,
        installation,
    )
}

#[test]
fn v1_publication_digests_keep_their_byte_contract_and_detect_substitution() {
    let receipt = receipt(PathBuf::from("program"));
    // Independently evaluated SHA-256 vectors for the v1 byte layout,
    // including its non-parameter-order fingerprint sequence.
    let vectors = [
        (
            *receipt.container_digest().as_bytes(),
            "50e382152c267a4e5d5b9ce1a60d864d0840fcf4514d5a9694139feb25fa7fe2",
        ),
        (
            *receipt.certificate_digest().as_bytes(),
            "1cb48893f39d6a1e05aaabb2e24f2afdc49aacc6bb60874a4d6569e329628b59",
        ),
        (
            *receipt.publication_evidence_digest().as_bytes(),
            "6d3a2225c2c6bd534cf9b82947e9f40051a987d5fb399848155f66f649a4f434",
        ),
        (
            *receipt.installation_evidence_digest().as_bytes(),
            "3ac999e49d255cff416675cea788ca383418ef3f02f21ae6deda5455afd5e641",
        ),
    ];
    for (bytes, expected) in vectors {
        let actual = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    }
    assert!(receipt.has_consistent_installation_identity());
    let mut substituted = receipt.clone();
    substituted.output_path = PathBuf::from("other");
    assert!(!substituted.has_consistent_installation_identity());
    let mut substituted = receipt.clone();
    substituted.container_byte_count += 1;
    assert!(!substituted.has_consistent_installation_identity());
    let mut substituted = receipt.clone();
    substituted.inventory_report_fingerprint += 1;
    assert!(!substituted.has_consistent_installation_identity());
}

#[test]
fn installed_file_replay_detects_content_length_and_mode_drift() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("omega-publication-{}-{nonce}", std::process::id()));
    std::fs::write(&path, b"native").expect("write fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("set executable mode");
    }
    let receipt = receipt(path.clone());
    receipt
        .validate_published_file()
        .expect("exact installed file");
    std::fs::write(&path, b"edited").expect("same-length edit");
    assert!(
        receipt
            .validate_published_file()
            .unwrap_err()
            .contains("content digest")
    );
    std::fs::write(&path, b"short").expect("length edit");
    assert!(
        receipt
            .validate_published_file()
            .unwrap_err()
            .contains("5 bytes")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(&path, b"native").expect("restore content");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("remove executable mode");
        assert!(
            receipt
                .validate_published_file()
                .unwrap_err()
                .contains("mode")
        );
    }
    std::fs::remove_file(&path).expect("remove fixture");
    assert!(receipt.validate_published_file().is_err());
}
