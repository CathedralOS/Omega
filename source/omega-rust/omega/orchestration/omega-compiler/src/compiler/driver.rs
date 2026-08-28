use super::{CompileReport, CompileRequest, RequestedCompileProduct};
use psi_diagnostics::Diagnostic;

/// Drive the one production route and stop at the requested product.
///
/// Installed output is the sole remaining StateGraph compatibility branch.
/// Check, Terminal Psi, and retained native artifacts share one checked-Psi
/// frontend and differ only in how far the result proceeds.
pub(super) fn compile(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    if request.requested_product == RequestedCompileProduct::InstalledOutput {
        return crate::pipeline::legacy_driver::LegacyDriver::from_request(request)
            .compile_installed_output();
    }

    let checked = crate::pipeline::compile_to_checked_for_terminal(
        &request.options,
        request.package_inputs.as_ref(),
    )?;
    match request.requested_product {
        RequestedCompileProduct::Check => checked_report(request, &checked),
        RequestedCompileProduct::TerminalArtifact => terminal_report(request, checked),
        RequestedCompileProduct::NativeArtifact => native_report(request, checked),
        RequestedCompileProduct::InstalledOutput => unreachable!("handled before Psi checking"),
    }
}

fn checked_report(
    request: CompileRequest,
    checked: &crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    CompileReport::checked(
        request.options.root_path,
        checked.source_file_count(),
        false,
        super::CompileOutputKind::CheckOnly,
        None,
        None,
        None,
        None,
        checked.build_evaluation_usage(),
        checked.build_observation_summary().cloned(),
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn terminal_report(
    request: CompileRequest,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let source_file_count = checked.source_file_count();
    let entry_machine = checked
        .selected_program_entry_machine()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "terminal-artifact production requires one exact selected program entry",
            )]
        })?
        .to_owned();
    let build_evaluation_usage = checked.build_evaluation_usage();
    let build_observation_summary = checked.build_observation_summary().cloned();
    let artifact =
        psi_checked_trees_to_terminal::produce_terminal_artifact(&checked, &entry_machine)
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "terminal-artifact production failed: {error}"
                ))]
            })?;
    CompileReport::from_terminal_artifact(
        request.options.root_path,
        source_file_count,
        artifact,
        build_evaluation_usage,
        build_observation_summary,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn native_report(
    request: CompileRequest,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let source_file_count = checked.source_file_count();
    let entry_machine = checked
        .selected_program_entry_machine()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "native-artifact production requires one exact selected program entry",
            )]
        })?
        .to_owned();
    let target = checked.selected_native_target().ok_or_else(|| {
        vec![Diagnostic::error(
            "native-artifact production requires one exact selected native target",
        )]
    })?;
    if checked
        .component_progress()
        .is_some_and(|manifest| !manifest.pending().is_empty())
    {
        return Err(vec![Diagnostic::error(
            "native-artifact production cannot discard pending build-bound component progress; request component staging with explicit establishment evidence",
        )]);
    }
    let build_evaluation_usage = checked.build_evaluation_usage();
    let build_observation_summary = checked.build_observation_summary().cloned();
    let artifact =
        psi_checked_trees_to_terminal::produce_terminal_artifact(&checked, &entry_machine)
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "native-artifact Terminal production failed: {error}"
                ))]
            })?;
    let native_artifact = crate::pipeline::realize_terminal_native_artifact(
        artifact,
        target,
        checked.subsystem(),
        &request.terminal_admission_profile,
        checked.optimization_selections(),
        checked.selected_provider_plans(),
        &[],
    )?;
    CompileReport::from_retained_native_artifact(
        request.options.root_path,
        source_file_count,
        native_artifact,
        build_evaluation_usage,
        build_observation_summary,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}
