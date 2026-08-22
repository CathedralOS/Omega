use std::path::PathBuf;

/// Immutable custody for one compiler-published executable container.
///
/// This records the exact final-footprint certificate, publication seal, and
/// checked atomic installation that produced `output_path`. It is compiler
/// artifact evidence only; it does not authorize loading or runtime
/// installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutablePublicationReceipt {
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
        output_path: PathBuf,
        certificate_fingerprint: u64,
        inventory_fingerprint: u64,
        publication_evidence_fingerprint: u64,
        container_byte_count: usize,
        container_fingerprint: u64,
        installation_evidence_fingerprint: u64,
    ) -> Self {
        Self {
            output_path,
            certificate_fingerprint,
            inventory_fingerprint,
            publication_evidence_fingerprint,
            container_byte_count,
            container_fingerprint,
            installation_evidence_fingerprint,
        }
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
    pub wrote_output: bool,
    /// Exact checked publication receipt for a native executable image.
    /// Object-container fallbacks and check-only compilations retain `None`.
    pub executable_publication: Option<ExecutablePublicationReceipt>,
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
    pub fn summary(&self) -> String {
        format!(
            "compiled {} source file(s) from {}; write_output={}",
            self.source_file_count,
            self.root_path.display(),
            self.wrote_output
        )
    }
}
