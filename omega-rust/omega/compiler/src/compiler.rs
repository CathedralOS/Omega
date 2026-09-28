//! Compile shared source into independently checked target products.
//!
//! `compile` prepares the source set once, then runs each requested target
//! through the stage chain in `compile_target`, isolating one target's
//! failure from the others. Every step there is one call; the folders beside
//! this file own what each call does.

use crate::checked::{
    BuiltCheckedProgram, CheckedChildExecution, CheckedCompilation, PreparedCheckedSource,
    check_selected_execution, evaluate_build_and_continue, run_on_compile_thread,
};
use crate::native::produce_native_report;
use crate::terminal::produce_terminal_report;
use crate::{
    CompileOutcomes, CompileReport, CompileRequest, CompileTargetOutcome, RequestedCompileProduct,
};
use artifacts::compile_timings::{
    BUILD_AND_CHECKED_CONTINUATION, CompileTimings, EXECUTION_SETTLEMENT, StageMeta,
};
use compilation_products::compile_request::{SharedCompileInputs, ValidatedTargetCompilation};
use diagnostics::Diagnostic;
use package_compilation::BuildSourceCustody;

pub(crate) mod package;

/// Compile every requested target, retaining failures alongside successful products.
/// Invalid requests reject before source acquisition. Shared preparation failures
/// are reported for every target. Publication remains a separate operation.
pub fn compile(request: CompileRequest) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    run_on_compile_thread(move || {
        let (shared, targets) = request.validate_for_execution()?.into_parts();
        // Psi 00-01: read, tokenize and parse the root's source set once.
        let source = PreparedCheckedSource::prepare(
            &shared.root_path,
            shared.package_sources.clone(),
            shared.timings,
        );
        let target_count = targets.len();
        let mut builds = SharedBuilds::for_targets(target_count);
        let mut outcomes = Vec::with_capacity(target_count);
        // Checkpoint clones share immutable parsing. repeat_n moves the final
        // copy, so the last (and the only) target consumes the original arenas.
        let sources = std::iter::repeat_n(source, target_count);
        for (target, source) in targets.into_iter().zip(sources) {
            let profile = target.profile();
            let outcome =
                source.and_then(|source| compile_target(&shared, target, source, &mut builds));
            outcomes.push(CompileTargetOutcome::new(profile, outcome));
        }
        CompileOutcomes::new(outcomes)
    })
}

