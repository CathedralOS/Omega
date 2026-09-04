//! Prepared native-report state and exact Terminal-input reuse identity.

use super::super::{admission, native_realization, rollback};
use crate::compiler::CompileRequest;

#[derive(Clone, PartialEq, Eq)]
pub(in crate::compiler) struct NativeInputReuseKey {
    terminal_artifact_identity: psi_terminal_codec::TerminalArtifactIdentity,
    admission_profile: psi_proof_admission::AdmissionProfile,
    optimized: bool,
}

pub(in crate::compiler) struct PreparedNativeReport {
    pub(super) request: CompileRequest,
    pub(super) checked: crate::pipeline::CheckedCompilation,
    pub(super) admission: admission::NativeOptimizationAdmission,
    pub(super) rollback: rollback::OptimizationRollbackSettlement,
    pub(super) terminal: native_realization::PreparedTerminalNativeArtifact,
    pub(super) production_subject: Option<crate::compiler::ProductionCompilationSubject>,
    pub(super) source_file_count: usize,
}

impl PreparedNativeReport {
    pub(in crate::compiler::optimization) fn new(
        request: CompileRequest,
        checked: crate::pipeline::CheckedCompilation,
        admission: admission::NativeOptimizationAdmission,
        rollback: rollback::OptimizationRollbackSettlement,
        terminal: native_realization::PreparedTerminalNativeArtifact,
        production_subject: Option<crate::compiler::ProductionCompilationSubject>,
        source_file_count: usize,
    ) -> Self {
        Self {
            request,
            checked,
            admission,
            rollback,
            terminal,
            production_subject,
            source_file_count,
        }
    }

    pub(in crate::compiler) fn reuse_key(&self) -> NativeInputReuseKey {
        NativeInputReuseKey {
            terminal_artifact_identity: self.terminal.artifact().manifest().identity(),
            admission_profile: self.request.terminal_admission_profile.clone(),
            optimized: !self.rollback.effective().is_empty(),
        }
    }
}
