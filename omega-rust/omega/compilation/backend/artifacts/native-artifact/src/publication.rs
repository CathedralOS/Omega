//! Executable publication custody: the SHA-256 digest newtypes and domains,
//! native pair validation, and the self-verifying
//! [`ExecutablePublicationReceipt`] that ties published bytes to the artifact
//! they came from.
//! `PreparedExecutablePublication` admits the artifact's publication evidence
//! before file installation and binds the resulting receipt to its destination.
//! It does not write files or grant permission to execute them.

use std::path::PathBuf;

mod dynamic_elf;
mod preparation;
pub use dynamic_elf::{DynamicElfPublicationReceipt, PreparedDynamicElfPublication};
pub use preparation::PreparedExecutablePublication;

#[cfg(test)]
mod tests;

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
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.installed-executable-publication-evidence.sha256.v1\0",
    );
    digest.raw(publication_evidence_digest.as_bytes());
    digest.raw(&callback_placement_identity_report_fingerprint.to_le_bytes());
    // The fixed v1 flat-destination tag preserves existing installation digests.
    digest.byte(0);
    let path = output_path.as_os_str().as_encoded_bytes();
    digest.u64(path.len() as u64);
    digest.raw(&path);
    digest.u64(container_byte_count as u64);
    digest.raw(container_digest.as_bytes());
    ExecutableInstallationEvidenceDigest::from_digest(digest.finish())
}

/// Commit the producer's retained native publication evidence. This local
/// custody commitment is not a standalone proof of native behavior.
#[allow(clippy::too_many_arguments)]
pub(crate) fn native_publication_certificate_digest(
    native_artifact_identity: &[u8; 32],
    semantic_bytes: &[u8],
    target: target::NativeTarget,
    final_image_symbol_digest: image::FinalImageSymbolDigest,
    boundary_contract_report_fingerprint: Option<u64>,
    text_validation_digest: image::CompilerTextDerivationDigest,
    function_validation_digest: image::CompilerFunctionValidationDigest,
    function_validation_report_fingerprint: u64,
    inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
) -> NativePublicationCertificateDigest {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-publication-certificate.sha256.v1\0",
    );
    digest.raw(native_artifact_identity);
    digest.u64(semantic_bytes.len() as u64);
    digest.raw(&semantic_bytes);
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
    function_validation_digest: image::CompilerFunctionValidationDigest,
    function_validation_report_fingerprint: u64,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
) -> NativePublicationEvidenceDigest {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-publication-evidence.sha256.v1\0",
    );
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

pub(crate) fn executable_container_digest(bytes: &[u8]) -> ExecutableContainerDigest {
    ExecutableContainerDigest::from_digest(crate::digest_encoder::DomainDigestEncoder::content(
        b"omega.published-executable-container.sha256.v1\0",
        bytes,
    ))
}

/// Immutable custody for one compiler-published executable container.
///
/// This records the exact final-footprint certificate, publication seal, and
/// checked atomic installation that produced `output_path`. It is compiler
/// artifact evidence only; it does not authorize loading or runtime
/// installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutablePublicationReceipt {
    pub(crate) output_path: PathBuf,
    pub(crate) native_artifact_identity: [u8; 32],
    pub(crate) certificate_digest: NativePublicationCertificateDigest,
    /// Compact report coordinate. Exact callback placements are structurally
    /// replayed before this publication receipt can be produced.
    pub(crate) callback_placement_identity_report_fingerprint: u64,
    /// Compact report coordinate; exact final footprint authority is retained
    /// by the placed-region inventory and certificate digest. Crate-visible
    /// like its siblings for report-custody tests.
    pub(crate) boundary_contract_report_fingerprint: Option<u64>,
    pub(crate) inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    /// Compact report compatibility only. Publication and replay authority is
    /// `inventory_digest`.
    pub(crate) inventory_report_fingerprint: u64,
    pub(crate) compiler_text_validation_digest: image::CompilerTextDerivationDigest,
    pub(crate) compiler_function_validation_digest: image::CompilerFunctionValidationDigest,
    /// Compact report compatibility only. The validation digest preserves this
    /// summary's custody, while exact text and inventory commitments retain the
    /// underlying publication and replay authority.
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
        compiler_function_validation_digest: image::CompilerFunctionValidationDigest,
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

    pub const fn compiler_function_validation_digest(
        &self,
    ) -> image::CompilerFunctionValidationDigest {
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
