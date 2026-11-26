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
pub use product::ProductStage;
pub use registered_tests::{RegisteredTestPlan, TestExecution};
#[path = "planning/package_review/mod.rs"]
mod package_review;
pub use package_review::CheckedPackagePolicy;
#[path = "planning/source.rs"]
mod source;
pub use source::{
    OwedBuildRecheck, PendingProduct, PlannedProduct, PlanningRequest, PlanningSources,
    PreparedPlanningStage, prepare, rechecks_outlive_caller, with_build_rechecks,
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
        self.require_own_graph(&inputs)?;
        self.source
            .clone()
            .open_product(self.evaluated.clone(), Some(&inputs))
    }

    /// Open the Product while this Build is checked again on the Product's
    /// sources on an idle worker. Its handoff can be formed at once; the
    /// Product itself is released only after that check passes.
    pub fn open_pending(
        &self,
        inputs: PackageCompilationInputs,
    ) -> Result<PendingProduct, CompileFailure> {
        self.require_own_graph(&inputs)?;
        self.source
            .clone()
            .open_pending(self.evaluated.clone(), Some(&inputs))
    }

    fn require_own_graph(&self, inputs: &PackageCompilationInputs) -> Result<(), CompileFailure> {
        if inputs.source_inputs() != self.inputs.source_inputs() {
            return Err(refusal(
                "Product handoffs belong to another acquired source graph".into(),
            ));
        }
        Ok(())
    }
}

/// A normal return from the same invocation that admitted and checked this Build entry.
pub struct ReturnedPlanningStage {
    evaluation: PreparedBuildEvaluation,
    source: source::PlanningSource,
    returned: ReturnedBuild,
    preparations: BuildPreparations,
}

/// The stages of one package that returned another Build entry. Their chosen files
/// and observations stay with the eventual Product, separate from its own settings.
#[derive(Default)]
struct BuildPreparations {
    stages: Vec<(build_declarations::BuildEntryPath, EvaluatedBuild)>,
    files: Vec<std::path::PathBuf>,
    entries: Vec<(
        std::path::PathBuf,
        Vec<package_compilation::source_imports::SuppliedBuildLibrary>,
        source_compiler::InterpretedProviderSelections,
    )>,
}

/// Only the Build that returned this entry can issue its source view and incoming bindings.
pub struct PreparedSuccessor {
    providers: source_compiler::InterpretedProviderSelections,
    capture: package_compilation::CapturedLocalSource,
    path: std::path::PathBuf,
    libraries: Vec<package_compilation::source_imports::SuppliedBuildLibrary>,
    preparations: BuildPreparations,
}

impl PreparedSuccessor {
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl ReturnedPlanningStage {
    pub fn finish_successor(
        self,
        libraries: Vec<package_compilation::source_imports::SuppliedBuildLibrary>,
    ) -> Result<PreparedSuccessor, CompileFailure> {
        let (entry, capture) = self.returned.successor_source().map_err(refusal)?;
        let entry = entry.clone();
        let capture = capture.clone();
        let path = capture.canonical_live_root().join(entry.as_str());
        let mut preparations = self.preparations;
        let evaluated = self
            .evaluation
            .complete_successor(self.returned, &libraries)?;
        let mut source = self.source;
        let providers = source.successor_provider_selections(&entry, &libraries, &evaluated)?;
        if preparations.entries.iter().any(|(file, inputs, choices)| {
            file == &path && inputs == &libraries && choices == &providers
        }) {
            return Err(refusal(format!(
                "Build successor repeats the same inputs for {}",
                path.display()
            )));
        }
        source.retain_build_files(capture.canonical_live_root(), &mut preparations.files);
        preparations.stages.push((entry, evaluated));
        preparations
            .entries
            .push((path.clone(), libraries.clone(), providers.clone()));
        Ok(PreparedSuccessor {
            providers,
            capture,
            path,
            preparations,
            libraries,
        })
    }

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
        self,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<PlannedProduct, CompileFailure> {
        let (source, evaluated) = self.complete_activation(inputs)?;
        source.open_product(evaluated, inputs)
    }

    /// [`Self::open_product`], except that the returned Build is checked again
    /// on the Product's sources on an idle worker; the Product is released by
    /// [`PendingProduct::settle`] or [`PendingProduct::release_for_check`].
    pub fn open_pending(
        self,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<PendingProduct, CompileFailure> {
        let (source, evaluated) = self.complete_activation(inputs)?;
        source.open_pending(evaluated, inputs)
    }

    fn complete_activation(
        mut self,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<(source::PlanningSource, EvaluatedBuild), CompileFailure> {
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
        Ok((
            self.source,
            retain_preparations(self.preparations, evaluated),
        ))
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
            evaluated: retain_preparations(self.preparations, evaluated),
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
        let evaluated = self
            .evaluation
            .complete_with_sources(self.returned, inputs)?;
        Ok(retain_preparations(self.preparations, evaluated))
    }
}

fn retain_preparations(
    preparations: BuildPreparations,
    mut product: EvaluatedBuild,
) -> EvaluatedBuild {
    for (entry, preparation) in preparations.stages {
        product
            .preparations
            .push((entry, preparation.configuration));
        for request in preparation.restricted_build_requests {
            if !product.restricted_build_requests.contains(&request) {
                product.restricted_build_requests.push(request);
            }
        }
    }
    product
}

pub fn interpret(
    stage: PreparedPlanningStage,
    services: &mut impl PlanningServices,
) -> Result<ReturnedPlanningStage, CompileFailure> {
    let PreparedPlanningStage {
        preparations,
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
        preparations,
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
        match activation
            .advance_diagnostic(1024)
            .map_err(|mut diagnostic| {
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
