//! Run the admitted Build entry and retain its original activation through source settlement.

#[path = "planning/native_entry.rs"]
mod native_entry;
pub use native_entry::NativeEntryLimits;
#[path = "planning/observations/mod.rs"]
pub mod observations;
#[path = "planning/product.rs"]
mod product;
#[path = "planning/registered_tests.rs"]
mod registered_tests;
pub use registered_tests::{RegisteredTestPlan, TestExecution};
pub use product::ProductStage;
#[path = "planning/package_review/mod.rs"]
mod package_review;
pub use package_review::CheckedPackagePolicy;
#[path = "planning/source.rs"]
mod source;
pub use source::{
    PlannedProduct, PlanningRequest, PlanningSources, PreparedPlanningStage, prepare,
};

use build_declarations::BuildRequest;
use build_execution::host::bindings::BuildProductQuery;
use build_execution::host::requests::BuildDependencyPreparation;
use build_execution::{
    BuildDependencySelection, BuildSourceResult, CompileFailure, EvaluatedBuild,
    PreparedBuildEvaluation, ReturnedBuild, SourceBuildDeclaration, TerminalBuildActivation,
    TerminalBuildStep,
};
use diagnostics::{BuildDiagnosticContext, Diagnostic};
use package_compilation::PackageCompilationInputs;
use package_compilation::source_imports::ProductQueryDependency;

/// The coordinator resolves each request against the services granted to this stage.
/// The activation checks the answer against its own pending call before execution resumes.
pub trait PlanningServices {
    fn query_dependencies(
        &mut self,
        _activation: &TerminalBuildActivation,
        _request: &BuildProductQuery,
    ) -> Result<Vec<ProductQueryDependency>, CompileFailure> {
        Err(vec![Diagnostic::error(
            "Build requested a Product description without a granted query service",
        )]
        .into())
    }

    fn prepare_dependency(
        &mut self,
        _activation: &TerminalBuildActivation,
        _request: &BuildDependencyPreparation,
    ) -> Result<ProductQueryDependency, CompileFailure> {
        Err(vec![Diagnostic::error(
            "Build requested dependency preparation without a granted dependency service",
        )]
        .into())
    }

    /// Return the named root acquired from this activation when a later stage needs it.
    fn declared_source(
        &mut self,
        _activation: &TerminalBuildActivation,
    ) -> Result<Option<PackageCompilationInputs>, CompileFailure> {
        Ok(None)
    }

    /// Return the original settled Product that supplied this Build provider declaration.
    fn provider_product(
        &mut self,
        _handoff: &package_compilation::PackageBuildHandoff,
    ) -> Result<PlannedProduct, CompileFailure> {
        Err(refusal(
            "Build requested a provider without a granted provider service".into(),
        ))
    }
}

impl PlanningServices for () {}

/// A named dependency's original Build has settled against this exact source graph.
pub struct SettledDependency {
    evaluated: EvaluatedBuild,
    source: source::PlanningSource,
    inputs: PackageCompilationInputs,
}

impl SettledDependency {
    pub fn inputs(&self) -> &PackageCompilationInputs {
        &self.inputs
    }

    pub fn evaluated(&self) -> &EvaluatedBuild {
        &self.evaluated
    }

    pub fn open_product(
        &self,
        inputs: PackageCompilationInputs,
    ) -> Result<PlannedProduct, CompileFailure> {
        self.open_product_alongside(inputs, |_| Ok(()))
            .map(|(product, ())| product)
    }

    /// Open the Product and run `alongside` on it while this Build is checked
    /// again on the Product's sources. The Product is returned only after that
    /// check passes.
    pub fn open_product_alongside<T: Send>(
        &self,
        inputs: PackageCompilationInputs,
        alongside: impl FnOnce(&PlannedProduct) -> Result<T, CompileFailure> + Send,
    ) -> Result<(PlannedProduct, T), CompileFailure> {
        if inputs.source_inputs() != self.inputs.source_inputs() {
            return Err(refusal(
                "Product handoffs belong to another acquired source graph".into(),
            ));
        }
        self.source
            .clone()
            .open_product_alongside(self.evaluated.clone(), Some(&inputs), alongside)
    }
}

/// A normal return from the same invocation that admitted and checked this Build entry.
pub struct ReturnedPlanningStage {
    evaluation: PreparedBuildEvaluation,
    source: source::PlanningSource,
    returned: ReturnedBuild,
}

impl ReturnedPlanningStage {
    pub fn source_completion_accounting(
        &self,
    ) -> &source_compiler::compile_time_invocations::SourceCompletionAccounting {
        self.evaluation.source_completion_accounting()
    }

    pub fn returned(&self) -> &ReturnedBuild {
        &self.returned
    }

    pub fn observe_source_declaration(&self) -> Result<SourceBuildDeclaration, CompileFailure> {
        self.evaluation.observe_source_declaration(&self.returned)
    }

    pub fn settle_workspace_declaration(self) -> Result<SourceBuildDeclaration, CompileFailure> {
        self.evaluation.settle_workspace_declaration(self.returned)
    }

