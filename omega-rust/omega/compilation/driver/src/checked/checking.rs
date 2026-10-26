//! Prepare source, run the typed and Build stages, then seal checked execution.

pub(crate) mod build_continuation;
pub(crate) mod compile_thread;
mod source_checking;
pub(crate) use source_checking::check_product_source;

pub use compilation_products::{
    CheckedCompilation, CheckedCompileOutcome, CheckedCompileRequest,
    IndependentComponentDiscovery, IndependentComponentSelection, PreparedCheckedSource,
    PreparedPackageBuild, SettledCheckReuse,
};
pub use package_compilation::build_execution::RestrictedBuildGrants;

use crate::execution_settlement::SelectedCompileOutcome;
use artifacts::compile_timings::{CompileTimings, StageMeta, counters};
use compilation_products::{
    CheckedExecutionRequest, CompileFailure, PreparedBuild, PreparedProductSource,
    PreparedSourceFrontier, TypedCompilationFrontier,
};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use package_compilation::source_assembly::ImmutableSourceParseCheckpoint;
use std::path::Path;

/// Psi 00-01 through package-owned source preparation, then retain the result.
pub(crate) fn prepare_checked_source(
    root_path: &Path,
    package_sources: Option<std::sync::Arc<package_compilation::PackageCompilationSourceInputs>>,
    source_capture: Option<&package_compilation::CapturedLocalSource>,
    collect_timings: bool,
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
    )?;
    Ok(PreparedCheckedSource::from_parsed(sources, timings))
}

/// Consume retained frontend work or run assembly and Psi 02-03 explicitly.
/// The product owner revalidates custody on reuse and retains only completed work.
fn prepare_typed_frontier(
    source: PreparedCheckedSource,
    package_inputs: Option<&PackageCompilationInputs>,
    build_execution_profile: Option<target::TargetProfile>,
) -> Result<(TypedCompilationFrontier, CompileTimings), Vec<Diagnostic>> {
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
    let (source_file_count, syntax) = sources.assemble(package_inputs, &mut timings)?;
    timings.add_completed(
        artifacts::compile_timings::SOURCE_ASSEMBLY,
        assembly_started.elapsed().as_micros(),
    );
    let base_sources = syntax.sources.clone();
    let frontend_started = std::time::Instant::now();
    let frontier = build_continuation::lower_typed_frontier(
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
    // Preserve the report's inclusive Build ladder after separating preparation.
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
    root: std::path::PathBuf,
    target: target::TargetProfile,
    inputs: Option<PackageCompilationInputs>,
    rollback: optimization_core::OptimizationRollback,
    permit_unsettled_fused_service_fields: bool,
    source: build_continuation::BuildSourceInvocation,
}

impl PreparedPackageInvocation {
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

    pub(super) fn execution(&mut self) -> Option<&mut build_evaluation::PreparedBuildExecution> {
        self.source.execution()
    }

    pub(super) fn settle(
        self,
        returned: Option<build_evaluation::ReturnedBuild>,
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
            permit_unsettled_fused_service_fields: self.permit_unsettled_fused_service_fields,
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
        settled.permit_unsettled_fused_service_fields,
    )
    .map_err(|mut diagnostics| {
        attribute_diagnostics(&mut diagnostics, &base_sources, Some(&inputs));
        CompileFailure::from(diagnostics)
    })
}

pub(super) fn begin_package_build_worker(
    request: CheckedCompileRequest,
    source: PreparedCheckedSource,
    reuse: SettledCheckReuse,
    owner: &build_evaluation::PureBuildCompilation,
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
        permit_unsettled_fused_service_fields: false,
        settled_check_reuse: reuse,
    };
    let source = prepare_build_source_invocation(&request.root_path, source, child, Some(owner))?;
    Ok(PreparedPackageInvocation {
        root: request.root_path,
        target: selected_target,
        inputs,
        rollback: request.optimization_rollback,
        source,
        permit_unsettled_fused_service_fields: request.permit_unsettled_fused_service_fields,
    })
}

/// The checked request's feed-forward route. Settlement and source-sealing
/// failures retain completed Build observations beside the rejected outcome.
pub(super) fn select_checked_worker(
    request: CheckedCompileRequest,
    prepared: PreparedCheckedSource,
    reuse: SettledCheckReuse,
    pure_build_compilation: &build_evaluation::PureBuildCompilation,
    selections: &mut psi_compiler::ProductSelections,
) -> Result<SelectedCompileOutcome, CompileFailure> {
    // Target admission precedes source loading, including for fresh requests.
    let selected_target_profile = request
        .target_name
        .as_deref()
        .map(|target_name| target::TargetProfile::from_omega_target_name(Some(target_name)))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    let package_inputs = request.package_inputs.as_ref();
    let child = CheckedExecutionRequest {
        selected_target_profile,
        package_inputs,
        build: request.build.resolve_compilation_defaults(package_inputs),
        optimization_rollback: request.optimization_rollback.clone(),
        permit_unsettled_fused_service_fields: request.permit_unsettled_fused_service_fields,
        settled_check_reuse: reuse,
    };
    select_prepared_source(
        &request.root_path,
        prepared,
        child,
        Some(pure_build_compilation),
        selections,
    )
}

