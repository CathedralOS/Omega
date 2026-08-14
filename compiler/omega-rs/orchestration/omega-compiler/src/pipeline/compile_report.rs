use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileReport {
    pub root_path: PathBuf,
    pub source_file_count: usize,
    pub wrote_output: bool,
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
