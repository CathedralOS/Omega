//! Native product preparation and realization after checked-Psi admission.

mod admission;
mod prepared;
mod realization;
mod targets;
pub(super) use prepared::{NativeInputReuseKey, PreparedNativeCompilation};
pub(super) use targets::compile_targets;

use crate::compiler::request::ValidatedTargetCompilation;
use diagnostics::Diagnostic;

pub(super) fn prepare(
    request: ValidatedTargetCompilation,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<PreparedNativeCompilation, Vec<Diagnostic>> {
    admission::reject_unconsumed_callbacks(&checked)?;
    let production_subject = crate::pipeline::reporting::project_production_subject(&checked)?;
    let source_file_count = checked.source_file_count();
    let admission = admission::admit(&checked)?;
    let rollback = request
        .configuration
        .optimization_rollback
        .settle(checked.optimization_selections());
    realization::validate_terminal_authority_permissions(
        &checked,
        &request.configuration.terminal_authority_permission_policy,
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
