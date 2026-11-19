//! Run the admitted Build entry and retain its original activation through source settlement.

use build_settings::host::bindings::{
    BuildProductDescription, BuildProductQuery, BuildProviderRequest,
};
use build_settings::host::requests::BuildDependencyPreparation;
use build_settings::{
    BuildDependencySelection, BuildSourceResult, CompileFailure, EvaluatedBuild,
    PreparedBuildEvaluation, ProducedBuildProvider, ReturnedBuild, TerminalBuildActivation,
};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;

/// The coordinator resolves each request against the services granted to this stage.
/// The activation checks the answer against its own pending call before execution resumes.
pub trait PlanningServices {
    fn describe_product(
        &mut self,
        _activation: &TerminalBuildActivation,
        _request: &BuildProductQuery,
    ) -> Result<(BuildProductDescription, Vec<BuildDependencySelection>), CompileFailure> {
        Err(vec![Diagnostic::error(
            "Build requested a Product description without a granted query service",
        )]
        .into())
    }

    fn prepare_dependency(
        &mut self,
        _activation: &TerminalBuildActivation,
        _request: &BuildDependencyPreparation,
    ) -> Result<BuildDependencySelection, CompileFailure> {
        Err(vec![Diagnostic::error(
            "Build requested dependency preparation without a granted dependency service",
        )]
        .into())
    }

    fn provide(
        &mut self,
        _activation: &TerminalBuildActivation,
        _request: &BuildProviderRequest,
    ) -> Result<(BuildDependencySelection, ProducedBuildProvider), CompileFailure> {
        Err(vec![Diagnostic::error(
            "Build requested a provider without a granted provider service",
        )]
        .into())
    }
}

impl PlanningServices for () {}

/// A normal return from the same invocation that admitted and checked this Build entry.
pub struct ReturnedPlanningStage {
    evaluation: PreparedBuildEvaluation,
    returned: ReturnedBuild,
}

impl ReturnedPlanningStage {
    pub fn returned(&self) -> &ReturnedBuild {
        &self.returned
    }

    /// Join the exact acquired sources to the dependency requests that this Build issued.
    pub fn source_result(&mut self) -> Result<BuildSourceResult<'_>, CompileFailure> {
        self.evaluation.source_result(Some(&mut self.returned))
    }

    pub fn complete(
        self,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<EvaluatedBuild, CompileFailure> {
        self.evaluation.complete_with_sources(self.returned, inputs)
    }
}

pub fn interpret(
    mut evaluation: PreparedBuildEvaluation,
    services: &mut impl PlanningServices,
) -> Result<ReturnedPlanningStage, CompileFailure> {
    let result = (|| {
        let execution = evaluation.execution().ok_or_else(|| {
            vec![Diagnostic::error(
                "Build planning requires an admitted execution",
            )]
        })?;
        let host = execution.host()?;
        let mut activation = host.start().map_err(refusal)?;
        let ran = run(&mut activation, services);
        let spent = activation.helper_evaluation_accounting().clone();
        ran.and_then(|()| activation.complete().map_err(refusal))
            .map_err(|failure| failure.with_source_completion_accounting(spent))
    })();
    let returned = result.map_err(|failure| {
        failure.with_source_completion_accounting(evaluation.source_completion_accounting().clone())
    })?;
    evaluation.validate_return(&returned)?;
    Ok(ReturnedPlanningStage {
        evaluation,
        returned,
    })
}

fn run(
    activation: &mut TerminalBuildActivation,
    services: &mut impl PlanningServices,
) -> Result<(), CompileFailure> {
    loop {
        let step = activation.advance(1024).map_err(refusal)?;
        if let Some(query) = step.awaiting_query {
            let (description, selections) = services.describe_product(activation, &query)?;
            activation
                .answer_query(&query, description, &selections)
                .map_err(refusal)?;
        } else if let Some(request) = step.awaiting_provider {
            let (selection, provider) = services.provide(activation, &request)?;
            activation
                .install_provider(selection, &provider)
                .map_err(refusal)?;
        } else if let Some(request) = step.awaiting_preparation {
            let selected = services.prepare_dependency(activation, &request)?;
            activation.dependency_prepared(selected).map_err(refusal)?;
        } else if step.returned {
            return Ok(());
        }
    }
}

fn refusal(reason: String) -> CompileFailure {
    vec![Diagnostic::error(format!(
        "Build activation failed: {reason}"
    ))]
    .into()
}
