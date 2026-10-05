//! Prepare source, run the typed and Build stages, then seal checked execution.

pub(crate) mod build_continuation;
pub(crate) mod compile_thread;
pub(crate) mod execution_settlement;

pub use compilation_products::{
    CheckedCompilation, CheckedCompileOutcome, CheckedCompileRequest,
    IndependentComponentDiscovery, IndependentComponentSelection, PackageBuildOutcome,
    PreparedCheckedSource, PreparedPackageBuild,
};
pub use package_compilation::build_execution::RestrictedBuildGrants;

use artifacts::compile_timings::{
    BUILD_AND_CHECKED_CONTINUATION, CompileTimings, EXECUTION_SETTLEMENT, StageMeta, counters,
};
use compilation_products::{
    CheckedExecutionRequest, PreparedBuild, PreparedBuilds, PreparedSourceFrontier,
    TypedCompilationFrontier,
};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use package_compilation::source_assembly::ImmutableSourceParseCheckpoint;
use std::path::Path;

/// Run the owned request on the compiler stack and return its whole outcome.
/// Early failures retain no source frontier or Build observations.
pub fn compile_checked(request: CheckedCompileRequest) -> CheckedCompileOutcome {
    compile_checked_with_build_owner(request, None)
}

/// Compile another consumer of this explicit compilation's completed Pure
/// Builds. Fresh requests without this owner cannot import another invocation's
/// results. Both entries run the same custody/admission/checking pipeline.
pub fn compile_checked_reusing_builds(
    request: CheckedCompileRequest,
    owner: &build_evaluation::PureBuildCompilation,
) -> CheckedCompileOutcome {
    compile_checked_with_build_owner(request, Some(owner.clone()))
}

fn compile_checked_with_build_owner(
    request: CheckedCompileRequest,
    owner: Option<build_evaluation::PureBuildCompilation>,
) -> CheckedCompileOutcome {
    counters::record_checked_compilation();
    compile_thread::run_on_compile_thread(move || compile_checked_worker(request, owner))
        .unwrap_or_else(|diagnostics| CheckedCompileOutcome {
            result: Err(diagnostics),
            prepared_source: None,
            independent_components: IndependentComponentDiscovery::default(),
        })
}

/// Psi 00-01 through package-owned source preparation, then retain the result.
pub(crate) fn prepare_checked_source(
    root_path: &Path,
    package_sources: Option<std::sync::Arc<package_compilation::PackageCompilationSourceInputs>>,
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
        &mut timings,
    )?;
    Ok(PreparedCheckedSource::from_parsed(sources, timings))
}

