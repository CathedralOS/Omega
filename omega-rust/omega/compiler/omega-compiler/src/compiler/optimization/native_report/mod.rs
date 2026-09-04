//! Optimizer module role: stage group. Prepared native-report execution.

mod model;

pub(in crate::compiler) use model::{NativeInputReuseKey, PreparedNativeReport};

use super::native_realization;
use crate::compiler::{CompileReport, CompileRequest};
use psi_diagnostics::Diagnostic;

impl PreparedNativeReport {
    pub(in crate::compiler) fn prepare_reusable_input(
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

    pub(in crate::compiler) fn finish(
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
        crate::compiler::native_checked::NativeCompilationWithCheckedReceipt::new(checked, report)
            .map(crate::compiler::native_checked::NativeCompilationWithCheckedReceipt::into_report)
            .map_err(|message| vec![Diagnostic::error(message)])
    }
}
