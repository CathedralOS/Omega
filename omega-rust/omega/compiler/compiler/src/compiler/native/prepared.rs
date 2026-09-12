//! Prepared native compilation and exact Terminal-input reuse identity.

use super::{admission, realization};
use crate::compiler::CompileReport;
use crate::compiler::optimization::rollback;
use crate::compiler::request::ValidatedTargetCompilation;
use diagnostics::Diagnostic;

#[derive(Clone, PartialEq, Eq)]
pub(in crate::compiler) struct NativeInputReuseKey {
    terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
    admission_profile: proof_admission::AdmissionProfile,
    optimized: bool,
}

pub(in crate::compiler) struct PreparedNativeCompilation {
    pub(super) request: ValidatedTargetCompilation,
    pub(super) checked: crate::pipeline::CheckedCompilation,
    pub(super) admission: admission::NativeCompilationAdmission,
    pub(super) rollback: rollback::OptimizationRollbackSettlement,
    pub(super) terminal: realization::PreparedTerminalNativeArtifact,
    pub(super) production_subject: Option<crate::compiler::ProductionCompilationSubject>,
    pub(super) source_file_count: usize,
}

impl PreparedNativeCompilation {
    pub(in crate::compiler::native) fn new(
        request: ValidatedTargetCompilation,
        checked: crate::pipeline::CheckedCompilation,
        admission: admission::NativeCompilationAdmission,
        rollback: rollback::OptimizationRollbackSettlement,
        terminal: realization::PreparedTerminalNativeArtifact,
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
        let post_terminal = self.rollback.effective().project_post_terminal();
        NativeInputReuseKey {
            terminal_artifact_identity: self.terminal.artifact().manifest().identity(),
            admission_profile: self
                .request
                .configuration
                .terminal_admission_profile
                .clone(),
            optimized: !post_terminal.selections().is_empty(),
        }
    }
}

impl PreparedNativeCompilation {
    pub(in crate::compiler) fn prepare_reusable_input(
        &self,
    ) -> Result<::native_realization::PreparedNativeRealizationInput, Vec<Diagnostic>> {
        let post_terminal = self.rollback.effective().project_post_terminal();
        ::native_realization::prepare_native_realization_input(
            self.terminal.artifact(),
            &self.request.configuration.terminal_admission_profile,
            post_terminal.selections(),
        )
    }

    pub(in crate::compiler) fn finish(
        self,
        prepared_input: &::native_realization::PreparedNativeRealizationInput,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let Self {
            request,
            checked,
            admission,
            rollback,
            terminal,
            production_subject,
            source_file_count,
        } = self;
        let options = request.options;
        let crate::compiler::TargetCompileConfiguration {
            terminal_admission_profile,
            terminal_authority_permission_policy,
            ..
        } = request.configuration;
        let post_terminal = rollback.effective().project_post_terminal();
        let artifact = realization::realize(
            &checked,
            &admission,
            &terminal_admission_profile,
            terminal_authority_permission_policy,
            post_terminal.selections(),
            terminal,
            prepared_input,
        )?;
        let report = CompileReport::from_retained_native_artifact(
            options.root_path,
            source_file_count,
            artifact,
            rollback.into_receipt(),
            production_subject,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        crate::compiler::native_checked::NativeCompilationWithCheckedReceipt::new(checked, report)
            .map(crate::compiler::native_checked::NativeCompilationWithCheckedReceipt::into_report)
            .map_err(|message| vec![Diagnostic::error(message)])
    }
}
