//! Continue package-manager-owned checked production without reopening its inputs.

use crate::CheckedCompilation;
use artifacts::compile_timings::CompileTimings;
use compilation_products::RetainedTerminalArtifact;
use diagnostics::Diagnostic;

/// Revalidate package custody before producing the retained Terminal artifact.
/// Final product requirements remain with the caller; this is an intermediate
/// artifact used by native production and independent component descriptions.
/// Package review owns the checked input; this does not rerun its build machine.
/// The returned timings carry the checked record's ladder after production
/// appended its terminal rows; the record itself is consumed here, so the
/// measurements travel with the artifact's caller.
pub fn retained_terminal_artifact_from_checked_package(
    mut checked: CheckedCompilation,
    profile: proof_admission::AdmissionProfile,
    rollback: optimization_core::OptimizationRollback,
) -> Result<(RetainedTerminalArtifact, CompileTimings), Vec<Diagnostic>> {
    crate::checked::run_on_compile_thread(move || {
        checked.verify_current_source_consumption()?;
        if checked.production_subject()?.is_none() {
            return Err(vec![Diagnostic::error(
                "reviewed package Terminal production requires package-aware checked custody",
            )]);
        }
        let selections = rollback.settle(&checked.build().config.settings.optimizations);
        let retained = crate::terminal::produce_retained_terminal_artifact(
            &mut checked,
            &profile,
            selections.effective(),
        )?;
        Ok((retained, std::mem::take(checked.timings_mut())))
    })
}