    /// Settle this activation and open only the Product held by its original capture.
    pub fn open_product(
        mut self,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<PlannedProduct, CompileFailure> {
        let mut verify = || -> Result<(), String> {
            match (self.returned.declaration(), inputs) {
                (Some(build_declarations::BuildDeclaration::Workspace(_)), _) => {
                    Err("a workspace Build returns a member roster and has no Product".into())
                }
                (Some(_), None) => Err(
                    "a named Build requires its acquired source graph before Product compilation"
                        .into(),
                ),
                (_, Some(inputs)) => self.returned.join_preacquired_sources(inputs),
                (None, None) => Ok(()),
            }
        };
        verify().map_err(|reason| {
            CompileFailure::source_completion(
                vec![Diagnostic::error(reason)],
                self.evaluation.source_completion_accounting().clone(),
            )
        })?;
        let evaluated = self
            .evaluation
            .complete_with_sources(self.returned, inputs)?;
        self.source.open_product(evaluated, inputs)
    }

    /// Retain this dependency's deferred Product after the one Build invocation returns.
    pub fn settle_dependency(
        mut self,
        inputs: PackageCompilationInputs,
    ) -> Result<SettledDependency, CompileFailure> {
        self.returned
            .join_preacquired_sources(&inputs)
            .map_err(refusal)?;
        let evaluated = self
            .evaluation
            .complete_with_sources(self.returned, Some(&inputs))?;
        Ok(SettledDependency {
            evaluated,
            source: self.source,
            inputs,
        })
    }

    /// Join the exact acquired sources to the dependency requests that this Build issued.
    pub fn source_result(&mut self) -> Result<BuildSourceResult<'_>, CompileFailure> {
        self.evaluation.source_result(&mut self.returned)
    }

    pub fn complete(
        self,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<EvaluatedBuild, CompileFailure> {
        self.evaluation.complete_with_sources(self.returned, inputs)
    }
}

pub fn interpret(
    stage: PreparedPlanningStage,
    services: &mut impl PlanningServices,
) -> Result<ReturnedPlanningStage, CompileFailure> {
    let PreparedPlanningStage {
        mut evaluation,
        mut source,
    } = stage;
    let mut context = None;
    let result = (|| {
        let host = evaluation.execution().host()?;
        let mut activation =
            host.start(|owner, declaration| source.provide(owner, declaration, services))?;
        let ran = run(&mut activation, &mut source, services, &mut context);
        let spent = activation.helper_evaluation_accounting().clone();
        ran.map_err(|failure| failure.with_source_completion_accounting(spent.clone()))?;
        activation
            .complete()
            .map_err(refusal)
            .map_err(|failure| failure.with_source_completion_accounting(spent))
    })();
    let returned = result.map_err(|failure| {
        let failure = match context {
            Some(context) => failure.in_build_context(context),
            None => failure,
        };
        failure.with_source_completion_accounting(evaluation.source_completion_accounting().clone())
    })?;
    evaluation.record_return(&returned)?;
    Ok(ReturnedPlanningStage {
        evaluation,
        returned,
        source,
    })
}

fn run(
    activation: &mut TerminalBuildActivation,
    source: &mut source::PlanningSource,
    services: &mut impl PlanningServices,
    context: &mut Option<BuildDiagnosticContext>,
) -> Result<(), CompileFailure> {
    loop {
        match activation.advance_diagnostic(1024).map_err(|mut diagnostic| {
            diagnostic.message = format!("Build activation failed: {}", diagnostic.message);
            vec![*diagnostic]
        })? {
            TerminalBuildStep::Returned => return Ok(()),
            TerminalBuildStep::NeedsWork => continue,
            TerminalBuildStep::Query(query) => {
                let (description, selections) =
                    source.describe_product(activation, &query, services)?;
                activation
                    .answer_query(&query, description, &selections)
                    .map_err(refusal)?;
            }
            TerminalBuildStep::Provide => activation.install_provider().map_err(refusal)?,
            TerminalBuildStep::Prepare(request) => {
                *context = Some(BuildDiagnosticContext::PreparingDependency);
                let dependency = services.prepare_dependency(activation, &request)?;
                let registered_alias = activation
                    .requests()
                    .iter()
                    .filter_map(|issued| match issued.request() {
                        BuildRequest::Dependency { purpose, alias, .. }
                            if *purpose == request.purpose =>
                        {
                            Some(alias.as_ref())
                        }
                        _ => None,
                    })
                    .nth(request.request_ordinal)
                    .ok_or_else(|| {
                        refusal("dependency preparation has no issued registration".into())
                    })?;
                let root = dependency.inputs.root();
                let alias = registered_alias
                    .map(|name| name.as_str())
                    .or_else(|| dependency.inputs.package_name(root));
                if alias != Some(dependency.alias.as_str()) {
                    return Err(refusal(
                        "prepared dependency differs from its registered alias".into(),
                    ));
                }
                let selected = BuildDependencySelection {
                    purpose: request.purpose,
                    request_ordinal: request.request_ordinal,
                    package: root,
                };
                let inputs = dependency.inputs.identity_digest();
                let child_build_source = dependency
                    .child_build_source
                    .map(|commitment| commitment.digest());
                let handoff = dependency
                    .generated
                    .as_ref()
                    .map(package_compilation::PackageBuildHandoff::identity_digest);
                activation
                    .dependency_prepared(selected, inputs, child_build_source, handoff)
                    .map_err(refusal)?;
                *context = Some(BuildDiagnosticContext::ResumingBuild);
            }
        }
    }
}

fn refusal(reason: String) -> CompileFailure {
    vec![Diagnostic::error(format!(
        "Build activation failed: {reason}"
    ))]
    .into()
}
