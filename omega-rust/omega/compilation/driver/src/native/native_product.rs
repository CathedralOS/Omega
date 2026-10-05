//! Native product preparation and realization after checked-Psi admission.

use crate::checked::CheckedCompilation;
use diagnostics::Diagnostic;

use compilation_products::NativeProductRequest;

/// What one native stage sequence produces for the admitted settlement to turn
/// into a report: the realized image, the settled rollback receipt, the
/// post-production timing ladder and the producer-side rewrite explanation.
pub struct ProducedNativeProduct {
    pub artifact: native_artifact::RequestedNativeArtifact,
    pub rollback_receipt: Option<optimization_core::OptimizationRollbackReceipt>,
    pub timings: Vec<artifacts::compile_timings::PhaseTiming>,
    pub withheld_rewrites: Vec<optimization_core::WithheldRewrite>,
}

/// Produce the same retained Terminal product used by package re-entry, then
/// consume it through the single source-free native pipeline. The stop and its
/// Build requirements were admitted by
/// `CheckedCompilation::prepare_product_report`; nothing is re-admitted here.
pub fn produce_native_product(
    request: NativeProductRequest,
    mut checked: CheckedCompilation,
) -> Result<ProducedNativeProduct, Vec<Diagnostic>> {
    let rollback = request
        .optimization_rollback
        .settle(&checked.build().config.settings.optimizations);

    // Capture package acceptance from the checked source, independently of the
    // proposal being produced. These are NOT receiver permission rows: the
    // separately supplied receiver policy remains optional and unchanged.
    let accepted_package_terminal_authority_permission_policy =
        accepted_terminal_authority_permission_policy(&checked)?;
    let terminal =
        crate::terminal::produce_retained_terminal_artifact(&mut checked, rollback.effective())?;
    let post_terminal = rollback.effective().project_post_terminal();
    // The same Psi passes ran here, so a native product explains its
    // withheld rewrites exactly as a Terminal product does.
    let withheld_rewrites = terminal.withheld_rewrites;
    let artifact = crate::native::realize_retained_native_artifact(
        terminal.artifact,
        compilation_products::RetainedNativeRealizationRequest {
            profile: &request.terminal_admission_profile,
            optimization_selections: post_terminal.selections(),
            terminal_authority_policy: request.terminal_authority_policy,
            accepted_package_terminal_authority_permission_policy,
            terminal_authority_permission_policy: request.terminal_authority_permission_policy,
            image_request: image_emission::ExecutableImageEmissionRequest::direct(
                checked.build().config.settings.subsystem,
            ),
            imports: &[],
        },
    )
    .map_err(|(_, diagnostics)| diagnostics)?;
    Ok(ProducedNativeProduct {
        artifact,
        rollback_receipt: rollback.into_receipt(),
        timings: checked.timings().phases().to_vec(),
        withheld_rewrites,
    })
}

/// Project and validate the checked package's accepted Terminal permission
/// rows before native proposal production. These are package evidence, not
/// the independently supplied receiver-admission policy.
fn accepted_terminal_authority_permission_policy(
    checked: &CheckedCompilation,
) -> Result<
    terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
    Vec<Diagnostic>,
> {
    use terminal_psi_to_abstract_operations::provider_admission::{
        terminal_authority_permission_policy_with_rows,
        terminal_authority_permissions::validate_package_terminal_authority_permission_custody,
    };
    let permissions = checked.execution().terminal_authority_permissions();
    validate_package_terminal_authority_permission_custody(permissions.iter())?;
    terminal_authority_permission_policy_with_rows(permissions).map_err(|error| {
        vec![Diagnostic::error(format!(
            "checked package terminal-authority permissions are invalid: {error:?}"
        ))]
    })
}
