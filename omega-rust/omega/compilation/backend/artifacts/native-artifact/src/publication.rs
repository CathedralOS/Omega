/*
We define the domain-separated SHA-256 publication digest newtypes (`NativePublicationCertificateDigest`,
`NativePublicationEvidenceDigest`, `ExecutableContainerDigest`,
`ExecutableInstallationEvidenceDigest`, `PackageComponentDigest`, and
`NativePackageEvidenceDigest`), pre-installation preparation carriers
(`PreparedExecutablePublication` and `PreparedDynamicElfPublication`), and
self-verifying publication receipts (`ExecutablePublicationReceipt` and
`DynamicElfPublicationReceipt`). We keep Direct and Dynamic-ELF publication
domains distinct so dynamic bytes can never satisfy direct-image publication
custody.
*/

use crate::digest_encoder::DomainDigestEncoder;
use crate::{DynamicElfNativeArtifact, NativeArtifact};
use image::CompilerFunctionValidationDigest;
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

publication_digest!(NativePublicationCertificateDigest);
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn native_publication_certificate_digest(
    native_artifact_identity: &[u8; 32],
    semantic_bytes: &[u8],
    target: target::NativeTarget,
    final_image_symbol_digest: image::FinalImageSymbolDigest,
    boundary_contract_report_fingerprint: Option<u64>,
    text_validation_digest: image::CompilerTextDerivationDigest,
    function_validation_digest: CompilerFunctionValidationDigest,
    function_validation_report_fingerprint: u64,
    inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
) -> NativePublicationCertificateDigest {
    let mut digest = DomainDigestEncoder::new(b"omega.native-publication-certificate.sha256.v1\0");
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
    digest.raw(text_validation_digest.as_bytes());
    digest.raw(function_validation_digest.as_bytes());
    digest.raw(&function_validation_report_fingerprint.to_le_bytes());
    digest.raw(inventory_digest.as_bytes());
    digest.raw(&inventory_report_fingerprint.to_le_bytes());
    NativePublicationCertificateDigest::from_digest(digest.finish())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn native_publication_evidence_digest(
    native_artifact_identity: &[u8; 32],
    certificate_digest: NativePublicationCertificateDigest,
    callback_placement_identity_report_fingerprint: u64,
    inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
    text_validation_digest: image::CompilerTextDerivationDigest,
    function_validation_digest: CompilerFunctionValidationDigest,
    function_validation_report_fingerprint: u64,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
) -> NativePublicationEvidenceDigest {
    let mut digest = DomainDigestEncoder::new(b"omega.native-publication-evidence.sha256.v1\0");
    digest.raw(native_artifact_identity);
    digest.raw(certificate_digest.as_bytes());
    digest.raw(function_validation_digest.as_bytes());
    digest.raw(inventory_digest.as_bytes());
    for value in [
        callback_placement_identity_report_fingerprint,
        inventory_report_fingerprint,
        function_validation_report_fingerprint,
        container_byte_count as u64,
    ] {
        digest.raw(&value.to_le_bytes());
    }
    digest.raw(text_validation_digest.as_bytes());
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
    pub(crate) certificate_digest: NativePublicationCertificateDigest,
    pub(crate) callback_placement_identity_report_fingerprint: u64,
    pub(crate) boundary_contract_report_fingerprint: Option<u64>,
    pub(crate) inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    pub(crate) inventory_report_fingerprint: u64,
    pub(crate) compiler_text_validation_digest: image::CompilerTextDerivationDigest,
    pub(crate) compiler_function_validation_digest: CompilerFunctionValidationDigest,
    pub(crate) compiler_function_validation_report_fingerprint: u64,
    pub(crate) publication_evidence_digest: NativePublicationEvidenceDigest,
    pub(crate) container_byte_count: usize,
    pub(crate) container_digest: ExecutableContainerDigest,
    pub(crate) installation_evidence_digest: ExecutableInstallationEvidenceDigest,
}

impl ExecutablePublicationReceipt {
    pub fn new(
        output_path: PathBuf,
        native_artifact_identity: [u8; 32],
        certificate_digest: NativePublicationCertificateDigest,
        callback_placement_identity_report_fingerprint: u64,
        boundary_contract_report_fingerprint: Option<u64>,
        inventory_digest: image::PlacedExecutableRegionInventoryDigest,
        inventory_report_fingerprint: u64,
        compiler_text_validation_digest: image::CompilerTextDerivationDigest,
        compiler_function_validation_digest: CompilerFunctionValidationDigest,
        compiler_function_validation_report_fingerprint: u64,
        publication_evidence_digest: NativePublicationEvidenceDigest,
        container_byte_count: usize,
        container_digest: ExecutableContainerDigest,
        installation_evidence_digest: ExecutableInstallationEvidenceDigest,
    ) -> Self {
        Self {
            output_path,
            native_artifact_identity,
            certificate_digest,
            callback_placement_identity_report_fingerprint,
            boundary_contract_report_fingerprint,
            inventory_digest,
            inventory_report_fingerprint,
            compiler_text_validation_digest,
            compiler_function_validation_digest,
            compiler_function_validation_report_fingerprint,
            publication_evidence_digest,
            container_byte_count,
            container_digest,
            installation_evidence_digest,
        }
    }

    pub fn output_path(&self) -> &std::path::Path {
        &self.output_path
    }

    pub const fn certificate_digest(&self) -> NativePublicationCertificateDigest {
        self.certificate_digest
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

    pub const fn compiler_text_validation_digest(&self) -> image::CompilerTextDerivationDigest {
        self.compiler_text_validation_digest
    }

    pub const fn compiler_function_validation_report_fingerprint(&self) -> u64 {
        self.compiler_function_validation_report_fingerprint
    }

    pub const fn compiler_function_validation_digest(&self) -> CompilerFunctionValidationDigest {
        self.compiler_function_validation_digest
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
                self.certificate_digest,
                self.callback_placement_identity_report_fingerprint,
                self.inventory_digest,
                self.inventory_report_fingerprint,
                self.compiler_text_validation_digest,
                self.compiler_function_validation_digest,
                self.compiler_function_validation_report_fingerprint,
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
    artifact: &'artifact NativeArtifact,
    text_validation_digest: image::CompilerTextDerivationDigest,
    function_validation: image::CompilerFunctionValidationEvidence,
    container_digest: ExecutableContainerDigest,
}

impl<'artifact> PreparedExecutablePublication<'artifact> {
    pub fn new(artifact: &'artifact NativeArtifact) -> Result<Self, String> {
        let output = artifact.image().output();
        if Path::new(&output.file_name).components().count() != 1 {
            return Err("native artifact supplied a non-local output filename".to_owned());
        }
        let text_validation_digest = output
            .compiler_text_validation
            .map(|validation| validation.derivation_digest)
            .ok_or_else(|| {
                "native publication requires strong compiler-text validation evidence".to_owned()
            })?;
        let function_validation = output.compiler_function_validation.ok_or_else(|| {
            "native publication requires compiler-function validation evidence".to_owned()
        })?;
        Ok(Self {
            artifact,
            text_validation_digest,
            function_validation,
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
        let function_validation_digest = self.function_validation.evidence_digest();
        let function_validation_report_fingerprint =
            self.function_validation.evidence_report_fingerprint();
        let boundary_contract_report_fingerprint = self
            .function_validation
            .boundary_contract_report_fingerprint;
        let certificate_digest = native_publication_certificate_digest(
            &native_artifact_identity,
            artifact.semantic_bytes(),
            artifact.target(),
            artifact.image().final_image_symbol_digest(),
            boundary_contract_report_fingerprint,
            self.text_validation_digest,
            function_validation_digest,
            function_validation_report_fingerprint,
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
        );
        let publication_evidence_digest = native_publication_evidence_digest(
            &native_artifact_identity,
            certificate_digest,
            output.callback_placement_identity_report_fingerprint,
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
            self.text_validation_digest,
            function_validation_digest,
            function_validation_report_fingerprint,
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
            certificate_digest,
            output.callback_placement_identity_report_fingerprint,
            boundary_contract_report_fingerprint,
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
            self.text_validation_digest,
            function_validation_digest,
            function_validation_report_fingerprint,
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
    artifact: &'artifact DynamicElfNativeArtifact,
    text_validation_digest: image::CompilerTextDerivationDigest,
    container_digest: ExecutableContainerDigest,
}

impl<'artifact> PreparedDynamicElfPublication<'artifact> {
    pub fn new(artifact: &'artifact DynamicElfNativeArtifact) -> Result<Self, String> {
        let output = artifact.image().output();
        if Path::new(&output.file_name).components().count() != 1 {
            return Err("dynamic ELF artifact supplied a non-local output filename".to_owned());
        }
        let text_validation_digest = output
            .compiler_text_validation
            .map(|validation| validation.derivation_digest)
            .ok_or_else(|| {
                "dynamic ELF publication requires strong compiler-text validation evidence"
                    .to_owned()
            })?;
        Ok(Self {
            artifact,
            text_validation_digest,
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
            self.text_validation_digest,
            &output_path,
            container_byte_count,
            self.container_digest,
        );
        DynamicElfPublicationReceipt {
            output_path,
            dynamic_elf_artifact_identity,
            compiler_text_validation_digest: self.text_validation_digest,
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
    compiler_text_validation_digest: image::CompilerTextDerivationDigest,
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
    text_validation_digest: image::CompilerTextDerivationDigest,
    output_path: &Path,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.installed-dynamic-elf-publication-evidence.sha256.v1\0",
    );
    digest.raw(dynamic_elf_artifact_identity);
    digest.raw(text_validation_digest.as_bytes());
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
}