/// One target through the stage chain: assemble its sources and evaluate the
/// build (once for every target with equal build inputs), check, admit, then
/// produce the requested product.
fn compile_target(
    shared: &SharedCompileInputs,
    target: ValidatedTargetCompilation,
    source: PreparedCheckedSource,
    builds: &mut SharedBuilds,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let root_path = target.options().root_path.clone();
    let build_dir = target.options().build_dir();
    let child = CheckedChildExecution::for_target(
        target.profile(),
        target.package_inputs(),
        &build_dir,
        target.optimization_rollback(),
        target.build_snapshot(),
    );
    let selected_target_profile = child.selected_target_profile();
    let package_inputs = child.package_inputs();
    let rollback = child.optimization_rollback().clone();
    let permit_unsettled_fused_service_fields = child.permit_unsettled_fused_service_fields();

    // Source assembly, Psi 02-03 and build.omg: the shared parse plus the
    // imports and dependency-generated sources this target's package inputs
    // carry, resolved and typed, the build machine evaluated, and the sources
    // it generated resolved and typed. None of it observes the target, so
    // targets with equal build inputs share one result.
    source.admit_child(&child)?;
    let SharedBuild {
        built,
        build_sources,
        mut timings,
    } = builds.build(source, child, &root_path)?;
    // Provider selection, const folding, Psi 04 checking and selected dispatch.
    let execution = timed(&mut timings, EXECUTION_SETTLEMENT, |timings| {
        check_selected_execution(
            built,
            selected_target_profile,
            package_inputs,
            &rollback,
            timings,
            permit_unsettled_fused_service_fields,
        )
    })?;
    let checked = CheckedCompilation::seal(execution, build_sources, package_inputs, timings)?;

    // Trust settlement and the requested product's fences.
    let stage_timings = checked.timings().phases().to_vec();
    let trust_settlement = checked.admit_trust(target.accepted_trust_admissions())?;
    let build_observation = checked.build_observation_identity();
    // Build products are settled once, then carried alongside the requested
    // compiler product. A check-only stop publishes none.
    let build_outputs = if shared.requested_product != RequestedCompileProduct::Check {
        checked.completed_build_outputs()?
    } else {
        None
    };
    shared.requested_product.admit_build_requirements(
        checked.application_artifact_only(),
        checked.pcc_requests(),
        target.optimization_rollback(),
    )?;
    if checked.application_artifact_only()
        && shared.requested_product != RequestedCompileProduct::Check
    {
        let outputs = build_outputs
            .ok_or_else(|| vec![Diagnostic::error("artifact-only build completed no files")])?;
        return CompileReport::from_build_outputs(
            root_path,
            checked.source_file_count(),
            outputs,
            checked.production_subject()?,
        )
        .map(|report| {
            report
                .with_trust_admission_settlement(trust_settlement)
                .with_timings(stage_timings)
        })
        .map_err(|message| vec![Diagnostic::error(message)]);
    }

    let report = match shared.requested_product {
        RequestedCompileProduct::Check => {
            CompileReport::check_only(root_path, checked.source_file_count())
                .map_err(|message| vec![Diagnostic::error(message)])?
        }
        // Psi 05-07: lower, optimize and publish the Terminal artifact.
        RequestedCompileProduct::TerminalArtifact => produce_terminal_report(
            root_path,
            checked,
            target.terminal_admission_profile(),
            target.optimization_rollback(),
        )?,
        // The same retained Terminal boundary, then Omega 00-09 and image emission.
        RequestedCompileProduct::NativeArtifact => {
            produce_native_report(target.into_native_product_request(), checked)?
        }
    };
    let report = if shared.requested_product != RequestedCompileProduct::Check {
        report
            .with_build_outputs(build_outputs, build_observation)
            .map_err(|message| vec![Diagnostic::error(message)])?
    } else {
        report
    };
    Ok(report
        .with_trust_admission_settlement(trust_settlement)
        .with_timings(stage_timings))
}

/// The assembled, built and typed program of one invocation, keyed by the
/// package owner's complete build inputs. Captured builds with equal inputs
/// share one result and each target continues from its own copy; live output
/// directories remain distinct. Per-child custody admission precedes reuse.
struct SharedBuilds {
    builds: Vec<(package_compilation::BuildExecutionInputs, SharedBuild)>,
    /// Targets still to be served after the current one; the last target
    /// keeps nothing for reuse.
    remaining_targets: usize,
}

/// One shared build: the typed program with its evaluated Build, the source
/// custody spanning it, and the timing ladder its steps recorded.
#[derive(Clone)]
struct SharedBuild {
    built: BuiltCheckedProgram,
    build_sources: BuildSourceCustody,
    timings: CompileTimings,
}

impl SharedBuilds {
    fn for_targets(target_count: usize) -> Self {
        Self {
            builds: Vec::new(),
            remaining_targets: target_count,
        }
    }

    fn build(
        &mut self,
        source: PreparedCheckedSource,
        child: CheckedChildExecution<'_>,
        root_path: &std::path::Path,
    ) -> Result<SharedBuild, Vec<Diagnostic>> {
        let inputs = child.build_execution_inputs();
        self.remaining_targets = self.remaining_targets.saturating_sub(1);
        if let Some((_, shared)) = self.builds.iter().find(|(existing, _)| *existing == inputs) {
            return Ok(shared.clone());
        }
        let (frontier, mut timings) = source.prepare_frontier(&child)?;
        let (built, build_sources) =
            timed(&mut timings, BUILD_AND_CHECKED_CONTINUATION, |timings| {
                evaluate_build_and_continue(root_path, child, frontier, timings)
            })?;
        let shared = SharedBuild {
            built,
            build_sources,
            timings,
        };
        if self.remaining_targets > 0 {
            self.builds.push((inputs, shared.clone()));
        }
        Ok(shared)
    }
}

/// Run one step and record its duration under `stage` in the target's ladder.
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
