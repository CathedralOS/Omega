//! Optimizer module role: executable entrance. Compiler-owned native optimization realization.
//!
//! This join admits checked custody, settles the subtractive release overlay,
//! and passes the one effective exact selection into native realization.

mod admission;
#[cfg(any(test, feature = "experimental-external-optimization-policy"))]
pub(crate) mod external_policy;
mod native_realization;
pub mod rollback;

pub use rollback::{OptimizationRollback, OptimizationRollbackInputError};

use crate::compiler::request::ValidatedCompileRequest;
use crate::compiler::{CompileReport, CompileRequest};
use psi_diagnostics::Diagnostic;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct NativeInputReuseKey {
    terminal_artifact_identity: psi_terminal_codec::TerminalArtifactIdentity,
    admission_profile: psi_proof_admission::AdmissionProfile,
    optimized: bool,
}

pub(super) struct PreparedNativeReport {
    request: CompileRequest,
    checked: crate::pipeline::CheckedCompilation,
    admission: admission::NativeOptimizationAdmission,
    rollback: rollback::OptimizationRollbackSettlement,
    terminal: native_realization::PreparedTerminalNativeArtifact,
    production_subject: Option<crate::compiler::ProductionCompilationSubject>,
    source_file_count: usize,
}

impl PreparedNativeReport {
    pub(super) fn reuse_key(&self) -> NativeInputReuseKey {
        NativeInputReuseKey {
            terminal_artifact_identity: self.terminal.artifact().manifest().identity(),
            admission_profile: self.request.terminal_admission_profile.clone(),
            optimized: !self.rollback.effective().is_empty(),
        }
    }

    pub(super) fn prepare_reusable_input(
        &self,
    ) -> Result<
        omega_terminal_psi_to_native_artifact::PreparedNativeRealizationInput,
        Vec<Diagnostic>,
    > {
        omega_terminal_psi_to_native_artifact::prepare_native_realization_input(
            self.terminal.artifact(),
            &self.request.terminal_admission_profile,
            self.rollback.effective(),
        )
    }

    pub(super) fn finish(
        self,
        prepared_input: &omega_terminal_psi_to_native_artifact::PreparedNativeRealizationInput,
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
        let CompileRequest {
            options,
            terminal_admission_profile,
            terminal_authority_permission_policy,
            ..
        } = request;
        let artifact = native_realization::realize(
            &checked,
            &admission,
            &terminal_admission_profile,
            terminal_authority_permission_policy,
            rollback.effective(),
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
        super::native_checked::NativeCompilationWithCheckedReceipt::new(checked, report)
            .map(super::native_checked::NativeCompilationWithCheckedReceipt::into_report)
            .map_err(|message| vec![Diagnostic::error(message)])
    }
}

pub(super) fn prepare_native_report(
    request: ValidatedCompileRequest,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<PreparedNativeReport, Vec<Diagnostic>> {
    let request = request.into_inner();
    admission::reject_unconsumed_callbacks(&checked)?;
    let production_subject = crate::pipeline::reporting::project_production_subject(&checked)?;
    let source_file_count = checked.source_file_count();
    let admission = admission::admit(&checked)?;
    let rollback = request
        .optimization_rollback
        .settle(checked.optimization_selections());
    native_realization::validate_terminal_authority_permissions(
        &checked,
        &request.terminal_authority_permission_policy,
    )?;
    let terminal = native_realization::prepare_terminal_artifact(&checked, &admission)?;
    Ok(PreparedNativeReport {
        request,
        checked,
        admission,
        rollback,
        terminal,
        production_subject,
        source_file_count,
    })
}

pub(super) fn native_report(
    request: ValidatedCompileRequest,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let prepared = prepare_native_report(request, checked)?;
    let reusable_input = prepared.prepare_reusable_input()?;
    prepared.finish(&reusable_input)
}
