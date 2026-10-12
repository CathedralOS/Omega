//! Publish independent component descriptions from their checked source custody.

mod component_publication;

use crate::CheckedCompilation;
use diagnostics::Diagnostic;
use package_compilation::IndependentComponentDescription;

/// Publish one review description through the same batch producer as package
/// compilation. The public entrance owns its compiler stack; internal batches
/// already run on the package compilation worker.
pub fn publish_checked_component(
    checked: CheckedCompilation,
) -> Result<IndependentComponentDescription, Vec<Diagnostic>> {
    crate::checked::run_on_compile_thread(move || {
        publish_checked_components(vec![Ok(checked)])
            .into_iter()
            .next()
            .ok_or_else(|| vec![Diagnostic::error("component publication lost its outcome")])?
    })
}

/// Matching checked component owners share Terminal production. Each application
/// retains its own entry receipt, provider facts and description; neither a
/// sibling's publication nor source equality grants receiving authority.
pub(crate) fn publish_checked_components(
    checked: Vec<Result<CheckedCompilation, Vec<Diagnostic>>>,
) -> Vec<Result<IndependentComponentDescription, Vec<Diagnostic>>> {
    let mut outcomes: Vec<_> = (0..checked.len()).map(|_| None).collect();
    let mut applications = Vec::new();
    for (position, checked) in checked.into_iter().enumerate() {
        match checked.and_then(|checked| {
            if checked.sources().package_identity().is_none() {
                return Err(vec![Diagnostic::error(
                    "component publication requires package-aware checked custody for the dependency compiled as its own component",
                )]);
            }
            Ok(checked)
        }) {
            Ok(mut checked) => {
                let selections = optimization_core::OptimizationRollback::default()
                    .settle(&checked.build().config.settings.optimizations);
                let timings = std::mem::take(checked.timings_mut());
                applications.push((position, checked, timings, selections));
            }
            Err(diagnostics) => outcomes[position] = Some(Err(diagnostics)),
        }
    }

    let requests = applications
        .iter_mut()
        .map(
            |(_, checked, timings, selections)| crate::terminal::TerminalProductRequest {
                view: crate::terminal::TerminalProductionView::checked(checked, timings),
                selections: selections.effective(),
            },
        )
        .collect();
    let produced = crate::terminal::produce_retained_terminal_artifacts(requests);
    for ((position, checked, _, selections), produced) in applications.into_iter().zip(produced) {
        outcomes[position] = Some(produced.and_then(|produced| {
            let retained = crate::terminal::finish_terminal_application(
                &checked,
                produced,
                selections.effective(),
            )?;
            component_publication::describe_component(&checked, &retained)
        }));
    }
    outcomes
        .into_iter()
        .map(|outcome| {
            outcome.unwrap_or_else(|| {
                Err(vec![Diagnostic::error(
                    "component publication lost its outcome",
                )])
            })
        })
        .collect()
}
