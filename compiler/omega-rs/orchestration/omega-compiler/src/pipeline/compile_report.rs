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
    inventory_fingerprint: u64,
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
        inventory_fingerprint: u64,
        publication_evidence_fingerprint: u64,
        container_byte_count: usize,
        container_fingerprint: u64,
        installation_evidence_fingerprint: u64,
    ) -> Self {
        Self {
            destination,
            output_path,
            certificate_fingerprint,
            inventory_fingerprint,
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

    pub const fn inventory_fingerprint(&self) -> u64 {
        self.inventory_fingerprint
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    ) -> Result<Self, &'static str> {
        let report = Self {
            root_path,
            source_file_count,
            wrote_output,
            output_kind,
            executable_publication,
            app_bundle_publication,
            program_storage_entry,
            program_storage_entry_bridge,
            build_evaluation_usage,
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
            self.program_storage_entry_bridge
                .as_ref()
                .map(|bridge| bridge.emitted_wrapper_evidence().is_some()),
        )
    }

    /// Replays the only valid relationship between the flat executable and an
    /// optional app-bundle copy. This checks compiler-publication custody; it
    /// does not inspect or authorize runtime installation.
    pub fn has_consistent_executable_publication_custody(&self) -> bool {
        let cardinality_matches_kind = match self.output_kind {
            CompileOutputKind::CheckOnly => {
                !self.wrote_output
                    && self.executable_publication.is_none()
                    && self.app_bundle_publication.is_none()
            }
            CompileOutputKind::NativeExecutable => {
                self.wrote_output
                    && self.executable_publication.as_ref().is_some_and(|receipt| {
                        receipt.destination == ExecutablePublicationDestination::FlatOutput
                    })
            }
            CompileOutputKind::ObjectContainer => {
                self.wrote_output
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
            (None, None) | (Some(_), None) => true,
            (None, Some(_)) => false,
            (Some(flat), Some(bundle)) => {
                bundle.destination == ExecutablePublicationDestination::MacOsAppBundle
                    && expected_macos_app_bundle_executable_path(&self.root_path, &flat.output_path)
                        .as_deref()
                        == Some(bundle.output_path.as_path())
                    && flat.certificate_fingerprint == bundle.certificate_fingerprint
                    && flat.inventory_fingerprint == bundle.inventory_fingerprint
                    && flat.publication_evidence_fingerprint
                        == bundle.publication_evidence_fingerprint
                    && flat.container_byte_count == bundle.container_byte_count
                    && flat.container_fingerprint == bundle.container_fingerprint
                    && flat.installation_evidence_fingerprint
                        != bundle.installation_evidence_fingerprint
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
    bridge_has_emitted_evidence: Option<bool>,
) -> bool {
    match (output_kind, bridge_has_emitted_evidence) {
        (_, None) => true,
        (CompileOutputKind::CheckOnly, Some(false)) => true,
        (CompileOutputKind::NativeExecutable, Some(true)) => true,
        (CompileOutputKind::CheckOnly, Some(true))
        | (CompileOutputKind::NativeExecutable, Some(false))
        | (CompileOutputKind::ObjectContainer, Some(_)) => false,
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
        installation: u64,
    ) -> ExecutablePublicationReceipt {
        ExecutablePublicationReceipt::new(destination, path.into(), 1, 2, 3, 4, 5, installation)
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
            program_storage_entry: None,
            program_storage_entry_bridge: None,
            build_evaluation_usage: None,
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
            Some(false),
        ));
        assert!(!super::program_storage_emission_matches_output_kind(
            CompileOutputKind::CheckOnly,
            Some(true),
        ));
        assert!(super::program_storage_emission_matches_output_kind(
            CompileOutputKind::NativeExecutable,
            Some(true),
        ));
        assert!(!super::program_storage_emission_matches_output_kind(
            CompileOutputKind::NativeExecutable,
            Some(false),
        ));
        assert!(!super::program_storage_emission_matches_output_kind(
            CompileOutputKind::ObjectContainer,
            Some(false),
        ));
        assert!(!super::program_storage_emission_matches_output_kind(
            CompileOutputKind::ObjectContainer,
            Some(true),
        ));

        let flat = receipt(
            ExecutablePublicationDestination::FlatOutput,
            "build/main",
            6,
        );
        let bundle = receipt(
            ExecutablePublicationDestination::MacOsAppBundle,
            "build/Main.app/Contents/MacOS/main",
            7,
        );
        assert!(
            report(
                true,
                CompileOutputKind::NativeExecutable,
                Some(flat.clone()),
                Some(bundle.clone()),
            )
            .has_consistent_executable_publication_custody()
        );
        assert!(
            report(false, CompileOutputKind::CheckOnly, None, None)
                .has_consistent_executable_publication_custody()
        );
        assert!(
            report(true, CompileOutputKind::ObjectContainer, None, None)
                .has_consistent_executable_publication_custody()
        );
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
            )
            .is_err()
        );
        assert!(
            !report(true, CompileOutputKind::NativeExecutable, None, None)
                .has_consistent_executable_publication_custody()
        );
        assert!(
            !report(
                true,
                CompileOutputKind::ObjectContainer,
                Some(flat.clone()),
                None,
            )
            .has_consistent_executable_publication_custody()
        );
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
        assert!(
            !report(
                true,
                CompileOutputKind::NativeExecutable,
                None,
                Some(bundle.clone()),
            )
            .has_consistent_executable_publication_custody()
        );
        assert!(
            !report(
                true,
                CompileOutputKind::NativeExecutable,
                Some(flat.clone()),
                Some(flat.clone()),
            )
            .has_consistent_executable_publication_custody()
        );
        assert!(
            !report(
                true,
                CompileOutputKind::NativeExecutable,
                Some(bundle.clone()),
                Some(flat.clone()),
            )
            .has_consistent_executable_publication_custody()
        );
        let mut changed = bundle.clone();
        changed.output_path = "build/Other.app/Contents/MacOS/main".into();
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
        changed.certificate_fingerprint ^= 1;
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
        changed.inventory_fingerprint ^= 1;
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
        changed.container_byte_count += 1;
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
        changed.container_fingerprint ^= 1;
        assert!(
            !report(
                true,
                CompileOutputKind::NativeExecutable,
                Some(flat.clone()),
                Some(changed),
            )
            .has_consistent_executable_publication_custody()
        );
        let mut changed = bundle;
        changed.installation_evidence_fingerprint = flat.installation_evidence_fingerprint;
        assert!(
            !report(
                true,
                CompileOutputKind::NativeExecutable,
                Some(flat),
                Some(changed),
            )
            .has_consistent_executable_publication_custody()
        );
    }
}
