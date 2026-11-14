/*

Dependency-first package frontier scheduler and one-shot `build.omg` executor.

When compiling one or more target `PackagePlan`s (`prepare_packages`), each plan
contains a dependency-ordered list of `PackageSourceRequest` occurrences ending
at the root `Product` occurrence:
1. `validate_order` checks up front that the plan is non-empty, contains no
   duplicate `(package, purpose)` pairs, places every dependency before its
   consumers, and ends at a `Product` root.
2. In each frontier wave, we advance every active plan by one occurrence:
   joining generated-source handoffs from already-completed dependencies
   (`join_generated_sources`), preparing the occurrence (`prepare_package_source`
   — either executing a fresh `build.omg` activation or preparing a `Product`
   from an already-settled `CompletedBuild`), and recording any independent
   component packages required by that occurrence (`required_components`).
3. `complete_components` batch-completes all newly demanded independent
   component descriptions across all active plans in one cohort call
   (`compiler.complete_package_components`) before attaching those descriptions
   to the waiting occurrences.

For single-activation `build.omg` execution (`execute_build` and
`execute_returned_build`), we step the Terminal Psi guest activation in 1024-step
quanta, answering any `awaiting_query` product-description requests from the
retained `ProductQueries` context until the activation returns. Provider
readiness pauses (`awaiting_provider`) require multi-package dependency
scheduling and are handled by `source_resolution::readiness::ReadinessScheduler`
instead; hitting one in one-shot `execute_build` fails closed.

*/

pub(crate) mod source_resolution;

use build_declarations::DependencyPurpose;
use compiler::{CheckedCompileRequest, CompileFailure, PackageBuild};
use compiler::{CompilationSession, PackageCompileFailure, PackageSourceRequest};
use diagnostics::Diagnostic;
use package_compilation::{IndependentComponentDescription, PackageBuildHandoff};
use semantic_vocabulary::PackageKeyIdentity;

struct OccurrenceProduct {
    package: PackageKeyIdentity,
    purpose: DependencyPurpose,
    generated: PackageBuildHandoff,
    prepared: Option<PackageBuild>,
    description: Option<IndependentComponentDescription>,
}

struct PackageFrontier<Owner> {
    position: usize,
    owner: Owner,
    remaining: std::vec::IntoIter<PackageSourceRequest>,
    products: Vec<OccurrenceProduct>,
}

pub(crate) enum PreparedPackageRoot {
    Build(Box<PackageBuild>),
}

pub(crate) struct PackagePlan {
    pub occurrences: Vec<PackageSourceRequest>,
}

fn prepare_package_source(
    request: PackageSourceRequest,
    compiler: &mut CompilationSession,
    completed: &mut [compiler::CompletedBuild],
) -> Result<PackageBuild, CompileFailure> {
    match request {
        PackageSourceRequest::Execute(request) => execute_package_build(compiler, request),
        PackageSourceRequest::Product(request) => {
            let source = completed
                .iter_mut()
                .find(|source| source.package_inputs().root() == request.inputs.root())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "Product request has no settled source Build",
                    )]
                })?;
            source.require_execution_contract(&request.execution)?;
            compiler.prepare_package_product(
                source,
                request.target,
                request.inputs,
                request.options,
            )
        }
    }
}

struct PendingOccurrence<Owner> {
    frontier: PackageFrontier<Owner>,
    build: PackageBuild,
    components: Vec<usize>,
    failure: Option<CompileFailure>,
}

pub(crate) fn execute_package_build(
    compiler: &mut CompilationSession,
    request: CheckedCompileRequest,
) -> Result<PackageBuild, CompileFailure> {
    execute_returned_build(compiler, request)?.complete(compiler)
}

pub(crate) struct ReturnedPackageBuild {
    pub(crate) invocation: compiler::PackageBuildInvocation,
    pub(crate) returned: Option<build_settings::ReturnedBuild>,
}

impl ReturnedPackageBuild {
    pub(crate) fn complete(
        self,
        compiler: &mut CompilationSession,
    ) -> Result<PackageBuild, CompileFailure> {
        let Self {
            invocation,
            returned,
        } = self;
        compiler.complete_package_build(invocation, returned)
    }
}

pub(crate) fn execute_returned_build(
    compiler: &mut CompilationSession,
    request: CheckedCompileRequest,
) -> Result<ReturnedPackageBuild, CompileFailure> {
    let mut invocation = compiler.begin_package_build(request)?;
    let spent = invocation.source_completion_accounting().clone();
    let mut queries = invocation.product_queries();
    let returned = execute_build(invocation.execution(), Some(&mut queries))
        .map_err(|failure| failure.with_source_completion_accounting(spent))?;
    Ok(ReturnedPackageBuild {
        invocation,
        returned,
    })
}

