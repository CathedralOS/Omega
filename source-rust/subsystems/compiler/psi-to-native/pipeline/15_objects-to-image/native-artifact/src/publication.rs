/*
A direct executable publication receipt records digests of its inputs, the
emitted container and its installation. A dynamic ELF receipt uses a separate
digest domain. Both receipts bind the output path at installation.
*/

use crate::digest_encoder::DomainDigestEncoder;
use crate::{RetainedDynamicElfArtifact, RetainedNativeArtifact};
use std::path::{Path, PathBuf};

macro_rules! publication_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

pub(crate) use publication_digest;

// Hash of the artifact identity, semantic bytes, target, symbols and backend reports.
publication_digest!(NativePublicationInputsDigest);
publication_digest!(NativePublicationEvidenceDigest);
publication_digest!(ExecutableContainerDigest);
publication_digest!(ExecutableInstallationEvidenceDigest);
publication_digest!(PackageComponentDigest);
publication_digest!(NativePackageEvidenceDigest);

pub fn executable_installation_evidence_digest(
    publication_evidence_digest: NativePublicationEvidenceDigest,
    callback_placement_identity_report_fingerprint: u64,
    output_path: &std::path::Path,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
) -> ExecutableInstallationEvidenceDigest {
    let mut digest =
        DomainDigestEncoder::new(b"omega.installed-executable-publication-evidence.sha256.v1\0");
    digest.raw(publication_evidence_digest.as_bytes());
    digest.raw(&callback_placement_identity_report_fingerprint.to_le_bytes());
    digest.byte(0);
    let path = output_path.as_os_str().as_encoded_bytes();
    digest.u64(path.len() as u64);
    digest.raw(path);
    digest.u64(container_byte_count as u64);
    digest.raw(container_digest.as_bytes());
    ExecutableInstallationEvidenceDigest::from_digest(digest.finish())
}

pub(crate) fn native_publication_inputs_digest(
    native_artifact_identity: &[u8; 32],
    semantic_bytes: &[u8],
    target: target::NativeTarget,
    final_image_symbol_digest: image::FinalImageSymbolDigest,
    boundary_contract_report_fingerprint: Option<u64>,
    inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
) -> NativePublicationInputsDigest {
    // Replay reports are deliberately excluded from the v2 input commitment.
    let mut digest = DomainDigestEncoder::new(b"omega.native-publication-inputs.sha256.v2\0");
    digest.raw(native_artifact_identity);
    digest.u64(semantic_bytes.len() as u64);
    digest.raw(semantic_bytes);
    digest.raw(&target.wire_identity_bytes());
    digest.raw(final_image_symbol_digest.as_bytes());
    digest.byte(u8::from(boundary_contract_report_fingerprint.is_some()));
    digest.raw(
        &boundary_contract_report_fingerprint
            .unwrap_or_default()
            .to_le_bytes(),
    );
    digest.raw(inventory_digest.as_bytes());
    digest.raw(&inventory_report_fingerprint.to_le_bytes());
    NativePublicationInputsDigest::from_digest(digest.finish())
}

pub(crate) fn native_publication_evidence_digest(
    native_artifact_identity: &[u8; 32],
    publication_inputs_digest: NativePublicationInputsDigest,
    callback_placement_identity_report_fingerprint: u64,
    inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
) -> NativePublicationEvidenceDigest {
    let mut digest = DomainDigestEncoder::new(b"omega.native-publication-evidence.sha256.v2\0");
    digest.raw(native_artifact_identity);
    digest.raw(publication_inputs_digest.as_bytes());
    digest.raw(inventory_digest.as_bytes());
    for value in [
        callback_placement_identity_report_fingerprint,
        inventory_report_fingerprint,
        container_byte_count as u64,
    ] {
        digest.raw(&value.to_le_bytes());
    }
    digest.raw(container_digest.as_bytes());
    NativePublicationEvidenceDigest::from_digest(digest.finish())
}

