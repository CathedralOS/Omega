use super::{CompileReport, CompileRequest, RequestedCompileProduct};
use psi_diagnostics::Diagnostic;

/// Drive the one production route and stop at the requested product.
///
/// Check, Terminal Psi, and retained native artifacts share one checked-Psi
/// frontend and differ only in how far the result proceeds.
pub(super) fn compile(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    let checked = crate::pipeline::checked_entry::compile_to_checked_for_terminal(
        &request.options,
        request.package_inputs.as_ref(),
    )?;
    settle_checked_trust(&request, &checked)?;
    match request.requested_product {
        RequestedCompileProduct::Check => checked_report(request, &checked),
        RequestedCompileProduct::TerminalArtifact => terminal_report(request, checked),
        RequestedCompileProduct::NativeArtifact => native_report(request, checked),
    }
}

fn settle_checked_trust(
    request: &CompileRequest,
    checked: &crate::pipeline::CheckedCompilation,
) -> Result<(), Vec<Diagnostic>> {
    let prepared = omega_trust_ledger::prepare_trust_lockfile(
        &request.options.root_path,
        &checked.typed,
        checked.root_grants(),
        checked.provider_plans(),
        checked.selected_provider_plans(),
        checked.accepted_template_classifications(),
        checked.package_identity().is_some(),
    )?;
    omega_trust_ledger::enforce_trust_lockfile(prepared, checked)?;
    omega_trust_ledger::write_trust_report(
        &request.options.build_dir(),
        checked,
        checked.root_grants(),
        checked.provider_plans(),
        checked.selected_provider_plans(),
        checked.accepted_template_classifications(),
        request.artifact_policy.emits_auxiliary_artifacts(),
    )?;
    if request.artifact_policy.emits_auxiliary_artifacts() {
        crate::pipeline::write_checked_snapshot(
            &request.options,
            checked,
            checked.selected_program_entry_machine(),
            checked.selected_provider_plans(),
            checked.task_activations(),
            checked.component_progress(),
        )?;
    }
    Ok(())
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
    CompileReport::from_artifact(
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
    let selected_program_entry = checked.selected_program_entry().ok_or_else(|| {
        vec![Diagnostic::error(
            "native-artifact production requires one exact selected program entry",
        )]
    })?;
    let entry_machine = selected_program_entry.machine_name().to_owned();
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
    let calling_plans = selected_program_entry
        .calling_plans()
        .map(|plans| (&plans.semantic_boundary_entry_plan, &plans.storage_entry));
    let program_entry = omega_terminal_psi_to_native_artifact::NativeProgramEntrySettlement::new(
        selected_program_entry.source_signature(),
        calling_plans,
    );
    let native_artifact = omega_terminal_psi_to_native_artifact::realize_native_artifact(
        artifact,
        omega_terminal_psi_to_native_artifact::NativeRealizationRequest {
            target,
            subsystem: checked.subsystem(),
            profile: &request.terminal_admission_profile,
            program_entry,
            optimization_selections: checked.optimization_selections(),
            selected_provider_plans: checked.selected_provider_plans(),
            settlements: &[],
        },
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
