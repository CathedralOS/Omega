use super::native_checked::NativeCompilationWithCheckedReceipt;
use super::request::ValidatedCompileRequest;
use super::{
    CompileReport, CompileRequest, ExactTargetCompileOutcome, MultiTargetCompileOutcomes,
    MultiTargetCompileRequest, RequestedCompileProduct, TrustAdmissionSettlement,
};
use psi_diagnostics::Diagnostic;
use std::path::PathBuf;

/// Drive the one production route and stop at the requested product.
///
/// Check, Terminal Psi, and retained native artifacts share one checked-Psi
/// frontend and differ only in how far the result proceeds.
pub(super) fn compile(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    let request = request.validate_for_execution()?;
    compile_validated(request, None)
}

/// Compile every canonical exact child while retaining each ordinary result.
///
/// Request-shape failures reject before source acquisition. Once admitted, a
/// shared source-preparation failure and every child-local failure remain one
/// ordered outcome per requested target.
pub(super) fn compile_targets(
    request: MultiTargetCompileRequest,
) -> Result<MultiTargetCompileOutcomes, Vec<Diagnostic>> {
    let request = request.validate_batch_for_execution()?;
    let (target_set, children) = request.into_parts();
    let prepared = {
        let first = children
            .first()
            .expect("validated explicit target set retains one child");
        crate::pipeline::checked_entry::PreparedCheckedSource::prepare(
            &first.options().root_path,
            first.package_inputs(),
        )
    };
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(diagnostics) => {
            let outcomes = target_set
                .profiles()
                .iter()
                .copied()
                .map(|target| ExactTargetCompileOutcome::new(target, Err(diagnostics.clone())))
                .collect();
            return Ok(MultiTargetCompileOutcomes::new(target_set, outcomes));
        }
    };
    let outcomes = target_set
        .profiles()
        .iter()
        .copied()
        .zip(children)
        .map(|(target, child)| {
            ExactTargetCompileOutcome::new(target, compile_validated(child, Some(&prepared)))
        })
        .collect();
    Ok(MultiTargetCompileOutcomes::new(target_set, outcomes))
}

fn compile_validated(
    request: ValidatedCompileRequest,
    prepared: Option<&crate::pipeline::checked_entry::PreparedCheckedSource>,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let (checked, trust_settlement) = compile_checked_with_observations(&request, prepared)?;
    let finalize_report =
        |report: CompileReport| report.with_trust_admission_settlement(trust_settlement);
    match request.requested_product() {
        RequestedCompileProduct::Check => checked_report(request, &checked).map(finalize_report),
        RequestedCompileProduct::TerminalArtifact => {
            terminal_report(request, checked).map(finalize_report)
        }
        RequestedCompileProduct::NativeArtifact => {
            let report = finalize_report(super::optimization::native_report(request, &checked)?);
            NativeCompilationWithCheckedReceipt::new(checked, report)
                .map(NativeCompilationWithCheckedReceipt::into_report)
                .map_err(|message| vec![Diagnostic::error(message)])
        }
    }
}

fn compile_checked_with_observations(
    request: &ValidatedCompileRequest,
    prepared: Option<&crate::pipeline::checked_entry::PreparedCheckedSource>,
) -> Result<
    (
        crate::pipeline::CheckedCompilation,
        TrustAdmissionSettlement,
    ),
    Vec<Diagnostic>,
> {
    let checked = match prepared {
        Some(prepared) => {
            prepared.compile_for_terminal(request.options(), request.package_inputs())?
        }
        None => crate::pipeline::checked_entry::compile_to_checked_for_terminal(
            request.options(),
            request.package_inputs(),
        )?,
    };
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
    let profile = request.terminal_admission_profile;
    retained_terminal_report(request.options.root_path, checked, &profile, false)
}

/// Consume one already-checked package production into its retained Terminal
/// report without reopening source discovery or build evaluation.
///
/// Package review owns construction of the checked value and later admission
/// owns native realization. This handoff only closes the compiler-owned
/// checked-to-Terminal boundary.
pub(super) fn retained_terminal_report_from_checked_package(
    root_path: PathBuf,
    checked: crate::pipeline::CheckedCompilation,
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<CompileReport, Vec<Diagnostic>> {
    retained_terminal_report(root_path, checked, profile, true)
}

fn retained_terminal_report(
    root_path: PathBuf,
    checked: crate::pipeline::CheckedCompilation,
    profile: &psi_proof_admission::AdmissionProfile,
    require_package_custody: bool,
) -> Result<CompileReport, Vec<Diagnostic>> {
    if require_package_custody {
        checked.verify_current_source_consumption()?;
    }
    let production_subject = crate::pipeline::reporting::project_production_subject(&checked)?;
    if require_package_custody && production_subject.is_none() {
        return Err(vec![Diagnostic::error(
            "reviewed package Terminal production requires package-aware checked custody",
        )]);
    }
    let source_file_count = checked.source_file_count();
    let artifact = super::terminal_product::produce_retained_terminal_artifact(&checked, profile)?;
    CompileReport::from_retained_terminal_artifact(
        root_path,
        source_file_count,
        artifact,
        production_subject,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}
