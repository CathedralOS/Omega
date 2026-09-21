//! Compile shared source into independently checked target products.
//!
//! The target loop owns sequencing and failure isolation. Each checked
//! compilation joins trust settlement (`admit_checked_compilation`) and the
//! product fences (`admit_requested_product`) before dispatch. Product owners
//! build their artifacts and reports; native input reuse belongs to this
//! invocation.

use crate::{
    CompileOutcomes, CompileReport, CompileRequest, CompileTargetOutcome, OptimizationRollback,
    RequestedCompileProduct, admit_checked_compilation,
};
use assembled_syntax_to_checked_compilation::{
    CheckedCompilation, PreparedCheckedSource, run_on_compile_thread,
};
use checked_compilation_to_terminal_artifact::produce_terminal_report;
use diagnostics::Diagnostic;
use native_realization::{NativeInputReuse, prepare_native_product};

pub(crate) mod options;
pub(crate) mod package;
pub(crate) mod request;

/// Compile every requested target, retaining failures alongside successful products.
/// Invalid requests reject before source acquisition. Shared preparation failures
/// are reported for every target. Publication remains a separate operation.
pub fn compile(request: CompileRequest) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    run_on_compile_thread(move || {
        let request = request.validate_for_execution()?;
        let source = PreparedCheckedSource::prepare(
            &request.shared.root_path,
            request.shared.package_sources.clone(),
            request.shared.timings,
        );
        let target_count = request.targets.len();
        let mut native_inputs = NativeInputReuse::default();
        let mut outcomes = Vec::with_capacity(target_count);

        // Checkpoint clones share immutable parsing. repeat_n moves the final
        // copy, so the last (and the only) target can consume the original arenas.
        let sources = std::iter::repeat_n(source, target_count);
        for (target, source) in request.targets.into_iter().zip(sources) {
            let profile = target.profile;
            let compile_target = || {
                let options = target.options();
                let checked = source?.check(
                    &options.root_path,
                    options.target_name.as_deref(),
                    options.build_dir(),
                    target.package_inputs(),
                    &target.configuration.optimization_rollback,
                    target.configuration.build_snapshot.as_ref(),
                )?;
                let stage_timings = checked.timings().phases().to_vec();
                let admission =
                    admit_checked_compilation(&checked, target.accepted_trust_admissions())?;
                let trust_settlement = admission.into_settlement();
                let build_observation = checked.build_observation_identity();

                // Build products are settled once, then carried alongside the
                // requested compiler product. A check-only stop publishes none.
                let build_outputs =
                    if request.shared.requested_product != RequestedCompileProduct::Check {
                        checked.completed_build_outputs()?
                    } else {
                        None
                    };
                admit_requested_product(
                    &checked,
                    request.shared.requested_product,
                    &target.configuration.optimization_rollback,
                )?;
                if checked.application_artifact_only()
                    && request.shared.requested_product != RequestedCompileProduct::Check
                {
                    let outputs = build_outputs.ok_or_else(|| {
                        vec![Diagnostic::error("artifact-only build completed no files")]
                    })?;
                    return CompileReport::from_build_outputs(
                        target.options.root_path,
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

                let report = match request.shared.requested_product {
                    RequestedCompileProduct::Check => CompileReport::check_only(
                        target.options.root_path,
                        checked.source_file_count(),
                    )
                    .map_err(|message| vec![Diagnostic::error(message)])?,
                    RequestedCompileProduct::TerminalArtifact => produce_terminal_report(
                        target.options.root_path,
                        checked,
                        &target.configuration.terminal_admission_profile,
                        &target.configuration.optimization_rollback,
                    )?,
                    RequestedCompileProduct::NativeArtifact => {
                        let terminal =
                            prepare_native_product(target.into_native_product_request(), checked)?;
                        native_inputs.realize(terminal)?
                    }
                };
                let report = if request.shared.requested_product != RequestedCompileProduct::Check {
                    report
                        .with_build_outputs(build_outputs, build_observation)
                        .map_err(|message| vec![Diagnostic::error(message)])?
                } else {
                    report
                };
                Ok(report
                    .with_trust_admission_settlement(trust_settlement)
                    .with_timings(stage_timings))
            };
            outcomes.push(CompileTargetOutcome::new(profile, compile_target()));
        }
        Ok(CompileOutcomes::new(outcomes)?
            .with_prepared_terminal_native_input_count(native_inputs.prepared_input_count()))
    })
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

#[cfg(test)]
#[path = "compiler/tests.rs"]
mod tests;
