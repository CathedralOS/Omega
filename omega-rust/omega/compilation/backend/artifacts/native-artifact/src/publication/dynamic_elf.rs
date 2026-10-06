//! Flat-file publication custody for one dynamically linked ELF product.
//!
//! The dynamic product stays authority-distinct from [`crate::NativeArtifact`]:
//! its receipt names the exact [`DynamicElfNativeArtifact`] identity, the
//! admitted text-derivation evidence, the container bytes (which carry the
//! configured `PT_INTERP`), and the installed path. Writing the file is
//! ordinary artifact production
//! (wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission);
//! it grants no runtime installation, loader-policy, or execution authority,
//! and the receipt cannot stand in for [`super::ExecutablePublicationReceipt`].

use super::{ExecutableContainerDigest, executable_container_digest};
use crate::DynamicElfNativeArtifact;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Publication evidence admitted before any destination is modified.
///
/// Borrows the exact dynamic artifact throughout installation, so the final
/// receipt cannot combine a different image with the admitted evidence. File
/// staging and installation remain the caller's responsibility.
pub struct PreparedDynamicElfPublication<'artifact> {
    artifact: &'artifact DynamicElfNativeArtifact,
    text_validation_digest: image::CompilerTextDerivationDigest,
    container_digest: ExecutableContainerDigest,
}

impl<'artifact> PreparedDynamicElfPublication<'artifact> {
    /// Admit the local output name and the text-derivation evidence the
    /// dynamic writer retained. The artifact was sealed by the compile that
    /// publishes it and is not replayed here, matching the direct route.
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

    /// Bind the admitted dynamic image to its installed destination.
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

/// Immutable custody for one compiler-published dynamically linked ELF file.
///
/// Records the exact dynamic artifact, its admitted text evidence, and the
/// checked installation that produced `output_path`. It is compiler artifact
/// evidence only; it does not authorize loading the interpreter it names.
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

    /// Recompute the installation commitment from the retained fields.
    pub fn has_consistent_installation_identity(&self) -> bool {
        self.installation_evidence_digest
            == dynamic_elf_installation_evidence_digest(
                &self.dynamic_elf_artifact_identity,
                self.compiler_text_validation_digest,
                &self.output_path,
                self.container_byte_count,
                self.container_digest,
            )
    }
}

/// Domain-separated installation commitment. The dynamic domain keeps equal
/// bytes at an equal path from colliding with a direct-image receipt.
fn dynamic_elf_installation_evidence_digest(
    dynamic_elf_artifact_identity: &[u8; 32],
    text_validation_digest: image::CompilerTextDerivationDigest,
    output_path: &Path,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.installed-dynamic-elf-publication-evidence.sha256.v1\0");
    digest.update(dynamic_elf_artifact_identity);
    digest.update(text_validation_digest.as_bytes());
    let path = output_path.as_os_str().as_encoded_bytes();
    digest.update((path.len() as u64).to_le_bytes());
    digest.update(path);
    digest.update((container_byte_count as u64).to_le_bytes());
    digest.update(container_digest.as_bytes());
    digest.finalize().into()
}
