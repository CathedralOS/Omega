/*
We prepare source and Build continuations, join the application's returned Build
activation, and seal checked Product execution.

`prepare_checked_source` runs Psi 00-01 through package-owned source preparation
and retains the immutable parse checkpoint. `prepare_typed_frontier` either
consumes a retained typed frontier or runs source assembly and Psi 02-03
(`build_continuation::lower_typed_frontier`).
*/

/*
We also provide the explicit 256 MiB compiler worker stack (`compile_thread`) so
deep recursive parsing, representation walks, and nested build-time product
queries reach their explicit depth guards deterministically across hosts whose
default thread stacks are small (such as macOS or Windows). Nested calls on an
active compiler worker run inline without spawning additional threads.
*/

pub(crate) mod build_continuation;
mod entry_shapes;
mod product_queries;
mod product_selections;
pub(super) mod source_build;
mod source_checking;
pub(crate) mod typed_frontier;
pub(super) mod unbuilt_source;

pub use compilation_products::{
    CheckedCompilation, CheckedCompileOutcome, CheckedCompileRequest,
    IndependentComponentDiscovery, IndependentComponentSelection, PreparedCheckedSource,
    PreparedPackageBuild,
};
pub use package_compilation::build_execution::RestrictedBuildGrants;
pub use product_queries::ProductQueries;
pub(crate) use source_checking::check_product_source;

use crate::execution_settlement::SelectedCompileOutcome;
use artifacts::compile_timings::{CompileTimings, counters};
use compilation_products::{
    CheckedExecutionRequest, CompileFailure, PreparedBuild, PreparedProductSource,
    PreparedSourceFrontier, TypedCompilationFrontier,
};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use package_compilation::source_assembly::ImmutableSourceParseCheckpoint;
use std::path::Path;

pub(crate) mod compile_thread {
    use diagnostics::Diagnostic;

    pub const COMPILE_STACK_SIZE: usize = 256 * 1024 * 1024;

