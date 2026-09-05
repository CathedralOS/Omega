//! Optimizer module role: stage group. Prepared native-report execution.

mod model;

pub(in crate::compiler) use model::{NativeInputReuseKey, PreparedNativeReport};

use super::native_realization;
use crate::compiler::{CompileReport, CompileRequest};
use diagnostics::Diagnostic;

impl PreparedNativeReport {
    pub(in crate::compiler) fn prepare_reusable_input(
        &self,
    ) -> Result<::native_realization::PreparedNativeRealizationInput, Vec<Diagnostic>> {
        let post_terminal = self.rollback.effective().project_post_terminal();
        ::native_realization::prepare_native_realization_input(
            self.terminal.artifact(),
            &self.request.terminal_admission_profile,
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
        let CompileRequest {
            options,
            terminal_admission_profile,
            terminal_authority_permission_policy,
            ..
        } = request;
        let post_terminal = rollback.effective().project_post_terminal();
        let artifact = native_realization::realize(
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