/// Consume retained frontend work or run assembly and Psi 02-03 explicitly.
/// The product owner revalidates custody on reuse and retains only completed work.
fn prepare_typed_frontier(
    source: PreparedCheckedSource,
    child: &CheckedExecutionRequest<'_>,
) -> Result<(TypedCompilationFrontier, CompileTimings), Vec<Diagnostic>> {
    let (sources, mut timings, retention) =
        match source.into_frontier(child.package_inputs, child.build_execution_profile)? {
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
    let (source_file_count, syntax) = sources.assemble(child.package_inputs, &mut timings)?;
    timings.add_completed(
        artifacts::compile_timings::SOURCE_ASSEMBLY,
        assembly_started.elapsed().as_micros(),
        artifacts::allocations::AllocationDelta::default(),
    );
    let frontend_started = std::time::Instant::now();
    let frontier = build_continuation::lower_typed_frontier(
        source_file_count,
        syntax,
        child.build_execution_profile,
        child.package_inputs,
        &mut timings,
    )?;
    // Preserve the report's inclusive Build ladder after separating preparation.
    timings.add_completed(
        artifacts::compile_timings::BUILD_AND_CHECKED_CONTINUATION,
        frontend_started.elapsed().as_micros(),
        artifacts::allocations::AllocationDelta::default(),
    );
    if let Some(retention) = retention {
        retention.publish(&frontier);
    }
    Ok((frontier, timings))
}

/// Compile through checked Psi without backend lowering or native output.
/// Use `compile_checked` to retain preparation and rejection observations.
pub fn compile_to_checked(
    request: CheckedCompileRequest,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    compile_checked(request).result
}

/// Execute one dependency Build under the same admission and source custody as
/// ordinary compilation, retaining its generated handoff and typed continuation.
/// No final product check or review is performed. A required dependency product
/// consumes this result through `check_prepared_package_build`, never a replay.
pub fn prepare_package_build(
    request: CheckedCompileRequest,
    owner: Option<&build_evaluation::PureBuildCompilation>,
) -> PackageBuildOutcome {
    let owner = owner.cloned();
    compile_thread::run_on_compile_thread(move || prepare_package_build_worker(request, owner))
        .unwrap_or_else(|diagnostics| PackageBuildOutcome {
            result: Err(diagnostics),
            prepared_source: None,
        })
}

fn prepare_package_build_worker(
    request: CheckedCompileRequest,
    owner: Option<build_evaluation::PureBuildCompilation>,
) -> Result<PackageBuildOutcome, Vec<Diagnostic>> {
    let selected_target =
        target::TargetProfile::from_omega_target_name(request.target_name.as_deref())
            .map_err(|diagnostic| vec![diagnostic])?;
    let inputs = request.package_inputs.ok_or_else(|| {
        vec![Diagnostic::error(
            "dependency Build preparation requires package source custody",
        )]
    })?;
    let source = match request.prepared_source {
        Some(source) => source,
        None => prepare_checked_source(
            &request.root_path,
            Some(inputs.source_inputs()),
            request.collect_timings,
        )?,
    };
    let child = CheckedExecutionRequest {
        selected_target_profile: Some(selected_target),
        build_execution_profile: request
            .build_execution_profile
            .or_else(target::TargetProfile::host_if_supported),
        package_inputs: Some(&inputs),
        build_dir: request.build_dir.as_deref(),
        filesystem_sponsor: request.filesystem_sponsor,
        evaluation_sponsor: request.evaluation_sponsor,
        // Explicit custody always wins; otherwise the root package's own
        // canonical Source metadata selects its sealed inventory, the same
        // way the product route derives it.
        build_snapshot: request.build_snapshot.or_else(|| {
            package_compilation::BuildSnapshotRequest::automatic_package_inventory(Some(&inputs))
        }),
        optimization_rollback: request.optimization_rollback.clone(),
        restricted_build_grants: request.restricted_build_grants,
        permit_unsettled_fused_service_fields: false,
    };
    let prepared_source = request.retain_prepared_source.then(|| source.clone());
    let prepared = prepare_build_source(&request.root_path, source, child, None, owner.as_ref())?;
    let result = PreparedPackageBuild::seal(
        request.root_path,
        selected_target,
        inputs,
        request.optimization_rollback,
        prepared,
    );
    let prepared_source = result.as_ref().ok().and(prepared_source);
    Ok(PackageBuildOutcome {
        result,
        prepared_source,
    })
}

/// Complete a genuinely requested dependency product using its completed Build.
/// Source custody is revalidated when checking seals the product; no Build grant
/// or sponsor is recreated and no authored Build effect executes a second time.
pub fn check_prepared_package_build(build: PreparedPackageBuild) -> CheckedCompileOutcome {
    counters::record_checked_compilation();
    compile_thread::run_on_compile_thread(move || {
        let (_root_path, target, inputs, rollback, mut prepared) = build.into_continuation();
        Ok(check_completed_build(
            prepared,
            Some(target),
            Some(&inputs),
            &rollback,
            false,
        ))
    })
    .unwrap_or_else(|diagnostics| CheckedCompileOutcome {
        result: Err(diagnostics),
        prepared_source: None,
        independent_components: IndependentComponentDiscovery::default(),
    })
}

/// The checked request's feed-forward route. Settlement and source-sealing
/// failures retain completed Build observations beside the rejected outcome.
fn compile_checked_worker(
    request: CheckedCompileRequest,
    pure_build_compilation: Option<build_evaluation::PureBuildCompilation>,
) -> Result<CheckedCompileOutcome, Vec<Diagnostic>> {
    // Target admission precedes source loading, including for fresh requests.
    let selected_target_profile = request
        .target_name
        .as_deref()
        .map(|target_name| target::TargetProfile::from_omega_target_name(Some(target_name)))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    let prepared = match request.prepared_source {
        Some(prepared) => prepared,
        None => prepare_checked_source(
            &request.root_path,
            request
                .package_inputs
                .as_ref()
                .map(PackageCompilationInputs::source_inputs),
            request.collect_timings,
        )?,
    };
    let package_inputs = request.package_inputs.as_ref();
    let child = CheckedExecutionRequest {
        selected_target_profile,
        build_execution_profile: request
            .build_execution_profile
            .or_else(target::TargetProfile::host_if_supported),
        package_inputs,
        build_dir: request.build_dir.as_deref(),
        filesystem_sponsor: request.filesystem_sponsor,
        evaluation_sponsor: request.evaluation_sponsor,
        // Explicit custody always wins; otherwise the root package's own
        // canonical Source metadata selects its sealed inventory, the same
        // way the product route derives it.
        build_snapshot: request.build_snapshot.or_else(|| {
            package_compilation::BuildSnapshotRequest::automatic_package_inventory(package_inputs)
        }),
        optimization_rollback: request.optimization_rollback.clone(),
        restricted_build_grants: request.restricted_build_grants,
        permit_unsettled_fused_service_fields: request.permit_unsettled_fused_service_fields,
    };
    let retained_source = request.retain_prepared_source.then(|| prepared.clone());
    let mut outcome = check_prepared_source(
        &request.root_path,
        prepared,
        child,
        None,
        pure_build_compilation.as_ref(),
    )?;
    if outcome.result.is_ok() {
        outcome.prepared_source = retained_source;
    }
    Ok(outcome)
}

/// One source-to-checked route for product compilation and package review.
/// A product batch may retain completed Builds by exact inputs. Sponsored review
/// activations admit fresh and may consume an exactly matched completed Pure Build. Failures
/// after Build retain its discovery observations, but never a reusable source.
pub(crate) fn check_prepared_source(
    root_path: &Path,
    source: PreparedCheckedSource,
    child: CheckedExecutionRequest<'_>,
    builds: Option<&mut PreparedBuilds>,
    pure_build_compilation: Option<&build_evaluation::PureBuildCompilation>,
) -> Result<CheckedCompileOutcome, Vec<Diagnostic>> {
    let selected_target_profile = child.selected_target_profile;
    let package_inputs = child.package_inputs;
    let rollback = child.optimization_rollback.clone();
    let permit_unsettled_fused_service_fields = child.permit_unsettled_fused_service_fields;

    let prepared = prepare_build_source(root_path, source, child, builds, pure_build_compilation)?;
    Ok(check_completed_build(
        prepared,
        selected_target_profile,
        package_inputs,
        &rollback,
        permit_unsettled_fused_service_fields,
    ))
}

/// Shared admission and Build lifecycle. Both product compilation and a
/// dependency handoff consume the same completed typed continuation.
fn prepare_build_source(
    root_path: &Path,
    source: PreparedCheckedSource,
    child: CheckedExecutionRequest<'_>,
    builds: Option<&mut PreparedBuilds>,
    pure_build_compilation: Option<&build_evaluation::PureBuildCompilation>,
) -> Result<PreparedBuild, Vec<Diagnostic>> {
    source.admit_root(root_path)?;
    child.admit_package_inputs()?;

    // Lookup follows custody admission and never executes a stage. Single
    // review activations do not construct an unsponsored Build reuse key.
    let mut reuse = builds.map(|builds| (child.build_execution_inputs(), builds));
    let completed = reuse
        .as_mut()
        .and_then(|(inputs, builds)| builds.begin_target(inputs));
    match completed {
        Some(mut prepared) => {
            drop(source);
            drop(child);
            Ok(prepared)
        }
        None => {
            let (frontier, mut timings) = prepare_typed_frontier(source, &child)?;
            let (built, build_sources) =
                timed(&mut timings, BUILD_AND_CHECKED_CONTINUATION, |timings| {
                    build_continuation::evaluate_build_and_continue(
                        root_path,
                        child,
                        frontier,
                        timings,
                        pure_build_compilation,
                    )
                })?;
            let prepared = PreparedBuild {
                built,
                build_sources,
                timings,
            };
            if let Some((inputs, builds)) = reuse {
                builds.retain_completed(inputs, &prepared);
            }
            Ok(prepared)
        }
    }
}

fn check_completed_build(
    prepared: PreparedBuild,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    rollback: &crate::OptimizationRollback,
    permit_unsettled_fused_service_fields: bool,
) -> CheckedCompileOutcome {
    let PreparedBuild {
        built,
        mut build_sources,
        mut timings,
    } = prepared;
    let independent_components = built.computed_build_config.independent_components();
    let result = timed(&mut timings, EXECUTION_SETTLEMENT, |timings| {
        execution_settlement::check_selected_execution(
            built,
            &mut build_sources,
            selected_target_profile,
            package_inputs,
            rollback,
            timings,
            permit_unsettled_fused_service_fields,
        )
    });
    let result = result.and_then(|execution| {
        CheckedCompilation::seal(execution, build_sources, package_inputs, timings)
    });
    CheckedCompileOutcome {
        prepared_source: None,
        result,
        independent_components,
    }
}

/// Record an inclusive stage whose subordinate operations also record timings.
fn timed<T>(
    timings: &mut CompileTimings,
    stage: StageMeta,
    step: impl FnOnce(&mut CompileTimings) -> T,
) -> T {
    let started = std::time::Instant::now();
    let result = step(timings);
    timings.add_completed(
        stage,
        started.elapsed().as_micros(),
        artifacts::allocations::AllocationDelta::default(),
    );
    result
}
