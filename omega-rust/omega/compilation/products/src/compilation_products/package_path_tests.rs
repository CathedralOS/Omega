// Unit tests verifying that `checked_native_package_path` surfaces a published
// `.app` root only when the package receipt replays on its own, names the same
// executable as the flat receipt, and matches the retained application identity.

use super::*;
use native_artifact::{
    ExecutableContainerDigest, ExecutableInstallationEvidenceDigest,
    NativePublicationCertificateDigest, NativePublicationEvidenceDigest,
    PackagePublicationComponent, executable_container_digest, publish_macos_application_package,
};
use std::path::Path;

struct PublicationDirectory(PathBuf);

impl Drop for PublicationDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn signed_executable(identifier: &str) -> Vec<u8> {
    let mut image = image::FinalImage::with_capacity(
        target::NativeTarget::macos_arm64(),
        image::FinalImageMemory {
            text: 0xd65f_03c0u32.to_le_bytes().to_vec(),
            ..Default::default()
        },
        image::FinalImageSymbolHandle::invalid(),
        1,
        0,
        0,
    );
    image.symbol_table.entry_symbol = image.symbol_table.symbols.insert(image::FinalImageSymbol {
        name: "entry".into(),
        section: image::FinalImageSection::Text,
        size: 4,
        kind: object_file::SymbolKind::Function,
        ..Default::default()
    });
    image_macho::emit_macho_executable_signed(image, identifier)
        .expect("emit signed fixture")
        .bytes
}

fn executable_receipt(
    output_path: PathBuf,
    byte_count: usize,
    container: ExecutableContainerDigest,
) -> ExecutablePublicationReceipt {
    ExecutablePublicationReceipt::new(
        output_path,
        [1; 32],
        NativePublicationCertificateDigest::from_digest([2; 32]),
        29,
        None,
        image::PlacedExecutableRegionInventoryDigest::from_digest([5; 32]),
        23,
        image::CompilerTextDerivationDigest::from_digest([3; 32]),
        image::CompilerFunctionValidationDigest::from_digest([4; 32]),
        19,
        NativePublicationEvidenceDigest::from_digest([6; 32]),
        byte_count,
        container,
        ExecutableInstallationEvidenceDigest::from_digest([7; 32]),
    )
}

fn report(
    application: NativeApplicationMetadata,
    executable: ExecutablePublicationReceipt,
    package: NativePackagePublicationReceipt,
) -> CompileReport {
    CompileReport::shell(
        PathBuf::from("main.omg"),
        1,
        CompileProduct::NativeExecutable(PublishedNativeProduct {
            runtime_provisions: None,
            executable,
            package: Some(package),
            optimization_rollback: None,
            application,
        }),
    )
}

fn rebuilt(
    receipt: &NativePackagePublicationReceipt,
    root: Option<PathBuf>,
    identifier: Option<target::ApplicationIdentifier>,
    container: Option<ExecutableContainerDigest>,
    byte_count: Option<usize>,
    components: Option<Vec<PackagePublicationComponent>>,
) -> NativePackagePublicationReceipt {
    NativePackagePublicationReceipt::new(
        root.unwrap_or_else(|| receipt.package_root().to_path_buf()),
        receipt.application_name().to_owned(),
        identifier.unwrap_or_else(|| receipt.application_identifier().clone()),
        container.unwrap_or(receipt.executable_container_digest()),
        byte_count.unwrap_or(receipt.executable_byte_count()),
        components.unwrap_or_else(|| receipt.components().to_vec()),
    )
}

#[test]
fn package_path_requires_the_package_and_executable_receipts_to_agree() {
    let directory = PublicationDirectory(
        std::env::temp_dir().join(format!("omega-package-path-{}", std::process::id())),
    );
    std::fs::create_dir_all(&directory.0).unwrap();
    let identifier = target::ApplicationIdentifier::new(b"org.omega.window-test").unwrap();
    let executable = signed_executable(identifier.as_str());
    let container = executable_container_digest(&executable);
    let package = publish_macos_application_package(
        &directory.0,
        "window_test",
        &identifier,
        &executable,
        container,
    )
    .unwrap();
    let application = NativeApplicationMetadata {
        name: Some("window_test".to_owned()),
        identifier: Some(identifier.clone()),
        ..Default::default()
    };
    let flat = || {
        executable_receipt(
            package.inner_executable_path().unwrap(),
            executable.len(),
            container,
        )
    };

    let accepted = report(application.clone(), flat(), package.clone());
    assert_eq!(
        accepted.checked_native_package_path(),
        Some(directory.0.join("window_test.app").as_path())
    );

    let other_identifier = target::ApplicationIdentifier::new(b"org.other").unwrap();
    let other_container = executable_container_digest(b"other bytes");
    let mut reversed = package.components().to_vec();
    reversed.reverse();
    let refused: [(&str, NativePackagePublicationReceipt); 5] = [
        (
            "identifier",
            rebuilt(&package, None, Some(other_identifier), None, None, None),
        ),
        (
            "member order",
            rebuilt(&package, None, None, None, None, Some(reversed)),
        ),
        (
            "root",
            rebuilt(
                &package,
                Some(directory.0.join("different.app")),
                None,
                None,
                None,
                None,
            ),
        ),
        (
            "executable byte count",
            rebuilt(&package, None, None, None, Some(executable.len() + 1), None),
        ),
        (
            "container digest",
            rebuilt(&package, None, None, Some(other_container), None, None),
        ),
    ];
    for (mutation, receipt) in refused {
        assert_eq!(
            report(application.clone(), flat(), receipt).checked_native_package_path(),
            None,
            "a package receipt with a changed {mutation} must not surface"
        );
    }

    let elsewhere = executable_receipt(
        Path::new("elsewhere").to_path_buf(),
        executable.len(),
        container,
    );
    assert_eq!(
        report(application.clone(), elsewhere, package.clone()).checked_native_package_path(),
        None,
        "a flat receipt naming another executable path must not bind the package"
    );
    let other_executable = executable_receipt(
        package.inner_executable_path().unwrap(),
        executable.len(),
        other_container,
    );
    assert_eq!(
        report(application.clone(), other_executable, package.clone())
            .checked_native_package_path(),
        None,
        "a flat receipt for different bytes must not bind the package"
    );
    let other_size = executable_receipt(
        package.inner_executable_path().unwrap(),
        executable.len() + 1,
        container,
    );
    assert_eq!(
        report(application.clone(), other_size, package.clone()).checked_native_package_path(),
        None,
        "a flat receipt with another byte count must not bind the package"
    );
    let unnamed = NativeApplicationMetadata {
        name: None,
        ..application
    };
    assert_eq!(
        report(unnamed, flat(), package).checked_native_package_path(),
        None,
        "a report without the retained application name exposes no package"
    );
}
