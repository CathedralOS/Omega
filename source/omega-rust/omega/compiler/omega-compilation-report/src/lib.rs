use std::path::PathBuf;

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

pub fn executable_installation_evidence_fingerprint(
    destination: ExecutablePublicationDestination,
    publication_evidence_fingerprint: u64,
    callback_placement_identity_fingerprint: u64,
    output_path: &std::path::Path,
    container_byte_count: usize,
    container_fingerprint: u64,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    fingerprint_into(
        &mut hash,
        b"omega.installed-executable-publication-evidence.v1",
    );
    fingerprint_into(&mut hash, &publication_evidence_fingerprint.to_le_bytes());
    fingerprint_into(
        &mut hash,
        &callback_placement_identity_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut hash,
        &[match destination {
            ExecutablePublicationDestination::FlatOutput => 0,
            ExecutablePublicationDestination::MacOsAppBundle => 1,
        }],
    );
    let path = output_path.as_os_str().as_encoded_bytes();
    fingerprint_into(&mut hash, &(path.len() as u64).to_le_bytes());
    fingerprint_into(&mut hash, path);
    fingerprint_into(&mut hash, &(container_byte_count as u64).to_le_bytes());
    fingerprint_into(&mut hash, &container_fingerprint.to_le_bytes());
    hash
}

pub fn executable_publication_pair_matches(
    root_path: &std::path::Path,
    flat: &ExecutablePublicationReceipt,
    bundle: Option<&ExecutablePublicationReceipt>,
) -> bool {
    let Some(bundle) = bundle else {
        return true;
    };
    bundle.destination == ExecutablePublicationDestination::MacOsAppBundle
        && bundle.has_consistent_installation_identity()
        && expected_macos_app_bundle_executable_path(root_path, &flat.output_path).as_deref()
            == Some(bundle.output_path.as_path())
        && flat.certificate_fingerprint == bundle.certificate_fingerprint
        && flat.callback_placement_identity_fingerprint
            == bundle.callback_placement_identity_fingerprint
        && flat.boundary_contract_fingerprint == bundle.boundary_contract_fingerprint
        && flat.inventory_fingerprint == bundle.inventory_fingerprint
        && flat.compiler_text_validation_fingerprint == bundle.compiler_text_validation_fingerprint
        && flat.compiler_function_validation_fingerprint
            == bundle.compiler_function_validation_fingerprint
        && flat.publication_evidence_fingerprint == bundle.publication_evidence_fingerprint
        && flat.container_byte_count == bundle.container_byte_count
        && flat.container_fingerprint == bundle.container_fingerprint
        && flat.installation_evidence_fingerprint != bundle.installation_evidence_fingerprint
}

fn fingerprint_into(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn byte_fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    fingerprint_into(&mut hash, bytes);
    hash
}