/// One source-to-checked route for product compilation and package review.
/// A product batch shares its completed source prefix. Sponsored review
/// activations admit fresh and may consume an exactly matched completed Pure Build. Failures
/// after Build retain discovery observations, never output or execution authority.
pub(crate) fn select_prepared_source(
    root_path: &Path,
    source: PreparedCheckedSource,
    child: CheckedExecutionRequest<'_>,
    pure_build_compilation: Option<&build_evaluation::PureBuildCompilation>,
    selections: &mut psi_compiler::ProductSelections,
) -> Result<SelectedCompileOutcome, CompileFailure> {
    let selected_target_profile = child.selected_target_profile;
    let package_inputs = child.package_inputs;
    let rollback = child.optimization_rollback.clone();
    let permit_unsettled_fused_service_fields = child.permit_unsettled_fused_service_fields;
    let settled_check_reuse = child.settled_check_reuse.clone();

    let prepared = prepare_build_source(root_path, source, child, pure_build_compilation)?;
    Ok(select_completed_build(
        prepared,
        selected_target_profile,
        package_inputs,
        &rollback,
        permit_unsettled_fused_service_fields,
        settled_check_reuse,
        selections,
    ))
}

/// Shared admission and Build lifecycle. Both product compilation and a
/// dependency handoff consume the same completed typed continuation.
fn prepare_build_source(
    root_path: &Path,
    source: PreparedCheckedSource,
    child: CheckedExecutionRequest<'_>,
    pure_build_compilation: Option<&build_evaluation::PureBuildCompilation>,
) -> Result<PreparedBuild, CompileFailure> {
    let mut invocation =
        prepare_build_source_invocation(root_path, source, child, pure_build_compilation)?;
    let returned = if let Some(execution) = invocation.execution() {
        let mut host = execution.host()?;
        let refuse = |reason| {
            vec![Diagnostic::error(format!(
                "Terminal Build failed: {reason}"
            ))]
        };
        let mut activation = host.start().map_err(refuse)?;
        loop {
            if activation.advance(1024).map_err(refuse)?.returned {
                break;
            }
        }
        Some(activation.complete().map_err(refuse)?)
    } else {
        None
    };
    invocation.complete(returned, None, None)
}

fn prepare_build_source_invocation(
    root_path: &Path,
    mut source: PreparedCheckedSource,
    child: CheckedExecutionRequest<'_>,
    pure_build_compilation: Option<&build_evaluation::PureBuildCompilation>,
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

/// Complete the invocation's source prefix before iterating realizations.
pub(crate) fn prepare_product_source(
    root_path: &Path,
    source: PreparedCheckedSource,
    child: CheckedExecutionRequest<'_>,
    pure_build_compilation: Option<&build_evaluation::PureBuildCompilation>,
) -> Result<PreparedProductSource, CompileFailure> {
    let package_inputs = child.package_inputs;
    let prepared = prepare_build_source(root_path, source, child, pure_build_compilation)?;
    complete_prepared_build(prepared, package_inputs).map_err(CompileFailure::from)
}

pub(super) fn complete_prepared_build(
    prepared: PreparedBuild,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<PreparedProductSource, Vec<Diagnostic>> {
    let PreparedBuild {
        mut built,
        mut build_sources,
        mut timings,
    } = prepared;
    let retained_build_check = built.retained_build_check.take();
    let base_sources = built.base_sources.clone();
    let (typed, pending_pre_checks, source_completion_accounting, computed_build_config, admission) =
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
        pending_pre_checks,
        computed_build_config,
        admission,
        build_sources,
        timings,
        retained_build_check,
    })
}

pub(super) fn select_completed_build(
    prepared: PreparedBuild,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    rollback: &crate::OptimizationRollback,
    permit_unsettled_fused_service_fields: bool,
    settled_check_reuse: SettledCheckReuse,
    selections: &mut psi_compiler::ProductSelections,
) -> SelectedCompileOutcome {
    let independent_components = prepared
        .built
        .computed_build_config
        .independent_components();
    match complete_prepared_build(prepared, package_inputs).and_then(|source| {
        check_product_source(
            source,
            package_inputs,
            rollback,
            settled_check_reuse,
            selections,
        )
    }) {
        Ok(source) => crate::execution_settlement::select_product_target(
            &source,
            selected_target_profile,
            package_inputs,
            permit_unsettled_fused_service_fields,
        ),
        Err(diagnostics) => SelectedCompileOutcome {
            result: Err(CompileFailure::from(diagnostics)),
            independent_components,
        },
    }
}

/// Interpret each diagnostic's authored span against the compilation's
/// retained source map while that map is still alive: staged path, line,
/// column, and the owning package's display name. Error paths outside the
/// map's reach render this record instead of rereading source storage.
/// `attribute_diagnostics` through a compile failure. Consent requests
/// carry no diagnostics to attribute.
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

/// Record an inclusive stage whose subordinate operations also record timings.
pub(crate) fn timed<T>(
    timings: &mut CompileTimings,
    stage: StageMeta,
    step: impl FnOnce(&mut CompileTimings) -> T,
) -> T {
    let started = std::time::Instant::now();
    let result = step(timings);
    timings.add_completed(stage, started.elapsed().as_micros());
    result
}
