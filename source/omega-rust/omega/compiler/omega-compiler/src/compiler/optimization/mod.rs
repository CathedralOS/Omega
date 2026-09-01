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

use crate::compiler::CompileReport;
use crate::compiler::request::ValidatedCompileRequest;
use psi_diagnostics::Diagnostic;

pub(super) fn native_report(
    request: ValidatedCompileRequest,
    checked: &crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let request = request.into_inner();
    admission::reject_unconsumed_callbacks(checked)?;
    let production_subject = crate::pipeline::reporting::project_production_subject(checked)?;
    let source_file_count = checked.source_file_count();
    let admission = admission::admit(checked)?;
    let rollback = request
        .optimization_rollback
        .settle(checked.optimization_selections());
    let artifact = native_realization::realize(
        checked,
        admission,
        &request.terminal_admission_profile,
        rollback.effective(),
    )?;
    CompileReport::from_retained_native_artifact(
        request.options.root_path,
        source_file_count,
        artifact,
        rollback.into_receipt(),
        production_subject,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}
