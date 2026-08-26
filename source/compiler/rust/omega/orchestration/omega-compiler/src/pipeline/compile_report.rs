use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileOutputKind {
    CheckOnly,
    NativeExecutable,
    ObjectContainer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutablePublicationDestination {
    FlatOutput,
    MacOsAppBundle,
}

pub(super) fn macos_app_bundle_name(root_path: &std::path::Path) -> String {
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

pub(super) fn expected_macos_app_bundle_executable_path(
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

pub(super) fn executable_installation_evidence_fingerprint(
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

pub(super) fn executable_publication_pair_matches(
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
    pub(crate) fn new(
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

/// A rejected attempt to retain terminal deployment custody in a compiler
/// report. The complete published runnable is returned for exact recovery.
#[derive(Debug)]
pub struct TerminalComponentDeploymentReportError {
    deployment: omega_component_deployment::PublishedTerminalComponentFlatOutput,
    diagnostic: String,
}

impl TerminalComponentDeploymentReportError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_deployment(
        self,
    ) -> omega_component_deployment::PublishedTerminalComponentFlatOutput {
        self.deployment
    }
}

impl std::fmt::Display for TerminalComponentDeploymentReportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for TerminalComponentDeploymentReportError {}

#[derive(Debug)]
pub struct CompileReport {
    root_path: PathBuf,
    pub source_file_count: usize,
    wrote_output: bool,
    /// Exact output category selected by orchestration. This distinguishes a
    /// native executable, which requires publication custody, from the
    /// non-executable object-container fallback.
    output_kind: CompileOutputKind,
    /// Exact checked publication receipt for a native executable image.
    /// Object-container fallbacks and check-only compilations retain `None`.
    executable_publication: Option<ExecutablePublicationReceipt>,
    /// Exact checked publication receipt for the executable copied into an
    /// optional macOS application bundle. Non-GUI/non-Mach-O builds retain
    /// `None`; this remains distinct from the flat executable receipt.
    app_bundle_publication: Option<ExecutablePublicationReceipt>,
    /// Complete non-clonable terminal deployment result. This is mutually
    /// exclusive with the legacy executable receipts and retains both the
    /// runnable installation custody and its flat publication receipt.
    terminal_component_deployment:
        Option<omega_component_deployment::PublishedTerminalComponentFlatOutput>,
    /// Exact target root-slot/schema/ABI-capture binding for a program-storage
    /// entry. Hosted compatibility entries and unmigrated name discovery have
    /// no such authority-bearing artifact.
    program_storage_entry: Option<super::ProgramStorageEntryPlanBinding>,
    /// Emitted object-entry handoff awaiting concrete environment supply and
    /// runtime installation. This is not an installation receipt.
    program_storage_entry_bridge: Option<super::ProgramStorageEntryNativeBridgePlan>,
    /// Deterministic accounting from the transitional typed-tree build
    /// evaluator. This is explicitly not terminal-Psi fuel.
    pub build_evaluation_usage: Option<super::build_config::BuildEvaluationUsage>,
    /// Exact build-host observation ceiling and realized class for the
    /// selected build-machine run. This does not claim replayability or source
    /// rebuildability.
    pub build_observation_summary: Option<super::build_config::BuildObservationSummary>,
}

impl CompileReport {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn checked(
        root_path: PathBuf,
        source_file_count: usize,
        wrote_output: bool,
        output_kind: CompileOutputKind,
        executable_publication: Option<ExecutablePublicationReceipt>,
        app_bundle_publication: Option<ExecutablePublicationReceipt>,
        program_storage_entry: Option<super::ProgramStorageEntryPlanBinding>,
        program_storage_entry_bridge: Option<super::ProgramStorageEntryNativeBridgePlan>,
        build_evaluation_usage: Option<super::build_config::BuildEvaluationUsage>,
        build_observation_summary: Option<super::build_config::BuildObservationSummary>,
    ) -> Result<Self, &'static str> {
        let report = Self {
            root_path,
            source_file_count,
            wrote_output,
            output_kind,
            executable_publication,
            app_bundle_publication,
            terminal_component_deployment: None,
            program_storage_entry,
            program_storage_entry_bridge,
            build_evaluation_usage,
            build_observation_summary,
        };
        if report.has_consistent_executable_publication_custody() {
            if report.has_consistent_program_storage_entry_custody() {
                Ok(report)
            } else {
                Err("compiler report retained inconsistent program-storage entry custody")
            }
        } else {
            Err("compiler report retained inconsistent executable publication receipts")
        }
    }

    /// Retain one successfully published terminal deployment as the compiler
    /// report's native output custody.
    ///
    /// Validation replays the installation/image/file join before the report
    /// takes ownership. Rejection returns the complete non-clonable deployment
    /// instead of reducing it to a path or diagnostic.
    pub fn from_terminal_component_deployment(
        root_path: PathBuf,
        source_file_count: usize,
        deployment: omega_component_deployment::PublishedTerminalComponentFlatOutput,
        build_evaluation_usage: Option<super::build_config::BuildEvaluationUsage>,
        build_observation_summary: Option<super::build_config::BuildObservationSummary>,
    ) -> Result<Self, Box<TerminalComponentDeploymentReportError>> {
        if let Err(error) = deployment.validate() {
            return Err(Box::new(TerminalComponentDeploymentReportError {
                deployment,
                diagnostic: format!(
                    "terminal component deployment cannot enter compiler report custody: {}",
                    error.diagnostic()
                ),
            }));
        }
        Ok(Self {
            root_path,
            source_file_count,
            wrote_output: true,
            output_kind: CompileOutputKind::NativeExecutable,
            executable_publication: None,
            app_bundle_publication: None,
            terminal_component_deployment: Some(deployment),
            program_storage_entry: None,
            program_storage_entry_bridge: None,
            build_evaluation_usage,
            build_observation_summary,
        })
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

    pub fn executable_publication(&self) -> Option<&ExecutablePublicationReceipt> {
        self.executable_publication.as_ref()
    }

    pub fn app_bundle_publication(&self) -> Option<&ExecutablePublicationReceipt> {
        self.app_bundle_publication.as_ref()
    }

    pub const fn terminal_component_deployment(
        &self,
    ) -> Option<&omega_component_deployment::PublishedTerminalComponentFlatOutput> {
        self.terminal_component_deployment.as_ref()
    }

    /// Transfer the complete non-clonable terminal deployment result out of
    /// this report. Legacy and non-native reports return `None`.
    pub fn into_terminal_component_deployment(
        self,
    ) -> Option<omega_component_deployment::PublishedTerminalComponentFlatOutput> {
        self.terminal_component_deployment
    }

    /// Returns the exact installed flat executable only after independently
    /// replaying the complete report custody checks. Object/check-only reports
    /// and any internally drifted receipt graph fail closed.
    pub fn checked_native_executable_path(&self) -> Option<&std::path::Path> {
        if self.output_kind != CompileOutputKind::NativeExecutable
            || !self.has_consistent_executable_publication_custody()
            || !self.has_consistent_program_storage_entry_custody()
        {
            return None;
        }
        self.terminal_component_deployment
            .as_ref()
            .map(|deployment| deployment.receipt().output_path())
            .or_else(|| {
                self.executable_publication
                    .as_ref()
                    .map(ExecutablePublicationReceipt::output_path)
            })
    }

    pub fn program_storage_entry(&self) -> Option<&super::ProgramStorageEntryPlanBinding> {
        self.program_storage_entry.as_ref()
    }

    pub fn program_storage_entry_bridge(
        &self,
    ) -> Option<&super::ProgramStorageEntryNativeBridgePlan> {
        self.program_storage_entry_bridge.as_ref()
    }

    pub fn has_consistent_program_storage_entry_custody(&self) -> bool {
        optional_exact_pair_matches(
            self.program_storage_entry.as_ref(),
            self.program_storage_entry_bridge
                .as_ref()
                .map(super::ProgramStorageEntryNativeBridgePlan::binding),
        ) && program_storage_emission_matches_output_kind(
            self.output_kind,
            self.program_storage_entry_bridge.as_ref().map(|bridge| {
                (
                    bridge.wrapper_body_template().is_some(),
                    bridge.is_receiver_bound_without_wrapper_template(),
                    bridge.emitted_wrapper_evidence().is_some(),
                )
            }),
        ) && retained_entry_boundary_matches_publication(
            self.output_kind,
            self.executable_publication
                .as_ref()
                .and_then(ExecutablePublicationReceipt::boundary_contract_fingerprint),
            self.program_storage_entry
                .as_ref()
                .map(super::ProgramStorageEntryPlanBinding::boundary_contract_fingerprint),
        ) && emitted_inventory_matches_publication(
            self.executable_publication
                .as_ref()
                .map(ExecutablePublicationReceipt::inventory_fingerprint),
            self.program_storage_entry_bridge
                .as_ref()
                .and_then(super::ProgramStorageEntryNativeBridgePlan::emitted_wrapper_evidence)
                .map(|evidence| evidence.executable_inventory_fingerprint()),
        ) && emitted_validation_matches_publication(
            self.executable_publication.as_ref().map(|receipt| {
                (
                    receipt.compiler_text_validation_fingerprint,
                    receipt.compiler_function_validation_fingerprint,
                )
            }),
            self.program_storage_entry_bridge
                .as_ref()
                .and_then(super::ProgramStorageEntryNativeBridgePlan::emitted_wrapper_evidence)
                .map(|evidence| {
                    (
                        evidence.compiler_text_validation().derivation_fingerprint,
                        evidence
                            .compiler_function_validation()
                            .evidence_fingerprint(),
                    )
                }),
        ) && emitted_boundary_contract_matches_publication(
            self.executable_publication
                .as_ref()
                .and_then(ExecutablePublicationReceipt::boundary_contract_fingerprint),
            self.program_storage_entry_bridge
                .as_ref()
                .and_then(super::ProgramStorageEntryNativeBridgePlan::emitted_wrapper_evidence)
                .map(|evidence| evidence.arrival().boundary_contract_fingerprint()),
        )
    }

    /// Replays the exact native publication lane. Legacy output checks the
    /// relationship between the flat executable and optional app-bundle copy;
    /// terminal output instead replays the retained installation/image/file
    /// join. The two lanes are mutually exclusive.
    pub fn has_consistent_executable_publication_custody(&self) -> bool {
        let terminal_deployment_valid = self
            .terminal_component_deployment
            .as_ref()
            .is_some_and(|deployment| deployment.validate().is_ok());
        let cardinality_matches_kind = match self.output_kind {
            CompileOutputKind::CheckOnly => {
                !self.wrote_output
                    && self.executable_publication.is_none()
                    && self.app_bundle_publication.is_none()
                    && self.terminal_component_deployment.is_none()
            }
            CompileOutputKind::NativeExecutable => {
                self.wrote_output
                    && match (
                        self.executable_publication.as_ref(),
                        self.app_bundle_publication.as_ref(),
                        self.terminal_component_deployment.as_ref(),
                    ) {
                        (Some(receipt), _, None) => {
                            receipt.destination == ExecutablePublicationDestination::FlatOutput
                                && receipt.has_consistent_installation_identity()
                        }
                        (None, None, Some(_)) => terminal_deployment_valid,
                        _ => false,
                    }
            }
            CompileOutputKind::ObjectContainer => {
                self.wrote_output
                    && self.executable_publication.is_none()
                    && self.app_bundle_publication.is_none()
                    && self.terminal_component_deployment.is_none()
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

fn optional_exact_pair_matches<T: PartialEq>(left: Option<&T>, right: Option<&T>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => left == right,
        (None, Some(_)) | (Some(_), None) => false,
    }
}

fn program_storage_emission_matches_output_kind(
    output_kind: CompileOutputKind,
    bridge_emission: Option<(bool, bool, bool)>,
) -> bool {
    match (output_kind, bridge_emission) {
        (_, None) => true,
        (CompileOutputKind::CheckOnly, Some((_, _, false))) => true,
        (CompileOutputKind::NativeExecutable, Some((true, false, true)))
        | (CompileOutputKind::NativeExecutable, Some((false, true, false))) => true,
        (CompileOutputKind::CheckOnly, Some((_, _, true)))
        | (CompileOutputKind::NativeExecutable, Some(_))
        | (CompileOutputKind::ObjectContainer, Some(_)) => false,
    }
}

fn emitted_inventory_matches_publication(
    publication_inventory_fingerprint: Option<u64>,
    emitted_inventory_fingerprint: Option<u64>,
) -> bool {
    emitted_inventory_fingerprint
        .is_none_or(|emitted| publication_inventory_fingerprint == Some(emitted))
}

fn retained_entry_boundary_matches_publication(
    output_kind: CompileOutputKind,
    publication_boundary_contract_fingerprint: Option<u64>,
    retained_entry_boundary_contract_fingerprint: Option<u64>,
) -> bool {
    match (output_kind, retained_entry_boundary_contract_fingerprint) {
        (_, None) => true,
        (CompileOutputKind::CheckOnly, Some(_)) => {
            publication_boundary_contract_fingerprint.is_none()
        }
        (CompileOutputKind::NativeExecutable, Some(retained)) => {
            publication_boundary_contract_fingerprint == Some(retained)
        }
        (CompileOutputKind::ObjectContainer, Some(_)) => false,
    }
}

fn emitted_validation_matches_publication(
    publication_validation_fingerprints: Option<(u64, u64)>,
    emitted_validation_fingerprints: Option<(u64, u64)>,
) -> bool {
    emitted_validation_fingerprints
        .is_none_or(|emitted| publication_validation_fingerprints == Some(emitted))
}

fn emitted_boundary_contract_matches_publication(
    publication_boundary_contract_fingerprint: Option<u64>,
    emitted_boundary_contract_fingerprint: Option<u64>,
) -> bool {
    emitted_boundary_contract_fingerprint
        .is_none_or(|emitted| publication_boundary_contract_fingerprint == Some(emitted))
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
            executable_publication: flat,
            app_bundle_publication: bundle,
            terminal_component_deployment: None,
            program_storage_entry: None,
            program_storage_entry_bridge: None,
            build_evaluation_usage: None,
            build_observation_summary: None,
        }
    }

    #[test]
    fn executable_publication_pair_rejects_every_cross_copy_drift() {
        assert!(super::optional_exact_pair_matches::<u8>(None, None));
        assert!(super::optional_exact_pair_matches(Some(&1), Some(&1)));
        assert!(!super::optional_exact_pair_matches(Some(&1), Some(&2)));
        assert!(!super::optional_exact_pair_matches(Some(&1), None));
        assert!(!super::optional_exact_pair_matches(None, Some(&1)));
        for output_kind in [
            CompileOutputKind::CheckOnly,
            CompileOutputKind::NativeExecutable,
            CompileOutputKind::ObjectContainer,
        ] {
            assert!(super::program_storage_emission_matches_output_kind(
                output_kind,
                None,
            ));
        }
        assert!(super::program_storage_emission_matches_output_kind(
            CompileOutputKind::CheckOnly,
            Some((true, false, false)),
        ));
        assert!(!super::program_storage_emission_matches_output_kind(
            CompileOutputKind::CheckOnly,
            Some((true, false, true)),
        ));
        assert!(super::program_storage_emission_matches_output_kind(
            CompileOutputKind::NativeExecutable,
            Some((true, false, true)),
        ));
        assert!(super::program_storage_emission_matches_output_kind(
            CompileOutputKind::NativeExecutable,
            Some((false, true, false)),
        ));
        assert!(!super::program_storage_emission_matches_output_kind(
            CompileOutputKind::NativeExecutable,
            Some((true, false, false)),
        ));
        assert!(!super::program_storage_emission_matches_output_kind(
            CompileOutputKind::NativeExecutable,
            Some((false, false, false)),
        ));
        assert!(!super::program_storage_emission_matches_output_kind(
            CompileOutputKind::ObjectContainer,
            Some((false, true, false)),
        ));
        assert!(!super::program_storage_emission_matches_output_kind(
            CompileOutputKind::ObjectContainer,
            Some((true, false, true)),
        ));
        for output_kind in [
            CompileOutputKind::CheckOnly,
            CompileOutputKind::NativeExecutable,
            CompileOutputKind::ObjectContainer,
        ] {
            assert!(super::retained_entry_boundary_matches_publication(
                output_kind,
                None,
                None,
            ));
            assert!(super::retained_entry_boundary_matches_publication(
                output_kind,
                Some(1),
                None,
            ));
        }
        assert!(super::retained_entry_boundary_matches_publication(
            CompileOutputKind::CheckOnly,
            None,
            Some(1),
        ));
        assert!(!super::retained_entry_boundary_matches_publication(
            CompileOutputKind::CheckOnly,
            Some(1),
            Some(1),
        ));
        assert!(super::retained_entry_boundary_matches_publication(
            CompileOutputKind::NativeExecutable,
            Some(1),
            Some(1),
        ));
        assert!(!super::retained_entry_boundary_matches_publication(
            CompileOutputKind::NativeExecutable,
            None,
            Some(1),
        ));
        assert!(!super::retained_entry_boundary_matches_publication(
            CompileOutputKind::NativeExecutable,
            Some(2),
            Some(1),
        ));
        assert!(!super::retained_entry_boundary_matches_publication(
            CompileOutputKind::ObjectContainer,
            Some(1),
            Some(1),
        ));
        assert!(super::emitted_inventory_matches_publication(None, None));
        assert!(super::emitted_inventory_matches_publication(Some(1), None));
        assert!(super::emitted_inventory_matches_publication(
            Some(1),
            Some(1),
        ));
        assert!(!super::emitted_inventory_matches_publication(None, Some(1),));
        assert!(!super::emitted_inventory_matches_publication(
            Some(1),
            Some(2),
        ));
        assert!(super::emitted_validation_matches_publication(None, None));
        assert!(super::emitted_validation_matches_publication(
            Some((1, 2)),
            None,
        ));
        assert!(super::emitted_validation_matches_publication(
            Some((1, 2)),
            Some((1, 2)),
        ));
        assert!(!super::emitted_validation_matches_publication(
            None,
            Some((1, 2)),
        ));
        assert!(!super::emitted_validation_matches_publication(
            Some((1, 2)),
            Some((1, 3)),
        ));
        assert!(!super::emitted_validation_matches_publication(
            Some((1, 2)),
            Some((3, 2)),
        ));
        assert!(super::emitted_boundary_contract_matches_publication(
            None, None,
        ));
        assert!(super::emitted_boundary_contract_matches_publication(
            Some(1),
            None,
        ));
        assert!(super::emitted_boundary_contract_matches_publication(
            Some(1),
            Some(1),
        ));
        assert!(!super::emitted_boundary_contract_matches_publication(
            None,
            Some(1),
        ));
        assert!(!super::emitted_boundary_contract_matches_publication(
            Some(1),
            Some(2),
        ));

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
