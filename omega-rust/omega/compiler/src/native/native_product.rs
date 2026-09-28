//! Native product preparation and realization after checked-Psi admission.

use crate::checked::{CheckedCompilation, OptimizationRollback};
use diagnostics::Diagnostic;

/// One target's native product request: where the source root is, the
/// admission profile the Terminal artifact is verified under, the receiving
/// mechanism-classification policy, the optional service-permission admission
/// policy, and the optimization rollback to settle.
///
/// The terminal-authority policy is the receiving authority's explicit
/// mechanism table: the closed compiler-intrinsic inventory classifies itself,
/// while every demanded normalized-foreign, syscall, or checked-physical leaf
/// needs exactly one exact explicit row. It is an independent authority axis --
/// trust admissions, service permissions, and provider execution custody
/// never substitute for or widen its rows.
///
/// `terminal_authority_permission_policy` is the second, receiver-admission
/// axis. `None` means ordinary production makes no receiver-admission claim:
/// accepted package permissions are never projected into receiver rows, and
/// the emitted artifact cannot satisfy an explicit admission replay. `Some`
/// (including an explicit empty policy) adjudicates every closure leaf
/// against exact supplied rows.
pub struct NativeProductRequest {
    pub root_path: std::path::PathBuf,
    pub terminal_admission_profile: proof_admission::AdmissionProfile,
    pub terminal_authority_policy: crate::native::TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy:
        Option<crate::native::TerminalAuthorityPermissionPolicy>,
    pub optimization_rollback: OptimizationRollback,
}

/// Produce the same retained Terminal product used by package re-entry, then
/// consume it through the single source-free native pipeline.
pub fn produce_native_report(
    request: NativeProductRequest,
    mut checked: CheckedCompilation,
) -> Result<crate::report::CompileReport, Vec<Diagnostic>> {
    let production_subject = checked.production_subject()?;
    let source_file_count = checked.source_file_count();
    let pcc_requests = checked.pcc_requests();
    let rollback = request
        .optimization_rollback
        .settle(checked.optimization_selections());
    provider_planning::component_progress::reject_undischarged_build_bound_progress(
        checked.component_progress(),
    )?;

    // Capture package acceptance from the checked source, independently of the
    // proposal being produced. These are NOT receiver permission rows: the
    // separately supplied receiver policy remains optional and unchanged.
    let permissions = checked
        .resolved_semantic_bindings()
        .flat_map(|binding| binding.terminal_authority_permissions())
        .cloned()
        .collect::<Vec<_>>();
    crate::native::validate_package_terminal_authority_permission_custody(permissions.iter())?;
    let accepted_package_terminal_authority_permission_policy =
        crate::native::terminal_authority_permission_policy_with_rows(permissions).map_err(
            |error| {
                vec![Diagnostic::error(format!(
                    "checked package terminal-authority permissions are invalid: {error:?}"
                ))]
            },
        )?;
    let terminal = crate::terminal::produce_retained_terminal_artifact(
        &mut checked,
        &request.terminal_admission_profile,
        rollback.effective(),
    )?;
    let post_terminal = rollback.effective().project_post_terminal();
    let artifact = crate::native::realize_retained_native_artifact(
        terminal,
        crate::native::RetainedNativeRealizationRequest {
            profile: &request.terminal_admission_profile,
            optimization_selections: post_terminal.selections(),
            terminal_authority_policy: request.terminal_authority_policy,
            accepted_package_terminal_authority_permission_policy,
            terminal_authority_permission_policy: request.terminal_authority_permission_policy,
            image_request: crate::native::ExecutableImageEmissionRequest::direct(
                checked.subsystem(),
            ),
            imports: &[],
        },
    )
    .map_err(|(_, diagnostics)| diagnostics)?
    .into_direct()
    .map_err(|_| {
        vec![Diagnostic::error(
            "direct native production returned a different image carrier",
        )]
    })?;
    let report = crate::report::CompileReport::from_retained_native_artifact(
        request.root_path,
        source_file_count,
        artifact,
        rollback.into_receipt(),
        production_subject,
    )
    .map(|report| report.with_pcc_context(pcc_requests, request.terminal_admission_profile))
    .and_then(|report| {
        report.with_application_metadata(checked.project_native_application_metadata())
    })
    .map_err(|message| vec![Diagnostic::error(message)])?;
    report
        .verify_native_source_metadata(
            checked.source_file_count(),
            checked.selected_target_profile(),
            checked.selected_native_target(),
            &checked.project_native_application_metadata(),
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
    Ok(report)
}
