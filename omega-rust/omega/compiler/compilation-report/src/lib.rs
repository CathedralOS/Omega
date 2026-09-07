//! The compiler's outbound custody record: what a compilation produced, and
//! the digest chain tying bytes on disk back to the artifact they came from.
//!
//! A `CompileReport` is one of five products - `CheckOnly`, `TerminalArtifact`,
//! `RetainedNativeArtifact`, `NativeExecutable`, `ObjectContainer` - and each
//! kind fixes which payload slots may be occupied.
//! `has_consistent_executable_publication_custody` checks that cardinality and
//! validates the retained artifact or flat executable receipt.
//!
//! Four SHA-256 domains carry the chain, each prefix NUL-terminated so no
//! prefix can be a prefix of another, and each carrying a `.v1` suffix that
//! makes a future change a new domain rather than a silent reinterpretation:
//!
//! ```text
//!   artifact identity + target + image symbols + text/function validation
//!        -> omega.native-publication-certificate.sha256.v1
//!   certificate + validation digests + fingerprints + container
//!        -> omega.native-publication-evidence.sha256.v1
//!   the published bytes, length-prefixed
//!        -> omega.published-executable-container.sha256.v1
//!   evidence + destination tag + output path + container
//!        -> omega.installed-executable-publication-evidence.sha256.v1
//! ```
//!
//! A receipt verifies itself. `has_consistent_installation_identity` recomputes
//! the evidence and installation digests from the receipt's own fields and
//! compares, so a receipt with one field edited stops matching. Note the one
//! field that does not appear in either recomputation directly:
//! `boundary_contract_report_fingerprint` reaches the chain only through the
//! certificate digest, so it is covered transitively rather than by name.
//!
//! `native_publication_evidence_digest` does not feed its parameters in
//! parameter order. Four `u64`s go through one loop in the order
//! `[callback_placement_identity_report_fingerprint, inventory_report_fingerprint,
//! function_validation_report_fingerprint, container_byte_count]`, and the
//! grouping is load-bearing: reorder that loop and every receipt ever stored
//! stops replaying.
//!
//! `publish_exact_executable_bytes` writes once and reads back twice. It stages
//! to `.{file_name}.{process_id}.tmp` beside the target, reads the staged file
//! and compares byte-for-byte, removes any existing target, renames, sets mode
//! 0o755 on unix, then reads the installed file and compares byte-for-byte
//! again. Either replay failure deletes the file and returns an error, so a
//! failed publication leaves nothing behind that looks installed.

//! The four 32-byte digest newtypes are minted by a macro with four identical
//! bodies rather than sharing one `Digest([u8; 32])`, and the duplication is
//! the point. `executable_installation_evidence_digest` takes both a
//! `NativePublicationEvidenceDigest` and an `ExecutableContainerDigest`; under
//! one shared type, passing them in the wrong order compiles and produces a
//! plausible digest that nothing will ever reproduce. Four types make that a
//! type error, and the macro is what keeps the cost to four lines.
//!
//! Publication is a consuming method that returns a new report, not a compiler
//! request kind. Compilation is over by the time `publish_retained_native_artifact`
//! runs; path selection and filesystem mutation are a product operation, and
//! routing them back through the driver is what the deleted legacy route did.
//!
//! Every constructor ends by replaying custody on the report it just built and
//! returns `Err` if it fails, so a `CompileReport` that exists is one whose
//! custody already passed. That is why the accessors can be cheap and why
//! `checked_native_executable_path` can afford to replay again anyway.

//! `omega/src/command/output.rs:12` is the only production caller of
//! `publish_retained_native_artifact`; nine canary tests call it too.
//! `compiler/src/compiler/native_checked.rs:23` calls the custody check.
//! Custody tests reject rollback on check-only products and receipt drift
//! in flat executable publication. Terminal rollback additionally rejoins the
//! effective selection to the published Psi execution and pending proposal.
//!
//! `ProductionCompilationSubject::from_checked` is `pub` and `#[doc(hidden)]`
//! rather than `pub(crate)` because its only caller lives in another crate, at
//! `compiler/src/pipeline/reporting/production_subject.rs:36`.
//!
//! @Incomplete: publication currently installs flat executables only.
//! Whole macOS application packages require the executable, plist, and exact
//! directory shape to be validated together, as specified in
//! `wiki/design_briefs/macos_application_publication.md`. A second executable
//! receipt would not establish that contract.

