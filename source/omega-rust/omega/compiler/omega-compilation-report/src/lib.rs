use sha2::{Digest, Sha256};
use std::path::PathBuf;

mod optimization_rollback;
pub use optimization_rollback::OptimizationRollbackReceipt;

/// Complete non-clonable Terminal-Psi native artifact retained before output
/// publication. The compatibility name remains while callers migrate from the
/// former legacy `EmissionPlan + EmittedProgram` payload.
pub use omega_native_artifact::NativeArtifact as RetainedNativeArtifact;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileOutputKind {
    CheckOnly,
    TerminalArtifact,
    RetainedNativeArtifact,
    NativeExecutable,
    ObjectContainer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutablePublicationDestination {
    FlatOutput,
    MacOsAppBundle,
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

pub fn macos_app_bundle_name(root_path: &std::path::Path) -> String {
    root_path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("omega-program")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | ' ' | '-') {
                character
            } else {
                '-'
            }
        })
        .collect()
}

pub fn expected_macos_app_bundle_executable_path(
    root_path: &std::path::Path,
    flat_output_path: &std::path::Path,
) -> Option<PathBuf> {
    let build_dir = flat_output_path.parent()?;
    let executable_name = flat_output_path.file_name()?;
    Some(
        build_dir
            .join(format!("{}.app", macos_app_bundle_name(root_path)))
            .join("Contents")
            .join("MacOS")
            .join(executable_name),
    )
}

pub fn executable_installation_evidence_digest(
    destination: ExecutablePublicationDestination,
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
    digest.update([match destination {
        ExecutablePublicationDestination::FlatOutput => 0,
        ExecutablePublicationDestination::MacOsAppBundle => 1,
    }]);
    let path = output_path.as_os_str().as_encoded_bytes();
    digest.update((path.len() as u64).to_le_bytes());
    digest.update(path);
    digest.update((container_byte_count as u64).to_le_bytes());
    digest.update(container_digest.as_bytes());
    ExecutableInstallationEvidenceDigest::from_digest(digest.finalize().into())
}

pub fn executable_publication_pair_matches(
    root_path: &std::path::Path,
    flat: &ExecutablePublicationReceipt,
    bundle: Option<&ExecutablePublicationReceipt>,
) -> bool {
    if !flat.has_consistent_installation_identity() {
        return false;
    }
    let Some(bundle) = bundle else {
        return true;
    };
    bundle.destination == ExecutablePublicationDestination::MacOsAppBundle
        && bundle.has_consistent_installation_identity()
        && expected_macos_app_bundle_executable_path(root_path, &flat.output_path).as_deref()
            == Some(bundle.output_path.as_path())
        && flat.certificate_digest == bundle.certificate_digest
        && flat.callback_placement_identity_report_fingerprint
            == bundle.callback_placement_identity_report_fingerprint
        && flat.boundary_contract_report_fingerprint == bundle.boundary_contract_report_fingerprint
        && flat.inventory_digest == bundle.inventory_digest
        && flat.inventory_report_fingerprint == bundle.inventory_report_fingerprint
        && flat.compiler_text_validation_digest == bundle.compiler_text_validation_digest
        && flat.compiler_function_validation_digest == bundle.compiler_function_validation_digest
        && flat.compiler_function_validation_report_fingerprint
            == bundle.compiler_function_validation_report_fingerprint
        && flat.publication_evidence_digest == bundle.publication_evidence_digest
        && flat.container_byte_count == bundle.container_byte_count
        && flat.container_digest == bundle.container_digest
        && flat.installation_evidence_digest != bundle.installation_evidence_digest
}

