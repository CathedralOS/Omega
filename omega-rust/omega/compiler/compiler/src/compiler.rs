//! Compile shared source into independently checked target products.
//!
//! The target loop owns sequencing and failure isolation. Product owners build
//! their artifacts and reports; native input reuse belongs to this invocation.

use crate::pipeline::checked_entry::PreparedCheckedSource;
use crate::{
    CompileOutcomes, CompileReport, CompileRequest, CompileTargetOutcome, RequestedCompileProduct,
    admit_checked_compilation,
};
use diagnostics::Diagnostic;

pub(crate) mod admission;
pub(crate) mod execution;
mod intrinsic_settlements;
mod native;
mod native_checked;
pub(crate) mod optimization;
pub(crate) mod options;
pub(crate) mod package;
pub(crate) mod request;
mod terminal_authority_permissions;
pub(crate) mod terminal_native_realization;
pub(crate) mod terminal_product;

/// Compile every requested target, retaining failures alongside successful products.
/// Invalid requests reject before source acquisition. Shared preparation failures
/// are reported for every target. Publication remains a separate operation.
pub fn compile(request: CompileRequest) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    execution::run_on_compile_thread(move || {
        let request = request.validate_for_execution()?;
        let source = PreparedCheckedSource::prepare(
            &request.shared.root_path,
            request.shared.package_sources.clone(),
        );
        let target_count = request.targets.len();
        let mut native_inputs = native::NativeInputReuse::default();
        let mut outcomes = Vec::with_capacity(target_count);

        // Checkpoint clones share immutable parsing. repeat_n moves the final
        // copy, so the last (and the only) target can consume the original arenas.
        let sources = std::iter::repeat_n(source, target_count);
        for (target, source) in request.targets.into_iter().zip(sources) {
            let profile = target.profile;
            let compile_target = || {
                let checked = source?.check(target.options(), target.package_inputs())?;
                let admission =
                    admit_checked_compilation(&checked, target.accepted_trust_admissions())?;
                admission.write_observations(target.options(), target.artifact_policy())?;
                let trust_settlement = admission.into_settlement();

                let report = match request.shared.requested_product {
                    RequestedCompileProduct::Check => CompileReport::check_only(
                        target.options.root_path,
                        checked.source_file_count(),
                    )
                    .map_err(|message| vec![Diagnostic::error(message)])?,
                    RequestedCompileProduct::TerminalArtifact => terminal_product::compile_report(
                        target.options.root_path,
                        checked,
                        &target.configuration.terminal_admission_profile,
                        &target.configuration.optimization_rollback,
                    )?,
                    RequestedCompileProduct::NativeArtifact => {
                        let terminal = native::prepare(target, checked)?;
                        native_inputs.realize(terminal)?
                    }
                };
                Ok(report.with_trust_admission_settlement(trust_settlement))
            };
            outcomes.push(CompileTargetOutcome::new(profile, compile_target()));
        }
        Ok(CompileOutcomes::new(outcomes)
            .with_prepared_terminal_native_input_count(native_inputs.prepared_input_count()))
    })
}

#[cfg(test)]
#[path = "compiler/tests.rs"]
mod tests;
