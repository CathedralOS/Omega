/*
We batch and reuse the published `PortableTerminalProgram` across the targets
that share a checked source and Psi optimization projection.

`produce_retained_terminal_artifacts` groups requests that share a checked
source and equal Psi production inputs (`shares_production`). Each cohort
obtains the shared `PortableTerminalProgram` once (`produce_shared_terminal`);
every request of the cohort then forms its own target's provider proposal from
that artifact and its settled provider plans
(`SelectedExecution::provider_proposal`), and the caller's request order is
restored.

A product here is descriptive: the accepted program and the provider proposal
its target-local conditions are checked against. An executable application
also needs the selected program entry, its callback roots and behavior
exclusions; none has a producer on the lowered route, so the compiler refuses
those products before they request a production (`product_execution`).
*/

use super::product_code::ProductCodeSource;
use crate::execution_settlement::SelectedProviderCustody;
use crate::observations::PsiStageTimings;
use artifacts::compile_timings::{CompileTimings, StageMeta};
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use std::sync::Arc;

const TERMINAL_PRODUCTION_STAGE: StageMeta = StageMeta::new(
    "terminal-production",
    "FrozenTypedTrees",
    "CanonicalTerminalArtifact",
);

pub(crate) struct TerminalProductionView<'a> {
    source_identity: compilation_products::ProductSourceIdentity,
    publication: &'a compilation_products::ProductTerminalPublication,
    sources: &'a package_compilation::CheckedSourceCustody,
    execution: &'a selected_dispatch::SelectedExecution,
    timings: &'a mut CompileTimings,
}

impl<'a> TerminalProductionView<'a> {
    // Roots decide which machines the published Terminal program retains, so
    // two products share a production only when they share that publication
    // as well as checked source: a separately selected test root over the same
    // source must not adopt the application's program.
    fn shares_source(&self, other: &Self) -> bool {
        self.source_identity.same_source(&other.source_identity)
            && self.sources == other.sources
            && std::ptr::eq(self.publication, other.publication)
    }

    pub(crate) fn selection(selection: &'a mut SelectedProviderCustody) -> Self {
        Self {
            source_identity: compilation_products::ProductSourceIdentity::new(
                selection.source_inspection.program(),
            ),
            publication: &selection.terminal_publication,
            sources: &selection.sources,
            execution: &selection.execution,
            timings: &mut selection.timings,
        }
    }
}

pub(crate) struct TerminalProductRequest<'a> {
    pub view: TerminalProductionView<'a>,
    pub selections: &'a OptimizationSelections,
    pub use_kind: TerminalProductUse<'a>,
}

#[derive(Clone, Copy)]
pub(crate) enum TerminalProductUse<'a> {
    Declarations,
    Inspection {
        name: &'a str,
        source: ProductCodeSource<'a>,
    },
}

impl PartialEq for TerminalProductUse<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Declarations, Self::Declarations) => true,
            (Self::Inspection { name: left, .. }, Self::Inspection { name: right, .. }) => {
                left == right
            }
            _ => false,
        }
    }
}
impl Eq for TerminalProductUse<'_> {}

impl<'a> TerminalProductRequest<'a> {
    pub(crate) fn declarations(
        selection: &'a mut SelectedProviderCustody,
        selections: &'a OptimizationSelections,
    ) -> Self {
        Self {
            view: TerminalProductionView::selection(selection),
            selections,
            use_kind: TerminalProductUse::Declarations,
        }
    }

    pub(crate) fn inspection(
        selection: &'a mut SelectedProviderCustody,
        name: &'a str,
        selections: &'a OptimizationSelections,
    ) -> Self {
        let view = TerminalProductionView::selection(selection);
        let source = ProductCodeSource {
            publication: view.publication,
        };
        Self {
            view,
            selections,
            use_kind: TerminalProductUse::Inspection { name, source },
        }
    }
}

pub(crate) struct ProducedTerminalProduct {
    pub(crate) program: Arc<psi_compiler::PortableTerminalProgram>,
    pub(super) proposal: backend_plan::TerminalTargetProviderProposal,
}

impl ProducedTerminalProduct {
    pub(crate) fn module(&self) -> &terminal_psi::TerminalModule {
        self.program.artifact().view().module()
    }

