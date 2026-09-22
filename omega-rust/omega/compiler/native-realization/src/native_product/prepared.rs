//! Prepared native compilation and exact Terminal-input reuse identity.

use super::{NativeProductRequest, admission, realization};
use assembled_syntax_to_checked_compilation::CheckedCompilation;
use checked_compilation_to_terminal_artifact::ProgramEntryTerminalArtifact;
use compilation_report::CompileReport;
use diagnostics::Diagnostic;

/// The identity under which two targets share one prepared native input.
#[derive(Clone, PartialEq, Eq)]
pub struct NativeInputReuseKey {
    terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
    admission_profile: proof_admission::AdmissionProfile,
    post_terminal_optimizations: optimization_core::PostTerminalOptimizationSelections,
}

/// One admitted native product before physical realization.
pub struct PreparedNativeCompilation {
    pub(super) request: NativeProductRequest,
    pub(super) checked: CheckedCompilation,
    pub(super) admission: admission::NativeCompilationAdmission,
    pub(super) rollback: assembled_syntax_to_checked_compilation::OptimizationRollbackSettlement,
    pub(super) terminal: ProgramEntryTerminalArtifact,
    pub(super) production_subject: Option<compilation_report::ProductionCompilationSubject>,
    pub(super) source_file_count: usize,
}

impl PreparedNativeCompilation {
    pub(super) fn new(
        request: NativeProductRequest,
        checked: CheckedCompilation,
        admission: admission::NativeCompilationAdmission,
        rollback: assembled_syntax_to_checked_compilation::OptimizationRollbackSettlement,
        terminal: ProgramEntryTerminalArtifact,
        production_subject: Option<compilation_report::ProductionCompilationSubject>,
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

    pub(super) fn reuse_key(&self) -> NativeInputReuseKey {
        let post_terminal = self.rollback.effective().project_post_terminal();
        NativeInputReuseKey {
            terminal_artifact_identity: self.terminal.artifact().manifest().identity(),
            admission_profile: self.request.terminal_admission_profile.clone(),
            post_terminal_optimizations: post_terminal.selections().clone(),
        }
    }
}

impl PreparedNativeCompilation {
    pub(super) fn prepare_reusable_input(
        &self,
    ) -> Result<crate::PreparedNativeRealizationInput, Vec<Diagnostic>> {
        let post_terminal = self.rollback.effective().project_post_terminal();
        crate::prepare_native_realization_input(
            self.terminal.artifact(),
            &self.request.terminal_admission_profile,
            post_terminal.selections(),
            &[],
        )
    }

    pub(super) fn finish(
        self,
        prepared_input: &crate::PreparedNativeRealizationInput,
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
        let NativeProductRequest {
            root_path,
            terminal_admission_profile,
            terminal_authority_policy,
            terminal_authority_permission_policy,
            ..
        } = request;
        let post_terminal = rollback.effective().project_post_terminal();
        let pcc_requests = checked.pcc_requests();
        let artifact = realization::realize(
            &checked,
            &admission,
            &terminal_admission_profile,
            terminal_authority_policy,
            terminal_authority_permission_policy,
            post_terminal.selections(),
            terminal,
            prepared_input,
        )?;
        let report = CompileReport::from_retained_native_artifact(
            root_path,
            source_file_count,
            artifact,
            rollback.into_receipt(),
            production_subject,
        )
        .map(|report| report.with_pcc_context(pcc_requests, terminal_admission_profile))
        .and_then(|report| {
            report.with_application_metadata(
                checked
                    .application_name()
                    .map(|name| name.as_str().to_owned()),
                checked.application_intent(),
                checked.application_identifier().cloned(),
            )
        })
        .map_err(|message| vec![Diagnostic::error(message)])?;
        super::receipt::NativeCompilationWithCheckedReceipt::new(checked, report)
            .map(super::receipt::NativeCompilationWithCheckedReceipt::into_report)
            .map_err(|message| vec![Diagnostic::error(message)])
    }
}

#[cfg(any(test, feature = "test-support"))]
impl NativeInputReuseKey {
    /// Build one reuse key from its parts so tests outside this crate can
    /// check which prepared inputs an invocation shares.
    pub fn from_parts(
        terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
        admission_profile: proof_admission::AdmissionProfile,
        post_terminal_optimizations: optimization_core::PostTerminalOptimizationSelections,
    ) -> Self {
        Self {
            terminal_artifact_identity,
            admission_profile,
            post_terminal_optimizations,
        }
    }

    pub fn post_terminal_optimizations(
        &self,
    ) -> &optimization_core::PostTerminalOptimizationSelections {
        &self.post_terminal_optimizations
    }
}
