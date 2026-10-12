//! Publish independent component products from their checked source custody.

mod component_publication;
pub use component_publication::publish_checked_component;

use crate::CheckedCompilation;
use artifacts::compile_timings::CompileTimings;
use compilation_products::RetainedTerminalArtifact;
use diagnostics::Diagnostic;

/// Produce a dependency's Terminal artifact for component publication from its
/// retained checked custody. No sources reopen and no Build runs again. Timings
/// include Terminal production because this consumes the checked record.
fn retained_terminal_artifact_from_checked_package(
    mut checked: CheckedCompilation,
    rollback: optimization_core::OptimizationRollback,
) -> Result<(RetainedTerminalArtifact, CompileTimings), Vec<Diagnostic>> {
    crate::checked::run_on_compile_thread(move || {
        let selections = rollback.settle(&checked.build().config.settings.optimizations);
        let mut timings = std::mem::take(checked.timings_mut());
        let produced = crate::terminal::produce_retained_terminal_artifacts(vec![
            crate::terminal::TerminalProductRequest {
                view: crate::terminal::TerminalProductionView::checked(&checked, &mut timings),
                selections: selections.effective(),
            },
        ])
        .into_iter()
        .next()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "Terminal production lost its package outcome",
            )]
        })??;
        let retained = crate::terminal::finish_terminal_application(
            &checked,
            produced,
            selections.effective(),
        )?;
        Ok((retained, timings))
    })
}
