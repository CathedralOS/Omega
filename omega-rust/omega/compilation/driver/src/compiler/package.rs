//! Continue package-manager-owned checked production without reopening its inputs.

use crate::CheckedCompilation;
use artifacts::compile_timings::CompileTimings;
use compilation_products::RetainedTerminalArtifact;
use diagnostics::Diagnostic;

/// Produce the retained Terminal artifact of a package-manager-owned checked
/// record. The checked record carries the sources it consumed; this does not
/// reopen them.
/// Final product requirements remain with the caller, including the custody a
/// product report needs: accepted native realization requires the production
/// subject its report settlement admitted, and component publication requires
/// the dependency's package identity. This is an intermediate artifact used by
/// native production and independent component descriptions.
/// Package review owns the checked input; this does not rerun its build machine.
/// The returned timings carry the checked record's ladder after production
/// appended its terminal rows; the record itself is consumed here, so the
/// measurements travel with the artifact's caller.
pub fn retained_terminal_artifact_from_checked_package(
    mut checked: CheckedCompilation,
    rollback: optimization_core::OptimizationRollback,
) -> Result<(RetainedTerminalArtifact, CompileTimings), Vec<Diagnostic>> {
    crate::checked::run_on_compile_thread(move || {
        let selections = rollback.settle(&checked.build().config.settings.optimizations);
        let retained = crate::terminal::produce_retained_terminal_artifact(
            &mut checked,
            selections.effective(),
        )?;
        Ok((retained.artifact, std::mem::take(checked.timings_mut())))
    })
}