    thread_local! {
        static ON_COMPILER_WORKER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    pub fn run_on_compile_thread<T, Failure>(
        work: impl FnOnce() -> Result<T, Failure> + Send,
    ) -> Result<T, Failure>
    where
        T: Send,
        Failure: From<Vec<Diagnostic>> + Send,
    {
        if ON_COMPILER_WORKER.get() {
            return work();
        }
        std::thread::scope(|scope| {
            spawn_compile_worker(scope, work)
                .map_err(|error| {
                    Failure::from(vec![Diagnostic::error(format!(
                        "failed to spawn compiler thread: {error}"
                    ))])
                })?
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
    }

    pub fn spawn_compile_worker<'scope, 'env, T>(
        scope: &'scope std::thread::Scope<'scope, 'env>,
        work: impl FnOnce() -> T + Send + 'scope,
    ) -> std::io::Result<std::thread::ScopedJoinHandle<'scope, T>>
    where
        T: Send + 'scope,
    {
        std::thread::Builder::new()
            .name("omega-compile".to_owned())
            .stack_size(COMPILE_STACK_SIZE)
            .spawn_scoped(scope, move || {
                ON_COMPILER_WORKER.set(true);
                work()
            })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn scoped_worker_keeps_nested_compilation_on_its_large_stack() {
            std::thread::scope(|scope| {
                spawn_compile_worker(scope, || -> Result<(), Vec<Diagnostic>> {
                    let worker = std::thread::current().id();
                    let nested = run_on_compile_thread(|| {
                        Ok::<_, Vec<Diagnostic>>(std::thread::current().id())
                    })?;
                    assert_eq!(worker, nested);
                    Ok(())
                })
                .unwrap()
                .join()
                .unwrap()
                .unwrap();
            });
        }
    }
}

pub(crate) fn prepare_checked_source(
    root_path: &Path,
    package_sources: Option<std::sync::Arc<package_compilation::PackageCompilationSourceInputs>>,
    source_capture: Option<&package_compilation::CapturedLocalSource>,
    collect_timings: bool,
    preparation: compilation_products::SourcePreparation,
) -> Result<PreparedCheckedSource, Vec<Diagnostic>> {
    let mut timings = if collect_timings {
        CompileTimings::enabled()
    } else {
        CompileTimings::default()
    };
    let sources = ImmutableSourceParseCheckpoint::prepare_from_sources(
        root_path,
        package_sources,
        source_capture,
        &mut timings,
        &mut *preparation.catalogue()?,
    )?;
    Ok(PreparedCheckedSource::from_parsed(
        sources,
        timings,
        preparation,
    ))
}

fn prepare_typed_frontier(
    source: PreparedCheckedSource,
    package_inputs: Option<&PackageCompilationInputs>,
    build_execution_profile: Option<target::TargetProfile>,
) -> Result<(TypedCompilationFrontier, CompileTimings), Vec<Diagnostic>> {
    let preparation = source.preparation();
    let (sources, mut timings, retention) =
        match source.into_frontier(package_inputs, build_execution_profile)? {
            PreparedSourceFrontier::Retained(frontier, timings) => {
                counters::record_typed_frontier_reuse();
                return Ok((frontier, timings));
            }
            PreparedSourceFrontier::Pending {
                sources,
                timings,
                retention,
            } => (sources, timings, retention),
        };
    counters::record_source_assembly();
    let assembly_started = std::time::Instant::now();
    let (source_file_count, syntax) =
        sources.assemble(package_inputs, &mut timings, &mut *preparation.catalogue()?)?;
    timings.add_completed(
        artifacts::compile_timings::SOURCE_ASSEMBLY,
        assembly_started.elapsed().as_micros(),
    );
    let base_sources = syntax.sources.clone();
    let frontend_started = std::time::Instant::now();
    let frontier = build_continuation::lower_typed_frontier(
        preparation,
        source_file_count,
        syntax,
        build_execution_profile,
        package_inputs,
        &mut timings,
    )
    .map_err(|mut diagnostics| {
        attribute_diagnostics(&mut diagnostics, &base_sources, package_inputs);
        diagnostics
    })?;
    timings.add_completed(
        artifacts::compile_timings::BUILD_AND_CHECKED_CONTINUATION,
        frontend_started.elapsed().as_micros(),
    );
    if let Some(retention) = retention {
        retention.publish(&frontier);
    }
    Ok((frontier, timings))
}

pub(super) struct PreparedPackageInvocation {
    capture: Option<package_compilation::CapturedLocalSource>,
    root: std::path::PathBuf,
    target: target::TargetProfile,
    inputs: Option<PackageCompilationInputs>,
    rollback: optimization_core::OptimizationRollback,
    source: source_build::SourceBuild,
}

impl PreparedPackageInvocation {
    pub(super) fn product_queries(&self) -> ProductQueries {
        self.source.product_queries()
    }
    pub(super) fn settle_catalog(
        self,
        returned: Option<build_settings::ReturnedBuild>,
    ) -> Result<super::package_compilation::SourceBuildDeclaration, CompileFailure> {
        let capture = self.capture.ok_or_else(|| {
            vec![Diagnostic::error(
                "workspace settlement requires acquired unbound source custody",
            )]
        })?;
        let settled = self.source.settle(returned, None, Some(self.target))?;
        let declaration = settled
            .evaluated
            .configuration
            .project_declaration
            .clone()
            .ok_or_else(|| vec![Diagnostic::error("workspace Build returned no declaration")])?;
        if !matches!(
            declaration,
            build_declarations::BuildDeclaration::Workspace(_)
        ) {
            return Err(vec![Diagnostic::error(
                "source catalog requires a workspace declaration",
            )]
            .into());
        }
        Ok(super::package_compilation::SourceBuildDeclaration::settled(
            capture,
            declaration,
        ))
    }
    pub(super) fn observe_declaration(
        &mut self,
        returned: Option<&mut build_settings::ReturnedBuild>,
    ) -> Result<super::package_compilation::SourceBuildDeclaration, CompileFailure> {
        let result = self.source.source_result(returned)?;
        let capture = self.capture.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "source declaration observation requires acquired unbound source custody",
            )]
        })?;
        Ok(super::package_compilation::SourceBuildDeclaration::new(
            capture.clone(),
            result.declaration().cloned().ok_or_else(|| {
                vec![Diagnostic::error(
                    "default Build has no authored named project declaration",
                )]
            })?,
        ))
    }
    pub(super) fn source_result<'result>(
        &'result mut self,
        returned: Option<&'result mut build_settings::ReturnedBuild>,
    ) -> Result<build_settings::BuildSourceResult<'result>, CompileFailure> {
        self.source.source_result(returned)
    }
    pub(super) fn join_product_context(
        &mut self,
        target: target::TargetProfile,
        inputs: PackageCompilationInputs,
    ) -> Result<(), CompileFailure> {
        if let Some(original) = &self.inputs
            && original != &inputs
        {
            return Err(vec![Diagnostic::error(
                "Build invocation cannot replace supplied package inputs",
            )]
            .into());
        }
        if self.inputs.is_some() && self.target != target {
            return Err(vec![Diagnostic::error(
                "Build invocation cannot replace its admitted Product target",
            )]
            .into());
        }
        self.target = target;
        self.inputs = Some(inputs);
        Ok(())
    }

    pub(super) fn execution(&mut self) -> Option<&mut build_settings::PreparedBuildExecution> {
        self.source.execution()
    }

    pub(super) fn settle(
        self,
        returned: Option<build_settings::ReturnedBuild>,
    ) -> Result<compilation_products::SettledPackageBuild, CompileFailure> {
        let inputs = self.inputs.ok_or_else(|| {
            vec![Diagnostic::error(
                "Build completion requires joined package source custody",
            )]
        })?;
        let source = self
            .source
            .settle(returned, Some(&inputs), Some(self.target))?;
        Ok(compilation_products::SettledPackageBuild {
            root: self.root,
            target: self.target,
            inputs,
            rollback: self.rollback,
            source,
        })
    }
}

