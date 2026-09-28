//! Compile shared source into independently checked target products.
//!
//! `compile` prepares the source set once, then runs each requested target
//! through the stage chain in `compile_target`, isolating one target's
//! failure from the others. Every step there is one call; the folders beside
//! this file own what each call does.

use crate::checked::{
    CheckedCompilation, PreparedCheckedSource, check_selected_execution,
    evaluate_build_and_continue, prepare_checked_source, prepare_typed_frontier,
    run_on_compile_thread,
};
use crate::native::produce_native_report;
use crate::terminal::produce_terminal_report;
use crate::{CompileOutcomes, CompileReport, CompileRequest, CompileTargetOutcome};
use artifacts::compile_timings::{
    BUILD_AND_CHECKED_CONTINUATION, CompileTimings, EXECUTION_SETTLEMENT, StageMeta,
};
use compilation_products::compile_request::{SharedCompileInputs, ValidatedTargetCompilation};
use compilation_products::{CheckedProductReport, PreparedBuild, PreparedBuilds};
use diagnostics::Diagnostic;

pub(crate) mod package;

/// Compile every requested target, retaining failures alongside successful products.
/// Invalid requests reject before source acquisition. Shared preparation failures
/// are reported for every target. Publication remains a separate operation.
pub fn compile(request: CompileRequest) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    run_on_compile_thread(move || {
        let (shared, targets) = request.validate_for_execution()?.into_parts();
        // Psi 00-01: read, tokenize and parse the root's source set once.
        let source = prepare_checked_source(
            &shared.root_path,
            shared.package_sources.clone(),
            shared.timings,
        );
        let target_count = targets.len();
        let mut builds = PreparedBuilds::for_targets(target_count);
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
    builds: &mut PreparedBuilds,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let root_path = target.options().root_path.clone();
    let build_dir = target.options().build_dir();
    let child = target.checked_execution(&build_dir);
    let selected_target_profile = child.selected_target_profile;
    let package_inputs = child.package_inputs;
    let rollback = child.optimization_rollback.clone();
    let permit_unsettled_fused_service_fields = child.permit_unsettled_fused_service_fields;

    // Source assembly, Psi 02-03 and build.omg: the shared parse plus the
    // imports and dependency-generated sources this target's package inputs
    // carry, resolved and typed, the build machine evaluated, and the sources
    // it generated resolved and typed. None of it observes the target, so
    // targets with equal build inputs share one result.
    child.admit_package_inputs()?;
    let inputs = child.build_execution_inputs();
    let prepared = match builds.begin_target(&inputs) {
        Some(prepared) => {
            // A hit consumes this target's unused inputs before checking begins.
            drop(source);
            drop(child);
            prepared
        }
        None => {
            let (frontier, mut timings) = prepare_typed_frontier(source, &child)?;
            let (built, build_sources) =
                timed(&mut timings, BUILD_AND_CHECKED_CONTINUATION, |timings| {
                    evaluate_build_and_continue(&root_path, child, frontier, timings)
                })?;
            let prepared = PreparedBuild {
                built,
                build_sources,
                timings,
            };
            builds.retain_completed(inputs, &prepared);
            prepared
        }
    };
    let PreparedBuild {
        built,
        build_sources,
        mut timings,
    } = prepared;
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

    // Product ownership admits the stop and retains report companions before
    // the selected stage sequence consumes the checked compilation.
    let trust_settlement = checked.admit_trust(target.accepted_trust_admissions())?;
    let product = checked.prepare_product_report(
        root_path.clone(),
        shared.requested_product,
        target.optimization_rollback(),
        trust_settlement,
    )?;
    match product {
        CheckedProductReport::Complete(report) => Ok(report),
        // Psi 05-07: lower, optimize and publish the Terminal artifact.
        CheckedProductReport::Terminal(settlement) => settlement.finish(produce_terminal_report(
            root_path,
            checked,
            target.terminal_admission_profile(),
            target.optimization_rollback(),
        )?),
        // The same retained Terminal boundary, then Omega 00-09 and image emission.
        CheckedProductReport::Native(settlement) => settlement.finish(produce_native_report(
            target.into_native_product_request(),
            checked,
        )?),
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
