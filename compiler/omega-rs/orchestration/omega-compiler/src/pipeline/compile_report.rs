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
    pub root_path: PathBuf,
    pub source_file_count: usize,
    pub(crate) wrote_output: bool,
    /// Exact output category selected by orchestration. This distinguishes a
    /// native executable, which requires publication custody, from the
    /// non-executable object-container fallback.
    pub(crate) output_kind: CompileOutputKind,
    /// Exact checked publication receipt for a native executable image.
    /// Object-container fallbacks and check-only compilations retain `None`.
    pub(crate) executable_publication: Option<ExecutablePublicationReceipt>,
    /// Exact checked publication receipt for the executable copied into an
    /// optional macOS application bundle. Non-GUI/non-Mach-O builds retain
    /// `None`; this remains distinct from the flat executable receipt.
    pub(crate) app_bundle_publication: Option<ExecutablePublicationReceipt>,
    /// Exact target root-slot/schema/ABI-capture binding for a program-storage
    /// entry. Hosted compatibility entries and unmigrated name discovery have
    /// no such authority-bearing artifact.
    pub program_storage_entry: Option<super::ProgramStorageEntryPlanBinding>,
    /// Emitted object-entry handoff awaiting concrete environment supply and
    /// runtime installation. This is not an installation receipt.
    pub program_storage_entry_bridge: Option<super::ProgramStorageEntryNativeBridgePlan>,
    /// Deterministic accounting from the transitional typed-tree build
    /// evaluator. This is explicitly not terminal-Psi fuel.
    pub build_evaluation_usage: Option<super::build_config::BuildEvaluationUsage>,
}

impl CompileReport {
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
                    && flat.output_path != bundle.output_path
                    && flat.output_path.file_name() == bundle.output_path.file_name()
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
            root_path: "main.omg".into(),
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
