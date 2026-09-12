//! Native product preparation and realization after checked-Psi admission.

mod admission;
mod prepared;
mod realization;
pub(super) use prepared::{NativeInputReuseKey, PreparedNativeCompilation};

use crate::compiler::CompileReport;
use crate::compiler::request::ValidatedCompileRequest;
use diagnostics::Diagnostic;

pub(super) fn prepare(
    request: ValidatedCompileRequest,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<PreparedNativeCompilation, Vec<Diagnostic>> {
    let request = request.into_inner();
    admission::reject_unconsumed_callbacks(&checked)?;
    let production_subject = crate::pipeline::reporting::project_production_subject(&checked)?;
    let source_file_count = checked.source_file_count();
    let admission = admission::admit(&checked)?;
    let rollback = request
        .optimization_rollback
        .settle(checked.optimization_selections());
    realization::validate_terminal_authority_permissions(
        &checked,
        &request.terminal_authority_permission_policy,
    )?;
    let terminal =
        realization::prepare_terminal_artifact(&checked, &admission, rollback.effective())?;
    Ok(PreparedNativeCompilation::new(
        request,
        checked,
        admission,
        rollback,
        terminal,
        production_subject,
        source_file_count,
    ))
}

pub(super) fn compile(
    request: ValidatedCompileRequest,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let prepared = prepare(request, checked)?;
    let reusable_input = prepared.prepare_reusable_input()?;
    prepared.finish(&reusable_input)
}
