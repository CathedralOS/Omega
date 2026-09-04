//! Optimizer module role: executable entrance. Compiler-owned native optimization realization.
//!
//! This join admits checked custody, settles the subtractive release overlay,
//! and passes the one effective exact selection into native realization.

mod admission;
#[cfg(any(test, feature = "experimental-external-optimization-policy"))]
pub(crate) mod external_policy;
mod native_realization;
mod native_report;
pub mod rollback;

pub(super) use native_report::{NativeInputReuseKey, PreparedNativeReport};
pub use rollback::{OptimizationRollback, OptimizationRollbackInputError};

use crate::compiler::CompileReport;
use crate::compiler::request::ValidatedCompileRequest;
use psi_diagnostics::Diagnostic;

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
    let terminal =
        native_realization::prepare_terminal_artifact(&checked, &admission, rollback.effective())?;
    Ok(PreparedNativeReport::new(
        request,
        checked,
        admission,
        rollback,
        terminal,
        production_subject,
        source_file_count,
    ))
}

pub(super) fn native_report(
    request: ValidatedCompileRequest,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let prepared = prepare_native_report(request, checked)?;
    let reusable_input = prepared.prepare_reusable_input()?;
    prepared.finish(&reusable_input)
}