pub fn execute_build(
    execution: Option<&mut build_settings::PreparedBuildExecution>,
    mut queries: Option<&mut compiler::ProductQueries>,
) -> Result<Option<build_settings::ReturnedBuild>, CompileFailure> {
    let returned = if let Some(execution) = execution {
        let host = execution.host()?;
        let refuse = |reason| {
            vec![Diagnostic::error(format!(
                "Terminal Build failed: {reason}"
            ))]
        };
        let mut activation = host.start().map_err(refuse)?;
        loop {
            let step = activation.advance(1024).map_err(refuse)?;
            if let Some(query) = step.awaiting_query {
                let queries = queries
                    .as_mut()
                    .ok_or_else(|| refuse("Build query has no retained source context".into()))?;
                let description = queries.describe(&query, Vec::new())?;
                activation
                    .answer_query(&query, description, &[])
                    .map_err(refuse)?;
                continue;
            }
            if step.awaiting_provider.is_some() {
                return Err(refuse(
                    "Build provider readiness requires dependency scheduling".into(),
                )
                .into());
            }
            if step.returned {
                break;
            }
        }
        Some(activation.complete().map_err(refuse)?)
    } else {
        None
    };
    Ok(returned)
}

pub(crate) fn prepare_packages<Owner>(
    compiler: &mut CompilationSession,
    plans: Vec<(Owner, PackagePlan)>,
    completed: &mut [compiler::CompletedBuild],
) -> Vec<(Owner, Result<PreparedPackageRoot, PackageCompileFailure>)> {
    let mut outcomes = Vec::with_capacity(plans.len());
    let mut frontiers = Vec::new();
    for (position, (owner, plan)) in plans.into_iter().enumerate() {
        let PackagePlan { occurrences } = plan;
        if let Err(error) = validate_order(&occurrences) {
            outcomes.push((position, owner, Err(error)));
            continue;
        }
        let frontier = PackageFrontier {
            position,
            owner,
            products: Vec::with_capacity(occurrences.len()),
            remaining: occurrences.into_iter(),
        };
        frontiers.push(frontier);
    }

    while !frontiers.is_empty() {
        let mut pending = Vec::with_capacity(frontiers.len());
        for mut frontier in frontiers {
            let prepared = (|| {
                let occurrence = frontier.remaining.next().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package frontier lost its next occurrence",
                    )]
                })?;
                let occurrence = join_generated_sources(occurrence, &frontier.products)?;
                let build = prepare_package_source(occurrence, compiler, completed)?;
                let components = required_components(&build, &frontier.products)?;
                Ok::<_, CompileFailure>((build, components))
            })();
            match prepared {
                Ok((build, components)) => pending.push(PendingOccurrence {
                    frontier,
                    build,
                    components,
                    failure: None,
                }),
                Err(failure) => {
                    outcomes.push((
                        frontier.position,
                        frontier.owner,
                        Err(PackageCompileFailure {
                            occurrence: frontier.products.len(),
                            failure,
                        }),
                    ));
                }
            }
        }

        complete_components(compiler, &mut pending);

        frontiers = Vec::with_capacity(pending.len());
        for pending in pending {
            let PendingOccurrence {
                mut frontier,
                build,
                components,
                failure,
            } = pending;
            let completed = if let Some(failure) = failure {
                Err(failure)
            } else {
                let descriptions = components
                    .into_iter()
                    .map(|position| {
                        frontier.products[position]
                            .description
                            .clone()
                            .ok_or_else(|| {
                                vec![Diagnostic::error(
                                    "component completion lost its description",
                                )]
                            })
                    })
                    .collect::<Result<Vec<_>, _>>();
                descriptions
                    .map_err(CompileFailure::from)
                    .and_then(|descriptions| {
                        build
                            .with_independent_component_descriptions(descriptions)
                            .map_err(|errors| CompileFailure::from(input_diagnostics(errors)))
                    })
            };
            match completed {
                Err(failure) => {
                    outcomes.push((
                        frontier.position,
                        frontier.owner,
                        Err(PackageCompileFailure {
                            occurrence: frontier.products.len(),
                            failure,
                        }),
                    ));
                }
                Ok(build) if frontier.remaining.len() == 0 => {
                    outcomes.push((
                        frontier.position,
                        frontier.owner,
                        Ok(PreparedPackageRoot::Build(Box::new(build))),
                    ));
                }
                Ok(build) => {
                    frontier.products.push(OccurrenceProduct {
                        package: build.package_inputs().root(),
                        purpose: build.package_inputs().compilation_purpose(),
                        generated: build.build_handoff().clone(),
                        prepared: Some(build),
                        description: None,
                    });
                    frontiers.push(frontier);
                }
            }
        }
    }

    outcomes.sort_unstable_by_key(|(position, _, _)| *position);
    outcomes
        .into_iter()
        .map(|(_, owner, outcome)| (owner, outcome))
        .collect()
}

