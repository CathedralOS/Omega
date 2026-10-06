use super::{
    ExecutableContainerDigest, ExecutablePublicationReceipt, executable_container_digest,
    executable_installation_evidence_digest, native_publication_certificate_digest,
    native_publication_evidence_digest,
};
use crate::NativeArtifact;
use std::path::{Path, PathBuf};

/// Publication evidence admitted before any destination is modified.
///
/// Borrows the exact artifact throughout installation, so the final receipt
/// cannot accidentally combine a different image with the admitted evidence.
/// File staging and installation remain the caller's responsibility.
pub struct PreparedExecutablePublication<'artifact> {
    artifact: &'artifact NativeArtifact,
    text_validation_digest: image::CompilerTextDerivationDigest,
    function_validation: image::CompilerFunctionValidationEvidence,
    container_digest: ExecutableContainerDigest,
}

impl<'artifact> PreparedExecutablePublication<'artifact> {
    /// Admit the local output name and required text/function evidence.
    /// This supplements the artifact's ordinary validation; it does not
    /// replace independent artifact replay or receiver admission.
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

    /// Bind the admitted image and its evidence to the installed destination.
    /// The receipt records producer custody, not a proof of native behavior.
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