pub(super) fn prepare_settled_package_product(
    settled: compilation_products::SettledPackageBuild,
    target: target::TargetProfile,
    inputs: PackageCompilationInputs,
) -> Result<PreparedPackageBuild, CompileFailure> {
    inputs.validate_execution_context(Some(target), settled.source.build_execution_profile)?;
    let prepared = build_continuation::prepare_settled_product(settled.source, Some(&inputs))?;
    let base_sources = prepared.built.base_sources.clone();
    PreparedPackageBuild::seal(
        settled.root,
        target,
        inputs.clone(),
        settled.rollback,
        prepared,
    )
    .map_err(|mut diagnostics| {
        attribute_diagnostics(&mut diagnostics, &base_sources, Some(&inputs));
        CompileFailure::from(diagnostics)
    })
}

pub(super) fn begin_package_build_worker(
    request: CheckedCompileRequest,
    source: PreparedCheckedSource,
    owner: &build_settings::PureBuildCompilation,
) -> Result<PreparedPackageInvocation, CompileFailure> {
    let selected_target =
        target::TargetProfile::from_omega_target_name(request.target_name.as_deref())
            .map_err(|diagnostic| vec![diagnostic])?;
    let inputs = request.package_inputs;
    let child = CheckedExecutionRequest {
        selected_target_profile: Some(selected_target),
        package_inputs: inputs.as_ref(),
        build: request.build.resolve_compilation_defaults(inputs.as_ref()),
        optimization_rollback: request.optimization_rollback.clone(),
    };
    let source = source_build::SourceBuild::prepare(&request.root_path, source, child, owner)?;
    Ok(PreparedPackageInvocation {
        capture: request.source_capture,
        root: request.root_path,
        target: selected_target,
        inputs,
        rollback: request.optimization_rollback,
        source,
    })
}

pub struct SourceCompilation {
    root: std::path::PathBuf,
    target: Option<target::TargetProfile>,
    inputs: Option<PackageCompilationInputs>,
    rollback: crate::OptimizationRollback,
    source: source_build::SourceBuild,
}

