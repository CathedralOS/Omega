//! Prepare source, run the typed and Build stages, then seal checked execution.

pub(crate) mod build_continuation;
pub(crate) mod compile_thread;
pub(crate) mod execution_settlement;

pub use compilation_products::{
    CheckedCompilation, CheckedCompileOutcome, CheckedCompileRequest,
    IndependentComponentDiscovery, IndependentComponentSelection, PreparedCheckedSource,
};
pub use package_compilation::build_execution::RestrictedBuildGrants;

use artifacts::compile_timings::{CompileTimings, counters};
use compilation_products::{
    CheckedExecutionRequest, PreparedSourceFrontier, TypedCompilationFrontier,
};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use package_compilation::source_assembly::ImmutableSourceParseCheckpoint;
use std::path::Path;

/// Run the owned request on the compiler stack and return its whole outcome.
/// Early failures retain no source frontier or Build observations.
pub fn compile_checked(request: CheckedCompileRequest) -> CheckedCompileOutcome {
    counters::record_checked_compilation();
    compile_thread::run_on_compile_thread(move || compile_checked_worker(request)).unwrap_or_else(
        |diagnostics| CheckedCompileOutcome {
            result: Err(diagnostics),
            prepared_source: None,
            independent_components: IndependentComponentDiscovery::default(),
        },
    )
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
pub(crate) fn prepare_typed_frontier(
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
        child
            .selected_target_profile
            .map(target::TargetProfile::target_name),
        child
            .build_execution_profile
            .map(target::TargetProfile::target_name),
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

/// The checked request's feed-forward route. Settlement and source-sealing
/// failures retain completed Build observations beside the rejected outcome.
fn compile_checked_worker(
    request: CheckedCompileRequest,
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
    prepared.admit_root(&request.root_path)?;
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
        build_snapshot: request.build_snapshot,
        optimization_rollback: request.optimization_rollback.clone(),
        restricted_build_grants: request.restricted_build_grants,
        permit_unsettled_fused_service_fields: request.permit_unsettled_fused_service_fields,
    };
    let retained_source = request.retain_prepared_source.then(|| prepared.clone());
    child.admit_package_inputs()?;
    let (frontier, mut timings) = prepare_typed_frontier(prepared, &child)?;
    let build_started = std::time::Instant::now();
    let (built, sources) = build_continuation::evaluate_build_and_continue(
        &request.root_path,
        child,
        frontier,
        &mut timings,
    )?;
    timings.add_completed(
        artifacts::compile_timings::BUILD_AND_CHECKED_CONTINUATION,
        build_started.elapsed().as_micros(),
        artifacts::allocations::AllocationDelta::default(),
    );
    let independent_components = built.computed_build_config.independent_components();
    let settlement_started = std::time::Instant::now();
    let result = execution_settlement::check_selected_execution(
        built,
        selected_target_profile,
        package_inputs,
        &request.optimization_rollback,
        &mut timings,
        request.permit_unsettled_fused_service_fields,
    );
    timings.add_completed(
        artifacts::compile_timings::EXECUTION_SETTLEMENT,
        settlement_started.elapsed().as_micros(),
        artifacts::allocations::AllocationDelta::default(),
    );
    let result = result.and_then(|execution| {
        CheckedCompilation::seal(execution, sources, package_inputs, timings)
    });
    Ok(CheckedCompileOutcome {
        prepared_source: if result.is_ok() {
            retained_source
        } else {
            None
        },
        result,
        independent_components,
    })
}