    // Target-local scalar conditions of the produced program, evaluated for
    // the selected target against its provider proposal.
    pub(crate) fn check_conditions(
        &self,
        selection: &mut SelectedProviderCustody,
    ) -> Result<(), Vec<Diagnostic>> {
        build_settings::scalar_completion::complete(
            self.program.artifact(),
            &self.proposal.target,
            &[],
            std::slice::from_ref(&self.proposal),
            &mut selection.source_completion_accounting,
        )
        .map_err(|reason| vec![Diagnostic::error(format!("constant checking: {reason}"))])?;
        Ok(())
    }
}

struct PreparedTerminalProduct<'a> {
    position: usize,
    request: TerminalProductRequest<'a>,
}

impl PreparedTerminalProduct<'_> {
    fn shares_production(&self, other: &Self) -> bool {
        self.request.use_kind == other.request.use_kind
            && self.request.view.publication.optimizations().project_psi()
                == other.request.view.publication.optimizations().project_psi()
            && self.request.view.shares_source(&other.request.view)
            && self.request.selections.project_psi() == other.request.selections.project_psi()
    }
}

pub(crate) fn produce_retained_terminal_artifacts(
    requests: Vec<TerminalProductRequest<'_>>,
) -> Vec<Result<ProducedTerminalProduct, Vec<Diagnostic>>> {
    let mut outcomes = Vec::with_capacity(requests.len());
    let mut cohorts: Vec<Vec<PreparedTerminalProduct<'_>>> = Vec::new();
    for (position, request) in requests.into_iter().enumerate() {
        let product = PreparedTerminalProduct { position, request };
        match cohorts
            .iter_mut()
            .find(|cohort| cohort[0].shares_production(&product))
        {
            Some(cohort) => cohort.push(product),
            None => cohorts.push(vec![product]),
        }
    }

    for mut cohort in cohorts {
        let mut timings = std::mem::take(cohort[0].request.view.timings);
        timings.add_completed(TERMINAL_PRODUCTION_STAGE, 0);
        let started = std::time::Instant::now();
        let produced = produce_shared_terminal(&cohort[0], &mut timings);
        let elapsed = started.elapsed().as_micros();
        timings.add_completed(TERMINAL_PRODUCTION_STAGE, elapsed);
        *cohort[0].request.view.timings = timings;
        for product in &mut cohort[1..] {
            product
                .request
                .view
                .timings
                .add_completed(TERMINAL_PRODUCTION_STAGE, elapsed);
        }
        for product in cohort {
            let outcome = produced.clone().map(|program| ProducedTerminalProduct {
                proposal: product.request.view.execution.provider_proposal(
                    program.artifact().view(),
                    product.request.selections.identity(),
                ),
                program,
            });
            outcomes.push((product.position, outcome));
        }
    }
    outcomes.sort_unstable_by_key(|(position, _)| *position);
    outcomes.into_iter().map(|(_, outcome)| outcome).collect()
}

fn produce_shared_terminal(
    first: &PreparedTerminalProduct<'_>,
    timings: &mut CompileTimings,
) -> Result<Arc<psi_compiler::PortableTerminalProgram>, Vec<Diagnostic>> {
    let mut observer = PsiStageTimings::new(timings);
    match first.request.use_kind {
        TerminalProductUse::Inspection { name, source } => {
            source.inspect(name, first.request.selections, &mut observer)
        }
        TerminalProductUse::Declarations => ProductCodeSource {
            publication: first.request.view.publication,
        }
        .publish(first.request.selections, &mut observer),
    }
}

// Checking one selected target outside a batch: produce its accepted program
// and evaluate the target-local conditions against it.
pub(crate) fn check_terminal_conditions(
    selection: &mut SelectedProviderCustody,
) -> Result<ProducedTerminalProduct, Vec<Diagnostic>> {
    let selections = selection.terminal_publication.optimizations().clone();
    let produced = produce_retained_terminal_artifacts(vec![TerminalProductRequest::declarations(
        selection,
        &selections,
    )])
    .pop()
    .ok_or_else(|| {
        vec![Diagnostic::error(
            "declaration checking lost its production",
        )]
    })??;
    produced.check_conditions(selection)?;
    Ok(produced)
}