impl SourceCompilation {
    pub fn product_queries(&self) -> ProductQueries {
        self.source.product_queries()
    }
    pub fn execution(&mut self) -> Option<&mut build_settings::PreparedBuildExecution> {
        self.source.execution()
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn require_product_policy(
        &self,
        product: &crate::CompilationProduct,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.rollback != product.optimization_rollback() {
            return Err(vec![Diagnostic::error(
                "source product cannot replace its admitted optimization rollback",
            )]);
        }
        Ok(())
    }

    pub(super) fn select(
        self,
        returned: Result<Option<build_settings::ReturnedBuild>, CompileFailure>,
        frontier: Option<&mut Option<typed_frontier::TypedFrontier>>,
    ) -> Result<SelectedCompileOutcome, CompileFailure> {
        let prepared = self.source.complete(returned?, None, None)?;
        Ok(select_completed_build(
            prepared,
            self.target,
            self.inputs.as_ref(),
            &self.rollback,
            frontier,
        ))
    }

    pub(super) fn check(
        self,
        returned: Result<Option<build_settings::ReturnedBuild>, CompileFailure>,
    ) -> Result<compilation_products::CheckedProductSource, Vec<Diagnostic>> {
        let prepared = self
            .source
            .complete(
                returned.map_err(CompileFailure::into_diagnostics)?,
                None,
                None,
            )
            .map_err(CompileFailure::into_diagnostics)?;
        let source = complete_prepared_build(prepared, self.inputs.as_ref())?;
        check_product_source(source, self.inputs.as_ref(), &self.rollback, None)
    }
}

pub(super) fn prepare_source_compilation(
    root: &Path,
    source: PreparedCheckedSource,
    child: CheckedExecutionRequest<'_>,
    pure_build_compilation: &build_settings::PureBuildCompilation,
) -> Result<SourceCompilation, CompileFailure> {
    let target = child.selected_target_profile;
    let inputs = child.package_inputs.cloned();
    let rollback = child.optimization_rollback.clone();
    let source = source_build::SourceBuild::prepare(root, source, child, pure_build_compilation)?;
    Ok(SourceCompilation {
        root: root.to_path_buf(),
        target,
        inputs,
        rollback,
        source,
    })
}

fn prepare_build_source_invocation(
    root_path: &Path,
    mut source: PreparedCheckedSource,
    child: CheckedExecutionRequest<'_>,
    pure_build_compilation: Option<&build_settings::PureBuildCompilation>,
) -> Result<build_continuation::BuildSourceInvocation, CompileFailure> {
    source.admit_root(root_path)?;
    child.admit_package_inputs()?;

    let parsed = source.fork();
    let (frontier, timings) =
        prepare_typed_frontier(source, child.package_inputs, child.build.profile)?;
    let base_sources = frontier.base_sources.clone();
    let package_inputs = child.package_inputs;
    build_continuation::prepare_build_invocation(
        root_path,
        child,
        parsed,
        frontier,
        timings,
        pure_build_compilation,
    )
    .map_err(|failure| attribute_failure(failure, &base_sources, package_inputs))
}

pub(super) fn complete_prepared_build(
    prepared: PreparedBuild,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<PreparedProductSource, Vec<Diagnostic>> {
    let PreparedBuild {
        built,
        mut build_sources,
        mut timings,
    } = prepared;
    let base_sources = built.base_sources.clone();
    let (typed, source_completion_accounting, computed_build_config, admission) =
        build_continuation::complete_product_frontend(
            built,
            &mut build_sources,
            package_inputs,
            &mut timings,
        )
        .map_err(|mut diagnostics| {
            attribute_diagnostics(&mut diagnostics, &base_sources, package_inputs);
            diagnostics
        })?;
    Ok(PreparedProductSource {
        source_completion_accounting,
        typed,
        computed_build_config,
        admission,
        build_sources,
        timings,
    })
}

pub(super) fn select_completed_build(
    prepared: PreparedBuild,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    rollback: &crate::OptimizationRollback,
    frontier: Option<&mut Option<typed_frontier::TypedFrontier>>,
) -> SelectedCompileOutcome {
    let independent_components = prepared
        .built
        .computed_build_config
        .independent_components();
    match complete_prepared_build(prepared, package_inputs)
        .and_then(|source| check_product_source(source, package_inputs, rollback, frontier))
    {
        Ok(source) => crate::execution_settlement::select_product_target(
            &source,
            crate::checked::PreparedProductTarget::capture(
                &source,
                selected_target_profile,
                package_inputs,
            ),
        ),
        Err(diagnostics) => SelectedCompileOutcome {
            result: Err(CompileFailure::from(diagnostics)),
            independent_components,
            tests: Vec::new(),
            test_notes: Vec::new(),
        },
    }
}

fn attribute_failure(
    failure: CompileFailure,
    sources: &source::SourceMap,
    package_inputs: Option<&PackageCompilationInputs>,
) -> CompileFailure {
    match failure {
        CompileFailure::Diagnostics(mut diagnostics) => {
            attribute_diagnostics(&mut diagnostics, sources, package_inputs);
            CompileFailure::Diagnostics(diagnostics)
        }
        other => other,
    }
}

pub(crate) fn attribute_diagnostics(
    diagnostics: &mut [Diagnostic],
    sources: &source::SourceMap,
    package_inputs: Option<&PackageCompilationInputs>,
) {
    for diagnostic in diagnostics {
        if diagnostic.resolved_location.is_some() {
            continue;
        }
        let Some(source_span) = diagnostic.source_span else {
            continue;
        };
        if source_span == source::SourceSpan::default() {
            continue;
        }
        let Some(file) = sources.file_at(source_span) else {
            continue;
        };
        let position = source::SourcePosition::of(&file.source, source_span.span.start);
        let package = file
            .package_identity
            .and_then(|identity| package_inputs.and_then(|inputs| inputs.package_name(identity)))
            .map(str::to_string);
        diagnostic.resolved_location = Some(diagnostics::ResolvedSourceLocation {
            package,
            path: file.path.clone(),
            line: position.line as u32,
            column: position.column as u32,
        });
    }
}
