//! Each target receives the already-published source program and its own provider proposal.
//! Targets that use the same source publication and Psi selection share that immutable input.
//! Scalar checking and executable products read the target's effective artifact.

use crate::execution_settlement::SelectedProviderCustody;
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
    program: &'a Arc<source_compiler::PortableTerminalProgram>,
    execution: &'a native_compiler::SelectedExecution,
    timings: &'a mut CompileTimings,
}

impl<'a> TerminalProductionView<'a> {
    pub(crate) fn selection(selection: &'a mut SelectedProviderCustody) -> Self {
        Self {
            program: &selection.source_program,
            execution: &selection.execution,
            timings: &mut selection.timings,
        }
    }
}

pub(crate) struct TerminalProductRequest<'a> {
    pub view: TerminalProductionView<'a>,
    pub selections: &'a OptimizationSelections,
}

impl<'a> TerminalProductRequest<'a> {
    pub(crate) fn new(
        selection: &'a mut SelectedProviderCustody,
        selections: &'a OptimizationSelections,
    ) -> Self {
        Self {
            view: TerminalProductionView::selection(selection),
            selections,
        }
    }
}

pub(crate) struct ProducedTerminalProduct {
    pub(crate) program: Arc<source_compiler::PortableTerminalProgram>,
    effective: Arc<terminal_codec::CanonicalTerminalArtifact>,
    pub(crate) proposal: Option<backend_plan::TerminalTargetProviderProposal>,
}

impl ProducedTerminalProduct {
    pub(crate) fn into_executable_proposal(
        self,
        published: &products::PublishedCompilation,
    ) -> Result<Option<backend_plan::TerminalTargetProviderProposal>, Vec<Diagnostic>> {
        if !Arc::ptr_eq(&self.effective, published.program().artifact()) {
            return Err(vec![Diagnostic::error(
                "entry planning requires this compilation's published program",
            )]);
        }
        Ok(self.proposal)
    }

    pub(crate) fn module(&self) -> &terminal_psi::TerminalModule {
        self.effective.semantic_module()
    }

    // Check the produced program's scalar conditions without inventing a
    // deployment for targetless source. A selected target supplies its proposal.
    pub(crate) fn check_conditions(
        &self,
        selection: &mut SelectedProviderCustody,
    ) -> Result<products::ProductScalarCompletion, Vec<Diagnostic>> {
        build_settings::scalar_completion::prepare(
            &self.effective,
            self.proposal
                .as_ref()
                .map(|proposal| proposal.target.as_str()),
            &[],
            self.proposal.as_slice(),
            &mut selection.source_completion_accounting,
        )
        .map_err(|reason| vec![Diagnostic::error(format!("constant checking: {reason}"))])
    }
}

struct PreparedTerminalProduct<'a> {
    position: usize,
    request: TerminalProductRequest<'a>,
}

impl PreparedTerminalProduct<'_> {
    fn shares_production(&self, other: &Self) -> bool {
        Arc::ptr_eq(self.request.view.program, other.request.view.program)
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
        let produced = shared_terminal(&cohort[0]);
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
            let outcome = produced.clone().and_then(|program| {
                let execution = product.request.view.execution;
                let effective = Arc::clone(execution.effective_artifact(program.artifact())
                    .map_err(|reason| vec![Diagnostic::error(reason)])?);
                let proposal = execution.provider_proposal(
                    effective.view(), product.request.selections.identity(),
                );
                Ok(ProducedTerminalProduct { program, effective, proposal })
            });
            outcomes.push((product.position, outcome));
        }
    }
    outcomes.sort_unstable_by_key(|(position, _)| *position);
    outcomes.into_iter().map(|(_, outcome)| outcome).collect()
}

fn shared_terminal(
    first: &PreparedTerminalProduct<'_>,
) -> Result<Arc<source_compiler::PortableTerminalProgram>, Vec<Diagnostic>> {
    if first.request.view.program.artifact().optimization().selections() != first.request.selections.project_psi().selections() {
        return Err(vec![Diagnostic::error(
            "Terminal publication differs from the Product's fixed Psi optimization selection",
        )]);
    }
    Ok(Arc::clone(first.request.view.program))
}

// Checking one selected target outside a batch: produce its accepted program
// and evaluate the target-local conditions against it.
pub(crate) fn check_terminal_conditions(
    selection: &mut SelectedProviderCustody,
) -> Result<ProducedTerminalProduct, Vec<Diagnostic>> {
    let selections = selection.optimizations.clone();
    let produced = produce_retained_terminal_artifacts(vec![TerminalProductRequest::new(
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