fn publication_boundary_contract_report_fingerprint(
    validation: omega_image::CompilerFunctionValidationEvidence,
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
    text_validation_digest: omega_image::CompilerTextDerivationDigest,
    function_validation_digest: omega_image::CompilerFunctionValidationDigest,
    function_validation_report_fingerprint: u64,
    inventory_digest: omega_image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
) -> NativePublicationCertificateDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-publication-certificate.sha256.v1\0");
    digest.update((artifact.semantic_bytes().len() as u64).to_le_bytes());
    digest.update(artifact.semantic_bytes());
    digest.update((artifact.proof_bytes().len() as u64).to_le_bytes());
    digest.update(artifact.proof_bytes());
    let target = artifact.target();
    digest.update([match target.architecture {
        omega_target::Architecture::Aarch64 => 1,
        omega_target::Architecture::X86_64 => 2,
    }]);
    digest.update([match target.object_format {
        omega_target::ObjectFormat::Elf => 1,
        omega_target::ObjectFormat::MachO => 2,
        omega_target::ObjectFormat::Coff => 3,
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
    certificate_digest: NativePublicationCertificateDigest,
    callback_placement_identity_report_fingerprint: u64,
    inventory_digest: omega_image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
    text_validation_digest: omega_image::CompilerTextDerivationDigest,
    function_validation_digest: omega_image::CompilerFunctionValidationDigest,
    function_validation_report_fingerprint: u64,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
) -> NativePublicationEvidenceDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-publication-evidence.sha256.v1\0");
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
    destination: ExecutablePublicationDestination,
    output_path: PathBuf,
    certificate_digest: NativePublicationCertificateDigest,
    /// Compact report coordinate. Exact callback placements are structurally
    /// replayed before this publication receipt can be produced.
    callback_placement_identity_report_fingerprint: u64,
    /// Compact report coordinate; exact final footprint authority is retained
    /// by the placed-region inventory and certificate digest.
    boundary_contract_report_fingerprint: Option<u64>,
    inventory_digest: omega_image::PlacedExecutableRegionInventoryDigest,
    /// Compact report compatibility only. Publication and replay authority is
    /// `inventory_digest`.
    inventory_report_fingerprint: u64,
    compiler_text_validation_digest: omega_image::CompilerTextDerivationDigest,
    compiler_function_validation_digest: omega_image::CompilerFunctionValidationDigest,
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
        destination: ExecutablePublicationDestination,
        output_path: PathBuf,
        certificate_digest: NativePublicationCertificateDigest,
        callback_placement_identity_report_fingerprint: u64,
        boundary_contract_report_fingerprint: Option<u64>,
        inventory_digest: omega_image::PlacedExecutableRegionInventoryDigest,
        inventory_report_fingerprint: u64,
        compiler_text_validation_digest: omega_image::CompilerTextDerivationDigest,
        compiler_function_validation_digest: omega_image::CompilerFunctionValidationDigest,
        compiler_function_validation_report_fingerprint: u64,
        publication_evidence_digest: NativePublicationEvidenceDigest,
        container_byte_count: usize,
        container_digest: ExecutableContainerDigest,
        installation_evidence_digest: ExecutableInstallationEvidenceDigest,
    ) -> Self {
        Self {
            destination,
            output_path,
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

    pub const fn destination(&self) -> ExecutablePublicationDestination {
        self.destination
    }

    pub fn output_path(&self) -> &std::path::Path {
        &self.output_path
    }

    pub const fn certificate_digest(&self) -> NativePublicationCertificateDigest {
        self.certificate_digest
    }

    pub const fn callback_placement_identity_report_fingerprint(&self) -> u64 {
        self.callback_placement_identity_report_fingerprint
    }

    pub const fn boundary_contract_report_fingerprint(&self) -> Option<u64> {
        self.boundary_contract_report_fingerprint
    }

    pub const fn inventory_digest(&self) -> omega_image::PlacedExecutableRegionInventoryDigest {
        self.inventory_digest
    }

    pub const fn inventory_report_fingerprint(&self) -> u64 {
        self.inventory_report_fingerprint
    }

    pub const fn compiler_text_validation_digest(
        &self,
    ) -> omega_image::CompilerTextDerivationDigest {
        self.compiler_text_validation_digest
    }

    pub const fn compiler_function_validation_report_fingerprint(&self) -> u64 {
        self.compiler_function_validation_report_fingerprint
    }

    pub const fn compiler_function_validation_digest(
        &self,
    ) -> omega_image::CompilerFunctionValidationDigest {
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
                    self.destination,
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
    artifact: Option<psi_terminal_codec::CanonicalTerminalArtifact>,
    /// Exact checked publication receipt for a native executable image.
    /// Object-container fallbacks and check-only compilations retain `None`.
    executable_publication: Option<ExecutablePublicationReceipt>,
    /// Exact checked publication receipt for the executable copied into an
    /// optional macOS application bundle. Non-GUI/non-Mach-O builds retain
    /// `None`; this remains distinct from the flat executable receipt.
    app_bundle_publication: Option<ExecutablePublicationReceipt>,
    /// Exact subtractive release overlay applied after build selection and
    /// before native realization. Ordinary requests retain `None`.
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    /// Filesystem-free comparison between the request's explicit admissions
    /// and every trust obligation reconstructed by compilation.
    trust_admission_settlement: omega_trust_model::TrustAdmissionSettlement,
}

impl CompileReport {
    pub fn checked(
        root_path: PathBuf,
        source_file_count: usize,
        wrote_output: bool,
        output_kind: CompileOutputKind,
        executable_publication: Option<ExecutablePublicationReceipt>,
        app_bundle_publication: Option<ExecutablePublicationReceipt>,
    ) -> Result<Self, &'static str> {
        let report = Self {
            root_path,
            source_file_count,
            wrote_output,
            output_kind,
            retained_native_artifact: None,
            artifact: None,
            executable_publication,
            app_bundle_publication,
            optimization_rollback: None,
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
    ) -> Result<Self, &'static str> {
        artifact
            .validate()
            .map_err(|_| "compiler report received an invalid native artifact")?;
        let report = Self {
            root_path,
            source_file_count,
            wrote_output: false,
            output_kind: CompileOutputKind::RetainedNativeArtifact,
            retained_native_artifact: Some(artifact),
            artifact: None,
            executable_publication: None,
            app_bundle_publication: None,
            optimization_rollback,
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
            || self.app_bundle_publication.is_some()
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
            ExecutablePublicationDestination::FlatOutput,
            publication_evidence_digest,
            output.callback_placement_identity_report_fingerprint,
            &output_path,
            output.bytes.len(),
            container_digest,
        );
        let receipt = ExecutablePublicationReceipt::new(
            ExecutablePublicationDestination::FlatOutput,
            output_path,
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
            app_bundle_publication: None,
            optimization_rollback: self.optimization_rollback,
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

    pub fn with_trust_admission_settlement(
        mut self,
        settlement: omega_trust_model::TrustAdmissionSettlement,
    ) -> Self {
        self.trust_admission_settlement = settlement;
        self
    }

    pub const fn trust_admission_settlement(&self) -> &omega_trust_model::TrustAdmissionSettlement {
        &self.trust_admission_settlement
    }

    pub fn from_artifact(
        root_path: PathBuf,
        source_file_count: usize,
        artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    ) -> Result<Self, &'static str> {
        artifact
            .validate()
            .map_err(|_| "compiler report received an invalid canonical Terminal artifact")?;
        let report = Self {
            root_path,
            source_file_count,
            wrote_output: false,
            output_kind: CompileOutputKind::TerminalArtifact,
            retained_native_artifact: None,
            artifact: Some(artifact),
            executable_publication: None,
            app_bundle_publication: None,
            optimization_rollback: None,
            trust_admission_settlement: Default::default(),
        };
        report
            .has_consistent_executable_publication_custody()
            .then_some(report)
            .ok_or("compiler report retained inconsistent Terminal-artifact custody")
    }

    pub const fn artifact(&self) -> Option<&psi_terminal_codec::CanonicalTerminalArtifact> {
        self.artifact.as_ref()
    }

    pub fn into_artifact(self) -> Option<psi_terminal_codec::CanonicalTerminalArtifact> {
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

    pub fn app_bundle_publication(&self) -> Option<&ExecutablePublicationReceipt> {
        self.app_bundle_publication.as_ref()
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
    /// mutually exclusive with publication, legacy output checks the flat
    /// executable and optional app-bundle copy, and terminal output replays
    /// the retained installation/image/file join.
    pub fn has_consistent_executable_publication_custody(&self) -> bool {
        let rollback_matches_kind = match self.output_kind {
            CompileOutputKind::RetainedNativeArtifact | CompileOutputKind::NativeExecutable => self
                .optimization_rollback
                .as_ref()
                .is_none_or(OptimizationRollbackReceipt::is_consistent),
            CompileOutputKind::CheckOnly
            | CompileOutputKind::TerminalArtifact
            | CompileOutputKind::ObjectContainer => self.optimization_rollback.is_none(),
        };
        if !rollback_matches_kind {
            return false;
        }
        let cardinality_matches_kind = match self.output_kind {
            CompileOutputKind::CheckOnly => {
                !self.wrote_output
                    && self.artifact.is_none()
                    && self.retained_native_artifact.is_none()
                    && self.executable_publication.is_none()
                    && self.app_bundle_publication.is_none()
            }
            CompileOutputKind::TerminalArtifact => {
                !self.wrote_output
                    && self.retained_native_artifact.is_none()
                    && self
                        .artifact
                        .as_ref()
                        .is_some_and(|artifact| artifact.validate().is_ok())
                    && self.executable_publication.is_none()
                    && self.app_bundle_publication.is_none()
            }
            CompileOutputKind::RetainedNativeArtifact => {
                !self.wrote_output
                    && self.artifact.is_none()
                    && self
                        .retained_native_artifact
                        .as_ref()
                        .is_some_and(|artifact| artifact.validate().is_ok())
                    && self.executable_publication.is_none()
                    && self.app_bundle_publication.is_none()
            }
            CompileOutputKind::NativeExecutable => {
                self.wrote_output
                    && self.artifact.is_none()
                    && self.retained_native_artifact.is_none()
                    && self.executable_publication.as_ref().is_some_and(|receipt| {
                        receipt.destination == ExecutablePublicationDestination::FlatOutput
                            && receipt.has_consistent_installation_identity()
                    })
            }
            CompileOutputKind::ObjectContainer => {
                self.wrote_output
                    && self.artifact.is_none()
                    && self.retained_native_artifact.is_none()
                    && self.executable_publication.is_none()
                    && self.app_bundle_publication.is_none()
            }
        };
        if !cardinality_matches_kind {
            return false;
        }
        match (
            self.executable_publication.as_ref(),
            self.app_bundle_publication.as_ref(),
        ) {
            (None, None) => true,
            (None, Some(_)) => false,
            (Some(flat), bundle) => {
                executable_publication_pair_matches(&self.root_path, flat, bundle)
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
    use super::{
        CompileOutputKind, CompileReport, ExecutablePublicationDestination,
        ExecutablePublicationReceipt,
    };

    fn function_validation_digest(
        validation_report_fingerprint: u64,
    ) -> omega_image::CompilerFunctionValidationDigest {
        omega_image::CompilerFunctionValidationEvidence {
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

    fn receipt(
        destination: ExecutablePublicationDestination,
        path: &str,
    ) -> ExecutablePublicationReceipt {
        let path: std::path::PathBuf = path.into();
        let certificate = super::NativePublicationCertificateDigest::from_digest([1; 32]);
        let text_validation = omega_image::CompilerTextDerivationDigest::from_digest([3; 32]);
        let function_validation = function_validation_digest(7);
        let inventory = omega_image::PlacedExecutableRegionInventoryDigest::from_digest([5; 32]);
        let container = super::ExecutableContainerDigest::from_digest([7; 32]);
        let publication = super::native_publication_evidence_digest(
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
        let installation = super::executable_installation_evidence_digest(
            destination,
            publication,
            8,
            &path,
            6,
            container,
        );
        ExecutablePublicationReceipt::new(
            destination,
            path,
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
        bundle: Option<ExecutablePublicationReceipt>,
    ) -> CompileReport {
        CompileReport {
            root_path: "Main/main.omg".into(),
            source_file_count: 1,
            wrote_output,
            output_kind,
            retained_native_artifact: None,
            artifact: None,
            executable_publication: flat,
            app_bundle_publication: bundle,
            optimization_rollback: None,
            trust_admission_settlement: Default::default(),
        }
    }

    #[test]
    fn rollback_receipt_is_custody_only_for_native_products() {
        let selected = omega_optimization_core::OptimizationSelections::new([
            omega_optimization_core::Optimization::ControlFlowCleanup,
        ])
        .unwrap();
        let requested = selected.clone();
        let rollback = super::OptimizationRollbackReceipt::new(selected, requested);
        let mut native = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(receipt(
                ExecutablePublicationDestination::FlatOutput,
                "build/main",
            )),
            None,
        );
        native.optimization_rollback = Some(rollback.clone());
        assert!(native.has_consistent_executable_publication_custody());

        let mut check = report(false, CompileOutputKind::CheckOnly, None, None);
        check.optimization_rollback = Some(rollback);
        assert!(!check.has_consistent_executable_publication_custody());
    }

    #[test]
    fn executable_publication_pair_rejects_every_cross_copy_drift() {
        let flat = receipt(ExecutablePublicationDestination::FlatOutput, "build/main");
        let bundle = receipt(
            ExecutablePublicationDestination::MacOsAppBundle,
            "build/Main.app/Contents/MacOS/main",
        );
        assert!(flat.has_consistent_installation_identity());
        assert!(bundle.has_consistent_installation_identity());
        let native = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(bundle.clone()),
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
            Some(bundle.clone()),
        );
        changed_kind.output_kind = CompileOutputKind::ObjectContainer;
        assert!(changed_kind.checked_native_executable_path().is_none());
        let check_only = report(false, CompileOutputKind::CheckOnly, None, None);
        assert!(check_only.has_consistent_executable_publication_custody());
        assert!(check_only.checked_native_executable_path().is_none());
        let object = report(true, CompileOutputKind::ObjectContainer, None, None);
        assert!(object.has_consistent_executable_publication_custody());
        assert!(object.checked_native_executable_path().is_none());
        assert!(
            !report(
                false,
                CompileOutputKind::CheckOnly,
                Some(flat.clone()),
                None,
            )
            .has_consistent_executable_publication_custody()
        );
        assert!(
            CompileReport::checked(
                "Main/main.omg".into(),
                1,
                false,
                CompileOutputKind::CheckOnly,
                Some(flat.clone()),
                None,
            )
            .is_err()
        );
        let missing_native_receipt = report(true, CompileOutputKind::NativeExecutable, None, None);
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
            None,
        );
        assert!(!dropped_output.has_consistent_executable_publication_custody());
        assert!(dropped_output.checked_native_executable_path().is_none());
        assert!(
            !report(
                true,
                CompileOutputKind::ObjectContainer,
                Some(flat.clone()),
                None,
            )
            .has_consistent_executable_publication_custody()
        );
        let mut changed = flat.clone();
        changed.installation_evidence_digest =
            super::ExecutableInstallationEvidenceDigest::from_digest([99; 32]);
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(changed),
            None,
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = flat.clone();
        changed.output_path = "build/redirected-main".into();
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(changed),
            None,
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = flat.clone();
        changed.compiler_function_validation_report_fingerprint ^= 1;
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(changed),
            Some(bundle.clone()),
        );
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
            None,
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
            Some(bundle.clone()),
        );
        assert!(retained.wrote_output());
        assert_eq!(retained.root_path(), std::path::Path::new("Main/main.omg"));
        assert_eq!(retained.output_kind(), CompileOutputKind::NativeExecutable);
        assert_eq!(retained.executable_publication(), Some(&flat));
        assert_eq!(retained.app_bundle_publication(), Some(&bundle));
        let missing_flat = report(
            true,
            CompileOutputKind::NativeExecutable,
            None,
            Some(bundle.clone()),
        );
        assert!(!missing_flat.has_consistent_executable_publication_custody());
        assert!(missing_flat.checked_native_executable_path().is_none());
        let self_aliased = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(flat.clone()),
        );
        assert!(!self_aliased.has_consistent_executable_publication_custody());
        assert!(self_aliased.checked_native_executable_path().is_none());
        let swapped_roles = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(bundle.clone()),
            Some(flat.clone()),
        );
        assert!(!swapped_roles.has_consistent_executable_publication_custody());
        assert!(swapped_roles.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.output_path = "build/Other.app/Contents/MacOS/main".into();
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());

        let mut changed = bundle.clone();
        changed.certificate_digest =
            super::NativePublicationCertificateDigest::from_digest([99; 32]);
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.callback_placement_identity_report_fingerprint ^= 1;
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.boundary_contract_report_fingerprint = Some(99);
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.inventory_report_fingerprint ^= 1;
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut compact_equal_inventory_substitution = bundle.clone();
        compact_equal_inventory_substitution.inventory_digest =
            omega_image::PlacedExecutableRegionInventoryDigest::from_digest([99; 32]);
        assert_eq!(
            compact_equal_inventory_substitution.inventory_report_fingerprint,
            bundle.inventory_report_fingerprint,
            "the adversary preserves the compact inventory report coordinate",
        );
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(compact_equal_inventory_substitution),
        );
        assert!(
            !changed.has_consistent_executable_publication_custody(),
            "strong inventory drift must reject even with a collision-equal report fingerprint",
        );
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.compiler_text_validation_digest =
            omega_image::CompilerTextDerivationDigest::from_digest([99; 32]);
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.compiler_function_validation_report_fingerprint ^= 1;
        assert!(
            !report(
                true,
                CompileOutputKind::NativeExecutable,
                Some(flat.clone()),
                Some(changed),
            )
            .has_consistent_executable_publication_custody()
        );
        let mut changed = bundle.clone();
        changed.publication_evidence_digest =
            super::NativePublicationEvidenceDigest::from_digest([99; 32]);
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.container_byte_count += 1;
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.container_digest = super::ExecutableContainerDigest::from_digest([99; 32]);
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle;
        changed.installation_evidence_digest = flat.installation_evidence_digest;
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
    }
}
