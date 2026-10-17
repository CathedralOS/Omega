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
    // Independently evaluated SHA-256 vectors for the post-PCC v1 byte layout,
    // including its non-parameter-order fingerprint sequence. The retired
    // proof-length and proof-byte frame is absent from the certificate.
    let vectors = [
        (
            *receipt.container_digest().as_bytes(),
            "50e382152c267a4e5d5b9ce1a60d864d0840fcf4514d5a9694139feb25fa7fe2",
        ),
        (
            *receipt.certificate_digest().as_bytes(),
            "41be7cb69cfeae55c854e5770667279a4411d040e65c656cabccbd87e9440cce",
        ),
        (
            *receipt.publication_evidence_digest().as_bytes(),
            "a6dbd639bd5dc4af9dd495786dbb7df8fb30f0ad90d7420521b380cdfa64ef4a",
        ),
        (
            *receipt.installation_evidence_digest().as_bytes(),
            "42a165c5cfef1fde6476c4ebe68bd3d982b53d805d319974192eb9ff35db13dc",
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
