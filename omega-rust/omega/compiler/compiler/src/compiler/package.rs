//! Continue package-manager-owned checked production without reopening its inputs.

use crate::{CheckedCompilation, CompileReport, OptimizationRollback};
use diagnostics::Diagnostic;
use std::path::PathBuf;

/// Revalidate package custody before producing a retained Terminal report.
/// Package review owns the checked input; this does not rerun its build machine.
pub fn retained_terminal_report_from_checked_package(
    root_path: PathBuf,
    checked: CheckedCompilation,
    profile: proof_admission::AdmissionProfile,
) -> Result<CompileReport, Vec<Diagnostic>> {
    super::execution::run_on_compile_thread(move || {
        checked.verify_current_source_consumption()?;
        if crate::pipeline::reporting::project_production_subject(&checked)?.is_none() {
            return Err(vec![Diagnostic::error(
                "reviewed package Terminal production requires package-aware checked custody",
            )]);
        }
        super::terminal_product::compile_report(
            root_path,
            checked,
            &profile,
            &OptimizationRollback::default(),
        )
    })
}