use sha2::{Digest, Sha256};
use std::path::PathBuf;

mod optimization_rollback;
mod production_manifest;
mod terminal_product;
pub use optimization_rollback::OptimizationRollbackReceipt;
pub use production_manifest::{
    FinalRealizationEvidenceError, ProductionArtifactIdentity, ProductionCompilationManifest,
    ProductionCompilationManifestIdentity, ProductionCompilationSubject,
};
pub use terminal_product::{
    RetainedTerminalArtifact, TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact,
    TerminalCompilerBuiltinProposal, TerminalIeeeFloatFmaOccurrenceProposal,
    TerminalNativeRealizationProposal, TerminalX86ScalarFmaAdmission,
};

/// Complete non-clonable Terminal-Psi native artifact retained before output
/// publication. The compatibility name remains while callers migrate from the
/// former legacy `EmissionPlan + EmittedProgram` payload.
pub use native_artifact::NativeArtifact as RetainedNativeArtifact;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileOutputKind {
    CheckOnly,
    TerminalArtifact,
    RetainedNativeArtifact,
    NativeExecutable,
    ObjectContainer,
}

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

fn publication_boundary_contract_report_fingerprint(
    validation: image::CompilerFunctionValidationEvidence,
) -> Result<Option<u64>, String> {
    let body = (validation.body_specification_instruction_count > 0)
        .then_some(validation.body_specification_boundary_contract_report_fingerprint);
    let mechanics = (validation.fixed_mechanics_instruction_count > 0)
        .then_some(validation.fixed_mechanics_boundary_contract_report_fingerprint);
    match (body, mechanics) {
        (Some(left), Some(right)) if left != right => {
            Err("native artifact final validation names inconsistent boundary contracts".to_owned())
        }
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn native_publication_certificate_digest(
    artifact: &RetainedNativeArtifact,
    boundary_contract_report_fingerprint: Option<u64>,
    text_validation_digest: image::CompilerTextDerivationDigest,
    function_validation_digest: image::CompilerFunctionValidationDigest,
    function_validation_report_fingerprint: u64,
    inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
) -> NativePublicationCertificateDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-publication-certificate.sha256.v1\0");
    digest.update(artifact.identity().as_bytes());
    digest.update((artifact.semantic_bytes().len() as u64).to_le_bytes());
    digest.update(artifact.semantic_bytes());
    digest.update((artifact.proof_bytes().len() as u64).to_le_bytes());
    digest.update(artifact.proof_bytes());
    let target = artifact.target();
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
    digest.update(artifact.image().final_image_symbol_digest().as_bytes());
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

fn native_publication_evidence_digest(
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

fn executable_container_digest(bytes: &[u8]) -> ExecutableContainerDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.published-executable-container.sha256.v1\0");
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    ExecutableContainerDigest::from_digest(digest.finalize().into())
}

fn publish_exact_executable_bytes(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "native publication path has no parent directory".to_owned())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("omega-output");
    let staged = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&staged);
    std::fs::write(&staged, bytes)
        .map_err(|error| format!("failed to stage {}: {error}", staged.display()))?;
    let staged_bytes = std::fs::read(&staged)
        .map_err(|error| format!("failed to replay {}: {error}", staged.display()))?;
    if staged_bytes != bytes {
        let _ = std::fs::remove_file(&staged);
        return Err("staged native output bytes failed exact replay".to_owned());
    }
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|error| format!("failed to replace {}: {error}", path.display()))?;
    }
    std::fs::rename(&staged, path)
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))?;
    make_executable(path)?;
    let installed = std::fs::read(path)
        .map_err(|error| format!("failed to replay {}: {error}", path.display()))?;
    if installed != bytes {
        let _ = std::fs::remove_file(path);
        return Err("published native output bytes failed exact replay".to_owned());
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("failed to make {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}

/// Immutable custody for one compiler-published executable container.
///
/// This records the exact final-footprint certificate, publication seal, and
/// checked atomic installation that produced `output_path`. It is compiler
/// artifact evidence only; it does not authorize loading or runtime
/// installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutablePublicationReceipt {
    output_path: PathBuf,
    native_artifact_identity: [u8; 32],
    certificate_digest: NativePublicationCertificateDigest,
    /// Compact report coordinate. Exact callback placements are structurally
    /// replayed before this publication receipt can be produced.
    callback_placement_identity_report_fingerprint: u64,
    /// Compact report coordinate; exact final footprint authority is retained
    /// by the placed-region inventory and certificate digest.
    boundary_contract_report_fingerprint: Option<u64>,
    inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    /// Compact report compatibility only. Publication and replay authority is
    /// `inventory_digest`.
    inventory_report_fingerprint: u64,
    compiler_text_validation_digest: image::CompilerTextDerivationDigest,
    compiler_function_validation_digest: image::CompilerFunctionValidationDigest,
    /// Compact report compatibility only. The validation digest preserves this
    /// summary's custody, while exact text and inventory commitments retain the
    /// underlying publication and replay authority.
    compiler_function_validation_report_fingerprint: u64,
    publication_evidence_digest: NativePublicationEvidenceDigest,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
    installation_evidence_digest: ExecutableInstallationEvidenceDigest,
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

#[derive(Debug)]
pub struct CompileReport {
    root_path: PathBuf,
    pub source_file_count: usize,
    wrote_output: bool,
    /// Exact output category selected by orchestration. This distinguishes a
    /// native executable, which requires publication custody, from the
    /// non-executable object-container fallback.
    output_kind: CompileOutputKind,
    /// Complete validated native payload retained before any output or runtime
    /// installation. Exactly the retained-native output kind owns this value.
    retained_native_artifact: Option<RetainedNativeArtifact>,
    /// Canonical source-free Psi artifact retained at the exact Psi/Omega
    /// ownership seam. It carries no target or deployment authority.
    artifact: Option<RetainedTerminalArtifact>,
    /// Exact checked publication receipt for a native executable image.
    /// Object-container fallbacks and check-only compilations retain `None`.
    executable_publication: Option<ExecutablePublicationReceipt>,
    /// Exact subtractive release overlay applied after build selection and
    /// before native realization. Ordinary requests retain `None`.
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    /// Canonical package-source/build/target/artifact authority for a
    /// package-aware production. Standalone probes carry no such manifest.
    production_manifest: Option<ProductionCompilationManifest>,
    /// Filesystem-free comparison between the request's explicit admissions
    /// and every trust obligation reconstructed by compilation.
    trust_admission_settlement: trust_model::TrustAdmissionSettlement,
}

impl CompileReport {
    pub fn checked(
        root_path: PathBuf,
        source_file_count: usize,
        wrote_output: bool,
        output_kind: CompileOutputKind,
        executable_publication: Option<ExecutablePublicationReceipt>,
    ) -> Result<Self, &'static str> {
        let report = Self {
            root_path,
            source_file_count,
            wrote_output,
            output_kind,
            retained_native_artifact: None,
            artifact: None,
            executable_publication,
            optimization_rollback: None,
            production_manifest: None,
            trust_admission_settlement: Default::default(),
        };
        if report.has_consistent_executable_publication_custody() {
            Ok(report)
        } else {
            Err("compiler report retained inconsistent executable publication receipts")
        }
    }

    pub fn from_retained_native_artifact(
        root_path: PathBuf,
        source_file_count: usize,
        artifact: RetainedNativeArtifact,
        optimization_rollback: Option<OptimizationRollbackReceipt>,
        production_subject: Option<ProductionCompilationSubject>,
    ) -> Result<Self, &'static str> {
        artifact
            .validate()
            .map_err(|_| "compiler report received an invalid native artifact")?;
        let production_manifest = production_subject
            .map(|subject| ProductionCompilationManifest::for_native(subject, &artifact))
            .transpose()?;
        let report = Self {
            root_path,
            source_file_count,
            wrote_output: false,
            output_kind: CompileOutputKind::RetainedNativeArtifact,
            retained_native_artifact: Some(artifact),
            artifact: None,
            executable_publication: None,
            optimization_rollback,
            production_manifest,
            trust_admission_settlement: Default::default(),
        };
        if !report.has_consistent_executable_publication_custody() {
            return Err("compiler report retained inconsistent native-artifact custody");
        }
        Ok(report)
    }

    /// Publish the retained native product as one flat executable and replace
    /// pre-publication custody with an exact publication receipt.
    ///
    /// Compilation ends before this operation. Path selection and filesystem
    /// mutation are an explicit product operation, never another compiler
    /// request route.
    pub fn publish_retained_native_artifact(
        self,
        build_dir: &std::path::Path,
    ) -> Result<Self, String> {
        if self.output_kind != CompileOutputKind::RetainedNativeArtifact
            || self.wrote_output
            || self.artifact.is_some()
            || self.executable_publication.is_some()
        {
            return Err(
                "native publication requires exactly one retained native artifact".to_owned(),
            );
        }
        let artifact = self.retained_native_artifact.as_ref().ok_or_else(|| {
            "native publication requires exactly one retained native artifact".to_owned()
        })?;
        artifact
            .validate()
            .map_err(|error| format!("refusing to publish an invalid native artifact: {error}"))?;
        if self
            .production_manifest
            .as_ref()
            .is_some_and(|manifest| !manifest.matches_native_artifact(artifact))
        {
            return Err("native publication manifest disagrees with retained artifact".to_owned());
        }
        let native_artifact_identity = *artifact.identity().as_bytes();

        let output = artifact.image().output();
        if std::path::Path::new(&output.file_name).components().count() != 1 {
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
        let function_validation_digest = function_validation.evidence_digest();
        let function_validation_report_fingerprint =
            function_validation.evidence_report_fingerprint();
        let boundary_contract_report_fingerprint = output
            .compiler_function_validation
            .map(publication_boundary_contract_report_fingerprint)
            .transpose()?
            .flatten();

        std::fs::create_dir_all(build_dir).map_err(|error| {
            format!(
                "failed to create output directory {}: {error}",
                build_dir.display()
            )
        })?;
        let output_path = build_dir.join(&output.file_name);
        publish_exact_executable_bytes(&output_path, &output.bytes)?;

        let container_digest = executable_container_digest(&output.bytes);
        let certificate_digest = native_publication_certificate_digest(
            artifact,
            boundary_contract_report_fingerprint,
            text_validation_digest,
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
            text_validation_digest,
            function_validation_digest,
            function_validation_report_fingerprint,
            output.bytes.len(),
            container_digest,
        );
        let installation_evidence_digest = executable_installation_evidence_digest(
            publication_evidence_digest,
            output.callback_placement_identity_report_fingerprint,
            &output_path,
            output.bytes.len(),
            container_digest,
        );
        let receipt = ExecutablePublicationReceipt::new(
            output_path,
            native_artifact_identity,
            certificate_digest,
            output.callback_placement_identity_report_fingerprint,
            boundary_contract_report_fingerprint,
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
            text_validation_digest,
            function_validation_digest,
            function_validation_report_fingerprint,
            publication_evidence_digest,
            output.bytes.len(),
            container_digest,
            installation_evidence_digest,
        );
        if !receipt.has_consistent_installation_identity() {
            return Err("native publication produced an inconsistent installation receipt".into());
        }

        let report = Self {
            root_path: self.root_path,
            source_file_count: self.source_file_count,
            wrote_output: true,
            output_kind: CompileOutputKind::NativeExecutable,
            retained_native_artifact: None,
            artifact: None,
            executable_publication: Some(receipt),
            optimization_rollback: self.optimization_rollback,
            production_manifest: self.production_manifest,
            trust_admission_settlement: self.trust_admission_settlement,
        };
        if !report.has_consistent_executable_publication_custody() {
            return Err("published native report failed custody replay".to_owned());
        }
        Ok(report)
    }

    pub fn root_path(&self) -> &std::path::Path {
        &self.root_path
    }

    pub const fn wrote_output(&self) -> bool {
        self.wrote_output
    }

    pub const fn output_kind(&self) -> CompileOutputKind {
        self.output_kind
    }

    pub const fn retained_native_artifact(&self) -> Option<&RetainedNativeArtifact> {
        self.retained_native_artifact.as_ref()
    }

    pub const fn optimization_rollback_receipt(&self) -> Option<&OptimizationRollbackReceipt> {
        self.optimization_rollback.as_ref()
    }

    pub const fn production_manifest(&self) -> Option<&ProductionCompilationManifest> {
        self.production_manifest.as_ref()
    }

    /// Require the exact native physical evidence for this package-aware,
    /// retained-native production. Standalone and already-published reports do
    /// not carry the package/artifact join needed to make this claim.
    pub fn require_package_native_physical_evidence(
        &self,
    ) -> Result<&native_artifact::NativePhysicalEvidence, FinalRealizationEvidenceError> {
        if !self.has_consistent_executable_publication_custody() {
            return Err(FinalRealizationEvidenceError::InvalidReportCustody);
        }
        if self.output_kind != CompileOutputKind::RetainedNativeArtifact {
            return Err(FinalRealizationEvidenceError::RetainedNativeArtifactRequired);
        }
        let artifact = self
            .retained_native_artifact
            .as_ref()
            .ok_or(FinalRealizationEvidenceError::RetainedNativeArtifactRequired)?;
        let manifest = self
            .production_manifest
            .as_ref()
            .ok_or(FinalRealizationEvidenceError::PackageProductionManifestRequired)?;
        manifest.require_native_physical_evidence(artifact)
    }

    pub fn with_trust_admission_settlement(
        mut self,
        settlement: trust_model::TrustAdmissionSettlement,
    ) -> Self {
        self.trust_admission_settlement = settlement;
        self
    }

    pub const fn trust_admission_settlement(&self) -> &trust_model::TrustAdmissionSettlement {
        &self.trust_admission_settlement
    }

    pub fn from_retained_terminal_artifact(
        root_path: PathBuf,
        source_file_count: usize,
        artifact: RetainedTerminalArtifact,
        production_subject: Option<ProductionCompilationSubject>,
    ) -> Result<Self, &'static str> {
        if artifact.native_realization_proposal().is_none() {
            return Err(
                "compiler report requires the Terminal product's native realization proposal",
            );
        }
        artifact
            .validate()
            .map_err(|_| "compiler report received an invalid retained Terminal product")?;
        let production_manifest = production_subject
            .map(|subject| {
                ProductionCompilationManifest::for_terminal(subject, artifact.artifact())
            })
            .transpose()?;
        let report = Self {
            root_path,
            source_file_count,
            wrote_output: false,
            output_kind: CompileOutputKind::TerminalArtifact,
            retained_native_artifact: None,
            artifact: Some(artifact),
            executable_publication: None,
            optimization_rollback: None,
            production_manifest,
            trust_admission_settlement: Default::default(),
        };
        report
            .has_consistent_executable_publication_custody()
            .then_some(report)
            .ok_or("compiler report retained inconsistent Terminal-artifact custody")
    }

    pub const fn artifact(&self) -> Option<&terminal_codec::CanonicalTerminalArtifact> {
        match &self.artifact {
            Some(retained) => Some(retained.artifact()),
            None => None,
        }
    }

    /// Attach the subtractive build overlay only when the published Psi and
    /// pending native proposal carry its exact effective selection.
    pub fn with_terminal_optimization_rollback(
        mut self,
        rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<Self, &'static str> {
        if self.output_kind != CompileOutputKind::TerminalArtifact {
            return Err("Terminal rollback requires a Terminal product");
        }
        self.optimization_rollback = rollback;
        self.has_consistent_executable_publication_custody()
            .then_some(self)
            .ok_or("Terminal rollback does not match the published optimization selection")
    }

    pub fn terminal_callback_placements(
        &self,
    ) -> Option<&[backend_plan::BoundNominalCallbackPlacement]> {
        self.artifact
            .as_ref()
            .map(RetainedTerminalArtifact::callback_placements)
    }

    /// Borrow the target-constrained native proposal without detaching it from
    /// the retained Terminal product or its report custody.
    pub fn terminal_native_realization_proposal(
        &self,
    ) -> Option<&TerminalNativeRealizationProposal> {
        self.artifact
            .as_ref()
            .and_then(RetainedTerminalArtifact::native_realization_proposal)
    }

    /// Transfer the complete Terminal product without dropping its callback
    /// sidecar. There is deliberately no consuming artifact-only projection.
    pub fn into_retained_terminal_artifact(self) -> Option<RetainedTerminalArtifact> {
        self.artifact
    }

    /// Transfer the complete non-clonable pre-publication native payload out
    /// of this report. Other requested products return `None`.
    pub fn into_retained_native_artifact(self) -> Option<RetainedNativeArtifact> {
        self.retained_native_artifact
    }

    pub fn executable_publication(&self) -> Option<&ExecutablePublicationReceipt> {
        self.executable_publication.as_ref()
    }

    /// Returns the exact installed flat executable only after independently
    /// replaying the complete report custody checks. Object/check-only reports
    /// and any internally drifted receipt graph fail closed.
    pub fn checked_native_executable_path(&self) -> Option<&std::path::Path> {
        if self.output_kind != CompileOutputKind::NativeExecutable
            || !self.has_consistent_executable_publication_custody()
        {
            return None;
        }
        self.executable_publication
            .as_ref()
            .map(ExecutablePublicationReceipt::output_path)
    }

    /// Replays exact output-product cardinality. A retained native artifact is
    /// mutually exclusive with publication, native output checks the flat
    /// executable receipt, and terminal output replays
    /// the retained installation/image/file join.
    pub fn has_consistent_executable_publication_custody(&self) -> bool {
        let rollback_matches_kind = match self.output_kind {
            CompileOutputKind::RetainedNativeArtifact | CompileOutputKind::NativeExecutable => self
                .optimization_rollback
                .as_ref()
                .is_none_or(OptimizationRollbackReceipt::is_consistent),
            CompileOutputKind::TerminalArtifact => {
                self.optimization_rollback.as_ref().is_none_or(|receipt| {
                    receipt.is_consistent()
                        && receipt.requested_disabled().as_slice().iter().all(|rule| {
                            rule.execution_phase()
                                == optimization_core::OptimizationExecutionPhase::Psi
                        })
                        && self.artifact().is_some_and(|artifact| {
                            artifact.optimization().selection()
                                == receipt.effective().project_psi().selections().identity()
                        })
                        && self
                            .terminal_native_realization_proposal()
                            .is_some_and(|proposal| {
                                proposal.post_terminal_optimizations()
                                    == &receipt.effective().project_post_terminal()
                            })
                })
            }
            CompileOutputKind::CheckOnly | CompileOutputKind::ObjectContainer => {
                self.optimization_rollback.is_none()
            }
        };
        if !rollback_matches_kind {
            return false;
        }
        if self
            .production_manifest
            .as_ref()
            .is_some_and(|manifest| !manifest.validate())
        {
            return false;
        }
        match self.output_kind {
            CompileOutputKind::CheckOnly => {
                !self.wrote_output
                    && self.artifact.is_none()
                    && self.retained_native_artifact.is_none()
                    && self.executable_publication.is_none()
            }
            CompileOutputKind::TerminalArtifact => {
                !self.wrote_output
                    && self.retained_native_artifact.is_none()
                    && self
                        .artifact
                        .as_ref()
                        .is_some_and(|artifact| artifact.validate().is_ok())
                    && self.executable_publication.is_none()
                    && self.production_manifest.as_ref().is_none_or(|manifest| {
                        self.artifact
                            .as_ref()
                            .is_some_and(|artifact| {
                                manifest.matches_terminal_artifact(artifact.artifact())
                                    && artifact.native_realization_proposal().is_some_and(
                                        |proposal| {
                                            proposal.target_profile()
                                                == manifest.subject().target_profile()
                                                && proposal.native_target()
                                                    == manifest.subject().native_target()
                                        },
                                    )
                            })
                    })
            }
            CompileOutputKind::RetainedNativeArtifact => {
                !self.wrote_output
                    && self.artifact.is_none()
                    && self
                        .retained_native_artifact
                        .as_ref()
                        .is_some_and(|artifact| artifact.validate().is_ok())
                    && self.executable_publication.is_none()
                    && self.production_manifest.as_ref().is_none_or(|manifest| {
                        self.retained_native_artifact
                            .as_ref()
                            .is_some_and(|artifact| manifest.matches_native_artifact(artifact))
                    })
            }
            CompileOutputKind::NativeExecutable => {
                self.wrote_output
                    && self.artifact.is_none()
                    && self.retained_native_artifact.is_none()
                    && self.executable_publication.as_ref().is_some_and(|receipt| {
                        receipt.has_consistent_installation_identity()
                    })
                    && self.production_manifest.as_ref().is_none_or(|manifest| {
                        matches!(manifest.artifact(), ProductionArtifactIdentity::Native(identity)
                            if self.executable_publication.as_ref().is_some_and(|receipt| receipt.native_artifact_identity() == identity.as_bytes()))
                    })
            }
            CompileOutputKind::ObjectContainer => {
                self.wrote_output
                    && self.artifact.is_none()
                    && self.retained_native_artifact.is_none()
                    && self.executable_publication.is_none()
            }
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "compiled {} source file(s) from {}; wrote_output={}",
            self.source_file_count,
            self.root_path.display(),
            self.wrote_output
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{CompileOutputKind, CompileReport, ExecutablePublicationReceipt};

    fn function_validation_digest(
        validation_report_fingerprint: u64,
    ) -> image::CompilerFunctionValidationDigest {
        image::CompilerFunctionValidationEvidence {
            function_count: 1,
            instruction_count: 2,
            zero_width_instruction_count: 0,
            checked_assembly_instruction_count: 0,
            fixed_mechanics_instruction_count: 2,
            fixed_mechanics_validation_report_fingerprint: 3,
            fixed_mechanics_boundary_contract_report_fingerprint: 2,
            fixed_mechanics_footprint_report_fingerprint: 4,
            body_specification_instruction_count: 0,
            body_specification_validation_report_fingerprint: 0,
            body_specification_boundary_contract_report_fingerprint: 0,
            body_specification_footprint_report_fingerprint: 0,
            composed_footprint_report_fingerprint: 5,
            final_region_binding_report_fingerprint: 6,
            validation_report_fingerprint,
        }
        .evidence_digest()
    }

    fn receipt(path: &str) -> ExecutablePublicationReceipt {
        let path: std::path::PathBuf = path.into();
        let certificate = super::NativePublicationCertificateDigest::from_digest([1; 32]);
        let text_validation = image::CompilerTextDerivationDigest::from_digest([3; 32]);
        let function_validation = function_validation_digest(7);
        let inventory = image::PlacedExecutableRegionInventoryDigest::from_digest([5; 32]);
        let container = super::ExecutableContainerDigest::from_digest([7; 32]);
        let native_artifact_identity = [9; 32];
        let publication = super::native_publication_evidence_digest(
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
            super::executable_installation_evidence_digest(publication, 8, &path, 6, container);
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
        }
    }

    #[test]
    fn flat_installation_v1_digest_is_byte_identical() {
        let digest = super::executable_installation_evidence_digest(
            super::NativePublicationEvidenceDigest::from_digest([1; 32]),
            8,
            std::path::Path::new("build/main"),
            6,
            super::ExecutableContainerDigest::from_digest([7; 32]),
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
                    super::NativePublicationCertificateDigest::from_digest([99; 32])
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
                    super::NativePublicationEvidenceDigest::from_digest([99; 32])
            },
            |receipt| receipt.container_byte_count += 1,
            |receipt| {
                receipt.container_digest = super::ExecutableContainerDigest::from_digest([99; 32])
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
        let rollback = super::OptimizationRollbackReceipt::new(selected, requested);
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
            super::ExecutableInstallationEvidenceDigest::from_digest([99; 32]);
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
}
