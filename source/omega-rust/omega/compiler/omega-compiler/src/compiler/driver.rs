use super::request::ValidatedCompileRequest;
use super::{CompileReport, CompileRequest, RequestedCompileProduct};
use psi_diagnostics::Diagnostic;

/// Drive the one production route and stop at the requested product.
///
/// Check, Terminal Psi, and retained native artifacts share one checked-Psi
/// frontend and differ only in how far the result proceeds.
pub(super) fn compile(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    let request = request.validate_for_execution()?;
    let checked = crate::pipeline::checked_entry::compile_to_checked_for_terminal(
        request.options(),
        request.package_inputs(),
    )?;
    let trust_settlement = crate::pipeline::reporting::report_checked_observations(
        crate::pipeline::reporting::CheckedObservationInput {
            options: request.options(),
            artifact_policy: request.artifact_policy(),
            accepted_trust_admissions: request.accepted_trust_admissions(),
            checked: &checked,
        },
    )?;
    let report = match request.requested_product() {
        RequestedCompileProduct::Check => checked_report(request, &checked),
        RequestedCompileProduct::TerminalArtifact => terminal_report(request, checked),
        RequestedCompileProduct::NativeArtifact => native_report(request, checked),
    }?;
    Ok(report.with_trust_admission_settlement(trust_settlement))
}

fn checked_report(
    request: ValidatedCompileRequest,
    checked: &crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let request = request.into_inner();
    CompileReport::checked(
        request.options.root_path,
        checked.source_file_count(),
        false,
        super::CompileOutputKind::CheckOnly,
        None,
        None,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn terminal_report(
    request: ValidatedCompileRequest,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let request = request.into_inner();
    reject_unconsumed_callback_placements("terminal-artifact", &checked)?;
    let production_subject =
        crate::pipeline::reporting::project_production_subject(&checked)?;
    let source_file_count = checked.source_file_count();
    let entry_machine = checked
        .selected_program_entry_machine()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "terminal-artifact production requires one exact selected program entry",
            )]
        })?
        .to_owned();
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
        production_subject,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn native_report(
    request: ValidatedCompileRequest,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let request = request.into_inner();
    reject_unconsumed_callback_placements("native-artifact", &checked)?;
    let production_subject =
        crate::pipeline::reporting::project_production_subject(&checked)?;
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
    crate::pipeline::component_progress::reject_undischarged_build_bound_progress(
        checked.component_progress(),
    )?;
    let optimization_rollback = request
        .optimization_rollback
        .reconcile(checked.optimization_selections());
    let effective_optimizations = optimization_rollback.as_ref().map_or_else(
        || checked.optimization_selections(),
        |receipt| receipt.effective(),
    );
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
            optimization_selections: effective_optimizations,
            selected_provider_plans: checked.selected_provider_plans(),
            settlements: &[],
        },
    )?;
    CompileReport::from_retained_native_artifact(
        request.options.root_path,
        source_file_count,
        native_artifact,
        optimization_rollback,
        production_subject,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn reject_unconsumed_callback_placements(
    product: &str,
    checked: &crate::pipeline::CheckedCompilation,
) -> Result<(), Vec<Diagnostic>> {
    let placements = checked.callback_placements();
    if placements.is_empty() {
        return Ok(());
    }

    let requirements = placements
        .iter()
        .map(|placement| format!("`{}`", placement.canonical_requirement_overload))
        .collect::<Vec<_>>()
        .join(", ");
    Err(vec![Diagnostic::error(format!(
        "{product} production cannot discard {} validated callback placement(s) for {requirements}; canonical Terminal callback-use custody is not implemented",
        placements.len()
    ))])
}
