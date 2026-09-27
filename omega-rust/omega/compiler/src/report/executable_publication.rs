//! Executable publication custody: the SHA-256 digest newtypes and domains,
//! native pair validation, and the self-verifying
//! [`ExecutablePublicationReceipt`] that ties published bytes to the artifact
//! they came from.

use sha2::{Digest, Sha256};
use std::path::PathBuf;

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

publication_digest!(NativePublicationCertificateDigest);
publication_digest!(NativePublicationEvidenceDigest);
publication_digest!(ExecutableContainerDigest);
publication_digest!(ExecutableInstallationEvidenceDigest);

pub fn executable_installation_evidence_digest(
    publication_evidence_digest: NativePublicationEvidenceDigest,
    callback_placement_identity_report_fingerprint: u64,
    output_path: &std::path::Path,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
) -> ExecutableInstallationEvidenceDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.installed-executable-publication-evidence.sha256.v1\0");
    digest.update(publication_evidence_digest.as_bytes());
    digest.update(callback_placement_identity_report_fingerprint.to_le_bytes());
    // The fixed v1 flat-destination tag preserves existing installation digests.
    digest.update([0]);
    let path = output_path.as_os_str().as_encoded_bytes();
    digest.update((path.len() as u64).to_le_bytes());
    digest.update(path);
    digest.update((container_byte_count as u64).to_le_bytes());
    digest.update(container_digest.as_bytes());
    ExecutableInstallationEvidenceDigest::from_digest(digest.finalize().into())
}

/// Commit the producer's retained native publication evidence. This local
/// custody commitment is not a standalone proof of native behavior.
#[allow(clippy::too_many_arguments)]
pub(crate) fn native_publication_certificate_digest(
    native_artifact_identity: &[u8; 32],
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    target: target::NativeTarget,
    final_image_symbol_digest: image::FinalImageSymbolDigest,
    boundary_contract_report_fingerprint: Option<u64>,
    text_validation_digest: image::CompilerTextDerivationDigest,
    function_validation_digest: image::CompilerFunctionValidationDigest,
    function_validation_report_fingerprint: u64,
    inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
) -> NativePublicationCertificateDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-publication-certificate.sha256.v1\0");
    digest.update(native_artifact_identity);
    digest.update((semantic_bytes.len() as u64).to_le_bytes());
    digest.update(semantic_bytes);
    digest.update((proof_bytes.len() as u64).to_le_bytes());
    digest.update(proof_bytes);
    digest.update([match target.architecture {
        target::Architecture::Aarch64 => 1,
        target::Architecture::X86_64 => 2,
    }]);
    digest.update([match target.object_format {
        target::ObjectFormat::Elf => 1,
        target::ObjectFormat::MachO => 2,
        target::ObjectFormat::Coff => 3,
    }]);
    digest.update((target.pointer_size as u64).to_le_bytes());
    digest.update((target.pointer_alignment as u64).to_le_bytes());
    digest.update(final_image_symbol_digest.as_bytes());
    digest.update([u8::from(boundary_contract_report_fingerprint.is_some())]);
    digest.update(
        boundary_contract_report_fingerprint
            .unwrap_or_default()
            .to_le_bytes(),
    );
    digest.update(text_validation_digest.as_bytes());
    digest.update(function_validation_digest.as_bytes());
    digest.update(function_validation_report_fingerprint.to_le_bytes());
    digest.update(inventory_digest.as_bytes());
    digest.update(inventory_report_fingerprint.to_le_bytes());
    NativePublicationCertificateDigest::from_digest(digest.finalize().into())
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
    let mut digest = Sha256::new();
    digest.update(b"omega.native-publication-evidence.sha256.v1\0");
    digest.update(native_artifact_identity);
    digest.update(certificate_digest.as_bytes());
    digest.update(function_validation_digest.as_bytes());
    digest.update(inventory_digest.as_bytes());
    for value in [
        callback_placement_identity_report_fingerprint,
        inventory_report_fingerprint,
        function_validation_report_fingerprint,
        container_byte_count as u64,
    ] {
        digest.update(value.to_le_bytes());
    }
    digest.update(text_validation_digest.as_bytes());
    digest.update(container_digest.as_bytes());
    NativePublicationEvidenceDigest::from_digest(digest.finalize().into())
}

pub(crate) fn executable_container_digest(bytes: &[u8]) -> ExecutableContainerDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.published-executable-container.sha256.v1\0");
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    ExecutableContainerDigest::from_digest(digest.finalize().into())
}

/// Producer-side pair validation for one native executable/companion: replay
/// the exact checks a receiver performs under a self-consistent policy so a
/// malformed or byte-inconsistent pair can never be reported as published.
/// The bounded placed-image evidence must decode and replay against the
/// published bytes. `Incomplete(UnsupportedEvidence)` is the honest ceiling —
/// the coverage leg validates while the behavioral legs have no standalone
/// checking yet — but `Reject` or a named resource limit means the emitted
/// evidence cannot carry even its bounded claim, and refuses publication.
pub(crate) fn validate_native_pair(
    executable_bytes: &[u8],
    sidecar: &terminal_codec::PccProofSidecar,
    admission_profile: &proof_admission::AdmissionProfile,
) -> Result<(), String> {
    let policy =
        terminal_codec::PccReceiverPolicy::for_offered_claim(sidecar, admission_profile.clone());
    match native_artifact::verify_native_proof_sidecar(
        executable_bytes,
        &sidecar.to_bytes(),
        &policy,
    ) {
        terminal_codec::PccVerificationOutcome::Complete(_)
        | terminal_codec::PccVerificationOutcome::Incomplete(
            terminal_codec::PccIncompleteness::UnsupportedEvidence {
                product: terminal_codec::PccProductKind::Native,
            },
        ) => Ok(()),
        outcome => Err(format!(
            "native proof sidecar failed producer validation: {outcome:?}"
        )),
    }
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

    /// Replay the published file against this receipt: length, content digest
    /// and executable mode must all still match — the publication contract's
    /// byte- and mode-drift detection.
    pub fn validate_published_file(&self) -> Result<(), String> {
        let bytes = std::fs::read(&self.output_path).map_err(|error| {
            format!(
                "cannot replay published executable {}: {error}",
                self.output_path.display()
            )
        })?;
        if bytes.len() != self.container_byte_count {
            return Err(format!(
                "published executable {} has {} bytes; the receipt recorded {}",
                self.output_path.display(),
                bytes.len(),
                self.container_byte_count
            ));
        }
        if executable_container_digest(&bytes) != self.container_digest {
            return Err(format!(
                "published executable {} no longer matches its receipt's content digest",
                self.output_path.display()
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = std::fs::metadata(&self.output_path)
                .map_err(|error| {
                    format!(
                        "cannot stat published executable {}: {error}",
                        self.output_path.display()
                    )
                })?
                .permissions()
                .mode();
            if mode & 0o777 != 0o755 {
                return Err(format!(
                    "published executable {} drifted to mode {:o}; the receipt expects 755",
                    self.output_path.display(),
                    mode & 0o7777
                ));
            }
        }
        Ok(())
    }
}
