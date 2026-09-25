//! Compile shared source into independently checked target products.
//!
//! `compile` prepares the source set once, then runs each requested target
//! through the stage chain in `compile_target`, isolating one target's
//! failure from the others. Every step there is one call; the folders beside
//! this file own what each call does.

use crate::checked::{
    AssembledSource, CheckedChildExecution, CheckedCompilation, PreparedCheckedSource,
    check_selected_execution, evaluate_build_and_continue, run_on_compile_thread,
};
use crate::compiler::request::{SharedCompileInputs, ValidatedTargetCompilation};
use crate::native::{NativeInputReuse, prepare_native_product};
use crate::terminal::produce_terminal_report;
use crate::{
    CompileOutcomes, CompileReport, CompileRequest, CompileTargetOutcome, OptimizationRollback,
    RequestedCompileProduct, admit_checked_compilation,
};
use artifacts::compile_timings::{
    BUILD_AND_CHECKED_CONTINUATION, CompileTimings, EXECUTION_SETTLEMENT, StageMeta,
};
use diagnostics::Diagnostic;

pub(crate) mod options;
pub(crate) mod package;
pub(crate) mod request;

/// Compile every requested target, retaining failures alongside successful products.
/// Invalid requests reject before source acquisition. Shared preparation failures
/// are reported for every target. Publication remains a separate operation.
pub fn compile(request: CompileRequest) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    run_on_compile_thread(move || {
        let request = request.validate_for_execution()?;
        // Psi 00-01: read, tokenize and parse the root's source set once.
        let source = PreparedCheckedSource::prepare(
            &request.shared.root_path,
            request.shared.package_sources.clone(),
            request.shared.timings,
        );
        let mut native_inputs = NativeInputReuse::default();
        let target_count = request.targets.len();
        let mut outcomes = Vec::with_capacity(target_count);
        // Checkpoint clones share immutable parsing. repeat_n moves the final
        // copy, so the last (and the only) target consumes the original arenas.
        let sources = std::iter::repeat_n(source, target_count);
        for (target, source) in request.targets.into_iter().zip(sources) {
            let profile = target.profile;
            let outcome = source.and_then(|source| {
                compile_target(&request.shared, target, source, &mut native_inputs)
            });
            outcomes.push(CompileTargetOutcome::new(profile, outcome));
        }
        CompileOutcomes::new(outcomes)
    })
}

/// One target through the stage chain: assemble its sources, evaluate the
/// build, check, admit, then produce the requested product.
fn compile_target(
    shared: &SharedCompileInputs,
    target: ValidatedTargetCompilation,
    source: PreparedCheckedSource,
    native_inputs: &mut NativeInputReuse,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let root_path = target.options().root_path.clone();
    let build_dir = target.options().build_dir();
    let child = CheckedChildExecution::for_target(
        target.profile,
        target.package_inputs(),
        &build_dir,
        &target.configuration.optimization_rollback,
        target.configuration.build_snapshot.as_ref(),
    );
    let selected_target_profile = child.selected_target_profile();
    let package_inputs = child.package_inputs();
    let rollback = child.optimization_rollback().clone();
    let permit_unsettled_fused_service_fields = child.permit_unsettled_fused_service_fields();

    // Source assembly: the shared parse plus this target's imports and
    // dependency-generated sources.
    let AssembledSource {
        source_file_count,
        syntax,
        mut timings,
    } = source.assemble(&child)?;
    // Psi 02-03 and build.omg: resolve and type the program, evaluate the
    // build machine, then resolve and type the sources it generated.
    let (built, build_sources) = timed(&mut timings, BUILD_AND_CHECKED_CONTINUATION, |timings| {
        evaluate_build_and_continue(&root_path, child, source_file_count, syntax, timings)
    })?;
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
    let trust_settlement =
        admit_checked_compilation(&checked, target.accepted_trust_admissions())?.into_settlement();
    let build_observation = checked.build_observation_identity();
    // Build products are settled once, then carried alongside the requested
    // compiler product. A check-only stop publishes none.
    let build_outputs = if shared.requested_product != RequestedCompileProduct::Check {
        checked.completed_build_outputs()?
    } else {
        None
    };
    admit_requested_product(
        &checked,
        shared.requested_product,
        &target.configuration.optimization_rollback,
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
            &target.configuration.terminal_admission_profile,
            &target.configuration.optimization_rollback,
        )?,
        // Psi 05-07 for the program entry, then Omega 00-09, image emission
        // and the native report; equal Terminal inputs are prepared once.
        RequestedCompileProduct::NativeArtifact => {
            let terminal = prepare_native_product(target.into_native_product_request(), checked)?;
            native_inputs.realize(terminal)?
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

/// Refuse a requested product the checked compilation cannot satisfy, once
/// per target and before the route dispatches production. These are product
/// fences, not trust settlement, so they stay out of
/// `admit_checked_compilation`: an artifact-only build executes no product
/// optimization stages and publishes no proof companions, and a check or
/// Terminal stop cannot satisfy proof-product requests. Evaluation order
/// matches the fence order the product dispatch previously enforced.
fn admit_requested_product(
    checked: &CheckedCompilation,
    requested_product: RequestedCompileProduct,
    optimization_rollback: &OptimizationRollback,
) -> Result<(), Vec<Diagnostic>> {
    if checked.application_artifact_only() && requested_product != RequestedCompileProduct::Check {
        if !optimization_rollback.is_empty() {
            return Err(vec![Diagnostic::error(
                "artifact-only builds execute no product optimization stages to roll back",
            )]);
        }
        if checked.pcc_requests().any() {
            return Err(vec![Diagnostic::error(
                "artifact-only builds cannot publish Psi or native proof products",
            )]);
        }
    }
    match requested_product {
        RequestedCompileProduct::Check if checked.pcc_requests().any() => {
            Err(vec![Diagnostic::error(
                "a check-only stop cannot satisfy an optional proof-product request",
            )])
        }
        RequestedCompileProduct::TerminalArtifact if checked.pcc_requests().native => {
            Err(vec![Diagnostic::error(
                "a Terminal stop cannot satisfy a native proof-product request",
            )])
        }
        _ => Ok(()),
    }
}