fn complete_components<Owner>(
    compiler: &mut CompilationSession,
    pending: &mut [PendingOccurrence<Owner>],
) {
    let mut builds = Vec::new();
    for (request_position, occurrence) in pending.iter_mut().enumerate() {
        for &component_position in &occurrence.components {
            let product = &mut occurrence.frontier.products[component_position];
            if product.description.is_some() {
                continue;
            }
            let Some(build) = product.prepared.take() else {
                occurrence.failure = Some(
                    vec![Diagnostic::error(
                        "independent component continuation was already consumed",
                    )]
                    .into(),
                );
                break;
            };
            builds.push(((request_position, component_position), build));
        }
    }
    let completed = match compiler.complete_package_components(builds) {
        Ok(completed) => completed,
        Err(diagnostics) => {
            for occurrence in pending {
                if occurrence.failure.is_none() {
                    occurrence.failure = Some(diagnostics.clone().into());
                }
            }
            return;
        }
    };
    for ((request_position, component_position), product) in completed {
        let occurrence = &mut pending[request_position];
        match product {
            Ok(product) => {
                occurrence.frontier.products[component_position].description = Some(product);
            }
            Err(failure) => {
                if occurrence.failure.is_none() {
                    occurrence.failure = Some(failure.into());
                }
            }
        }
    }
}

fn join_generated_sources(
    occurrence: PackageSourceRequest,
    products: &[OccurrenceProduct],
) -> Result<PackageSourceRequest, CompileFailure> {
    match occurrence {
        PackageSourceRequest::Execute(mut request) => {
            let inputs = request.package_inputs.take().ok_or_else(|| {
                vec![Diagnostic::error(
                    "package compilation requires source custody",
                )]
            })?;
            request.package_inputs = Some(join_generated_inputs(inputs, products)?);
            Ok(PackageSourceRequest::Execute(request))
        }
        PackageSourceRequest::Product(mut request) => {
            request.inputs = join_generated_inputs(request.inputs, products)?;
            Ok(PackageSourceRequest::Product(request))
        }
    }
}

fn join_generated_inputs(
    inputs: package_compilation::PackageCompilationInputs,
    products: &[OccurrenceProduct],
) -> Result<package_compilation::PackageCompilationInputs, CompileFailure> {
    let required = inputs.required_dependency_source_instances();
    let mut generated = Vec::with_capacity(required.len());
    for (dependency, dependency_purpose) in required {
        let product = products
            .iter()
            .find(|product| product.package == dependency && product.purpose == dependency_purpose)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "dependency Build output is missing for its required purpose",
                )]
            })?;
        generated.push(product.generated.clone());
    }
    inputs
        .with_complete_dependency_build_handoffs(generated)
        .map_err(|errors| input_diagnostics(errors).into())
}

fn required_components(
    prepared: &PackageBuild,
    products: &[OccurrenceProduct],
) -> Result<Vec<usize>, CompileFailure> {
    let inputs = prepared.package_inputs();
    let mut components = Vec::new();
    for component in prepared.required_component_packages() {
        if component == inputs.root() {
            continue;
        }
        let position = products
            .iter()
            .position(|product| {
                product.package == component && product.purpose == inputs.compilation_purpose()
            })
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "independent component has no prepared Build for this purpose",
                )]
            })?;
        if !components.contains(&position) {
            components.push(position);
        }
    }
    Ok(components)
}

fn validate_order(request: &[PackageSourceRequest]) -> Result<(), PackageCompileFailure> {
    if request.is_empty() {
        return Err(PackageCompileFailure {
            occurrence: 0,
            failure: vec![Diagnostic::error("package compilation has no Product root")].into(),
        });
    }
    for (position, occurrence) in request.iter().enumerate() {
        let refuse = |message| PackageCompileFailure {
            occurrence: position,
            failure: vec![Diagnostic::error(message)].into(),
        };
        let inputs = occurrence
            .package_inputs()
            .ok_or_else(|| refuse("package compilation requires source custody"))?;
        let previous = &request[..position];
        if previous
            .iter()
            .filter_map(PackageSourceRequest::package_inputs)
            .any(|prior| {
                prior.root() == inputs.root()
                    && prior.compilation_purpose() == inputs.compilation_purpose()
            })
        {
            return Err(refuse("package compilation repeats a package occurrence"));
        }
        for (package, purpose) in inputs.required_dependency_source_instances() {
            if !previous
                .iter()
                .filter_map(PackageSourceRequest::package_inputs)
                .any(|prior| prior.root() == package && prior.compilation_purpose() == purpose)
            {
                return Err(refuse(
                    "package compilation requires dependency-first occurrence order",
                ));
            }
        }
        if position + 1 == request.len() && !inputs.compilation_purpose().is_product() {
            return Err(refuse("package compilation must end at its Product root"));
        }
    }
    Ok(())
}

fn input_diagnostics(
    errors: Vec<package_compilation::PackageCompilationInputError>,
) -> Vec<Diagnostic> {
    errors
        .into_iter()
        .map(|error| Diagnostic::error(error.to_string()))
        .collect()
}
