use super::request::ValidatedCompileRequest;
use super::{
    CompileReport, CompileRequest, NativeCompilationWithCheckedReceipt, RequestedCompileProduct,
    TrustAdmissionSettlement,
};
use psi_diagnostics::Diagnostic;

/// Drive the one production route and stop at the requested product.
///
/// Check, Terminal Psi, and retained native artifacts share one checked-Psi
/// frontend and differ only in how far the result proceeds.
pub(super) fn compile(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    if request.requested_product() == RequestedCompileProduct::NativeArtifact {
        return compile_native_with_checked_receipt(request)
            .map(NativeCompilationWithCheckedReceipt::into_report);
    }
    let request = request.validate_for_execution()?;
    let (checked, trust_settlement) = compile_checked_with_observations(&request)?;
    let report = match request.requested_product() {
        RequestedCompileProduct::Check => checked_report(request, &checked),
        RequestedCompileProduct::TerminalArtifact => terminal_report(request, checked),
        RequestedCompileProduct::NativeArtifact => unreachable!(
            "native requests enter the checked-receipt route before frontend compilation"
        ),
    }?;
    Ok(report.with_trust_admission_settlement(trust_settlement))
}

pub(super) fn compile_native_with_checked_receipt(
    request: CompileRequest,
) -> Result<NativeCompilationWithCheckedReceipt, Vec<Diagnostic>> {
    let request = request.validate_for_native_execution()?;
    let (checked, trust_settlement) = compile_checked_with_observations(&request)?;
    let report = super::optimization::native_report(request, &checked)?
        .with_trust_admission_settlement(trust_settlement);
    NativeCompilationWithCheckedReceipt::new(checked, report)
        .map_err(|message| vec![Diagnostic::error(message)])
}

fn compile_checked_with_observations(
    request: &ValidatedCompileRequest,
) -> Result<
    (
        crate::pipeline::CheckedCompilation,
        TrustAdmissionSettlement,
    ),
    Vec<Diagnostic>,
> {
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
    Ok((checked, trust_settlement))
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
    let production_subject = crate::pipeline::reporting::project_production_subject(&checked)?;
    let source_file_count = checked.source_file_count();
    let callback_placements = checked.callback_placements().to_vec();
    let entry_machine = checked
        .selected_program_entry_machine()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "terminal-artifact production requires one exact selected program entry",
            )]
        })?
        .to_owned();
    omega_selected_dispatch::validate_selected_operator_terminal_custody(
        &checked,
        checked.selected_provider_plans(),
    )?;
    let produced = psi_checked_trees_to_terminal::produce_terminal_artifact_with_callback_custody(
        &checked,
        &entry_machine,
        callback_placements,
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "terminal-artifact production failed: {}",
            error.error(),
        ))]
    })?;
    let (artifact, callback_placements) = produced.into_parts();
    let artifact =
        omega_compilation_report::RetainedTerminalArtifact::new(artifact, callback_placements)
            .map_err(|message| vec![Diagnostic::error(message)])?;
    CompileReport::from_retained_terminal_artifact(
        request.options.root_path,
        source_file_count,
        artifact,
        production_subject,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}