pub fn executable_container_digest(bytes: &[u8]) -> ExecutableContainerDigest {
    ExecutableContainerDigest::from_digest(DomainDigestEncoder::content(
        b"omega.published-executable-container.sha256.v1\0",
        bytes,
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutablePublicationReceipt {
    pub(crate) output_path: PathBuf,
    pub(crate) native_artifact_identity: [u8; 32],
    pub(crate) publication_inputs_digest: NativePublicationInputsDigest,
    pub(crate) callback_placement_identity_report_fingerprint: u64,
    pub(crate) boundary_contract_report_fingerprint: Option<u64>,
    pub(crate) inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    pub(crate) inventory_report_fingerprint: u64,
    pub(crate) publication_evidence_digest: NativePublicationEvidenceDigest,
    pub(crate) container_byte_count: usize,
    pub(crate) container_digest: ExecutableContainerDigest,
    pub(crate) installation_evidence_digest: ExecutableInstallationEvidenceDigest,
}

impl ExecutablePublicationReceipt {
    pub fn new(
        output_path: PathBuf,
        native_artifact_identity: [u8; 32],
        publication_inputs_digest: NativePublicationInputsDigest,
        callback_placement_identity_report_fingerprint: u64,
        boundary_contract_report_fingerprint: Option<u64>,
        inventory_digest: image::PlacedExecutableRegionInventoryDigest,
        inventory_report_fingerprint: u64,
        publication_evidence_digest: NativePublicationEvidenceDigest,
        container_byte_count: usize,
        container_digest: ExecutableContainerDigest,
        installation_evidence_digest: ExecutableInstallationEvidenceDigest,
    ) -> Self {
        Self {
            output_path,
            native_artifact_identity,
            publication_inputs_digest,
            callback_placement_identity_report_fingerprint,
            boundary_contract_report_fingerprint,
            inventory_digest,
            inventory_report_fingerprint,
            publication_evidence_digest,
            container_byte_count,
            container_digest,
            installation_evidence_digest,
        }
    }

    pub fn output_path(&self) -> &std::path::Path {
        &self.output_path
    }

    pub const fn publication_inputs_digest(&self) -> NativePublicationInputsDigest {
        self.publication_inputs_digest
    }

    pub const fn native_artifact_identity(&self) -> &[u8; 32] {
        &self.native_artifact_identity
    }

    pub const fn callback_placement_identity_report_fingerprint(&self) -> u64 {
        self.callback_placement_identity_report_fingerprint
    }

    pub const fn boundary_contract_report_fingerprint(&self) -> Option<u64> {
        self.boundary_contract_report_fingerprint
    }

    pub const fn inventory_digest(&self) -> image::PlacedExecutableRegionInventoryDigest {
        self.inventory_digest
    }

    pub const fn inventory_report_fingerprint(&self) -> u64 {
        self.inventory_report_fingerprint
    }

    pub const fn publication_evidence_digest(&self) -> NativePublicationEvidenceDigest {
        self.publication_evidence_digest
    }

    pub const fn container_byte_count(&self) -> usize {
        self.container_byte_count
    }

    pub const fn container_digest(&self) -> ExecutableContainerDigest {
        self.container_digest
    }

    pub const fn installation_evidence_digest(&self) -> ExecutableInstallationEvidenceDigest {
        self.installation_evidence_digest
    }

    pub fn has_consistent_installation_identity(&self) -> bool {
        self.publication_evidence_digest
            == native_publication_evidence_digest(
                &self.native_artifact_identity,
                self.publication_inputs_digest,
                self.callback_placement_identity_report_fingerprint,
                self.inventory_digest,
                self.inventory_report_fingerprint,
                self.container_byte_count,
                self.container_digest,
            )
            && self.installation_evidence_digest
                == executable_installation_evidence_digest(
                    self.publication_evidence_digest,
                    self.callback_placement_identity_report_fingerprint,
                    &self.output_path,
                    self.container_byte_count,
                    self.container_digest,
                )
    }
}

pub struct PreparedExecutablePublication<'artifact> {
    artifact: &'artifact RetainedNativeArtifact,
    container_digest: ExecutableContainerDigest,
}

impl<'artifact> PreparedExecutablePublication<'artifact> {
    pub fn new(artifact: &'artifact RetainedNativeArtifact) -> Result<Self, String> {
        let output = artifact.image().output();
        if Path::new(&output.file_name).components().count() != 1 {
            return Err("native artifact supplied a non-local output filename".to_owned());
        }
        Ok(Self {
            artifact,
            container_digest: executable_container_digest(&output.bytes),
        })
    }

    pub const fn container_digest(&self) -> ExecutableContainerDigest {
        self.container_digest
    }

    pub fn into_receipt(
        self,
        output_path: PathBuf,
    ) -> Result<ExecutablePublicationReceipt, String> {
        let artifact = self.artifact;
        let output = artifact.image().output();
        let native_artifact_identity = *artifact.identity().as_bytes();
        // This report was never populated by the direct producer. Per-provider
        // boundary reports remain separately bound by the artifact identity.
        let boundary_contract_report_fingerprint = None;
        let publication_inputs_digest = native_publication_inputs_digest(
            &native_artifact_identity,
            artifact.semantic_bytes(),
            artifact.target(),
            artifact.image().final_image_symbol_digest(),
            boundary_contract_report_fingerprint,
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
        );
        let publication_evidence_digest = native_publication_evidence_digest(
            &native_artifact_identity,
            publication_inputs_digest,
            output.callback_placement_identity_report_fingerprint,
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
            output.bytes.len(),
            self.container_digest,
        );
        let installation_evidence_digest = executable_installation_evidence_digest(
            publication_evidence_digest,
            output.callback_placement_identity_report_fingerprint,
            &output_path,
            output.bytes.len(),
            self.container_digest,
        );
        let receipt = ExecutablePublicationReceipt::new(
            output_path,
            native_artifact_identity,
            publication_inputs_digest,
            output.callback_placement_identity_report_fingerprint,
            boundary_contract_report_fingerprint,
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
            publication_evidence_digest,
            output.bytes.len(),
            self.container_digest,
            installation_evidence_digest,
        );
        if !receipt.has_consistent_installation_identity() {
            return Err("native publication produced an inconsistent installation receipt".into());
        }
        Ok(receipt)
    }
}

pub struct PreparedDynamicElfPublication<'artifact> {
    artifact: &'artifact RetainedDynamicElfArtifact,
    container_digest: ExecutableContainerDigest,
}

impl<'artifact> PreparedDynamicElfPublication<'artifact> {
    pub fn new(artifact: &'artifact RetainedDynamicElfArtifact) -> Result<Self, String> {
        let output = artifact.image().output();
        if Path::new(&output.file_name).components().count() != 1 {
            return Err("dynamic ELF artifact supplied a non-local output filename".to_owned());
        }
        Ok(Self {
            artifact,
            container_digest: executable_container_digest(&output.bytes),
        })
    }

    pub const fn container_digest(&self) -> ExecutableContainerDigest {
        self.container_digest
    }

    pub fn into_receipt(self, output_path: PathBuf) -> DynamicElfPublicationReceipt {
        let container_byte_count = self.artifact.image().output().bytes.len();
        let dynamic_elf_artifact_identity = *self.artifact.identity().as_bytes();
        let installation_evidence_digest = dynamic_elf_installation_evidence_digest(
            &dynamic_elf_artifact_identity,
            &output_path,
            container_byte_count,
            self.container_digest,
        );
        DynamicElfPublicationReceipt {
            output_path,
            dynamic_elf_artifact_identity,
            container_byte_count,
            container_digest: self.container_digest,
            installation_evidence_digest,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicElfPublicationReceipt {
    output_path: PathBuf,
    dynamic_elf_artifact_identity: [u8; 32],
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
    installation_evidence_digest: [u8; 32],
}

impl DynamicElfPublicationReceipt {
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    pub const fn dynamic_elf_artifact_identity(&self) -> &[u8; 32] {
        &self.dynamic_elf_artifact_identity
    }

    pub const fn container_byte_count(&self) -> usize {
        self.container_byte_count
    }

    pub const fn container_digest(&self) -> ExecutableContainerDigest {
        self.container_digest
    }
}

fn dynamic_elf_installation_evidence_digest(
    dynamic_elf_artifact_identity: &[u8; 32],
    output_path: &Path,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.installed-dynamic-elf-publication-evidence.sha256.v2\0",
    );
    digest.raw(dynamic_elf_artifact_identity);
    let path = output_path.as_os_str().as_encoded_bytes();
    digest.u64(path.len() as u64);
    digest.raw(path);
    digest.u64(container_byte_count as u64);
    digest.raw(container_digest.as_bytes());
    digest.finish()
}

#[cfg(test)]
mod tests {
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
        let inventory = image::PlacedExecutableRegionInventoryDigest::from_digest([5; 32]);
        let publication_inputs_digest = native_publication_inputs_digest(
            &[1; 32],
            b"psi",
            target::NativeTarget::linux_x64(),
            image::final_image_symbol_digest(&symbol_image),
            Some(17),
            inventory,
            23,
        );
        let evidence = native_publication_evidence_digest(
            &[1; 32],
            publication_inputs_digest,
            29,
            inventory,
            23,
            6,
            container,
        );
        let installation =
            executable_installation_evidence_digest(evidence, 29, &output_path, 6, container);
        ExecutablePublicationReceipt::new(
            output_path,
            [1; 32],
            publication_inputs_digest,
            29,
            Some(17),
            inventory,
            23,
            evidence,
            6,
            container,
            installation,
        )
    }

    #[test]
    fn dynamic_publication_binds_identity_path_and_exact_container_without_replay_reports() {
        let path = Path::new("program");
        let container = executable_container_digest(b"native");
        let digest = dynamic_elf_installation_evidence_digest(&[1; 32], path, 6, container);
        assert_eq!(
            digest,
            [
                229, 154, 98, 36, 84, 186, 140, 222, 235, 195, 86, 46, 201, 28, 41, 177, 243, 123,
                130, 49, 94, 96, 63, 134, 129, 115, 187, 39, 254, 99, 18, 198
            ]
        );
        for changed in [
            dynamic_elf_installation_evidence_digest(&[2; 32], path, 6, container),
            dynamic_elf_installation_evidence_digest(&[1; 32], Path::new("other"), 6, container),
            dynamic_elf_installation_evidence_digest(&[1; 32], path, 7, container),
            dynamic_elf_installation_evidence_digest(
                &[1; 32],
                path,
                6,
                executable_container_digest(b"other!"),
            ),
        ] {
            assert_ne!(digest, changed);
        }
    }

    #[test]
    fn publication_digests_without_recheck_reports_detect_substitution() {
        let receipt = receipt(PathBuf::from("program"));
        let vectors = [
            (
                *receipt.container_digest().as_bytes(),
                "50e382152c267a4e5d5b9ce1a60d864d0840fcf4514d5a9694139feb25fa7fe2",
            ),
            (
                *receipt.publication_inputs_digest().as_bytes(),
                "f9d1c8b39c123ff6defc6c5082cf523dd2bd31aab4414b4d84eade84dbd49c23",
            ),
            (
                *receipt.publication_evidence_digest().as_bytes(),
                "31e2ecf1015aa3aea4704ce58915fcbc61c1778fa897c333ed8626ac4b1d4386",
            ),
            (
                *receipt.installation_evidence_digest().as_bytes(),
                "017d078f58a7285788892a0491a5e7bbc8bef043a17f4f2fca83eb469ad3b093",
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
}