fn publication_boundary_contract_fingerprint(
    validation: omega_image::CompilerFunctionValidationEvidence,
) -> Result<Option<u64>, String> {
    let body = (validation.body_specification_instruction_count > 0)
        .then_some(validation.body_specification_boundary_contract_fingerprint);
    let mechanics = (validation.fixed_mechanics_instruction_count > 0)
        .then_some(validation.fixed_mechanics_boundary_contract_fingerprint);
    match (body, mechanics) {
        (Some(left), Some(right)) if left != right => {
            Err("native artifact final validation names inconsistent boundary contracts".to_owned())
        }
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn native_publication_certificate_fingerprint(
    artifact: &RetainedNativeArtifact,
    boundary_contract_fingerprint: Option<u64>,
    text_validation_fingerprint: u64,
    function_validation_fingerprint: u64,
    inventory_fingerprint: u64,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    fingerprint_into(&mut hash, b"omega.native-publication-certificate.v1");
    fingerprint_into(&mut hash, artifact.semantic_bytes());
    fingerprint_into(&mut hash, artifact.proof_bytes());
    fingerprint_into(&mut hash, format!("{:?}", artifact.target()).as_bytes());
    fingerprint_into(
        &mut hash,
        &boundary_contract_fingerprint
            .unwrap_or_default()
            .to_le_bytes(),
    );
    fingerprint_into(&mut hash, &text_validation_fingerprint.to_le_bytes());
    fingerprint_into(&mut hash, &function_validation_fingerprint.to_le_bytes());
    fingerprint_into(&mut hash, &inventory_fingerprint.to_le_bytes());
    hash
}

fn native_publication_evidence_fingerprint(
    certificate_fingerprint: u64,
    callback_placement_identity_fingerprint: u64,
    inventory_fingerprint: u64,
    text_validation_fingerprint: u64,
    function_validation_fingerprint: u64,
    container_byte_count: usize,
    container_fingerprint: u64,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    fingerprint_into(&mut hash, b"omega.native-publication-evidence.v1");
    for value in [
        certificate_fingerprint,
        callback_placement_identity_fingerprint,
        inventory_fingerprint,
        text_validation_fingerprint,
        function_validation_fingerprint,
        container_byte_count as u64,
        container_fingerprint,
    ] {
        fingerprint_into(&mut hash, &value.to_le_bytes());
    }
    hash
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
    certificate_fingerprint: u64,
    callback_placement_identity_fingerprint: u64,
    boundary_contract_fingerprint: Option<u64>,
    inventory_fingerprint: u64,
    compiler_text_validation_fingerprint: u64,
    compiler_function_validation_fingerprint: u64,
    publication_evidence_fingerprint: u64,
    container_byte_count: usize,
    container_fingerprint: u64,
    installation_evidence_fingerprint: u64,
}

impl ExecutablePublicationReceipt {
    pub fn new(
        destination: ExecutablePublicationDestination,
        output_path: PathBuf,
        certificate_fingerprint: u64,
        callback_placement_identity_fingerprint: u64,
        boundary_contract_fingerprint: Option<u64>,
        inventory_fingerprint: u64,
        compiler_text_validation_fingerprint: u64,
        compiler_function_validation_fingerprint: u64,
        publication_evidence_fingerprint: u64,
        container_byte_count: usize,
        container_fingerprint: u64,
        installation_evidence_fingerprint: u64,
    ) -> Self {
        Self {
            destination,
            output_path,
            certificate_fingerprint,
            callback_placement_identity_fingerprint,
            boundary_contract_fingerprint,
            inventory_fingerprint,
            compiler_text_validation_fingerprint,
            compiler_function_validation_fingerprint,
            publication_evidence_fingerprint,
            container_byte_count,
            container_fingerprint,
            installation_evidence_fingerprint,
        }
    }

    pub const fn destination(&self) -> ExecutablePublicationDestination {
        self.destination
    }

    pub fn output_path(&self) -> &std::path::Path {
        &self.output_path
    }

    pub const fn certificate_fingerprint(&self) -> u64 {
        self.certificate_fingerprint
    }

    pub const fn callback_placement_identity_fingerprint(&self) -> u64 {
        self.callback_placement_identity_fingerprint
    }

    pub const fn boundary_contract_fingerprint(&self) -> Option<u64> {
        self.boundary_contract_fingerprint
    }

    pub const fn inventory_fingerprint(&self) -> u64 {
        self.inventory_fingerprint
    }

    pub const fn compiler_text_validation_fingerprint(&self) -> u64 {
        self.compiler_text_validation_fingerprint
    }

    pub const fn compiler_function_validation_fingerprint(&self) -> u64 {
        self.compiler_function_validation_fingerprint
    }

    pub const fn publication_evidence_fingerprint(&self) -> u64 {
        self.publication_evidence_fingerprint
    }

    pub const fn container_byte_count(&self) -> usize {
        self.container_byte_count
    }

    pub const fn container_fingerprint(&self) -> u64 {
        self.container_fingerprint
    }

    pub const fn installation_evidence_fingerprint(&self) -> u64 {
        self.installation_evidence_fingerprint
    }

    pub fn has_consistent_installation_identity(&self) -> bool {
        self.installation_evidence_fingerprint
            == executable_installation_evidence_fingerprint(
                self.destination,
                self.publication_evidence_fingerprint,
                self.callback_placement_identity_fingerprint,
                &self.output_path,
                self.container_byte_count,
                self.container_fingerprint,
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
    /// Deterministic accounting from the transitional typed-tree build
    /// evaluator. This is explicitly not terminal-Psi fuel.
    pub build_evaluation_usage: Option<omega_build_evaluation::BuildEvaluationUsage>,
    /// Exact build-host observation ceiling and realized class for the
    /// selected build-machine run. This does not claim replayability or source
    /// rebuildability.
    pub build_observation_summary: Option<omega_build_evaluation::BuildObservationSummary>,
}

impl CompileReport {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        root_path: PathBuf,
        source_file_count: usize,
        wrote_output: bool,
        output_kind: CompileOutputKind,
        executable_publication: Option<ExecutablePublicationReceipt>,
        app_bundle_publication: Option<ExecutablePublicationReceipt>,
        build_evaluation_usage: Option<omega_build_evaluation::BuildEvaluationUsage>,
        build_observation_summary: Option<omega_build_evaluation::BuildObservationSummary>,
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
            build_evaluation_usage,
            build_observation_summary,
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
        build_evaluation_usage: Option<omega_build_evaluation::BuildEvaluationUsage>,
        build_observation_summary: Option<omega_build_evaluation::BuildObservationSummary>,
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
            build_evaluation_usage,
            build_observation_summary,
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
    /// route selected by `CompileOptions`.
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
        let text_validation_fingerprint = output
            .compiler_text_validation
            .map(|validation| validation.derivation_fingerprint)
            .unwrap_or_else(|| byte_fingerprint(&output.final_text_bytes));
        let function_validation_fingerprint = output
            .compiler_function_validation
            .map(|validation| validation.evidence_fingerprint())
            .unwrap_or_else(|| {
                let mut hash = byte_fingerprint(&output.final_text_bytes);
                fingerprint_into(
                    &mut hash,
                    &(artifact.image().functions().len() as u64).to_le_bytes(),
                );
                hash
            });
        let boundary_contract_fingerprint = output
            .compiler_function_validation
            .map(publication_boundary_contract_fingerprint)
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

        let container_fingerprint = byte_fingerprint(&output.bytes);
        let certificate_fingerprint = native_publication_certificate_fingerprint(
            artifact,
            boundary_contract_fingerprint,
            text_validation_fingerprint,
            function_validation_fingerprint,
            output.executable_regions.inventory_fingerprint,
        );
        let publication_evidence_fingerprint = native_publication_evidence_fingerprint(
            certificate_fingerprint,
            output.callback_placement_identity_fingerprint,
            output.executable_regions.inventory_fingerprint,
            text_validation_fingerprint,
            function_validation_fingerprint,
            output.bytes.len(),
            container_fingerprint,
        );
        let installation_evidence_fingerprint = executable_installation_evidence_fingerprint(
            ExecutablePublicationDestination::FlatOutput,
            publication_evidence_fingerprint,
            output.callback_placement_identity_fingerprint,
            &output_path,
            output.bytes.len(),
            container_fingerprint,
        );
        let receipt = ExecutablePublicationReceipt::new(
            ExecutablePublicationDestination::FlatOutput,
            output_path,
            certificate_fingerprint,
            output.callback_placement_identity_fingerprint,
            boundary_contract_fingerprint,
            output.executable_regions.inventory_fingerprint,
            text_validation_fingerprint,
            function_validation_fingerprint,
            publication_evidence_fingerprint,
            output.bytes.len(),
            container_fingerprint,
            installation_evidence_fingerprint,
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
            build_evaluation_usage: self.build_evaluation_usage,
            build_observation_summary: self.build_observation_summary,
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

    pub fn from_artifact(
        root_path: PathBuf,
        source_file_count: usize,
        artifact: psi_terminal_codec::CanonicalTerminalArtifact,
        build_evaluation_usage: Option<omega_build_evaluation::BuildEvaluationUsage>,
        build_observation_summary: Option<omega_build_evaluation::BuildObservationSummary>,
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
            build_evaluation_usage,
            build_observation_summary,
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
            "compiled {} source file(s) from {}; write_output={}",
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

    fn receipt(
        destination: ExecutablePublicationDestination,
        path: &str,
    ) -> ExecutablePublicationReceipt {
        let path: std::path::PathBuf = path.into();
        let installation =
            super::executable_installation_evidence_fingerprint(destination, 5, 8, &path, 6, 7);
        ExecutablePublicationReceipt::new(
            destination,
            path,
            1,
            8,
            Some(2),
            2,
            3,
            4,
            5,
            6,
            7,
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
            build_evaluation_usage: None,
            build_observation_summary: None,
        }
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
                None,
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
        changed.installation_evidence_fingerprint ^= 1;
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
        changed.compiler_function_validation_fingerprint ^= 1;
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(changed),
            Some(bundle.clone()),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
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
        changed.certificate_fingerprint ^= 1;
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.callback_placement_identity_fingerprint ^= 1;
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.boundary_contract_fingerprint = Some(99);
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.inventory_fingerprint ^= 1;
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.compiler_text_validation_fingerprint ^= 1;
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle.clone();
        changed.compiler_function_validation_fingerprint ^= 1;
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
        changed.publication_evidence_fingerprint ^= 1;
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
        changed.container_fingerprint ^= 1;
        let changed = report(
            true,
            CompileOutputKind::NativeExecutable,
            Some(flat.clone()),
            Some(changed),
        );
        assert!(!changed.has_consistent_executable_publication_custody());
        assert!(changed.checked_native_executable_path().is_none());
        let mut changed = bundle;
        changed.installation_evidence_fingerprint = flat.installation_evidence_fingerprint;
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
