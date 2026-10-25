//! Publish independent component descriptions from their checked source custody.

mod component_publication;

use crate::CheckedCompilation;
use compilation_products::IndependentComponentProduct;
use diagnostics::Diagnostic;
use package_compilation::IndependentComponentDescription;

/// Publish one review description through the same batch producer as package
/// compilation. The public entrance owns its compiler stack; internal batches
/// already run on the package compilation worker.
pub fn publish_checked_component(
    checked: CheckedCompilation,
) -> Result<IndependentComponentDescription, Vec<Diagnostic>> {
    crate::checked::run_on_compile_thread(move || {
        let ((), product) = publish_checked_component_products(vec![((), Ok(checked))])
            .into_iter()
            .next()
            .ok_or_else(|| vec![Diagnostic::error("component publication lost its outcome")])?;
        product.map(|product| product.description().clone())
    })
}

/// Executable composition retains the completed source-free product beside
/// its review description. Review-only callers project the description above;
/// package preparation keeps both before releasing the source continuation.
/// Matching checked owners share Terminal production, but each application keeps
/// its own entry receipt, provider facts, destination and publication authority.
pub(crate) fn publish_checked_component_products<Owner>(
    checked: Vec<(Owner, Result<CheckedCompilation, Vec<Diagnostic>>)>,
) -> Vec<(Owner, Result<IndependentComponentProduct, Vec<Diagnostic>>)> {
    let mut applications = Vec::with_capacity(checked.len());
    for (owner, checked) in checked {
        let application = checked.and_then(|mut checked| {
            if checked.sources().package_identity().is_none() {
                return Err(vec![Diagnostic::error(
                    "component publication requires package-aware checked custody for the dependency compiled as its own component",
                )]);
            }
            let selections = optimization_core::OptimizationRollback::default()
                .settle(&checked.build().config.settings.optimizations);
            let timings = std::mem::take(checked.timings_mut());
            Ok((checked, timings, selections))
        });
        applications.push((owner, application));
    }

    let requests = applications
        .iter_mut()
        .filter_map(|(_, application)| application.as_mut().ok())
        .map(
            |(checked, timings, selections)| crate::terminal::TerminalProductRequest {
                view: crate::terminal::TerminalProductionView::checked(checked, timings),
                selections: selections.effective(),
            },
        )
        .collect();
    let mut produced = crate::terminal::produce_retained_terminal_artifacts(requests).into_iter();
    let mut outcomes = Vec::with_capacity(applications.len());
    for (owner, application) in applications {
        let product = application.and_then(|(checked, _, selections)| {
            let produced = produced.next().ok_or_else(|| {
                vec![Diagnostic::error(
                    "component publication lost its Terminal outcome",
                )]
            })??;
            let retained = crate::terminal::finish_terminal_application(
                &checked,
                produced,
                selections.effective(),
                &checked
                    .execution()
                    .selected_provider_plan_facts
                    .plans()
                    .iter()
                    .collect::<Vec<_>>(),
            )?;
            let description = component_publication::describe_component(&checked, &retained)?;
            let accepted_permissions =
                crate::native::accepted_terminal_authority_permission_policy(&checked)?;
            IndependentComponentProduct::new(description, retained, accepted_permissions)
                .map_err(|reason| vec![Diagnostic::error(reason)])
        });
        outcomes.push((owner, product));
    }
    outcomes
}
